# Genuine model cost: small tier, paid run

Usage as each route reports it, priced at the list prices in prices.json.

**claude-code-haiku-5-5: Claude Code cloud session: tokens as reported; $ estimated at API list price, not billed per token.**

prereg.json SHA-256 `c605c7cd7a34ce7f68bb20dae799e4a3fa78182c5572c6fb8a180bcafc729183`; prices checked 2026-10-29.

| Model | Language | Correct | First try | $ per correct | $ per trial | Attempts | Input / cached / output tokens per trial |
|---|---|---|---|---|---|---|---|
| claude-code-haiku-5-5 | c | 2/2 | 2 | $0.00062 | $0.00062 | 1.00 | 1584 / 0 / 600 |
| claude-code-haiku-5-5 | python | 2/2 | 2 | $0.00039 | $0.00039 | 1.00 | 1550 / 0 / 153 |
| claude-code-haiku-5-5 | sley | 2/2 | 2 | $0.00051 | $0.00051 | 1.00 | 2226 / 384 / 270 |

Total spend: $0.00302.
Infrastructure failures (excluded): 0.

Claude Code overhead: a calibration call whose own text is about 8 tokens reported 1329 input tokens (0 cache reads, 0 cache writes), so about 1321 input tokens per call come from the CLI itself, not the task.
