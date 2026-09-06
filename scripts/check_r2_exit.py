#!/usr/bin/env python3
"""Aggregate REWEAVE R2 exit decision (RW-075 repair of AR-06).

The staged bootstrap-capability checker (`scripts/check_bootstrap_capability.py`)
remains the capability/profile audit result (READY is a profile audit, not
the R2 exit). This script is the separate aggregate R2 exit decision: it
consumes actual package evidence for P / RW-060 / RW-070+RW-075 successor /
independent review state, and nothing else. S/C0/C1/C2/C3 and RW-080+ are
later R3 evidence and MUST NOT block R2 (they are explicitly skipped).

The aggregate R2 gate FAILS when any of these hold:
- RW-060 evidence is missing or mismatched;
- the current ABI successor is not frozen/accepted;
- the execution closure is incomplete;
- a current architecture BLOCKER exists;
- required review does not pass.

The historical BOOTSTRAP_READY TRUE record (RW-070) is preserved but marked
superseded/invalidated for R3 advancement by R2_ARCHITECTURE_FAIL. This
script never relabels the capability checker as the exit gate.
"""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

PROFILE_JSON = ROOT / "conformance/bootstrap-profile/v1/profile.json"
PROFILE_V2_JSON = ROOT / "conformance/bootstrap-profile/v2/profile.json"
HOST_ABI_JSON = ROOT / "conformance/host-abi/v1/host-abi.json"
HOST_ABI_V2_JSON = ROOT / "conformance/host-abi/v2/host-abi.json"
EXEC_JSON = ROOT / "conformance/exec-package/v1/exec-package.json"
EXEC_V2_JSON = ROOT / "conformance/exec-package/v2/exec-package.json"
HASH_JSON = ROOT / "conformance/raw-hash/v1/raw-hash.json"
BOUNDARY = ROOT / "host-boundary.json"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
RW060 = ROOT / "machineresearch/sley-2.0/reweave/rw-060.md"
RW070 = ROOT / "machineresearch/sley-2.0/reweave/rw-070.md"
RW075 = ROOT / "machineresearch/sley-2.0/reweave/rw-075.md"
RW075_CORRECTION = ROOT / "machineresearch/sley-2.0/reweave/rw-075-correction.md"
FAIL_RECORD = ROOT / "machineresearch/sley-2.0/reweave/rw-075-premium-fail.md"
REVIEWS = ROOT / "machineresearch/sley-2.0/reviews"

FROZEN = {
    "profile": "4f2691504b5c756eae1f5ef01e6e998cc4cd628d4b524b038b10d583bfefd630",
    "boundary": "d935d238a4d75d154df128aad630411ca3fdcc18e4db084dcd2317ab73bdb18a",
    "host_abi": "e6de00b820a094ec2abc7a6ae43263d7340c2bf1a38a6231e7426fda0f04ecc2",
    "exec_package": "9e20da24a3b3647d15d052ce759ed9b1ca7682d950421baf48978ad59a5795d4",
    "raw_hash": "785205fb49490237cbec7ffe2fc4c2b0f98014b9aae76cc54795921e5d969f72",
}

# Successor (RW-075 correction, current R2 candidate). V1 above stays as
# preserved history; READY requires the successor below.
FROZEN_V2 = {
    "profile": "fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459",
    "host_abi": "bc564653302a73eb5f998427250a2bb7cd87f5685ef12619bd4ae1f1b2af70d5",
    "exec_package": "f4958c5e3d57762173b881288b008af17d45b5f07a431fcc442d9eec5770da94",
}

RW060_IDS = {
    "genesis_root": "c2e16e8a7688373377023a76f0478da2552fc6de1d041adaa75d939ad45aa83d",
    "committed_root": "dfffde279b358260f3a4c166d5daf63055e84d8a19aa1e482d63e83a8305bc6e",
    "commit_tx": "8cf379c495c8eb28df269bf493a3e41b9961456ebb0b195fa8e73ab88d4bde0c",
    "observation_ok": "b249155034e9965f68795cec5a7d8b32168620a2808da4e6eb80336d3c7d5473",
    "observation_err": "55f9651f448ef45fd1e5b6dd06345611488c525c0c64fd645b82b42b5f2867f0",
    "pack_id": "70f16b5752797cda737ff93dbd61b0376a8d425d70e258a1189b2392cac0e13e",
}

failures: list[str] = []
evidence: dict = {"skipped_r3": ["S", "C0", "C1", "C2", "C3", "RW-080+"]}


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check(name: str, ok: bool, detail: str = "") -> None:
    evidence[name] = {"pass": ok, "detail": detail}
    if not ok:
        failures.append(name)


# P / BOOTSTRAP_PROFILE_1.
try:
    profile_ok = sha(PROFILE_JSON) == FROZEN["profile"]
except FileNotFoundError:
    profile_ok = False
check("P_bootstrap_profile_1", profile_ok, FROZEN["profile"][:12])
try:
    boundary_ok = sha(BOUNDARY) == FROZEN["boundary"]
except FileNotFoundError:
    boundary_ok = False
check("P_host_boundary", boundary_ok, FROZEN["boundary"][:12])

# RW-060 lifecycle identity.
try:
    summary = json.loads(SUMMARY.read_bytes().decode("utf-8"))
except (FileNotFoundError, json.JSONDecodeError):
    summary = {}
rw060 = summary.get("rw060_source_free_lifecycle", {})
rw060_ok = (
    rw060.get("status") == "RW060_COMPLETE"
    and all(rw060.get(key) == value for key, value in RW060_IDS.items())
    and RW060.exists()
)
check("RW060_lifecycle", rw060_ok, rw060.get("status", "missing"))

# RW-070 + RW-075 successor closure (v1 preserved, v2 current).
try:
    abi_ok = sha(HOST_ABI_JSON) == FROZEN["host_abi"]
except FileNotFoundError:
    abi_ok = False
check("RW070_host_abi_frozen", abi_ok, FROZEN["host_abi"][:12])
try:
    exec_ok = sha(EXEC_JSON) == FROZEN["exec_package"]
except FileNotFoundError:
    exec_ok = False
check("RW075_exec_package_frozen", exec_ok, FROZEN["exec_package"][:12])
try:
    hash_ok = sha(HASH_JSON) == FROZEN["raw_hash"]
except FileNotFoundError:
    hash_ok = False
check("RW075_raw_hash_admitted", hash_ok, FROZEN["raw_hash"][:12])
closure_ok = RW070.exists() and RW075.exists() and RW075_CORRECTION.exists()
check("RW070_RW075_records", closure_ok, "rw-070.md+rw-075.md+correction")
# Successor bindings (current R2 candidate).
try:
    profile_v2_ok = sha(PROFILE_V2_JSON) == FROZEN_V2["profile"]
except FileNotFoundError:
    profile_v2_ok = False
check("R2_profile_v2", profile_v2_ok, FROZEN_V2["profile"][:12])
try:
    abi_v2_ok = sha(HOST_ABI_V2_JSON) == FROZEN_V2["host_abi"]
except FileNotFoundError:
    abi_v2_ok = False
check("R2_host_abi_v2", abi_v2_ok, FROZEN_V2["host_abi"][:12])
try:
    exec_v2_ok = sha(EXEC_V2_JSON) == FROZEN_V2["exec_package"]
except FileNotFoundError:
    exec_v2_ok = False
check("R2_exec_package_v2", exec_v2_ok, FROZEN_V2["exec_package"][:12])


def run_checker(script: str) -> bool:
    try:
        result = subprocess.run(
            [sys.executable, str(ROOT / "scripts" / script)],
            capture_output=True,
            text=True,
            timeout=300,
        )
        return result.returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


check("checker_host_abi_v1", run_checker("check_host_abi_v1.py"))
check("checker_host_abi_v2", run_checker("check_host_abi_v2.py"))
check("checker_exec_package_v1", run_checker("check_exec_package_v1.py"))
check("checker_exec_package_v2", run_checker("check_exec_package_v2.py"))
check(
    "checker_exec_package_markers",
    run_checker("check_exec_package_markers.py"),
)
check(
    "checker_bootstrap_profile_2",
    run_checker("check_bootstrap_profile_2.py"),
)

# Independent review state (required: no BLOCKER, review PASS).
fail_preserved = FAIL_RECORD.exists() and "R2_ARCHITECTURE_FAIL" in FAIL_RECORD.read_text(
    encoding="utf-8", errors="replace"
)
check("R2_ARCHITECTURE_FAIL_preserved", fail_preserved, "negative evidence")


def review_verdict(pattern: str) -> str:
    """Exact verdict parse over review transcripts: FAIL takes precedence.

    A review passes only on an exact `VERDICT: PASS` line with no
    `VERDICT: FAIL` line anywhere in the same file. Substring mentions of
    PASS inside a FAIL review (or vice versa) never satisfy the gate.
    Returns "PASS", "FAIL", or "PENDING" (no verdict line found).
    """
    state = "PENDING"
    for path in sorted(REVIEWS.glob(pattern)):
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        if re.search(r"^VERDICT:\s*FAIL", text, re.MULTILINE):
            return "FAIL"
        if re.search(r"^VERDICT:\s*PASS", text, re.MULTILINE):
            state = "PASS"
    return state


def lane_round(path: Path) -> tuple[int, str]:
    """Numeric round identity: `-rN-` suffix parses as N, round 1 has no
    suffix and parses as 1. Ties break on the full filename so selection
    is total and deterministic (`r9` before `r10`, unlike lexicographic
    filename order)."""
    match = re.search(r"-r(\d+)-", path.name)
    round_no = int(match.group(1)) if match else 1
    return (round_no, path.name)


def latest_lane_verdict(prefix: str) -> str:
    """Round-aware lane verdict: evaluates the latest round file only.

    Candidates matching `<prefix>*.log` except `*infra*`
    (infrastructure no-verdict events, never reviews) order by numeric
    round, then filename. Only the latest round file is evaluated, with
    within-file FAIL precedence. Earlier failed rounds stay preserved as
    history but never poison a later passing round; a later FAIL always
    re-blocks.
    """
    candidates = sorted(
        (lane_round(path), path)
        for path in REVIEWS.glob(f"{prefix}*.log")
        if "infra" not in path.name
    )
    if not candidates:
        return "PENDING"
    try:
        text = candidates[-1][1].read_text(encoding="utf-8", errors="replace")
    except OSError:
        return "PENDING"
    if re.search(r"^VERDICT:\s*FAIL", text, re.MULTILINE):
        return "FAIL"
    if re.search(r"^VERDICT:\s*PASS", text, re.MULTILINE):
        return "PASS"
    return "PENDING"


rw070_ariadne = review_verdict("reweave-rw070-ariadne-r5-*.log")
rw070_nabu = (
    review_verdict("reweave-rw070-nabu-r4-*.log")
    if review_verdict("reweave-rw070-nabu-r4-*.log") != "PENDING"
    else review_verdict("reweave-rw070-nabu-r5-*.log")
)
check("RW070_ariadne_pass", rw070_ariadne == "PASS", "round-5 verdict line")
check(
    "RW070_nabu_pass",
    rw070_nabu == "PASS",
    "round-4 verdict line plus round-5 confirmation",
)
rw075_ariadne = latest_lane_verdict("reweave-rw075-ariadne-")
rw075_nabu = latest_lane_verdict("reweave-rw075-nabu-")
check("RW075_ariadne_pass", rw075_ariadne == "PASS", f"latest-round verdict: {rw075_ariadne}")
check("RW075_nabu_pass", rw075_nabu == "PASS", f"latest-round verdict: {rw075_nabu}")


def premium_verdict() -> str:
    """Premium delta verdict: exactly `VERDICT: R2_ARCHITECTURE_PASS`."""
    for path in sorted(REVIEWS.glob("reweave-rw075-premium-*.log")):
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        if re.search(r"^VERDICT:\s*R2_ARCHITECTURE_FAIL", text, re.MULTILINE):
            return "FAIL"
        if re.search(r"^VERDICT:\s*R2_ARCHITECTURE_PASS", text, re.MULTILINE):
            return "PASS"
    return "PENDING"


premium = premium_verdict()
check("premium_delta_R2_ARCHITECTURE_PASS", premium == "PASS", f"verdict: {premium}")

blocker_open = fail_preserved and premium != "PASS"
evidence["architecture_blocker_open"] = blocker_open
if blocker_open:
    failures.append("architecture_blocker_open")

ready = not failures
print(f"R2_EXIT: {'READY' if ready else 'NOT_READY'}")
print(f"historical_BOOTSTRAP_READY: TRUE (RW-070, superseded/invalidated by R2_ARCHITECTURE_FAIL)")
for name, item in evidence.items():
    if isinstance(item, dict) and "pass" in item:
        print(f"  - {name}: {'PASS' if item['pass'] else 'FAIL'} ({item['detail']})")
    elif name == "architecture_blocker_open":
        print(f"  - architecture_blocker_open: {item}")
print("  - S/C0/C1/C2/C3/RW-080+: explicitly not evaluated (later R3 evidence)")
sys.exit(0 if ready else 3)
