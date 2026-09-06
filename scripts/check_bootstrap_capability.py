#!/usr/bin/env python3
"""RW-040 staged bootstrap-capability checker (audit scope only).

Checks evidence bindings, coverage, prerequisites, and stage status for the
bootstrap-capability audit. It NEVER executes bytecode, judges opcodes, or
re-derives observations: production semantics belong to
`scripts/check_vm_extended_opcode_profile.py` and the `sley-vm` test suite,
which this checker does not duplicate. It only verifies that audit rows bind
to committed evidence (vector ids/digests in
`conformance/vm-extended/v1/accepted.json`), that every gap is routed, that
the stage manifest is structurally honest (no invented hashes), and that
readiness cannot pass while stages are unbound or RW-030 is unchartered.

Two outputs: `audit` (PASS/FAIL, exit code) and `readiness` (READY/NOT_READY
with reasons; informational -- NOT_READY is the expected audit-scope outcome
and never fails this checker). The negative corpus under
`conformance/bootstrap-capability/v1/` proves no target passes
unconditionally: each corpus case declares its expected verdict and any
mismatch fails this checker, including one synthetic positive proving the
readiness path is not rigged to NOT_READY.
"""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REWEAVE = ROOT / "machineresearch/sley-2.0/reweave"
EXERCISES = REWEAVE / "rw-040-bootstrap-exercises-slice-2.json"
GAPS = REWEAVE / "rw-040-m2-gap-list-slice-2.json"
FINDINGS = REWEAVE / "rw-040-findings-slice-2.json"
MANIFEST = REWEAVE / "bootstrap-manifest.json"
ACCEPTED = ROOT / "conformance/vm-extended/v1/accepted.json"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
CORPUS = ROOT / "conformance/bootstrap-capability/v1"
CHARTER_CANDIDATES = (
    ROOT / "reweave/host-boundary.json",
    ROOT / "evidence/reweave/host-boundary.json",
)

STAGES = ("S", "P", "C0", "C1", "C2", "C3")
CLASSIFICATIONS = (
    "BOOTSTRAP_BLOCKER",
    "FULL_MG_OBLIGATION",
    "EVIDENCE_GATE",
    "NON_GOAL",
)
HEX64 = re.compile(r"[0-9a-f]{64}")
HEXISH = re.compile(r"[0-9a-f]{8,64}\Z")


def read(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def manifest_stages(doc) -> dict:
    if isinstance(doc, dict) and isinstance(doc.get("stages"), dict):
        return doc["stages"]
    return doc if isinstance(doc, dict) else {}


def check_manifest_structure(stages: dict, chartered: bool) -> list[str]:
    """Structural honesty of S/P/H/C0-C3. Missing values are unbound (a
    readiness matter), never a structural failure; anything hash-shaped
    without provenance is."""
    problems: list[str] = []
    for name in STAGES:
        stage = stages.get(name)
        if not isinstance(stage, dict):
            problems.append(f"manifest:missing-stage-key:{name}")
            continue
        value = stage.get("value")
        if value is None:
            continue
        digest = stage.get("digest")
        provenance = stage.get("provenance")
        if not (isinstance(digest, str) and HEX64.fullmatch(digest)):
            problems.append(f"manifest:{name}-bound-without-digest")
        if not (isinstance(provenance, dict) and provenance):
            problems.append(f"manifest:{name}-bound-without-provenance")
        blob_hashes = set(HEX64.findall(json.dumps(stage, sort_keys=True)))
        blob_hashes.discard(digest if isinstance(digest, str) else "")
        if blob_hashes:
            problems.append(f"manifest:{name}-unprovenanced-hash")
    bound = lambda name: isinstance(stages.get(name), dict) and stages[name].get("value") is not None
    if (not bound("S") or not bound("P")) and any(bound(stage) for stage in ("C1", "C2", "C3")):
        problems.append("manifest:stage-without-source")
    host = stages.get("H")
    if not isinstance(host, dict):
        problems.append("manifest:missing-stage-key:H")
    else:
        host_digest = host.get("manifest_digest")
        if host_digest is not None and not chartered:
            problems.append("manifest:h-digest-without-charter")
    return problems


def check_exercise_bindings(doc, accepted_ids: set[str], accepted_digests: dict) -> list[str]:
    """Every PASS-exists row binds committed vectors; every digest-shaped
    claim keyed by a vector id must prefix-match the committed digest. An
    audit may document missing capabilities (REWEAVE 24.1: an audit is not a
    readiness claim), so PARTIAL-documented / FAIL-missing-capability rows
    pass binding only when routed (routes_to + gap); unrouted gaps fail."""
    problems: list[str] = []
    exercises = doc.get("exercises", []) if isinstance(doc, dict) else []
    if not exercises:
        return ["exercises:no-rows"]
    for row in exercises:
        label = row.get("id", "?")
        disposition = row.get("disposition")
        vectors = row.get("vectors", [])
        if disposition == "PASS-exists":
            if not vectors:
                problems.append(f"exercises:{label}-unbound")
        elif disposition in ("PARTIAL-documented", "FAIL-missing-capability"):
            if not row.get("routes_to") or not row.get("gap"):
                problems.append(f"exercises:{label}-unrouted-open-part")
        else:
            problems.append(f"exercises:{label}-not-pass")
            continue
        for vector in vectors:
            if vector not in accepted_ids:
                problems.append(f"exercises:{label}-unknown-vector:{vector}")
    def scan(node, where: str):
        if isinstance(node, dict):
            for key, value in node.items():
                if (
                    isinstance(value, str)
                    and key in accepted_ids
                    and HEXISH.fullmatch(value)
                    and not accepted_digests.get(key, "").startswith(value)
                ):
                    problems.append(f"exercises:digest-mismatch:{key}-at-{where}")
                else:
                    scan(value, where)
        elif isinstance(node, list):
            for index, value in enumerate(node):
                scan(value, f"{where}[{index}]")
    scan(doc, "exercises")
    return problems


def check_gap_routing(doc) -> tuple[list[str], list[str]]:
    """Every gap is classified, owned, and evidenced. Returns (problems,
    open_bootstrap_blockers). A well-routed open blocker is an audit PASS
    with a readiness reason, never a silent deferral."""
    problems: list[str] = []
    open_blockers: list[str] = []
    gaps = doc.get("gaps", []) if isinstance(doc, dict) else []
    if not gaps:
        return ["gaps:no-rows"], []
    for row in gaps:
        label = row.get("id", "?")
        classification = row.get("classification")
        if classification not in CLASSIFICATIONS:
            problems.append(f"gaps:{label}-unclassified")
        if not row.get("owner"):
            problems.append(f"gaps:{label}-unowned")
        if not row.get("evidence"):
            problems.append(f"gaps:{label}-unevidenced")
        closed = bool(row.get("closed_how"))
        if closed and not str(row.get("disposition", "")).startswith("CLOSED"):
            problems.append(f"gaps:{label}-closed-without-disposition")
        if classification == "BOOTSTRAP_BLOCKER" and not closed:
            open_blockers.append(str(label))
    return problems, open_blockers


def check_findings_format(doc) -> list[str]:
    problems: list[str] = []
    if not isinstance(doc, dict) or not isinstance(doc.get("findings"), list):
        return ["findings:shape"]
    for finding in doc["findings"]:
        for field in ("id", "severity", "status", "owner", "evidence"):
            if not finding.get(field):
                problems.append(f"findings:missing-{field}:{finding.get('id', '?')}")
    return problems


def readiness_reasons(exercises, gaps, stages, chartered: bool) -> list[str]:
    reasons: list[str] = []
    rows = exercises.get("exercises", []) if isinstance(exercises, dict) else []
    incomplete = [row.get("id") for row in rows if row.get("disposition") != "PASS-exists"]
    if incomplete:
        reasons.append(f"exercises-incomplete:{incomplete}")
    _, open_blockers = check_gap_routing(gaps)
    if open_blockers:
        reasons.append(f"open-bootstrap-blockers:{open_blockers}")
    unbound = [
        name
        for name in STAGES
        if not (
            isinstance(stages.get(name), dict) and stages[name].get("value") is not None
        )
    ]
    if unbound:
        reasons.append(f"unbound-stages:{unbound}")
    if not chartered:
        reasons.append("rw030-not-chartered")
    return reasons


def live_chartered() -> bool:
    return any(path.is_file() for path in CHARTER_CANDIDATES)


def run_corpus(accepted_ids: set[str], accepted_digests: dict) -> tuple[dict, list[str]]:
    """Evaluate every corpus case with the same functions as the live audit.
    Any expected-verdict mismatch is a checker failure."""
    outcomes: dict = {}
    problems: list[str] = []
    cases = sorted(CORPUS.glob("*.json"))
    if not cases:
        return outcomes, ["corpus:empty"]
    for path in cases:
        doc = read(path)
        expect = doc.get("expect", {})
        name = path.name
        if name.startswith("manifest-"):
            structural = check_manifest_structure(manifest_stages(doc), chartered=False)
            reasons = readiness_reasons(
                {"exercises": []}, {"gaps": []}, manifest_stages(doc), chartered=False
            )
            actual = {
                "structural": "FAIL" if structural else "PASS",
                "readiness": "READY" if not reasons else "NOT_READY",
            }
        elif name == "exercises-bad-binding.json":
            binding = check_exercise_bindings(doc, accepted_ids, accepted_digests)
            actual = {"binding": "FAIL" if binding else "PASS"}
        elif name == "exercises-partial-routed.json":
            present = set(doc.get("vectors_present", []))
            binding = check_exercise_bindings(doc, present, {v: v for v in present})
            actual = {"binding": "FAIL" if binding else "PASS"}
        elif name == "gaps-unowned.json":
            routing, _ = check_gap_routing(doc)
            actual = {"routing": "FAIL" if routing else "PASS"}
        elif name == "readiness-logic-positive.json":
            if not doc.get("synthetic"):
                problems.append(f"corpus:{name}-positive-case-must-stay-synthetic")
                continue
            stages = manifest_stages(doc.get("manifest", {}))
            structural = check_manifest_structure(stages, chartered=True)
            vectors_present = set(doc.get("vectors_present", []))
            binding = check_exercise_bindings(doc, vectors_present, {v: v for v in vectors_present})
            routing, _ = check_gap_routing(doc)
            reasons = readiness_reasons(doc, doc, stages, chartered=bool(doc.get("chartered_override")))
            actual = {
                "structural": "FAIL" if structural else "PASS",
                "binding": "FAIL" if binding else "PASS",
                "routing": "FAIL" if routing else "PASS",
                "readiness": "READY" if not reasons else "NOT_READY",
            }
        else:
            problems.append(f"corpus:{name}-unknown-case")
            continue
        outcomes[name] = {"expected": expect, "actual": actual}
        for key, wanted in expect.items():
            if actual.get(key) != wanted:
                problems.append(f"corpus:{name}-{key}:expected-{wanted}-got-{actual.get(key)}")
    return outcomes, problems


def main() -> int:
    problems: list[str] = []
    for path in (EXERCISES, GAPS, FINDINGS, MANIFEST, ACCEPTED, SUMMARY):
        if not path.is_file():
            problems.append(f"missing:{path.relative_to(ROOT)}")
    if problems:
        print(json.dumps({"problems": problems, "result": "FAIL"}, indent=2, sort_keys=True))
        return 1
    exercises = read(EXERCISES)
    gaps = read(GAPS)
    findings = read(FINDINGS)
    manifest = read(MANIFEST)
    accepted = read(ACCEPTED)
    summary = read(SUMMARY)
    accepted_vectors = accepted.get("vectors", [])
    accepted_ids = {vector.get("id") for vector in accepted_vectors}
    accepted_digests = {vector.get("id"): vector.get("bytecode_sha256", "") for vector in accepted_vectors}

    stages = manifest_stages(manifest)
    chartered = live_chartered()
    structural = check_manifest_structure(stages, chartered)
    binding = check_exercise_bindings(exercises, accepted_ids, accepted_digests)
    routing, open_blockers = check_gap_routing(gaps)
    formatted = check_findings_format(findings)
    live_problems = (
        [f"manifest:{p}" for p in structural]
        + [f"binding:{p}" for p in binding]
        + [f"routing:{p}" for p in routing]
        + [f"findings:{p}" for p in formatted]
    )
    if summary.get("phase") != "M2":
        live_problems.append("summary:phase-context")
    reasons = readiness_reasons(exercises, gaps, stages, chartered)
    corpus_outcomes, corpus_problems = run_corpus(accepted_ids, accepted_digests)
    problems = live_problems + [f"corpus:{p}" for p in corpus_problems]
    result = {
        "audit": "RW-040",
        "chartered_rw030": chartered,
        "corpus": corpus_outcomes,
        "open_bootstrap_blockers": open_blockers,
        "problems": problems,
        "readiness": "READY" if not reasons else "NOT_READY",
        "readiness_note": "NOT_READY is the expected audit-scope outcome; an audit documenting missing capabilities or unbound stages must not yield BOOTSTRAP_READY.",
        "readiness_reasons": reasons,
        "result": "PASS" if not problems else "FAIL",
        "vectors": len(accepted_vectors),
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())
