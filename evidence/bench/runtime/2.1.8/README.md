# Runtime records for the Sley 2.1.8 release notes

hyperfine JSON (`<program>_r<round>.json`, 2 warmups, at least 10 runs, `taskset -c 3`, process
start included) for the 2.1.7 and 2.1.8 release-archive musl `sley-agent` and CPython 3.13.5, in
three rounds with the command order rotated, all in one session on one 8-vCPU cloud VM (the
same kind of machine as the 2.1.7 records). `vm217_<program>.jsonl` and `vm218_<program>.jsonl`
are 10 `sley-agent --json call` records each (fuel, instructions, result, `vm_micros`). The
programs are the 2.1.7 suite (`../2.1.7/suite`). `run.sh` reproduces the session (set `DIST`
to a directory holding both unpacked archives); `summ.py <dir>` prints the table.

`s1-fuzz/gen.py <seed>` writes one random Python-dialect program (five functions mixing signs,
`//`, `%`, range counters, reassignment and `while x > 0` digit loops) with 40 assertions whose
expected values come from CPython. Seeds 1-200 were run with `sley-agent try --familiar`:
8,000 assertions, 0 mismatches.
