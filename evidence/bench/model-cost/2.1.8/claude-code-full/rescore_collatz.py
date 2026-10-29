#!/usr/bin/env python3
"""Re-score the collatz trials of a run with the fixed collatz oracle (PR #80).

  rescore_collatz.py <bench/model-cost dir with the fix> <run dir>

Run from a checkout that has #80; NODE_MODULES and SLEY_AGENT as for run.py.

Only the scoring changes: each saved final collatz program is run on the 128
hidden cases again; spend, tokens and attempts are untouched (the bug was in a
hidden case, so it never triggered a retry). Writes, in the run dir:
rescore-collatz.json (per trial: before/after) and summary-rescored.{md,json}.
"""
import json, os, shutil, sys, tempfile

harness, run_dir = map(os.path.abspath, sys.argv[1:3])
sys.path.insert(0, harness)
import langs, summarize, tasks  # noqa: E402

assert tasks.reference("collatz", [2]) == 1, "needs the fixed oracle"
rows = [json.loads(l) for l in open(os.path.join(run_dir, "results.jsonl"))]
out, changes = [], []
with tempfile.TemporaryDirectory() as tmp:
    for r in rows:
        if r["kind"] == "trial" and r["task"] == "collatz" and not r["infra"]:
            src_path = os.path.join(run_dir, "programs", r["model"], f"collatz-{r['rep']}.{langs.SRC[r['lang']]}")
            o = langs.oracle("collatz", r["lang"], open(src_path).read(), os.path.join(tmp, f"{r['lang']}-{r['rep']}"))
            new = dict(r, hidden_passed=o["passed"], first_hidden_failure=o["first_failure"],
                       success=bool(r["visible_passed"] and o["passed"] == o["total"]))
            new["first_try"] = new["success"] and r["attempts"] == 1
            changes.append({"lang": r["lang"], "rep": r["rep"], "program": os.path.relpath(src_path, run_dir),
                            "before": {"hidden_passed": r["hidden_passed"], "success": r["success"], "first_failure": r["first_hidden_failure"]},
                            "after": {"hidden_passed": o["passed"], "success": new["success"], "first_failure": o["first_failure"]}})
            r = new
        out.append(r)
    work = os.path.join(tmp, "run")
    os.makedirs(work)
    for f in ("manifest.json", "calibration.json"):
        if os.path.exists(os.path.join(run_dir, f)):
            shutil.copy(os.path.join(run_dir, f), work)
    with open(os.path.join(work, "results.jsonl"), "w") as f:
        f.writelines(json.dumps(r) + "\n" for r in out)
    summarize.summarize(work)
    md = open(os.path.join(work, "summary.md")).read()
    note = ("> **Re-scored.** Same calls, spend and tokens as `summary.md`; only the 18 collatz trials were scored again "
            "with the fixed collatz oracle (sley-lang/sley#80: limit = 2 answers 1, not 0). Per-trial before/after: "
            "`rescore-collatz.json`. `summary.md` keeps the score under the pre-registered oracle.\n\n")
    open(os.path.join(run_dir, "summary-rescored.md"), "w").write(note + md)
    shutil.copy(os.path.join(work, "summary.json"), os.path.join(run_dir, "summary-rescored.json"))
json.dump({"oracle_fix": "sley-lang/sley#80", "trials": changes}, open(os.path.join(run_dir, "rescore-collatz.json"), "w"), indent=2)
print(f"{sum(c['after']['success'] for c in changes)}/{len(changes)} collatz trials pass after; "
      f"{sum(c['before']['success'] for c in changes)} before")
