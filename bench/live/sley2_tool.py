#!/usr/bin/env python3
"""Model-facing thin CLI over a live Sley 2.0 trial session.

One invocation drives one disposable `sley serve --json` session over the
trial workspace: it imports the staged base pack (privileged seeding, the
same pattern as the endpoint smoke runner), opens a session, executes
exactly one documented command, closes, and prints a JSON result envelope.
No state survives across invocations; the harness records every result.

The agent surface is the frozen eighteen-method allowlist (pinned equal to
the smoke runner's list by test). Request bodies cross as lowercase hex,
validated here; owner bodies stay opaque per the bridge contract — the
tool decodes read responses into JSON views (display only, never a
verdict) and assembles candidate records from structured operations, all
through the pinned oracle/scb1 codecs. It performs no evaluation: an
invalid assembly is refused by the server, and acceptance belongs solely
to the independent trial oracle.

Commands (run from the trial workspace directory):

  inventory                  list served-repo object ids with decoded kinds
  read <entity-hex>          entity.version with a decoded view
  sig <entity-hex>           entity.signature with a decoded view
  revision                   current accepted-head summary
  caps | budgets             session capability / budget views (raw)
  raw <method> <body-hex>    one guarded frame of any allowlisted method
  propose <ops-json>         assemble + create + validate a candidate
  inspect <record-hex>       candidate.inspect with a decoded view
  validate <record-hex>      candidate.validate with a decoded view
  finish <record-hex>        re-validate and write final_candidate.hex

Completion protocol: validate the candidate that carries the fix, then
`finish` with its exact bytes. The oracle judges only final_candidate.hex.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import sys
from pathlib import Path
from typing import Any

from bench.live import sley2_codecs
from bench.sley2.runner import (
    PROFILE_ARGS,
    Endpoint,
    endpoint_offer,
    probe_handshake,
    request_frame,
)


ROOT = Path(__file__).resolve().parents[2]
ARM_ID = "sley_2_0"
PACK_NAME = "base.pack"
FINAL_NAME = "final_candidate.hex"
REPO_DIR = "repo"
REPORT_NAME = "serve-report.json"
PROTOCOL_VERSION = 2
SESSION_TIMEOUT = 120
HEX_64 = re.compile(r"[0-9a-f]{64}\Z")
HEX_ANY = re.compile(r"[0-9a-f]*\Z")

# The frozen eighteen-method surface, pinned equal to the smoke runner's
# allowlist by test. Commit, merge, execute, export, import, report,
# session management, and tests stay outside the agent's reach by
# construction: the dispatcher below has no path that names them.
TOOL_METHODS = (
    "candidate.append",
    "candidate.create",
    "candidate.discard",
    "candidate.inspect",
    "candidate.validate",
    "capsule",
    "compare",
    "entity.signature",
    "entity.version",
    "handle.expand",
    "query.continue",
    "query.restricted",
    "query.root",
    "refs.list",
    "refs.resolve",
    "revision.read",
    "session.budgets",
    "session.capabilities",
)

# Trial principal and candidate expiry: fixed constants, identical for
# every trial, recorded in every transcript. The principal carries no
# grants (empty capability projection, the S3 fixture pattern); authority
# to mutate comes from the task pack's provisioned policy, never from
# caller identity smuggled through the tool. The principal is
# blake3("sley2.live-trial-principal.v1") (system python has no blake3;
# value computed once against the pinned oracle project).
TRIAL_PRINCIPAL = "efc9efb80dbb95f850914c4ff5f713604b1daefa5df4aa8f5ccdb98d2f1728f3"
TRIAL_EXPIRY_MILLIS = 4102444800000


class Sley2ToolError(ValueError):
    """A live sley_2_0 tool request exceeded its exact command boundary."""


def _fail(detail: str) -> None:
    raise Sley2ToolError(f"LIVE_SLEY2_TOOL_INVALID: {detail}")


def _hex(value: str, length: int | None = None) -> bytes:
    if not isinstance(value, str) or len(value) % 2 or HEX_ANY.fullmatch(value) is None:
        _fail("hex")
    if length is not None and len(value) != length:
        _fail("hex length")
    try:
        return bytes.fromhex(value)
    except ValueError as error:
        raise Sley2ToolError(f"LIVE_SLEY2_TOOL_INVALID: hex: {error}") from error


def resolve_binary() -> Path:
    """The bound sley binary: explicit environment binding only, never a
    silent fallback to an unpinned build."""

    raw = os.environ.get("SLEY2_SLEY_BINARY", "")
    if not raw or "\x00" in raw:
        _fail("binary unbound (set SLEY2_SLEY_BINARY)")
    path = Path(raw)
    try:
        if not path.is_file() or path.is_symlink() or not os.access(path, os.X_OK):
            _fail("binary unbound")
    except OSError as error:
        raise Sley2ToolError(f"LIVE_SLEY2_TOOL_INVALID: binary: {error}") from error
    return path


def _parse_record_fields(body: bytes) -> dict[int, bytes]:
    """Framing-only record parse (tag -> raw bytes): display and session
    bookkeeping, never identity or judgment. No digests are computed here."""

    fields: dict[int, bytes] = {}
    position = 0
    if position >= len(body):
        _fail("record")
    count, width = _uvar(body, position)
    position += width
    for _ in range(count):
        tag, width = _uvar(body, position)
        position += width
        size, width = _uvar(body, position)
        position += width
        if position + size > len(body) or tag in fields:
            _fail("record")
        fields[tag] = body[position:position + size]
        position += size
    if position != len(body):
        _fail("record")
    return fields


def _uvar(body: bytes, position: int) -> tuple[int, int]:
    value = 0
    for width in range(1, 10):
        if position >= len(body):
            _fail("uvar")
        byte = body[position + width - 1]
        value |= (byte & 0x7F) << (7 * (width - 1))
        if not byte & 0x80:
            return value, width
    _fail("uvar")
    raise AssertionError("unreachable")


class Session:
    """One disposable serve session over the workspace repository."""

    def __init__(self, sley: Path, workspace: Path, transcript: list[dict[str, Any]],
                 seed_pack: bool = True) -> None:
        self._workspace = Path(workspace).resolve(strict=True)
        self._sley = sley
        repo = self._workspace / REPO_DIR
        if not repo.is_dir() or repo.is_symlink():
            _fail("repository")
        if seed_pack:
            pack = self._workspace / PACK_NAME
            if not pack.is_file() or pack.is_symlink():
                _fail("pack")
            self._pack_hex = pack.read_bytes().hex()
        else:
            self._pack_hex = ""
        self._endpoint = Endpoint(sley, repo, self._workspace / REPORT_NAME, SESSION_TIMEOUT, PROFILE_ARGS)
        self._transcript = transcript
        self._seq = 0
        self._session: str | None = None
        self._head: dict[str, str] = {}
        hello, offered, _ = endpoint_offer(sley)
        if any(name not in offered for name in TOOL_METHODS):
            _fail("endpoint offer")
        self._record({"direction": "hello", "frame": "client"})
        greeting = self._endpoint.send(hello)
        self._record({"direction": "greeting", "kinds": [frame["kind"] for frame in greeting]})
        if greeting[-1]["kind"] != "hello":
            _fail("negotiation")
        if seed_pack:
            self._seed_once()
        opened = self._raw_request("session.open", self._handshake_id(sley), 0)
        self._fail_if(opened, "open")
        self._session = opened["body"]
        if HEX_64.fullmatch(self._session or "") is None:
            _fail("session")
        self._read_head()

    def _seed_once(self) -> None:
        """Import the staged base pack exactly once per workspace: the
        server deterministically refuses a repeated import
        (SESSION_BINDING_INVALID), so the tool records the seeded pack
        digest beside (never inside) the served repository and skips the
        import when it already matches. On the skip path the head
        transaction comes from the accepted-head pointer file. A pack
        change fails closed."""

        marker = self._workspace / ".sley-live-seed"
        digest = hashlib.sha256(self._pack_hex.encode()).hexdigest()
        try:
            if marker.is_file() and not marker.is_symlink() and marker.read_bytes().decode() == digest:
                self._record({"direction": "seed", "seeded": "already"})
                return
        except OSError as error:
            raise Sley2ToolError(f"LIVE_SLEY2_TOOL_INVALID: seed marker: {error}") from error
        imported = self._raw_request("exchange.import", self._pack_hex, 0)
        self._fail_if(imported, "seed")
        try:
            marker.write_text(digest, encoding="utf-8")
        except OSError as error:
            raise Sley2ToolError(f"LIVE_SLEY2_TOOL_INVALID: seed marker: {error}") from error
        self._record({"direction": "seed", "seeded": "imported"})

    def _read_head(self) -> None:
        """Current accepted-head ids through the read-only workspace
        opener (no session, no import): tx, state root, policy root,
        workspace, and schema epoch for envelope assembly."""

        opened = self._raw_request("workspace.open", "")
        self._fail_if(opened, "head")
        summary = _parse_record_fields(bytes.fromhex(opened["body"]))
        for key, tag in (("tx", 1), ("root", 2), ("policy", 3), ("workspace", 4), ("epoch", 5)):
            raw = summary.get(tag, b"")
            if len(raw) != 32:
                _fail("head")
            self._head[key] = raw.hex()

    def _handshake_id(self, sley: Path) -> str:
        report = self._workspace / "probe-report.json"
        import subprocess

        hello, _, _ = endpoint_offer(sley)
        completed = subprocess.run(
            [str(sley), "serve", "--repository", str(self._workspace / REPO_DIR),
             "--json", "--report", str(report), *PROFILE_ARGS],
            input=(json.dumps(hello) + "\n").encode("utf-8"),
            capture_output=True,
            timeout=SESSION_TIMEOUT,
            check=False,
        )
        if completed.returncode != 0:
            _fail("handshake")
        try:
            value = json.loads(report.read_text(encoding="utf-8"))["handshake_id"]
        except (OSError, ValueError, KeyError) as error:
            raise Sley2ToolError(f"LIVE_SLEY2_TOOL_INVALID: handshake: {error}") from error
        if not isinstance(value, str) or HEX_64.fullmatch(value) is None:
            _fail("handshake")
        return value

    def _record(self, entry: dict[str, Any]) -> None:
        self._transcript.append(entry)

    def _fail_if(self, reply: dict[str, Any], what: str) -> None:
        if reply["flags"].get("failed"):
            _fail(what)

    def _raw_request(self, method: str, body_hex: str, request_id: int | None = None, counter: int | None = None) -> dict[str, Any]:
        """Privileged seeding call (import/open/head reads): the agent's
        allowlist never covers these, and dispatch() cannot reach them."""

        _hex(body_hex)
        if request_id is None:
            self._seq += 1
            request_id = self._seq
        frame = request_frame(method, body_hex, self._session, request_id, protocol_version=PROTOCOL_VERSION)
        self._record({"direction": "request", "method": method, "body_sha256": hashlib.sha256(body_hex.encode()).hexdigest()})
        replies = self._endpoint.send(frame)
        out = replies[-1]
        # Bounds travel in the transcript (not bodies): later audits can
        # verify bounded usage (omitted/truncated flags) without bodies.
        bounds = out.get("bounds") if isinstance(out.get("bounds"), dict) else {}
        self._record({"direction": "response", "failed": bool(out["flags"].get("failed")),
                      "body_sha256": hashlib.sha256((out.get("body") or "").encode()).hexdigest(),
                      "omitted": bounds.get("omitted", 0), "truncated": bounds.get("truncated", False),
                      "returned_bytes": bounds.get("returned_bytes", 0)})
        return out

    def _request(self, method: str, body_hex: str, request_id: int | None = None, counter: int | None = None) -> dict[str, Any]:
        if method not in TOOL_METHODS:
            _fail("method")
        return self._raw_request(method, body_hex, request_id, counter)

    def call(self, method: str, body_hex: str) -> dict[str, Any]:
        """One guarded method call; denied methods have no path here."""

        if self._session is None:
            _fail("session")
        return self._request(method, body_hex)

    def side(self, name: str) -> dict[str, Any]:
        """Inventory a frozen MERGE side state (ours/theirs pack) through
        a throwaway session: import, enumerate, close, remove. The trial
        repo is never touched; the side inventory is read-only evidence
        for composing the merged outcome."""

        import shutil
        import tempfile

        pack = self._workspace / f"{name}.pack"
        try:
            if not pack.is_file() or pack.is_symlink():
                _fail("side pack")
        except OSError as error:
            raise Sley2ToolError(f"LIVE_SLEY2_TOOL_INVALID: side: {error}") from error
        with tempfile.TemporaryDirectory(prefix="sley2-side-") as temporary:
            stage = Path(temporary) / "side"
            stage.mkdir(mode=0o700)
            (stage / "repo").mkdir(mode=0o700)
            shutil.copyfile(pack, stage / PACK_NAME)
            side_transcript: list[dict[str, Any]] = []
            side = Session(self._sley, stage, side_transcript)
            try:
                return _inventory_of(side, stage, REPO_DIR)
            finally:
                side.close()

    def close(self) -> None:
        try:
            if self._session is not None:
                self._endpoint.send(request_frame("session.close", "", self._session, self._seq + 1,
                                                  protocol_version=PROTOCOL_VERSION))
        finally:
            self._session = None
            self._endpoint.close()

    @property
    def head(self) -> dict[str, str]:
        return dict(self._head)

    @property
    def workspace(self) -> Path:
        return self._workspace



def _view(session: Session, method: str, body_hex: str) -> dict[str, Any]:
    """A read with its decoded view (display only). Undecodable methods
    return raw hex; failures decode through the failure envelope where
    possible; the view never gates anything."""

    reply = session.call(method, body_hex)
    if reply["flags"].get("failed"):
        try:
            [decoded] = sley2_codecs.run_batch([{"op": "decode_failure", "body": reply["body"]}])
            return {"failed": True, "body": reply.get("body") or "", "decoded": decoded["decoded"]}
        except sley2_codecs.CodecError:
            return {"failed": True, "body": reply.get("body") or ""}
    try:
        [decoded] = sley2_codecs.run_batch([{"op": "decode_response", "method": method, "body": reply["body"]}])
        return {"failed": False, "body": reply["body"], "decoded": decoded["decoded"]}
    except sley2_codecs.CodecError:
        return {"failed": False, "body": reply["body"], "decoded": None}


def _inventory(session: Session, workspace: Path) -> dict[str, Any]:
    """Object ids served from the repository files, with entity identities
    and decoded kinds.

    Content-addressed object files name their own digest; envelopes decode
    through the pinned codecs against the live head epoch. Enumeration
    needs no query method. Undecodable files are skipped, never guessed:
    the server remains the source of truth for every read.
    """
    return _inventory_of(session, workspace, REPO_DIR)


def _inventory_of(session: Session, workspace: Path, repo_name: str) -> dict[str, Any]:
    """Enumerate one served repository by its object files (see
    `_inventory`). Used for the trial repo and, for MERGE trials, for
    frozen side states imported on demand into throwaway repos."""

    entries: list[dict[str, str]] = []
    epoch = session.head["epoch"]
    base = Path(workspace).resolve(strict=True) / repo_name / "objects" / "scb1"
    if base.is_dir():
        for path in sorted(base.rglob("*.scb1")):
            try:
                if path.is_symlink() or not path.is_file():
                    continue
                digest = path.stem
                if len(digest) != 64 or any(ch not in "0123456789abcdef" for ch in digest):
                    continue
                [decoded] = sley2_codecs.run_batch([{
                    "op": "decode_object", "stored": path.read_bytes().hex(), "epoch": epoch,
                }])
                entry = decoded["decoded"]
                entries.append({"object": digest, "entity": entry["entity_id"],
                                "kind": entry["kind"]})
            except (OSError, sley2_codecs.CodecError, KeyError):
                continue
    return {"objects": entries, "count": len(entries)}


def _assemble(session: Session, ops: list[dict[str, Any]]) -> str:
    """Assemble a candidate record from structured operations.

    The tool fills every envelope field mechanically from live session
    state (base ids from the accepted head, empty capability projection
    over the fixed trial principal, the frozen validation profile, a
    fresh nonce, the fixed expiry bound) and derives ExactEntityVersion
    preconditions from live reads. It interprets nothing: op classes,
    targets, and payloads are the agent's, encoded verbatim, and the
    server judges the result."""

    if not isinstance(ops, list) or not ops or len(ops) > 64:
        _fail("operations")
    head = session.head
    nonce = _fresh_nonce()
    assembled_ops: list[dict[str, Any]] = []
    preconditions: list[dict[str, Any]] = []
    create_ordinal = 0
    for index, op in enumerate(ops):
        if not isinstance(op, dict):
            _fail("operation")
        class_name = op.get("class")
        kind = op.get("kind")
        target = op.get("target")
        field_tag = op.get("field_tag")
        payload = op.get("payload")
        if class_name not in {"CreateEntity", "ReplaceEntityVersion", "DeleteEntityBinding",
                              "SetScalarField", "ReplaceTypedField", "RetargetReference",
                              "InsertOrderedChild", "RemoveOrderedChild", "MoveOrderedChild"}:
            _fail("operation class")
        if not isinstance(kind, int):
            _fail("operation shape")
        if class_name == "CreateEntity":
            if target is not None:
                _fail("create target")
        elif not isinstance(target, str):
            _fail("operation shape")
        if class_name == "CreateEntity":
            # Fresh identities derive from the record nonce, which only
            # exists inside assembly: the agent must not supply one.
            # The server numbers CreateEntity targets with a per-create
            # ordinal, so the tool derives with the same counter.
            if target is not None:
                _fail("create target")
            [derived] = sley2_codecs.run_batch([{
                "op": "derive_entity", "workspace": head["workspace"],
                "nonce": nonce, "kind": kind, "ordinal": create_ordinal,
            }])
            target = derived["entity"]
            create_ordinal += 1
        entry: dict[str, Any] = {
            "ordinal": index,
            "class": class_name,
            "target_kind": kind,
            "field_tag": field_tag,
            "target_entity": target,
            "precondition_ordinal": index,
            "payload": payload,
        }
        assembled_ops.append(entry)
        if class_name == "CreateEntity":
            preconditions.append({
                "operation_ordinal": index,
                "requirement": "ExpectedIdentityAbsent",
                "payload": {"entity_id": target},
            })
            continue
        current = session.call("entity.version", _entity_body(session, target))
        if current["flags"].get("failed"):
            _fail("precondition read")
        [decoded] = sley2_codecs.run_batch([{
            "op": "decode_response", "method": "entity.version", "body": current["body"],
        }])
        entries = decoded["decoded"].get("entries") or []
        if len(entries) != 1:
            _fail("precondition read")
        precondition: dict[str, Any] = {
            "operation_ordinal": index,
            "payload": {"entity_id": target, "object_id": entries[0]["object_id"]},
        }
        if field_tag is None:
            precondition["requirement"] = "ExactEntityVersion"
        elif class_name in ("InsertOrderedChild", "RemoveOrderedChild", "MoveOrderedChild"):
            # Ordered-child mutations bind the container version; every
            # other field mutation binds the entity version (frozen
            # descriptor inventory).
            precondition["requirement"] = "ExactContainerVersion"
            precondition["payload"] = {
                "container_id": target,
                "object_id": entries[0]["object_id"],
                "field_tag": field_tag,
            }
        else:
            precondition["requirement"] = "ExactEntityVersion"
        preconditions.append(precondition)
    [capability] = sley2_codecs.run_batch([{
        "op": "capability_digest", "principal": TRIAL_PRINCIPAL, "workspace": head["workspace"],
        "policy_root": head["policy"], "state_root": head["root"],
    }])
    [profile] = sley2_codecs.run_batch([{"op": "validation_profile"}])
    [record] = sley2_codecs.run_batch([{
        "op": "assemble_record",
        "record": {
            "format_version": 1,
            "workspace_id": head["workspace"],
            "base_transaction_id": head["tx"],
            "base_root": head["root"],
            "schema_epoch_id": head["epoch"],
            "policy_root_id": head["policy"],
            "principal_id": TRIAL_PRINCIPAL,
            "capability_summary_digest": capability["digest"],
            "operations": assembled_ops,
            "preconditions": preconditions,
            "validation_profile_id": profile["profile"],
            "candidate_nonce": nonce,
            "expiry": {"clock": 1, "not_after": TRIAL_EXPIRY_MILLIS},
        },
    }])
    return record["record"]


def _stored_from_record(record_hex: str) -> str:
    """Stored candidate bytes for a record: magic, version, sized record,
    and the pinned digest. The validate response carries a result object,
    not stored bytes, so the digest comes from the pinned implementation
    (never recomputed by hand)."""

    [stored] = sley2_codecs.run_batch([{"op": "stored_from_record", "record": record_hex}])
    return stored["stored"]


def _fresh_nonce() -> str:
    import secrets

    return secrets.token_hex(32)


def dispatch(session: Session, workspace: Path, argv: list[str]) -> dict[str, Any]:
    """Dispatch one documented command; anything else is refused."""

    if not argv:
        _fail("command")
    command, rest = argv[0], argv[1:]
    if command == "inventory" and not rest:
        return {"inventory": _inventory(session, workspace)}
    if command == "side" and len(rest) == 1 and rest[0] in ("ours", "theirs"):
        return {"side": session.side(rest[0])}
    if command == "read" and len(rest) == 1:
        return _view(session, "entity.version", _entity_body(session, _hex(rest[0], 64).hex()))
    if command == "sig" and len(rest) == 1:
        return _view(session, "entity.signature", _entity_body(session, _hex(rest[0], 64).hex()))
    if command == "revision" and not rest:
        return _view(session, "revision.read", session.head["tx"])
    if command == "caps" and not rest:
        return _view(session, "session.capabilities", "")
    if command == "budgets" and not rest:
        return _view(session, "session.budgets", "")
    if command == "raw" and len(rest) == 2 and rest[0] in TOOL_METHODS:
        reply = session.call(rest[0], _hex(rest[1]).hex())
        return {"failed": bool(reply["flags"].get("failed")), "body": reply.get("body") or ""}
    if command in ("inspect", "validate") and len(rest) == 1:
        return _view(session, "candidate." + command, _hex(rest[0]).hex())
    if command == "propose" and len(rest) == 1:
        try:
            ops = json.loads(rest[0])
        except json.JSONDecodeError as error:
            raise Sley2ToolError(f"LIVE_SLEY2_TOOL_INVALID: ops JSON: {error}") from error
        record_hex = _assemble(session, ops)
        created = session.call("candidate.create", record_hex)
        if created["flags"].get("failed"):
            return {"created": False, "record": record_hex, "body": created.get("body") or ""}
        validated = session.call("candidate.validate", _validate_body(session, record_hex))
        if validated["flags"].get("failed"):
            return {"created": True, "record": record_hex,
                    "valid": False, "body": validated.get("body") or ""}
        stored = _stored_from_record(record_hex)
        return {"created": True, "record": record_hex, "stored": stored,
                "valid": True, "body": validated.get("body") or ""}
    if command == "finish" and len(rest) == 1:
        record_hex = _hex(rest[0]).hex()
        validated = session.call("candidate.validate", _validate_body(session, record_hex))
        if validated["flags"].get("failed"):
            return {"finished": False, "body": validated.get("body") or ""}
        stored = _stored_from_record(record_hex)
        target = workspace / FINAL_NAME
        if target.exists() or target.is_symlink():
            _fail("finish exists")
        target.write_text(stored + "\n", encoding="utf-8")
        digest = hashlib.sha256(bytes.fromhex(stored)).hexdigest()
        return {"finished": True, "sha256": digest}
    _fail("command")
    raise AssertionError("unreachable")


def _encode_uvar(value: int) -> bytes:
    """Framing-only unsigned varint (display/session use, never identity)."""

    if not isinstance(value, int) or value < 0:
        _fail("uvar")
    encoded = bytearray()
    while True:
        group = value & 0x7F
        value >>= 7
        encoded.append(group | (0x80 if value else 0))
        if not value:
            return bytes(encoded)


def _encode_sized(value: bytes) -> bytes:
    return _encode_uvar(len(value)) + value


def _encode_record(fields: list[tuple[int, bytes]]) -> bytes:
    return _encode_uvar(len(fields)) + b"".join(
        _encode_uvar(tag) + _encode_sized(value) for tag, value in fields
    )


# Entity-read bounds: fixed documented constants identical for every
# trial (limits shape the bounded context, never acceptance).
READ_MAX_OBJECTS = 1024
READ_MAX_BYTES = 8388608
READ_MAX_WORK = 100000000


def _entity_body(session: Session, entity_hex: str) -> str:
    """The entity.version/signature request body: state root, entity,
    and fixed bounds (the test-harness shape, not agent input)."""

    return _encode_record([
        (1, bytes.fromhex(session.head["root"])),
        (2, bytes.fromhex(entity_hex)),
        (3, _encode_uvar(READ_MAX_OBJECTS)),
        (4, _encode_uvar(READ_MAX_BYTES)),
        (5, _encode_uvar(READ_MAX_WORK)),
    ]).hex()


def _validate_body(session: Session, record_hex: str) -> str:
    """The candidate.validate request body: base, principal, now, bytes.

    `now` is the wallclock at call time, recorded in the transcript; the
    fixed far-future expiry always exceeds it, so identical trials
    validate identically. Record layout mirrors the server's four-field
    validate input."""

    import time

    now = int(time.time() * 1000)
    head = session.head
    return _encode_record([
        (1, bytes.fromhex(head["tx"])),
        (2, bytes.fromhex(TRIAL_PRINCIPAL)),
        (3, _encode_uvar(now)),
        (4, bytes.fromhex(record_hex)),
    ]).hex()


def main(argv: list[str] | None = None) -> int:
    arguments = sys.argv[1:] if argv is None else argv
    transcript: list[dict[str, Any]] = []
    workspace = Path.cwd()
    try:
        session = Session(resolve_binary(), workspace, transcript)
        try:
            result = dispatch(session, workspace, arguments)
        finally:
            session.close()
        digest = hashlib.sha256(json.dumps(transcript, sort_keys=True).encode("utf-8")).hexdigest()
        print(json.dumps({"command": arguments[:1], "outcome": "completed", "return_code": 0,
                          "report": result, "transcript_sha256": digest,
                          "stderr_text": "", "stdout_bytes": "", "stderr_bytes": "",
                          "truncated": False}, sort_keys=True))
        return 0
    except (Sley2ToolError, sley2_codecs.CodecError, OSError, ValueError) as error:
        print(json.dumps({"code": "LIVE_SLEY2_TOOL_INVALID", "detail": str(error)[:500]}, sort_keys=True))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
