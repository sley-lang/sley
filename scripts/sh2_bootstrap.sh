#!/usr/bin/env bash
# SH2 bootstrap proof: Sley builds Sley.
#
#   C1 = C0.build(S, P)       native seed build (sley-sh2 stage)
#   C2 = H.run(C1, S)         pinned host, seed absent, sandboxed
#   C3 = H.run(C2, S)         pinned host, seed absent, sandboxed
#
# The fixed point requires C2 == C3 byte for byte. Under the unique
# deterministic SLEYBC02/EXEC_PACKAGE_V3 encoding, C1 == C2 is required too.
#
# Stages 2 and 3 run inside bubblewrap with a root filesystem that contains
# only the statically linked host binary, the staged build request S, the
# preceding toolchain package, and an output directory: no Rust toolchain,
# no Cargo, no Rust sources, no native compiler service, no network, no
# shell. The host admits a package only when its identity is pinned on the
# command line.
#
# Usage: scripts/sh2_bootstrap.sh WORKDIR
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
work="${1:?usage: sh2_bootstrap.sh WORKDIR}"
mkdir -p "$work"
work="$(cd "$work" && pwd)"
target="$repo/target-bootstrap"
cage=(systemd-run --user --scope -p MemoryMax=6G --quiet)

log() { printf '[sh2] %s\n' "$*" | tee -a "$work/bootstrap.log"; }

log "repository $(git -C "$repo" rev-parse HEAD) dirty=$(git -C "$repo" status --porcelain | wc -l)"

# 1. Tools: the C0-side staging tool and the static pinned host.
log "building sley-sh2 (C0 side) and the static musl sley-host"
CARGO_TARGET_DIR="$target" "${cage[@]}" cargo build --release -j2 -p sley-selfhost --bin sley-sh2 >>"$work/bootstrap.log" 2>&1
CARGO_TARGET_DIR="$target" RUSTFLAGS="-C target-feature=+crt-static" "${cage[@]}" \
  cargo build --release -j2 -p sley-host --target x86_64-unknown-linux-musl >>"$work/bootstrap.log" 2>&1
sh2="$target/release/sley-sh2"
host="$target/x86_64-unknown-linux-musl/release/sley-host"
mkdir -p "$work/bin"
cp "$host" "$work/bin/sley-host"
log "host binary sha256 $(sha256sum "$work/bin/sley-host" | cut -d' ' -f1)"
file "$work/bin/sley-host" | tee -a "$work/bootstrap.log"

# Compiler-service absence in the host binary: none of the native semantic
# entry points is linked.
nm -C "$work/bin/sley-host" >"$work/host-symbols.txt" 2>/dev/null || true
for symbol in lower_function judge_bootstrap_profile validate_function_graph \
              judge_extended_operation judge_function_operations admit_v3_package \
              admit_v2_package reference_lower_for_seed; do
  if grep -q "$symbol" "$work/host-symbols.txt"; then
    log "FAIL: host binary links $symbol"
    exit 1
  fi
done
log "host binary links no native compiler service (symbol audit: $(wc -l <"$work/host-symbols.txt") symbols)"

# 2. S and C1 = C0.build(S, P).
log "staging canonical S and building C1 with the native seed (C0)"
rm -rf "$work/stage"
"${cage[@]}" "$sh2" stage --out "$work/stage" | tee "$work/stage-manifest.json"
c1_digest="$(cat "$work/stage/C1.digest")"
log "C1 package digest $c1_digest"

# 3. Seed-absent sandboxed stages.
run_stage() {
  local name="$1" package="$2" trust="$3"
  local out="$work/$name"
  rm -rf "$out"
  mkdir -p "$out"
  log "stage $name: host runs $(basename "$package") (trust $trust) in an empty sandbox"
  local started
  started="$(date +%s)"
  "${cage[@]}" bwrap \
    --unshare-all --die-with-parent --new-session --clearenv \
    --tmpfs / \
    --ro-bind "$work/bin/sley-host" /sley-host \
    --ro-bind "$work/stage/S" /S \
    --ro-bind "$package" /toolchain.pkg \
    --bind "$out" /out \
    --chdir / \
    /sley-host build --package /toolchain.pkg --trust "$trust" --stage /S \
      --out /out/output.pkg --receipt /out/receipt.json \
    >"$out/host.stdout" 2>"$out/host.stderr"
  log "stage $name finished in $(( $(date +%s) - started ))s: $(tr -d '\n ' <"$out/receipt.json" | head -c 400)"
}

run_stage C2 "$work/stage/C1.pkg" "$c1_digest"
c2_digest="$("$sh2" inspect "$work/C2/output.pkg" | sed -E 's/.*"package_digest": "([0-9a-f]+)".*/\1/')"
log "C2 package digest $c2_digest"
run_stage C3 "$work/C2/output.pkg" "$c2_digest"
c3_digest="$("$sh2" inspect "$work/C3/output.pkg" | sed -E 's/.*"package_digest": "([0-9a-f]+)".*/\1/')"
log "C3 package digest $c3_digest"

# 4. Fixed point.
"$sh2" compare "$work/C2/output.pkg" "$work/C3/output.pkg" | tee "$work/fixed-point-c2-c3.json"
"$sh2" compare "$work/stage/C1.pkg" "$work/C2/output.pkg" | tee "$work/parity-c1-c2.json"
log "FIXED POINT: C2 == C3 and C1 == C2 (package $c2_digest)"
