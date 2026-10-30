# Genuine model cost evidence, 2.1.8

`dry-run/`: the small tier (15 tasks x 6 languages x 2 models) run against the
in-process mock provider on 29 Oct 2026 (`bench/model-cost/run.py --tier small`),
with the default models `claude-haiku-5-5` (Message Batches for first attempts)
and `grok-build-0.1`.
It checks the harness end to end (Message Batches flow, retries, guard, oracle, summary)
with no network call and no spend; its dollar figures price synthetic usage and
say nothing about any model.

The paid runs (small, then full) will be added here with `results.jsonl`,
`summary.json`, `summary.md` and `manifest.json`, after the API keys are
confirmed. Design: `bench/model-cost/prereg.json`.

`api-full-card/`: the full tier on the raw Anthropic Messages API (30 Oct
2026; `claude-haiku-5-5`, effort low, Message Batches off, $0.05169 billed),
with the same `python-task` card as `claude-code-full-card/`.
`route-comparison.md` sets it against the Claude Code route: the CLI route
thinks 1.5 to 3.2 times more per call and dilutes every ratio, so the two
routes are reported separately and never pooled.
