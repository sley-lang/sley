# Genuine model cost benchmark (Sley 2.1.8)

What does a **correct program** cost, in **billed dollars**, when a current
low-cost model writes it in Python, Sley (Python dialect), C, Rust, Go or
TypeScript? The token-cost benchmark counts tokens with local tokenizers; this
one sends the work to real models and prices the usage each API bills.

Everything here is fixed before the first paid call: `prereg.json` (design),
`prices.json` (list prices with source URLs and the date they were read),
`tasks.py` (tasks and oracle). The run manifest records their SHA-256.

## Design (pre-registered)

| | |
|---|---|
| Models | `gpt-6-luna` (OpenAI Responses API, reasoning effort low, **Batch API for first attempts**) and `grok-build-0.1` (xAI chat completions; xAI has no Batch API). Optional third: `deepseek-flash`, off-peak only. |
| Tasks | 18: 8 from the token-loop benchmark, 5 new, 5 held out (2 are edits of an existing file). Small tier: 15 (drops fib, collatz, isqrt). |
| Trials | small: 15 tasks x 6 languages x 1 = 90 per model; full: 18 x 6 x 3 = 324 per model |
| Attempts | 1 + at most 2 retries; a retry gets the visible test or compiler output (4,000 characters at most) |
| Output cap | 2,000 tokens including reasoning; a reply cut off by the cap is a failed attempt (and still bills) |
| Sley card | `sley-agent help python` is sent with every Sley attempt, billed, first in the prompt so it can be cached |
| Oracle | 128 seeded cases per task (seed 218), edge cases first, from `tasks.reference`; independent of the visible tests |
| Success | the final program passes the visible tests and all 128 hidden cases |
| Exclusions | provider or network failures and guard stops are recorded and shown, and left out of the denominators |
| Spend guard | $10 per run: each call reserves its worst case (prompt + full cap at list price) and is refused if spend could pass the guard; the ledger persists |

**Metric:** dollars per correct program = all billed spend for a model x language
(failed trials included) / its correct programs. Also reported: success and
first-try rates, attempts, dollars per trial, input / cached / output tokens.

## Cost projection (real prices, checked 28 Oct 2026)

`python run.py --project --tier small|full [--models ...]` prints it. Per
attempt: 1,000 prompt tokens + the 407-token card on one language in six (68 on
average, cached), 1,200 output tokens; 1.3 attempts per trial expected. Ceiling:
3 attempts at 2,000 in and 2,000 out. Luna's first attempts are at the Batch
rate, its retries at the standard rate.

| Run | Models | Expected | Ceiling |
|---|---|---|---|
| small (90 trials/model) | Luna + grok-build | $0.45 | $1.89 |
| full (324 trials/model) | Luna + grok-build | $1.62 | $6.80 |
| full | + deepseek-flash off-peak | $1.99 | $8.26 |

## Layout

| File | |
|---|---|
| `tasks.py` | the 18 tasks, the reference oracle and the seeded hidden cases |
| `refs/<task>/` | a reference program per language (checked by `verify_refs.py`) |
| `starters/<task>/` | the existing file for the two edit tasks |
| `langs.py` | signatures, visible tests, toolchain runs and oracle drivers per language |
| `providers.py` | OpenAI (Responses + Batch), xAI and DeepSeek clients; `MockTransport` |
| `accounting.py` | usage normalization, pricing, the spend guard |
| `run.py` | the run (`--project` for the projection) |
| `summarize.py` | `summary.json` and `summary.md` |
| `test_model_cost.py` | unit tests and mock end-to-end runs |

## Running

Toolchains: `SLEY_AGENT` (sley-agent binary), `PYTEST_PYTHON` (a Python with
pytest), `NODE_MODULES` (typescript and @types/node), and gcc, rustc, go, node
on PATH.

```sh
python verify_refs.py                         # 108 reference programs vs the oracle
python -m pytest test_model_cost.py           # no network
python run.py --out /tmp/dry --tier small     # dry run: in-process mock, no key, no spend
python summarize.py /tmp/dry
```

A dry run exercises every request, Batch, retry, guard and scoring path
against `MockTransport` and prices its synthetic usage at real list prices; its
numbers describe the mock, not any model.

**Paid run** (only after the keys are confirmed): re-read the price pages and
update `prices.json` if anything changed, then

```sh
OPENAI_API_KEY=... XAI_API_KEY=... python run.py --paid --tier small --out evidence/bench/model-cost/2.1.8/small
```

First a one-call pilot per provider (`--tasks fib --langs python --models grok-build-0.1`)
to confirm how xAI reports reasoning tokens: by default reasoning is billed on
top of `completion_tokens` (the worse case); pass `--reasoning-in-completion`
if the pilot's usage shows it is already included. Publish `results.jsonl`,
`summary.json`, `summary.md` and `manifest.json` under
`evidence/bench/model-cost/2.1.8/`.

## Conventions

Inputs are integers and lists of integers; results are integers. Errors:
Python raises `ValueError`, Sley `raise`s (traps), C returns non-zero from
`int f(..., long long *out)`, Rust returns `Err`, Go returns a non-nil
`error`, TypeScript throws. A panic or another exception type fails the case.

## Disclosures

The held-out tasks were written for this release, but they are classic
problems, so training-data contamination is possible. The visible tests are
shown to the model; the hidden cases are not. Box timings play no part: the
metric is dollars, from billed usage.
