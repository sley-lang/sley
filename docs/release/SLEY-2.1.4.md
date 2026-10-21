# Sley 2.1.4 release notes

Sley 2.1.4 is a point release with two parts. The VM's compact scalar plan runs loops faster,
with fuel, instruction, value-unit and output-unit accounting unchanged. The Python dialect and
`sley-agent try` cut the tokens an agent spends in a write-test-fix loop. The kernel, SSMC1, SCB1,
SMP1 rules, the instruction set, admission and commit semantics are untouched.

## VM performance (#49)

- **Slot coalescing.** A liveness pass gives each register a slot. Registers joined by a block
  edge share a slot when the blocks they are live in are disjoint, so most edge moves become
  no-ops. This is the main gain for Python-dialect loops, which thread every live value through
  each checked operation's continuation.
- **Prepaid fuel batches.** Each block is cut into runs of steps between calls. When a run's
  worst-case fuel, instruction count and value units fit inside every limit, the run skips its
  per-step checks and adds the exact totals once. A fault partway through a run fixes the
  counts up to what the per-step path would have charged.
- **Word results built whole.** The 64-bit fast path builds each value in one store, which
  removes two store-forwarding stalls.

Fuel, instruction counts and value units are identical to `call --reference`. A new agent-level
test checks this on the five benchmark programs at several sizes, and a differential test covers
the word fast path on edge values.

## Python dialect and `try` output (#50, #51)

- **Inline pytest-style tests.** Python-dialect files may contain `def test_*():` functions whose
  lines are `assert f(...) == value`, `assert f(...)`, `assert not f(...)` or
  `with pytest.raises(E): f(...)`. They become TestCases in the same frame, so one
  `try --familiar` compiles and tests the file. `pytest.raises(E)` is checked statically: the
  called function, or a function it calls in the file, must be able to raise `E`. At run time
  any trap still satisfies it, because the dialect lowers every exception to one trap.
- **Record classes.** A class of annotated fields (`@dataclass class Point: x: int`) declares a
  record type. Before, the class was skipped and the type needed a separate AF1 `types` frame.
  Other classes are still skipped.
- **Shorter `try` output.** A Valid candidate whose tests all pass prints one line and `next:`.
  With tests, the advisory test policy fits on one line: still advisory, not admission evidence,
  and only fuel and the fixed VM limits apply. `changed:` collapses constant and numbered-test
  runs. A failing test from familiar or dialect text names its source line (`(line N)`), and
  `next:` says to fix the text and re-run `try --familiar`. `--verbose`, `--json`, `test` and
  `status` still print the full report and policy.
- **Token harness.** [`bench/token-cost`](../../bench/token-cost) is the scripted harness
  behind the token table below.

## Benchmarks

These are development measurements of `sley-agent call` on five small integer programs. They are
not a GA claim and say nothing about typical workloads. **Sley is still about 1.9-2.7x slower
than CPython 3.13 on every program measured here** (1.92x to 2.65x), and slower than every other
runtime measured except at process start. Against 2.1.3, the four loop
programs run 1.56-1.95x faster. `fib(30)`, which is call-bound, is unchanged.

Wall time in ms, hyperfine median [min-max] (2 warmups, at least 10 runs, process start
included). Both Sley columns are the static musl `sley-agent` binaries from the 2.1.3 and 2.1.4
release archives, measured in the same session.

| benchmark | Sley 2.1.3 | Sley 2.1.4 | 2.1.3 / 2.1.4 | CPython 3.13.5 | PyPy 7.3.19 | Lua 5.4.7 | LuaJIT 2.1 | Node 22.23 | Ruby 3.3.8 |
|---|---|---|---|---|---|---|---|---|---|
| fib 30 | 207 [206-218] | 209 [203-226] | 0.99x | 79 [77-110] | 163 [141-183] | 38 [37-42] | 8.0 [7.5-8.9] | 28 [24-34] | 94 [93-98] |
| loopsum 1e6 | 181 [180-266] | 117 [113-121] | 1.56x | 52 [51-61] | 20 [20-77] | 8.6 [8.1-9.3] | 8.6 [8.4-65] | 32 [29-53] | 67 [67-84] |
| collatz 5000 | 137 [136-170] | 71 [68-80] | 1.95x | 30 [30-42] | 19 [18-25] | 8.0 [7.7-9.1] | 2.4 [2.3-3.8] | 20 [18-28] | 52 [52-56] |
| primes 30000 | 143 [137-276] | 84 [83-97] | 1.71x | 36 [35-37] | 19 [19-26] | 7.7 [7.3-11] | 2.4 [2.3-3.2] | 20 [19-27] | 56 [54-77] |
| gcdsum 400 | 132 [130-144] | 71 [70-81] | 1.87x | 37 [36-38] | 60 [55-83] | 12 [12-40] | 6.1 [5.9-89] | 27 [24-39] | 62 [61-78] |
| fib 1 (startup) | 1.6 [1.4-2.1] | 1.7 [1.4-5.6] | 0.94x | 9.2 [9.0-11] | 15 [15-20] | 0.6 [0.6-1.7] | 0.5 [0.5-1.4] | 17 [16-29] | 39 [38-40] |

Sley 2.1.4 median relative to each runtime (above 1 means Sley is slower):

| benchmark | CPython | PyPy | Lua | LuaJIT | Node | Ruby |
|---|---|---|---|---|---|---|
| fib 30 | 2.65x | 1.28x | 5.5x | 26x | 7.6x | 2.2x |
| loopsum 1e6 | 2.24x | 5.8x | 14x | 14x | 3.7x | 1.7x |
| collatz 5000 | 2.33x | 3.8x | 8.9x | 29x | 3.5x | 1.4x |
| primes 30000 | 2.36x | 4.3x | 11x | 34x | 4.2x | 1.5x |
| gcdsum 400 | 1.92x | 1.2x | 5.8x | 12x | 2.6x | 1.1x |

In-VM time from `sley-agent --json call` (`vm_micros`, median of 10) falls from 176.5 to 110.0 ms
for loopsum, 130.0 to 62.6 ms for collatz, 134.7 to 79.4 ms for primes and 129.5 to 64.8 ms for
gcdsum (1.6-2.1x), and from 207.1 to 196.1 ms for `fib 30`. Fuel and instruction counts are
identical in the two releases for every program (for example loopsum: 64,000,012 fuel and
11,000,003 instructions; `fib 30`: 40,388,044 and 14,808,950). Every program returns the same
answer in every runtime. The Node column is Node 22 here and Node 20 in the 2.1.3 notes, so the
two releases' Node ratios are not comparable.

How it was measured: as for 2.1.3. Sley programs are the Python-dialect files in
[`bench/runtime/progs`](../../bench/runtime), and the same files run unchanged under CPython and
PyPy. The Lua, LuaJIT, Node and Ruby versions are ports of the same algorithms. Each run is
`bench/runtime/run.sh` pinned to one vCPU with `taskset`, on an 8-vCPU "Intel Xeon Processor"
cloud VM (Linux 6.12, Debian 13) with hyperfine 1.19.0, on 2026-10-21. One shared VM is a noisy
host: see the ranges. `sley-agent call` has a fixed 100,000,000-fuel ceiling, which keeps the
inputs small. The raw hyperfine and `vm_micros` records are in
[`evidence/bench/runtime/2.1.4`](../../evidence/bench/runtime/2.1.4) (the 2.1.3 binary's records from the same session are in its `sley-2.1.3/` folder).

Where the remaining gap is: the scalar plan is still an interpreter over 32-byte tagged
registers. Calls are the slowest path, which is why `fib 30` did not move. CPython's
specializing interpreter, and especially the JITs (PyPy, LuaJIT, V8), do far less work per
operation.

## Token cost of an agent loop

This is a scripted, static count, **not model spend**: no model is called, the write-test-fix
loop is scripted, and tokens are counted with tiktoken (`o200k_base`). The harness is
[`bench/token-cost`](../../bench/token-cost). Each of 9 tasks is written, run, its output read,
one fix applied (counted as old+new snippet) and run and read again. Everything written and read
is counted. Totals over the 9 tasks, ×Python:

| scenario | Python (pytest) | Sley 2.1.3 dialect | Sley 2.1.4 dialect, inline tests |
|---|---|---|---|
| clean | 1338 | 4317 (3.23x) | **1473 (1.10x)** |
| logic bug | 2690 | 6747 (2.51x) | **2916 (1.08x)** |
| typo | 3933 | 6239 (1.59x) | **2873 (0.73x)** |

Tool calls: Python 9/18/18, 2.1.3 dialect 19/28/28 (tests layered as an AF1-X `test_tables`
frame, the only way 2.1.3 could test dialect text), 2.1.4 inline 9/18/18. The Python column is
the 2.1.4 session; in the 2.1.3 session it was 1338/2690/3935 (pytest output differs by two
tokens). `cl100k_base` is within 1%.

Caveats:

- These are scripted tokenizer counts, not tokens a model spent. The live-model benchmark is
  `bench/authoring-cost/`.
- The Python form keeps the conventional separate test file (`test_<task>.py`), which costs a
  `from <task> import ...` line per task (about 10 tokens). The inline dialect form does not
  need it.
- The one-time guide an agent reads is excluded from the loop totals: `help python` is 407
  tokens in 2.1.4 (252 in 2.1.3).
- The native and JSON forms are still heavier on 2.1.4: familiar syntax 2.55x/1.91x/1.23x of
  Python and AF1-X JSON 2.02x/1.66x/1.08x (clean/logic/typo). The 2.1.4 dialect with tests
  layered the 2.1.3 way is 2.24x/1.74x/1.12x. Reading a compiled function back (`view`) is still
  4.6-92x its source size.
- The typo loop favours Sley because pytest's traceback for an undefined name is longer than
  Sley's located `unknown name` error.

The raw rows and transcripts are in
[`evidence/bench/token-cost/2.1.4`](../../evidence/bench/token-cost/2.1.4).

## Compatibility

Programs written for 2.1.3 keep their meaning and their answers, and their fuel and instruction
counts are unchanged. Two Python-dialect changes can alter how existing text compiles:

- a `def test_*():` function made of the assert forms above now becomes TestCases instead of a
  function;
- a class of annotated fields now declares a record type instead of being skipped.

The human-readable `try` output is shorter. Scripts should read `--json`, which is unchanged. Schema epoch, SSMC1, SCB1, SMP1, validation semantics,
the instruction set and the protocol method table are unchanged. No third-party dependency was
added to the binaries. `ga_claimed` remains `false`.

The native test supervisor is still source only, as in 2.1.2. It is not in the release archive and
not qualified on an installed host, so committing a candidate that selects TestCases still
refuses with `TXN_TEST_EVIDENCE_UNSUPPORTED`. The inline tests are advisory matches, like every
`try` test.

## Waived checks

Carried forward from the operator decision of 2026-10-19 (Magister) for Sley 2.1.2. The operator
has not re-confirmed it for 2.1.3 or 2.1.4. For Sley 2.1.4,
`scripts/check_candidate_result_contract.py` and `scripts/check_transaction_contract.py` are not
run because the Sley 2.0 master goal (`SLEY2_MASTER_GOAL`) is unavailable off the maintainer
host. This waives the two checks for this release only and is not a pass; both passed at v2.1.1
on the maintainer host. The waiver is also recorded in the machine summary's `release_decision`.

## Candidate packaging

The archive is `sley-2.1.4-linux-x86_64.tar.gz`. It contains static musl Linux x86_64 `sley`
and `sley-agent` binaries. From a clean checkout:

```sh
cargo fetch --locked
python3 scripts/build_release_candidate.py --require-clean
sha256sum dist/sley-2.1.4-linux-x86_64.tar.gz
```

The Linux x86_64 archive was built from `a043a1b8355120e22b08f126fedf02d17a16fded`. Two clean
builds on the primary host produced identical bytes. Cross-host reproducibility is not claimed.
The archive SHA-256 is `91364bac4864f001640df127836af18e7bf88fa3131d6c203e7edb8d27de5cf1`
(7,698,959 bytes, 17 members). Provenance remains unsigned.

The persistent-fuzz proofs for this release (all 18 lanes) were run at the built commit with
nightly-2026-02-27 and Debian clang 18.1.8 through the fuzz runners' documented toolchain
override, as for 2.1.2 and 2.1.3. As at 2.1.3, the GA acceptance report shows 22 criteria not
evidenced and 30 open review obligations.
