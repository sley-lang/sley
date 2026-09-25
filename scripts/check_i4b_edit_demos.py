#!/usr/bin/env python3
"""Run the AT-MW-02 I4b edit demonstrations and judge them independently.

For each fixture variant (a/b) and each demonstration (expression,
signature) this script drives the real `sley serve` endpoint over the
version-2 profile, records the frame transcript, then judges the run
with logic independent of the demonstration derivation:

* every agent request names an allowlisted method; no denied method moves;
* a fresh live re-read reproduces the frozen before-body byte for byte;
* the frozen after-body differs in exactly the intended field;
* the recorded stored candidate validates again on a fresh session;
* each variant's candidate replayed against the other variant fails.

Usage (blake3 comes from the oracle environment):
  uv run --project oracle/scb1 --frozen python3 scripts/check_i4b_edit_demos.py \
    --sley target/debug/sley [--receipt <path>] [--timeout-seconds 90]
"""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "oracle" / "scb1" / "src"))

from bench.sley2 import entity_read_edit_demo as demo
from bench.sley2.entity_read_edit_demo import DemoError
from bench.sley2.runner import (
    Endpoint,
    endpoint_offer,
    probe_handshake,
    request_frame,
    PROFILE_ARGS,
)
from sley2_scb1_oracle import candidate as CAN
from sley2_scb1_oracle import candidate_result as CR
from sley2_scb1_oracle import entity_read as ER
from sley2_scb1_oracle.codec import encode_record, encode_uvar
from sley2_scb1_oracle.mutation_value import _encode_type as encode_declared

FIXTURES = ROOT / "bench" / "sley2" / "fixtures" / "i4b_demo_v1.json"
DEFAULT_RECEIPT = Path("/home/dev/.local/state/sley2/at-mw-02/i4b-demonstrations-receipt.json")


class Live:
    """One imported fixture on one endpoint with per-run sessions."""

    def __init__(self, sley: Path, scratch: Path, vector: dict[str, str], timeout: int) -> None:
        self.vector = vector
        self.timeout = timeout
        scratch.mkdir(parents=True, exist_ok=True)
        self.endpoint = Endpoint(sley, scratch / "repo", scratch / "report.json", timeout, PROFILE_ARGS)
        self.methods: list[str] = []
        self._rid = 0

    def seed(self, frame: dict[str, Any]) -> None:
        replies = self.endpoint.send(frame)
        assert replies[-1]["kind"] == "hello"
        answer = self.endpoint.send(
            request_frame("exchange.import", self.vector["exchange_hex"], None, 0, protocol_version=2)
        )[-1]
        if answer["kind"] != "response" or answer["flags"].get("failed"):
            raise DemoError("seed refused")

    def open_session(self, handshake: str) -> tuple[str, Any]:
        answer = self.endpoint.send(request_frame("session.open", handshake, None, 0, protocol_version=2))[-1]
        if answer["kind"] != "response" or answer["flags"].get("failed"):
            raise DemoError("open refused")
        session = answer["body"]

        def transact(method: str, body_hex: str) -> dict[str, Any]:
            self.methods.append(method)
            reply = self.endpoint.send(
                request_frame(method, body_hex, session, self._next_id(), protocol_version=2)
            )[-1]
            if reply["kind"] != "response":
                raise DemoError(f"no response for {method}")
            if reply["flags"].get("failed"):
                raise DemoError(f"{method} refused: {reply['body'][:96]}")
            return reply

        return session, transact

    def _next_id(self) -> int:
        self._rid += 1
        return self._rid

    def close(self) -> int:
        status, _ = self.endpoint.close()
        return status


def check_confinement(methods: list[str]) -> list[str]:
    problems = []
    for method in methods:
        if method not in demo.ALLOWLIST:
            problems.append(f"non-allowlisted method {method}")
    if set(methods) & demo.DENIED:
        problems.append("denied method named")
    return problems


def live_operation_body(live: Live, session: str, root: bytes, target: bytes) -> tuple[bytes, bytes]:
    request = ER.build_request_body(root, target, demo.READ_MAX_OBJECTS,
                                    demo.READ_MAX_RESPONSE_BYTES, demo.READ_MAX_WORK)
    reply = live.endpoint.send(request_frame("entity.version", request.hex(), session, live._next_id(), protocol_version=2))[-1]
    assert reply["kind"] == "response" and not reply["flags"].get("failed")
    response = ER.decode_response_body(bytes.fromhex(reply["body"]))
    entry = ER.decode_response_entry(response["entries"][0])
    checked = ER.check_stored_object(entry["stored"], 8, target, response["epoch"])
    return checked["body"], checked["object_id"]


def judge_expression(live: Live, session: str, evidence: dict[str, Any], ids: dict[str, str]) -> list[str]:
    problems: list[str] = []
    problems.extend(check_confinement(evidence["methods"]))
    root = bytes.fromhex(evidence["root"])
    target = bytes.fromhex(ids["operation"])
    body, object_id = live_operation_body(live, session, root, target)
    if object_id.hex() != evidence["before"]["object_id"]:
        problems.append("before object id moved under the demonstration")
    if encode_declared("OperationBody", without_object_id(evidence["before"]), 0) != body:
        problems.append("frozen before-body does not match the live re-read")
    before, after = evidence["before"], evidence["after"]
    changed = [key for key in ("block", "ordinal", "opcode", "operands", "result_types", "immediate")
               if before[key] != after[key]]
    if changed != ["opcode"]:
        problems.append(f"expression edit must move only the opcode, moved {changed}")
    pair = {before["opcode"], after["opcode"]}
    if pair != {demo.BOOL_AND, demo.BOOL_OR}:
        problems.append(f"expression edit must flip And<->Or, saw {pair}")
    if after["operands"] != before["operands"]:
        problems.append("expression edit must preserve operands")
    return problems


def judge_signature(live: Live, session: str, evidence: dict[str, Any], ids: dict[str, str]) -> list[str]:
    problems: list[str] = []
    problems.extend(check_confinement(evidence["methods"]))
    root = bytes.fromhex(evidence["root"])
    target = bytes.fromhex(ids["operation"])
    body, object_id = live_operation_body(live, session, root, target)
    if object_id.hex() != evidence["before"]["object_id"]:
        problems.append("before object id moved under the demonstration")
    if encode_declared("OperationBody", without_object_id(evidence["before"]), 0) != body:
        problems.append("frozen before-body does not match the live re-read")
    before, after = evidence["before"], evidence["after"]
    changed = [key for key in ("block", "ordinal", "opcode", "operands", "result_types", "immediate")
               if before[key] != after[key]]
    if changed != ["operands"]:
        problems.append(f"signature edit must move only operands, moved {changed}")
    if before["operands"][0]["variant"] != "Parameter" or after["operands"][0]["variant"] != "Parameter":
        problems.append("signature edit first operand must stay a parameter reference")
    order = evidence["parameter_order"]
    types = evidence["parameter_types"]
    if len(order) != 2 or set(types) != {"Bool"}:
        problems.append("signature evidence must show two Bool parameters")
    if before["operands"][0]["value"] not in order or after["operands"][0]["value"] not in order:
        problems.append("signature edit operands must name declared parameters")
    if after["operands"][0]["value"] == before["operands"][0]["value"]:
        problems.append("signature edit must change the first operand")
    if after["operands"][1:] != before["operands"][1:]:
        problems.append("signature edit must preserve the remaining operands")
    return problems


def without_object_id(frozen: dict[str, Any]) -> dict[str, Any]:
    return {key: value for key, value in frozen.items() if key != "object_id"}


def revalidate(live: Live, session: str, head_tx: bytes, principal: bytes, stored_hex: str) -> dict[str, Any]:
    CAN.import_candidate(bytes.fromhex(stored_hex))
    now_ms = int(time.time() * 1_000)
    body = encode_record(
        [(1, head_tx), (2, principal), (3, encode_uvar(now_ms)), (4, bytes.fromhex(stored_hex))]
    )
    reply = live.endpoint.send(request_frame("candidate.validate", body.hex(), session, live._next_id(), protocol_version=2))[-1]
    if reply["kind"] != "response":
        raise DemoError("no validate response")
    if reply["flags"].get("failed"):
        return {"valid": False, "body": reply["body"][:96]}
    result = CR.decode_candidate_result(bytes.fromhex(reply["body"]))
    return {"valid": result["decision_tag"] == 1 and result["failed_phase"] is None, "result": result}


def run_case(live: Live, handshake: str, demo_id: str, ids: dict[str, str]) -> dict[str, Any]:
    session, transact = live.open_session(handshake)
    principal = bytes.fromhex(ids["principal"])
    start = len(live.methods)
    if demo_id == "expression":
        evidence = demo.demo_expression_edit(transact, bytes.fromhex(ids["operation"]), principal)
    else:
        evidence = demo.demo_signature_edit(
            transact, bytes.fromhex(ids["function"]), bytes.fromhex(ids["operation"]), principal
        )
    evidence["methods"] = live.methods[start:]
    evidence["variant"] = live.vector["variant"]
    submitted = evidence.pop("submitted")
    evidence["candidate_id"] = submitted["candidate_id"]
    evidence["record_hex"] = submitted["record_hex"]
    evidence["stored_hex"] = submitted["stored_hex"]
    evidence["decision"] = submitted["result"]["decision_tag"]
    evidence["failed_phase"] = submitted["result"]["failed_phase"]
    evidence["phase_count"] = submitted["result"]["phase_count"]
    evidence["result_id"] = submitted["result"]["candidate_result_id_hex"]
    return evidence


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sley", required=True, help="path to the built sley binary")
    parser.add_argument("--fixtures", default=str(FIXTURES))
    parser.add_argument("--receipt", default=str(DEFAULT_RECEIPT))
    parser.add_argument("--timeout-seconds", type=int, default=90)
    arguments = parser.parse_args()
    sley = Path(arguments.sley)
    problems: list[str] = []
    evidence: dict[str, Any] = {"contract": "at-mw-02-i4b-demonstrations-v1", "cases": {}}
    try:
        fixtures = json.loads(Path(arguments.fixtures).read_text(encoding="utf-8"))["vectors"]
    except (OSError, ValueError) as error:
        print(json.dumps({"result": "FAIL", "problems": [f"unreadable-fixtures:{error}"]}))
        return 1
    if not sley.is_file():
        print(json.dumps({"result": "FAIL", "problems": [f"missing binary {sley}"]}))
        return 1
    scratch = Path(tempfile.mkdtemp(prefix="i4b-demo-"))
    try:
        hello, _, _ = endpoint_offer(sley, arguments.timeout_seconds)
        handshake = probe_handshake(sley, hello, scratch, arguments.timeout_seconds, PROFILE_ARGS)
        lives: dict[str, Live] = {}
        for variant in ("a", "b"):
            vector = dict(fixtures[variant], variant=variant)
            live = Live(sley, scratch / variant, vector, arguments.timeout_seconds)
            live.seed(hello)
            lives[variant] = live
        try:
            for variant in ("a", "b"):
                for demo_id in ("expression", "signature"):
                    key = f"{variant}/{demo_id}"
                    try:
                        case = run_case(lives[variant], handshake, demo_id, fixtures[variant])
                    except DemoError as error:
                        problems.append(f"{key} harness: {error}")
                        continue
                    evidence["cases"][key] = case
                    session, _ = lives[variant].open_session(handshake)
                    if demo_id == "expression":
                        problems.extend(f"{key} judge: {item}" for item in
                                        judge_expression(lives[variant], session, case, fixtures[variant]))
                    else:
                        problems.extend(f"{key} judge: {item}" for item in
                                        judge_signature(lives[variant], session, case, fixtures[variant]))
                    replay = revalidate(lives[variant], session,
                                        bytes.fromhex(case["head_tx"]),
                                        bytes.fromhex(fixtures[variant]["principal"]),
                                        case["stored_hex"])
                    if not replay.get("valid"):
                        problems.append(f"{key} judge: recorded candidate does not re-validate: {replay}")
            for variant, other in (("a", "b"), ("b", "a")):
                for demo_id in ("expression", "signature"):
                    key = f"{variant}/{demo_id}"
                    if key not in evidence["cases"]:
                        continue
                    session, _ = lives[other].open_session(handshake)
                    replay = revalidate(lives[other], session,
                                        bytes.fromhex(evidence["cases"][key]["head_tx"]),
                                        bytes.fromhex(fixtures[variant]["principal"]),
                                        evidence["cases"][key]["stored_hex"])
                    evidence["cases"][key][f"cross_replay_on_{other}"] = replay.get("valid")
                    if replay.get("valid"):
                        problems.append(f"{key} judge: {variant} candidate validates on {other} (transferable)")
        finally:
            for live in lives.values():
                live.close()
    except DemoError as error:
        problems.append(f"harness: {error}")
    evidence["problems"] = problems
    evidence["result"] = "PASS" if not problems and len(evidence["cases"]) == 4 else "FAIL"
    receipt = Path(arguments.receipt)
    receipt.parent.mkdir(parents=True, exist_ok=True)
    receipt.write_text(json.dumps(evidence, indent=1, sort_keys=True) + "\n", encoding="utf-8")
    summary = {key: {"decision": case["decision"], "failed_phase": case["failed_phase"]}
               for key, case in evidence["cases"].items()}
    print(json.dumps({"contract": "at-mw-02-i4b-demonstrations-v1", "result": evidence["result"],
                      "cases": summary, "problems": problems,
                      "receipt": str(receipt)}, indent=1, sort_keys=True))
    return 0 if evidence["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
