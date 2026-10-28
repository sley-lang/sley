# Runtime records for the Sley 2.1.7 release notes

hyperfine JSON (`<program>.json`, 2 warmups, at least 10 runs, `taskset -c 3`, process start
included) and `sley-agent --json call` records (`vm_<program>.jsonl`, 10 runs) for the 2.1.7
release-archive `sley-agent` and every other runtime, with the 2.1.6 archive binary's records
from the same session in `sley-2.1.6/`. `suite/` holds the programs: the Python-dialect files
(run unchanged by CPython and PyPy through `runpy.py`), the Lua (`bench.lua`, `bench_jit.lua` for
LuaJIT), Node, Ruby, C, Rust and Go ports, the sizes and the inversion inputs. See
docs/release/SLEY-2.1.7.md.
