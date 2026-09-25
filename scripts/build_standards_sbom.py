#!/usr/bin/env python3
"""S20-710 full: CycloneDX 1.6 and SPDX 2.3 documents from the T52 inventory.

Derives `evidence/release/sbom/cyclonedx-1.6.json` and
`evidence/release/sbom/spdx-2.3.json` from the locked, offline dependency
inventory and the local candidate evidence. No network, no signature, no
legal opinion, no timestamp.
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
INVENTORY = ROOT / "evidence/security/T52/pre-release-inventory.json"
REPRO_REPORT = ROOT / "evidence/release/reproducibility-report.json"
CANDIDATE = ROOT / "evidence/runtime/s20-720-release-candidate/evidence.json"
CYCLONEDX = ROOT / "evidence/release/sbom/cyclonedx-1.6.json"
SPDX = ROOT / "evidence/release/sbom/spdx-2.3.json"
INVENTORY_CONTRACT = "s20-710-pre-release-inventory-v1"
TOOL_NAME = "sley2-standards-sbom"
TOOL_VERSION = "1"
CANDIDATE_VERSION = "2.0.1"
ROOT_LICENSE = "Apache-2.0"
SPDX_ID = re.compile(r"[^A-Za-z0-9.\-]")
LICENSE_TOKEN = re.compile(r"[A-Za-z0-9:._+\-]+")
LICENSE_OPERATORS = ("AND", "OR", "WITH")


class SbomErrorCode(IntEnum):
    """S20-710 full SBOM failures (contract section 6)."""

    INVENTORY_MISSING = 74000
    INVENTORY_INVALID = 74001
    COMPONENT_INCOMPLETE = 74002
    DOCUMENT_DRIFT = 74003


class SbomError(Exception):
    """One exact S20-710 full failure."""

    def __init__(self, code: SbomErrorCode, detail: str) -> None:
        super().__init__(f"{code.name}: {detail}")
        self.code = code
        self.detail = detail


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def digest_of(value: object) -> str:
    return hashlib.sha256(canonical(value).encode("utf-8")).hexdigest()


def file_digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_inventory() -> dict:
    if not INVENTORY.exists():
        raise SbomError(
            SbomErrorCode.INVENTORY_MISSING,
            "evidence/security/T52/pre-release-inventory.json does not exist; "
            "run python3 scripts/generate_supply_chain_evidence.py",
        )
    try:
        inventory = json.loads(INVENTORY.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SbomError(SbomErrorCode.INVENTORY_INVALID, str(error)) from error
    if not isinstance(inventory, dict) or inventory.get("contract") != INVENTORY_CONTRACT:
        raise SbomError(SbomErrorCode.INVENTORY_INVALID, "wrong or missing inventory contract")
    packages = inventory.get("packages")
    relationships = inventory.get("relationships")
    if not isinstance(packages, list) or not packages:
        raise SbomError(SbomErrorCode.INVENTORY_INVALID, "the inventory lists no package")
    if not isinstance(relationships, list) or not relationships:
        raise SbomError(SbomErrorCode.INVENTORY_INVALID, "the inventory lists no relationship")
    return inventory


def load_candidate() -> dict:
    """The S20-720 candidate facts the SBOM root component needs."""
    if not CANDIDATE.exists():
        raise SbomError(
            SbomErrorCode.INVENTORY_MISSING,
            "the S20-720 candidate evidence record does not exist; "
            "run make release-candidate-smoke",
        )
    try:
        candidate = json.loads(CANDIDATE.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SbomError(SbomErrorCode.INVENTORY_INVALID, str(error)) from error
    required = ("artifact_name", "artifact_sha256", "artifact_size_bytes", "commit", "manifest_digest")
    missing = [key for key in required if key not in candidate]
    if missing:
        raise SbomError(SbomErrorCode.INVENTORY_INVALID, f"candidate evidence missing {missing}")
    return candidate


def normalize_license(expression: str) -> str:
    """The SPDX expression for one declared license string.

    Cargo documents `/` as an OR-equivalent dual-license separator
    (`MIT/Apache-2.0` means MIT or Apache-2.0), but `/` is not SPDX
    expression syntax, so emitting it verbatim produces documents that fail
    strict validation. A `/`-joined declaration normalizes to an `OR`
    chain with that exact meaning; anything else passes through unchanged
    for the grammar check below, including `LicenseRef-` identifiers.
    """
    if "/" not in expression:
        return " ".join(expression.split())
    parts = [part.strip() for part in expression.split("/")]
    if any(not part for part in parts):
        raise SbomError(
            SbomErrorCode.COMPONENT_INCOMPLETE,
            f"license declaration {expression!r} has an empty /-separated part",
        )
    return " OR ".join(" ".join(part.split()) for part in parts)


def valid_spdx_expression(expression: str) -> bool:
    """Whether an expression parses under the SPDX license-expression subset.

    Operators are the uppercase `AND`, `OR`, and `WITH`; `WITH` joins a
    license id to an exception id; parentheses group; a trailing `+` keeps
    its spec meaning. Anything else (including `/`, lowercase operators,
    empty operands, or unbalanced parentheses) is not a usable expression.
    """

    def parse_primary(tokens: list[str], position: int) -> int:
        if position >= len(tokens):
            return -1
        token = tokens[position]
        if token == "(":
            position = parse_or(tokens, position + 1)
            if position < 0 or position >= len(tokens) or tokens[position] != ")":
                return -1
            return position + 1
        if token in LICENSE_OPERATORS or token == ")":
            return -1
        if not LICENSE_TOKEN.fullmatch(token) or not any(
            character.isalnum() for character in token
        ):
            return -1
        position += 1
        if position < len(tokens) and tokens[position] == "WITH":
            position += 1
            if (
                position >= len(tokens)
                or tokens[position] in LICENSE_OPERATORS
                or tokens[position] in ("(", ")")
                or not LICENSE_TOKEN.fullmatch(tokens[position])
            ):
                return -1
            position += 1
        return position

    def parse_and(tokens: list[str], position: int) -> int:
        position = parse_primary(tokens, position)
        while position >= 0 and position < len(tokens) and tokens[position] == "AND":
            position = parse_primary(tokens, position + 1)
        return position

    def parse_or(tokens: list[str], position: int) -> int:
        position = parse_and(tokens, position)
        while position >= 0 and position < len(tokens) and tokens[position] == "OR":
            position = parse_and(tokens, position + 1)
        return position

    spaced = expression.replace("(", " ( ").replace(")", " ) ")
    tokens = spaced.split()
    if not tokens:
        return False
    end = parse_or(tokens, 0)
    return end == len(tokens)


def component_facts(package: dict) -> dict:
    """The section 1 required facts of one inventory package."""
    purl = package.get("bom_ref")
    name = package.get("name")
    version = package.get("version")
    ecosystem = package.get("ecosystem")
    license_expression = package.get("license_declared")
    for key, value in (
        ("bom_ref", purl),
        ("name", name),
        ("version", version),
        ("ecosystem", ecosystem),
        ("license_declared", license_expression),
    ):
        if not isinstance(value, str) or not value:
            raise SbomError(
                SbomErrorCode.COMPONENT_INCOMPLETE,
                f"{purl or name or 'component'} has no {key}",
            )
    normalized = normalize_license(license_expression)
    if not valid_spdx_expression(normalized):
        raise SbomError(
            SbomErrorCode.COMPONENT_INCOMPLETE,
            f"{purl or name or 'component'} declares {license_expression!r}, "
            "which is not a usable SPDX license expression",
        )
    digests = package.get("artifact_hashes")
    single = package.get("checksum_sha256")
    if isinstance(digests, list) and len(digests) == 1:
        single = digests[0].split(":")[-1]
    return {
        "purl": purl,
        "name": name,
        "version": version,
        "ecosystem": ecosystem,
        "license": normalized,
        "disposition": package.get("license_disposition", "UNKNOWN"),
        "source": package.get("source", "workspace"),
        "workspace": bool(package.get("workspace")),
        "single_digest": single if isinstance(single, str) and len(single) == 64 else None,
        "locked_artifact_digests": len(digests) if isinstance(digests, list) else None,
    }


def dependency_map(relationships: list) -> dict[str, list[str]]:
    edges: dict[str, list[str]] = {}
    for relationship in relationships:
        if not isinstance(relationship, dict) or "from" not in relationship or "to" not in relationship:
            raise SbomError(SbomErrorCode.INVENTORY_INVALID, "malformed dependency relationship")
        edges.setdefault(relationship["from"], []).append(relationship["to"])
    return {key: sorted(set(value)) for key, value in edges.items()}


def derived_uuid(digest: str) -> str:
    """A deterministic UUID from a digest: version nibble 8, variant nibble 8."""
    raw = list(digest[:32])
    raw[12] = "8"
    raw[16] = "8"
    hexadecimal = "".join(raw)
    return (
        f"{hexadecimal[0:8]}-{hexadecimal[8:12]}-{hexadecimal[12:16]}-"
        f"{hexadecimal[16:20]}-{hexadecimal[20:32]}"
    )


def cyclonedx(facts: list[dict], edges: dict[str, list[str]], candidate: dict, inventory_digest: str) -> dict:
    root_ref = f"pkg:generic/sley@{CANDIDATE_VERSION}"
    components = []
    for fact in facts:
        component = {
            "bom-ref": fact["purl"],
            "type": "library",
            "name": fact["name"],
            "version": fact["version"],
            "purl": fact["purl"],
            "licenses": [{"expression": fact["license"]}],
            "properties": [
                {"name": "sley2:ecosystem", "value": fact["ecosystem"]},
                {"name": "sley2:license-disposition", "value": fact["disposition"]},
                {"name": "sley2:locked-source", "value": fact["source"]},
            ],
        }
        if fact["single_digest"] is not None:
            component["hashes"] = [{"alg": "SHA-256", "content": fact["single_digest"]}]
        elif fact["locked_artifact_digests"]:
            component["properties"].append(
                {
                    "name": "sley2:locked-artifact-digests",
                    "value": str(fact["locked_artifact_digests"]),
                }
            )
        if not fact["workspace"]:
            component["externalReferences"] = [
                {"type": "distribution", "url": fact["source"].replace("registry+", "")}
            ]
        component["properties"].sort(key=lambda item: item["name"])
        components.append(component)
    blocked = sorted(
        {fact["purl"] for fact in facts if fact["disposition"].startswith("BLOCKED")}
    )
    bom = {
        "$schema": "http://cyclonedx.org/schema/bom-1.6.schema.json",
        "bomFormat": "CycloneDX",
        "specVersion": "1.6",
        "version": 1,
        "metadata": {
            "component": {
                "bom-ref": root_ref,
                "type": "application",
                "name": candidate["artifact_name"],
                "version": CANDIDATE_VERSION,
                "hashes": [{"alg": "SHA-256", "content": candidate["artifact_sha256"]}],
                "licenses": [{"expression": ROOT_LICENSE}],
            },
            "properties": sorted(
                [
                    {"name": "sley2:commit", "value": candidate["commit"]},
                    {"name": "sley2:manifest-digest", "value": candidate["manifest_digest"]},
                    {"name": "sley2:inventory-digest", "value": inventory_digest},
                    {
                        "name": "sley2:license-disposition-blocked",
                        "value": str(len(blocked)),
                    },
                    {"name": "sley2:signed", "value": "false"},
                    {"name": "sley2:publication-authorized", "value": "false"},
                ],
                key=lambda item: item["name"],
            ),
            "tools": {
                "components": [
                    {"type": "application", "name": TOOL_NAME, "version": TOOL_VERSION}
                ]
            },
        },
        "components": components,
        "dependencies": [
            {"ref": root_ref, "dependsOn": sorted(fact["purl"] for fact in facts if fact["workspace"])},
            *[
                {"ref": fact["purl"], "dependsOn": edges.get(fact["purl"], [])}
                for fact in facts
            ],
        ],
    }
    bom["serialNumber"] = f"urn:uuid:{derived_uuid(digest_of(bom))}"
    return bom


def spdx_identifier(fact: dict) -> str:
    return "SPDXRef-" + SPDX_ID.sub("-", f"{fact['ecosystem']}-{fact['name']}-{fact['version']}")


def spdx(facts: list[dict], relationships: list, candidate: dict, inventory_digest: str) -> dict:
    root = "SPDXRef-Package-sley-candidate"
    identifiers = {fact["purl"]: spdx_identifier(fact) for fact in facts}
    packages = [
        {
            "SPDXID": root,
            "name": candidate["artifact_name"],
            "versionInfo": CANDIDATE_VERSION,
            "downloadLocation": "NOASSERTION",
            "filesAnalyzed": False,
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": ROOT_LICENSE,
            "copyrightText": "NOASSERTION",
            "checksums": [{"algorithm": "SHA256", "checksumValue": candidate["artifact_sha256"]}],
            "comment": (
                "the local Sley 2 release candidate; unsigned, unpublished, and not a "
                "GA claim (S20-720, S20-730)"
            ),
        }
    ]
    for fact in facts:
        package = {
            "SPDXID": identifiers[fact["purl"]],
            "name": fact["name"],
            "versionInfo": fact["version"],
            "downloadLocation": (
                "NOASSERTION" if fact["workspace"] else fact["source"].replace("registry+", "")
            ),
            "filesAnalyzed": False,
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": fact["license"],
            "copyrightText": "NOASSERTION",
            "externalRefs": [
                {
                    "referenceCategory": "PACKAGE-MANAGER",
                    "referenceType": "purl",
                    "referenceLocator": fact["purl"],
                }
            ],
            "comment": f"license disposition {fact['disposition']}",
        }
        if fact["single_digest"] is not None:
            package["checksums"] = [
                {"algorithm": "SHA256", "checksumValue": fact["single_digest"]}
            ]
        elif fact["locked_artifact_digests"]:
            package["comment"] += (
                f"; {fact['locked_artifact_digests']} locked platform artifacts, so no single "
                "artifact digest identifies this package"
            )
        packages.append(package)
    document = {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": f"sley-{CANDIDATE_VERSION}-linux-x86_64",
        "documentNamespace": f"urn:sley2:spdx:{inventory_digest}:{candidate['artifact_sha256']}",
        "creationInfo": {
            "created": "1970-01-01T00:00:00Z",
            "creators": [f"Tool: {TOOL_NAME}-{TOOL_VERSION}"],
            "comment": (
                "derived from the S20-710 locked offline inventory and the S20-720 candidate "
                "evidence; the fixed creation instant keeps the document deterministic, and "
                "the document is a local draft with no license conclusion and no publication"
            ),
        },
        # No `hasExtractedLicensingInfos`: SPDX 2.3 clause 10.1 reserves that
        # section for licenses absent from the SPDX license list, identified
        # by `LicenseRef-` ids. `Apache-2.0` is a listed identifier, so it is
        # referenced by id only (Ariadne P3, 2026-09-15 at a809906).
        "packages": packages,
        "relationships": [
            {
                "spdxElementId": "SPDXRef-DOCUMENT",
                "relationshipType": "DESCRIBES",
                "relatedSpdxElement": root,
            },
            *sorted(
                (
                    {
                        "spdxElementId": identifiers[relationship["from"]],
                        "relationshipType": "DEPENDS_ON",
                        "relatedSpdxElement": identifiers[relationship["to"]],
                    }
                    for relationship in relationships
                    if relationship["from"] in identifiers and relationship["to"] in identifiers
                ),
                key=lambda item: (item["spdxElementId"], item["relatedSpdxElement"]),
            ),
        ],
    }
    return document


def git_head() -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        raise SbomError(SbomErrorCode.INVENTORY_INVALID, "git HEAD unavailable")
    return completed.stdout.strip()


def require_attested_candidate(candidate: dict) -> None:
    """The SBOM write-mode admission gate, mirroring the provenance builder.

    The namespace and root are derived from the candidate evidence, so a
    candidate that is not HEAD or that no clean REPRODUCIBLE attestation
    names must refuse here instead of emitting documents the checker must
    catch (contract section 5; 74001 SBOM_INVENTORY_INVALID). A candidate
    behind HEAD is still admissible when HEAD is a provable records-closure
    of the candidate commit (contract revision 5: attestation-bound inputs
    unchanged, bound artifacts invariant, SBOM stays candidate-bound, the
    closure HEAD is recorded separately and nothing is re-minted). The full
    partial record a failed mint keeps is still not admissible: only a
    PASS record whose manifest digest and size agree with the attestation
    derives documents.
    """
    if candidate.get("commit") != git_head():
        status = records_closure.closure_status(str(candidate.get("commit")))
        if not status.is_closure:
            raise SbomError(
                SbomErrorCode.INVENTORY_INVALID,
                f"candidate commit {candidate.get('commit')} is not HEAD; "
                f"{status.reason}; rebuild the candidate on this tree "
                "before deriving SBOMs",
            )
    if candidate.get("result") != "PASS":
        raise SbomError(
            SbomErrorCode.INVENTORY_INVALID,
            "candidate evidence is not a PASS record; "
            "rebuild the candidate on this tree before deriving SBOMs",
        )
    try:
        report = json.loads(REPRO_REPORT.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SbomError(SbomErrorCode.INVENTORY_INVALID, f"attestation report unreadable: {error}") from error
    if repro.select_attestation(report, candidate) is None:
        raise SbomError(
            SbomErrorCode.INVENTORY_INVALID,
            "no clean REPRODUCIBLE attestation names this candidate; "
            "re-mint the reproducibility attestation first",
        )


def build_documents() -> tuple[dict, dict, dict]:
    inventory = load_inventory()
    candidate = load_candidate()
    require_attested_candidate(candidate)
    facts = sorted(
        (component_facts(package) for package in inventory["packages"]),
        key=lambda fact: fact["purl"],
    )
    edges = dependency_map(inventory["relationships"])
    inventory_digest = file_digest(INVENTORY)
    return (
        cyclonedx(facts, edges, candidate, inventory_digest),
        spdx(facts, inventory["relationships"], candidate, inventory_digest),
        {"components": len(facts), "relationships": len(inventory["relationships"])},
    )


def tracked_candidate_facts() -> tuple[str, str] | None:
    """The commit and artifact digest the tracked CycloneDX document records."""
    if not CYCLONEDX.exists():
        return None
    try:
        bom = json.loads(CYCLONEDX.read_text(encoding="utf-8"))
        commit = next(
            entry["value"]
            for entry in bom["metadata"]["properties"]
            if entry["name"] == "sley2:commit"
        )
        digest = next(
            entry["content"]
            for entry in bom["metadata"]["component"]["hashes"]
            if entry["alg"] == "SHA-256"
        )
        return commit, digest
    except (OSError, json.JSONDecodeError, KeyError, StopIteration):
        return None


def recorded_summary_facts(bom: dict, provenance: dict) -> dict:
    """Summary mirrors of the emitted S20-710 documents, with required shape."""
    properties = {item["name"]: item["value"] for item in bom["metadata"]["properties"]}
    blocked = properties["sley2:license-disposition-blocked"]
    blockers = provenance["attestation"]["blockers"]
    if not isinstance(blocked, str) or not re.fullmatch(r"[0-9]+", blocked):
        raise ValueError("invalid blocked-license count")
    if not isinstance(blockers, list) or not all(isinstance(item, str) for item in blockers):
        raise ValueError("invalid provenance blockers")
    return {"license_disposition_blocked_components": int(blocked), "blockers": blockers}


def validate_tracked() -> list[str]:
    """Internal consistency of the tracked pair, checked even when the
    untracked candidate evidence disagrees with the tracked documents
    (a skipped verification must not read PASS over an unchecked document).
    Mirrors the provenance builder's validate_tracked:
    shape, determinism pins, and the attestation binding of the namespace."""
    problems: list[str] = []
    try:
        bom = json.loads(CYCLONEDX.read_text(encoding="utf-8"))
        document = json.loads(SPDX.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"unreadable: {error}"]
    if bom.get("specVersion") != "1.6" or bom.get("bomFormat") != "CycloneDX":
        problems.append("cyclonedx-format")
    if not str(bom.get("serialNumber", "")).startswith("urn:uuid:"):
        problems.append("cyclonedx-serial")
    if "timestamp" in bom.get("metadata", {}):
        problems.append("cyclonedx-timestamp")
    if document.get("spdxVersion") != "SPDX-2.3":
        problems.append("spdx-version")
    if document.get("creationInfo", {}).get("created") != "1970-01-01T00:00:00Z":
        problems.append("spdx-created")
    try:
        inventory_digest = file_digest(INVENTORY)
    except SbomError as error:
        return problems + [f"inventory: {error.detail}"]
    try:
        report = json.loads(REPRO_REPORT.read_text(encoding="utf-8"))
        attested = {
            attestation.get("artifact_sha256")
            for attestation in repro.admissible_attestations(report)
        }
    except (OSError, json.JSONDecodeError):
        attested = set()
    expected = {f"urn:sley2:spdx:{inventory_digest}:{digest}" for digest in attested}
    if document.get("documentNamespace") not in expected:
        problems.append("namespace-not-attestation-bound")
    return problems


def candidate_evidence_mismatch() -> bool:
    """Whether a local candidate build replaced the evidence the documents describe.

    `evidence/runtime/` is not tracked, so a fresh candidate build legitimately
    leaves the tracked documents describing the previous candidate until
    `make release-candidate-smoke` reconciles them (contract section 5). The
    short-circuit fires only when the evidence loads and disagrees with the
    tracked documents: missing or unreadable evidence is missing input, and
    `--check` must fail with the input code instead of passing open.
    """
    tracked = tracked_candidate_facts()
    if tracked is None:
        return False
    try:
        candidate = load_candidate()
    except SbomError:
        return False
    return tracked != (candidate["commit"], candidate["artifact_sha256"])


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
        bom, document, counts = build_documents()
        outputs = ((CYCLONEDX, bom), (SPDX, document))
        if args.check:
            for path, value in outputs:
                current = path.read_text(encoding="utf-8") if path.exists() else None
                if current != canonical(value):
                    print(
                        canonical(
                            {
                                "mode": "check",
                                "result": "FAIL",
                                "code": int(SbomErrorCode.DOCUMENT_DRIFT),
                                "name": SbomErrorCode.DOCUMENT_DRIFT.name,
                                "detail": f"{path.relative_to(ROOT)} differs from the derived document",
                            }
                        ),
                        end="",
                    )
                    return 1
            print(canonical({"mode": "check", "result": "PASS", **counts}), end="")
            return 0
        for path, value in outputs:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(canonical(value), encoding="utf-8")
        print(
            canonical(
                {
                    "mode": "write",
                    "result": "PASS",
                    "cyclonedx": str(CYCLONEDX.relative_to(ROOT)),
                    "spdx": str(SPDX.relative_to(ROOT)),
                    **counts,
                }
            ),
            end="",
        )
        return 0
    except SbomError as error:
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
