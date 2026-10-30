# Each language against Python: paired bootstrap over tasks

10000 draws, seed 218. Each draw resamples tasks with replacement and keeps all of a task's trials in both languages. Intervals are 95% percentile intervals. The per-trial delta is unaffected by overhead both languages pay equally (such as the Claude Code CLI's own prompt); the ratio is diluted by it.

## claude-code-haiku-5-5

| Language | $ per trial vs Python (95% CI) | Delta, micro-$ per trial (95% CI) | Thinking tok per call | Delta vs Python (95% CI) | Calls that think | Tokens per thinking call | Tasks thinking more / less (sign test p) |
|---|---|---|---|---|---|---|---|
| c | 1.69x (1.33–2.14) | +218.8 (+102.5 to +359.7) | 295 | +241 (+116 to +385) | 82% | 359 | 16 / 0 (p = 3.1e-05) |
| go | 1.41x (1.17–1.75) | +130.4 (+50.9 to +245.2) | 185 | +131 (+29 to +258) | 56% | 329 | 11 / 2 (p = 0.022) |
| python | 1.00x (baseline) | – | 55 | – | 37% | 147 | – |
| rust | 1.20x (1.12–1.29) | +64.7 (+40.0 to +91.1) | 118 | +64 (+24 to +110) | 61% | 194 | 11 / 2 (p = 0.022) |
| sley | 1.25x (1.10–1.45) | +77.9 (+32.3 to +137.1) | 206 | +152 (+101 to +203) | 91% | 227 | 18 / 0 (p = 7.6e-06) |
| ts | 1.15x (1.08–1.23) | +46.3 (+24.4 to +77.9) | 108 | +53 (+10 to +110) | 52% | 207 | 10 / 5 (p = 0.3) |
