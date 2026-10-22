# Cross-language execution benchmark

A development measurement of how fast `sley-agent call` runs small integer programs, compared
with other language runtimes on the same machine. It is not a GA claim and not a claim about
typical workloads.

- `progs/*.py` are the Sley programs, written in the Sley Python dialect. The same files run
  unchanged under CPython and PyPy (`progs/runpy.py`). `bench.lua`, `bench_jit.lua` (LuaJIT has
  no `//`), `bench.js` and `bench.rb` are ports of the same algorithms. Every language prints the
  same answer. `progs/native/bench.c` and `bench.rs` are the same algorithms in C and Rust,
  built by `run.sh` (gcc `-O2`; rustc `-C opt-level=3`, and once more with
  `-C overflow-checks=on`, the `rust_checked` column, whose integers fail on overflow as Sley's
  do; Python's grow instead) for the `c`, `rust` and `rust_checked` languages.
- `run.sh <sley-agent> <out-dir> [langs...]` commits each program to a fresh workspace with the
  given `sley-agent`. It then times every language end to end with hyperfine (2 warmups, at
  least 10 runs, pinned to one CPU with `taskset`; `BENCH_CPU` picks the CPU), and records
  `sley-agent`'s in-VM time (`vm_micros`) over 10 runs.
- `summarize.py <out-dir>` prints the median and range table and the ratio table.

`sley-agent call` has a fixed 100,000,000-fuel ceiling, so the inputs are small. End-to-end times
include process start. `fib 1` is the startup row. The Python dialect lowers the Euclid helper in
`gcdsum.py` to the builtin `gcd` expression, so that benchmark makes no calls.

Results for each release are in its release notes ([2.1.3](../../docs/release/SLEY-2.1.3.md#benchmarks)).
