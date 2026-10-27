# Sley 2.1.3 release notes

Sley 2.1.3 is a performance point release. The VM runs call-heavy and loop-heavy programs
several times faster than 2.1.2. Fuel, instruction counts, value-unit and output-unit accounting
are unchanged, and so is what a committed program means: the kernel, SSMC1, SCB1, SMP1 rules,
the instruction set, admission and commit semantics are untouched.

## Performance

- **Direct calls in the compact scalar plan** (#46). The VM's compact scalar plan used to refuse
  any function containing `call_direct`, so every recursive or call-heavy program ran on the
  general reference interpreter. The plan now compiles the entry function and every function it
  reaches. If any of them is ineligible, the whole execution keeps the reference path, as before.
  Calls follow contract E6 exactly as the reference interpreter runs them: the depth ceiling is
  checked before the call action, each argument's value units are charged, the callee's return
  is measured against the output limit, and the returned value's units and the call instruction
  are charged on return. Frames live on an explicit stack, not on host recursion, and returned
  register files are reused. A new test sweeps every fuel, instruction, cancellation,
  value-unit and output cut for a direct call, a nested call and unbounded self-recursion
  (`CallDepth`), and compares the plan's observations with the reference interpreter's.
- **Faster scalar loops** (#45, #46). The plan has a native fast path for 64-bit checked
  arithmetic, checked against the general 128-bit path in a differential test. It moves
  block-edge arguments directly when they cannot overlap, and its registers are 32 bytes
  instead of 48, with no padding. A register's one-byte shape used to leave 7 padding bytes,
  which the compiler copied as overlapping moves that stalled on store forwarding.
- **Python `%` and `//`** (#44). The dialect computed the remainder up to three times per
  operation and tested a literal divisor's sign at run time. It now holds one remainder and
  folds a literal divisor's sign. Results, traps (`/ 0`, `MIN // -1`) and CPython floor
  semantics are unchanged; fewer instructions and less fuel are spent.

## Benchmarks

These are development measurements of `sley-agent call` on five small integer programs. They are
not a GA claim and say nothing about typical workloads. **Sley is still 2.7-4.6x slower than
CPython 3.13 on every program measured here**, and slower than every other runtime measured
except at process start. Against 2.1.2, `fib(30)` runs about 13x faster and the loops about
2.7-3.6x faster.

Wall time in ms, hyperfine median [min-max] (2 warmups, at least 10 runs, process start
included). Both Sley columns are the published static musl `sley-agent` binaries from the 2.1.2
and 2.1.3 archives, measured in the same session.

| benchmark | Sley 2.1.2 | Sley 2.1.3 | CPython 3.13.5 | PyPy 7.3.19 | Lua 5.4.7 | LuaJIT 2.1 | Node 20.19 | Ruby 3.3.8 |
|---|---|---|---|---|---|---|---|---|
| fib 30 | 2739 [2706-2861] | 210 [208-229] | 78 [76-82] | 174 [170-186] | 38 [37-58] | 7.8 [7.4-13] | 78 [73-106] | 97 [94-108] |
| loopsum 1e6 | 665 [639-723] | 183 [180-238] | 54 [51-64] | 22 [21-28] | 8.5 [8.0-13] | 8.9 [8.4-9.5] | 88 [82-93] | 70 [69-73] |
| collatz 5000 | 488 [477-521] | 144 [139-189] | 31 [30-37] | 22 [20-29] | 8.6 [8.0-15] | 2.5 [2.3-3.4] | 72 [69-78] | 55 [54-62] |
| primes 30000 | 424 [417-467] | 147 [143-167] | 37 [36-41] | 22 [21-32] | 7.7 [7.3-8.4] | 2.5 [2.3-6.2] | 75 [69-243] | 57 [56-61] |
| gcdsum 400 | 374 [368-395] | 138 [133-206] | 38 [37-52] | 78 [72-89] | 13 [12-15] | 6.2 [5.9-7.4] | 79 [76-101] | 63 [62-77] |
| fib 1 (startup) | 1.7 [1.4-8.6] | 1.7 [1.4-2.6] | 9.2 [8.9-12] | 15 [15-19] | 0.6 [0.6-1.5] | 0.6 [0.5-1.6] | 68 [64-76] | 39 [39-42] |

Sley 2.1.3 median relative to each runtime (above 1 means Sley is slower):

| benchmark | CPython | PyPy | Lua | LuaJIT | Node | Ruby |
|---|---|---|---|---|---|---|
| fib 30 | 2.7x | 1.2x | 5.6x | 27x | 2.7x | 2.2x |
| loopsum 1e6 | 3.4x | 8.1x | 22x | 21x | 2.1x | 2.6x |
| collatz 5000 | 4.6x | 6.7x | 17x | 57x | 2.0x | 2.6x |
| primes 30000 | 4.0x | 6.7x | 19x | 60x | 2.0x | 2.6x |
| gcdsum 400 | 3.6x | 1.8x | 11x | 22x | 1.7x | 2.2x |

In-VM time from `sley-agent --json call` (`vm_micros`, median of 10) for `fib 30` is 2,740 ms
in 2.1.2 and 209 ms in 2.1.3. Both report the same fuel (40,388,044) and instructions
(14,808,950). For loopsum, collatz and primes, 2.1.3 spends less fuel and runs fewer
instructions because of the `%`/`//` lowering change (#44). For example, loopsum drops from
90,000,012 to 64,000,012 fuel and from 21,000,003 to 11,000,003 instructions. Every program
returns the same answer in every runtime.

How it was measured: Sley programs are the Python-dialect files in
[`bench/runtime/progs`](../../bench/runtime), and the same files run unchanged under CPython and
PyPy. The Lua, LuaJIT, Node and Ruby versions are ports of the same algorithms. Each run is
`bench/runtime/run.sh` pinned to one vCPU with `taskset`, on an 8-vCPU "Intel Xeon Processor"
cloud VM (Linux 6.12, Debian 13) with hyperfine 1.19.0, on 2026-10-20. One shared VM is a noisy
host: see the ranges. `sley-agent call` has a fixed 100,000,000-fuel ceiling, which keeps the
inputs small. The Python dialect lowers `gcdsum`'s Euclid helper to the builtin `gcd`
expression, so that program makes no calls. The raw hyperfine and `vm_micros` records are in
[`evidence/bench/runtime`](../../evidence/bench/runtime).

Where the remaining gap is: the scalar plan is still an interpreter over 32-byte tagged
registers, with per-instruction fuel and value-unit accounting. CPython's specializing
interpreter, and especially the JITs (PyPy, LuaJIT, V8), do far less work per operation. Closing
the gap would take a register-typed plan (untagged machine words where the types are known) and
batched fuel charging that keeps the accounting contract exact.

## Compatibility

Programs written for 2.1.2 keep their meaning and their answers. A program's fuel and
instruction counts are unchanged, except that Python-dialect programs using `%` or `//`
compile to fewer instructions and spend less fuel. Schema epoch, SSMC1, SCB1, SMP1, validation
semantics, the instruction set, the protocol method table and the `sley-agent` JSON shape are
unchanged. No third-party dependency was added. `ga_claimed` remains `false`.

The native test supervisor is still source only, as in 2.1.2. It is not in the release archive and
not qualified on an installed host, so committing a candidate that selects TestCases still
refuses with `TXN_TEST_EVIDENCE_UNSUPPORTED`.

## Waived checks

Operator decision 2026-10-20 (Magister), carried forward from the 2.1.2 waiver of 2026-10-19: for
Sley 2.1.3, `scripts/check_candidate_result_contract.py` and `scripts/check_transaction_contract.py`
are not run because the Sley 2.0 design specification is unavailable off the
maintainer host. This waives the two checks for this release only and is not a pass; both passed
at v2.1.1 on the maintainer host. The waiver is also recorded in the machine summary's
`release_decision`.

## Candidate packaging

The archive is `sley-2.1.3-linux-x86_64.tar.gz`. It contains static musl Linux x86_64 `sley`
and `sley-agent` binaries. From a clean checkout:

```sh
cargo fetch --locked
python3 scripts/build_release_candidate.py --require-clean
sha256sum dist/sley-2.1.3-linux-x86_64.tar.gz
```

The Linux x86_64 archive was built from `03387499cacb33c9fee2f97abdc09a3e5574f7e2`. Two clean
builds on the primary host produced identical bytes. Cross-host reproducibility is not claimed.
The archive SHA-256 is `6fd18c3c19b5a4891fbcb2e9703130af52b0fe54e777d3e449d0d375dc43c9ae`
(7,656,373 bytes, 17 members). Provenance remains unsigned.

The persistent-fuzz proofs for this release were run at the built commit with nightly-2026-02-27
and Debian clang 18.1.8 through the fuzz runners' documented toolchain override, as for 2.1.2.
Releases before 2.1.2 used the canonical Arch toolchain layout on the maintainer host. As at
2.1.2, the GA acceptance report shows 22 criteria not evidenced and 30 open review obligations.

The release's own CodeQL run covers the release branch. #46 merged without a CodeQL run of its
own, because it was opened against #45's branch and retargeted to `main` after its last push.
Its code was covered by the workspace tests and the strict Clippy gate.
