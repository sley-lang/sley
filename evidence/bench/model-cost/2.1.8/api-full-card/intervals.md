# Each language against Python: paired bootstrap over tasks

10000 draws, seed 218. Each draw resamples tasks with replacement and keeps all of a task's trials in both languages. Intervals are 95% percentile intervals. The per-trial delta is unaffected by overhead both languages pay equally (such as the Claude Code CLI's own prompt); the ratio is diluted by it.

## claude-haiku-5-5

| Language | $ per trial vs Python (95% CI) | Delta, micro-$ per trial (95% CI) | Thinking tok per call | Delta vs Python (95% CI) | Calls that think | Tokens per thinking call | Tasks thinking more / less (sign test p) |
|---|---|---|---|---|---|---|---|
| c | 2.74x (2.03–3.76) | +164.2 (+93.8 to +247.1) | 188 | +169 (+71 to +290) | 50% | 376 | 10 / 0 (p = 0.002) |
| go | 1.96x (1.40–2.79) | +90.8 (+33.4 to +168.6) | 123 | +105 (+7 to +232) | 28% | 444 | 5 / 0 (p = 0.062) |
| python | 1.00x (baseline) | – | 19 | – | 6% | 335 | – |
| rust | 1.72x (1.30–2.23) | +67.6 (+32.9 to +103.7) | 69 | +50 (-6 to +112) | 30% | 232 | 6 / 1 (p = 0.12) |
| sley | 1.27x (0.91–1.78) | +25.4 (-9.1 to +64.5) | 65 | +47 (-1 to +99) | 23% | 287 | 5 / 2 (p = 0.45) |
| ts | 1.47x (1.35–1.60) | +44.0 (+29.5 to +61.3) | 51 | +32 (+6 to +66) | 19% | 275 | 5 / 0 (p = 0.062) |
