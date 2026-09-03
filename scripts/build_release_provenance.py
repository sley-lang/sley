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
import sys
from enum import IntEnum
from pathlib import Path


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
    "root_license_text_operator_approval",
    "signing_key_and_transparency_log_unauthorized",
    "final_argus_and_vulcan_dispositions",
    "release_candidate_history_reanchor",
    "second_host_attestation_operator_lane",
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


def build_statement() -> dict:
    candidate = load_candidate()
    root_digest = sbom_root_digest()
    if root_digest != candidate["artifact_sha256"]:
        raise ProvenanceError(
            ProvenanceErrorCode.SUBJECT_MISMATCH,
            "the CycloneDX root component digest differs from the candidate evidence digest; "
            "rebuild the SBOM documents",
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
                    "make_target": "release-candidate-smoke",
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


def local_build_ahead() -> bool:
    """Whether a local candidate build replaced the evidence the documents describe.

    `evidence/runtime/` is not tracked, so a fresh candidate build legitimately
    leaves the tracked statement describing the previous candidate until
    `make release-candidate-smoke` reconciles them (contract section 5).
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
        return True
    return recorded != (candidate["commit"], candidate["artifact_sha256"])


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
            "blockers": list(BLOCKERS),
        },
        "statement_digest": digest_of(statement),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        if args.check and local_build_ahead():
            print(
                canonical(
                    {
                        "mode": "check",
                        "result": "PASS",
                        "state": "LOCAL_BUILD_AHEAD_OF_TRACKED_DOCUMENTS",
                        "detail": "a local candidate build replaced the untracked evidence this "
                        "statement describes; make release-candidate-smoke reconciles them",
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
