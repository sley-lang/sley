#!/usr/bin/env python3
"""Deterministic DEAD witness through the real trial surface (no model).

S2B-DEAD-001: live function 89 lists an explicitly unreachable block 90
(one ConstantRef op 93) beside its live block 8a; private helper 8e
(parameter 91, block 8f) is unreferenced. The corpus fix removes the
unreachable block and the unused helper without changing reachable
behavior, public identities, tests or effects.

Every variant proposes through the real tool (`propose` = candidate.create
+ candidate.validate), finishes through `finish` when valid, and runs the
frozen live judge (scratch copy, production commit, native execution).
Variants (regression spec in bench/live/DEAD-TOMBSTONE-PROPOSAL.md,
landed by REQ-11):

  pos                    namespace drops 8e, function 89 drops block 90;
                         delete 90, 93, 8e, 8f, 91 (must accept)
  probe_survivor_test    pos + one TestCase targeting survivor 89: the
                         validation result must select it (non-empty
                         survivor selection, helper tombstoned). The frozen
                         pack carries no TestCase, so this probe adds one;
                         a candidate with a selected test then meets the
                         pre-existing commit gate TXN_TEST_EVIDENCE_UNSUPPORTED
                         (the CREATE co-commit gate), recorded verbatim
  neg_orphan93           pos without deleting op 93 (must be refused)
  neg_orphan93_bypass    the same candidate written to final_candidate.hex
                         directly (finish bypassed): the judge's own
                         production commit must refuse it
  neg_test_on_tombstone  pos + a TestCase targeting the deleted helper 8e
                         (must be refused: a tombstone cannot be tested)
  neg_helper_kept        drop block 90 + op 93 only; helper 8e kept
                         (must reject ORACLE_UNEXPECTED_ENTITY)
  neg_reachable_changed  dead code kept, live comparison 98 -> 100
                         (must reject ORACLE_REACHABLE_CHANGED)
  neg_public_deleted     live function 89 deleted with its structure
                         (must reject ORACLE_PUBLIC_DELETED)
  neg_extra_delete       pos + deleting the unowned constant 92 (dropped
                         from the namespace): ownership cascade must not
                         cover it (must reject ORACLE_COLLATERAL_TOUCHED)

The log records provenance (git sha, dirty count, binary sha256), the
decoded validation result (decision, failed phase, diagnostic source
symbols, affected closure, selected tests) and the judge's JSON verdict
verbatim. Result decoding is display only (pinned oracle codec over uv);
the judge never reads it.

Usage: succ_witness_dead.py VARIANT LOGFILE
Env: SLEY2_SLEY_BINARY, SUCC_JUDGE_TEST_BINARY (both required).
"""

from __future__ import annotations

import contextlib
import datetime
import hashlib
import io
import json
import os
import re
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

TASK_ID = "S2B-DEAD-001"
TASK_DIR = ROOT / "bench" / "fixtures" / "sley2" / TASK_ID
VARIANTS = ("pos", "probe_survivor_test", "neg_orphan93", "neg_orphan93_bypass",
            "neg_test_on_tombstone", "neg_helper_kept", "neg_reachable_changed",
            "neg_public_deleted", "neg_extra_delete")
KINDS = {"namespace": 3, "function": 5, "parameter": 6, "block": 7,
         "operation": 8, "constant": 9, "testcase": 14}
BOOL = {"variant": "Bool"}
SINT = {"variant": "SInt", "value": 64}

# Display-only decoder over the pinned oracle codec (same project the
# tool's codec service runs): decision, phase, diagnostic symbols and the
# result's entity sets.
_RESULT_DECODER = r"""
import json, sys
from sley2_scb1_oracle import candidate_result as R
from sley2_scb1_oracle.codec import Cursor, decode_uvar, read_sized
data = bytes.fromhex(sys.stdin.read().strip())
summary = R.decode_candidate_result(data)
cursor = Cursor(data[:-32])
cursor.read(len(R.MAGIC), "SCB_MAGIC_INVALID")
decode_uvar(cursor)
fields = R._record(read_sized(cursor, R.MAX_STORED_BYTES), 13)
summary["diagnostics"] = [
    {k: (v.hex() if isinstance(v, bytes) else v) for k, v in R._diagnostic(p).items()}
    for p in R._list(fields[7], R.MAX_DIAGNOSTICS)]
summary["affected_closure"] = [e.hex() for e in R._entity_set(fields[8])]
summary["required_capabilities"] = [e.hex() for e in R._entity_set(fields[9])]
summary["selected_tests"] = [e.hex() for e in R._entity_set(fields[10])]
print(json.dumps(summary, sort_keys=True))
"""


def decode_result(body_hex: str) -> dict:
    completed = subprocess.run(
        [sley2_codecs._uv(), "run", "--offline", "--frozen", "--project",
         str(sley2_codecs.ORACLE_PROJECT), "python", "-c", _RESULT_DECODER],
        input=body_hex.encode(), capture_output=True, timeout=120, check=False)
    if completed.returncode != 0:
        return {"decode_error": completed.stderr.decode("utf-8", "replace")[-300:]}
    return json.loads(completed.stdout.decode("utf-8"))


def failure_symbols(body_hex: str) -> list[str]:
    """Symbol-shaped ASCII runs inside a protocol failure body (display)."""

    try:
        raw = bytes.fromhex(body_hex)
    except ValueError:
        return []
    return [m.decode() for m in re.findall(rb"[A-Z][A-Z0-9_]{5,}", raw)]


def sha256_file(path: str) -> str:
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def provenance() -> dict:
    def git(*args: str) -> str:
        return subprocess.run(["git", "-C", str(ROOT), *args], capture_output=True,
                              text=True, check=False).stdout.strip()

    status = git("status", "--porcelain")
    return {
        "utc": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "git_sha": git("rev-parse", "HEAD"),
        "git_branch": git("rev-parse", "--abbrev-ref", "HEAD"),
        "dirty_count": len([line for line in status.splitlines() if line.strip()]),
        "dirty_paths": [line[3:] for line in status.splitlines() if line.strip()],
        "sley_binary": os.environ["SLEY2_SLEY_BINARY"],
        "sley_binary_sha256": sha256_file(os.environ["SLEY2_SLEY_BINARY"]),
        "judge_driver_binary": os.environ["SUCC_JUDGE_TEST_BINARY"],
        "judge_driver_sha256": sha256_file(os.environ["SUCC_JUDGE_TEST_BINARY"]),
        "base_pack_sha256": sha256_file(str(TASK_DIR / "base.pack")),
        "python": sys.version.split()[0],
    }


def op_replace(kind: int, target: str, payload: dict) -> dict:
    return {"class": "ReplaceEntityVersion", "kind": kind, "target": target,
            "field_tag": None, "payload": payload}


def op_delete(kind: int, target: str) -> dict:
    return {"class": "DeleteEntityBinding", "kind": kind, "target": target,
            "field_tag": None, "payload": None}


def testcase(target: str) -> dict:
    return {"class": "CreateEntity", "kind": KINDS["testcase"], "target": None,
            "field_tag": None, "payload": {
                "target": target,
                "inputs": [{"value_type": SINT, "data": {"variant": "SInt", "value": 7}},
                           {"value_type": SINT, "data": {"variant": "SInt", "value": 10}}],
                "effect_environment": {"variant": "Replay", "value": []},
                "expected": {"variant": "Value", "value": {
                    "value_type": BOOL, "data": {"variant": "Bool", "value": True}}},
                "observations": [],
                # Within the frozen principal grant (fuel/memory/output
                # 1000, effects 100): a selected test is checked against
                # the grant at phase 12.
                "resource_limits": {"fuel": 1000, "memory_bytes": 1000,
                                    "output_bytes": 1000, "effect_count": 0,
                                    "call_depth": 16, "wall_timeout_millis": 60000}}}


def build_ops(variant: str, ids: dict, bodies: dict) -> list[dict]:
    ns, live, dead_block, helper = ids["namespace"], ids["live_func"], ids["dead_block"], ids["dead_helper"]
    live_body = bodies[live]
    dead_op = bodies[dead_block]["operations"][0]
    helper_body = bodies[helper]
    helper_block = helper_body["entry_block"]
    [helper_param] = helper_body["parameters"]
    live_op = bodies[live_body["entry_block"]]["operations"][0]

    def without(values: list, drop: str) -> list:
        assert drop in values, (drop, values)
        return [value for value in values if value != drop]

    drop_ns = op_replace(KINDS["namespace"], ns,
                         dict(bodies[ns], members=without(bodies[ns]["members"], helper)))
    drop_block = op_replace(KINDS["function"], live,
                            dict(live_body, blocks=without(live_body["blocks"], dead_block)))
    cascade_block = [op_delete(KINDS["block"], dead_block),
                     op_delete(KINDS["operation"], dead_op)]
    cascade_helper = [op_delete(KINDS["function"], helper),
                      op_delete(KINDS["block"], helper_block),
                      op_delete(KINDS["parameter"], helper_param)]
    full = [drop_ns, drop_block, *cascade_block, *cascade_helper]
    if variant == "pos":
        return full
    # Creates lead the op list (assembly numbers create preconditions
    # first; the PERF/CREATE witnesses use the same order).
    if variant == "probe_survivor_test":
        return [testcase(live)] + full
    if variant in ("neg_orphan93", "neg_orphan93_bypass"):
        return [op for op in full if op["target"] != dead_op]
    if variant == "neg_test_on_tombstone":
        return [testcase(helper)] + full
    if variant == "neg_helper_kept":
        return [drop_block, *cascade_block]
    if variant == "neg_reachable_changed":
        body = dict(bodies[live_op])
        assert body["opcode"] == 98, body["opcode"]
        return [op_replace(KINDS["operation"], live_op, dict(body, opcode=100))]
    if variant == "neg_public_deleted":
        live_block = live_body["entry_block"]
        ops = [op_replace(KINDS["namespace"], ns,
                          dict(bodies[ns], members=without(without(
                              bodies[ns]["members"], helper), live))),
               op_delete(KINDS["function"], live),
               op_delete(KINDS["block"], live_block),
               op_delete(KINDS["operation"], live_op)]
        ops += [op_delete(KINDS["parameter"], p) for p in live_body["parameters"]]
        return ops + cascade_block + cascade_helper
    if variant == "neg_extra_delete":
        constant = bodies[dead_op]["immediate"]["value"]
        members = without(without(bodies[ns]["members"], helper), constant)
        return [op_replace(KINDS["namespace"], ns, dict(bodies[ns], members=members)),
                drop_block, *cascade_block, *cascade_helper,
                op_delete(KINDS["constant"], constant)]
    raise SystemExit(f"unknown variant {variant}")


def main() -> int:
    if len(sys.argv) != 3 or sys.argv[1] not in VARIANTS:
        raise SystemExit(f"usage: succ_witness_dead.py {{{'|'.join(VARIANTS)}}} LOGFILE")
    variant, log_path = sys.argv[1], Path(sys.argv[2])
    for key in ("SLEY2_SLEY_BINARY", "SUCC_JUDGE_TEST_BINARY"):
        if not os.environ.get(key):
            raise SystemExit(f"missing env {key}")
    if log_path.exists():
        raise SystemExit(f"refusing to overwrite existing log {log_path}")
    lines: list[str] = []

    def emit(text: str) -> None:
        lines.append(text)
        print(text, flush=True)

    tag = f"DEAD witness/{variant}"
    emit(f"{tag}: provenance {json.dumps(provenance(), sort_keys=True)}")
    tmp = tempfile.mkdtemp(prefix="sley2-dead-witness-")
    ws = Path(tmp) / "ws"
    stage_initial("sley_2_0", TASK_ID, ws)
    stage_tooling("sley_2_0", ws)
    manifest = json.loads((TASK_DIR / "task_manifest.json").read_text())
    ids = manifest["entities"]
    saved_cwd = os.getcwd()
    os.chdir(ws)
    verdict: dict | None = None
    try:
        def run(*argv: str) -> tuple[int, dict]:
            out = io.StringIO()
            with mock.patch.object(sley2_tool.sys, "stdout", out):
                code = sley2_tool.main(list(argv))
            return code, json.loads(out.getvalue())

        # Read the whole reachable structure the fix touches (agent view).
        bodies: dict[str, dict] = {}
        frontier = [ids["namespace"], ids["live_func"], ids["dead_block"], ids["dead_helper"]]
        while frontier:
            entity = frontier.pop()
            if entity in bodies:
                continue
            _, rep = run("read", entity)
            entry = rep["report"]["decoded"]["entries"][0]
            bodies[entity] = entry["body"]
            if entry["kind"] == KINDS["function"]:
                frontier += entry["body"]["blocks"] + entry["body"]["parameters"]
            elif entry["kind"] == KINDS["block"]:
                frontier += entry["body"]["operations"]
        emit(f"{tag}: read {len(bodies)} entities")
        ops = build_ops(variant, ids, bodies)
        emit(f"{tag}: ops {json.dumps([(op['class'], op['kind'], (op['target'] or 'new')[:4]) for op in ops])}")
        _, proposed = run("propose", json.dumps(ops))
        report = proposed.get("report", {})
        emit(f"{tag}: created {report.get('created')} valid {report.get('valid')} "
             f"decision {report.get('decision')}")
        body = report.get("body") or ""
        if "created" not in report:
            emit(f"{tag}: tool refused {json.dumps(proposed, sort_keys=True)[:600]}")
        if report.get("created") is False:
            emit(f"{tag}: create refused symbols {failure_symbols(body)}")
        elif body:
            emit(f"{tag}: validation result {json.dumps(decode_result(body), sort_keys=True)}")
        record = report.get("record")
        if variant == "neg_orphan93_bypass":
            if report.get("valid"):
                emit(f"{tag}: UNEXPECTED valid orphan candidate")
                return 2
            # An agent can write the workspace directly: the judge must
            # refuse the stored bytes on its own production commit.
            [stored] = sley2_codecs.run_batch([{"op": "stored_from_record", "record": record}])
            (ws / "final_candidate.hex").write_text(stored["stored"] + "\n", encoding="utf-8")
            emit(f"{tag}: finish bypassed; stored candidate written directly")
        elif report.get("valid"):
            _, done = run("finish", record)
            emit(f"{tag}: finished {done.get('report', {}).get('finished')}")
        else:
            _, done = run("finish", record) if record else (None, {})
            emit(f"{tag}: finish refused {json.dumps(done.get('report', {}).get('finished'))}; "
                 f"judge not reached (validation refusal is the verdict)")
            return 0
        saved_argv = sys.argv
        sys.argv = ["sley2_live_judge", str(ws)]
        out = io.StringIO()
        try:
            with contextlib.redirect_stdout(out):
                exit_code = judge.main(TASK_ID)
        finally:
            sys.argv = saved_argv
        raw = out.getvalue().strip()
        emit(f"{tag}: judge verdict {raw}")
        verdict = json.loads(raw.splitlines()[-1])
        if verdict.get("code") == "ORACLE_COMMIT_REJECTED":
            emit(f"{tag}: commit refusal symbols {failure_symbols(verdict.get('detail', ''))}")
        emit(f"{TASK_ID} witness/{variant} judge exit: {exit_code}")
        emit(f"workspace kept at: {ws}")
    finally:
        os.chdir(saved_cwd)
        log_path.parent.mkdir(parents=True, exist_ok=True)
        log_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
