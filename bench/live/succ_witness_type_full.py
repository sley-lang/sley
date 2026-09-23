#!/usr/bin/env python3
"""Full TYPE migration witness through the real trial surface (no model).

Corrected-fixture witness for S2B-TYPE-001 v2 (targets include 6d/6e).
Performs the complete JobState migration and runs the frozen live judge:
- JobState typedef (Queued/Running/Succeeded unit + Failed(SInt)).
- Status migrates to Failed(7) (explicit code, deterministic).
- Param 0x6c migrates Bool -> Named(JobState).
- Function 0x6b result Bool -> SInt(64), blocks [6d,6e,newA,newB,newC].
- Entry 0x6d CondBranch -> exhaustive VariantSwitch on param (4 sorted
  Member cases; Failed forwards CasePayload).
- Leaf 0x6e + 2 new leaves return distinct SInt constants (0,1,2);
  Failed leaf returns its Block param (payload).
- All blocks Required, no Trap, every block is entry or a case target.

Usage: succ_witness_type_full.py [pos|neg_bool|neg_trap|neg_typedef_only] [logfile]
Env: SLEY2_SLEY_BINARY, SUCC_JUDGE_TEST_BINARY (both required).
Provenance params: TYPE_CODE (default 7), TYPE_STATUS
(FAILED|QUEUED|RUNNING|SUCCEEDED, default FAILED), TYPE_LEAFS
("0,1,2" default), TYPE_NULLCODE=1 (Failed member, null payload:
production validation refuses), TYPE_DROPPAYLOAD=1 (Failed arm drops
CasePayload: judge ORACLE_FAILED_CODE).
"""

from __future__ import annotations

import io
import json
import os
import sys
import tempfile
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from bench.fixtures import sley2_live_judge as judge  # noqa: E402
from bench.live import sley2_codecs, sley2_tool  # noqa: E402
from bench.live.taskpacks import stage_initial  # noqa: E402
from bench.live.tooling import stage_tooling  # noqa: E402
from bench.live.witness_provenance import source_identity  # noqa: E402

SINT = {"variant": "SInt", "value": 64}
QUEUED = "51" * 32
RUNNING = "52" * 32
SUCCEEDED = "53" * 32
FAILED = "54" * 32
# Provenance parameters (env-overridable; defaults reproduce the
# original positive: Failed(7), distinct leaves 0/1/2):
# - TYPE_CODE: explicit Failed SInt code (any int; corpus pins none).
# - TYPE_STATUS: status value member (FAILED/QUEUED/RUNNING/SUCCEEDED;
#   corpus pins no status value).
# - TYPE_LEAFS: comma-separated SInt leaf constants for the Queued /
#   Running / Succeeded arms (deterministic; distinctness retired).
# - TYPE_NULLCODE=1: status Failed member with null payload
#   (wrong-code negative: forbidden null error).
FAILED_CODE = int(os.environ.get("TYPE_CODE", "7"))
STATUS_MEMBER = os.environ.get("TYPE_STATUS", "FAILED")
LEAF_VALUES = {QUEUED: 0, RUNNING: 1, SUCCEEDED: 2}
try:
    _leaf_env = [int(v) for v in os.environ.get("TYPE_LEAFS", "0,1,2").split(",")]
    if len(_leaf_env) == 3:
        LEAF_VALUES = {QUEUED: _leaf_env[0], RUNNING: _leaf_env[1],
                       SUCCEEDED: _leaf_env[2]}
except ValueError:
    pass
NULL_CODE = os.environ.get("TYPE_NULLCODE", "") == "1"
DROP_PAYLOAD = os.environ.get("TYPE_DROPPAYLOAD", "") == "1"
STATUS_IDS = {"FAILED": FAILED, "QUEUED": QUEUED, "RUNNING": RUNNING,
              "SUCCEEDED": SUCCEEDED}


def op_create(kind: int, payload: dict) -> dict:
    return {"class": "CreateEntity", "kind": kind, "target": None,
            "field_tag": None, "payload": payload}


def op_replace(kind: int, target: str, payload: dict) -> dict:
    return {"class": "ReplaceEntityVersion", "kind": kind,
            "target": target, "field_tag": None, "payload": payload}


def named(typedef_id: str) -> dict:
    return {"variant": "Named",
            "value": {"definition": typedef_id, "arguments": []}}


def typedef_payload() -> dict:
    def case(member: str, payload: dict) -> dict:
        return {"member_id": member, "payload_type": payload}

    return {"type_parameters": [],
            "form": {"variant": "Variant", "value": [
                case(QUEUED, {"variant": "None"}),
                case(RUNNING, {"variant": "None"}),
                case(SUCCEEDED, {"variant": "None"}),
                case(FAILED, {"variant": "Some", "value": SINT})]},
            "invariants": [], "visibility": "Private"}


def variant_const(typedef_id: str, member: str,
                  payload: dict | None) -> dict:
    return {"value_type": named(typedef_id),
            "data": {"variant": "Variant", "value": {
                "definition": typedef_id, "member_id": member,
                "payload": ({"variant": "None"} if payload is None
                            else {"variant": "Some", "value": payload})}}}


def sint_const(value: int) -> dict:
    return {"value_type": SINT,
            "data": {"variant": "SInt", "value": value}}


def derive(workspace: str, nonce: str, kind: int, ordinal: int) -> str:
    [out] = sley2_codecs.run_batch([{
        "op": "derive_entity", "workspace": workspace,
        "nonce": nonce, "kind": kind, "ordinal": ordinal}])
    return out["entity"]


def main() -> int:
    variant = sys.argv[1] if len(sys.argv) > 1 else "pos"
    log_path = Path(sys.argv[2]) if len(sys.argv) > 2 else None
    for key in ("SLEY2_SLEY_BINARY", "SUCC_JUDGE_TEST_BINARY"):
        if not os.environ.get(key):
            raise SystemExit(f"missing env {key}")
    lines: list[str] = []

    def emit(text: str) -> None:
        lines.append(text)
        print(text, flush=True)

    emit(f"TYPE-FULL witness/{variant}: {source_identity(ROOT)}")
    tmp = tempfile.mkdtemp(prefix="sley2-type-full-")
    ws = Path(tmp) / "ws"
    stage_initial("sley_2_0", "S2B-TYPE-001", ws)
    stage_tooling("sley_2_0", ws)
    manifest = json.loads((ROOT / "bench" / "fixtures" / "sley2"
                           / "S2B-TYPE-001" / "task_manifest.json"
                           ).read_text())
    status = manifest["entities"]["status"]
    switch = manifest["entities"]["switch"]
    saved_cwd = os.getcwd()
    os.chdir(ws)
    try:
        def run(*argv: str) -> tuple[int, dict]:
            out = io.StringIO()
            with mock.patch.object(sley2_tool.sys, "stdout", out):
                code = sley2_tool.main(list(argv))
            return code, json.loads(out.getvalue())

        # Phase 1: skeleton typedef to establish the record nonce.
        skel = [op_create(4, typedef_payload())]
        code, rep1 = run("propose", json.dumps(skel))
        report1 = rep1.get("report", {})
        emit(f"TYPE-FULL witness/{variant}: skeleton valid {report1.get('valid')}")
        if not report1.get("valid"):
            emit(f"skeleton detail {json.dumps(rep1)[:800]}")
            return 2
        base_record = report1["record"]
        [described] = sley2_codecs.run_batch(
            [{"op": "describe_record", "record": base_record}])
        nonce = described["nonce"]
        # Workspace from live head (0707... frozen trial workspace).
        # Head read through the agent's own opener, then revision.read of
        # the tx it reported (no harness-supplied head id).
        _, rep_open = run("open")
        _, rep_rev = run("revision", rep_open["report"]["decoded"]["tx"])
        # Workspace id is in manifest (frozen); use it for derivation.
        workspace = manifest["workspace"]
        # Create order (11 creates): typedef, newA, newB, newC block,
        # block-param, const0/1/2, op_6e/opA/opB. Ordinals 0..10.
        create_kinds = [4, 7, 7, 7, 6, 9, 9, 9, 8, 8, 8]
        ids = [derive(workspace, nonce, k, i)
               for i, k in enumerate(create_kinds)]
        (typedef_id, new_a, new_b, new_c, blk_param,
         const0, const1, const2, op_6e, op_a, op_b) = ids
        emit(f"TYPE-FULL typedef {typedef_id[:16]} newA {new_a[:8]} "
             f"newB {new_b[:8]} newC {new_c[:8]}")

        # Fresh payloads referencing derived ids.
        def block_param_payload() -> dict:
            return {"owner": new_c, "role": "Block", "ordinal": 0,
                    "value_type": SINT}

        def const_payload(v: int) -> dict:
            return {"value": sint_const(v)}

        def op_payload(block: str, const: str) -> dict:
            return {"block": block, "ordinal": 0, "opcode": 1,
                    "operands": [], "result_types": [SINT],
                    "immediate": {"variant": "Entity", "value": const}}

        def leaf_block(block: str, op: str) -> dict:
            return {"function": switch, "parameters": [],
                    "operations": [op],
                    "terminator": {"variant": "Return", "value": {
                        "value": {"variant": "OperationResult",
                                  "value": {"operation": op,
                                            "result_index": 0}}}},
                    "reachability": "Required"}

        # Entry VariantSwitch cases (sorted Member keys).
        cases = [
            {"case_key": {"variant": "Member", "value": QUEUED},
             "edge": {"target": "6e" * 32, "arguments": []}},
            {"case_key": {"variant": "Member", "value": RUNNING},
             "edge": {"target": new_a, "arguments": []}},
            {"case_key": {"variant": "Member", "value": SUCCEEDED},
             "edge": {"target": new_b, "arguments": []}},
            {"case_key": {"variant": "Member", "value": FAILED},
             "edge": {"target": new_c,
                      "arguments": ([] if DROP_PAYLOAD
                                    else [{"variant": "CasePayload"}])}},
        ]
        entry_payload = {
            "function": switch, "parameters": [], "operations": [],
            "terminator": {"variant": "VariantSwitch", "value": {
                "value": {"variant": "Parameter", "value": manifest_targets_param(manifest)},
                "cases": cases}},
            "reachability": "Required"}
        # Note: switch param id is the manifest 6c target; resolve below.
        param_id = manifest_param(manifest)
        entry_payload["terminator"]["value"]["value"] = {
            "variant": "Parameter", "value": param_id}
        failed_leaf = {
            "function": switch, "parameters": [blk_param],
            "operations": [],
            "terminator": {"variant": "Return", "value": {
                "value": {"variant": "Parameter", "value": blk_param}}},
            "reachability": "Required"}
        func_payload = {
            "type_parameters": [], "parameters": [param_id],
            "result_type": SINT, "effects": [],
            "entry_block": "6d" * 32,
            "blocks": ["6d" * 32, "6e" * 32, new_a, new_b, new_c],
            "contracts": [], "visibility": "Private"}
        param_payload = {
            "owner": switch, "role": "Function", "ordinal": 0,
            "value_type": named(typedef_id)}

        if variant == "neg_bool":
            # Leave status Bool (parallel compat path).
            _, rep_s = run("read", status)
            entry_body = rep_s["report"]["decoded"]["entries"][0]["body"]
            status_payload = {"value": entry_body["value"]}
            # Still do full switch? No — minimal Bool violation suffices.
            full = [op_create(4, typedef_payload()),
                    op_replace(9, status, status_payload)]
            # Pad creates to keep nonce derivation? No, fresh record.
            code, rep2 = run("propose", json.dumps(full))
            report2 = rep2.get("report", {})
            emit(f"TYPE-FULL neg_bool propose valid {report2.get('valid')}")
            if not report2.get("valid"):
                return 2
            code, rep3 = run("finish", report2["record"])
            emit(f"TYPE-FULL neg_bool finished {rep3.get('report', {}).get('finished')}")
        elif variant == "neg_trap":
            # Full migration but Failed leaf is a Trap (forbidden).
            trap_leaf = {
                "function": switch, "parameters": [],
                "operations": [],
                "terminator": {"variant": "Trap", "value": {
                    "code": "Unreachable",
                    "payload": {"variant": "None"}}},
                "reachability": "Required"}
            full = [
                op_create(4, typedef_payload()),
                op_create(7, leaf_block(new_a, op_a)),
                op_create(7, leaf_block(new_b, op_b)),
                op_create(7, trap_leaf),
                op_create(6, block_param_payload()),
                op_create(9, const_payload(0)),
                op_create(9, const_payload(1)),
                op_create(9, const_payload(2)),
                op_create(8, op_payload("6e" * 32, const0)),
                op_create(8, op_payload(new_a, const1)),
                op_create(8, op_payload(new_b, const2)),
                op_replace(9, status, {"value": variant_const(
                    typedef_id, FAILED, sint_const(FAILED_CODE))}),
                op_replace(6, param_id, param_payload),
                op_replace(5, switch, func_payload),
                op_replace(7, "6d" * 32, entry_payload),
                op_replace(7, "6e" * 32, leaf_block("6e" * 32, op_6e)),
            ]
            # Fix ids: creates derive in list order; our precomputed ids
            # assumed order typedef,newA,newB,newC,param,consts,ops.
            # Here newC payload differs (trap) but id same (derivation
            # is payload-independent). Block-param owner still new_c.
            code, rep2 = run("compose", base_record, json.dumps(full))
            report2 = rep2.get("report", {})
            emit(f"TYPE-FULL neg_trap compose valid {report2.get('valid')} "
                 f"decision {report2.get('decision')}")
            if not report2.get("valid"):
                emit(f"compose detail {json.dumps(rep2)[:800]}")
                return 2
            code, rep3 = run("finish", report2["record"])
            emit(f"TYPE-FULL neg_trap finished {rep3.get('report', {}).get('finished')}")
        elif variant == "neg_typedef_only":
            # Typedef + Failed status only (no switch change).
            full = [op_create(4, typedef_payload()),
                    op_replace(9, status, {"value": variant_const(
                        typedef_id, FAILED, sint_const(FAILED_CODE))})]
            code, rep2 = run("compose", base_record, json.dumps(full))
            report2 = rep2.get("report", {})
            emit(f"TYPE-FULL neg_typedef_only compose valid {report2.get('valid')}")
            if not report2.get("valid"):
                return 2
            code, rep3 = run("finish", report2["record"])
            emit(f"TYPE-FULL neg_typedef_only finished {rep3.get('report', {}).get('finished')}")
        else:
            status_id = STATUS_IDS.get(STATUS_MEMBER, FAILED)
            if NULL_CODE:
                status_payload = {"value": variant_const(
                    typedef_id, FAILED, None)}
            elif status_id == FAILED:
                status_payload = {"value": variant_const(
                    typedef_id, FAILED, sint_const(FAILED_CODE))}
            else:
                status_payload = {"value": variant_const(
                    typedef_id, status_id, None)}
            leaf_consts = [LEAF_VALUES[QUEUED], LEAF_VALUES[RUNNING],
                             LEAF_VALUES[SUCCEEDED]]
            full = [
                op_create(4, typedef_payload()),
                op_create(7, leaf_block(new_a, op_a)),
                op_create(7, leaf_block(new_b, op_b)),
                op_create(7, failed_leaf),
                op_create(6, block_param_payload()),
                op_create(9, const_payload(leaf_consts[0])),
                op_create(9, const_payload(leaf_consts[1])),
                op_create(9, const_payload(leaf_consts[2])),
                op_create(8, op_payload("6e" * 32, const0)),
                op_create(8, op_payload(new_a, const1)),
                op_create(8, op_payload(new_b, const2)),
                op_replace(9, status, status_payload),
                op_replace(6, param_id, param_payload),
                op_replace(5, switch, func_payload),
                op_replace(7, "6d" * 32, entry_payload),
                op_replace(7, "6e" * 32, leaf_block("6e" * 32, op_6e)),
            ]
            code, rep2 = run("compose", base_record, json.dumps(full))
            report2 = rep2.get("report", {})
            emit(f"TYPE-FULL witness/{variant}: compose valid {report2.get('valid')} "
                 f"decision {report2.get('decision')}")
            if not report2.get("valid"):
                emit(f"compose detail {json.dumps(rep2)[:2000]}")
                return 2
            code, rep3 = run("finish", report2["record"])
            emit(f"TYPE-FULL witness/{variant}: finished "
                 f"{rep3.get('report', {}).get('finished')}")
        saved_argv = sys.argv
        sys.argv = ["sley2_live_judge", str(ws)]
        try:
            exit_code = judge.main("S2B-TYPE-001")
        finally:
            sys.argv = saved_argv
        emit(f"S2B-TYPE-001 witness/{variant} judge exit: {exit_code}")
        emit(f"workspace kept at: {ws}")
    finally:
        os.chdir(saved_cwd)
    if log_path is not None:
        log_path.parent.mkdir(parents=True, exist_ok=True)
        log_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return 0


def manifest_param(manifest: dict) -> str:
    # Corrected manifest exposes param via targets (6c); original named
    # only switch/status. Resolve 6c from targets by position.
    targets = manifest.get("targets", [])
    for candidate in targets:
        if isinstance(candidate, str) and candidate.startswith("6c"):
            return candidate
    # Fallback: live function body's sole parameter.
    return "6c" * 32


def manifest_targets_param(manifest: dict) -> str:
    return manifest_param(manifest)


if __name__ == "__main__":
    raise SystemExit(main())
