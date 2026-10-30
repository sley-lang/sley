# Genuine model cost: full tier, paid run

Billed usage at the list prices in prices.json.

prereg.json SHA-256 `633246287c2c97156bc22e916f32437b6dac124df0efdc7d019c3c2c8fb5e1af`; prices checked 2026-10-29.

| Model | Language | Correct | First try | $ per correct | $ per trial | Attempts | Input / cached / output tokens per trial |
|---|---|---|---|---|---|---|---|
| claude-haiku-5-5 | c | 54/54 | 54 | $0.00026 | $0.00026 | 1.00 | 275 / 0 / 462 |
| claude-haiku-5-5 | go | 54/54 | 54 | $0.00019 | $0.00019 | 1.00 | 250 / 0 / 320 |
| claude-haiku-5-5 | python | 54/54 | 54 | $0.00009 | $0.00009 | 1.00 | 234 / 0 / 142 |
| claude-haiku-5-5 | rust | 54/54 | 54 | $0.00016 | $0.00016 | 1.00 | 251 / 0 / 273 |
| claude-haiku-5-5 | sley | 54/54 | 51 | $0.00012 | $0.00012 | 1.06 | 853 / 665 / 188 |
| claude-haiku-5-5 | ts | 54/54 | 54 | $0.00014 | $0.00014 | 1.00 | 246 / 0 / 227 |

Total spend: $0.05169.
Infrastructure failures (excluded): 0.

Each language against Python with 95% intervals (paired bootstrap over tasks): `intervals.md`.
