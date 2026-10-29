#!/usr/bin/env python3
"""Run the genuine model cost benchmark (pre-registered in prereg.json).

  run.py --out DIR [--tier small|full] [--models gpt-6-luna,grok-build-0.1]
         [--langs ...] [--tasks ...] [--reps N] [--guard 10] [--jobs 4]
         [--mock | --paid] [--project]

Without --paid every call goes to the in-process MockTransport: no network,
no key, no spend (the dry run). --paid sends real, billed requests and needs
OPENAI_API_KEY / XAI_API_KEY; the spend guard stops the run before billed
spend could pass --guard dollars. --project prints the cost projection from
the plan's assumptions and real prices, and sends nothing.
"""
import argparse, concurrent.futures, hashlib, json, os, random, re, subprocess, sys, threading

import accounting, langs, providers, tasks

HERE = os.path.dirname(os.path.abspath(__file__))
PREREG = json.load(open(os.path.join(HERE, "prereg.json")))
MAX_OUT = PREREG["output_cap_tokens"]
MAX_ATTEMPTS = PREREG["attempts"]["max"]
LANG_NAME = {"python": "Python 3", "sley": "Sley (Python dialect)", "c": "C (C17, gcc)", "rust": "Rust (2021 edition)",
             "go": "Go", "ts": "TypeScript (strict, ES2022 modules)"}

SYSTEM = ("You write one source file for an automated benchmark. Reply with exactly one fenced code block that "
          "holds the complete file, and nothing else. Use only the standard library. Do not write tests or a main "
          "function; the harness runs its own tests.")


def card():
    out = subprocess.run([langs.sley_agent(), "help", "python"], capture_output=True, text=True)
    return out.stdout.strip()


def system_text(lang, sley_card):
    if lang != "sley":
        return SYSTEM
    return (SYSTEM + "\n\nThe language is Sley's Python dialect: typed Python that `sley-agent try --familiar` "
            "compiles to Sley. Errors are `raise ValueError(...)`. Its reference card:\n\n" + sley_card)


def task_prompt(task, lang):
    t = tasks.BY_NAME[task]
    lines = [f"Language: {LANG_NAME[lang]}", f"Write: {langs.signature(task, lang)}", "", t["statement"], ""]
    if t.get("edit"):
        starter = open(os.path.join(HERE, "starters", task, langs.SRC[lang])).read()
        lines += ["The existing file:", f"```{langs.FENCE[lang]}", starter.rstrip(), "```", ""]
    lines.append("Examples (the visible tests):")
    for args, want in t["visible"]:
        lines.append(f"  {langs.call(task, 'python' if lang == 'sley' else lang, args)} -> {want}")
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
    for t in tasks.TASKS:
        for l in langs.LANGS:
            by_fn[(langs.fn(t["name"], l), l)] = t["name"]
    lang_of = {v: k for k, v in LANG_NAME.items()}

    def answer(model, messages):
        user = [m["content"] for m in messages if m["role"] == "user"]
        first = user[0]
        lang = lang_of[re.search(r"^Language: (.*)$", first, re.M).group(1)]
        fname = re.search(r"^Write: (?:def |pub fn |func |export function |int |long long )(\w+)\(", first, re.M).group(1)
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
                                            if prov.name == "openai" else self.a.reasoning_in_completion)
        cost = accounting.cost(price, tokens, batch=reply.batch, when=when)
        self.guard.settle(rid, cost, f"{model}/{'/'.join(map(str, key))}/a{attempt}")
        self.log({"kind": "call", **self.ids(model, key), "attempt": attempt, "utc": when.isoformat(),
                  "batch": reply.batch, "finish": reply.finish, "response_id": reply.raw_id,
                  "model_reported": reply.model_reported, "usage": reply.usage, "billed_tokens": tokens,
                  "cost_usd": round(cost, 8), "prices_checked": self.prices["checked"]})
        return reply, cost

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
        firsts = {}
        if price.get("batch") and isinstance(prov, providers.OpenAIProvider) and not self.a.no_batch and keys:
            reqs, rids = [], {}
            for k in keys:
                cid = f"{k[0]}.{k[1]}.{k[2]}"
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
                k = tuple(cid.split("."))
                k = (k[0], k[1], int(k[2]))
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


def projection(prices, models, tier, reps):
    """The plan's cost model at real prices: expected and ceiling dollars."""
    n_tasks = len(tasks.TIERS[tier])
    trials = n_tasks * len(langs.LANGS) * reps
    card_avg = 407 / len(langs.LANGS)  # the card is on Sley attempts only, and cached
    rows, tot_e, tot_c = [], 0.0, 0.0
    for m in models:
        p = prices["models"][m]
        b = bool(p.get("batch"))
        when = None
        if "off_peak" in p:
            import datetime
            when = datetime.datetime(2026, 11, 7, 12, tzinfo=datetime.timezone.utc)  # a Saturday: off-peak
        exp_tok = {"input": 1000 + round(card_avg), "cached": round(card_avg), "cache_write": 0, "output": 1200}
        first = accounting.cost(p, exp_tok, batch=b, when=when)
        retry = accounting.cost(p, exp_tok, batch=False, when=when)
        expected = trials * (first + 0.3 * retry)
        cap_tok = {"input": 2000, "cached": 0, "cache_write": 0, "output": 2000}
        ceiling = trials * (accounting.cost(p, cap_tok, batch=b, when=when) + 2 * accounting.cost(p, cap_tok, when=when))
        rows.append({"model": m, "trials": trials, "per_attempt_first": first, "expected": expected, "ceiling": ceiling})
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
        print(json.dumps(projection(prices, a.models, a.tier, reps), indent=2))
        return
    if not a.out:
        ap.error("--out is required")
    if a.guard > PREREG["spend_guard"]["per_run_usd"]:
        ap.error(f"--guard above the pre-registered ${PREREG['spend_guard']['per_run_usd']:.0f} needs a new registration")
    names = [t for t in (a.tasks.split(",") if a.tasks else tasks.TIERS[a.tier])]
    keys = [(t, l, r) for r in range(1, reps + 1) for t in names for l in a.langs]
    run = Run(a, prices)
    manifest = {"prereg_sha256": sha256(os.path.join(HERE, "prereg.json")),
                "prices_sha256": sha256(os.path.join(HERE, "prices.json")),
                "tasks_sha256": sha256(os.path.join(HERE, "tasks.py")),
                "mode": "paid" if a.paid else "mock", "tier": a.tier, "reps": reps, "models": a.models,
                "langs": a.langs, "tasks": names, "guard_usd": a.guard, "started_utc": accounting.utc_now().isoformat(),
                "sley_card_sha256": hashlib.sha256(run.card.encode()).hexdigest() if run.card else None}
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
