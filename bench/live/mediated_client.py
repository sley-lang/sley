#!/usr/bin/env python3
"""Confined deterministic adapter: the agent side of the mediated trial.

Stdlib only. Runs inside the bwrap sandbox with no protected
filesystem: the only channel is frames on stdin/stdout to the trusted
gateway (request frame out, response frame in). Trial inputs
(entity ids, task id) arrive as `/scratch/trial_inputs.json` staged by
the runner; everything else the adapter needs comes back through
allowed responses. No codec, pack, repository, oracle, or capture path
is available inside the sandbox.

Multi-phase composition needs no client-side derivation: round 1
proposes the skeleton record to establish the nonce (mirrors the
unmediated witness); a gateway-local `resolve` call derives the full
create set mechanically (the server re-checks every identity at
create/validate time); round 2 composes the full op list under the
base nonce. The server re-derives and re-checks everything,
including identity preservation.

Diagnostics and the final outcome summary go to stderr
(parent-captured, never authoritative). Stdout carries ONLY gateway
request frames. Anything this process writes to /scratch (notes, hex
dumps, forged transcripts) is candidate-side diagnostics:
reconciliation never reads it.

Sequences (argv[1]):
  type_pos      bounded reads + creates propose + full compose + finish
                (complete JobState migration, Failed(7))
  type_code8    same shape with Failed(8) (alternative-literal positive)
  type_queued   same shape with status Queued (alternative-value positive)
  type_shared   same shape with two leaves sharing a constant
  type_nullcode Failed member with null payload (wrong-code negative:
                forbidden null error, loss of the explicit code)
  refusal_probe allowed read + denied command (expects ok then refusal;
                also drops forged candidate-side diagnostics)
  access_probe  attempt direct opens of protected paths given in
                argv[2:] (expects failures; reports errno outcomes)
"""

from __future__ import annotations

import json
import os
import sys

SINT = {"variant": "SInt", "value": 64}

SOCK_ENV = "SLEY2_GATEWAY_SOCK"
FRAME_LIMIT = 8 * 1024 * 1024


def log(text: str) -> None:
    sys.stderr.write(text + "\n")
    sys.stderr.flush()


def emit_provider_stream(commands: list[str]) -> None:
    """Print the provider-observed event stream on stdout (socket
    mode only; stdio mode reserves stdout for gateway frames). One
    completed tool item per gateway frame keeps the observed tool
    count reconciled with the captured exchange count."""

    def emit(value: dict) -> None:
        sys.stdout.write(json.dumps(value, sort_keys=True) + "\n")

    emit({"type": "thread.started", "thread_id": "thread-1"})
    emit({"type": "turn.started"})
    for index, command in enumerate(commands):
        emit({"type": "item.completed",
              "item": {"id": f"tool-{index}",
                       "type": "command_execution",
                       "command": f"sley-tool {command}",
                       "aggregated_output": "OK\n",
                       "status": "completed",
                       "exit_code": 0}})
    emit({"type": "turn.completed",
          "usage": {"input_tokens": 120, "cached_input_tokens": 20,
                    "output_tokens": 30,
                    "reasoning_output_tokens": 5}})
    sys.stdout.flush()


class Gateway:
    def __init__(self, session_id: str) -> None:
        self.session_id = session_id
        self.frames = 0
        self.cmdlog: list[str] = []
        sock_path = os.environ.get(SOCK_ENV, "")
        self._sockfile = None
        if sock_path:
            import socket as _socket

            try:
                client = _socket.socket(_socket.AF_UNIX,
                                        _socket.SOCK_STREAM)
                client.connect(sock_path)
            except OSError as error:
                raise RuntimeError(f"gateway socket: {error}")
            self._sockfile = client.makefile("rwb")

    def call(self, phase: str, command: str,
             args: list[str]) -> dict:
        frame = {"phase": phase, "session_id": self.session_id,
                 "command": command, "args": args}
        if self._sockfile is not None:
            raw = (json.dumps(frame, sort_keys=True) + "\n").encode()
            try:
                self._sockfile.write(raw)
                self._sockfile.flush()
                line = self._sockfile.readline(FRAME_LIMIT + 2)
            except OSError as error:
                raise RuntimeError(f"gateway transport: {error}")
            if not line:
                raise RuntimeError("gateway EOF")
            reply = json.loads(line.decode("utf-8"))
        else:
            sys.stdout.write(json.dumps(frame, sort_keys=True) + "\n")
            sys.stdout.flush()
            line = sys.stdin.readline()
            if not line:
                raise RuntimeError("gateway EOF")
            reply = json.loads(line)
        self.frames += 1
        self.cmdlog.append(command)
        if reply.get("uncaptured"):
            raise RuntimeError(
                f"gateway capture failure: {reply.get('error')}")
        return reply


def op_create(kind: int, payload: dict) -> dict:
    return {"class": "CreateEntity", "kind": kind, "target": None,
            "field_tag": None, "payload": payload}


def op_replace(kind: int, target: str, payload: dict) -> dict:
    return {"class": "ReplaceEntityVersion", "kind": kind,
            "target": target, "field_tag": None, "payload": payload}


def named(typedef_id: str) -> dict:
    return {"variant": "Named",
            "value": {"definition": typedef_id, "arguments": []}}


def typedef_payload(members: dict[str, str]) -> dict:
    def case(member: str, payload: dict) -> dict:
        return {"member_id": member, "payload_type": payload}

    return {"type_parameters": [],
            "form": {"variant": "Variant", "value": [
                case(members["queued"], {"variant": "None"}),
                case(members["running"], {"variant": "None"}),
                case(members["succeeded"], {"variant": "None"}),
                case(members["failed"],
                     {"variant": "Some", "value": SINT})]},
            "invariants": [], "visibility": "Private"}


def variant_value(typedef_id: str, member: str,
                  payload: dict | None) -> dict:
    return {"value_type": named(typedef_id),
            "data": {"variant": "Variant", "value": {
                "definition": typedef_id, "member_id": member,
                "payload": ({"variant": "None"} if payload is None
                            else {"variant": "Some", "value": payload})}}}


def sint_const(value: int) -> dict:
    return {"value_type": SINT,
            "data": {"variant": "SInt", "value": value}}


def _finish_skeleton(gw: Gateway, inputs: dict) -> int:
    """Minimal legitimate flow: bounded read, skeleton propose, and
    finish. The finished record lands in protected state only."""

    revision = gw.call("read", "revision", [])
    if not revision.get("ok"):
        log("SUMMARY " + json.dumps({"ok": False,
                                     "error": "revision refused"},
                                    sort_keys=True))
        return 0
    proposal = gw.call(
        "compose", "propose",
        [json.dumps([op_create(4, typedef_payload(inputs["members"]))])])
    report = proposal.get("report", {}) if proposal.get("ok") else {}
    if not report.get("created"):
        log("SUMMARY " + json.dumps({"ok": False,
                                     "error": "propose refused"},
                                    sort_keys=True))
        return 0
    finale = gw.call("finish", "finish", [report["record"]])
    finished = bool(finale.get("ok")
                    and (finale.get("report") or {}).get("finished"))
    log("SUMMARY " + json.dumps({"ok": finished, "frames": gw.frames,
                                 "finished": finished}, sort_keys=True))
    return 0


def seq_type(gw: Gateway, inputs: dict, code: int,
             status_member: str | None, leaf_values: list[int],
             null_payload: bool) -> dict:
    """Bounded read / propose-creates / compose-full / finish via gateway."""

    outcome: dict = {"ok": False, "steps": []}

    def step(name: str, phase: str, command: str,
             args: list[str]) -> dict:
        reply = gw.call(phase, command, args)
        outcome["steps"].append(
            {"name": name, "ok": reply.get("ok"),
             "bytes": len(json.dumps(reply, sort_keys=True))})
        return reply

    members = inputs["members"]
    status_id = inputs["entities"]["status"]
    switch_id = inputs["entities"]["switch"]
    param_id = inputs["entities"]["param"]
    entry_id = inputs["entities"]["switch_entry"]
    leaf_id = inputs["entities"]["switch_leaf"]
    failed = members["failed"]
    if status_member is None:
        status_member = failed

    revision = step("revision", "read", "revision", [])
    if not revision.get("ok"):
        outcome["error"] = "revision refused"
        return outcome
    # Bounded status read: behavior evidence through the allowed
    # interface (the pre-migration Bool body is expected here).
    status_read = step("status_read", "read", "read", [status_id])
    if not status_read.get("ok"):
        outcome["error"] = "status read refused"
        return outcome

    create_kinds = [4, 7, 7, 7, 6, 9, 9, 9, 8, 8, 8]

    # Round 1: skeleton typedef to establish the record nonce (mirrors
    # the unmediated witness; known-valid standalone).
    round1 = step("propose_skeleton", "compose", "propose",
                  [json.dumps([op_create(4, typedef_payload(members))])])
    if not round1.get("ok"):
        outcome["error"] = "round1 propose refused"
        return outcome
    report1 = round1.get("report", {})
    if not report1.get("created"):
        outcome["error"] = "round1 not created"
        return outcome
    base_record = report1["record"]
    # Gateway-local derivation of the full create set under the base
    # nonce (mechanical; the server re-checks every identity at
    # create/validate time). No client-side codec stack needed.
    resolved = step("resolve_ids", "compose", "resolve",
                    [base_record, json.dumps(create_kinds)])
    if not resolved.get("ok"):
        outcome["error"] = "resolve refused"
        return outcome
    ids = (resolved.get("report") or {}).get("ids", [])
    if len(ids) != 11:
        outcome["error"] = f"expected 11 derived ids, got {len(ids)}"
        return outcome
    (typedef_id, new_a, new_b, new_c, blk_param,
     const0, const1, const2, op_6e, op_a, op_b) = ids

    def leaf_block(block: str, op: str) -> dict:
        return {"function": switch_id, "parameters": [],
                "operations": [op],
                "terminator": {"variant": "Return", "value": {
                    "value": {"variant": "OperationResult",
                              "value": {"operation": op,
                                        "result_index": 0}}}},
                "reachability": "Required"}

    def op_payload(block: str, const: str) -> dict:
        return {"block": block, "ordinal": 0, "opcode": 1,
                "operands": [], "result_types": [SINT],
                "immediate": {"variant": "Entity", "value": const}}

    cases = [
        {"case_key": {"variant": "Member",
                      "value": members["queued"]},
         "edge": {"target": leaf_id, "arguments": []}},
        {"case_key": {"variant": "Member",
                      "value": members["running"]},
         "edge": {"target": new_a, "arguments": []}},
        {"case_key": {"variant": "Member",
                      "value": members["succeeded"]},
         "edge": {"target": new_b, "arguments": []}},
        {"case_key": {"variant": "Member", "value": failed},
         "edge": {"target": new_c,
                  "arguments": [{"variant": "CasePayload"}]}},
    ]
    entry_payload = {
        "function": switch_id, "parameters": [], "operations": [],
        "terminator": {"variant": "VariantSwitch", "value": {
            "value": {"variant": "Parameter", "value": param_id},
            "cases": cases}},
        "reachability": "Required"}
    failed_leaf = {
        "function": switch_id, "parameters": [blk_param],
        "operations": [],
        "terminator": {"variant": "Return", "value": {
            "value": {"variant": "Parameter", "value": blk_param}}},
        "reachability": "Required"}
    func_payload = {
        "type_parameters": [], "parameters": [param_id],
        "result_type": SINT, "effects": [],
        "entry_block": entry_id,
        "blocks": [entry_id, leaf_id, new_a, new_b, new_c],
        "contracts": [], "visibility": "Private"}
    param_payload = {
        "owner": switch_id, "role": "Function", "ordinal": 0,
        "value_type": named(typedef_id)}
    blk_param_payload = {"owner": new_c, "role": "Block", "ordinal": 0,
                         "value_type": SINT}
    if null_payload:
        status_value: dict = variant_value(typedef_id, failed, None)
    elif status_member == failed:
        status_value = variant_value(typedef_id, failed, sint_const(code))
    else:
        status_value = variant_value(typedef_id, status_member, None)

    full = [
        op_create(4, typedef_payload(members)),
        op_create(7, leaf_block(new_a, op_a)),
        op_create(7, leaf_block(new_b, op_b)),
        op_create(7, failed_leaf),
        op_create(6, blk_param_payload),
        op_create(9, {"value": sint_const(leaf_values[0])}),
        op_create(9, {"value": sint_const(leaf_values[1])}),
        op_create(9, {"value": sint_const(leaf_values[2])}),
        op_create(8, op_payload(leaf_id, const0)),
        op_create(8, op_payload(new_a, const1)),
        op_create(8, op_payload(new_b, const2)),
        op_replace(9, status_id, {"value": status_value}),
        op_replace(6, param_id, param_payload),
        op_replace(5, switch_id, func_payload),
        op_replace(7, entry_id, entry_payload),
        op_replace(7, leaf_id, leaf_block(leaf_id, op_6e)),
    ]
    round2 = step("compose_full", "compose", "compose",
                  [base_record, json.dumps(full)])
    if not round2.get("ok"):
        outcome["error"] = "round2 compose refused"
        return outcome
    report2 = round2.get("report", {})
    if not report2.get("created"):
        outcome["error"] = "round2 not created"
        return outcome
    outcome["valid"] = report2.get("valid")
    outcome["decision"] = report2.get("decision")
    if not report2.get("valid"):
        outcome["error"] = "round2 invalid"
        return outcome
    finale = step("finish", "finish", "finish", [report2["record"]])
    outcome["finished"] = bool(
        finale.get("ok") and (finale.get("report") or {}).get("finished"))
    outcome["ok"] = outcome["finished"]
    if not outcome["ok"]:
        outcome["error"] = "finish refused"
    return outcome


def main() -> int:
    if len(sys.argv) < 2:
        log("usage: mediated_client.py SEQUENCE [args...]")
        return 2
    sequence = sys.argv[1]
    socket_mode = bool(os.environ.get(SOCK_ENV, ""))
    if sequence == "malformed_ingress":
        # Raw-socket discipline violation: unparseable input must
        # permanently invalidate the attempt (fail-closed ingress).
        import socket as _socket

        try:
            client = _socket.socket(_socket.AF_UNIX, _socket.SOCK_STREAM)
            client.connect(os.environ[SOCK_ENV])
            client.sendall(b"this is not a frame\n")
            client.close()
        except OSError as error:
            log(f"ingress setup failed: {error}")
            return 2
        if socket_mode:
            emit_provider_stream([])
        log("SUMMARY " + json.dumps({"ok": True, "frames": 0},
                                    sort_keys=True))
        return 0
    if sequence == "oversized_ingress":
        import socket as _socket

        try:
            client = _socket.socket(_socket.AF_UNIX, _socket.SOCK_STREAM)
            client.connect(os.environ[SOCK_ENV])
            client.sendall(b"x" * (FRAME_LIMIT + 1024) + b"\n")
            client.close()
        except OSError as error:
            log(f"ingress setup failed: {error}")
            return 2
        if socket_mode:
            emit_provider_stream([])
        log("SUMMARY " + json.dumps({"ok": True, "frames": 0},
                                    sort_keys=True))
        return 0
    try:
        with open("/scratch/trial_inputs.json", encoding="utf-8") as handle:
            inputs = json.load(handle)
    except OSError as error:
        log(f"no trial inputs: {error}")
        return 2
    gw = Gateway(session_id=f"agent-{os.getpid()}")
    code = _run_sequence(gw, inputs, sequence)
    if socket_mode:
        emit_provider_stream(gw.cmdlog)
    return code


def _run_sequence(gw: Gateway, inputs: dict, sequence: str) -> int:
    if sequence == "access_probe":
        results: dict = {"reads": {}, "writes": {}}
        for path in sys.argv[2:]:
            try:
                with open(path, "rb") as handle:
                    results["reads"][path] = {
                        "ok": True, "n": len(handle.read(16))}
            except Exception as error:  # noqa: BLE001 - probe records outcomes
                results["reads"][path] = {
                    "ok": False, "error": type(error).__name__,
                    "errno": getattr(error, "errno", None)}
            probe_file = os.path.join(path, ".sley-client-probe")
            try:
                with open(probe_file, "wb") as handle:
                    handle.write(b"probe")
                results["writes"][path] = {"ok": True}
            except Exception as error:  # noqa: BLE001 - probe records outcomes
                results["writes"][path] = {
                    "ok": False, "error": type(error).__name__,
                    "errno": getattr(error, "errno", None)}
        log("ACCESS_PROBE_RESULT " + json.dumps(results, sort_keys=True))
        return 0
    if sequence == "refusal_probe":
        first = gw.call("read", "revision", [])
        second = gw.call("read", "commit", ["00"])
        log(json.dumps(
            {"first_ok": first.get("ok"),
             "second_ok": second.get("ok"),
             "second_error": str(second)[:200]}, sort_keys=True))
        # Forged candidate-side diagnostics: must never count.
        with open("/scratch/final_candidate.hex", "w",
                   encoding="utf-8") as handle:
            handle.write("deadbeef\n")
        with open("/scratch/.sley-live-transcript.jsonl", "w",
                   encoding="utf-8") as handle:
            handle.write('{"forged": true}\n')
        summary = {"ok": bool(first.get("ok"))
                   and not second.get("ok", True),
                   "frames": gw.frames}
        log("SUMMARY " + json.dumps(summary, sort_keys=True))
        return 0
    if sequence == "two_phase":
        # Two client sessions across read/compose phases: budgets and
        # usage must accumulate trial-wide, never reset per session.
        first = Gateway(session_id=f"agent-{os.getpid()}-a")
        second = Gateway(session_id=f"agent-{os.getpid()}-b")
        revision = first.call("read", "revision", [])
        caps = second.call("read", "caps", [])
        summary = {"ok": bool(revision.get("ok")) and bool(caps.get("ok")),
                   "frames": gw.frames + first.frames + second.frames}
        log("SUMMARY " + json.dumps(summary, sort_keys=True))
        gw.cmdlog.extend(first.cmdlog + second.cmdlog)
        return 0
    if sequence == "denied_then_finish":
        # A denied command is a recorded, counted refusal; the
        # attempt continues and may still legitimately finish.
        revision = gw.call("read", "revision", [])
        denied = gw.call("read", "commit", ["00"])
        if not revision.get("ok") or denied.get("ok", True):
            log("SUMMARY " + json.dumps({"ok": False}, sort_keys=True))
            return 0
        code = _finish_skeleton(gw, inputs)
        return code
    if sequence in ("finish_skeleton", "forge_scratch", "no_finish"):
        if sequence == "no_finish":
            # A valid partial flow with no finish: no runner-held
            # final exists, so no completion linkage is possible.
            revision = gw.call("read", "revision", [])
            proposal = gw.call(
                "compose", "propose",
                [json.dumps([op_create(
                    4, typedef_payload(inputs["members"]))])])
            summary = {"ok": bool(revision.get("ok"))
                       and bool(proposal.get("ok")),
                       "frames": gw.frames, "finished": False}
            log("SUMMARY " + json.dumps(summary, sort_keys=True))
            return 0
        code = _finish_skeleton(gw, inputs)
        if sequence == "forge_scratch" and code == 0:
            # Forged candidate-side diagnostics: the attempt may
            # still accept (protected flow is legitimate), but the
            # forged bytes must never become the held evidence.
            with open("/scratch/final_candidate.hex", "w",
                       encoding="utf-8") as handle:
                handle.write("forged-by-agent\n")
            with open("/scratch/.sley-live-transcript.jsonl", "w",
                       encoding="utf-8") as handle:
                handle.write('{"forged": true}\n')
        return code
    params: dict = {
        "type_pos": {"code": 7, "status_member": None,
                     "leaf_values": [0, 1, 2], "null_payload": False},
        "type_code8": {"code": 8, "status_member": None,
                       "leaf_values": [0, 1, 2], "null_payload": False},
        "type_queued": {"code": 7, "status_member": "QUEUED",
                        "leaf_values": [0, 1, 2], "null_payload": False},
        "type_shared": {"code": 7, "status_member": None,
                        "leaf_values": [0, 0, 2], "null_payload": False},
        "type_nullcode": {"code": 7, "status_member": None,
                          "leaf_values": [0, 1, 2], "null_payload": True},
    }
    if sequence in params:
        if sequence == "type_queued":
            params[sequence]["status_member"] = inputs["members"]["queued"]
        outcome = seq_type(gw, inputs, **params[sequence])
        outcome["frames"] = gw.frames
        log("SUMMARY " + json.dumps(
            {k: v for k, v in outcome.items() if k != "steps"},
            sort_keys=True))
        return 0
    log(f"sequence {sequence} unknown")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
