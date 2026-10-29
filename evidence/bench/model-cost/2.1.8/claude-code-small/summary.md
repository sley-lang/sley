# Genuine model cost: small tier, paid run

Usage as each route reports it, priced at the list prices in prices.json.

**claude-code-haiku-5-5: Claude Code cloud session: tokens as reported; $ estimated at API list price, not billed per token.**

prereg.json SHA-256 `c605c7cd7a34ce7f68bb20dae799e4a3fa78182c5572c6fb8a180bcafc729183`; prices checked 2026-10-29.

| Model | Language | Correct | First try | $ per correct | $ per trial | Attempts | Input / cached / output tokens per trial |
|---|---|---|---|---|---|---|---|
| claude-code-haiku-5-5 | c | 14/15 | 14 | $0.00060 | $0.00056 | 1.00 | 1595 / 55 / 499 |
| claude-code-haiku-5-5 | go | 15/15 | 15 | $0.00049 | $0.00049 | 1.00 | 1564 / 0 / 364 |
| claude-code-haiku-5-5 | python | 15/15 | 15 | $0.00038 | $0.00038 | 1.00 | 1545 / 51 / 166 |
| claude-code-haiku-5-5 | rust | 15/15 | 15 | $0.00046 | $0.00046 | 1.00 | 1564 / 0 / 295 |
| claude-code-haiku-5-5 | sley | 15/15 | 15 | $0.00045 | $0.00045 | 1.00 | 2219 / 813 / 320 |
| claude-code-haiku-5-5 | ts | 15/15 | 15 | $0.00042 | $0.00042 | 1.00 | 1559 / 0 / 226 |

Total spend: $0.04153.
Infrastructure failures (excluded): 0.

Claude Code overhead: a calibration call whose own text is about 8 tokens reported 1329 input tokens (0 cache reads, 0 cache writes), so about 1321 input tokens per call come from the CLI itself, not the task.
