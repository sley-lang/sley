#!/usr/bin/env python3
"""Paired bootstrap over tasks: each language against Python, same run.

  intervals.py <run dir>   -> <run dir>/intervals.json and intervals.md

Trials of the same task are correlated (same prompt, same model), so the unit
of resampling is the task, not the trial: each bootstrap draw picks tasks with
replacement and keeps every trial of each picked task, in both languages, so the
two arms stay paired. Reported per model and language, against Python:

- $ per trial ratio (sum of the language's spend / sum of Python's, over the
  drawn tasks) with a 95% percentile interval;
- the per-trial dollar delta (language minus Python), which is free of any
  overhead both languages pay equally (such as the Claude Code CLI prompt);
- thinking tokens per call, the delta against Python with its interval, the
  share of calls that think at all and the mean length when they do (thinking
  at low effort is bimodal: a call either skips it or thinks a few hundred
  tokens), and a sign test over tasks on the per-task thinking delta.

The resampling is seeded, so a run's intervals are reproducible.
"""
import ast, collections, json, math, os, random, sys

SEED = 218
DRAWS = 10000


def _usage(call):
    u = call.get("usage") or {}
    return ast.literal_eval(u) if isinstance(u, str) else u


def thinking(call):
    return (_usage(call).get("output_tokens_details") or {}).get("thinking_tokens", 0) or 0


def sign_test(wins, losses):
    """Two-sided exact sign test p-value (ties dropped)."""
    n = wins + losses
    if n == 0:
        return 1.0
    k = min(wins, losses)
    p = sum(math.comb(n, i) for i in range(k + 1)) / 2 ** n
    return min(1.0, 2 * p)


def percentile(xs, q):
    xs = sorted(xs)
    i = (len(xs) - 1) * q
    lo, hi = math.floor(i), math.ceil(i)
    return xs[lo] + (xs[hi] - xs[lo]) * (i - lo)


def per_task(rows, model, lang):
    """task -> {spend, trials, think, calls, thinking_calls}"""
    out = collections.defaultdict(lambda: collections.Counter())
    for r in rows:
        if r.get("model") != model or r.get("lang") != lang:
            continue
        if r["kind"] == "trial" and not r.get("infra"):
            c = out[r["task"]]
            c["spend"] += r["cost_usd"]
            c["trials"] += 1
            c["success"] += 1 if r["success"] else 0
        elif r["kind"] == "call" and "billed_tokens" in r:
            c = out[r["task"]]
            t = thinking(r)
            c["think"] += t
            c["calls"] += 1
            c["thinking_calls"] += 1 if t > 0 else 0
    return out


def compare(a, b, draws=DRAWS, seed=SEED):
    """Paired bootstrap of language a against baseline b over their shared tasks."""
    tasks = sorted(set(a) & set(b))
    if not tasks:
        return None

    def stats(sample):
        sa = {k: sum(a[t][k] for t in sample) for k in ("spend", "trials", "think", "calls")}
        sb = {k: sum(b[t][k] for t in sample) for k in ("spend", "trials", "think", "calls")}
        ratio = (sa["spend"] / sa["trials"]) / (sb["spend"] / sb["trials"]) if sb["spend"] else float("nan")
        delta = sa["spend"] / sa["trials"] - sb["spend"] / sb["trials"]
        think = sa["think"] / sa["calls"] - sb["think"] / sb["calls"]
        return ratio, delta, think

    point = stats(tasks)
    rng = random.Random(seed)
    boot = [stats([rng.choice(tasks) for _ in tasks]) for _ in range(draws)]
    ci = [(percentile([s[i] for s in boot], 0.025), percentile([s[i] for s in boot], 0.975)) for i in range(3)]
    diffs = [a[t]["think"] / a[t]["calls"] - b[t]["think"] / b[t]["calls"] for t in tasks]
    wins, losses = sum(d > 0 for d in diffs), sum(d < 0 for d in diffs)
    return {"tasks": len(tasks),
            "usd_per_trial_ratio": point[0], "usd_per_trial_ratio_ci95": list(ci[0]),
            "usd_per_trial_delta": point[1], "usd_per_trial_delta_ci95": list(ci[1]),
            "thinking_per_call_delta": point[2], "thinking_per_call_delta_ci95": list(ci[2]),
            "tasks_thinking_more": wins, "tasks_thinking_less": losses,
            "thinking_sign_test_p": sign_test(wins, losses)}


def describe(cell):
    calls = sum(c["calls"] for c in cell.values())
    tc = sum(c["thinking_calls"] for c in cell.values())
    think = sum(c["think"] for c in cell.values())
    trials = sum(c["trials"] for c in cell.values())
    return {"trials": trials, "calls": calls, "usd_per_trial": sum(c["spend"] for c in cell.values()) / trials if trials else None,
            "thinking_per_call": think / calls if calls else None,
            "share_of_calls_thinking": tc / calls if calls else None,
            "thinking_per_thinking_call": think / tc if tc else None}


def intervals(run_dir, baseline="python", draws=DRAWS, seed=SEED):
    rows = [json.loads(l) for l in open(os.path.join(run_dir, "results.jsonl"))]
    models = sorted({r["model"] for r in rows if r.get("kind") == "trial"})
    langs = sorted({r["lang"] for r in rows if r.get("kind") == "trial"})
    out = {"method": "paired bootstrap over tasks, percentile 95% intervals", "draws": draws, "seed": seed,
           "baseline": baseline, "models": {}}
    md = ["# Each language against Python: paired bootstrap over tasks", "",
          f"{draws} draws, seed {seed}. Each draw resamples tasks with replacement and keeps all of a task's trials in both "
          "languages. Intervals are 95% percentile intervals. The per-trial delta is unaffected by overhead both languages "
          "pay equally (such as the Claude Code CLI's own prompt); the ratio is diluted by it.", ""]
    for m in models:
        base = per_task(rows, m, baseline)
        if not base:
            continue
        cells = {}
        md += [f"## {m}", "",
               "| Language | $ per trial vs Python (95% CI) | Delta, micro-$ per trial (95% CI) | Thinking tok per call | "
               "Delta vs Python (95% CI) | Calls that think | Tokens per thinking call | Tasks thinking more / less (sign test p) |",
               "|---|---|---|---|---|---|---|---|"]
        for lang in langs:
            cell = per_task(rows, m, lang)
            d = describe(cell)
            c = compare(cell, base, draws, seed) if lang != baseline else None
            cells[lang] = {**d, "vs_baseline": c}
            share = f"{d['share_of_calls_thinking']:.0%}" if d["share_of_calls_thinking"] is not None else "n/a"
            per_tc = f"{d['thinking_per_thinking_call']:.0f}" if d["thinking_per_thinking_call"] else "n/a"
            if c is None:
                md.append(f"| {lang} | 1.00x (baseline) | \u2013 | {d['thinking_per_call']:.0f} | \u2013 | {share} | {per_tc} | \u2013 |")
            else:
                r, rc = c["usd_per_trial_ratio"], c["usd_per_trial_ratio_ci95"]
                dl, dc = c["usd_per_trial_delta"] * 1e6, [x * 1e6 for x in c["usd_per_trial_delta_ci95"]]
                t, tcI = c["thinking_per_call_delta"], c["thinking_per_call_delta_ci95"]
                md.append(f"| {lang} | {r:.2f}x ({rc[0]:.2f}\u2013{rc[1]:.2f}) | {dl:+.1f} ({dc[0]:+.1f} to {dc[1]:+.1f}) | "
                          f"{d['thinking_per_call']:.0f} | {t:+.0f} ({tcI[0]:+.0f} to {tcI[1]:+.0f}) | {share} | {per_tc} | "
                          f"{c['tasks_thinking_more']} / {c['tasks_thinking_less']} (p = {c['thinking_sign_test_p']:.2g}) |")
        out["models"][m] = cells
        md.append("")
    json.dump(out, open(os.path.join(run_dir, "intervals.json"), "w"), indent=2)
    open(os.path.join(run_dir, "intervals.md"), "w").write("\n".join(md))
    return out


if __name__ == "__main__":
    intervals(sys.argv[1])
    print(open(os.path.join(sys.argv[1], "intervals.md")).read())
