# Genuine model cost: small tier, mock run

**Dry run: the mock provider answered every call. Dollar figures price synthetic usage at real list prices; nothing was billed and they say nothing about any model.**

prereg.json SHA-256 `9e2b54748c01d9ecf8a2a042878395423dea89f1306ae9dd53666fbd97ce8aa7`; prices checked 2026-10-28.

| Model | Language | Correct | First try | $ per correct | $ per trial | Attempts | Input / cached / output tokens per trial |
|---|---|---|---|---|---|---|---|
| gpt-6-luna | c | 14/15 | 10 | $0.00012 | $0.00011 | 1.40 | 401 / 0 / 265 |
| gpt-6-luna | go | 14/15 | 9 | $0.00007 | $0.00006 | 1.47 | 275 / 0 / 126 |
| gpt-6-luna | python | 14/15 | 9 | $0.00008 | $0.00008 | 1.47 | 349 / 0 / 148 |
| gpt-6-luna | rust | 14/15 | 10 | $0.00006 | $0.00006 | 1.40 | 264 / 0 / 117 |
| gpt-6-luna | sley | 13/15 | 9 | $0.00019 | $0.00016 | 1.53 | 1092 / 0 / 284 |
| gpt-6-luna | ts | 15/15 | 10 | $0.00005 | $0.00005 | 1.33 | 249 / 0 / 120 |
| grok-build-0.1 | c | 12/15 | 8 | $0.00104 | $0.00083 | 1.67 | 414 / 0 / 209 |
| grok-build-0.1 | go | 14/15 | 10 | $0.00066 | $0.00061 | 1.40 | 258 / 0 / 178 |
| grok-build-0.1 | python | 14/15 | 7 | $0.00151 | $0.00141 | 1.60 | 547 / 0 / 432 |
| grok-build-0.1 | rust | 14/15 | 5 | $0.00183 | $0.00171 | 1.73 | 605 / 0 / 552 |
| grok-build-0.1 | sley | 14/15 | 10 | $0.00178 | $0.00166 | 1.40 | 987 / 0 / 335 |
| grok-build-0.1 | ts | 14/15 | 9 | $0.00137 | $0.00128 | 1.47 | 434 / 0 / 424 |

Total spend: $0.12035.
Infrastructure failures (excluded): 0.
