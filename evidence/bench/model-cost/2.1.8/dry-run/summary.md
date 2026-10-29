# Genuine model cost: small tier, mock run

**Dry run: the mock provider answered every call. Dollar figures price synthetic usage at real list prices; nothing was billed and they say nothing about any model.**

prereg.json SHA-256 `be471ef7672aee338438d0569608a453ec01614d9c560e70b82225e299fae369`; prices checked 2026-10-29.

| Model | Language | Correct | First try | $ per correct | $ per trial | Attempts | Input / cached / output tokens per trial |
|---|---|---|---|---|---|---|---|
| claude-haiku-5-5 | c | 15/15 | 11 | $0.00010 | $0.00010 | 1.27 | 356 / 0 / 266 |
| claude-haiku-5-5 | go | 14/15 | 8 | $0.00011 | $0.00011 | 1.53 | 378 / 0 / 258 |
| claude-haiku-5-5 | python | 15/15 | 8 | $0.00012 | $0.00012 | 1.47 | 408 / 0 / 281 |
| claude-haiku-5-5 | rust | 13/15 | 10 | $0.00008 | $0.00007 | 1.47 | 303 / 0 / 165 |
| claude-haiku-5-5 | sley | 14/15 | 10 | $0.00019 | $0.00017 | 1.40 | 1015 / 0 / 357 |
| claude-haiku-5-5 | ts | 13/15 | 8 | $0.00025 | $0.00021 | 1.60 | 629 / 0 / 593 |
| grok-build-0.1 | c | 12/15 | 8 | $0.00104 | $0.00083 | 1.67 | 414 / 0 / 209 |
| grok-build-0.1 | go | 14/15 | 10 | $0.00066 | $0.00061 | 1.40 | 258 / 0 / 178 |
| grok-build-0.1 | python | 14/15 | 7 | $0.00151 | $0.00141 | 1.60 | 547 / 0 / 432 |
| grok-build-0.1 | rust | 14/15 | 5 | $0.00183 | $0.00171 | 1.73 | 605 / 0 / 552 |
| grok-build-0.1 | sley | 14/15 | 10 | $0.00178 | $0.00166 | 1.40 | 987 / 0 / 335 |
| grok-build-0.1 | ts | 14/15 | 9 | $0.00137 | $0.00128 | 1.47 | 434 / 0 / 424 |

Total spend: $0.12427.
Infrastructure failures (excluded): 0.
