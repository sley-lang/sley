#!/usr/bin/env python3
"""S20-730 reproducibility report: host attestations of the S20-720 candidate.

Derives this host's attestation from the S20-720 evidence record, merges it
with attestations from other hosts, carries previously merged attestations
no fresh input supersedes, and writes
`evidence/release/reproducibility-report.json`. Nothing here builds, claims
GA, or opens `release-check`.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from enum import IntEnum
from pathlib import Path


sys.path.insert(0, str(Path(__file__).resolve().parent))
from build_release_candidate import ARTIFACT_NAME

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / "evidence/runtime/s20-720-release-candidate/evidence.json"
REPORT = ROOT / "evidence/release/reproducibility-report.json"
ATTESTATION_CONTRACT = "sley2.reproducibility-attestation.v1"
REPORT_CONTRACT = "sley2.reproducibility-report.v1"
REQUIRED_HOSTS = 2
HEX_64 = re.compile(r"^[0-9a-f]{64}$")
HEX_40 = re.compile(r"^[0-9a-f]{40}$")
# The wave-time blocker list (contract section 2, revision 7). It is a
# constant on purpose: the report is digest-bound and re-minted only by the
# two-host release smoke, so deriving it from live package statuses would
# make a records-only status change read as report drift. The list records
# which packages were open when the candidate was minted; it is not a live
# gate, and the live gates (`release-check`, `v2`, the package checkers)
# stay authoritative. Only `second_host_attestation_operator_lane` is
# derived, dropping exactly when the merge reaches `required_hosts`.
BLOCKERS = [
    "second_host_attestation_operator_lane",
    "standards_sbom_and_provenance_s20_710_full",
    "succession_thresholds_s20_640",
    "council_reviews",
]


class ReproErrorCode(IntEnum):
    """S20-730 reproducibility failures (contract section 7)."""

    EVIDENCE_MISSING = 73000
    EVIDENCE_INVALID = 73001
    ATTESTATION_INVALID = 73002
    ATTESTATION_CONFLICT = 73003


class ReproError(Exception):
    """One exact S20-730 failure."""

    def __init__(self, code: ReproErrorCode, detail: str) -> None:
        super().__init__(f"{code.name}: {detail}")
        self.code = code
        self.detail = detail


def display(path: Path) -> str:
    """The repository-relative path when the path is inside the tree."""
    return str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path)


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def digest_of(value: object) -> str:
    return hashlib.sha256(canonical(value).encode("utf-8")).hexdigest()


def local_attestation(host_label: str, evidence_path: Path = EVIDENCE) -> dict:
    """Derives this host's attestation from the S20-720 evidence record."""
    if not evidence_path.exists():
        raise ReproError(
            ReproErrorCode.EVIDENCE_MISSING,
            f"{display(evidence_path)} does not exist; run make release-candidate-smoke",
        )
    try:
        evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ReproError(ReproErrorCode.EVIDENCE_INVALID, str(error)) from error
    if not isinstance(evidence, dict):
        raise ReproError(ReproErrorCode.EVIDENCE_INVALID, "evidence is not an object")
    required = (
        "artifact_name",
        "artifact_sha256",
        "artifact_size_bytes",
        "commit",
        "manifest_digest",
        "member_count",
        "toolchain",
        "reproducibility",
        "working_tree_clean",
        "result",
    )
    missing = [key for key in required if key not in evidence]
    if missing:
        raise ReproError(ReproErrorCode.EVIDENCE_INVALID, f"missing keys {missing}")
    if evidence["result"] != "PASS":
        raise ReproError(ReproErrorCode.EVIDENCE_INVALID, "evidence result is not PASS")
    reproducibility = evidence["reproducibility"]
    if not isinstance(reproducibility, dict) or reproducibility.get("result") != "REPRODUCIBLE":
        raise ReproError(ReproErrorCode.EVIDENCE_INVALID, "the two builds were not REPRODUCIBLE")
    toolchain = evidence["toolchain"]
    if not isinstance(toolchain, dict) or set(toolchain) != {"cargo", "rustc"}:
        raise ReproError(ReproErrorCode.EVIDENCE_INVALID, "toolchain must name cargo and rustc")
    return validate_attestation(
        {
            "contract": ATTESTATION_CONTRACT,
            "host_label": host_label,
            "commit": evidence["commit"],
            "artifact_name": evidence["artifact_name"],
            "artifact_sha256": evidence["artifact_sha256"],
            "artifact_size_bytes": evidence["artifact_size_bytes"],
            "manifest_digest": evidence["manifest_digest"],
            "member_count": evidence["member_count"],
            "toolchain": {"cargo": toolchain["cargo"], "rustc": toolchain["rustc"]},
            "reproducibility": "REPRODUCIBLE",
            "working_tree_clean": bool(evidence["working_tree_clean"]),
            "differing_members": list(reproducibility.get("differing_members", [])),
        }
    )


def validate_attestation(value: object) -> dict:
    """Checks the contract section 1 shape and returns the attestation."""
    if not isinstance(value, dict):
        raise ReproError(ReproErrorCode.ATTESTATION_INVALID, "attestation is not an object")
    expected = {
        "contract",
        "host_label",
        "commit",
        "artifact_name",
        "artifact_sha256",
        "artifact_size_bytes",
        "manifest_digest",
        "member_count",
        "toolchain",
        "reproducibility",
        "working_tree_clean",
        "differing_members",
    }
    if set(value) != expected:
        raise ReproError(
            ReproErrorCode.ATTESTATION_INVALID,
            f"attestation keys {sorted(set(value) ^ expected)} differ from the contract",
        )
    if value["artifact_name"] != ARTIFACT_NAME:
        raise ReproError(ReproErrorCode.ATTESTATION_INVALID, "artifact_name differs from S20-720")
    if value["contract"] != ATTESTATION_CONTRACT:
        raise ReproError(ReproErrorCode.ATTESTATION_INVALID, "wrong attestation contract")
    if not isinstance(value["host_label"], str) or not value["host_label"].strip():
        raise ReproError(ReproErrorCode.ATTESTATION_INVALID, "host_label must be a non-empty string")
    if not isinstance(value["commit"], str) or not HEX_40.match(value["commit"]):
        raise ReproError(ReproErrorCode.ATTESTATION_INVALID, "commit must be 40 lowercase hex")
    for key in ("artifact_sha256", "manifest_digest"):
        if not isinstance(value[key], str) or not HEX_64.match(value[key]):
            raise ReproError(ReproErrorCode.ATTESTATION_INVALID, f"{key} must be 64 lowercase hex")
    for key in ("artifact_size_bytes", "member_count"):
        if not isinstance(value[key], int) or isinstance(value[key], bool) or value[key] <= 0:
            raise ReproError(ReproErrorCode.ATTESTATION_INVALID, f"{key} must be a positive integer")
    toolchain = value["toolchain"]
    if (
        not isinstance(toolchain, dict)
        or set(toolchain) != {"cargo", "rustc"}
        or not all(isinstance(item, str) and item for item in toolchain.values())
    ):
        raise ReproError(ReproErrorCode.ATTESTATION_INVALID, "toolchain must name cargo and rustc")
    if value["reproducibility"] != "REPRODUCIBLE":
        raise ReproError(ReproErrorCode.ATTESTATION_INVALID, "only REPRODUCIBLE builds attest")
    if value["working_tree_clean"] is not True:
        raise ReproError(ReproErrorCode.ATTESTATION_INVALID, "the attested tree must be clean")
    if value["differing_members"] != []:
        raise ReproError(ReproErrorCode.ATTESTATION_INVALID, "differing members contradict REPRODUCIBLE")
    return dict(value)


def admissible_attestation(attestation: object) -> bool:
    """Whether one tracked attestation may serve as candidate authority.

    The single owner of the admissibility predicate every consumer of the
    tracked report imports (contract section 1, revision 7): an attestation
    is admissible exactly when it passes the section 1 shape, which already
    requires a REPRODUCIBLE build from a whole-tree clean checkout with no
    differing members and non-empty toolchain strings. Anything else
    (dirty, unreproduced, malformed, a null toolchain, not an object) is
    inadmissible; consumers must not restate a weaker subset by hand.
    """
    try:
        validate_attestation(attestation)
    except ReproError:
        return False
    return True


def admissible_attestations(report: object, commit: str | None = None) -> list[dict]:
    """The admissible attestations of a report, optionally for one commit.

    Returns attestations in report order (sorted by host label). A report
    that is not an object or carries no attestation list yields nothing:
    callers that must refuse on a malformed report check the shape first
    (the checker runs `verify_report`), and an empty result never admits.
    """
    if not isinstance(report, dict):
        return []
    attestations = report.get("attestations")
    if not isinstance(attestations, list):
        return []
    return [
        dict(attestation)
        for attestation in attestations
        if admissible_attestation(attestation)
        and (commit is None or attestation["commit"] == commit)
    ]


def binds_candidate(attestation: dict, candidate: dict) -> bool:
    """Whether an attestation names one candidate evidence record exactly.

    The binding is the full 4-tuple the SBOM, provenance, and packaging
    consumers agree on: commit, artifact digest, manifest digest, and
    artifact size. Missing candidate keys never bind.
    """
    return all(
        key in candidate and attestation.get(key) == candidate[key]
        for key in ("commit", "artifact_sha256", "manifest_digest", "artifact_size_bytes")
    )


def select_attestation(
    report: object, candidate: dict | None = None, commit: str | None = None
) -> dict | None:
    """The one attestation that names the current candidate, or None.

    The single owner of candidate selection: consumers must not pick
    `attestations[0]` (the alphabetically first host label) by hand. Only
    admissible attestations are considered. With a candidate evidence
    record, the attestation must bind its 4-tuple. Without one, the current
    candidate is the attested commit carrying the most agreeing hosts; when
    two commits tie, no single current candidate exists and the selection
    fails closed with None. Among one commit's attestations the first host
    label in sorted order is returned, matching the report's own order.
    """
    pool = admissible_attestations(report, commit)
    if candidate is not None:
        pool = [attestation for attestation in pool if binds_candidate(attestation, candidate)]
    if not pool:
        return None
    hosts: dict[str, int] = {}
    for attestation in pool:
        hosts[attestation["commit"]] = hosts.get(attestation["commit"], 0) + 1
    most = max(hosts.values())
    leading = [commit_id for commit_id, count in hosts.items() if count == most]
    if len(leading) != 1:
        return None
    return min(
        (attestation for attestation in pool if attestation["commit"] == leading[0]),
        key=lambda attestation: attestation["host_label"],
    )


def build_report(attestations: list[dict], superseded: list[dict] | None = None) -> dict:
    """Merges attestations into the contract section 2 report."""
    by_label: dict[str, dict] = {}
    for attestation in attestations:
        checked = validate_attestation(attestation)
        label = checked["host_label"]
        if label in by_label:
            raise ReproError(ReproErrorCode.ATTESTATION_INVALID, f"duplicate host label {label!r}")
        by_label[label] = checked
    commits: dict[str, dict] = {}
    for label in sorted(by_label):
        attestation = by_label[label]
        entry = commits.setdefault(
            attestation["commit"],
            {"artifact_sha256": attestation["artifact_sha256"], "hosts": []},
        )
        if entry["artifact_sha256"] != attestation["artifact_sha256"]:
            raise ReproError(
                ReproErrorCode.ATTESTATION_CONFLICT,
                f"commit {attestation['commit']} has different artifact digests across hosts",
            )
        entry["hosts"].append(label)
    multi_host = any(len(entry["hosts"]) >= REQUIRED_HOSTS for entry in commits.values())
    report = {
        "contract": REPORT_CONTRACT,
        "work_package": "S20-730",
        "required_hosts": REQUIRED_HOSTS,
        "distinct_hosts": len(by_label),
        "attestations": [by_label[label] for label in sorted(by_label)],
        "commits": {commit: commits[commit] for commit in sorted(commits)},
        "result": "MULTI_HOST_REPRODUCIBLE" if multi_host else "SINGLE_HOST_REPRODUCIBLE",
        "second_host": {
            "status": "ATTESTED" if multi_host else "GATED_OPERATOR_LANE",
            "note": (
                "at least two hosts attested the same commit and artifact digest"
                if multi_host
                else "the second host build is an operator-gated lane; only the primary host attested"
            ),
        },
        "superseded_attestations": list(superseded or []),
        "ga_claimed": False,
        "publication_authorized": False,
        "blockers": [
            blocker
            for blocker in BLOCKERS
            if not (multi_host and blocker == "second_host_attestation_operator_lane")
        ],
    }
    report["report_digest"] = digest_of(report)
    return report


def load_attestation(path: Path) -> dict:
    try:
        return validate_attestation(json.loads(path.read_text(encoding="utf-8")))
    except (OSError, json.JSONDecodeError) as error:
        raise ReproError(ReproErrorCode.ATTESTATION_INVALID, f"{path}: {error}") from error


def verify_report(report: object) -> list[str]:
    """Hermetic integrity problems of a tracked report: digest plus shapes.

    Reads only the report itself, so it runs on a clean checkout with no
    local S20-720 evidence: a hand-edited file fails the digest, and a
    malformed attestation fails the section 1 shape. Freshness against the
    filing tree is the checker's job, not this function's.
    """
    if not isinstance(report, dict) or report.get("contract") != REPORT_CONTRACT:
        return ["not a reproducibility report"]
    problems: list[str] = []
    body = {key: value for key, value in report.items() if key != "report_digest"}
    digest = report.get("report_digest")
    if not isinstance(digest, str) or digest != digest_of(body):
        problems.append("report_digest does not recompute from the report body")
    attestations = report.get("attestations")
    if not isinstance(attestations, list) or not attestations:
        problems.append("no attestation list")
    else:
        for attestation in attestations:
            try:
                validate_attestation(attestation)
            except ReproError as error:
                problems.append(f"attestation invalid: {error.detail}")
    # The supersession listing is part of the section 2 shape (revision 10)
    # and the hermetic gate checks it like the attestations (revision 11).
    superseded = report.get("superseded_attestations")
    if not isinstance(superseded, list):
        problems.append("no superseded_attestations list")
    else:
        for entry in superseded:
            problem = superseded_entry_problem(entry)
            if problem:
                problems.append(f"superseded attestation invalid: {problem}")
    return problems


def superseded_entry_problem(entry: object) -> str | None:
    """The section 2 shape of one `superseded_attestations` entry."""
    if not isinstance(entry, dict) or set(entry) != {"host_label", "commit", "artifact_sha256", "reason"}:
        return "entry keys"
    if not isinstance(entry["host_label"], str) or not entry["host_label"]:
        return "host_label"
    if not isinstance(entry["commit"], str) or not re.fullmatch(r"[0-9a-f]{40}", entry["commit"]):
        return "commit"
    if not isinstance(entry["artifact_sha256"], str) or not re.fullmatch(r"[0-9a-f]{64}", entry["artifact_sha256"]):
        return "artifact_sha256"
    if not isinstance(entry["reason"], str) or not entry["reason"]:
        return "reason"
    return None


def carried_attestations(
    report_path: Path,
    skip_labels: set[str],
    current_commit: str | None = None,
    superseded: list[dict] | None = None,
) -> list[dict]:
    """Previously merged attestations no fresh input supersedes.

    A rebuild with no --attest must not silently drop other hosts' recorded
    attestations: every tracked attestation whose label is not re-attested in
    this run is re-validated and carried. The fresh local attestation wins
    its own label, and an explicit --attest file wins its label. A tracked
    file that is not a report, or that carries a malformed attestation,
    fails closed instead of being silently skipped.

    Supersession at a re-mint (contract section 2, revisions 10 and 11): when the
    fresh local attestation names a commit, a tracked attestation of ANOTHER
    commit describes a superseded candidate and is not carried onto the new
    one; it is appended to `superseded` so the report names what the re-mint
    left behind instead of silently dropping it or silently keeping it.
    """
    if not report_path.exists():
        return []
    try:
        tracked = json.loads(report_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ReproError(
            ReproErrorCode.ATTESTATION_INVALID, f"{display(report_path)}: {error}"
        ) from error
    if not isinstance(tracked, dict) or tracked.get("contract") != REPORT_CONTRACT:
        raise ReproError(
            ReproErrorCode.ATTESTATION_INVALID,
            f"{display(report_path)} is not a reproducibility report",
        )
    # The tracked report is carried only when it is integral: a hand-edited
    # file (stale digest, malformed attestation or listing entry) would
    # otherwise be laundered into a fresh, digest-valid report by a plain
    # rebuild (Vulcan P4 at c04539b9).
    integrity = verify_report(tracked)
    if integrity:
        raise ReproError(
            ReproErrorCode.ATTESTATION_INVALID,
            f"{display(report_path)} fails integrity: {'; '.join(integrity)}",
        )
    attestations = tracked.get("attestations")
    if not isinstance(attestations, list):
        raise ReproError(
            ReproErrorCode.ATTESTATION_INVALID,
            f"{display(report_path)} has no attestation list",
        )
    carried: list[dict] = []
    listed: list[dict] = []
    for attestation in attestations:
        checked = validate_attestation(attestation)
        if checked["host_label"] in skip_labels:
            continue
        if current_commit is not None and checked["commit"] != current_commit:
            listed.append(
                {
                    "host_label": checked["host_label"],
                    "commit": checked["commit"],
                    "artifact_sha256": checked["artifact_sha256"],
                    "reason": "attests a commit the re-mint superseded",
                }
            )
            continue
        carried.append(checked)
    if superseded is not None:
        # The tracked listing persists across plain rebuilds (revision 11):
        # an entry is retired only when its host re-attests the current
        # commit (fresh, explicit, or carried), never by a re-run of the
        # build, and never carried back into `attestations`.
        tracked_listing = tracked.get("superseded_attestations", [])
        if not isinstance(tracked_listing, list):
            raise ReproError(
                ReproErrorCode.ATTESTATION_INVALID,
                f"{display(report_path)} has a malformed superseded_attestations list",
            )
        for entry in tracked_listing:
            problem = superseded_entry_problem(entry)
            if problem:
                raise ReproError(
                    ReproErrorCode.ATTESTATION_INVALID,
                    f"{display(report_path)} superseded attestation invalid: {problem}",
                )
            listed.append(dict(entry))
        re_attested = skip_labels | {item["host_label"] for item in carried}
        seen: dict[tuple[str, str], str] = {}
        for entry in sorted(listed, key=lambda item: (item["host_label"], item["commit"])):
            key = (entry["host_label"], entry["commit"])
            if entry["host_label"] in re_attested:
                continue
            if current_commit is not None and entry["commit"] == current_commit:
                # A listing entry naming the current candidate is not a
                # superseded attestation; it is refused, not dropped.
                raise ReproError(
                    ReproErrorCode.ATTESTATION_CONFLICT,
                    f"superseded listing names the current commit {current_commit} for {entry['host_label']!r}",
                )
            if key in seen:
                if seen[key] != entry["artifact_sha256"]:
                    raise ReproError(
                        ReproErrorCode.ATTESTATION_CONFLICT,
                        f"superseded listing carries two artifact digests for {key}",
                    )
                continue
            seen[key] = entry["artifact_sha256"]
            superseded.append(entry)
    return carried


def explicit_of_current_commit(explicit: list[dict], current_commit: str) -> None:
    """An explicit `--attest` file names the commit the fresh local one names.

    The supersession rule listed only carried attestations of another commit;
    an explicit file of another commit used to be merged as-is and produced a
    two-commit report with no selectable candidate, refused only downstream
    by the checker (Vulcan P4 at 92fa6646). The builder is the refusing
    owner: it is `REPRO_ATTESTATION_CONFLICT`, and no report is written.
    """
    for attestation in explicit:
        if attestation["commit"] != current_commit:
            raise ReproError(
                ReproErrorCode.ATTESTATION_CONFLICT,
                f"explicit attestation {attestation['host_label']!r} names commit "
                f"{attestation['commit']}, the fresh local attestation names {current_commit}",
            )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host-label", default="primary")
    parser.add_argument("--attest", action="append", default=[], type=Path)
    parser.add_argument("--emit-attestation", type=Path)
    parser.add_argument("--output", type=Path, default=REPORT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        local = local_attestation(args.host_label)
        if args.emit_attestation is not None:
            args.emit_attestation.parent.mkdir(parents=True, exist_ok=True)
            args.emit_attestation.write_text(canonical(local), encoding="utf-8")
            print(canonical({"result": "PASS", "attestation": str(args.emit_attestation)}), end="")
            return 0
        explicit = [load_attestation(path) for path in args.attest]
        explicit_of_current_commit(explicit, local["commit"])
        skip = {args.host_label} | {attestation["host_label"] for attestation in explicit}
        superseded: list[dict] = []
        carried = carried_attestations(args.output, skip, local["commit"], superseded)
        report = build_report([local, *explicit, *carried], superseded)
        text = canonical(report)
        if args.check:
            current = args.output.read_text(encoding="utf-8") if args.output.exists() else None
            drift = current != text
            print(
                canonical(
                    {
                        "mode": "check",
                        "result": "FAIL" if drift else "PASS",
                        "distinct_hosts": report["distinct_hosts"],
                        "reproducibility": report["result"],
                    }
                ),
                end="",
            )
            return 1 if drift else 0
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding="utf-8")
        print(
            canonical(
                {
                    "mode": "write",
                    "result": "PASS",
                    "distinct_hosts": report["distinct_hosts"],
                    "reproducibility": report["result"],
                    "second_host": report["second_host"]["status"],
                    "output": display(args.output),
                }
            ),
            end="",
        )
        return 0
    except ReproError as error:
        print(
            canonical(
                {
                    "result": "FAIL",
                    "code": int(error.code),
                    "name": error.code.name,
                    "detail": error.detail,
                }
            ),
            end="",
        )
        return 1


if __name__ == "__main__":
    sys.exit(main())
