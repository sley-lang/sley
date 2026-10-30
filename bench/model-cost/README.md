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
| Models | `claude-haiku-5-5` (Anthropic Messages API, effort low, **Message Batches API for first attempts**, a prompt-cache breakpoint on the system block) and `grok-build-0.1` (xAI chat completions; xAI has no Batch API). Optional: `gpt-6-luna` (OpenAI Responses + Batch; the default until 29 Oct 2026, see `prereg.json` amendments) and `deepseek-flash`, off-peak only. |
| Tasks | 18: 8 from the token-loop benchmark, 5 new, 5 held out (2 are edits of an existing file). Small tier: 15 (drops fib, collatz, isqrt). |
| Trials | small: 15 tasks x 6 languages x 1 = 90 per model; full: 18 x 6 x 3 = 324 per model |
| Attempts | 1 + at most 2 retries; a retry gets the visible test or compiler output (4,000 characters at most) |
| Output cap | 2,000 tokens including reasoning; a reply cut off by the cap is a failed attempt (and still bills) |
| Sley card | `sley-agent help python-task` (the dialect card without the inline-test syntax, since the harness supplies the tests; `help python` before 29 Oct 2026, see `prereg.json` amendments) is sent with every Sley attempt, billed, first in the prompt so it can be cached |
| Oracle | 128 seeded cases per task (seed 218), edge cases first, from `tasks.reference`; independent of the visible tests |
| Success | the final program passes the visible tests and all 128 hidden cases |
| Exclusions | provider or network failures and guard stops are recorded and shown, and left out of the denominators |
| Spend guard | $10 per run: each call reserves its worst case (prompt + full cap at list price) and is refused if spend could pass the guard; the ledger persists |

**Metric:** dollars per correct program = all billed spend for a model x language
(failed trials included) / its correct programs. Also reported: success and
first-try rates, attempts, dollars per trial, input / cached / output tokens.

## Cost projection (real prices, checked 29 Oct 2026)

`python run.py --project --tier small|full [--models ...]` prints it. Per
attempt: 1,000 prompt tokens + the 407-token card on one language in six (68 on
average), 1,200 output tokens; 1.3 attempts per trial expected (the first, plus
0.3 retries at the standard rate). Claude Haiku 5.5 uses a newer tokenizer that
produces about 30% more tokens for the same text, so its token counts are
scaled by 1.3 (1,300 in, 88 card, 1,560 out). Its first attempts go through
the Message Batches API at half price and pay the card as a cache write
(batch cache hits are best effort); its retries read the card from the cache.

| | Haiku 5.5 (Batch) first attempt | Haiku 5.5 retry | grok-build-0.1 attempt |
|---|---|---|---|
| prompt | 1,300 x $0.05 | 1,300 x $0.10 | 1,000 x $1.00 |
| card | 88 x $0.0625 (write) | 88 x $0.01 (read) | 68 x $0.20 (cached) |
| output | 1,560 x $0.25 | 1,560 x $0.50 | 1,200 x $2.00 |
| per attempt | $0.000461 | $0.000911 | $0.003414 |
| per trial (first + 0.3 retries) | $0.000734 | | $0.004438 |

| Run | Trials per model | Haiku 5.5 | grok-build-0.1 | Expected | Ceiling |
|---|---|---|---|---|---|
| small | 90 | $0.07 | $0.40 | **$0.47** | $2.97 |
| full | 324 | $0.24 | $1.44 | **$1.68** | $10.67, so the $10 guard binds |
| full + deepseek-flash off-peak | 324 | | | $2.04 | $12.63 |

Ceiling: every trial takes 3 attempts at the full 2,000-token output cap, and
attempt n's prompt carries every earlier reply (2,000 tokens) and its feedback
(about 1,400 tokens); on Claude every prompt token is priced as a cache write.
(Before 29 Oct the ceiling held every prompt at 2,000 tokens, which understated
it.) The guard stops a run before billed spend could pass $10.

## Layout

| File | |
|---|---|
| `tasks.py` | the 18 tasks, the reference oracle and the seeded hidden cases |
| `refs/<task>/` | a reference program per language (checked by `verify_refs.py`) |
| `starters/<task>/` | the existing file for the two edit tasks |
| `langs.py` | signatures, visible tests, toolchain runs and oracle drivers per language |
| `providers.py` | Anthropic (Messages + Message Batches, prompt caching), Claude Code CLI, OpenAI (Responses + Batch), xAI and DeepSeek clients; `MockTransport` |
| `accounting.py` | usage normalization, pricing, the spend guard |
| `run.py` | the run (`--project` for the projection) |
| `summarize.py` | `summary.json` and `summary.md` |
| `intervals.py` | `intervals.json` and `intervals.md`: each language against Python, paired bootstrap over tasks (95% intervals for the $ per trial ratio, the per-trial delta and the thinking delta; share of calls that think; sign test). `summarize.py` runs it. |
| `taskset-v2/` | task set v2 for 2.2 (design rules, independently authored statements, not yet pre-registered); see its README |
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
ANTHROPIC_API_KEY=... XAI_API_KEY=... python run.py --paid --tier small --out evidence/bench/model-cost/2.1.8/small
```

First a one-call pilot per provider (`--tasks fib --langs python,sley --models
claude-haiku-5-5 --no-batch`, then the same with `--models grok-build-0.1`).
The Claude pilot shows whether the Sley system block (494 tokens in o200k)
reaches Haiku 5.5's 512-token caching minimum: `cache_creation_input_tokens`
or `cache_read_input_tokens` above zero on the Sley call. The xAI pilot
confirms how xAI reports reasoning tokens: by default reasoning is billed on
top of `completion_tokens` (the worse case); pass `--reasoning-in-completion`
if the pilot's usage shows it is already included. Publish `results.jsonl`,
`summary.json`, `summary.md` and `manifest.json` under
`evidence/bench/model-cost/2.1.8/`.

## Claude Code route (`claude-code-haiku-5-5`)

An optional model that reaches Claude Haiku 5.5 through the logged-in Claude
Code CLI instead of the API, so a Claude subscription or Claude Code
cloud-session credit pays for it. Its rows are labeled **"Claude Code cloud
session: tokens as reported; $ estimated at API list price, not billed per
token"** and are never pooled with API-billed rows.

Each call is

```sh
CLAUDE_CODE_MAX_OUTPUT_TOKENS=2000 claude -p --output-format json --model claude-haiku-5-5 \
  --tools "" --system-prompt "<the harness system text, with the card on Sley>" --effort low \
  --safe-mode --disable-slash-commands --no-session-persistence --strict-mcp-config   # < prompt on stdin
```

run in an empty temporary directory. Flags the installed CLI does not list in
`claude --help` are dropped (and recorded in the calibration entry); a CLI
without `--output-format`, `--system-prompt`, `--tools` or `--model` is refused.

What this removes from Claude Code's own scaffolding: its system prompt
(replaced), every tool and MCP server, CLAUDE.md, skills, plugins, hooks and
custom agents (`--safe-mode`), session files, and any project files. What may
remain is whatever the CLI still adds to each request. A **calibration call**
at the start of every run (system "Reply with the single word OK.", prompt
"OK?", about 10 tokens of text) measures it: its reported input tokens minus
about 10 is the per-call overhead, written to `calibration.json` and to the
summary. The calibration call is counted in the run's spend.

Other differences from the API route, disclosed with the results:
- No Batch API: every call is priced at the standard rate.
- `claude -p` takes one prompt, so a retry sends the task, the earlier
  replies and their feedback as text (`<previous_attempt>`/`<feedback>`
  blocks) instead of as separate turns.
- Usage comes from the CLI's JSON `usage` (Messages API field names). A call
  whose `modelUsage` shows any model other than the pinned one is rejected as
  an infrastructure failure, so a fallback model cannot leak into the results.
- The output cap is set through `CLAUDE_CODE_MAX_OUTPUT_TOKENS`; a reply at
  the cap counts as truncated.

Run it where `claude` is logged in (for example inside a Claude Code cloud
session), or from another machine with `CLAUDE_CODE_SSH=user@host`:

```sh
python run.py --paid --models claude-code-haiku-5-5 --tasks fib,parse_int --langs python,sley --jobs 2 --out /tmp/cc-pilot
python run.py --paid --models claude-code-haiku-5-5 --tier small --jobs 2 --out ../../evidence/bench/model-cost/2.1.8/claude-code-small
python summarize.py ../../evidence/bench/model-cost/2.1.8/claude-code-small
```

`run.py --project --models claude-code-haiku-5-5` estimates $0.11 for the
small tier and $0.39 for the full tier at list price. Those are estimates for
the benchmark calls only. In a cloud session, the session's own agent also
uses credit, and that is usually much more.

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

Claude's usage is billed as the Messages API reports it: `input_tokens` (the
uncached prompt), `cache_creation_input_tokens` (5-minute writes at 1.25x
input, 1-hour writes at 2x), `cache_read_input_tokens` (0.1x) and
`output_tokens`, which include adaptive thinking. Haiku 5.5 cannot turn
thinking off; effort `low` keeps it short, and it counts toward the 2,000-token
cap. A prompt over 100,000 tokens would switch Haiku 5.5 to its long-context
prices; `accounting.py` applies them, though no prompt here comes close.
