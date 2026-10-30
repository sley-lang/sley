# Raw API against the Claude Code route (same card, same tasks)

Both runs: Claude Haiku 5.5 at effort low, full tier (18 tasks x 6 languages x 3
reps = 324 trials), Sley card `help python-task` with SHA-256
`e26c1bfc…b4c00f` in both manifests.

| | Raw API (this directory) | Claude Code CLI (`claude-code-full-card/`, #84) |
|---|---|---|
| Route | Anthropic Messages API, `--no-batch`, billed per token | `claude -p` in a Claude Code cloud session |
| Dollars | billed usage, list price ($0.05169 in all) | estimated at API list price from reported tokens ($0.13183) |
| sley-agent | main 470bc29 (card unchanged since d0c5bd7) | main d0c5bd7 |
| Correct | 324 / 324 | 324 / 324 |
| Run | 30 Oct 2026, 17:36 to 17:40 EDT | 29 Oct 2026 |

Message Batches were off so that every call is priced like the CLI's
estimate (standard rate); the pre-registered Batch discount would halve the
first-attempt price for every language alike.

## Each language against Python (paired bootstrap over tasks, 95% intervals)

| Language | API: $ per trial vs Python | CLI: $ per trial vs Python | API: delta µ$ per trial | CLI: delta µ$ per trial | API: calls that think | CLI: calls that think | API: thinking tok per call | CLI: thinking tok per call |
|---|---|---|---|---|---|---|---|---|
| python | 1.00x | 1.00x | – | – | 6% | 37% | 19 | 55 |
| sley | 1.27x (0.91–1.78) | 1.25x (1.10–1.45) | +25.4 (−9.1 to +64.5) | +77.9 (+32.3 to +137.1) | 23% | 91% | 65 | 206 |
| ts | 1.47x (1.35–1.60) | 1.15x (1.08–1.23) | +44.0 (+29.5 to +61.3) | +46.3 (+24.4 to +77.9) | 19% | 52% | 51 | 108 |
| rust | 1.72x (1.30–2.23) | 1.20x (1.12–1.29) | +67.6 (+32.9 to +103.7) | +64.7 (+40.0 to +91.1) | 30% | 61% | 69 | 118 |
| go | 1.96x (1.40–2.79) | 1.41x (1.17–1.75) | +90.8 (+33.4 to +168.6) | +130.4 (+50.9 to +245.2) | 28% | 56% | 123 | 185 |
| c | 2.74x (2.03–3.76) | 1.69x (1.33–2.14) | +164.2 (+93.8 to +247.1) | +218.8 (+102.5 to +359.7) | 50% | 82% | 188 | 295 |

## What differs

1. **The CLI route thinks far more.** At effort low, 6% of Python calls think
   on the API against 37% through the CLI; Sley 23% against 91%. Thinking per
   call is 1.5 to 3.2 times higher through the CLI (highest for Sley and
   Python). The CLI's
   own context changes the model's behaviour, not only the bill.
2. **Sley's thinking gap shrinks on the API.** +47 tokens per call
   (−1 to +99) against +152 (+101 to +203) through the CLI. Sley thought
   more than Python on 5 tasks and less on 2 (sign test p = 0.45), against
   18 and 0 through the CLI.
3. **Ratios for other languages are diluted through the CLI.** The CLI adds
   about 220 to 280 µ$ per trial to every language (Python costs 94 µ$ per
   trial on the API, 320 µ$ through the CLI), which pulls every ratio
   toward 1. Per-trial deltas are not diluted, and for TypeScript and
   Rust they agree across routes.
4. **Ranking.** On the API, Sley is the cheapest language after Python
   (1.27x; TypeScript 1.47x, Rust 1.72x). Through the CLI it was behind
   TypeScript and Rust. Sley's interval on the API includes 1.0.

## Where Sley's API gap comes from (µ$ per trial, `cost-breakdown.md`)

| | uncached input | cache reads | thinking | visible output | total |
|---|---|---|---|---|---|
| python | 23.4 | 0.0 | 9.3 | 61.5 | 94.2 |
| sley | 18.8 | 6.7 | 34.5 | 59.6 | 119.6 |
| Sley − Python | −4.6 | +6.7 | +25.2 | −1.9 | +25.4 |

The whole gap is thinking (+25.2) and the cached card (+6.7); Sley's visible
output and uncached prompt cost less than Python's.

## Limits

- One run per route on 18 v1 tasks, whose held-out tasks are spent (audit
  by Claude Code, 30 Oct 2026). Sley's API interval half-width is about 0.43,
  far from the 2.2 gate of 0.07; task set v2 and repeated same-run arms are
  needed before any per-feature claim.
- The runs are a day apart and on different builds of an unchanged card.
