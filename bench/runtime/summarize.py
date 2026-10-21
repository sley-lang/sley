#!/usr/bin/env python3
"""Summarize hyperfine JSON + Sley vm_micros into a markdown table."""
import json, sys, glob, os, statistics as st
d = sys.argv[1]
order = ["fib_30", "loopsum_1000000", "collatz_5000", "primes_30000", "gcdsum_400", "fib_1"]
langs = ["sley", "python3", "pypy3", "lua5.4", "luajit", "node", "ruby", "c", "rust", "rust_checked"]
rows = []
hdr = "| benchmark | " + " | ".join(f"{l} median ms [min-max]" for l in langs) + " | Sley in-VM ms (median) |"
print(hdr); print("|" + "---|" * (len(langs) + 2))
ratios = {}
for tag in order:
    f = f"{d}/{tag}.json"
    if not os.path.exists(f): continue
    res = {r["command"]: r for r in json.load(open(f))["results"]}
    cells = []
    for l in langs:
        r = res.get(l)
        if not r: cells.append("–"); continue
        t = r["times"]; cells.append(f"{st.median(t)*1000:.1f} [{min(t)*1000:.1f}–{max(t)*1000:.1f}]")
    vm = "–"
    vf = f"{d}/vm_{tag}.jsonl"
    if os.path.exists(vf):
        v = [json.loads(x)["vm_micros"] for x in open(vf) if x.strip()]
        vm = f"{st.median(v)/1000:.1f}"
    print(f"| {tag} | " + " | ".join(cells) + f" | {vm} |")
    if "sley" in res:
        s = st.median(res["sley"]["times"])
        ratios[tag] = {l: s / st.median(res[l]["times"]) for l in langs if l in res and l != "sley"}
print()
print("Ratio = Sley median / other median (>1 means Sley is slower):")
print("| benchmark | " + " | ".join(langs[1:]) + " |"); print("|" + "---|" * len(langs))
for tag, r in ratios.items():
    print(f"| {tag} | " + " | ".join(f"{r[l]:.1f}x" if l in r else "–" for l in langs[1:]) + " |")
