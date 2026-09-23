#!/usr/bin/env python3
"""Full TYPE migration witness through the real trial surface (no model).

Corrected-fixture witness for S2B-TYPE-001 (manifest targets include the
switch blocks 6d/6e and the named `switch_param` role). Each variant
authors one JobState migration through the real tool surface
(propose/compose/finish) and runs the live judge; the log records the
provenance environment, the effective design, the judge's JSON verdict
(status/code/detail), and a self-check against the variant's expected
outcome.

Designs (every accept design is a complete, exhaustive migration):
- const (default positive): typedef Queued/Running/Succeeded unit +
  Failed(SInt64 code); status Failed(7); param 0x6c Bool -> JobState;
  switch 0x6b Bool -> SInt64 result, entry 0x6d CondBranch -> exhaustive
  VariantSwitch on the param; unit arms return SInt constants (0,1,2);
  the Failed arm binds the code (CasePayload) and returns it.
- uint: Failed(UInt64 code) and a UInt64 switch result (non-SInt).
- arith: Result<SInt64, ArithmeticError> switch result (non-SInt); every
  arm computes its value with checked addition; the Failed arm maps the
  code (code + 100) instead of returning it raw.
- join: every arm branches to a shared join block (not a case target)
  that returns its block parameter; the Failed arm forwards the code.

Variants (expected outcome in brackets):
  pos              const design                                [accepted]
  alt_code8        const, Failed(8)                            [accepted]
  alt_queued       const, status Queued (unit)                 [accepted]
  alt_shared       const, leaves 0,0,2 (shared value)          [accepted]
  alt_uint         uint design                                 [accepted]
  alt_arith        arith design                                [accepted]
  alt_join         join design                                 [accepted]
  alt_failed_fixed const, the Failed arm binds the code but maps
                   it to a fixed value (3)                     [accepted]
  neg_bool         status left Bool                            [ORACLE_BOOL_COMPAT_FIELD]
  neg_typedef_only typedef + Failed status, switch untouched   [ORACLE_BOOL_COMPAT_FIELD]
  neg_bool_const   const design + a fresh Bool constant        [ORACLE_BOOL_COMPAT_FIELD]
  neg_two_param    const design + a second Bool switch param   [ORACLE_BOOL_COMPAT_FIELD]
  neg_trap         const design, Failed arm traps              [ORACLE_TRAP_ARM]
  neg_droppayload  const design, Failed edge drops CasePayload [production refusal]
  neg_nullcode     const design, status Failed with null code  [production refusal]
  neg_dropcode     const design, Failed arm binds no code and
                   returns a constant (edge carries no payload) [ORACLE_FAILED_CODE]

Usage: succ_witness_type_full.py VARIANT [logfile]
Env: SLEY2_SLEY_BINARY, SUCC_JUDGE_TEST_BINARY (both required).
Legacy provenance overrides (applied on top of the variant): TYPE_CODE,
TYPE_STATUS (FAILED|QUEUED|RUNNING|SUCCEEDED), TYPE_LEAFS ("0,1,2"),
TYPE_NULLCODE=1, TYPE_DROPPAYLOAD=1. Production refusals
(neg_droppayload: phase 7 CFG_TARGET_ARGUMENTS; neg_nullcode: phase 6
TYPE_CONST_SHAPE) stop before the judge runs, so the judge's
null-payload ORACLE_FAILED_CODE path is a fail-closed backstop covered
by unit tests (bench/live/tests/test_judge_type_variant.py); the
judge's dropped-code path is reached live by neg_dropcode, which
production validation accepts.
"""

from __future__ import annotations

import contextlib
import datetime
import hashlib
import io
import json
import os
import platform
import subprocess
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

TASK_ID = "S2B-TYPE-001"
TASK_DIR = ROOT / "bench" / "fixtures" / "sley2" / TASK_ID
SINT = {"variant": "SInt", "value": 64}
UINT = {"variant": "UInt", "value": 64}
BOOL = {"variant": "Bool"}
ARITH = {"variant": "BuiltinFailure", "value": "ArithmeticError"}
RES_SINT = {"variant": "Result", "value": {"ok": SINT, "error": ARITH}}
CONST_REF, ADD = 1, 64
QUEUED = "51" * 32
RUNNING = "52" * 32
SUCCEEDED = "53" * 32
FAILED = "54" * 32
STATUS_IDS = {"FAILED": FAILED, "QUEUED": QUEUED, "RUNNING": RUNNING,
              "SUCCEEDED": SUCCEEDED}
ENTRY = "6d" * 32
LEAF = "6e" * 32

BASE_DESIGN = {"mode": "const", "code": 7, "status": "FAILED",
               "leafs": [0, 1, 2], "null_code": False,
               "drop_payload": False, "trap": False, "bool_const": False,
               "two_param": False, "failed_const": False}
VARIANTS = {
    "pos": ({}, "accepted"),
    "alt_code8": ({"code": 8}, "accepted"),
    "alt_queued": ({"status": "QUEUED"}, "accepted"),
    "alt_shared": ({"leafs": [0, 0, 2]}, "accepted"),
    "alt_uint": ({"mode": "uint"}, "accepted"),
    "alt_arith": ({"mode": "arith"}, "accepted"),
    "alt_join": ({"mode": "join"}, "accepted"),
    "alt_failed_fixed": ({"failed_const": True}, "accepted"),
    "neg_bool": ({"mode": "status_bool"}, "ORACLE_BOOL_COMPAT_FIELD"),
    "neg_typedef_only": ({"mode": "typedef_only"}, "ORACLE_BOOL_COMPAT_FIELD"),
    "neg_bool_const": ({"bool_const": True}, "ORACLE_BOOL_COMPAT_FIELD"),
    "neg_two_param": ({"two_param": True}, "ORACLE_BOOL_COMPAT_FIELD"),
    "neg_trap": ({"trap": True}, "ORACLE_TRAP_ARM"),
    "neg_droppayload": ({"drop_payload": True}, "production_refused"),
    "neg_nullcode": ({"null_code": True}, "production_refused"),
    "neg_dropcode": ({"drop_payload": True, "failed_const": True},
                     "ORACLE_FAILED_CODE"),
}


def op_create(kind: int, payload: dict) -> dict:
    return {"class": "CreateEntity", "kind": kind, "target": None,
            "field_tag": None, "payload": payload}


def op_replace(kind: int, target: str, payload: dict) -> dict:
    return {"class": "ReplaceEntityVersion", "kind": kind,
            "target": target, "field_tag": None, "payload": payload}


def named(typedef_id: str) -> dict:
    return {"variant": "Named",
            "value": {"definition": typedef_id, "arguments": []}}


def par(ref: str) -> dict:
    return {"variant": "Parameter", "value": ref}


def res(ref: str) -> dict:
    return {"variant": "OperationResult",
            "value": {"operation": ref, "result_index": 0}}


def typedef_payload(code_type: dict) -> dict:
    def case(member: str, payload: dict) -> dict:
        return {"member_id": member, "payload_type": payload}

    return {"type_parameters": [],
            "form": {"variant": "Variant", "value": [
                case(QUEUED, {"variant": "None"}),
                case(RUNNING, {"variant": "None"}),
                case(SUCCEEDED, {"variant": "None"}),
                case(FAILED, {"variant": "Some", "value": code_type})]},
            "invariants": [], "visibility": "Private"}


def int_const(value_type: dict, value: int) -> dict:
    return {"value_type": value_type,
            "data": {"variant": value_type["variant"], "value": value}}


def variant_const(typedef_id: str, member: str, payload: dict | None) -> dict:
    return {"value_type": named(typedef_id),
            "data": {"variant": "Variant", "value": {
                "definition": typedef_id, "member_id": member,
                "payload": ({"variant": "None"} if payload is None
                            else {"variant": "Some", "value": payload})}}}


def derive(workspace: str, nonce: str, kind: int, ordinal: int) -> str:
    [out] = sley2_codecs.run_batch([{
        "op": "derive_entity", "workspace": workspace,
        "nonce": nonce, "kind": kind, "ordinal": ordinal}])
    return out["entity"]


def slots_for(design: dict) -> list[tuple[str, int]]:
    """Fresh entities in create order (identity = (nonce, kind, create
    ordinal), so the order is part of the design)."""

    mode = design["mode"]
    if mode in ("status_bool", "typedef_only"):
        return [("typedef", 4)]
    slots = [("typedef", 4), ("A", 7), ("B", 7), ("C", 7)]
    if mode == "join":
        slots += [("J", 7), ("pC", 6), ("pJ", 6)]
    elif not (design["failed_const"] and design["drop_payload"]):
        slots += [("pC", 6)]
    slots += [("k0", 9), ("k1", 9), ("k2", 9)]
    if mode == "arith":
        slots += [("k100", 9)]
        slots += [(name, 8) for name in ("o6e", "o6e_add", "oA", "oA_add",
                                         "oB", "oB_add", "oC", "oC_add")]
    else:
        slots += [("o6e", 8), ("oA", 8), ("oB", 8)]
    if design["failed_const"]:
        slots += [("k3", 9), ("oC", 8)]
    if design["bool_const"]:
        slots.append(("kBool", 9))
    if design["two_param"]:
        slots.append(("pBool", 6))
    return slots


def build(design: dict, ids: dict[str, str], manifest: dict,
          status_body: dict) -> list:
    """The migration record ops for one design (creates first)."""

    status = manifest["entities"]["status"]
    switch = manifest["entities"]["switch"]
    param_id = manifest["entities"]["switch_param"]
    mode = design["mode"]
    code_type = UINT if mode == "uint" else SINT
    typedef_id = ids["typedef"]
    if mode == "status_bool":
        return [op_create(4, typedef_payload(code_type)),
                op_replace(9, status, {"value": status_body["value"]})]
    status_id = STATUS_IDS[design["status"]]
    if design["null_code"]:
        status_value = variant_const(typedef_id, FAILED, None)
    elif status_id == FAILED:
        status_value = variant_const(typedef_id, FAILED,
                                     int_const(code_type, design["code"]))
    else:
        status_value = variant_const(typedef_id, status_id, None)
    if mode == "typedef_only":
        return [op_create(4, typedef_payload(code_type)),
                op_replace(9, status, {"value": status_value})]
    result_type = {"uint": UINT, "arith": RES_SINT}.get(mode, SINT)
    leaf_type = UINT if mode == "uint" else SINT
    unit_leaves = [(LEAF, "o6e", "k0"), (ids["A"], "oA", "k1"),
                   (ids["B"], "oB", "k2")]

    def op(block: str, ordinal: int, opcode: int, operands: list,
           result: dict, immediate: dict | None) -> dict:
        return {"block": block, "ordinal": ordinal, "opcode": opcode,
                "operands": operands, "result_types": [result],
                "immediate": immediate or {"variant": "None"}}

    def const_ref(block: str, ordinal: int, const: str) -> dict:
        return op(block, ordinal, CONST_REF, [], leaf_type,
                  {"variant": "Entity", "value": ids[const]})

    def block(params: list, ops: list, terminator: dict) -> dict:
        return {"function": switch, "parameters": params, "operations": ops,
                "terminator": terminator, "reachability": "Required"}

    def ret(value: dict) -> dict:
        return {"variant": "Return", "value": {"value": value}}

    def branch(target: str, value: dict) -> dict:
        return {"variant": "Branch", "value": {"edge": {
            "target": target, "arguments": [value]}}}

    blocks: dict[str, dict] = {}
    ops: dict[str, dict] = {}
    for block_id, op_name, const in unit_leaves:
        if mode == "arith":
            ops[op_name] = const_ref(block_id, 0, const)
            ops[op_name + "_add"] = op(block_id, 1, ADD,
                                       [res(ids[op_name]), res(ids[op_name])],
                                       RES_SINT, None)
            blocks[block_id] = block([], [ids[op_name], ids[op_name + "_add"]],
                                     ret(res(ids[op_name + "_add"])))
        elif mode == "join":
            ops[op_name] = const_ref(block_id, 0, const)
            blocks[block_id] = block([], [ids[op_name]],
                                     branch(ids["J"], res(ids[op_name])))
        else:
            ops[op_name] = const_ref(block_id, 0, const)
            blocks[block_id] = block([], [ids[op_name]], ret(res(ids[op_name])))
    failed_block = ids["C"]
    if design["trap"]:
        blocks[failed_block] = block([ids["pC"]], [], {
            "variant": "Trap", "value": {"code": "Unreachable",
                                         "payload": {"variant": "None"}}})
    elif design["failed_const"]:
        ops["oC"] = const_ref(failed_block, 0, "k3")
        binds = [] if design["drop_payload"] else [ids["pC"]]
        blocks[failed_block] = block(binds, [ids["oC"]], ret(res(ids["oC"])))
    elif mode == "arith":
        ops["oC"] = const_ref(failed_block, 0, "k100")
        ops["oC_add"] = op(failed_block, 1, ADD, [par(ids["pC"]), res(ids["oC"])],
                           RES_SINT, None)
        blocks[failed_block] = block([ids["pC"]], [ids["oC"], ids["oC_add"]],
                                     ret(res(ids["oC_add"])))
    elif mode == "join":
        blocks[failed_block] = block([ids["pC"]], [],
                                     branch(ids["J"], par(ids["pC"])))
        blocks[ids["J"]] = block([ids["pJ"]], [], ret(par(ids["pJ"])))
    else:
        blocks[failed_block] = block([ids["pC"]], [], ret(par(ids["pC"])))
    cases = [
        {"case_key": {"variant": "Member", "value": QUEUED},
         "edge": {"target": LEAF, "arguments": []}},
        {"case_key": {"variant": "Member", "value": RUNNING},
         "edge": {"target": ids["A"], "arguments": []}},
        {"case_key": {"variant": "Member", "value": SUCCEEDED},
         "edge": {"target": ids["B"], "arguments": []}},
        {"case_key": {"variant": "Member", "value": FAILED},
         "edge": {"target": failed_block,
                  "arguments": ([] if design["drop_payload"]
                                else [{"variant": "CasePayload"}])}},
    ]
    entry = block([], [], {"variant": "VariantSwitch", "value": {
        "value": par(param_id), "cases": cases}})
    func_blocks = [ENTRY, LEAF, ids["A"], ids["B"], ids["C"]]
    if mode == "join":
        func_blocks.append(ids["J"])
    func_params = [param_id] + ([ids["pBool"]] if design["two_param"] else [])
    func = {"type_parameters": [], "parameters": func_params,
            "result_type": result_type, "effects": [], "entry_block": ENTRY,
            "blocks": func_blocks, "contracts": [], "visibility": "Private"}
    leaf_values = design["leafs"]
    payloads: dict[str, dict] = {
        "typedef": typedef_payload(code_type),
        "A": blocks[ids["A"]], "B": blocks[ids["B"]], "C": blocks[ids["C"]],
        "pC": {"owner": ids["C"], "role": "Block", "ordinal": 0,
               "value_type": code_type},
        "k0": {"value": int_const(leaf_type, leaf_values[0])},
        "k1": {"value": int_const(leaf_type, leaf_values[1])},
        "k2": {"value": int_const(leaf_type, leaf_values[2])},
        "k3": {"value": int_const(leaf_type, 3)},
        "k100": {"value": int_const(SINT, 100)},
        "kBool": {"value": {"value_type": BOOL,
                            "data": {"variant": "Bool", "value": True}}},
        "pBool": {"owner": switch, "role": "Function", "ordinal": 1,
                  "value_type": BOOL},
    }
    if mode == "join":
        payloads["J"] = blocks[ids["J"]]
        payloads["pJ"] = {"owner": ids["J"], "role": "Block", "ordinal": 0,
                          "value_type": SINT}
    payloads.update(ops)
    record = [op_create(kind, payloads[name]) for name, kind in slots_for(design)]
    record += [
        op_replace(9, status, {"value": status_value}),
        op_replace(6, param_id, {"owner": switch, "role": "Function",
                                 "ordinal": 0, "value_type": named(typedef_id)}),
        op_replace(5, switch, func),
        op_replace(7, ENTRY, entry),
        op_replace(7, LEAF, blocks[LEAF]),
    ]
    return record


def _sha256(path: str | Path) -> str:
    try:
        return hashlib.sha256(Path(path).read_bytes()).hexdigest()
    except OSError:
        return "unreadable"


def provenance(variant: str, design: dict, expect: str) -> dict:
    def git(*args: str) -> str:
        try:
            return subprocess.run(["git", "-C", str(ROOT), *args], check=False,
                                  capture_output=True, text=True).stdout.strip()
        except OSError:
            return "unavailable"

    return {
        "variant": variant, "expect": expect, "design": design,
        "utc": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "git_head": git("rev-parse", "HEAD"),
        "git_dirty_paths": git("status", "--porcelain").splitlines(),
        "python": platform.python_version(),
        "SLEY2_SLEY_BINARY": os.environ.get("SLEY2_SLEY_BINARY", ""),
        "sley_sha256": _sha256(os.environ.get("SLEY2_SLEY_BINARY", "")),
        "SUCC_JUDGE_TEST_BINARY": os.environ.get("SUCC_JUDGE_TEST_BINARY", ""),
        "driver_sha256": _sha256(os.environ.get("SUCC_JUDGE_TEST_BINARY", "")),
        "manifest_sha256": _sha256(TASK_DIR / "task_manifest.json"),
        "base_pack_sha256": _sha256(TASK_DIR / "base.pack"),
        "judge_sha256": _sha256(ROOT / "bench" / "fixtures" / "sley2_live_judge.py"),
        "env_overrides": {key: os.environ[key] for key in (
            "TYPE_CODE", "TYPE_STATUS", "TYPE_LEAFS", "TYPE_NULLCODE",
            "TYPE_DROPPAYLOAD") if key in os.environ},
    }


def refusal_tokens(reply: dict) -> list[str]:
    """Closed refusal codes (ASCII tokens) inside the refused record's
    decision body, so the log names the production refusal."""

    import re

    body = (reply.get("report") or {}).get("body", "")
    try:
        raw = bytes.fromhex(body) if isinstance(body, str) else b""
    except ValueError:
        raw = b""
    return sorted({t.decode() for t in re.findall(rb"[A-Z][A-Z0-9_]{5,}", raw)
                   if b"_" in t})


def design_for(variant: str) -> tuple[dict, str]:
    if variant not in VARIANTS:
        raise SystemExit(f"unknown variant {variant!r}; one of {sorted(VARIANTS)}")
    overrides, expect = VARIANTS[variant]
    design = dict(BASE_DESIGN)
    design.update(overrides)
    if "TYPE_CODE" in os.environ:
        design["code"] = int(os.environ["TYPE_CODE"])
    if "TYPE_STATUS" in os.environ:
        design["status"] = os.environ["TYPE_STATUS"]
    if "TYPE_LEAFS" in os.environ:
        leafs = [int(v) for v in os.environ["TYPE_LEAFS"].split(",")]
        if len(leafs) != 3:
            raise SystemExit("TYPE_LEAFS needs three values")
        design["leafs"] = leafs
    if os.environ.get("TYPE_NULLCODE") == "1":
        design["null_code"] = True
    if os.environ.get("TYPE_DROPPAYLOAD") == "1":
        design["drop_payload"] = True
    if design["status"] not in STATUS_IDS:
        raise SystemExit(f"TYPE_STATUS {design['status']!r} unknown")
    return design, expect


def main() -> int:
    variant = sys.argv[1] if len(sys.argv) > 1 else "pos"
    log_path = Path(sys.argv[2]) if len(sys.argv) > 2 else None
    for key in ("SLEY2_SLEY_BINARY", "SUCC_JUDGE_TEST_BINARY"):
        if not os.environ.get(key):
            raise SystemExit(f"missing env {key}")
    design, expect = design_for(variant)
    lines: list[str] = []

    def emit(text: str) -> None:
        lines.append(text)
        print(text, flush=True)

    emit("provenance " + json.dumps(provenance(variant, design, expect),
                                    sort_keys=True))
    outcome = "unknown"
    tmp = tempfile.mkdtemp(prefix="sley2-type-full-")
    ws = Path(tmp) / "ws"
    stage_initial("sley_2_0", TASK_ID, ws)
    stage_tooling("sley_2_0", ws)
    manifest = json.loads((TASK_DIR / "task_manifest.json").read_text())
    saved_cwd = os.getcwd()
    os.chdir(ws)
    try:
        def run(*argv: str) -> tuple[int, dict]:
            out = io.StringIO()
            with mock.patch.object(sley2_tool.sys, "stdout", out):
                code = sley2_tool.main(list(argv))
            return code, json.loads(out.getvalue())

        # Round 1: skeleton typedef establishes the record nonce the
        # fresh identities derive from.
        _, rep1 = run("propose", json.dumps([op_create(4, typedef_payload(SINT))]))
        report1 = rep1.get("report", {})
        emit(f"TYPE-FULL witness/{variant}: skeleton valid {report1.get('valid')}")
        if not report1.get("valid"):
            emit(f"skeleton detail {json.dumps(rep1)[:800]}")
            return 2
        base_record = report1["record"]
        [described] = sley2_codecs.run_batch(
            [{"op": "describe_record", "record": base_record}])
        nonce = described["nonce"]
        slots = slots_for(design)
        ids = {name: derive(manifest["workspace"], nonce, kind, ordinal)
               for ordinal, (name, kind) in enumerate(slots)}
        emit(f"TYPE-FULL fresh ids " + " ".join(
            f"{name}={ids[name][:8]}" for name, _ in slots))
        _, rep_s = run("read", manifest["entities"]["status"])
        status_body = rep_s["report"]["decoded"]["entries"][0]["body"]
        record = build(design, ids, manifest, status_body)
        verb = "propose" if design["mode"] == "status_bool" else "compose"
        args = ([json.dumps(record)] if verb == "propose"
                else [base_record, json.dumps(record)])
        _, rep2 = run(verb, *args)
        report2 = rep2.get("report", {})
        emit(f"TYPE-FULL witness/{variant}: {verb} valid {report2.get('valid')} "
             f"decision {report2.get('decision')}")
        if not report2.get("valid"):
            emit(f"{verb} refusal tokens {refusal_tokens(rep2)}")
            outcome = "production_refused"
        else:
            _, rep3 = run("finish", report2["record"])
            emit(f"TYPE-FULL witness/{variant}: finished "
                 f"{rep3.get('report', {}).get('finished')}")
            saved_argv = sys.argv
            sys.argv = ["sley2_live_judge", str(ws)]
            verdict_out = io.StringIO()
            try:
                with contextlib.redirect_stdout(verdict_out):
                    exit_code = judge.main(TASK_ID)
            finally:
                sys.argv = saved_argv
            verdict_text = verdict_out.getvalue().strip()
            emit(f"judge verdict: {verdict_text}")
            emit(f"{TASK_ID} witness/{variant} judge exit: {exit_code}")
            try:
                verdict = json.loads(verdict_text.splitlines()[-1])
            except (ValueError, IndexError):
                verdict = {}
            outcome = ("accepted" if verdict.get("status") == "accepted"
                       else str(verdict.get("code") or verdict.get("status")))
        emit(f"workspace kept at: {ws}")
    finally:
        os.chdir(saved_cwd)
    ok = outcome in expect.split("|")
    emit(f"outcome {outcome} expect {expect} -> {'PASS' if ok else 'FAIL'}")
    if log_path is not None:
        log_path.parent.mkdir(parents=True, exist_ok=True)
        if log_path.exists():
            raise SystemExit(f"refusing to overwrite {log_path}")
        log_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
