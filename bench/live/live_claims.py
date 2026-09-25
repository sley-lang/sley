"""Per-arm verifiers turning verified live attempts into accounting claims.

S20-630 accounting consumes claims, each verified by its arm's own
verifier. The live campaign writes one ``attempts.jsonl`` for all three
arms; this module is the verifier registry for that input. Every arm's
verifier:

1. runs ``verify_attempts`` (full chain, every referenced artifact
   resolved by digest, environment receipt, provider usage and oracle
   stdout/report reconciliation for judged attempts, sley_2_0 evidence
   completion binding);
2. reconciles the token and tool metrics of every *unjudged* attempt too:
   a provider stream that completed (the budget-breach case) must equal the
   recorded metrics, and a stream with no ``turn.completed`` must carry
   zero observable tokens and tool items (nothing observable was
   reported, so nothing may be claimed);
3. binds each attempt's starting state: the stored ``workspace_before``
   snapshot must be byte-identical to the frozen initial state the arm
   stages for that task in this checkout, and the manifest's arm fixture
   digest must equal the recomputed one.

The legacy verifier (``sley_1_2_0``, the chain S20-630 section 1 left
without a verifier) additionally binds the frozen 1.2.0 artifact: the
plan's pinned ``artifact_sha256`` and ``commit`` must equal the legacy
runner's frozen contract that every legacy tool invocation and oracle
stages and verifies.

It executes nothing: it reads the run directory, the artifact store, the
frozen fixtures, and the plan.
"""

from __future__ import annotations

import json
import tempfile
from pathlib import Path
from typing import Any, Callable, Mapping

from bench.live.artifacts import ArtifactStore
from bench.live.attempts import AGENT_NO_FINAL, AttemptError, verify_attempts
from bench.live.manifest import read_manifest
from bench.live.provider import ProviderError, parse_provider_events
from bench.live.snapshot import encode_snapshot, snapshot_directory


ROOT = Path(__file__).resolve().parents[2]
PLAN = ROOT / "bench" / "benchmark-plan.json"
EVIDENCE_STATUS = "VERIFIED_LIVE_EVIDENCE"
ORACLE_JUDGED = "VERIFIED_ORACLE_STDOUT_AND_REPORT_RECONCILED"
ORACLE_UNJUDGED = "VERIFIED_NO_ORACLE_VERDICT"
USAGE_RECONCILED = "VERIFIED_PROVIDER_USAGE_RECONCILED"
USAGE_ABSENT = "VERIFIED_NO_PROVIDER_USAGE_REPORTED"
# The oracle report's two section-22 fields are written by the runner as
# constants (bench/live/campaign.py _oracle_report: required_check_bypassed
# False, mutation_reconstructable True for every attempt), not measured by
# the judge. A constant is never admitted as evidence of the condition.
SECTION_22_BASIS = "RUNNER_CONSTANT_NOT_A_MEASUREMENT"


class LiveClaimError(ValueError):
    """A live attempt does not verify as an accounting claim."""


def _fail(detail: str) -> None:
    raise LiveClaimError(f"LIVE_CLAIM_INVALID: {detail}")


def _initial_snapshot(arm_id: str, task_id: str) -> bytes:
    from bench.live.taskpacks import stage_initial
    from bench.live.tooling import stage_tooling

    with tempfile.TemporaryDirectory(prefix="live-claim-start-") as temporary:
        workspace = Path(temporary) / "candidate"
        stage_initial(arm_id, task_id, workspace)
        if arm_id != "sley_2_0":
            stage_tooling(arm_id, workspace)
        return encode_snapshot(snapshot_directory(workspace))


def _reconcile_unjudged(record: Mapping[str, Any], events: bytes, model_provider: str) -> str:
    metrics = record["metrics"]
    try:
        if record["failure_code"] == "LIVE_PROVIDER_EVENT_INVALID":
            # The runner could not parse this stream, so it claimed no
            # usage; the claim is held to that even if a later parser
            # revision reads more of the retained stream.
            raise ProviderError("LIVE_PROVIDER_EVENT_INVALID: recorded")
        observed = parse_provider_events(model_provider, events)
    except ProviderError:
        if (metrics["model_input_tokens"], metrics["model_output_tokens"],
                metrics["total_observable_tokens"], metrics["tool_calls"]) != (0, 0, 0, 0):
            _fail(f"{record['attempt_id']}: usage claimed without a completed provider stream")
        return USAGE_ABSENT
    if (metrics["model_input_tokens"] != observed.input_tokens
            or metrics["model_output_tokens"] != observed.output_tokens
            or metrics["tool_calls"] != observed.tool_calls):
        _fail(f"{record['attempt_id']}: unjudged usage reconciliation")
    return USAGE_RECONCILED


def _claim(record: Mapping[str, Any], store: ArtifactStore, model_provider: str) -> dict[str, Any]:
    judged = record["status"] in {"accepted", "rejected"}
    events = store.read(record["artifacts"]["provider_events_sha256"])
    usage = USAGE_RECONCILED if judged else _reconcile_unjudged(record, events, model_provider)
    # An agent that ended without a final candidate is recorded like a
    # rejection, but no oracle ran on it (preregistration revision 4).
    oracle_ran = judged and record["failure_code"] != AGENT_NO_FINAL
    return {
        "accounting_verification_status": usage,
        "arm_id": record["arm_id"],
        "evidence_status": record["evidence_status"],
        "failure_code": record["failure_code"],
        "metrics": dict(record["metrics"]),
        "oracle_verification_status": ORACLE_JUDGED if oracle_ran else ORACLE_UNJUDGED,
        "record_digest": record["record_digest"],
        "section_22": {
            "basis": SECTION_22_BASIS,
            "mutation_reconstructable": None,
            "required_check_bypassed": None,
        },
        "seed": record["seed"],
        "status": record["status"],
        "task_id": record["task_id"],
        "trial_id": record["attempt_id"],
    }


def verify_live_arm_claims(run_directory: Path, arm_id: str) -> list[dict[str, Any]]:
    """Verified claims of one arm from a live run directory, chain order."""

    from bench.live.taskpacks import arm_fixture_digest

    run = Path(run_directory)
    try:
        manifest = read_manifest(run / "run_manifest.json")
        store = ArtifactStore(run / "artifacts")
        records = verify_attempts(run, store)
    except (AttemptError, ValueError, OSError) as error:
        raise LiveClaimError(f"LIVE_CLAIM_INVALID: {error}") from error
    if manifest["arm_fixture_digests"][arm_id] != arm_fixture_digest(arm_id):
        _fail(f"{arm_id}: arm fixture digest differs from this checkout")
    claims = []
    starts: dict[str, bytes] = {}
    for record in records:
        if record["evidence_status"] != EVIDENCE_STATUS:
            _fail(f"{record['attempt_id']}: evidence status")
        if record["arm_id"] != arm_id:
            continue
        task = record["task_id"]
        if task not in starts:
            starts[task] = _initial_snapshot(arm_id, task)
        before = store.read(record["artifacts"]["workspace_before_sha256"])
        if before != starts[task]:
            _fail(f"{record['attempt_id']}: starting state is not the frozen initial state")
        claims.append(_claim(record, store, manifest["model_provider"]))
    return claims


def verify_live_raw_claims(run_directory: Path) -> list[dict[str, Any]]:
    return verify_live_arm_claims(run_directory, "raw_files")


def verify_live_legacy_claims(run_directory: Path) -> list[dict[str, Any]]:
    """The legacy (sley_1_2_0) chain verifier: live attempts bound to the
    plan's frozen 1.2.0 artifact."""

    from bench.legacy.runner import FROZEN_CONTRACT

    plan = json.loads(PLAN.read_text(encoding="utf-8"))
    legacy = next(arm for arm in plan["arms"] if arm["id"] == "sley_1_2_0")
    if (legacy.get("artifact_sha256") != FROZEN_CONTRACT.artifact_sha256
            or legacy.get("commit") != FROZEN_CONTRACT.source_commit):
        _fail("sley_1_2_0: plan artifact differs from the legacy runner's frozen contract")
    return verify_live_arm_claims(run_directory, "sley_1_2_0")


def verify_live_sley2_claims(run_directory: Path) -> list[dict[str, Any]]:
    return verify_live_arm_claims(run_directory, "sley_2_0")


LIVE_ARM_VERIFIERS: dict[str, Callable[[Path], list[dict[str, Any]]]] = {
    "raw_files": verify_live_raw_claims,
    "sley_1_2_0": verify_live_legacy_claims,
    "sley_2_0": verify_live_sley2_claims,
}
