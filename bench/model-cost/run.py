#!/usr/bin/env python3
"""Run the genuine model cost benchmark (pre-registered in prereg.json).

  run.py --out DIR [--tier small|full] [--models claude-haiku-5-5,grok-build-0.1]
         [--langs ...] [--tasks ...] [--reps N] [--guard 10] [--jobs 4]
         [--mock | --paid] [--project]

Without --paid every call goes to the in-process MockTransport: no network,
no key, no spend (the dry run). --paid sends real, billed requests and needs
ANTHROPIC_API_KEY / XAI_API_KEY (OPENAI_API_KEY for the optional gpt-6-luna);
the claude-code model runs the logged-in `claude` CLI (locally, or over SSH with
CLAUDE_CODE_SSH=user@host) and needs no key; the spend guard stops the run before billed
spend could pass --guard dollars. --project prints the cost projection from
the plan's assumptions and real prices, and sends nothing.
"""
import argparse, concurrent.futures, hashlib, json, os, random, re, subprocess, sys, threading

import accounting, langs, providers, tasks, tasks_v2

HERE = os.path.dirname(os.path.abspath(__file__))
PREREG = json.load(open(os.path.join(HERE, "prereg.json")))
MAX_OUT = PREREG["output_cap_tokens"]
MAX_ATTEMPTS = PREREG["attempts"]["max"]
LANG_NAME = {"python": "Python 3", "sley": "Sley (Python dialect)", "c": "C (C17, gcc)", "rust": "Rust (2021 edition)",
             "go": "Go", "ts": "TypeScript (strict, ES2022 modules)"}

SYSTEM = ("You write one source file for an automated benchmark. Reply with exactly one fenced code block that "
          "holds the complete file, and nothing else. Use only the standard library. Do not write tests or a main "
          "function; the harness runs its own tests.")


# The harness grades with its own tests and tells models not to write any, so
# the Sley card is the dialect card without the inline-test syntax.
CARD_TOPIC = "python-task"


def card():
    out = subprocess.run([langs.sley_agent(), "help", CARD_TOPIC], capture_output=True, text=True)
    if out.returncode != 0 or not out.stdout.strip():
        raise RuntimeError(f"`sley-agent help {CARD_TOPIC}` failed; this sley-agent predates the task card: {out.stderr.strip()}")
    return out.stdout.strip()


def system_text(lang, sley_card):
    if lang != "sley":
        return SYSTEM
    return (SYSTEM + "\n\nThe language is Sley's Python dialect: typed Python that `sley-agent try --familiar` "
            "compiles to Sley. Errors are `raise ValueError(...)`. Its reference card:\n\n" + sley_card)


def task_prompt(task, lang):
    t = langs.task_def(task)
    lines = [f"Language: {LANG_NAME[lang]}", f"Write: {langs.signature(task, lang)}", "", t["statement"], ""]
    if t.get("edit"):
        starter = open(os.path.join(HERE, "starters", task, langs.SRC[lang])).read()
        lines += ["The existing file:", f"```{langs.FENCE[lang]}", starter.rstrip(), "```", ""]
    lines.append("Examples (the visible tests):")
    for args, want in t["visible"]:
        lines.append(f"  {langs.call(task, 'python' if lang == 'sley' else lang, args)} -> {langs.want_text(want)}")
    return "\n".join(lines)


FENCED = re.compile(r"```[\w+-]*[ \t]*\n(.*?)```", re.S)


def extract(text):
    blocks = FENCED.findall(text or "")
    return blocks[-1] if blocks else None


def retry_message(kind, fb=""):
    if kind == "truncated":
        return ("Your reply stopped at the output limit before the code block was complete. Reply with the complete "
                "file, shorter, in one fenced code block.")
    if kind == "no_code":
        return "Your reply had no fenced code block. Reply with the complete file in one fenced code block."
    return ("The visible tests failed. Output:\n```\n" + fb + "\n```\nReply with the complete corrected file in one "
            "fenced code block.")


# ---- the mock model (dry run only) --------------------------------------------

def mock_answer_factory(seed=218):
    """A scripted 'model' for dry runs: per (model, task, language) it is right
    first time, right after one failed attempt, cut off by the output cap, or
    never right; the code it returns is the reference program (or a broken
    version of it)."""
    by_fn = {}
    for t in tasks.TASKS + tasks_v2.TASKS:
        for l in langs.LANGS:
            by_fn[(langs.fn(t["name"], l), l)] = t["name"]
    lang_of = {v: k for k, v in LANG_NAME.items()}

    def answer(model, messages):
        user = [m["content"] for m in messages if m["role"] == "user"]
        first = user[0]
        if not re.search(r"^Language: ", first, re.M):
            return "OK", "stop"  # the claude-code calibration call
        lang = lang_of[re.search(r"^Language: (.*)$", first, re.M).group(1)]
        fname = re.search(r"^Write: (?:def |pub fn |func |export function |int |long long |void )(\w+)\(", first, re.M).group(1)
        task = by_fn[(fname, lang)]
        attempt = len(user)
        r = random.Random(f"{seed}:{model}:{task}:{lang}").random()
        ref = open(os.path.join(HERE, "refs", task, langs.SRC[lang])).read()
        broken = ref.replace("return", "return 1 + ", 1) if lang in ("python", "sley") else "this does not compile\n"
        if r < 0.60:
            code, finish = ref, "stop"
        elif r < 0.85:
            code, finish = (broken if attempt == 1 else ref), "stop"
        elif r < 0.92:
            code, finish = (ref * 40 if attempt == 1 else ref), ("length" if attempt == 1 else "stop")
        else:
            code, finish = broken, "stop"
        return f"```{langs.FENCE[lang]}\n{code}```", finish
    return answer


# ---- one trial ------------------------------------------------------------------

class Run:
    def __init__(self, args, prices):
        self.a = args
        self.prices = prices
        self.out = args.out
        os.makedirs(self.out, exist_ok=True)
        self.lock = threading.Lock()
        self.guard = accounting.SpendGuard(args.guard, os.path.join(self.out, "ledger.jsonl"))
        self.results = os.path.join(self.out, "results.jsonl")
        self.done = set()
        if os.path.exists(self.results):
            for line in open(self.results):
                e = json.loads(line)
                if e["kind"] == "trial":
                    self.done.add((e["model"], e["task"], e["lang"], e["rep"]))
        self.transport = (providers.HttpTransport() if args.paid
                          else providers.MockTransport(mock_answer_factory(), batch_polls=2))
        self.card = card() if "sley" in args.langs else ""

    def log(self, entry):
        with self.lock, open(self.results, "a") as f:
            f.write(json.dumps(entry) + "\n")

    def call(self, prov, model, messages, key, attempt, pre_reply=None, rid=None):
        """Send (or take the batch's) reply, settle billed cost, log the call."""
        price = self.prices["models"][model]
        if pre_reply is None:
            prompt = sum(accounting.estimate_tokens(m["content"]) for m in messages)
            rid = self.guard.reserve(accounting.worst_case(price, prompt, MAX_OUT), f"{model}/{'/'.join(map(str, key))}/a{attempt}")
            try:
                reply = prov.complete(messages)
            except providers.InfraError as e:
                self.guard.settle(rid, 0.0, "infra")
                self.log({"kind": "call", **self.ids(model, key), "attempt": attempt, "utc": accounting.utc_now().isoformat(),
                          "infra_error": str(e)[:500]})
                raise
        else:
            reply = pre_reply
        when = accounting.utc_now()
        tokens = accounting.normalize_usage(prov.name, reply.usage, price.get("reasoning_billed_as_output", True)
                                            if prov.name in ("openai", "anthropic", "claude-code") else self.a.reasoning_in_completion)
        cost = accounting.cost(price, tokens, batch=reply.batch, when=when)
        self.guard.settle(rid, cost, f"{model}/{'/'.join(map(str, key))}/a{attempt}")
        self.log({"kind": "call", **self.ids(model, key), "attempt": attempt, "utc": when.isoformat(),
                  "batch": reply.batch, "finish": reply.finish, "response_id": reply.raw_id,
                  "model_reported": reply.model_reported, "usage": reply.usage, "billed_tokens": tokens,
                  "cost_usd": round(cost, 8), "prices_checked": self.prices["checked"]})
        return reply, cost

    def calibrate(self, prov, model):
        """Measure what Claude Code adds to every call: one near-empty call,
        logged, guarded and counted in the spend like any other."""
        price = self.prices["models"][model]
        rid = self.guard.reserve(accounting.worst_case(price, 2000, MAX_OUT), f"{model}/calibration")
        try:
            reply = prov.calibrate()
        except providers.InfraError as e:
            self.guard.settle(rid, 0.0, "calibration infra")
            self.log({"kind": "calibration", "model": model, "infra_error": str(e)[:500],
                      "utc": accounting.utc_now().isoformat()})
            raise
        tokens = accounting.normalize_usage(prov.name, reply.usage)
        cost = accounting.cost(price, tokens)
        self.guard.settle(rid, cost, f"{model}/calibration")
        sent = "Reply with the single word OK." + "OK?"
        entry = {"kind": "calibration", "model": model, "utc": accounting.utc_now().isoformat(),
                 "argv": prov.argv("<system prompt>"), "env": prov.env(), "usage": reply.usage,
                 "billed_tokens": tokens, "cost_usd": round(cost, 8), "sent_chars": len(sent),
                 "overhead_input_tokens_estimate": tokens["input"] - max(1, len(sent) // 4),
                 "note": "input tokens of a call whose own text is about 10 tokens; the rest is what Claude Code adds"}
        self.log(entry)
        json.dump(entry, open(os.path.join(self.out, "calibration.json"), "w"), indent=2)
        print(f"{model}: calibration call: {tokens['input']} input tokens for ~{max(1, len(sent) // 4)} tokens of text",
              file=sys.stderr)

    @staticmethod
    def ids(model, key):
        task, lang, rep = key
        return {"model": model, "task": task, "lang": lang, "rep": rep}

    def trial(self, prov, model, key, first=None):
        """Attempts until the visible tests pass or the cap; then the oracle."""
        task, lang, rep = key
        work = os.path.join(self.out, "work", model, f"{task}-{lang}-{rep}")
        messages = [{"role": "system", "content": system_text(lang, self.card)},
                    {"role": "user", "content": task_prompt(task, lang)}]
        spent, attempts, program, passed, infra = 0.0, 0, None, False, None
        outcomes = []
        try:
            for attempt in range(1, MAX_ATTEMPTS + 1):
                if attempt == 1 and first is not None:
                    if isinstance(first[0], Exception):
                        raise first[0]
                    reply, cost = self.call(prov, model, messages, key, 1, pre_reply=first[0], rid=first[1])
                else:
                    reply, cost = self.call(prov, model, messages, key, attempt)
                spent += cost
                attempts = attempt
                code = extract(reply.text)
                if reply.finish == "length" or (code is None and reply.finish != "stop"):
                    kind, fb = "truncated", ""
                elif code is None:
                    kind, fb = "no_code", ""
                else:
                    program = code
                    ok, out = langs.check(task, lang, code, os.path.join(work, f"a{attempt}"))
                    if ok:
                        passed = True
                        outcomes.append("pass")
                        break
                    kind, fb = "failed", langs.feedback(out)
                outcomes.append(kind)
                messages += [{"role": "assistant", "content": reply.text},
                             {"role": "user", "content": retry_message(kind, fb)}]
        except accounting.GuardRefused as e:
            infra = f"guard: {e}"
        except providers.InfraError as e:
            infra = f"infra: {e}"
        result = {"passed": 0, "total": 128, "first_failure": None, "build_log": ""}
        if passed and program is not None:
            result = langs.oracle(task, lang, program, os.path.join(work, "oracle"))
        success = passed and result["passed"] == result["total"]
        entry = {"kind": "trial", **self.ids(model, key), "attempts": attempts, "outcomes": outcomes,
                 "visible_passed": passed, "hidden_passed": result["passed"], "hidden_total": result["total"],
                 "first_hidden_failure": result["first_failure"], "success": success,
                 "first_try": success and attempts == 1, "cost_usd": round(spent, 8), "infra": infra,
                 "utc": accounting.utc_now().isoformat()}
        self.log(entry)
        if program is not None:
            d = os.path.join(self.out, "programs", model)
            os.makedirs(d, exist_ok=True)
            open(os.path.join(d, f"{task}-{rep}.{langs.SRC[lang]}"), "w").write(program)
        return entry

    def run_model(self, model, keys):
        price = self.prices["models"][model]
        prov = providers.make(model, price, self.transport, self.a.paid, MAX_OUT, self.a.effort,
                              sleep=(lambda s: None) if not self.a.paid else __import__("time").sleep)
        keys = [k for k in keys if (model,) + k not in self.done]
        if prov.name == "claude-code" and keys:
            self.calibrate(prov, model)
        firsts = {}
        if price.get("batch") and hasattr(prov, "submit_batch") and not self.a.no_batch and keys:
            reqs, rids, key_of = [], {}, {}
            for k in keys:
                cid = f"{k[0]}-{k[1]}-{k[2]}"  # Anthropic custom_id: ^[a-zA-Z0-9_-]{1,64}$
                key_of[cid] = k
                msgs = [{"role": "system", "content": system_text(k[1], self.card)},
                        {"role": "user", "content": task_prompt(k[0], k[1])}]
                prompt = sum(accounting.estimate_tokens(m["content"]) for m in msgs)
                try:
                    rids[cid] = self.guard.reserve(accounting.worst_case(price, prompt, MAX_OUT, batch=True),
                                                   f"{model}/batch/{cid}")
                except accounting.GuardRefused:
                    break
                reqs.append((cid, msgs))
            print(f"{model}: Batch API, {len(reqs)} first attempts", file=sys.stderr)
            bid = prov.submit_batch(reqs)
            self.log({"kind": "batch", "model": model, "batch_id": bid, "requests": len(reqs),
                      "utc": accounting.utc_now().isoformat()})
            batch = prov.wait_batch(bid)
            got = prov.batch_results(batch)
            self.log({"kind": "batch_done", "model": model, "batch_id": bid, "status": batch["status"],
                      "request_counts": batch.get("request_counts"), "utc": accounting.utc_now().isoformat()})
            for cid, _ in reqs:
                k = key_of[cid]
                r = got.get(cid, providers.InfraError(f"no batch result for {cid}"))
                if isinstance(r, Exception):
                    self.guard.settle(rids[cid], 0.0, "batch item failed")
                firsts[k] = (r, rids[cid])
            keys = [k for k in keys if k in firsts]  # the guard may have cut the batch short
        with concurrent.futures.ThreadPoolExecutor(self.a.jobs) as ex:
            futs = [ex.submit(self.trial, prov, model, k, firsts.get(k)) for k in keys]
            for i, f in enumerate(concurrent.futures.as_completed(futs), 1):
                e = f.result()
                print(f"{model} {i}/{len(keys)} {e['task']}/{e['lang']}/{e['rep']}: "
                      f"{'ok' if e['success'] else 'fail'} in {e['attempts']} (${e['cost_usd']:.5f})"
                      f"{' ' + e['infra'] if e['infra'] else ''}", file=sys.stderr)


def sha256(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


TASK_SETS = {"v1": tasks, "v2": tasks_v2}


def projection(prices, models, tier, reps, task_set="v1"):
    """The plan's cost model at real prices: expected and ceiling dollars."""
    n_tasks = len(TASK_SETS[task_set].TIERS[tier])
    trials = n_tasks * len(langs.LANGS) * reps
    rows, tot_e, tot_c = [], 0.0, 0.0
    for m in models:
        p = prices["models"][m]
        b = bool(p.get("batch"))
        f = p.get("tokenizer_factor", 1.0)  # Claude's newer tokenizer: about 1.3x the tokens for the same text
        card_avg = 407 * f / len(langs.LANGS)  # the card is on Sley attempts only (one language in six)
        when = None
        if "off_peak" in p:
            import datetime
            when = datetime.datetime(2026, 11, 7, 12, tzinfo=datetime.timezone.utc)  # a Saturday: off-peak
        if p["provider"] in ("anthropic", "claude-code"):
            # Batch cache hits are best effort, so first attempts pay the card as a cache write;
            # retries run seconds later and read it from the cache.
            first_tok = {"input": round(1000 * f + card_avg), "cached": 0, "cache_write": round(card_avg), "output": round(1200 * f)}
            retry_tok = {"input": round(1000 * f + card_avg), "cached": round(card_avg), "cache_write": 0, "output": round(1200 * f)}
        else:
            first_tok = retry_tok = {"input": 1000 + round(card_avg), "cached": round(card_avg), "cache_write": 0, "output": 1200}
        first = accounting.cost(p, first_tok, batch=b, when=when)
        retry = accounting.cost(p, retry_tok, batch=False, when=when)
        expected = trials * (first + 0.3 * retry)
        # Ceiling: 3 attempts, each at the full 2,000-token output cap. Attempt n's prompt is the
        # 2,000-token first prompt plus, per earlier attempt, that reply (2,000) and its feedback
        # (4,000 characters, about 1,400 tokens); on Claude every prompt token is priced as a cache write.
        def cap(n):
            prompt = round(2000 * f + (n - 1) * (2000 + 1400 * f))
            return {"input": prompt, "cached": 0, "cache_write": prompt if p["provider"] in ("anthropic", "claude-code") else 0,
                    "output": 2000}
        ceiling = trials * (accounting.cost(p, cap(1), batch=b, when=when)
                            + accounting.cost(p, cap(2), when=when) + accounting.cost(p, cap(3), when=when))
        rows.append({"model": m, "trials": trials, "per_attempt_first": first, "per_attempt_retry": retry,
                     "expected": expected, "ceiling": ceiling})
        tot_e += expected
        tot_c += ceiling
    return {"tier": tier, "reps": reps, "trials_per_model": trials, "rows": rows, "expected": tot_e, "ceiling": tot_c}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", required=False)
    ap.add_argument("--tier", default="small", choices=["small", "full"])
    ap.add_argument("--models", default=",".join(PREREG["models"]["default"]))
    ap.add_argument("--langs", default=",".join(langs.LANGS))
    ap.add_argument("--tasks", default="")
    ap.add_argument("--task-set", default="v1", choices=sorted(TASK_SETS),
                    help="v1: tasks.py (pre-registered); v2: tasks_v2.py (not yet pre-registered; tier full only)")
    ap.add_argument("--reps", type=int, default=None)
    ap.add_argument("--guard", type=float, default=PREREG["spend_guard"]["per_run_usd"])
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--effort", default="low")
    ap.add_argument("--no-batch", action="store_true")
    ap.add_argument("--reasoning-in-completion", action="store_true",
                    help="xAI/DeepSeek: completion_tokens already include reasoning (default: bill reasoning on top)")
    g = ap.add_mutually_exclusive_group()
    g.add_argument("--mock", action="store_true", help="dry run against the in-process mock (the default)")
    g.add_argument("--paid", action="store_true", help="send real, billed requests")
    ap.add_argument("--project", action="store_true", help="print the cost projection and exit")
    a = ap.parse_args()
    a.models = [m for m in a.models.split(",") if m]
    a.langs = [l for l in a.langs.split(",") if l]
    reps = a.reps or PREREG["tiers"][a.tier]["repetitions"]
    prices = accounting.load_prices()
    if a.project:
        if a.tier not in TASK_SETS[a.task_set].TIERS:
            ap.error(f"task set {a.task_set} has no tier {a.tier}")
        print(json.dumps(projection(prices, a.models, a.tier, reps, a.task_set), indent=2))
        return
    if not a.out:
        ap.error("--out is required")
    if a.guard > PREREG["spend_guard"]["per_run_usd"]:
        ap.error(f"--guard above the pre-registered ${PREREG['spend_guard']['per_run_usd']:.0f} needs a new registration")
    ts = TASK_SETS[a.task_set]
    if not a.tasks and a.tier not in ts.TIERS:
        ap.error(f"task set {a.task_set} has no tier {a.tier}")
    names = [t for t in (a.tasks.split(",") if a.tasks else ts.TIERS[a.tier])]
    unknown = [t for t in names if t not in ts.BY_NAME]
    if unknown:
        ap.error(f"not in task set {a.task_set}: {', '.join(unknown)}")
    keys = [(t, l, r) for r in range(1, reps + 1) for t in names for l in a.langs]
    run = Run(a, prices)
    manifest = {"prereg_sha256": sha256(os.path.join(HERE, "prereg.json")),
                "prices_sha256": sha256(os.path.join(HERE, "prices.json")),
                "task_set": a.task_set,
                "tasks_sha256": sha256(os.path.join(HERE, "tasks.py" if a.task_set == "v1" else "tasks_v2.py")),
                **({"authored_sha256": sha256(tasks_v2.AUTHORED)} if a.task_set == "v2" else {}),
                "mode": "paid" if a.paid else "mock", "tier": a.tier, "reps": reps, "models": a.models,
                "langs": a.langs, "tasks": names, "guard_usd": a.guard, "started_utc": accounting.utc_now().isoformat(),
                "sley_card_topic": CARD_TOPIC,
                "sley_card_sha256": hashlib.sha256(run.card.encode()).hexdigest() if run.card else None,
                "labels": {m: prices["models"][m]["label"] for m in a.models if prices["models"][m].get("label")}}
    json.dump(manifest, open(os.path.join(a.out, "manifest.json"), "w"), indent=2)
    for m in a.models:
        run.run_model(m, keys)
    print(f"spent ${run.guard.spent:.6f} of the ${a.guard:.2f} guard ({'billed' if a.paid else 'mock usage at real prices; nothing billed'})",
          file=sys.stderr)
    if isinstance(run.transport, providers.MockTransport):
        hosts = {re.sub(r"^(https://[^/]+).*", r"\1", u) for _, u in run.transport.calls}
        print(f"mock transport: {len(run.transport.calls)} requests to {sorted(hosts)} answered in process; network calls: 0",
              file=sys.stderr)


if __name__ == "__main__":
    main()
