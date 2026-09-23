#!/usr/bin/env python3
"""Deterministic test-only adapter for mediated trials (never production).

Test stand-in at the provider boundary: drives scripted witness
sequences (seq_type/seq_stale), task-specific construction logic,
witness member literals (CLIENT_MEMBERS), and synthetic
provider-event emission (emit_provider_stream) for deterministic
integration tests only.

Never staged by ``stage_mediated_scratch``: the production scratch
holds only the generic transport (``mediated_transport.py``), the
documented shim, and authorized task inputs. Tests inject this file
after production staging via the adapter boundary and exercise the
real confinement, mediation, capture, oracle, append, and
verification machinery. Not counted as model trials.

Stdlib only. Runs inside the bwrap sandbox with no protected
filesystem: the only channel is frames to the trusted gateway
through ``mediated_transport.Gateway`` (every call captured and
counted). Anything this process writes to /scratch (notes, hex
dumps, forged transcripts) is candidate-side diagnostics:
reconciliation never reads it.

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
  stale_pos     STALE guard flip via inventory/read discovery + finish
                (non-TYPE mediated proof; no staged guard id)
  context MODE  CONTEXT bounded discovery + repair via the allowed
                surface: open (accepted-head snapshot binding), bounded
                class-4/class-14 root queries with explicit continuation,
                class-2 kind probes, reads, propose, finish. No identity
                arrives as an argument (modes: see CONTEXT_MODES)
  refusal_probe allowed read + denied command (expects ok then refusal;
                also drops forged candidate-side diagnostics)
  access_probe  attempt direct opens of protected paths given in
                argv[2:] (expects failures; reports errno outcomes)
"""

from __future__ import annotations

import json
import os
import sys

try:
    from mediated_transport import (
        FRAME_LIMIT,
        SINT,
        SOCK_ENV,
        Gateway,
        _decoded_body,
        _report,
        log,
        op_create,
        op_replace,
    )
except ImportError:  # repo-side lint/typecheck: same tree, package path
    from bench.live.mediated_transport import (
        FRAME_LIMIT,
        SINT,
        SOCK_ENV,
        Gateway,
        _decoded_body,
        _report,
        log,
        op_create,
        op_replace,
    )

# Agent-authored member ids for the new JobState typedef (generated
# client-side, never staged by the runner; the judge checks shape and
# coverage, never these literals). TEST-ONLY: scripted witness
# literals, never production inputs.
CLIENT_MEMBERS = {"queued": "51" * 32, "running": "52" * 32,
                  "succeeded": "53" * 32, "failed": "54" * 32}


def open_head(call) -> dict:
    """The agent's own head read: `open` (workspace.open, the afforded
    accepted-head opener), then `revision <tx>` of the tx that open
    reported. No harness-supplied head id is involved. ``call`` is
    ``(name, phase, command, args) -> reply``; returns the decoded head
    view (tx/root/policy/workspace/epoch/..., plus `snapshot` only when
    the head snapshot is materialized). Raises RuntimeError on refusal."""

    opened = call("open", "read", "open", [])
    report = _report(opened)
    head = report.get("decoded") if not report.get("failed") else None
    if not isinstance(head, dict) or len(str(head.get("tx", ""))) != 64:
        raise RuntimeError("open refused")
    revision = call("revision", "read", "revision", [head["tx"]])
    reread = _report(revision)
    if reread.get("failed"):
        raise RuntimeError("revision refused")
    return head


def _plain(gw: Gateway):
    return lambda name, phase, command, args: gw.call(phase, command, args)


def discover_type_roles(gw: Gateway) -> dict:
    """Discover TYPE starting identities through the allowed surface.

    inventory lists every served (entity, kind); read returns each
    decoded body. Roles are identified structurally, never by a
    staged manifest map or 6c-prefix convention:
    - status: kind-9 constant whose Bool payload is the pre-migration
      job flag (read to confirm);
    - switch: kind-5 function with exactly one parameter, two blocks,
      Bool result, whose entry block is a CondBranch on its parameter
      (the Bool dispatch under migration);
    - param: the switch's sole Function-role parameter;
    - entry/leaf: the switch's entry block and its sibling block.
    Every step is a captured gateway exchange like any agent action.
    """

    open_head(_plain(gw))
    inv_reply = gw.call("read", "inventory", [])
    report = _report(inv_reply)
    objects = ((report.get("inventory") or {}).get("objects")) or []
    if not isinstance(objects, list) or not objects:
        raise RuntimeError("empty inventory")
    by_kind: dict[int, list[str]] = {}
    for item in objects:
        if not isinstance(item, dict):
            continue
        entity = item.get("entity")
        kind = item.get("kind")
        if (isinstance(entity, str) and len(entity) == 64
                and isinstance(kind, int)):
            by_kind.setdefault(kind, []).append(entity)
    # Status: kind-9 constant holding the pre-migration Bool flag.
    status = ""
    for entity in by_kind.get(9, []):
        body = _decoded_body(_report(gw.call("read", "read", [entity])))
        value = (body.get("value") or {}) if isinstance(body, dict) else {}
        data = (value.get("data") or {}) if isinstance(value, dict) else {}
        vtype = (value.get("value_type") or {}) if isinstance(value, dict) else {}
        if (isinstance(data, dict) and data.get("variant") == "Bool"
                and isinstance(vtype, dict)
                and vtype.get("variant") == "Bool"):
            status = entity
            break
    if not status:
        raise RuntimeError("status role not discoverable")
    # Switch: kind-5 function, one param, two blocks, Bool result,
    # entry block CondBranch on its own parameter.
    switch = ""
    entry = ""
    leaf = ""
    param = ""
    for entity in by_kind.get(5, []):
        body = _decoded_body(_report(gw.call("read", "read", [entity])))
        if not isinstance(body, dict):
            continue
        params = body.get("parameters")
        blocks = body.get("blocks")
        result = body.get("result_type") or {}
        if (not isinstance(params, list) or len(params) != 1
                or not isinstance(blocks, list) or len(blocks) != 2
                or not isinstance(result, dict)
                or result.get("variant") != "Bool"):
            continue
        entry_cand = body.get("entry_block")
        if not isinstance(entry_cand, str):
            continue
        try:
            entry_body = _decoded_body(
                _report(gw.call("read", "read", [entry_cand])))
        except RuntimeError:
            continue
        if not isinstance(entry_body, dict):
            continue
        term = (entry_body.get("terminator") or {}) if isinstance(
            entry_body, dict) else {}
        if not isinstance(term, dict) or term.get("variant") != "CondBranch":
            continue
        value = (term.get("value") or {}) if isinstance(term, dict) else {}
        cond = (value.get("condition") or {}) if isinstance(value, dict) else {}
        if (not isinstance(cond, dict) or cond.get("variant") != "Parameter"
                or cond.get("value") not in params):
            continue
        if entry_body.get("function") != entity:
            continue
        # Param must be a Function-role Bool parameter owned by this fn.
        try:
            param_body = _decoded_body(
                _report(gw.call("read", "read", [params[0]])))
        except RuntimeError:
            continue
        if (not isinstance(param_body, dict)
                or param_body.get("owner") != entity
                or param_body.get("role") != "Function"
                or not isinstance(param_body.get("value_type"), dict)
                or param_body["value_type"].get("variant") != "Bool"):
            continue
        switch = entity
        entry = entry_cand
        param = params[0]
        others = [b for b in blocks if b != entry]
        if len(others) != 1 or not isinstance(others[0], str):
            continue
        leaf = others[0]
        break
    if not (switch and param and entry and leaf):
        raise RuntimeError("switch role not discoverable")
    return {"status": status, "switch": switch, "param": param,
            "switch_entry": entry, "switch_leaf": leaf}


# Root-backed query wire (docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md;
# sley-query encode_preimage / encode_response). Agent-side encoding of the
# served `query.root`/`query.continue` request preimage: every identity it
# names comes from the agent's own `open` (snapshot, epoch, root,
# workspace) or from earlier discovery responses, never from a staged
# input. The server rebuilds the preimage from the accepted head and
# refuses QUERY_SNAPSHOT_MISMATCH on any difference.
ROOT_QUERY_MAGIC = b"SLEYRQQ1"
ROOT_RESPONSE_MAGIC = b"SLEYRQR1"
CLASS_ROOT_SUMMARY = 1
CLASS_GET_ENTITY = 2
CLASS_ENTITIES_BY_KIND = 4
CLASS_REVERSE_IMPACT = 14
KIND_TYPEDEF = 4
KIND_CONSTANT = 9
CURSOR_ENTITY = 1


def _u32(value: int) -> bytes:
    return int(value).to_bytes(4, "big")


def _u64(value: int) -> bytes:
    return int(value).to_bytes(8, "big")


def root_query_preimage(head: dict, class_tag: int, class_body: bytes, *,
                        max_entities: int, allow_continuation: bool,
                        after: str | None = None,
                        snapshot: str | None = None) -> str:
    """Hex request preimage for one root-backed query over the opened
    head. ``snapshot`` defaults to the head's disclosed field 9."""

    bound = head.get("snapshot") if snapshot is None else snapshot
    if not isinstance(bound, str) or len(bound) != 64:
        raise RuntimeError("no snapshot binding")
    out = bytearray(ROOT_QUERY_MAGIC)
    out += _u32(1) + _u32(1)
    for ident in (bound, head["epoch"], head["root"], head["workspace"]):
        out += bytes.fromhex(ident)
    out += _u32(2) + _u32(1)
    # Limits: entities per page, edges, depth, response bytes, work.
    out += (_u64(max_entities) + _u64(1) + _u32(65_535)
            + _u64(1_048_576) + _u64(100_000_000))
    out += _u32(2 if allow_continuation else 1)
    if after is None:
        out += _u32(1)
    else:
        out += _u32(2) + _u32(CURSOR_ENTITY) + bytes.fromhex(after)
    out += _u32(class_tag) + class_body
    return bytes(out).hex()


def entity_list_body(seeds: list[str]) -> bytes:
    return _u64(len(seeds)) + b"".join(bytes.fromhex(s) for s in seeds)


def decode_root_response(body_hex: str) -> dict:
    """Decode a root-query response: header counts, truncation, next
    cursor, and the result of an entity-list class (4, 14) or of the
    single-entity class 2 (kind, object id)."""

    data = bytes.fromhex(body_hex)
    at = 0

    def take(size: int) -> bytes:
        nonlocal at
        if at + size > len(data):
            raise RuntimeError("root response short")
        chunk = data[at:at + size]
        at += size
        return chunk

    def u32() -> int:
        return int.from_bytes(take(4), "big")

    def u64() -> int:
        return int.from_bytes(take(8), "big")

    def cursor() -> str | None:
        if u32() == 1:
            return None
        tag = u32()
        if tag != CURSOR_ENTITY:
            raise RuntimeError("unexpected cursor kind")
        return take(32).hex()

    if take(8) != ROOT_RESPONSE_MAGIC or u32() != 1 or u32() != 1:
        raise RuntimeError("root response header")
    take(32)  # query id
    snapshot = take(32).hex()
    take(32 * 3)  # epoch, root, workspace
    if u32() != 2 or u32() != 1:
        raise RuntimeError("root response profile")
    take(8 + 8 + 4 + 8 + 8)  # echoed limits
    u32()  # continuation flag
    cursor()  # echoed request cursor
    class_tag = u32()
    total = u64()
    returned = u64()
    truncated = u32() == 2
    next_after = cursor()
    u32()  # reached depth
    u64()  # charged work
    u64()  # response bytes
    if u32() != class_tag:
        raise RuntimeError("root response class")
    page = {"snapshot": snapshot, "class": class_tag, "total": total,
            "returned": returned, "truncated": truncated,
            "next_after": next_after}
    if class_tag == CLASS_GET_ENTITY:
        page["kind"] = u32()
        page["object_id"] = take(32).hex()
        if u32() == 2:
            take(32)  # fingerprint
    else:
        count = u64()
        page["entities"] = [take(32).hex() for _ in range(count)]
        if count != returned:
            raise RuntimeError("root response count")
    if at != len(data):
        raise RuntimeError("root response trailer")
    return page


def emit_provider_stream(commands: list[str]) -> None:
    """TEST-ONLY synthetic provider-event emission (never production).

    Prints the provider-observed event stream on stdout (socket mode
    only). One completed tool item per gateway frame keeps the
    observed tool count reconciled with the captured exchange count.
    """

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


# Agent-authored identity of the added record member (generated
# client-side; the judge discovers added members by diffing the typedef
# against its pristine pre-image, never from this literal).
CONTEXT_MEMBER = "e1" * 32
CONTEXT_MODES = ("pos", "stop_early", "incomplete", "extra_continue",
                 "overbudget", "noarg_revision")


def context_discover_and_repair(call, mode: str = "pos",
                                impact_page_size: int = 3) -> dict:
    """CONTEXT bounded discovery and repair through the allowed surface.

    ``call`` is ``(name, phase, command, args) -> reply envelope``, so
    the same agent logic runs over the mediated gateway and over the
    direct tool. Every identity the sequence names is discovered through
    the interface — nothing arrives as an invocation argument or from a
    manifest:

    1. `open` (workspace.open) discloses the accepted head and, when the
       head's index snapshot is materialized, its identity (field 9). A
       cold head carries no snapshot: one bounded root query (refused
       QUERY_SNAPSHOT_MISMATCH) materializes it on the query path, and a
       second `open` discloses it. `revision <tx>` rereads the opened tx.
    2. A bounded class-4 listing of type definitions (paged) plus reads
       select the record typedef the task names by intent ("add a
       required record field").
    3. Class-14 reverse impact closure from that typedef, paged with
       explicit `query.continue` until no page is truncated.
    4. Class-2 kind probes on each closure member; constants are read and
       those holding a record of the typedef are the impact set.
    5. One propose (typedef + every impacted constant), then finish.

    ``impact_page_size`` bounds each impact page (3 on the mediated
    route, so the 10-entity closure needs three explicit continuations;
    the direct tool audits continuation per invocation, so its witness
    uses one untruncated page).

    Modes (negatives are for the rejection proofs):
      pos             complete discovery (paged) and repair
      stop_early      stops after the first (7-entity, truncated) impact
                      page although it already holds every constant:
                      complete repair, incomplete discovery evidence
      incomplete      complete discovery, typedef-only repair
      extra_continue  complete discovery, then one query.continue on the
                      finished chain (inconsistent continuation)
      overbudget      complete discovery and repair plus seven full
                      constant listings (cumulative response budget)
      noarg_revision  a no-argument `revision` first (must be refused),
                      then the positive flow
    """

    if mode not in CONTEXT_MODES:
        raise RuntimeError(f"unknown context mode {mode}")
    outcome: dict = {"ok": False, "mode": mode, "discovery": {}}
    disc = outcome["discovery"]
    if mode == "noarg_revision":
        bad = call("revision_noarg", "read", "revision", [])
        disc["noarg_revision_refused"] = not bad.get("ok")
        if bad.get("ok"):
            outcome["error"] = "no-argument revision was not refused"
            return outcome
    head = open_head(call)
    disc["opened_with_snapshot"] = "snapshot" in head
    if "snapshot" not in head:
        warm = _report(call("warm_snapshot", "read", "raw", [
            "query.root", root_query_preimage(
                head, CLASS_ROOT_SUMMARY, b"", max_entities=1,
                allow_continuation=False, snapshot="00" * 32)]))
        disc["warm_refused"] = bool(warm.get("failed"))
        head = open_head(call)
        if "snapshot" not in head:
            outcome["error"] = "snapshot not disclosed after warm query"
            return outcome
    disc["snapshot"] = head["snapshot"]

    def query(name: str, method: str, class_tag: int, body: bytes,
              page_size: int, allow: bool, after: str | None = None) -> dict:
        report = _report(call(name, "read", "raw", [
            method, root_query_preimage(
                head, class_tag, body, max_entities=page_size,
                allow_continuation=allow, after=after)]))
        if report.get("failed"):
            raise RuntimeError(f"{name} refused")
        page = decode_root_response(report.get("body") or "")
        if page["snapshot"] != head["snapshot"] or page["class"] != class_tag:
            raise RuntimeError(f"{name} binding")
        return page

    def paged(name: str, class_tag: int, body: bytes, page_size: int,
              stop_after: int | None = None) -> tuple[list[str], list[dict]]:
        page = query(f"{name}_0", "query.root", class_tag, body,
                     page_size, True)
        pages = [page]
        found = list(page["entities"])
        while page["truncated"]:
            if stop_after is not None and len(pages) >= stop_after:
                break
            page = query(f"{name}_{len(pages)}", "query.continue", class_tag,
                         body, page_size, True, after=page["next_after"])
            pages.append(page)
            found.extend(page["entities"])
        return found, pages

    def read_body(entity: str) -> tuple[int, dict]:
        report = _report(call(f"read_{entity[:8]}", "read", "read", [entity]))
        entries = (report.get("decoded") or {}).get("entries") or []
        if report.get("failed") or len(entries) != 1:
            raise RuntimeError("read body shape")
        entry = entries[0]
        if not isinstance(entry.get("body"), dict) or not isinstance(
                entry.get("kind"), int):
            raise RuntimeError("read body shape")
        return entry["kind"], entry["body"]

    try:
        typedefs, typedef_pages = paged(
            "typedefs", CLASS_ENTITIES_BY_KIND, _u32(KIND_TYPEDEF), 8)
        records = []
        for entity in typedefs:
            kind, body = read_body(entity)
            if ((body.get("form") or {}).get("variant")) == "Record":
                records.append((entity, kind, body))
        disc["typedef_pages"] = len(typedef_pages)
        disc["record_typedefs"] = len(records)
        if len(records) != 1:
            outcome["error"] = "record typedef not uniquely discoverable"
            return outcome
        typedef, typedef_kind, typedef_body = records[0]
        seeds = entity_list_body([typedef])
        page_size = 7 if mode == "stop_early" else impact_page_size
        closure, impact_pages = paged(
            "impact", CLASS_REVERSE_IMPACT, seeds, page_size,
            stop_after=1 if mode == "stop_early" else None)
        disc["impact_total"] = impact_pages[0]["total"]
        disc["impact_pages"] = len(impact_pages)
        disc["impact_seen"] = len(closure)
        disc["impact_continuations"] = len(impact_pages) - 1
        disc["impact_left_truncated"] = impact_pages[-1]["truncated"]
        if mode == "extra_continue":
            extra = query("impact_extra", "query.continue",
                          CLASS_REVERSE_IMPACT, seeds, page_size, True,
                          after=closure[-1])
            disc["extra_continue_returned"] = extra["returned"]
        targets: list[tuple[str, int, dict]] = []
        for entity in closure:
            if entity == typedef:
                continue
            probe = query(f"kind_{entity[:8]}", "query.root",
                          CLASS_GET_ENTITY, bytes.fromhex(entity), 1, False)
            if probe["kind"] != KIND_CONSTANT:
                continue
            kind, body = read_body(entity)
            data = ((body.get("value") or {}).get("data") or {})
            record = data.get("value") if isinstance(data, dict) else None
            if (isinstance(data, dict) and data.get("variant") == "Record"
                    and isinstance(record, dict)
                    and record.get("definition") == typedef):
                targets.append((entity, kind, body))
        disc["impacted_constants"] = len(targets)
    except RuntimeError as error:
        outcome["error"] = f"discovery refused: {error}"
        return outcome

    form = dict(typedef_body.get("form") or {})
    fields = list(form.get("value") or [])
    if any(isinstance(field, dict) and field.get("member_id") == CONTEXT_MEMBER
           for field in fields):
        outcome["error"] = "member already present"
        return outcome
    fields.append({"member_id": CONTEXT_MEMBER,
                   "value_type": {"variant": "Bool"},
                   "visibility": "Private"})
    form["value"] = fields
    typedef_body["form"] = form
    ops = [op_replace(typedef_kind, typedef, typedef_body)]
    if mode != "incomplete":
        for entity, kind, body in targets:
            value = dict(body.get("value") or {})
            data = dict(value.get("data") or {})
            record = dict(data.get("value") or {})
            record["fields"] = list(record.get("fields") or []) + [{
                "member_id": CONTEXT_MEMBER,
                "value": {"value_type": {"variant": "Bool"},
                          "data": {"variant": "Bool", "value": False}}}]
            data["value"] = record
            value["data"] = data
            body["value"] = value
            ops.append(op_replace(kind, entity, body))
    if mode == "overbudget":
        for index in range(7):
            try:
                query(f"constants_{index}", "query.root",
                      CLASS_ENTITIES_BY_KIND, _u32(KIND_CONSTANT), 65_535,
                      False)
            except RuntimeError as error:
                outcome["error"] = f"listing refused: {error}"
                return outcome
    proposal = call("propose", "compose", "propose", [json.dumps(ops)])
    report = proposal.get("report", {}) if proposal.get("ok") else {}
    outcome["valid"] = report.get("valid")
    outcome["decision"] = report.get("decision")
    if not report.get("valid"):
        outcome["error"] = "propose invalid"
        return outcome
    finale = call("finish", "finish", "finish", [report["record"]])
    outcome["finished"] = bool(
        finale.get("ok") and (finale.get("report") or {}).get("finished"))
    outcome["ok"] = outcome["finished"]
    if not outcome["ok"]:
        outcome["error"] = "finish refused"
    return outcome


def seq_context(gw: Gateway, mode: str) -> dict:
    """Mediated CONTEXT sequence: every step is a captured gateway
    exchange (see context_discover_and_repair)."""

    steps: list[dict] = []

    def call(name: str, phase: str, command: str, args: list[str]) -> dict:
        reply = gw.call(phase, command, args)
        steps.append({"name": name, "ok": reply.get("ok"),
                      "bytes": len(json.dumps(reply, sort_keys=True))})
        return reply

    try:
        outcome = context_discover_and_repair(call, mode)
    except RuntimeError as error:
        outcome = {"ok": False, "mode": mode, "error": str(error)}
    outcome["steps"] = steps
    return outcome


def _finish_skeleton(gw: Gateway) -> int:
    """Minimal legitimate flow: bounded read, skeleton propose, and
    finish. The finished record lands in protected state only.
    Member ids are agent-authored locally (never staged inputs)."""

    try:
        open_head(_plain(gw))
    except RuntimeError as error:
        log("SUMMARY " + json.dumps({"ok": False, "error": str(error)},
                                    sort_keys=True))
        return 0
    proposal = gw.call(
        "compose", "propose",
        [json.dumps([op_create(4, typedef_payload(dict(CLIENT_MEMBERS)))])])
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


def seq_type(gw: Gateway, code: int,
             status_member: str | None, leaf_values: list[int],
             null_payload: bool) -> dict:
    """Bounded discovery + propose-creates / compose-full / finish.

    Starting identities come from inventory/read discovery through the
    gateway (captured, counted); new member ids are agent-authored
    locally. No staged manifest map or 6c-prefix selection."""

    outcome: dict = {"ok": False, "steps": []}

    def step(name: str, phase: str, command: str,
             args: list[str]) -> dict:
        reply = gw.call(phase, command, args)
        outcome["steps"].append(
            {"name": name, "ok": reply.get("ok"),
             "bytes": len(json.dumps(reply, sort_keys=True))})
        return reply

    try:
        roles = discover_type_roles(gw)
    except RuntimeError as error:
        outcome["error"] = f"discovery refused: {error}"
        return outcome
    members = dict(CLIENT_MEMBERS)
    status_id = roles["status"]
    switch_id = roles["switch"]
    param_id = roles["param"]
    entry_id = roles["switch_entry"]
    leaf_id = roles["switch_leaf"]
    failed = members["failed"]
    if status_member is None:
        status_member = failed

    try:
        open_head(step)
    except RuntimeError as error:
        outcome["error"] = str(error)
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


def seq_stale(gw: Gateway) -> dict:
    """STALE guard flip through the allowed surface (non-TYPE proof).

    Discovers the Bool guard constant via inventory/read (kind-9
    constant with a Bool payload — the only such entity flips under
    the stale-sequence flow), flips it via propose/finish. No staged
    manifest guard id."""

    outcome: dict = {"ok": False, "steps": []}

    def step(name: str, phase: str, command: str,
             args: list[str]) -> dict:
        reply = gw.call(phase, command, args)
        outcome["steps"].append(
            {"name": name, "ok": reply.get("ok"),
             "bytes": len(json.dumps(reply, sort_keys=True))})
        return reply

    try:
        open_head(step)
    except RuntimeError as error:
        outcome["error"] = str(error)
        return outcome
    inv = step("inventory", "read", "inventory", [])
    try:
        objects = ((inv.get("report") or {}).get("inventory") or {}).get(
            "objects") or []
    except AttributeError:
        outcome["error"] = "inventory shape"
        return outcome
    guard = ""
    guard_kind = 0
    guard_body: dict = {}
    for item in objects if isinstance(objects, list) else []:
        if not isinstance(item, dict):
            continue
        entity = item.get("entity")
        kind = item.get("kind")
        if not (isinstance(entity, str) and isinstance(kind, int)):
            continue
        if kind != 9:
            continue
        reply = step(f"read_{entity[:8]}", "read", "read", [entity])
        try:
            body = _decoded_body(_report(reply))
        except RuntimeError:
            continue
        value = (body.get("value") or {}) if isinstance(body, dict) else {}
        data = (value.get("data") or {}) if isinstance(value, dict) else {}
        if isinstance(data, dict) and data.get("variant") == "Bool":
            guard = entity
            guard_kind = kind
            guard_body = body
            break
    if not guard:
        outcome["error"] = "guard not discoverable"
        return outcome
    flipped = json.loads(json.dumps(guard_body))
    try:
        flipped["value"]["data"]["value"] = not flipped["value"]["data"]["value"]
    except (KeyError, TypeError):
        outcome["error"] = "guard body shape"
        return outcome
    ops = [{"class": "ReplaceEntityVersion", "kind": guard_kind,
            "target": guard, "field_tag": None, "payload": flipped}]
    proposal = step("propose", "compose", "propose", [json.dumps(ops)])
    report = proposal.get("report", {}) if proposal.get("ok") else {}
    if not report.get("valid"):
        outcome["error"] = "propose invalid"
        outcome["valid"] = report.get("valid")
        return outcome
    finale = step("finish", "finish", "finish", [report["record"]])
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
    gw = Gateway(session_id=f"agent-{os.getpid()}")
    code = _run_sequence(gw, sequence)
    if socket_mode:
        emit_provider_stream(gw.cmdlog)
    return code


def _run_sequence(gw: Gateway, sequence: str) -> int:
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
        try:
            open_head(_plain(gw))
            first = {"ok": True}
        except RuntimeError as error:
            first = {"ok": False, "error": str(error)}
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
        try:
            open_head(_plain(first))
            head_ok = True
        except RuntimeError:
            head_ok = False
        caps = second.call("read", "caps", [])
        summary = {"ok": head_ok and bool(caps.get("ok")),
                   "frames": gw.frames + first.frames + second.frames}
        log("SUMMARY " + json.dumps(summary, sort_keys=True))
        gw.cmdlog.extend(first.cmdlog + second.cmdlog)
        return 0
    if sequence == "denied_then_finish":
        # A denied command is a recorded, counted refusal; the
        # attempt continues and may still legitimately finish.
        try:
            open_head(_plain(gw))
            head_ok = True
        except RuntimeError:
            head_ok = False
        denied = gw.call("read", "commit", ["00"])
        if not head_ok or denied.get("ok", True):
            log("SUMMARY " + json.dumps({"ok": False}, sort_keys=True))
            return 0
        code = _finish_skeleton(gw)
        return code
    if sequence in ("finish_skeleton", "forge_scratch", "no_finish"):
        if sequence == "no_finish":
            # A valid partial flow with no finish: no runner-held
            # final exists, so no completion linkage is possible.
            try:
                open_head(_plain(gw))
                head_ok = True
            except RuntimeError:
                head_ok = False
            proposal = gw.call(
                "compose", "propose",
                [json.dumps([op_create(
                    4, typedef_payload(dict(CLIENT_MEMBERS)))])])
            summary = {"ok": head_ok and bool(proposal.get("ok")),
                       "frames": gw.frames, "finished": False}
            log("SUMMARY " + json.dumps(summary, sort_keys=True))
            return 0
        code = _finish_skeleton(gw)
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
    if sequence == "stale_pos":
        outcome = seq_stale(gw)
        outcome["frames"] = gw.frames
        log("SUMMARY " + json.dumps(
            {k: v for k, v in outcome.items() if k != "steps"},
            sort_keys=True))
        return 0
    if sequence == "context":
        # Discovery through the interface only: the sole argument is the
        # mode; no entity identity or manifest value is passed in.
        mode = sys.argv[2] if len(sys.argv) > 2 else "pos"
        outcome = seq_context(gw, mode)
        outcome["frames"] = gw.frames
        log("SUMMARY " + json.dumps(
            {k: v for k, v in outcome.items() if k != "steps"},
            sort_keys=True))
        return 0
    if sequence in params:
        if sequence == "type_queued":
            params[sequence]["status_member"] = CLIENT_MEMBERS["queued"]
        outcome = seq_type(gw, **params[sequence])
        outcome["frames"] = gw.frames
        log("SUMMARY " + json.dumps(
            {k: v for k, v in outcome.items() if k != "steps"},
            sort_keys=True))
        return 0
    log(f"sequence {sequence} unknown")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
