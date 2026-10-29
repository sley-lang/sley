# Genuine model cost evidence, 2.1.8

`dry-run/`: the small tier (15 tasks x 6 languages x 2 models) run against the
in-process mock provider on 28 Oct 2026 (`bench/model-cost/run.py --tier small`).
It checks the harness end to end (Batch flow, retries, guard, oracle, summary)
with no network call and no spend; its dollar figures price synthetic usage and
say nothing about any model.

The paid runs (small, then full) will be added here with `results.jsonl`,
`summary.json`, `summary.md` and `manifest.json`, after the API keys are
confirmed. Design: `bench/model-cost/prereg.json`.
