#!/usr/bin/env bash
# Second-host (secondary-host) attestation for the exact selected candidate.
# Follows docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md section 5.1.
#
# Preconditions enforced here, never bypassed:
#   1. The lab lease is NOT held (gf-lab-lease status .gate != "held"). ZJX_BREAKGLASS is never used.
#   2. The lab host is reachable through lab-transport (ssh secondary-host) under the
#      existing coordination rules; the timestamp alone is not availability.
#   3. The candidate commit is exactly the one selected in the tracked report.
# Usage: scripts/second_host_attest.sh [--phase check|build|merge] (default: check)
#   Environment: SLEY2_LAB_HOST (ssh alias, default secondary-host), SLEY2_LAB_HOME
#   (default /home/lab), SLEY2_SECOND_HOST_OUT (default
#   evidence/runtime/second-host, gitignored runtime evidence).
# Tracked since the 76ae15ab round (Nabu P4 carried from c04539b9: the runbook
# lived outside the repository).
set -euo pipefail

REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
LABEL=secondary
LAB_HOST=${SLEY2_LAB_HOST:-secondary-host}
LAB_HOME=${SLEY2_LAB_HOME:-/home/lab}
# The candidate identity is read from the tracked report at run time (never
# hard-coded: the a7852028 pin went stale at the 7426bc0b re-mint, Nabu/Vulcan
# P3 at 92fa6646); the primary tree must be clean and at the candidate's
# records-only descendant so the report is the one committed.
test -z "$(git -C "$REPO" status --porcelain)" || { echo "REFUSED: primary tree not clean" >&2; exit 5; }
read -r CANDIDATE ARTIFACT_SHA < <(python3 - <<PY
import json
r=json.load(open("$REPO/evidence/release/reproducibility-report.json"))
c=r["commits"]; assert len(c)==1, c
(commit, entry), = c.items()
assert entry["hosts"]==["primary"], entry
print(commit, entry["artifact_sha256"])
PY
)
git -C "$REPO" merge-base --is-ancestor "$CANDIDATE" HEAD || { echo "REFUSED: candidate $CANDIDATE is not an ancestor of the primary HEAD" >&2; exit 6; }
LAB_DIR="$LAB_HOME/sley2-attest-${CANDIDATE:0:7}"
OUT=${SLEY2_SECOND_HOST_OUT:-$REPO/evidence/runtime/second-host}
PHASE=${2:-check}; [[ "${1:-}" == "--phase" ]] && PHASE=$2

mkdir -p "$OUT"
echo "== lease gate"
# gf-lab-lease: exit 0 = node free (prints "free"); 10 = lease held (prints gate JSON); 1 = unreachable.
set +e; LEASE=$(gf-lab-lease status 2>&1); RC=$?; set -e
echo "$LEASE"
if [[ $RC -ne 0 || "$LEASE" != free* ]]; then
  echo "REFUSED: lab lease gate rc=$RC (held or unreachable). Do not proceed; never use ZJX_BREAKGLASS." >&2
  exit 3
fi

echo "== candidate binding"
python3 - <<PY
import json
r=json.load(open("$REPO/evidence/release/reproducibility-report.json"))
c=r["commits"]; assert list(c)==["$CANDIDATE"], c
assert c["$CANDIDATE"]["artifact_sha256"]=="$ARTIFACT_SHA", c
s=json.load(open("$REPO/machineresearch/sley-2.0/machine-summary.json"))["release_candidate_packaging"]
assert s["candidate_commit"]=="$CANDIDATE" and s["candidate_artifact_sha256"]=="$ARTIFACT_SHA", (s["candidate_commit"], s["candidate_artifact_sha256"])
print("report and summary bind", "$CANDIDATE"[:7], "artifact", "$ARTIFACT_SHA"[:12], "hosts", c["$CANDIDATE"]["hosts"])
PY
[[ "$PHASE" == "check" ]] && { echo "check OK (lease released, binding verified); rerun with --phase build"; exit 0; }

echo "== lab reachability"
ssh -o BatchMode=yes -o ConnectTimeout=15 "$LAB_HOST" 'hostname; echo SSH_CONNECTION=$SSH_CONNECTION; nproc; df -h /home | tail -1' | tee "$OUT/lab-reach.txt"

if [[ "$PHASE" == "build" ]]; then
  echo "== bundle exact candidate and build on the lab (detached, clean by construction)"
  BUNDLE="$OUT/sley2-$CANDIDATE.bundle"
  # `git bundle create <file> <bare-sha>` is refused as an empty bundle: bundle
  # the candidate through a temporary ref, removed again once the bundle exists
  # (the 8d063f00 lane ran this step by hand for that reason).
  REF="refs/heads/attest/${CANDIDATE:0:7}"
  git -C "$REPO" update-ref "$REF" "$CANDIDATE"
  git -C "$REPO" bundle create "$BUNDLE" "$REF"
  git -C "$REPO" update-ref -d "$REF"
  git -C "$REPO" bundle verify "$BUNDLE" >/dev/null
  scp "$BUNDLE" "$LAB_HOST:$LAB_HOME/"
  ssh "$LAB_HOST" bash -s <<LAB
set -euo pipefail
source ~/.cargo/env 2>/dev/null || true
rm -rf "$LAB_DIR"
git clone --quiet "$LAB_HOME/$(basename "$BUNDLE")" "$LAB_DIR"
cd "$LAB_DIR"
git checkout --quiet --detach $CANDIDATE
test "\$(git rev-parse HEAD)" = "$CANDIDATE"
test -z "\$(git status --porcelain --ignored | grep -v '^!! target')" || { echo "lab tree not clean"; exit 4; }
cargo fetch --locked --quiet
make release-candidate-build
python3 scripts/build_reproducibility_report.py --host-label $LABEL --emit-attestation $LAB_HOME/$LABEL-${CANDIDATE:0:7}-attestation.json
sha256sum evidence/runtime/s20-720-release-candidate/evidence.json $LAB_HOME/$LABEL-${CANDIDATE:0:7}-attestation.json
LAB
  scp "$LAB_HOST:$LAB_HOME/$LABEL-${CANDIDATE:0:7}-attestation.json" "$OUT/"
  ssh "$LAB_HOST" "sha256sum $LAB_DIR/evidence/runtime/s20-720-release-candidate/evidence.json" > "$OUT/lab-evidence.sha256"
  echo "attestation transferred to $OUT; rerun with --phase merge"
  exit 0
fi

if [[ "$PHASE" == "merge" ]]; then
  ATT="$OUT/$LABEL-${CANDIDATE:0:7}-attestation.json"
  test -f "$ATT"
  python3 - "$ATT" <<PY
import json,sys
a=json.load(open(sys.argv[1]))
assert a["commit"]=="$CANDIDATE" and a["artifact_sha256"]=="$ARTIFACT_SHA" and a["reproducibility"]=="REPRODUCIBLE" and a["working_tree_clean"] is True, a
print("attestation binds the selected candidate")
PY
  cd "$REPO"
  test -z "$(git status --porcelain)" || { echo "primary tree not clean"; exit 5; }
  cp "$ATT" "evidence/release/attestations/${CANDIDATE:0:7}-$LABEL.json"
  python3 scripts/build_reproducibility_report.py --attest "$ATT"
  python3 -c 'import json;r=json.load(open("evidence/release/reproducibility-report.json"));print(r["result"], r["distinct_hosts"])'
  echo "NEXT (manual, records-only): file the lane row in evidence/release/second-host-lane-records.json"
  echo "  (date, transport lane from $OUT/lab-reach.txt, lab checkout $LAB_DIR, lab evidence digest $OUT/lab-evidence.sha256,"
  echo "   attestation file digest, bundle commit $CANDIDATE, merge commit = the commit you make now),"
  echo "  then: make evidence-refresh && make release-candidate-verify && make quick, re-pin attested_hosts/reproducibility_result/"
  echo "  second_host_status/blockers in machineresearch/sley-2.0/machine-summary.json, commit as a records-only descendant."
fi
