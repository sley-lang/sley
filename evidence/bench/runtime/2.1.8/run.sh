#!/usr/bin/env bash
# Sley 2.1.7 vs 2.1.8 (release-archive musl sley-agent) vs CPython 3.13, one session, 3 rounds, order rotated.
set -euo pipefail
SU=$(cd "$(dirname "$0")/../2.1.7/suite" && pwd)
OUT=$PWD/out; mkdir -p $OUT
declare -A A=([217]=${DIST:?set DIST to the unpacked archives dir}/sley-2.1.7-linux-x86_64/bin/sley-agent [218]=${DIST:?set DIST to the unpacked archives dir}/sley-2.1.8-linux-x86_64/bin/sley-agent)
PROGS=("fib 30" "loopsum 1000000" "collatz 5000" "primes 30000" "gcdsum 400" "tak 24 16 8" "triples 120" "digitsum 150000" "isqrt 50000" "inversions xs1000.json" "inversions xs400.json")
for v in 217 218; do for name in fib loopsum collatz primes gcdsum tak triples digitsum isqrt inversions; do
  d=$OUT/ws$v/$name; rm -rf $d; mkdir -p $d; (cd $d && ${A[$v]} init >/dev/null && ${A[$v]} try --familiar $SU/$name.py >/dev/null && ${A[$v]} commit c1 >/dev/null); done; done
for round in 1 2 3; do
 for x in "${PROGS[@]}"; do
  set -- $x; name=$1; shift; args=""; pargs="$*"
  for a in "$@"; do if [[ $a == *.json ]]; then args+=" '$(cat $SU/$a)'"; else args+=" $a"; fi; done
  tag=$name; [[ $pargs == *xs400* ]] && tag=inv400
  c217="cd $OUT/ws217/$name && ${A[217]} --json call $name$args"
  c218="cd $OUT/ws218/$name && ${A[218]} --json call $name$args"
  cpy="python3.13 $SU/runpy.py $name $(for a in "$@"; do [[ $a == *.json ]] && echo $SU/$a || echo $a; done | tr '\n' ' ')"
  case $round in 1) o=(217 218 py);; 2) o=(218 py 217);; 3) o=(py 217 218);; esac
  cmds=(); names=()
  for k in "${o[@]}"; do case $k in 217) cmds+=("$c217"); names+=(-n sley-2.1.7);; 218) cmds+=("$c218"); names+=(-n sley-2.1.8);; py) cmds+=("$cpy"); names+=(-n cpython-3.13);; esac; done
  taskset -c 3 hyperfine -N 2>/dev/null >/dev/null || true
  taskset -c 3 hyperfine --warmup 2 --min-runs 10 --export-json $OUT/${tag}_r$round.json "${names[@]}" "${cmds[@]}" > $OUT/${tag}_r$round.txt 2>&1
 done
done
for v in 217 218; do for x in "${PROGS[@]}"; do set -- $x; name=$1; shift; args=""; for a in "$@"; do if [[ $a == *.json ]]; then args+=" $(cat $SU/$a)"; else args+=" $a"; fi; done
  tag=$name; [[ "$*" == *xs400* ]] && tag=inv400
  for _ in $(seq 10); do (cd $OUT/ws$v/$name && eval taskset -c 3 ${A[$v]} --json call $name "'${args# }'" 2>/dev/null || true); done > $OUT/vm${v}_$tag.jsonl; done; done
echo done
