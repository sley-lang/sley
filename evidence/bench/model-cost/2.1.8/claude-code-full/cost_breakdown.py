#!/usr/bin/env python3
"""Split each language's spend into its parts, from the usage the CLI reported.

  cost_breakdown.py <run dir>   -> <run dir>/cost-breakdown.md

Prices are bench/model-cost/prices.json's claude-haiku-5-5 standard rates.
Thinking tokens come from usage.output_tokens_details.thinking_tokens and are
billed as output; "visible output" is output_tokens minus thinking. Program
sizes are the saved final programs in o200k_base tokens (an offline proxy;
Claude's tokenizer gives about 30% more), when tiktoken is installed.
"""
import ast, collections, glob, json, os, sys

run = sys.argv[1]
P = {"input": 0.10, "cw5": 0.125, "cw1": 0.20, "read": 0.01, "out": 0.50}  # $/MTok
rows = [json.loads(l) for l in open(os.path.join(run, "results.jsonl"))]
trials = collections.Counter(r["lang"] for r in rows if r["kind"] == "trial")
a = collections.defaultdict(collections.Counter)
for r in rows:
    if r["kind"] != "call":
        continue
    b, u = r["billed_tokens"], r["usage"]
    u = ast.literal_eval(u) if isinstance(u, str) else u
    think = (u.get("output_tokens_details") or {}).get("thinking_tokens", 0)
    c = a[r["lang"]]
    c["calls"] += 1
    c["uncached input"] += (b["input"] - b["cached"] - b["cache_write"] - b.get("cache_write_1h", 0)) * P["input"]
    c["cache reads"] += b["cached"] * P["read"]
    c["cache writes"] += b["cache_write"] * P["cw5"] + b.get("cache_write_1h", 0) * P["cw1"]
    c["thinking"] += think * P["out"]
    c["visible output"] += (b["output"] - think) * P["out"]
    c["thinking tok"] += think
    c["visible tok"] += b["output"] - think
try:
    import tiktoken
    enc = tiktoken.get_encoding("o200k_base")
    prog = collections.Counter()
    for f in glob.glob(os.path.join(run, "programs", "*", "*")):
        prog[os.path.basename(f).split(".")[1]] += len(enc.encode(open(f).read()))
except ImportError:
    prog = None
parts = ("uncached input", "cache reads", "cache writes", "thinking", "visible output")
md = ["# Where the money goes (micro-dollars per trial, list price)", "",
      "| Language | " + " | ".join(parts) + " | Total | Thinking tok / call | Visible output tok / call | Program tok / trial (o200k) |",
      "|---" * (len(parts) + 5) + "|"]
for lang in sorted(a):
    c, n = a[lang], trials[lang]
    md.append(f"| {lang} | " + " | ".join(f"{c[p] / n:.1f}" for p in parts) + f" | {sum(c[p] for p in parts) / n:.1f} | "
              f"{c['thinking tok'] / c['calls']:.0f} | {c['visible tok'] / c['calls']:.0f} | "
              + (f"{prog[lang] / n:.0f} |" if prog else "n/a |"))
open(os.path.join(run, "cost-breakdown.md"), "w").write("\n".join(md) + "\n")
print("\n".join(md))
