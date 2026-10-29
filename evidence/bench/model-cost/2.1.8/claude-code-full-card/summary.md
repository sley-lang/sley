# Genuine model cost: full tier, paid run

Usage as each route reports it, priced at the list prices in prices.json.

**claude-code-haiku-5-5: Claude Code cloud session: tokens as reported; $ estimated at API list price, not billed per token.**

prereg.json SHA-256 `633246287c2c97156bc22e916f32437b6dac124df0efdc7d019c3c2c8fb5e1af`; prices checked 2026-10-29.

| Model | Language | Correct | First try | $ per correct | $ per trial | Attempts | Input / cached / output tokens per trial |
|---|---|---|---|---|---|---|---|
| claude-code-haiku-5-5 | c | 54/54 | 52 | $0.00054 | $0.00054 | 1.04 | 1698 / 505 / 585 |
| claude-code-haiku-5-5 | go | 54/54 | 53 | $0.00045 | $0.00045 | 1.02 | 1626 / 489 / 431 |
| claude-code-haiku-5-5 | python | 54/54 | 54 | $0.00032 | $0.00032 | 1.00 | 1543 / 478 / 199 |
| claude-code-haiku-5-5 | rust | 54/54 | 54 | $0.00038 | $0.00038 | 1.00 | 1560 / 489 / 326 |
| claude-code-haiku-5-5 | sley | 54/54 | 52 | $0.00040 | $0.00040 | 1.04 | 2193 / 1084 / 325 |
| claude-code-haiku-5-5 | ts | 54/54 | 54 | $0.00036 | $0.00036 | 1.00 | 1556 / 486 / 290 |

Total spend: $0.13183.
Infrastructure failures (excluded): 0.

Claude Code overhead: a calibration call whose own text is about 8 tokens reported 1330 input tokens (0 cache reads, 1328 cache writes), so about 1322 input tokens per call come from the CLI itself, not the task.
