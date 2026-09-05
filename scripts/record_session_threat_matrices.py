#!/usr/bin/env python3
"""Record the S20-330 T15/T47/T56 threat matrices as evidence.

Runs the focused native tests that prove each matrix and writes one
`matrix.json` per threat under `evidence/security/T{15,47,56}/` with the
commit, the test set, the asserted behaviors, and the result. Fails
closed: any test failure fails the recording with no matrix written.
"""

from __future__ import annotations

import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / "evidence/security"

MATRICES = (
    {
        "threat": "T15",
        "name": "handle reuse across roots",
        "code": "SESSION_STALE_HANDLE",
        "tests": [
            "handles_name_their_expected_root",
            "sessions_bind_workspace_root_and_epoch_and_handles_name_their_root",
        ],
        "assertions": [
            "a handle expands under the session and root it names",
            "naming a root the session never bound is stale immediately",
            "a handle is stale after the head advances",
            "unknown past the bound root inventory",
            "a pre-renewal handle naming the old root stays stale after renewal",
            "only a handle naming the new root resolves after renewal",
            "the closed head-bound set refuses branch reads under a stale session",
        ],
    },
    {
        "threat": "T47",
        "name": "cross-workspace leakage",
        "code": "SESSION_WORKSPACE_MISMATCH",
        "tests": [
            "checks_follow_contract_order",
            "sessions_bind_workspace_root_and_epoch_and_handles_name_their_root",
            "binding_failures_precede_budget_exhaustion",
        ],
        "assertions": [
            "another workspace is refused before epoch, budget, and root checks",
            "the epoch refusal holds at the authority level",
            "a foreign workspace refuses renewal and transaction reads alike",
            "an exhausted budget answers the workspace failure, not the budget failure",
        ],
    },
    {
        "threat": "T56",
        "name": "live session name used by a non-opening caller",
        "code": "SESSION_UNKNOWN for foreign-instance names",
        "tests": [
            "issuance_binds_head_and_nonce_separates_instances",
            "session_repository_and_transaction_methods_answer_deterministically",
            "sessions_bind_workspace_root_and_epoch_and_handles_name_their_root",
            "live_sessions_are_capped_and_restarts_forget",
        ],
        "assertions": [
            "twin instances over equal state issue different identities",
            "a live name of one instance is unknown to the twin",
            "a restarted instance answers SESSION_UNKNOWN to every pre-restart name",
            "live sessions are capped at the negotiated max_sessions",
        ],
    },
)


def commit() -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True, stdout=subprocess.PIPE, check=True
    )
    return completed.stdout.strip()


def run_tests(filters: list[str]) -> None:
    completed = subprocess.run(
        ["cargo", "test", "-p", "sley-protocol", "--lib", "--", *filters],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    failures = [
        line
        for line in completed.stdout.splitlines()
        if line.startswith("test result: FAILED") or "FAILED" in line and line.startswith("test ")
    ]
    if completed.returncode != 0 or failures:
        print(completed.stdout[-4000:])
        raise SystemExit(f"threat matrix tests failed: {filters}")


def main() -> int:
    head = commit()
    for matrix in MATRICES:
        run_tests(matrix["tests"])
        record = {
            "contract": "s20-330-session-handle-profile-v1",
            "threat": matrix["threat"],
            "threat_name": matrix["name"],
            "expected_failure": matrix["code"],
            "tests": matrix["tests"],
            "assertions": matrix["assertions"],
            "commit": head,
            "result": "PASS",
        }
        directory = EVIDENCE / matrix["threat"]
        directory.mkdir(parents=True, exist_ok=True)
        (directory / "matrix.json").write_text(
            json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(json.dumps({"threat": matrix["threat"], "result": "PASS", "tests": len(matrix["tests"])}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
