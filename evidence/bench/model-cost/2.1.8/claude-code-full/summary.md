# Genuine model cost: full tier, paid run

Usage as each route reports it, priced at the list prices in prices.json.

**claude-code-haiku-5-5: Claude Code cloud session: tokens as reported; $ estimated at API list price, not billed per token.**

prereg.json SHA-256 `c605c7cd7a34ce7f68bb20dae799e4a3fa78182c5572c6fb8a180bcafc729183`; prices checked 2026-10-29.

| Model | Language | Correct | First try | $ per correct | $ per trial | Attempts | Input / cached / output tokens per trial |
|---|---|---|---|---|---|---|---|
| claude-code-haiku-5-5 | c | 50/54 | 49 | $0.00052 | $0.00048 | 1.02 | 1698 / 762 / 575 |
| claude-code-haiku-5-5 | go | 51/54 | 51 | $0.00046 | $0.00043 | 1.02 | 1601 / 696 / 485 |
| claude-code-haiku-5-5 | python | 51/54 | 51 | $0.00031 | $0.00029 | 1.00 | 1542 / 692 / 230 |
| claude-code-haiku-5-5 | rust | 51/54 | 51 | $0.00036 | $0.00034 | 1.00 | 1559 / 696 / 313 |
| claude-code-haiku-5-5 | sley | 51/54 | 49 | $0.00037 | $0.00035 | 1.04 | 2307 / 1398 / 311 |
| claude-code-haiku-5-5 | ts | 51/54 | 51 | $0.00034 | $0.00032 | 1.00 | 1555 / 692 / 280 |

Total spend: $0.11935.
Infrastructure failures (excluded): 0.

Claude Code overhead: a calibration call whose own text is about 8 tokens reported 1333 input tokens (0 cache reads, 0 cache writes), so about 1325 input tokens per call come from the CLI itself, not the task.
