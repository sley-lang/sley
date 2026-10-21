# Token cost of an agent loop: Python vs Sley forms

This is a static, reproducible count of the tokens an agent writes and reads to get one function right. It is **not** a live-model cost: no model is called, the loop is scripted, and tokens are counted with tiktoken. The live-model authoring-cost benchmark is `bench/authoring-cost/`.

## Method

- **Tasks** (`tasks/`): fib, loopsum, collatz, primes, gcdsum (the runtime benchmarks), plus parse_int (a parser), transform (a data transform), records (a record type) and errors (error handling). Every form implements the same function, with the same semantics and the same test cases (`spec.py`).
- **Forms:**
  - `python`: typed idiomatic Python, with a pytest file (`test_<task>.py`).
  - `sley_py`: the Sley Python dialect. It is tried, then tests are layered as an AF1-X `test_tables` frame with `try --on dN`.
  - `sley_py_inline`: the same dialect file with the pytest test function in it. One `try --familiar` compiles it and runs its tests.
  - `sley_fam`: familiar syntax, with an AF1 `types` frame where the program needs a declared type.
  - `sley_af1`: one AF1-X JSON frame with `test_tables`.
- **Scenarios**, per task and form:
  - `clean`: the program is correct.
  - `logic`: one deliberate wrong-answer bug, caught by the tests.
  - `typo`: one undefined name.

  The loop is: write → run → read the output → apply the fix (counted as old+new snippet, str_replace style) → run → read. Each bug is the same edit in every form (`spec.BUGS`).
- **Counted:** everything written (program, tests, frames, commands, the fix) and everything read (tool output, verbatim: `transcripts/`). pytest runs with `-q` and `PYTHONDONTWRITEBYTECODE=1`, and its timings are normalized to `0.01s`. The one-time guide (`help python`, etc.) is reported separately, not in the loop totals.
- **Tokenizers:** tiktoken 0.14.0 with `o200k_base` and `cl100k_base`. Python 3.13.5, pytest 9.1.1.
- **Run:** `python run_loop.py <sley-agent> <out>`, with `FORMS="..."` to choose the forms. Then `PY_TYPES=0|1 python summarize.py <sley-agent> <out>`. `PY_TYPES=1` is for a binary whose dialect skips record classes (2.1.3).

**Fairness notes**

- The Python dialect lowers `raise` to a trap, so it gets no types frame for errors/parse_int. An earlier draft of this harness wrongly charged it one.
- The Python form keeps the conventional separate test file, which costs a `from <task> import ...` line per task (about 10 tokens). The inline form doesn't need that line.
- In the dialect, `pytest.raises(E)` is checked statically: the called function (or one it calls) must be able to raise `E`. At run time any trap still satisfies it, because the dialect lowers every exception to the same trap code.

## Results (o200k_base; cl100k_base is within 1%; see `results/*/SUMMARY.md`)

The program text alone is the same as Python's for the dialect: 636 vs 636 tokens over 9 tasks on this branch, and 677 on 2.1.3 because of the records types frame. Familiar syntax is 1.33x and AF1-X JSON is 2.60x.

Agent-loop totals over 9 tasks (×Python):

| scenario | Python | 2.1.3 dialect | branch dialect (layered tests) | branch dialect (inline tests) | branch familiar | branch AF1-X |
|---|---|---|---|---|---|---|
| clean | 1338 | 4317 (3.23x) | 3496 (2.61x) | **1980 (1.48x)** | 3921 (2.93x) | 3220 (2.41x) |
| logic bug | 2690 | 6747 (2.51x) | 5196 (1.93x) | **3358 (1.25x)** | 5646 (2.10x) | 4967 (1.85x) |
| typo | 3935 | 6239 (1.59x) | 4900 (1.25x) | **3380 (0.86x)** | 5334 (1.36x) | 4752 (1.21x) |

Tool calls: Python 9/18/18, inline dialect 9/18/18, layered dialect 19/28/28.

**Where Sley is still heavier:**

- A passing `try` prints about 90 tokens; `pytest -q` prints about 15. Most of the gap is the candidate line, `changed:`, the advisory line and `next:`.
- An agent new to Sley reads `help python` once (299 tokens).
- Reading a compiled function back (`view`) is 4.6–92x the source size.

**Where Sley is lighter:** errors. pytest's traceback for a typo is longer than Sley's located `unknown name` error.

### Terse `try` (stacked on the branch above; `results/terse-branch/`)

These are retained measurements of the original terse-output branch. Later
correctness and help-text changes have not been recounted in this snapshot.

A passing `try` prints one line plus `next:`. Failing tests name the source line. `pytest.raises` is checked statically.

| scenario | Python | dialect, inline tests | dialect, layered tests | familiar | AF1-X |
|---|---|---|---|---|---|
| clean | 1338 | **1473 (1.10x)** | 2991 (2.24x) | 3406 (2.55x) | 2705 (2.02x) |
| logic bug | 2690 | **2916 (1.08x)** | 4691 (1.74x) | 5131 (1.91x) | 4452 (1.66x) |
| typo | 3934 | **2873 (0.73x)** | 4395 (1.12x) | 4819 (1.22x) | 4237 (1.08x) |

In this snapshot, `help python` was 322 tokens, read once.

## 2.1.5 branch vs 2.1.4, and other languages (`results/2.1.4-langs/`, `results/2.1.5-branch/`)

**What changed on the branch.**

- A valid `try` whose tests ran prints only the failures and one count line, for example `c1: 4 passed (advisory) d1@r1`.
- Familiar text refused at a source position prints `path:line:col: message`, the source line and a caret.
- Familiar syntax declares `struct` and `enum` types inline. This is the new `sley_fam_decl` form, which needs no AF1 `types` frame.

**Other languages.**

- Same tasks, semantics and test cases in C, Rust, Go and TypeScript (`tasks/*/{c.c,rust.rs,go.go,ts.ts}`), written idiomatically. The tests use each language's usual runner, with the real toolchain output read back verbatim:
  - C: `gcc -O2 -Wall` with an `assert` test file. Note that C's `assert` stops at the first failure, so its logic-bug output is short.
  - Rust: `cargo test -q`.
  - Go: `go test`.
  - TypeScript: `tsc && node --test --test-reporter=dot`. This is a quiet reporter, matching `pytest -q`.
- Toolchain setup (`cargo init`, `go mod init`, the tsconfig) is not counted.
- Timings and thread ids in the output are normalized.
- Toolchains: gcc 14.2.0, rustc 1.95.0-nightly, go 1.27.2, node 20.19.2, tsc 5.9.3.

**Run:** `NODE_MODULES=<dir with typescript and @types/node> PYTEST_PYTHON=<python with pytest> FORMS="python sley_py sley_py_inline sley_fam sley_fam_decl sley_af1 c rust go ts" python run_loop.py <sley-agent> <out>`, then `DECL=1 PY_TYPES=0 python summarize.py <sley-agent> <out>`.

Agent-loop totals over 9 tasks, o200k_base. cl100k_base is within 2%.

| scenario | Python | dialect inline 2.1.4 → 2.1.5 | dialect layered 2.1.4 → 2.1.5 | familiar 2.1.4 → 2.1.5 (decl) | AF1-X 2.1.4 → 2.1.5 | C | Rust | Go | TS |
|---|---|---|---|---|---|---|---|---|---|
| clean | 1338 | 1473 → **1275 (0.95x)** | 2991 → 2793 | 3406 → 2806 | 2705 → 2507 | 1735 (1.30x) | 2216 (1.66x) | 2228 (1.67x) | 1612 (1.20x) |
| logic bug | 2690 | 2916 → **1997 (0.74x)** | 4691 → 4493 | 5131 → 4531 | 4452 → 4254 | 2327 (0.87x) | 3646 (1.36x) | 2959 (1.10x) | 3667 (1.36x) |
| typo | 3932 | 2873 → **1796 (0.46x)** | 4395 → 3318 | 4819 → 3345 | 4237 → 4039 | 2951 (0.75x) | 3754 (0.95x) | 2569 (0.65x) | 1989 (0.51x) |

Program text alone, 9 tasks, o200k_base:

| Python | Sley dialect | familiar with struct/enum | C | Rust | Go | TS |
|---|---|---|---|---|---|---|
| 636 | 636 (1.00x) | 765 (1.20x) | 883 (1.39x) | 798 (1.25x) | 822 (1.29x) | 765 (1.20x) |

In this scripted loop, the dialect with inline tests now uses the fewest tokens of any form or language in all three scenarios. The program text is the same size as Python's. The savings come from what the agent reads back.

The one-time guide is not included in these totals: `help python` is 407 tokens. A model already knows the other languages.
