# Sley 2.1.7 release notes

Sley 2.1.7 is a performance point release. Python-dialect list indexing by a loop counter no
longer converts the index on every access, and the VM's word tier now runs vector length and
element reads and fuses more of each burst. Program meaning is unchanged, and fuel, instruction,
value-unit and output-unit accounting match `call --reference`. The kernel, SSMC1, SCB1, SMP1
rules, the instruction set, admission and commit semantics are untouched.

## Python dialect: loop-counter index shadow (#69)

`xs[i]` with an `int` (`i64`) loop counter used to lower through a `to(u64, i)` conversion on
every access. A `range` loop whose body converts its counter now carries a `u64` copy that steps
with the counter, and offsets such as `xs[i + 1]` or `xs[i - 1]` read from it directly. An
`enumerate` loop gets an `i64` copy of its index. Out-of-range and negative indexes still fail as
before. A new test compares each fast program with a twin that defeats the shadow, and checks
that results match (failing cases included), that fuel and instructions match
`call --reference`, and that the fast twin runs fewer instructions.

## VM: vector reads on the word tier and more fusion per burst (#70)

- **Vector reads.** `Vec<word>` arguments and `Option<word>` values are word-tier types. The new
  word ops `VecLen` and `VecGet` read a vector in place, and `Some`/`None` switches map onto the
  Ok and Err edges. A function that uses a vector in any other way keeps the 2.1.6 path.
- **More fusion.** A conditional now fuses one side whenever either side can fuse. Guards merge
  into the step that produced the guarded slot, and edge moves merge into their producer step.
  Moves into slots that are never read are dropped.

A new differential test runs vector reads at budgets of 1, 2, 3 and unlimited over empty,
single-element, mixed-sign and min/max vectors, against the reference interpreter.

## Benchmarks

These are development measurements of `sley-agent call` on eleven small integer programs. They are
not a GA claim and say nothing about typical workloads. Both Sley columns are the static musl
`sley-agent` binaries from the 2.1.6 and 2.1.7 release archives, measured in one session with
nothing else running. Five programs are new in this release's suite: tak, triples, digitsum,
isqrt and inversions (the inversion count over a list, at 1000 and 400 elements).

**Plainly:**

- **Against 2.1.6**, seven of the nine programs that ran on both releases are 1.09-1.26x faster.
  The inversion count now finishes: 2.1.6 stops at the 100,000,000 call fuel limit at both sizes.
- **Regression:** `fib(30)` is about 4% slower (76.4 vs 73.2 ms, 0.96x). The other call-heavy
  program, tak, is unchanged (0.99x). The likely cause is the larger fused step and the code
  layout around the call path.
- **Against CPython 3.13**, with process start included, 2.1.7 is faster on ten of the eleven
  programs (0.48-0.92x of CPython's time). It is **slower on digitsum** (53.1 vs 48.5 ms, 1.10x).
- **Against Node and Ruby**, 2.1.7 is faster on every program except `fib(30)` against Node (1.04x)
  and tak against Node (0.99x, parity). Much of that margin is their process start.
- **PyPy** is faster on loopsum, digitsum and isqrt. Sley is faster on the other eight.
- **C, Rust, Go, LuaJIT and Lua 5.4 are faster on every program**, by 1.9-58x.

Wall time in ms, hyperfine median [min-max] (2 warmups, at least 10 runs, process start
included):

| benchmark | Sley 2.1.6 | Sley 2.1.7 | 2.1.6 / 2.1.7 | CPython 3.13.5 | PyPy 7.3.19 | Lua 5.4.7 | LuaJIT 2.1 | Node 20.19 | Ruby 3.3.8 | Go 1.27 | C gcc -O2 | Rust -O |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fib 30 | 73.2 [72.6-105] | 76.4 [75.2-85.5] | 0.96x | 82.7 [81.2-85.0] | 169 [164-186] | 37.2 [36.8-45.2] | 7.8 [7.4-19.7] | 73.2 [70.4-108] | 93.9 [93.2-148] | 4.2 [3.9-57.4] | 1.4 [1.3-3.5] | 2.4 [2.3-76.1] |
| loopsum 1e6 | 40.7 [40.2-129] | 34.4 [33.8-44.2] | 1.18x | 57.8 [55.7-65.5] | 30.9 [30.5-34.0] | 8.4 [8.0-9.0] | 8.6 [8.4-9.1] | 78.8 [76.6-83.0] | 69.9 [68.9-75.4] | 3.9 [3.6-13.6] | 3.4 [3.3-28.3] | 3.7 [3.5-84.7] |
| collatz 5000 | 30.8 [30.3-68.0] | 27.1 [26.7-40.3] | 1.14x | 35.3 [34.8-41.8] | 29.3 [28.7-34.6] | 8.0 [7.7-11.2] | 2.5 [2.3-2.9] | 67.7 [65.3-70.6] | 55.9 [54.3-72.1] | 1.2 [1.2-12.3] | 1.1 [1.1-91.6] | 0.81 [0.77-1.7] |
| primes 30000 | 30.4 [30.0-49.7] | 27.9 [27.5-36.5] | 1.09x | 40.9 [40.4-42.9] | 31.1 [30.5-38.1] | 7.6 [7.4-8.4] | 2.5 [2.4-3.0] | 67.7 [65.3-70.1] | 58.5 [56.4-68.5] | 2.1 [2.0-4.1] | 1.7 [1.7-27.6] | 1.3 [1.2-7.9] |
| gcdsum 400 | 31.2 [30.8-86.2] | 25.6 [25.2-30.4] | 1.22x | 42.2 [41.8-43.1] | 69.2 [67.0-83.7] | 12.1 [11.9-12.9] | 6.1 [5.9-7.4] | 72.5 [70.6-83.8] | 63.7 [62.5-78.2] | 4.6 [4.3-111] | 4.0 [3.9-8.9] | 3.4 [3.4-15.9] |
| tak 24 16 8 | 68.9 [68.0-199] | 69.6 [67.8-82.3] | 0.99x | 80.0 [78.4-86.5] | 249 [238-275] | 35.9 [35.0-52.2] | 8.7 [7.8-11.7] | 70.3 [68.9-81.2] | 94.2 [93.7-260] | 5.2 [5.0-5.6] | 3.0 [3.0-10.1] | 3.4 [3.3-9.8] |
| triples 120 | 15.4 [14.9-70.8] | 12.2 [11.8-14.5] | 1.26x | 25.5 [25.0-30.9] | 29.1 [28.7-32.4] | 4.7 [4.4-16.1] | 1.2 [0.92-9.9] | 68.3 [65.8-87.3] | 50.6 [49.9-54.0] | 0.72 [0.68-1.7] | 0.46 [0.43-1.2] | 0.52 [0.49-1.1] |
| digitsum 150000 | 59.8 [59.1-82.6] | 53.1 [52.5-55.8] | 1.12x | 48.5 [47.0-56.6] | 30.9 [29.8-191] | 9.5 [9.1-13.3] | 2.3 [2.2-3.6] | 72.9 [68.9-165] | 66.2 [64.5-80.1] | 1.2 [1.1-2.8] | 2.3 [2.2-2.9] | 0.91 [0.87-2.2] |
| isqrt 50000 | 47.3 [46.6-49.2] | 39.9 [39.5-42.0] | 1.19x | 57.7 [56.9-59.0] | 33.5 [32.7-85.6] | 13.4 [13.3-84.7] | 3.7 [3.6-4.1] | 70.6 [69.1-77.9] | 68.1 [67.5-131] | 1.2 [1.2-61.3] | 2.2 [2.1-55.7] | 0.98 [0.94-53.7] |
| inversions 1000 | 98.9 [96.6-110] (out of fuel) | 19.7 [19.4-57.0] | 2.1.6 out of fuel | 30.0 [29.5-35.9] | 30.1 [29.9-34.4] | 5.8 [5.7-6.9] | 2.5 [2.4-3.2] | 67.9 [65.6-75.9] | 55.9 [54.7-59.1] | 0.85 [0.79-27.0] | 1.2 [1.1-44.8] | 0.63 [0.60-79.6] |
| inversions 400 | 97.3 [96.2-121] (out of fuel) | 8.5 [8.4-12.3] | 2.1.6 out of fuel | 16.9 [16.6-79.4] | 30.0 [29.4-91.2] | 1.6 [1.5-56.0] | 1.1 [1.0-12.5] | 69.7 [66.1-91.3] | 43.1 [42.6-56.7] | 0.66 [0.62-1.4] | 0.44 [0.41-4.8] | 0.47 [0.43-8.4] |

Sley 2.1.7 median relative to each runtime (below 1 means Sley is faster):

| benchmark | CPython | PyPy | Lua | LuaJIT | Node | Ruby | Go | C | Rust |
|---|---|---|---|---|---|---|---|---|---|
| fib 30 | 0.92x | 0.45x | 2.06x | 9.84x | 1.04x | 0.81x | 18x | 57x | 32x |
| loopsum 1e6 | 0.60x | 1.12x | 4.10x | 4.01x | 0.44x | 0.49x | 8.93x | 10x | 9.35x |
| collatz 5000 | 0.77x | 0.92x | 3.39x | 11x | 0.40x | 0.48x | 22x | 24x | 33x |
| primes 30000 | 0.68x | 0.90x | 3.65x | 11x | 0.41x | 0.48x | 13x | 16x | 21x |
| gcdsum 400 | 0.61x | 0.37x | 2.12x | 4.24x | 0.35x | 0.40x | 5.52x | 6.35x | 7.46x |
| tak 24 16 8 | 0.87x | 0.28x | 1.94x | 7.99x | 0.99x | 0.74x | 13x | 23x | 21x |
| triples 120 | 0.48x | 0.42x | 2.59x | 10x | 0.18x | 0.24x | 17x | 27x | 24x |
| digitsum 150000 | 1.10x | 1.72x | 5.62x | 23x | 0.73x | 0.80x | 43x | 23x | 58x |
| isqrt 50000 | 0.69x | 1.19x | 2.98x | 11x | 0.56x | 0.59x | 33x | 18x | 41x |
| inversions 1000 | 0.66x | 0.65x | 3.37x | 7.85x | 0.29x | 0.35x | 23x | 17x | 31x |
| inversions 400 | 0.50x | 0.28x | 5.34x | 7.80x | 0.12x | 0.20x | 13x | 19x | 18x |

In-VM time from `sley-agent --json call` (`vm_micros`, median of 10) goes from 2.1.6 to 2.1.7 as
follows:

| program | 2.1.6 | 2.1.7 |
|---|---|---|
| fib 30 | 71.4 | 75.0 |
| loopsum | 38.6 | 32.0 |
| collatz | 24.8 | 21.0 |
| primes | 26.0 | 24.1 |
| gcdsum | 26.3 | 20.7 |
| tak | 66.9 | 67.5 |
| triples | 10.8 | 7.8 |
| digitsum | 55.2 | 48.9 |
| isqrt | 43.0 | 36.0 |
| inversions 1000 | 81.9, then the fuel limit | 13.1 |

Fuel and instruction counts are identical in the two releases for the nine programs without list
indexing. For example, `fib 30` takes 40,388,044 fuel and 14,808,950 instructions in both. For the
inversion count, 2.1.7 takes 51,048,577 fuel and 4,007,971 instructions at n=1000, and 8,203,516
fuel and 646,212 instructions at n=400. Every program returns the same answer in every runtime.

How it was measured:

- **Programs.** Sley runs the Python-dialect files, and the same files run unchanged under CPython
  and PyPy. The Lua, LuaJIT, Node, Ruby, C, Rust and Go versions are ports of the same algorithms.
  Lua 5.4 and LuaJIT 2.1 run the same file; C is gcc 14.2 `-O2`, Rust is 1.93 with `opt-level=3`
  and unchecked arithmetic, and Go is 1.27.3.
- **Runs.** hyperfine 1.19.0 with `-N`, pinned to one vCPU with `taskset -c 3`, on an 8-vCPU
  "Intel Xeon Processor" cloud VM (Linux 6.12, Debian 13), on 2026-10-27. One shared VM is a noisy
  host: see the ranges.
- **Limits.** `sley-agent call` has a fixed 100,000,000-fuel ceiling, which keeps the inputs small
  and limits the O(n²) inversion count to about n≈1400.
- **Records.** The raw hyperfine and `vm_micros` records and every program are in
  [`evidence/bench/runtime/2.1.7`](../../evidence/bench/runtime/2.1.7). The 2.1.6 binary's records
  from the same session are in its `sley-2.1.6/` folder.

The word tier applies only when every value in the program fits a machine word or is a vector of
words read by `len` and indexing. Programs that use records, enums, strings or build lists run on
the scalar loop and are not measured here.

## Token cost of an agent loop

This is a scripted, static count, **not model spend**: no model is called, the write-test-fix
loop is scripted, and tokens are counted with tiktoken (`o200k_base`). The harness is
[`bench/token-cost`](../../bench/token-cost), with five new tasks (tak, triples, digitsum, isqrt,
inversions) beside the nine of the 2.1.5 notes. Each of the 14 tasks is written with its tests,
run, its output read verbatim, one fix applied (counted as old+new snippet, str_replace style),
and run and read again. Everything written and read is counted. The loop ran on the 2.1.7
archive `sley-agent`. Totals over the 14 tasks, with the ratio to Python:

| scenario | Python (pytest -q) | **Sley 2.1.7 dialect, inline tests** | C (gcc, assert) | Rust (cargo test -q) | Go (go test) | TypeScript (node --test, dot) |
|---|---|---|---|---|---|---|
| clean | 2054 | **1953 (0.95x)** | 2576 (1.25x) | 3362 (1.64x) | 3315 (1.61x) | 2490 (1.21x) |
| logic bug | 4063 | **3093 (0.76x)** | 3475 (0.86x) | 5581 (1.37x) | 4460 (1.10x) | 5703 (1.40x) |
| typo | 6064 | **2768 (0.46x)** | 4412 (0.73x) | 5220 (0.86x) | 3833 (0.63x) | 3065 (0.51x) |
| all three | 12181 | **7814 (0.64x)** | 10463 (0.86x) | 14163 (1.16x) | 11608 (0.95x) | 11258 (0.92x) |

In this scripted loop, the dialect with inline tests uses the fewest tokens of the six languages
in every scenario. Tool calls are the same for every language: 14, 28 and 28. The VM changes in
this release do not change what `try` prints, so the Sley counts equal those of the
pre-release build.

Caveats:

- These are scripted tokenizer counts, not tokens a model spent, and no model wrote or fixed
  these programs. The live-model benchmark is `bench/authoring-cost/`.
- The one-time guide an agent reads to write the dialect, `help python` (407 tokens), is excluded
  from the loop totals. A model needs it for Sley but not for the other languages.
- C's logic-bug loop is cheap because `assert` stops at the first failure, so it prints one
  failure where the other runners print all of them.
- TypeScript uses Node's quiet `dot` reporter, chosen to match `pytest -q`.
- Toolchains: Python 3.13.5 with pytest 9.2.0, gcc 14.2.0, Rust 1.93, Go 1.27.3, Node 20.19.2;
  tiktoken 0.14.0. `cl100k_base` gives the same ratios within 0.01 (0.64x over all three
  scenarios).

The raw rows and transcripts, and the five new tasks, are in
[`evidence/bench/token-cost/2.1.7`](../../evidence/bench/token-cost/2.1.7).

## Compatibility

Programs written for 2.1.6 keep their meaning and their answers. Fuel and instruction counts are
unchanged for every program in the speed suite except those that index lists by a loop counter,
which now spend less: the inversion count over 400 elements finishes in 8,203,516 fuel and 646,212
instructions, where 2.1.6 stops at the 100,000,000 call fuel limit. Both match
`call --reference`. Schema epoch, SSMC1, SCB1, SMP1,
validation semantics, the instruction set and the protocol method table are unchanged. No
third-party dependency was added to the binaries. `ga_claimed` remains `false`.

The native test supervisor is still source only, as in 2.1.2. It is not in the release archive
and not qualified on an installed host, so committing a candidate that selects TestCases still
refuses with `TXN_TEST_EVIDENCE_UNSUPPORTED`.

## Design-specification checks

For Sley 2.1.7, `scripts/check_candidate_result_contract.py` and
`scripts/check_transaction_contract.py` were not run, because the Sley 2.0 design specification
(`SLEY2_DESIGN_SPEC`) is unavailable off the maintainer host. This uses the waiver carried forward
from the operator decision of 2026-10-19 (Magister) for 2.1.2; the operator has not re-confirmed
it for 2.1.7. It waives the two checks for this release only and is not a pass. Both passed at
v2.1.6 on the maintainer host. The machine summary's `release_decision` records this.

## Candidate packaging

The archive is `sley-2.1.7-linux-x86_64.tar.gz`. It contains static musl Linux x86_64 `sley`
and `sley-agent` binaries. From a clean checkout:

```sh
cargo fetch --locked
python3 scripts/build_release_candidate.py --require-clean
sha256sum dist/sley-2.1.7-linux-x86_64.tar.gz
```

The Linux x86_64 archive was built from `f5ef71254c57cb8fdbef33bc6077287cc1cdaa43`. Two clean
builds on the primary host produced identical bytes. Cross-host reproducibility is not claimed.
The archive SHA-256 is `a44b76e070228e2924c7efe16319d5339b233abf1e45986431c59a4c88460923`
(7,677,962 bytes, 17 members). Provenance remains unsigned.

The persistent-fuzz proofs for this release (all 18 lanes) were run at the built commit with
nightly-2026-02-27 and Debian clang 18.1.8. Clang was supplied through the documented
`SLEY_FUZZ_CC`/`SLEY_FUZZ_LIBFUZZER_A` override, because the runners' canonical Debian layout
refuses Debian's clang banner. The proofs record `toolchain_overridden: true`. The GA acceptance
report is unchanged from 2.1.6: of 52 criteria, 31 are evidenced, 17 await review and 4 are
gated.
