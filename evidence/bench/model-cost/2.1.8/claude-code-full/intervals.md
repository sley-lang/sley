# Each language against Python: paired bootstrap over tasks

10000 draws, seed 218. Each draw resamples tasks with replacement and keeps all of a task's trials in both languages. Intervals are 95% percentile intervals. The per-trial delta is unaffected by overhead both languages pay equally (such as the Claude Code CLI's own prompt); the ratio is diluted by it.

## claude-code-haiku-5-5

| Language | $ per trial vs Python (95% CI) | Delta, micro-$ per trial (95% CI) | Thinking tok per call | Delta vs Python (95% CI) | Calls that think | Tokens per thinking call | Tasks thinking more / less (sign test p) |
|---|---|---|---|---|---|---|---|
| c | 1.65x (1.29–2.15) | +190.4 (+84.1 to +338.1) | 267 | +185 (+88 to +292) | 80% | 333 | 17 / 0 (p = 1.5e-05) |
| go | 1.48x (1.23–1.74) | +138.9 (+61.9 to +243.0) | 225 | +144 (+42 to +264) | 69% | 326 | 14 / 2 (p = 0.0042) |
| python | 1.00x (baseline) | – | 82 | – | 50% | 163 | – |
| rust | 1.15x (1.00–1.32) | +44.2 (-0.5 to +90.2) | 113 | +32 (-47 to +117) | 54% | 211 | 10 / 4 (p = 0.18) |
| sley | 1.20x (1.04–1.41) | +59.7 (+14.3 to +108.6) | 186 | +105 (+49 to +151) | 86% | 217 | 16 / 2 (p = 0.0013) |
| ts | 1.10x (1.02–1.17) | +27.8 (+6.2 to +54.0) | 97 | +16 (-19 to +59) | 44% | 219 | 9 / 5 (p = 0.42) |
