#!/usr/bin/env bash
# SH2 changed-input and failure-preservation proofs (REWEAVE §13.4, §15),
# run after scripts/sh2_bootstrap.sh in the same WORKDIR.
#
# 1. A typed mutation changes S: the toolchain constant for the
#    SCB_MAGIC_INVALID diagnostic (10000) becomes 10900 (one
#    ReplaceEntityVersion candidate applied through sley-mutate).
# 2. The prior Sley toolchain C2 rebuilds the changed S in the empty sandbox,
#    seed absent: C2'. C2' must equal the native C0 build of the changed S,
#    differ from C2, and be its own fixed point (C2' rebuilds itself to C2').
# 3. Changed behavior: on a build request whose first object has a corrupt
#    magic, C2 refuses with 10000 and C2' with 10900.
# 4. Failure preservation: an omitted object refuses (29002) with no output.
# 5. A fake builder that ignores its input and returns the prior image C2 is
#    rejected: its "changed" toolchain does not change.
#
# Usage: scripts/sh2_changed_input.sh WORKDIR
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
work="$(cd "${1:?usage: sh2_changed_input.sh WORKDIR}" && pwd)"
target="$repo/target-bootstrap"
cage=(systemd-run --user --scope -p MemoryMax=6G --quiet)
sh2="$target/release/sley-sh2"
host="$work/bin/sley-host"
log() { printf '[sh2-change] %s\n' "$*" | tee -a "$work/changed-input.log"; }

CARGO_TARGET_DIR="$target" "${cage[@]}" cargo build --release -j2 -p sley-selfhost --bin sley-sh2 >>"$work/changed-input.log" 2>&1

digest() { "$sh2" inspect "$1" | sed -E 's/.*"package_digest": "([0-9a-f]+)".*/\1/'; }

# run_host NAME PACKAGE TRUST STAGE -> status in $work/NAME/status
run_host() {
  local name="$1" package="$2" trust="$3" stage="$4"
  local out="$work/$name"
  rm -rf "$out"
  mkdir -p "$out"
  set +e
  "${cage[@]}" bwrap \
    --unshare-all --die-with-parent --new-session --clearenv \
    --ro-bind "$host" /sley-host \
    --ro-bind "$stage" /S \
    --ro-bind "$package" /toolchain.pkg \
    --bind "$out" /out \
    --chdir / \
    /sley-host build --package /toolchain.pkg --trust "$trust" --stage /S \
      --out /out/output.pkg --receipt /out/receipt.json \
    >"$out/host.stdout" 2>"$out/host.stderr"
  local code=$?
  set -e
  local status
  status="$(sed -nE 's/.*"status": "([^"]+)".*/\1/p' "$out/receipt.json" 2>/dev/null || true)"
  echo "${status:-host-exit-$code}" >"$out/status"
  log "$name: $(cat "$out/status") (host exit $code)"
}

c2="$work/C2/output.pkg"
c2_digest="$(digest "$c2")"

# 1. Typed mutation of S.
rm -rf "$work/mut"
"$sh2" mutate-constant --stage "$work/stage/S" --out "$work/mut" --from 10000 --to 10900 | tee "$work/mutation.json"

# 2. C2' from the prior Sley toolchain, seed absent.
run_host CI2 "$c2" "$c2_digest" "$work/mut/S"
[ "$(cat "$work/CI2/status")" = ok ] || { log "FAIL: C2 could not rebuild the changed S"; exit 1; }
ci2_digest="$(digest "$work/CI2/output.pkg")"
log "C2' package digest $ci2_digest"
if cmp -s "$c2" "$work/CI2/output.pkg"; then log "FAIL: C2' equals C2"; exit 1; fi
"$sh2" c0 --stage "$work/mut/S" --out "$work/mut/c0.pkg" >/dev/null
"$sh2" compare "$work/mut/c0.pkg" "$work/CI2/output.pkg" | tee "$work/parity-c0-ci2.json"
run_host CI3 "$work/CI2/output.pkg" "$ci2_digest" "$work/mut/S"
"$sh2" compare "$work/CI2/output.pkg" "$work/CI3/output.pkg" | tee "$work/fixed-point-ci2-ci3.json"

# 3. Changed behavior on a corrupt build request.
rm -rf "$work/bad"
"$sh2" corrupt --stage "$work/stage/S" --out "$work/bad/S"
run_host BAD-C2 "$c2" "$c2_digest" "$work/bad/S"
run_host BAD-CI2 "$work/CI2/output.pkg" "$ci2_digest" "$work/bad/S"
[ "$(cat "$work/BAD-C2/status")" = toolchain-error-10000 ] || { log "FAIL: C2 did not refuse with 10000"; exit 1; }
[ "$(cat "$work/BAD-CI2/status")" = toolchain-error-10900 ] || { log "FAIL: C2' did not refuse with 10900"; exit 1; }
[ ! -e "$work/BAD-C2/output.pkg" ] && [ ! -e "$work/BAD-CI2/output.pkg" ] || { log "FAIL: refused builds left output"; exit 1; }
log "changed behavior: C2 refuses the corrupt request with 10000, C2' with 10900; no output"

# 4. Omitted input.
rm -rf "$work/omit"
"$sh2" omit --stage "$work/stage/S" --out "$work/omit/S"
run_host OMIT-C2 "$c2" "$c2_digest" "$work/omit/S"
[ "$(cat "$work/OMIT-C2/status")" = toolchain-error-29002 ] || { log "FAIL: omitted object not refused"; exit 1; }
[ ! -e "$work/OMIT-C2/output.pkg" ] || { log "FAIL: omitted build left output"; exit 1; }

# 5. A fake builder returning the prior image is rejected.
rm -rf "$work/fake"
"$sh2" fake --prior "$c2" --out "$work/fake" | tee "$work/fake.json"
run_host FAKE "$work/fake/fake.pkg" "$(cat "$work/fake/fake.digest")" "$work/mut/S"
if cmp -s "$c2" "$work/FAKE/output.pkg"; then
  log "fake builder returned the prior image (as constructed); acceptance requires a changed toolchain"
else
  log "FAIL: fake builder did not behave as constructed"; exit 1
fi
fake_out_digest="$(digest "$work/FAKE/output.pkg")"
run_host FAKE-BAD "$work/FAKE/output.pkg" "$fake_out_digest" "$work/bad/S"
if [ "$(cat "$work/FAKE-BAD/status")" = toolchain-error-10900 ]; then
  log "FAIL: the fake builder's output passed the changed-behavior check"; exit 1
fi
log "fake builder REJECTED: its output refuses with $(cat "$work/FAKE-BAD/status"), not 10900"
log "CHANGED INPUT PROOF: PASS"
