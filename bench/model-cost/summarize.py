#!/usr/bin/env python3
"""Summarize a run: summary.json and summary.md per model and language.

  summarize.py <run dir>

Dollars per correct program = all billed spend on a model x language's trials
(failed trials included) / its successful trials. Infrastructure failures are
listed and left out of the denominators, as pre-registered.
"""
import collections, json, os, sys


def summarize(run_dir):
    rows = [json.loads(l) for l in open(os.path.join(run_dir, "results.jsonl"))]
    manifest = json.load(open(os.path.join(run_dir, "manifest.json")))
    calls = [r for r in rows if r["kind"] == "call" and "billed_tokens" in r]
    trials = [r for r in rows if r["kind"] == "trial"]
    tok = collections.defaultdict(lambda: collections.Counter())
    for c in calls:
        k = (c["model"], c["lang"])
        for f in ("input", "cached", "output", "reasoning"):
            tok[k][f] += c["billed_tokens"][f]
        tok[k]["calls"] += 1
        tok[k]["batch_calls"] += 1 if c.get("batch") else 0
        tok[k]["truncated"] += 1 if c.get("finish") == "length" else 0
    cells = collections.defaultdict(list)
    for t in trials:
        cells[(t["model"], t["lang"])].append(t)
    out = []
    for (model, lang), ts in sorted(cells.items()):
        counted = [t for t in ts if not t["infra"]]
        spend = sum(t["cost_usd"] for t in ts)
        ok = sum(1 for t in counted if t["success"])
        n = len(counted)
        k = tok[(model, lang)]
        out.append({"model": model, "lang": lang, "trials": n, "infra_excluded": len(ts) - n,
                    "success": ok, "success_rate": ok / n if n else None,
                    "first_try": sum(1 for t in counted if t["first_try"]),
                    "visible_pass_hidden_fail": sum(1 for t in counted if t["visible_passed"] and not t["success"]),
                    "attempts_per_trial": sum(t["attempts"] for t in counted) / n if n else None,
                    "spend_usd": spend, "usd_per_trial": spend / len(ts) if ts else None,
                    "usd_per_correct": spend / ok if ok else None,
                    "tokens_per_trial": {f: k[f] / len(ts) for f in ("input", "cached", "output", "reasoning")} if ts else {},
                    "calls": k["calls"], "batch_calls": k["batch_calls"], "truncated_calls": k["truncated"]})
    total = sum(c["cost_usd"] for c in calls)
    s = {"mode": manifest["mode"], "manifest": manifest, "cells": out, "spend_usd": total,
         "infra_failures": [{k: t[k] for k in ("model", "task", "lang", "rep", "infra")} for t in trials if t["infra"]]}
    json.dump(s, open(os.path.join(run_dir, "summary.json"), "w"), indent=2)
    md = [f"# Genuine model cost: {manifest['tier']} tier, {manifest['mode']} run", "",
          ("**Dry run: the mock provider answered every call. Dollar figures price synthetic usage at real list prices; "
           "nothing was billed and they say nothing about any model.**" if manifest["mode"] == "mock" else
           "Billed usage at the list prices in prices.json."), "",
          *[f"**{m}: {label}.**" for m, label in sorted((manifest.get("labels") or {}).items())],
          *([""] if manifest.get("labels") else []),
          f"prereg.json SHA-256 `{manifest['prereg_sha256']}`; prices checked {json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'prices.json')))['checked']}.", "",
          "| Model | Language | Correct | First try | $ per correct | $ per trial | Attempts | Input / cached / output tokens per trial |",
          "|---|---|---|---|---|---|---|---|"]
    for c in out:
        t = c["tokens_per_trial"]
        per = f"${c['usd_per_correct']:.5f}" if c["usd_per_correct"] is not None else "n/a"
        md.append(f"| {c['model']} | {c['lang']} | {c['success']}/{c['trials']} | {c['first_try']} | {per} | "
                  f"${c['usd_per_trial']:.5f} | {c['attempts_per_trial']:.2f} | "
                  f"{t['input']:.0f} / {t['cached']:.0f} / {t['output']:.0f} |")
    md += ["", f"Total spend: ${total:.5f}.", f"Infrastructure failures (excluded): {len(s['infra_failures'])}."]
    cal = os.path.join(run_dir, "calibration.json")
    if os.path.exists(cal):
        c = json.load(open(cal))
        s["calibration"] = c
        md += ["", f"Claude Code overhead: a calibration call whose own text is about {c['sent_chars'] // 4} tokens "
               f"reported {c['billed_tokens']['input']} input tokens ({c['billed_tokens']['cached']} cache reads, "
               f"{c['billed_tokens']['cache_write']} cache writes), so about {c['overhead_input_tokens_estimate']} "
               f"input tokens per call come from the CLI itself, not the task."]
        json.dump(s, open(os.path.join(run_dir, "summary.json"), "w"), indent=2)
    open(os.path.join(run_dir, "summary.md"), "w").write("\n".join(md) + "\n")
    return s


if __name__ == "__main__":
    s = summarize(sys.argv[1])
    print(open(os.path.join(sys.argv[1], "summary.md")).read())
