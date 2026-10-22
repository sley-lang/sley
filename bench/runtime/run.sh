#!/usr/bin/env bash
# Cross-language execution benchmark (development measurement, not a GA claim).
# Usage: bench/runtime/run.sh <sley-agent> <out-dir> [langs...]
# Needs hyperfine; each language is optional (pass the ones installed).
# Every program prints the same answer in every language; the Sley programs are
# the Python-dialect files in progs/, which also run unchanged under CPython/PyPy.
set -euo pipefail
SLEY=$(readlink -f "$1"); OUT=$(mkdir -p "$2" && cd "$2" && pwd); shift 2
LANGS=${*:-"sley python3 pypy3 lua5.4 luajit node ruby"}
P=$(cd "$(dirname "$0")/progs" && pwd)
CPU=${BENCH_CPU:-3}
SIZES=("fib 30" "loopsum 1000000" "collatz 5000" "primes 30000" "gcdsum 400" "fib 1")
WS=$OUT/ws
# Native baselines, built here: C with gcc -O2 and Rust with opt-level 3, plus a
# Rust build with overflow checks on (the checked integers Sley and Python have).
if [[ " $LANGS " == *" c "* ]]; then mkdir -p "$OUT/native" && gcc -O2 -o "$OUT/native/bench_c" "$P/native/bench.c"; fi
if [[ " $LANGS " == *" rust"* ]]; then
  mkdir -p "$OUT/native"
  rustc -C opt-level=3 -o "$OUT/native/bench_rs" "$P/native/bench.rs"
  rustc -C opt-level=3 -C overflow-checks=on -o "$OUT/native/bench_rs_checked" "$P/native/bench.rs"
fi
for name in fib loopsum collatz primes gcdsum; do
  rm -rf "$WS/$name"; mkdir -p "$WS/$name"
  (cd "$WS/$name" && "$SLEY" init >/dev/null && "$SLEY" try --familiar "$P/$name.py" >/dev/null && "$SLEY" commit c1 >/dev/null)
done
for x in "${SIZES[@]}"; do
  set -- $x; name=$1; n=$2
  cmds=(); names=()
  for L in $LANGS; do
    case $L in
      sley)          cmds+=("cd $WS/$name && $SLEY --json call $name $n");;
      python3|pypy3) cmds+=("$L $P/runpy.py $name $n");;
      lua5.4)        cmds+=("lua5.4 $P/bench.lua $name $n");;
      luajit)        cmds+=("luajit $P/bench_jit.lua $name $n");;
      node)          cmds+=("node $P/bench.js $name $n");;
      ruby)          cmds+=("ruby $P/bench.rb $name $n");;
      c)             cmds+=("$OUT/native/bench_c $name $n");;
      rust)          cmds+=("$OUT/native/bench_rs $name $n");;
      rust_checked)  cmds+=("$OUT/native/bench_rs_checked $name $n");;
    esac
    names+=(-n "$L")
  done
  taskset -c "$CPU" hyperfine --warmup 2 --min-runs 10 --export-json "$OUT/${name}_$n.json" \
    "${names[@]}" "${cmds[@]}" > "$OUT/${name}_$n.txt" 2>&1
done
# In-VM time (vm_micros) reported by sley-agent, 10 runs each.
if [[ " $LANGS " == *" sley "* ]]; then
  for x in "${SIZES[@]}"; do set -- $x
    for _ in $(seq 10); do (cd "$WS/$1" && taskset -c "$CPU" "$SLEY" --json call "$1" "$2"); done \
      > "$OUT/vm_${1}_$2.jsonl"
  done
fi
echo "results in $OUT"
