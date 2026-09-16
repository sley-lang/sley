#!/usr/bin/env python3
"""S20-710 full: an unsigned in-toto provenance statement for the candidate.

Derives `evidence/release/provenance.json` from the S20-720 candidate
evidence, the T52 inventory digests, both SBOM documents, and the S20-730
reports. Nothing is signed, logged, or published; no timestamp is recorded.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from enum import IntEnum
from pathlib import Path

try:
    import build_reproducibility_report as repro
    import records_closure
except ImportError:  # loaded by path (unit lane) without scripts/ on sys.path
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import build_reproducibility_report as repro
    import records_closure


ROOT = Path(__file__).resolve().parents[1]
CANDIDATE = ROOT / "evidence/runtime/s20-720-release-candidate/evidence.json"
INVENTORY = ROOT / "evidence/security/T52/pre-release-inventory.json"
CYCLONEDX = ROOT / "evidence/release/sbom/cyclonedx-1.6.json"
SPDX = ROOT / "evidence/release/sbom/spdx-2.3.json"
REPRO_REPORT = ROOT / "evidence/release/reproducibility-report.json"
CONFORMANCE_REPORT = ROOT / "evidence/conformance/independent-conformance-report.json"
CARGO_LOCK = ROOT / "Cargo.lock"
UV_LOCK = ROOT / "oracle/scb1/uv.lock"
PROVENANCE = ROOT / "evidence/release/provenance.json"
FILE_CONTRACT = "sley2.release-provenance.v1"
STATEMENT_TYPE = "https://in-toto.io/Statement/v1"
PREDICATE_TYPE = "https://slsa.dev/provenance/v1"
BUILD_TYPE = "urn:sley2:buildtype:release-candidate/v1"
BUILDER_ID = "urn:sley2:builder:local-primary"
HEX_64 = re.compile(r"^[0-9a-f]{64}$")
HEX_40 = re.compile(r"^[0-9a-f]{40}$")
REMAPS = (
    "--remap-path-prefix <worktree>=/sley2",
    "--remap-path-prefix <cargo-registry-src>=/cargo/registry/src",
    "--remap-path-prefix <home>=/home-remapped",
)
BLOCKERS = (
    "signing_key_and_transparency_log_unauthorized",
    "final_argus_and_vulcan_dispositions",
    "council_reviews",
)


class ProvenanceErrorCode(IntEnum):
    """S20-710 full provenance failures (contract section 6)."""

    EVIDENCE_MISSING = 74004
    EVIDENCE_INVALID = 74005
    SUBJECT_MISMATCH = 74006
    DOCUMENT_DRIFT = 74007


class ProvenanceError(Exception):
    """One exact S20-710 full failure."""

    def __init__(self, code: ProvenanceErrorCode, detail: str) -> None:
        super().__init__(f"{code.name}: {detail}")
        self.code = code
        self.detail = detail


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def digest_of(value: object) -> str:
    return hashlib.sha256(canonical(value).encode("utf-8")).hexdigest()


def file_digest(path: Path) -> str:
    if not path.exists():
        raise ProvenanceError(
            ProvenanceErrorCode.EVIDENCE_MISSING, f"{path.relative_to(ROOT)} does not exist"
        )
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_candidate() -> dict:
    if not CANDIDATE.exists():
        raise ProvenanceError(
            ProvenanceErrorCode.EVIDENCE_MISSING,
            "the S20-720 candidate evidence record does not exist; run make release-candidate-smoke",
        )
    try:
        candidate = json.loads(CANDIDATE.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ProvenanceError(ProvenanceErrorCode.EVIDENCE_INVALID, str(error)) from error
    if not isinstance(candidate, dict):
        raise ProvenanceError(ProvenanceErrorCode.EVIDENCE_INVALID, "evidence is not an object")
    required = (
        "artifact_name",
        "artifact_sha256",
        "artifact_size_bytes",
        "commit",
        "manifest_digest",
        "member_count",
        "toolchain",
        "reproducibility",
        "result",
    )
    missing = [key for key in required if key not in candidate]
    if missing:
        raise ProvenanceError(ProvenanceErrorCode.EVIDENCE_INVALID, f"missing keys {missing}")
    if candidate["result"] != "PASS":
        raise ProvenanceError(ProvenanceErrorCode.EVIDENCE_INVALID, "evidence result is not PASS")
    reproducibility = candidate["reproducibility"]
    if not isinstance(reproducibility, dict) or reproducibility.get("result") != "REPRODUCIBLE":
        raise ProvenanceError(
            ProvenanceErrorCode.EVIDENCE_INVALID, "the two builds were not REPRODUCIBLE"
        )
    if not HEX_40.match(str(candidate["commit"])):
        raise ProvenanceError(ProvenanceErrorCode.EVIDENCE_INVALID, "commit is not 40 hex")
    if not HEX_64.match(str(candidate["artifact_sha256"])):
        raise ProvenanceError(
            ProvenanceErrorCode.EVIDENCE_INVALID, "artifact digest is not 64 hex"
        )
    toolchain = candidate["toolchain"]
    if not isinstance(toolchain, dict) or set(toolchain) != {"cargo", "rustc"}:
        raise ProvenanceError(
            ProvenanceErrorCode.EVIDENCE_INVALID, "toolchain must name cargo and rustc"
        )
    return candidate


def sbom_root_digest() -> str:
    """The artifact digest the CycloneDX root component records."""
    if not CYCLONEDX.exists():
        raise ProvenanceError(
            ProvenanceErrorCode.EVIDENCE_MISSING,
            "the CycloneDX document does not exist; run python3 scripts/build_standards_sbom.py",
        )
    try:
        bom = json.loads(CYCLONEDX.read_text(encoding="utf-8"))
        hashes = bom["metadata"]["component"]["hashes"]
        return next(entry["content"] for entry in hashes if entry["alg"] == "SHA-256")
    except (OSError, json.JSONDecodeError, KeyError, StopIteration) as error:
        raise ProvenanceError(
            ProvenanceErrorCode.EVIDENCE_INVALID, f"the CycloneDX root component is unusable: {error}"
        ) from error


def git_head() -> str:
    """The current commit, or refuse: deriving a statement while the tree's
    commit is unknown would mix live-tree inputs with an unbound subject."""
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, check=False, capture_output=True, text=True
    )
    if completed.returncode != 0:
        raise ProvenanceError(ProvenanceErrorCode.EVIDENCE_INVALID, "git HEAD is unavailable")
    return completed.stdout.strip()


def is_admittable(attestation: object) -> bool:
    """Whether one attestation may serve as subject authority.

    The predicate is owned by the S20-730 builder
    (`build_reproducibility_report.admissible_attestation`, contract
    revision 7): an attestation admits exactly when it passes the
    attestation contract shape (clean, REPRODUCIBLE, no differing members,
    non-empty toolchain). Kept as a name so the unit lane tests the filter
    at this call site; the rule itself is not restated here.
    """
    return repro.admissible_attestation(attestation)


def attested_candidates() -> list[dict]:
    """Tracked reproducibility attestations that may serve as the subject
    authority: REPRODUCIBLE builds from a clean tree only."""
    if not REPRO_REPORT.exists():
        raise ProvenanceError(
            ProvenanceErrorCode.EVIDENCE_MISSING,
            "evidence/release/reproducibility-report.json does not exist; "
            "run make release-candidate-smoke",
        )
    try:
        report = json.loads(REPRO_REPORT.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ProvenanceError(ProvenanceErrorCode.EVIDENCE_INVALID, str(error)) from error
    attestations = report.get("attestations")
    if not isinstance(attestations, list):
        raise ProvenanceError(ProvenanceErrorCode.EVIDENCE_INVALID, "report has no attestation list")
    return [attestation for attestation in attestations if is_admittable(attestation)]


def build_statement() -> dict:
    candidate = load_candidate()
    # The subject must be the tree under derivation: inputs below are read
    # from the live tree, so a candidate from another commit would misbind
    # the statement (every input digest would describe the wrong tree).
    # Contract revision 5 admits one narrow exception: a HEAD that is a
    # provable records-closure of the candidate commit (records-only
    # advancement, bound artifacts invariant) derives the identical
    # candidate-bound statement with nothing re-minted; the closure HEAD is
    # recorded separately, never inside the statement.
    if candidate["commit"] != git_head():
        status = records_closure.closure_status(str(candidate["commit"]))
        if not status.is_closure:
            raise ProvenanceError(
                ProvenanceErrorCode.EVIDENCE_INVALID,
                f"candidate commit {candidate['commit']} is not HEAD; "
                f"{status.reason}; rebuild the candidate on this tree "
                "before deriving provenance",
            )
    # The tracked reproducibility attestation is the subject authority: the
    # derivation refuses a candidate no clean REPRODUCIBLE attestation
    # names, instead of emitting a statement the checker must catch. The
    # binding is the full 4-tuple (contract section 5), mirroring the SBOM
    # side: commit, artifact digest, manifest digest, and size.
    if not any(
        repro.binds_candidate(attestation, candidate) for attestation in attested_candidates()
    ):
        raise ProvenanceError(
            ProvenanceErrorCode.SUBJECT_MISMATCH,
            "no clean REPRODUCIBLE attestation names this candidate; "
            "re-mint the reproducibility attestation first",
        )
    root_digest = sbom_root_digest()
    if root_digest != candidate["artifact_sha256"]:
        raise ProvenanceError(
            ProvenanceErrorCode.SUBJECT_MISMATCH,
            "the CycloneDX root component digest differs from the candidate evidence digest; "
            "rebuild the SBOM documents",
        )
    clean = bool(candidate.get("working_tree_clean"))
    # A dirty tree cannot produce a statement: the subject would bind a
    # commit to an artifact built from a modified tree, which the release
    # packaging contract forbids. Enforcement lives at derivation, not
    # downstream in the checker (S20-710 re-review round: the builder
    # emitted what only the checker refused).
    if not clean:
        raise ProvenanceError(
            ProvenanceErrorCode.EVIDENCE_INVALID,
            "candidate working tree is not clean; rebuild the candidate "
            "from a clean tree before deriving provenance",
        )
    # The invocation is derivation input, not decoration: a candidate
    # without one predates invocation recording and cannot produce a
    # statement (contract section 4; 74005 PROVENANCE_EVIDENCE_INVALID).
    if "invocation" not in candidate:
        raise ProvenanceError(
            ProvenanceErrorCode.EVIDENCE_INVALID,
            "candidate evidence carries no recorded invocation; "
            "rebuild the candidate before deriving provenance",
        )
    # make_target is derived from the recorded invocation, never inferred
    # from cleanliness: the Makefile build rendering (900 seconds,
    # require-clean, no-keep) names the build target; anything else,
    # including the --keep rendering the Makefile never produces, is a
    # direct script invocation. The label therefore means
    # build-equivalent invocation; it does not claim the verify target ran.
    invocation = candidate["invocation"]
    make_target = (
        "release-candidate-build"
        if invocation
        == "build_release_candidate.py --timeout-seconds=900 --require-clean --no-keep"
        else "build_release_candidate.py direct"
    )
    return {
        "_type": STATEMENT_TYPE,
        "subject": [
            {
                "name": candidate["artifact_name"],
                "digest": {"sha256": candidate["artifact_sha256"]},
            }
        ],
        "predicateType": PREDICATE_TYPE,
        "predicate": {
            "buildDefinition": {
                "buildType": BUILD_TYPE,
                "externalParameters": {
                    "commit": candidate["commit"],
                    "artifact_name": candidate["artifact_name"],
                    "make_target": make_target,
                    "invocation": invocation,
                    "working_tree_clean": clean,
                },
                "internalParameters": {
                    "cargo": candidate["toolchain"]["cargo"],
                    "rustc": candidate["toolchain"]["rustc"],
                    "profile": "release",
                    "locked": True,
                    "path_remaps": list(REMAPS),
                    "artifact_size_bytes": candidate["artifact_size_bytes"],
                    "member_count": candidate["member_count"],
                },
                "resolvedDependencies": [
                    {"uri": "urn:sley2:git-commit", "digest": {"sha1": candidate["commit"]}},
                    {"name": "Cargo.lock", "digest": {"sha256": file_digest(CARGO_LOCK)}},
                    {"name": "oracle/scb1/uv.lock", "digest": {"sha256": file_digest(UV_LOCK)}},
                    {
                        "name": "evidence/security/T52/pre-release-inventory.json",
                        "digest": {"sha256": file_digest(INVENTORY)},
                    },
                    {
                        "name": "evidence/release/sbom/cyclonedx-1.6.json",
                        "digest": {"sha256": file_digest(CYCLONEDX)},
                    },
                    {
                        "name": "evidence/release/sbom/spdx-2.3.json",
                        "digest": {"sha256": file_digest(SPDX)},
                    },
                ],
            },
            "runDetails": {
                "builder": {"id": BUILDER_ID},
                "metadata": {"invocationId": candidate["manifest_digest"]},
                "byproducts": [
                    {
                        "name": "evidence/release/reproducibility-report.json",
                        "digest": {"sha256": file_digest(REPRO_REPORT)},
                    },
                    {
                        "name": "evidence/conformance/independent-conformance-report.json",
                        "digest": {"sha256": file_digest(CONFORMANCE_REPORT)},
                    },
                ],
            },
        },
    }


def candidate_evidence_mismatch() -> bool:
    """Whether a local candidate build replaced the evidence the documents describe.

    `evidence/runtime/` is not tracked, so a fresh candidate build legitimately
    leaves the tracked statement describing the previous candidate until
    `make release-candidate-smoke` reconciles them (contract section 5). The
    short-circuit fires only when the evidence loads and disagrees with the
    tracked statement: missing or unreadable evidence is missing input, and
    `--check` must fail with the input code instead of passing open.
    """
    if not PROVENANCE.exists():
        return False
    try:
        tracked = json.loads(PROVENANCE.read_text(encoding="utf-8"))["statement"]
        recorded = (
            tracked["predicate"]["buildDefinition"]["externalParameters"]["commit"],
            tracked["subject"][0]["digest"]["sha256"],
        )
    except (OSError, json.JSONDecodeError, KeyError, IndexError):
        return False
    try:
        candidate = load_candidate()
    except ProvenanceError:
        return False
    return recorded != (candidate["commit"], candidate["artifact_sha256"])


def validate_tracked() -> list[str]:
    """Internal consistency of the tracked statement, checked even when the
    untracked candidate evidence disagrees with the tracked documents
    (a skipped verification must not read PASS over an unchecked document)."""
    problems: list[str] = []
    try:
        document = json.loads(PROVENANCE.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"unreadable: {error}"]
    if document.get("contract") != FILE_CONTRACT:
        problems.append("contract")
    statement = document.get("statement", {})
    try:
        subjects = statement["subject"]
        predicate = statement["predicate"]
        external = predicate["buildDefinition"]["externalParameters"]
        subjects[0]["digest"]["sha256"]
        external["commit"]
        external["artifact_name"]
        external["make_target"]
        external["working_tree_clean"]
        predicate["buildDefinition"]["internalParameters"]["artifact_size_bytes"]
        predicate["runDetails"]["metadata"]["invocationId"]
    except (KeyError, IndexError, TypeError):
        problems.append("statement-shape")
    else:
        if document.get("statement_digest") != digest_of(statement):
            problems.append("statement-digest")
            return problems
        # The attestation binding of the provenance subject (contract
        # section 5, mirroring the SBOM namespace binding): a tracked
        # statement whose subject no clean REPRODUCIBLE attestation names
        # is not validated, even with a self-consistent digest. The
        # binding is the full 4-tuple (commit, digest, manifest, size),
        # symmetric with build_statement().
        try:
            report = json.loads(REPRO_REPORT.read_text(encoding="utf-8"))
            attested = {
                (
                    attestation.get("commit"),
                    attestation.get("artifact_sha256"),
                    attestation.get("manifest_digest"),
                    attestation.get("artifact_size_bytes"),
                )
                for attestation in repro.admissible_attestations(report)
            }
        except (OSError, json.JSONDecodeError):
            attested = set()
        statement_size = statement["predicate"]["buildDefinition"]["internalParameters"][
            "artifact_size_bytes"
        ]
        statement_manifest = statement["predicate"]["runDetails"]["metadata"]["invocationId"]
        if (
            external["commit"],
            subjects[0]["digest"]["sha256"],
            statement_manifest,
            statement_size,
        ) not in attested:
            problems.append("subject-not-attestation-bound")
    return problems


def blockers_for_candidate(report: dict, candidate: dict) -> list[str]:
    """Release decisions remain held; host coverage is a candidate-bound fact."""
    hosts = {
        attestation["host_label"]
        for attestation in repro.admissible_attestations(report)
        if repro.binds_candidate(attestation, candidate)
    }
    blockers = list(BLOCKERS)
    if len(hosts) < repro.REQUIRED_HOSTS:
        blockers.append("second_host_attestation_operator_lane")
    return blockers


def build_file() -> dict:
    statement = build_statement()
    return {
        "contract": FILE_CONTRACT,
        "statement": statement,
        "attestation": {
            "signed": False,
            "signature_algorithm": None,
            "transparency_log": None,
            "publication_authorized": False,
            "blockers": blockers_for_candidate(
                json.loads(REPRO_REPORT.read_text(encoding="utf-8")), load_candidate()
            ),
        },
        "statement_digest": digest_of(statement),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        if args.check and candidate_evidence_mismatch():
            invalid = validate_tracked()
            if invalid:
                print(
                    canonical(
                        {
                            "mode": "check",
                            "result": "FAIL",
                            "state": "MISMATCH_TRACKED_INVALID",
                            "problems": invalid,
                        }
                    ),
                    end="",
                )
                return 1
            print(
                canonical(
                    {
                        "mode": "check",
                        "result": "MISMATCH_TRACKED_VALIDATED",
                        "state": "CANDIDATE_EVIDENCE_MISMATCH_TRACKED_DOCUMENTS",
                        "detail": "the untracked candidate evidence disagrees with the tracked documents; "
                        "make release-candidate-smoke reconciles them",
                    }
                ),
                end="",
            )
            return 0
        document = build_file()
        text = canonical(document)
        summary = {
            "subject": document["statement"]["subject"][0]["name"],
            "statement_digest": document["statement_digest"],
            "signed": False,
        }
        if args.check:
            current = PROVENANCE.read_text(encoding="utf-8") if PROVENANCE.exists() else None
            if current != text:
                print(
                    canonical(
                        {
                            "mode": "check",
                            "result": "FAIL",
                            "code": int(ProvenanceErrorCode.DOCUMENT_DRIFT),
                            "name": ProvenanceErrorCode.DOCUMENT_DRIFT.name,
                            "detail": "the tracked provenance differs from the derived statement",
                        }
                    ),
                    end="",
                )
                return 1
            print(canonical({"mode": "check", "result": "PASS", **summary}), end="")
            return 0
        PROVENANCE.parent.mkdir(parents=True, exist_ok=True)
        PROVENANCE.write_text(text, encoding="utf-8")
        print(
            canonical(
                {
                    "mode": "write",
                    "result": "PASS",
                    "output": str(PROVENANCE.relative_to(ROOT)),
                    **summary,
                }
            ),
            end="",
        )
        return 0
    except ProvenanceError as error:
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
