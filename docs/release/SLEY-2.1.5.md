# Sley 2.1.5 release notes

Sley 2.1.5 is a point release with two parts. The VM gains a **word tier** for programs whose
values all fit a machine word, and the release binary is built with fat LTO. Fuel,
instruction, value-unit and output-unit accounting is unchanged. `sley-agent try` prints less,
familiar text is refused with compiler-style diagnostics, and familiar syntax declares `struct`
and `enum` types. The kernel, SSMC1, SCB1, SMP1 rules, the instruction set, admission and
commit semantics are untouched.

## VM performance (#54, #56, #57)

- **Word tier (#54).** When every register of every function is an `i64`/`u64`, a `bool`, a
  checked result read only by its block's switch, or that result's failure code, the scalar
  plan runs on plain `u64` registers in one contiguous register file. Signedness is resolved at
  compile time, and checked arithmetic branches straight to its Err edge. Up to 64 whole blocks
  run back to back against one prepaid worst case. Any other program keeps the 2.1.4 scalar loop.
- **Fused fast blocks (#56).** Constants are written once per frame. A fast block continues
  through unconditional branches, a checked switch's Ok edge and loop exits, with a guard on
  each; a failed guard stops with exactly the charges of the blocks already run.
- **Calls and returns inside bursts (#57).** A direct call, the callee's frame and the return
  run inside the burst, with the same charges in the same order as the general loop. So
  `down(255)` runs and `down(256)` stops at `CallDepth`, as in 2.1.4.
- **Release profile (#57).** `[profile.release]` now sets `lto = "fat"` and
  `codegen-units = 1`. The two clean release builds still produce identical archives (see
  [Candidate packaging](#candidate-packaging)).

Fuel and instruction counts are identical to 2.1.4 and to `call --reference` on all five
benchmark programs. New differential tests compare the word tier with the reference runner and
the scalar loop at every fuel, cancel, instruction, value-unit and output cut, including
two-argument call chains to depth 400. Mutation testing on the three PRs killed every mutant
tried (6, 6 and 22).

## Agent output and familiar syntax (#55)

- **Quiet `try`.** A valid candidate whose tests ran prints its failures (familiar text only,
  each with its source line) and one count line naming the candidate and draft, for example
  `c2: 2 passed (advisory) d2@r1`. Test provenance is shown only when tests other than the
  authored ones ran. Replaced or deleted entities are still listed.
- **Compiler-style refusals.** Familiar text refused at a source position prints
  `file:line:col: message`, the source line and a caret, then the draft state. Further
  obligations are counted as `(+N more; --verbose lists them)`.
- **`struct` and `enum` in familiar syntax.** `struct Point { x: i64, y: i64 }` and
  `enum Error { Empty, TooBig }` declare the AF1 `types` entries an AF1 frame would, before or
  between functions. Payload variants, repeated members and empty declarations are refused at
  their position.
- **Token harness.** [`bench/token-cost`](../../bench/token-cost) gains C, Rust, Go and
  TypeScript versions of every task, run with their usual test runners.

## Benchmarks

These are development measurements of `sley-agent call` on five small integer programs. They are
not a GA claim and say nothing about typical workloads. Both Sley columns are the static musl
`sley-agent` binaries from the 2.1.4 and 2.1.5 release archives, measured in one session with
nothing else running.

**Against CPython 3.13, plainly:** with process start included, 2.1.5 is faster on loopsum
(0.75x of CPython's time), primes (0.85x) and gcdsum (0.91x). It is at parity on `fib(30)`
(75.8 vs 78.7 ms, 0.96x, inside both runtimes' run-to-run spread). It is slower on collatz
(35.2 vs 33.0 ms, 1.07x). Against 2.1.4, every program runs 2.2-2.9x faster. PyPy, Lua, LuaJIT,
C and Rust are still faster on the loops.

Wall time in ms, hyperfine median [min-max] (2 warmups, at least 10 runs, process start
included):

| benchmark | Sley 2.1.4 | Sley 2.1.5 | 2.1.4 / 2.1.5 | CPython 3.13.5 | PyPy 7.3.19 | Lua 5.4.7 | LuaJIT 2.1 | Node 20.19 | Ruby 3.3.8 | C gcc -O2 | Rust | Rust, overflow checks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fib 30 | 217 [207-237] | 75.8 [73.3-113] | 2.86x | 78.7 [76.4-91.9] | 184 [144-210] | 39.7 [37.6-78.9] | 8.0 [7.3-11.1] | 80.1 [73.4-108] | 99.9 [93.8-129] | 1.4 | 2.2 | 3.8 |
| loopsum 1e6 | 121 [116-145] | 43.0 [40.8-53.6] | 2.81x | 57.0 [51.3-106] | 20.6 [19.1-25.8] | 8.8 [8.1-18.4] | 8.9 [8.4-18.0] | 91.2 [82.6-148] | 70.3 [67.8-88.2] | 3.5 | 3.9 | 4.2 |
| collatz 5000 | 77.6 [72.0-106] | 35.2 [32.4-46.9] | 2.20x | 33.0 [30.3-56.0] | 23.2 [19.0-34.8] | 8.9 [7.9-43.3] | 2.5 [2.3-12.2] | 72.3 [67.1-81.5] | 56.2 [53.1-74.5] | 1.1 | 0.8 | 1.1 |
| primes 30000 | 89.1 [87.1-130] | 30.8 [30.2-62.0] | 2.89x | 36.2 [35.3-62.2] | 19.8 [18.5-50.2] | 8.0 [7.4-28.6] | 2.7 [2.3-41.8] | 80.3 [72.9-101] | 59.7 [56.5-88.6] | 1.8 | 1.3 | 1.4 |
| gcdsum 400 | 79.6 [73.5-110] | 34.0 [31.5-46.5] | 2.34x | 37.4 [36.6-45.6] | 72.4 [67.5-110] | 12.7 [12.0-24.0] | 6.2 [5.9-7.8] | 83.0 [78.4-94.9] | 66.1 [63.4-70.8] | 4.4 | 3.7 | 3.7 |
| fib 1 (startup) | 1.6 [1.3-17.1] | 1.6 [1.2-16.5] | 1.04x | 9.8 [8.9-29.6] | 16.0 [14.4-32.4] | 0.7 [0.6-6.4] | 0.6 [0.5-14.6] | 73.8 [69.1-97.5] | 44.2 [40.7-58.8] | 0.3 | 0.4 | 0.4 |

Sley 2.1.5 median relative to each runtime (below 1 means Sley is faster):

| benchmark | CPython | PyPy | Lua | LuaJIT | Node | Ruby | C | Rust | Rust, checked |
|---|---|---|---|---|---|---|---|---|---|
| fib 30 | 0.96x (parity) | 0.41x | 1.9x | 9.4x | 0.95x | 0.76x | 56x | 35x | 20x |
| loopsum 1e6 | 0.75x | 2.1x | 4.9x | 4.8x | 0.47x | 0.61x | 12x | 11x | 10x |
| collatz 5000 | 1.07x | 1.5x | 4.0x | 14x | 0.49x | 0.63x | 32x | 45x | 31x |
| primes 30000 | 0.85x | 1.6x | 3.8x | 11x | 0.38x | 0.52x | 17x | 23x | 22x |
| gcdsum 400 | 0.91x | 0.47x | 2.7x | 5.4x | 0.41x | 0.51x | 7.8x | 9.3x | 9.1x |

In-VM time from `sley-agent --json call` (`vm_micros`, median of 10) falls from 216.4 to 76.8 ms
for `fib 30`, 122.0 to 41.5 ms for loopsum, 65.7 to 27.3 ms for collatz, 82.1 to 28.5 ms for
primes and 68.5 to 28.4 ms for gcdsum. Fuel and instruction counts are identical in the two
releases for every program (for example `fib 30`: 40,388,044 fuel and 14,808,950 instructions;
loopsum: 64,000,012 and 11,000,003). Every program returns the same answer in every runtime.

The shipped binary is the static musl build. During development the same source built for
glibc ran loopsum in 36.3 ms against 42.0 ms for musl, in a different session, so a glibc build
from source can be faster than the release archive on some programs. This table measures only
the archive.

How it was measured: as for 2.1.4. Sley programs are the Python-dialect files in
[`bench/runtime/progs`](../../bench/runtime), and the same files run unchanged under CPython and
PyPy. The Lua, LuaJIT, Node, Ruby, C and Rust versions are ports of the same algorithms (C with
gcc 14.2 `-O2`; Rust 1.93 with `opt-level=3`, and again with `-C overflow-checks=on`, which is
closest to Sley's checked integers). Each run is `bench/runtime/run.sh` pinned to one vCPU with
`taskset`, on an 8-vCPU "Intel Xeon Processor" cloud VM (Linux 6.12, Debian 13) with hyperfine
1.19.0, on 2026-10-21. One shared VM is a noisy host: see the ranges. In particular, CPython's
`fib 30` median has measured 77-84 ms across sessions on this host, which is why 0.96x is
reported as parity. The Node column is Node 20 here and Node 22 in the 2.1.4 notes. `sley-agent
call` has a fixed 100,000,000-fuel ceiling, which keeps the inputs small. The raw hyperfine and
`vm_micros` records are in [`evidence/bench/runtime/2.1.5`](../../evidence/bench/runtime/2.1.5),
with the 2.1.4 binary's records from the same session in its `sley-2.1.4/` folder.

The word tier applies only when every value in the program fits a machine word. Programs that use
records, enums, strings or lists run on the 2.1.4 scalar loop and are not measured here.

## Token cost of an agent loop

This is a scripted, static count, **not model spend**: no model is called, the write-test-fix
loop is scripted, and tokens are counted with tiktoken (`o200k_base`). The harness is
[`bench/token-cost`](../../bench/token-cost). Each of 9 tasks is written with its tests, run, its
output read verbatim, one fix applied (counted as old+new snippet, str_replace style) and run and
read again. Everything written and read is counted. Totals over the 9 tasks, ×Python:

| scenario | Python (pytest -q) | Sley 2.1.4 dialect, inline tests | **Sley 2.1.5 dialect, inline tests** | C (gcc, assert) | Rust (cargo test -q) | Go (go test) | TypeScript (node --test, dot) |
|---|---|---|---|---|---|---|---|
| clean | 1338 | 1473 (1.10x) | **1275 (0.95x)** | 1735 (1.30x) | 2216 (1.66x) | 2228 (1.67x) | 1612 (1.20x) |
| logic bug | 2690 | 2916 (1.08x) | **1997 (0.74x)** | 2327 (0.87x) | 3646 (1.36x) | 2959 (1.10x) | 3667 (1.36x) |
| typo | 3933 | 2873 (0.73x) | **1796 (0.46x)** | 2951 (0.75x) | 3754 (0.95x) | 2569 (0.65x) | 1989 (0.51x) |

In this scripted loop, the 2.1.5 dialect with inline tests uses the fewest tokens of the six
languages in all three scenarios. The program text is the same size as Python's (636 tokens over
the 9 tasks; C 883, Rust 798, Go 822, TypeScript 765): the saving is in what Sley prints back.
The 2.1.4 column is from the 2.1.4 release notes, measured on the 2.1.4 binary.

Caveats:

- These are scripted tokenizer counts, not tokens a model spent, and no model wrote or fixed
  these programs. The live-model benchmark is `bench/authoring-cost/`.
- The one-time guide an agent reads to write the dialect, `help python` (407 tokens), is excluded
  from the loop totals. A model needs it for Sley but not for the other languages.
- C's logic-bug loop is cheap because `assert` stops at the first failure, so it prints one
  failure where the other runners print all of them.
- TypeScript uses Node's quiet `dot` reporter, chosen to match `pytest -q`.
- The Python form keeps the conventional separate test file, which costs a `from <task> import`
  line per task (about 10 tokens).
- The native and JSON Sley forms are still heavier than Python except in the typo loop:
  familiar syntax with `struct`/`enum` 2.10x/1.68x/0.85x, AF1-X JSON 1.87x/1.58x/1.03x, and the
  dialect with tests layered as a separate frame 2.09x/1.67x/0.84x (clean/logic/typo). Reading a
  compiled function back (`view`) is still 4.6-92x its source size.
- Toolchains: Python 3.13.5 with pytest 9.1.1, gcc 14.2.0, rustc 1.95.0-nightly, go 1.27.2,
  node 20.19.2 with tsc 5.9.3; tiktoken 0.14.0. `cl100k_base` is within 2%.

The raw rows and transcripts are in
[`evidence/bench/token-cost/2.1.5`](../../evidence/bench/token-cost/2.1.5).

## Compatibility

Programs written for 2.1.4 keep their meaning and their answers, and their fuel and instruction
counts are unchanged. The human-readable `try` output and the default text of a familiar
refusal are shorter. `--verbose` and `--json` are unchanged and still give the full report,
the advisory policy, `next:` and every pointer; scripts should read `--json`. A familiar
declaration starting with `struct` or `enum` now declares a type. Schema epoch, SSMC1, SCB1,
SMP1, validation semantics, the instruction set and the protocol method table are unchanged. No
third-party dependency was added to the binaries. `ga_claimed` remains `false`.

The native test supervisor is still source only, as in 2.1.2. It is not in the release archive and
not qualified on an installed host, so committing a candidate that selects TestCases still
refuses with `TXN_TEST_EVIDENCE_UNSUPPORTED`.

## Waived checks

Carried forward from the operator decision of 2026-10-19 (Magister) for Sley 2.1.2. The operator
has not re-confirmed it for 2.1.3, 2.1.4 or 2.1.5. For Sley 2.1.5,
`scripts/check_candidate_result_contract.py` and `scripts/check_transaction_contract.py` are not
run because the Sley 2.0 design specification (`SLEY2_DESIGN_SPEC`) is unavailable off the maintainer
host. This waives the two checks for this release only and is not a pass; both passed at v2.1.1
on the maintainer host. The waiver is also recorded in the machine summary's `release_decision`.

## Candidate packaging

The archive is `sley-2.1.5-linux-x86_64.tar.gz`. It contains static musl Linux x86_64 `sley`
and `sley-agent` binaries. From a clean checkout:

```sh
cargo fetch --locked
python3 scripts/build_release_candidate.py --require-clean
sha256sum dist/sley-2.1.5-linux-x86_64.tar.gz
```

The Linux x86_64 archive was built from `2a8561dd59c2163249e9ccad6d84ec60bafc2909` with the new
fat-LTO, single-codegen-unit release profile. Two clean builds on the primary host produced
identical bytes, and a third clean build on a freshly installed toolchain produced the same
archive again. Cross-host reproducibility is not claimed. The archive SHA-256 is
`ff003f03674b386d75f721319ff15d2ac019fa0cc4cb0d78554074466a2861ff` (7,657,333 bytes, 17
members). Provenance remains unsigned.

The persistent-fuzz proofs for this release (all 18 lanes) were run at the built commit with
nightly-2026-02-27 and Debian clang 18.1.8 through the fuzz runners' documented toolchain
override, as for 2.1.2 through 2.1.4. As at 2.1.4, the GA acceptance report shows 22 criteria not
evidenced and 30 open review obligations.
