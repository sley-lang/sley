"""Model providers: Anthropic (Messages API, with Message Batches for first
attempts and a prompt-cache breakpoint on the system block), OpenAI (Responses
API, with the Batch API for first attempts), xAI and DeepSeek (chat
completions), over a pluggable transport.

`HttpTransport` is the only code that touches the network. `MockTransport`
answers the same endpoints in process (messages, message batches, files,
batches, responses, chat completions) so every request, batch and parsing path
runs in a dry run without a key or a paid call.
"""
import json, os, random, re, time, urllib.error, urllib.request, uuid

import accounting


class InfraError(Exception):
    """A failure of the provider or network, not of the model."""


class Reply:
    def __init__(self, text, usage, finish, batch, raw_id, model_reported):
        self.text = text            # the model's text ("" if none)
        self.usage = usage          # the provider's usage object, verbatim
        self.finish = finish        # "stop" | "length" | "error"
        self.batch = batch          # billed at the Batch rate
        self.raw_id = raw_id
        self.model_reported = model_reported


# ---- transports --------------------------------------------------------------

class HttpTransport:
    def __init__(self, timeout=300):
        self.timeout = timeout

    def request(self, method, url, headers, body=None):
        req = urllib.request.Request(url, data=body, method=method, headers=headers)
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as r:
                return r.status, r.read()
        except urllib.error.HTTPError as e:
            return e.code, e.read()
        except (urllib.error.URLError, TimeoutError, OSError) as e:
            raise InfraError(f"{method} {url}: {e}")


class MockTransport:
    """In-process stand-in for the Anthropic, OpenAI and xAI endpoints the
    harness uses.

    `answer(model, messages) -> (text, finish)` plays the model. Usage is
    synthesized from text length (about 4 characters per token) with the
    provider's field names; a repeated system prefix of 1,024+ tokens is
    reported as cached, as OpenAI and xAI do. On the Anthropic shape, a system
    block marked with cache_control and at least 512 tokens long is reported as
    a cache write the first time and a cache read after, as the Messages API
    does for Claude Haiku 5.5. Network calls: none."""

    def __init__(self, answer, batch_polls=1):
        self.answer = answer
        self.files = {}
        self.batches = {}
        self.calls = []
        self.batch_polls = batch_polls
        self.seen_prefixes = set()
        self.msg_batches = {}
        self.cache_marks = []

    @staticmethod
    def toks(text):
        return max(1, len(text) // 4)

    def _usage(self, shape, messages, text, max_out):
        prefix = messages[0]["content"] if messages and messages[0]["role"] in ("system", "developer") else ""
        prompt = sum(self.toks(m["content"]) for m in messages) + 4 * len(messages)
        p = self.toks(prefix)
        cached = (p // 128) * 128 if p >= 1024 and prefix in self.seen_prefixes else 0
        self.seen_prefixes.add(prefix)
        out = min(self.toks(text) + 40, max_out)  # +40 for hidden reasoning
        reasoning = 40 if out == max_out else min(40, out)
        if shape == "openai":
            return {"input_tokens": prompt, "input_tokens_details": {"cached_tokens": cached},
                    "output_tokens": out, "output_tokens_details": {"reasoning_tokens": reasoning},
                    "total_tokens": prompt + out}
        return {"prompt_tokens": prompt, "prompt_tokens_details": {"cached_tokens": cached},
                "completion_tokens": out, "completion_tokens_details": {"reasoning_tokens": reasoning},
                "total_tokens": prompt + out}

    def _response(self, body):
        msgs = body["input"]
        text, finish = self.answer(body["model"], msgs)
        usage = self._usage("openai", msgs, text, body.get("max_output_tokens", 10 ** 9))
        if finish == "length":
            text = text[: len(text) // 2]
        return {"id": "resp_" + uuid.uuid4().hex[:12], "object": "response", "model": body["model"],
                "status": "incomplete" if finish == "length" else "completed",
                "incomplete_details": {"reason": "max_output_tokens"} if finish == "length" else None,
                "output": [{"type": "reasoning", "summary": []},
                           {"type": "message", "role": "assistant",
                            "content": [{"type": "output_text", "text": text}]}],
                "usage": usage}

    def _message(self, body):
        system = body.get("system") or []
        sys_text = "".join(b["text"] for b in system)
        marked = any(b.get("cache_control") for b in system)
        msgs = [{"role": "system", "content": sys_text}] + [
            {"role": m["role"], "content": m["content"] if isinstance(m["content"], str)
             else "".join(c.get("text", "") for c in m["content"])} for m in body["messages"]]
        self.cache_marks.append(marked)
        text, finish = self.answer(body["model"], msgs)
        prompt = sum(self.toks(m["content"]) for m in msgs) + 4 * len(msgs)
        p = self.toks(sys_text)
        cr = cw = 0
        if marked and p >= 512:
            if sys_text in self.seen_prefixes:
                cr = p
            else:
                cw = p
        self.seen_prefixes.add(sys_text)
        max_out = body.get("max_tokens", 10 ** 9)
        out = min(self.toks(text) + 40, max_out)  # +40 for adaptive thinking, billed as output
        if finish == "length":
            text = text[: len(text) // 2]
        return {"id": "msg_" + uuid.uuid4().hex[:12], "type": "message", "role": "assistant", "model": body["model"],
                "content": [{"type": "thinking", "thinking": "", "signature": "x"}, {"type": "text", "text": text}],
                "stop_reason": "max_tokens" if finish == "length" else "end_turn", "stop_sequence": None,
                "usage": {"input_tokens": prompt - cr - cw, "cache_creation_input_tokens": cw,
                          "cache_read_input_tokens": cr, "output_tokens": out,
                          "cache_creation": {"ephemeral_5m_input_tokens": cw, "ephemeral_1h_input_tokens": 0}}}

    def _anthropic(self, method, path, headers, body):
        assert headers.get("anthropic-version") and headers.get("x-api-key"), headers
        if path == "/v1/messages" and method == "POST":
            return 200, json.dumps(self._message(json.loads(body))).encode()
        if path == "/v1/messages/batches" and method == "POST":
            b = json.loads(body)
            for r in b["requests"]:
                assert re.match(r"^[a-zA-Z0-9_-]{1,64}$", r["custom_id"]), r["custom_id"]
            bid = "msgbatch_" + uuid.uuid4().hex[:12]
            self.msg_batches[bid] = {"id": bid, "type": "message_batch", "processing_status": "in_progress",
                                     "request_counts": {"processing": len(b["requests"]), "succeeded": 0,
                                                        "errored": 0, "canceled": 0, "expired": 0},
                                     "results_url": None, "polls": 0, "_requests": b["requests"]}
            return 200, json.dumps({k: v for k, v in self.msg_batches[bid].items() if not k.startswith("_")}).encode()
        m = re.match(r"^/v1/messages/batches/([\w-]+)$", path)
        if m and method == "GET":
            bt = self.msg_batches[m.group(1)]
            bt["polls"] += 1
            if bt["polls"] >= self.batch_polls and bt["processing_status"] != "ended":
                lines = []
                for r in bt["_requests"]:
                    lines.append(json.dumps({"custom_id": r["custom_id"],
                                             "result": {"type": "succeeded", "message": self._message(r["params"])}}))
                bt["_results"] = ("\n".join(lines) + "\n").encode()
                n = len(lines)
                bt.update(processing_status="ended", results_url=f"https://api.anthropic.com/v1/messages/batches/{bt['id']}/results",
                          request_counts={"processing": 0, "succeeded": n, "errored": 0, "canceled": 0, "expired": 0})
            return 200, json.dumps({k: v for k, v in bt.items() if not k.startswith("_")}).encode()
        m = re.match(r"^/v1/messages/batches/([\w-]+)/results$", path)
        if m and method == "GET":
            return 200, self.msg_batches[m.group(1)]["_results"]
        return 404, json.dumps({"type": "error", "error": {"type": "not_found_error", "message": f"mock: no route {method} {path}"}}).encode()

    def request(self, method, url, headers, body=None):
        self.calls.append((method, url))
        path = re.sub(r"^https://[^/]+", "", url)
        if url.startswith("https://api.anthropic.com/"):
            return self._anthropic(method, path, headers, body)
        if path == "/v1/responses" and method == "POST":
            return 200, json.dumps(self._response(json.loads(body))).encode()
        if path == "/v1/chat/completions" and method == "POST":
            b = json.loads(body)
            text, finish = self.answer(b["model"], b["messages"])
            usage = self._usage("chat", b["messages"], text, b.get("max_tokens", 10 ** 9))
            if finish == "length":
                text = text[: len(text) // 2]
            return 200, json.dumps({"id": "chatcmpl-" + uuid.uuid4().hex[:12], "model": b["model"],
                                    "choices": [{"index": 0, "message": {"role": "assistant", "content": text},
                                                 "finish_reason": finish}], "usage": usage}).encode()
        if path == "/v1/files" and method == "POST":
            parts = body.split(b"\r\n\r\n")
            content = parts[-1].rsplit(b"\r\n--", 1)[0]
            fid = "file-" + uuid.uuid4().hex[:12]
            self.files[fid] = content
            return 200, json.dumps({"id": fid, "object": "file", "purpose": "batch", "bytes": len(content)}).encode()
        if path == "/v1/batches" and method == "POST":
            b = json.loads(body)
            assert b["endpoint"] == "/v1/responses", b
            bid = "batch_" + uuid.uuid4().hex[:12]
            self.batches[bid] = {"id": bid, "status": "validating", "input_file_id": b["input_file_id"],
                                 "polls": 0, "endpoint": b["endpoint"]}
            return 200, json.dumps(self.batches[bid]).encode()
        m = re.match(r"^/v1/batches/([\w-]+)$", path)
        if m and method == "GET":
            bt = self.batches[m.group(1)]
            bt["polls"] += 1
            if bt["polls"] >= self.batch_polls and bt["status"] != "completed":
                out = []
                for line in self.files[bt["input_file_id"]].decode().splitlines():
                    r = json.loads(line)
                    assert r["url"] == "/v1/responses" and r["method"] == "POST", r
                    out.append(json.dumps({"id": "batch_req_" + uuid.uuid4().hex[:8], "custom_id": r["custom_id"],
                                           "response": {"status_code": 200, "request_id": "req_x",
                                                        "body": self._response(r["body"])}, "error": None}))
                fid = "file-" + uuid.uuid4().hex[:12]
                self.files[fid] = ("\n".join(out) + "\n").encode()
                bt.update(status="completed", output_file_id=fid, error_file_id=None,
                          request_counts={"total": len(out), "completed": len(out), "failed": 0})
            elif bt["status"] == "validating":
                bt["status"] = "in_progress"
            return 200, json.dumps(bt).encode()
        m = re.match(r"^/v1/files/([\w-]+)/content$", path)
        if m and method == "GET":
            return 200, self.files[m.group(1)]
        return 404, json.dumps({"error": {"message": f"mock: no route {method} {path}"}}).encode()


# ---- providers ---------------------------------------------------------------

def _retrying(fn, attempts=4, base=2.0, sleep=time.sleep):
    """Retry rate limits and server errors; anything else is returned."""
    for i in range(attempts):
        status, body = fn()
        if status == 429 or status >= 500:
            if i == attempts - 1:
                raise InfraError(f"HTTP {status} after {attempts} tries: {body[:300]!r}")
            sleep(base * (2 ** i))
            continue
        return status, body


class OpenAIProvider:
    name = "openai"
    base = "https://api.openai.com"

    def __init__(self, model, transport, key=None, reasoning_effort="low", max_output=2000,
                 poll_seconds=30, sleep=time.sleep):
        self.model = model
        self.t = transport
        self.key = key or "mock-key"
        self.effort = reasoning_effort
        self.max_output = max_output
        self.poll_seconds = poll_seconds
        self.sleep = sleep

    def headers(self, ctype="application/json"):
        return {"Authorization": f"Bearer {self.key}", "Content-Type": ctype}

    def body(self, messages):
        b = {"model": self.model, "input": messages, "max_output_tokens": self.max_output, "store": False}
        if self.effort:
            b["reasoning"] = {"effort": self.effort}
        return b

    @staticmethod
    def parse(resp, batch):
        text = "".join(c.get("text", "") for item in resp.get("output", []) if item.get("type") == "message"
                       for c in item.get("content", []) if c.get("type") == "output_text")
        status = resp.get("status")
        reason = (resp.get("incomplete_details") or {}).get("reason")
        finish = "length" if status == "incomplete" and reason == "max_output_tokens" else (
            "stop" if status == "completed" else "error")
        return Reply(text, resp.get("usage") or {}, finish, batch, resp.get("id"), resp.get("model"))

    def complete(self, messages):
        status, body = _retrying(lambda: self.t.request("POST", self.base + "/v1/responses", self.headers(),
                                                        json.dumps(self.body(messages)).encode()), sleep=self.sleep)
        if status != 200:
            raise InfraError(f"responses HTTP {status}: {body[:300]!r}")
        return self.parse(json.loads(body), batch=False)

    # The Batch API: upload a JSONL of /v1/responses requests, create the batch,
    # poll it, download the output. Billed at the batch discount.
    def submit_batch(self, requests):
        """requests: [(custom_id, messages)] -> batch id."""
        lines = "".join(json.dumps({"custom_id": cid, "method": "POST", "url": "/v1/responses",
                                    "body": self.body(msgs)}) + "\n" for cid, msgs in requests).encode()
        boundary = "----sley" + uuid.uuid4().hex
        form = (f"--{boundary}\r\nContent-Disposition: form-data; name=\"purpose\"\r\n\r\nbatch\r\n"
                f"--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"batch.jsonl\"\r\n"
                f"Content-Type: application/jsonl\r\n\r\n").encode() + lines + f"\r\n--{boundary}--\r\n".encode()
        status, body = _retrying(lambda: self.t.request("POST", self.base + "/v1/files",
                                                        self.headers(f"multipart/form-data; boundary={boundary}"), form),
                                 sleep=self.sleep)
        if status != 200:
            raise InfraError(f"file upload HTTP {status}: {body[:300]!r}")
        fid = json.loads(body)["id"]
        status, body = _retrying(lambda: self.t.request("POST", self.base + "/v1/batches", self.headers(), json.dumps(
            {"input_file_id": fid, "endpoint": "/v1/responses", "completion_window": "24h",
             "metadata": {"benchmark": "sley-model-cost-2.1.8"}}).encode()), sleep=self.sleep)
        if status != 200:
            raise InfraError(f"batch create HTTP {status}: {body[:300]!r}")
        return json.loads(body)["id"]

    def wait_batch(self, batch_id, max_wait=26 * 3600):
        start = time.monotonic()
        while True:
            status, body = _retrying(lambda: self.t.request("GET", f"{self.base}/v1/batches/{batch_id}",
                                                            self.headers()), sleep=self.sleep)
            if status != 200:
                raise InfraError(f"batch poll HTTP {status}: {body[:300]!r}")
            b = json.loads(body)
            if b["status"] == "completed":
                return b
            if b["status"] in ("failed", "expired", "cancelled"):
                return b  # partial output may still exist and bills
            if time.monotonic() - start > max_wait:
                raise InfraError(f"batch {batch_id} still {b['status']} after {max_wait}s")
            self.sleep(self.poll_seconds)

    def batch_results(self, batch):
        """{custom_id: Reply or InfraError} for a finished batch."""
        out = {}
        for key in ("output_file_id", "error_file_id"):
            fid = batch.get(key)
            if not fid:
                continue
            status, body = _retrying(lambda: self.t.request("GET", f"{self.base}/v1/files/{fid}/content",
                                                            self.headers()), sleep=self.sleep)
            if status != 200:
                raise InfraError(f"batch download HTTP {status}")
            for line in body.decode().splitlines():
                if not line.strip():
                    continue
                r = json.loads(line)
                resp = r.get("response") or {}
                if resp.get("status_code") == 200:
                    out[r["custom_id"]] = self.parse(resp["body"], batch=True)
                else:
                    out[r["custom_id"]] = InfraError(f"batch item {r['custom_id']}: {r.get('error') or resp}")
        return out


class AnthropicProvider:
    """Claude through the Messages API. First attempts can go through the
    Message Batches API (billed at half price). The system block carries one
    5-minute cache breakpoint, so the shared text and the Sley card are a
    cacheable prefix; the API caches it only when it is at least the model's
    minimum (512 tokens on Claude Haiku 5.5), and reports cache writes and
    reads in usage, which is what gets billed."""
    name = "anthropic"
    base = "https://api.anthropic.com"
    version = "2023-06-01"

    def __init__(self, model, transport, key=None, effort="low", max_output=2000, poll_seconds=30,
                 sleep=time.sleep, cache=True):
        self.model = model
        self.t = transport
        self.key = key or "mock-key"
        self.effort = effort
        self.max_output = max_output
        self.poll_seconds = poll_seconds
        self.sleep = sleep
        self.cache = cache

    def headers(self):
        return {"x-api-key": self.key, "anthropic-version": self.version, "content-type": "application/json"}

    def body(self, messages):
        system = [m["content"] for m in messages if m["role"] in ("system", "developer")]
        b = {"model": self.model, "max_tokens": self.max_output,
             "messages": [{"role": m["role"], "content": m["content"]} for m in messages
                          if m["role"] in ("user", "assistant")]}
        if system:
            block = {"type": "text", "text": "\n\n".join(system)}
            if self.cache:
                block["cache_control"] = {"type": "ephemeral"}
            b["system"] = [block]
        if self.effort:
            b["output_config"] = {"effort": self.effort}
        return b

    @staticmethod
    def parse(msg, batch):
        text = "".join(c.get("text", "") for c in msg.get("content", []) if c.get("type") == "text")
        sr = msg.get("stop_reason")
        finish = "length" if sr == "max_tokens" else ("stop" if sr in ("end_turn", "stop_sequence", "refusal") else "error")
        return Reply(text, msg.get("usage") or {}, finish, batch, msg.get("id"), msg.get("model"))

    def complete(self, messages):
        status, body = _retrying(lambda: self.t.request("POST", self.base + "/v1/messages", self.headers(),
                                                        json.dumps(self.body(messages)).encode()), sleep=self.sleep)
        if status != 200:
            raise InfraError(f"messages HTTP {status}: {body[:300]!r}")
        return self.parse(json.loads(body), batch=False)

    def submit_batch(self, requests):
        """requests: [(custom_id, messages)] -> message batch id."""
        reqs = [{"custom_id": cid, "params": self.body(msgs)} for cid, msgs in requests]
        status, body = _retrying(lambda: self.t.request("POST", self.base + "/v1/messages/batches", self.headers(),
                                                        json.dumps({"requests": reqs}).encode()), sleep=self.sleep)
        if status != 200:
            raise InfraError(f"message batch create HTTP {status}: {body[:300]!r}")
        return json.loads(body)["id"]

    def wait_batch(self, batch_id, max_wait=26 * 3600):
        start = time.monotonic()
        while True:
            status, body = _retrying(lambda: self.t.request("GET", f"{self.base}/v1/messages/batches/{batch_id}",
                                                            self.headers()), sleep=self.sleep)
            if status != 200:
                raise InfraError(f"message batch poll HTTP {status}: {body[:300]!r}")
            b = json.loads(body)
            if b["processing_status"] == "ended":
                b["status"] = "completed"
                return b
            if time.monotonic() - start > max_wait:
                raise InfraError(f"message batch {batch_id} still {b['processing_status']} after {max_wait}s")
            self.sleep(self.poll_seconds)

    def batch_results(self, batch):
        """{custom_id: Reply or InfraError}. Errored, canceled and expired
        items are not billed and come back as infrastructure failures."""
        url = batch.get("results_url")
        if not url:
            return {}
        status, body = _retrying(lambda: self.t.request("GET", url, self.headers()), sleep=self.sleep)
        if status != 200:
            raise InfraError(f"message batch results HTTP {status}")
        out = {}
        for line in body.decode().splitlines():
            if not line.strip():
                continue
            r = json.loads(line)
            res = r.get("result") or {}
            if res.get("type") == "succeeded":
                out[r["custom_id"]] = self.parse(res["message"], batch=True)
            else:
                out[r["custom_id"]] = InfraError(f"batch item {r['custom_id']}: {res.get('type')} {res.get('error')}")
        return out


class ChatProvider:
    """xAI and DeepSeek: OpenAI-style chat completions."""

    def __init__(self, name, base, model, transport, key=None, max_output=2000, sleep=time.sleep):
        self.name = name
        self.base = base
        self.model = model
        self.t = transport
        self.key = key or "mock-key"
        self.max_output = max_output
        self.sleep = sleep

    def complete(self, messages):
        body = json.dumps({"model": self.model, "messages": messages, "max_tokens": self.max_output}).encode()
        status, raw = _retrying(lambda: self.t.request("POST", self.base + "/v1/chat/completions", {
            "Authorization": f"Bearer {self.key}", "Content-Type": "application/json"}, body), sleep=self.sleep)
        if status != 200:
            raise InfraError(f"chat HTTP {status}: {raw[:300]!r}")
        r = json.loads(raw)
        ch = (r.get("choices") or [{}])[0]
        fin = ch.get("finish_reason")
        finish = "length" if fin == "length" else ("stop" if fin in ("stop", "end_turn") else "error")
        return Reply((ch.get("message") or {}).get("content") or "", r.get("usage") or {}, finish, False,
                     r.get("id"), r.get("model"))


KEYS = {"anthropic": "ANTHROPIC_API_KEY", "openai": "OPENAI_API_KEY", "xai": "XAI_API_KEY", "deepseek": "DEEPSEEK_API_KEY"}
BASES = {"xai": "https://api.x.ai", "deepseek": "https://api.deepseek.com"}


def make(model, price, transport, paid, max_output, effort="low", sleep=time.sleep):
    """A provider for `model`. Paid providers need their key in the environment."""
    prov = price["provider"]
    key = None
    if paid:
        key = os.environ.get(KEYS[prov])
        if not key:
            raise SystemExit(f"--paid needs {KEYS[prov]} in the environment for {model}")
    api_model = price.get("api_model", model) if "api_model" in price else model
    if api_model is None:
        raise SystemExit(f"{model}: set api_model in prices.json before running it")
    if prov == "anthropic":
        return AnthropicProvider(api_model, transport, key, effort, max_output, sleep=sleep)
    if prov == "openai":
        return OpenAIProvider(api_model, transport, key, effort, max_output, sleep=sleep)
    return ChatProvider(prov, BASES[prov], api_model, transport, key, max_output, sleep=sleep)
