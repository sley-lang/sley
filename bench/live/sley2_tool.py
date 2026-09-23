#!/usr/bin/env python3
"""Model-facing thin CLI over a live Sley 2.0 trial session.

One invocation drives one disposable `sley serve --json` session over the
trial workspace: it imports the staged base pack (privileged seeding, the
same pattern as the endpoint smoke runner), opens a session, executes
exactly one documented command, closes, and prints a JSON result envelope.
No state survives across invocations; the harness records every result.

The agent surface is the frozen nineteen-method allowlist, equal in tuple
order to the smoke runner's `ARM_AFFORDANCES` (asserted by
`test_tool_methods_equal_runner_allowlist_in_order`). Request bodies
cross as lowercase hex, validated here; owner bodies stay opaque per the
bridge contract — the tool decodes read responses into JSON views
(display only, never a verdict) and assembles candidate records from structured operations, all
through the pinned oracle/scb1 codecs. It performs no evaluation: an
invalid assembly is refused by the server, and acceptance belongs solely
to the independent trial oracle.

Commands (run from the trial workspace directory):

  inventory                  list served-repo object ids with decoded kinds
  read <entity-hex>          entity.version with a decoded view
  sig <entity-hex>           entity.signature with a decoded view
  open                       workspace.open: accepted-head summary, plus the
                             head's materialized index snapshot id when present
  revision <tx-hex>          revision.read of a transaction id (e.g. from open)
  caps | budgets             session capability / budget views (raw)
  side ours|theirs           frozen MERGE side state with decoded bodies
  raw <method> <body-hex>    one guarded frame of any allowlisted method
   propose <ops-json>         assemble + create + validate a candidate
   append <record-hex> <ops-json>
                            extend a proposed record through candidate.append
   compose <record-hex> <ops-json>
                            reassemble a full op list under a base nonce
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
USAGE_NAME = ".sley-live-usage"
CHAIN_NAME = ".sley-live-transcript.jsonl"
# Agent/tool boundary version, pinned by the judge: every chained entry
# carries it, and access evidence from any other version is unverifiable.
# "2" (trial-runner contract revision 5): the `open` command, `revision
# <tx>`, the nineteen-method raw set, and continuation bindings in the
# transcript; evidence stamped "1" is the revision 4 surface and rejects.
TOOL_VERSION = "2"
# Stated per-call agent-visible response bound (frozen S3 CONTEXT ceiling):
# no single response the agent sees may exceed it.
MAX_RESPONSE_BYTES = 1048576
# Whole-store enumeration methods: each call reads every served object.
WHOLE_STORE_METHODS = frozenset({"inventory"})
# Continuation machinery: bounded paging with explicit omitted/truncated
# accounting, never silent drops.
CONTINUATION_METHODS = frozenset({"query.continue", "query.root", "query.restricted"})
# Root-backed query methods whose answered pages carry a continuation
# binding (`_root_query_chain`): the judge discharges a truncated page only
# with a successful `query.continue` of the same query at that page's next
# cursor.
ROOT_QUERY_METHODS = frozenset({"query.root", "query.continue"})
PROTOCOL_VERSION = 2
SESSION_TIMEOUT = 120
HEX_64 = re.compile(r"[0-9a-f]{64}\Z")
HEX_ANY = re.compile(r"[0-9a-f]*\Z")

# The frozen nineteen-method surface, in the smoke runner's
# `ARM_AFFORDANCES` tuple order (contract revision 5; the pin test
# `test_tool_methods_equal_runner_allowlist_in_order` asserts the
# equality). Commit, merge, execute, export, import, report, session
# management, and tests stay outside the agent's reach by construction:
# the dispatcher below has no path that names them.
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
    "workspace.open",
)
# Revision summaries (revision.read, workspace.open) share one record
# layout; the decoded view names its fields (display only). Field 9 is the
# accepted head's materialized index snapshot identity, carried by
# workspace.open only and structurally absent when not materialized.
SUMMARY_METHODS = frozenset({"revision.read", "workspace.open"})
SUMMARY_ID_FIELDS = ((1, "tx"), (2, "root"), (3, "policy"), (4, "workspace"),
                     (5, "epoch"), (8, "receipt"), (9, "snapshot"))
SUMMARY_COUNT_FIELDS = ((6, "objects"), (7, "tombstones"))

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


# Root query wire offsets (SMP1 appendix A rows 300/301; ROOT_BACKED
# profile sections 5-6): the request cursor starts after the fixed request
# prefix, the response echo cursor after the fixed response prefix.
_RQ_PREFIX = 8 + 4 + 4 + 4 * 32 + 4 + 4 + (8 + 8 + 4 + 8 + 8) + 4
_RR_PREFIX = 8 + 4 + 4 + 32 + 4 * 32 + 4 + 4 + (8 + 8 + 4 + 8 + 8) + 4
_CURSOR_PAYLOAD = {1: 32, 2: 68, 3: 32}


def _cursor_span(data: bytes, at: int) -> tuple[str | None, int]:
    """(cursor token, end offset) of an option cursor at `at`: None for
    none, else "<kind>:<payload hex>"."""

    if at + 4 > len(data):
        raise ValueError("cursor")
    tag = int.from_bytes(data[at:at + 4], "big")
    if tag == 1:
        return None, at + 4
    if tag != 2 or at + 8 > len(data):
        raise ValueError("cursor")
    kind = int.from_bytes(data[at + 4:at + 8], "big")
    size = _CURSOR_PAYLOAD.get(kind)
    if size is None or at + 8 + size > len(data):
        raise ValueError("cursor")
    return f"{kind}:{data[at + 8:at + 8 + size].hex()}", at + 8 + size


def _root_query_chain(request_hex: str, response_hex: str) -> dict[str, Any] | None:
    """Continuation binding of one answered root query, derived on the
    trusted side from the exact request and response bodies: `query` is the
    sha256 of the request with its cursor elided (same class, body, limits,
    paging, and head binding), `after` the request cursor, `truncated` and
    `next` the response's own flag and next cursor. None when a body does
    not parse (such a page can never discharge a continuation)."""

    try:
        request = bytes.fromhex(request_hex)
        response = bytes.fromhex(response_hex)
        if request[:8] != b"SLEYRQQ1" or response[:8] != b"SLEYRQR1":
            return None
        after, end = _cursor_span(request, _RQ_PREFIX)
        key = hashlib.sha256(request[:_RQ_PREFIX] + request[end:]).hexdigest()
        _, echo_end = _cursor_span(response, _RR_PREFIX)
        at = echo_end + 4 + 8 + 8
        if at + 4 > len(response):
            return None
        truncated = {1: False, 2: True}.get(int.from_bytes(response[at:at + 4], "big"))
        if truncated is None:
            return None
        following, _ = _cursor_span(response, at + 4)
    except ValueError:
        return None
    return {"query": key, "after": after, "truncated": truncated, "next": following}


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
        failed = bool(out["flags"].get("failed"))
        entry = {"direction": "response", "failed": failed,
                 "body_sha256": hashlib.sha256((out.get("body") or "").encode()).hexdigest(),
                 "omitted": bounds.get("omitted", 0), "truncated": bounds.get("truncated", False),
                 "returned_bytes": bounds.get("returned_bytes", 0)}
        if method in ROOT_QUERY_METHODS and not failed:
            entry["chain"] = _root_query_chain(body_hex, out.get("body") or "")
        self._record(entry)
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
        """Read a frozen MERGE side state (ours/theirs pack) through a
        throwaway session: import, resolve CURRENT versions with
        decoded bodies, close, remove. The trial repo is never touched.

        Currency comes from live entity.version bindings under the side
        head, never file order: packs retain stale object versions
        beside current ones, and a branch read must not resurrect them.
        Tombstoned entities drop out. The side state is read-only
        evidence for composing the merged outcome."""

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
                return _side_current(side, stage)
            finally:
                side.close()
                # Side sessions are agent-visible work: their transcript
                # joins this invocation's evidence, marked by scope.
                for entry in side_transcript:
                    marked = dict(entry)
                    marked["scope"] = f"side:{name}"
                    self._transcript.append(marked)

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
    if method in SUMMARY_METHODS:
        return {"failed": False, "body": reply["body"],
                "decoded": _summary_view(reply["body"])}
    try:
        [decoded] = sley2_codecs.run_batch([{"op": "decode_response", "method": method, "body": reply["body"]}])
        return {"failed": False, "body": reply["body"], "decoded": decoded["decoded"]}
    except sley2_codecs.CodecError:
        return {"failed": False, "body": reply["body"], "decoded": None}


def _summary_view(body_hex: str) -> dict[str, Any] | None:
    """Framing-only view of a revision summary (display, never a verdict):
    32-byte identities as hex, counts as integers. Field 9 appears only
    when the response carries it; nothing is inferred for its absence."""

    try:
        fields = _parse_record_fields(bytes.fromhex(body_hex))
    except (Sley2ToolError, ValueError):
        return None
    view: dict[str, Any] = {}
    for tag, name in SUMMARY_ID_FIELDS:
        if tag in fields:
            view[name] = fields[tag].hex()
    for tag, name in SUMMARY_COUNT_FIELDS:
        if tag in fields:
            try:
                value, width = _uvar(fields[tag], 0)
            except Sley2ToolError:
                return None
            if width != len(fields[tag]):
                return None
            view[name] = value
    return view


def _side_current(session: Session, workspace: Path) -> dict[str, Any]:
    """Current side-state entities with decoded bodies (see
    Session.side). Distinct file entities resolve through live
    entity.version bindings; stale duplicates collapse and tombstones
    drop out. Small frozen packs: direct reads, failures skip."""

    seen: set[str] = set()
    for path in sorted((Path(workspace).resolve(strict=True) / REPO_DIR
                        / "objects" / "scb1").rglob("*.scb1")):
        try:
            if path.is_symlink() or not path.is_file():
                continue
            [decoded] = sley2_codecs.run_batch([{
                "op": "decode_object",
                "stored": path.read_bytes().hex(),
                "epoch": session.head["epoch"],
            }])
            entity = decoded["decoded"].get("entity_id", "")
            if entity:
                seen.add(str(entity))
        except (OSError, sley2_codecs.CodecError, KeyError):
            continue
    entries: list[dict[str, Any]] = []
    for entity in sorted(seen):
        try:
            reply = session.call("entity.version", _entity_body(session, entity))
        except Sley2ToolError:
            continue
        if reply["flags"].get("failed"):
            continue
        try:
            [decoded] = sley2_codecs.run_batch([{
                "op": "decode_response", "method": "entity.version",
                "body": reply["body"],
            }])
        except (sley2_codecs.CodecError, KeyError):
            continue
        items = decoded["decoded"].get("entries") or []
        if len(items) != 1:
            continue
        object_id = items[0].get("object_id", "")
        body = items[0].get("body")
        if not object_id or not isinstance(body, dict):
            continue
        entries.append({"object": object_id, "entity": entity,
                        "kind": items[0].get("kind"), "body": body})
    return {"objects": entries, "count": len(entries)}


def _inventory(session: Session, workspace: Path) -> dict[str, Any]:
    """Object ids served from the repository files, with entity identities
    and decoded kinds.

    Content-addressed object files name their own digest; envelopes decode
    through the pinned codecs against the live head epoch. Enumeration
    needs no query method. Undecodable files are skipped, never guessed:
    the server remains the source of truth for every read.
    """
    return _inventory_of(session, workspace, REPO_DIR)


def _inventory_of(session: Session, workspace: Path, repo_name: str,
                    include_bodies: bool = False) -> dict[str, Any]:
    """Enumerate one served repository by its object files (see
    `_inventory`). Used for the trial repo and, for MERGE trials, for
    frozen side states imported on demand into throwaway repos.

    Side inventories include decoded bodies (display only, the same
    views as `read`): the branch contents are legitimate trial inputs,
    and the merge must be authored through the allowed interface, not
    through raw pack reads. Trial-repo inventories stay id-only."""

    entries: list[dict[str, Any]] = []
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
                item: dict[str, Any] = {"object": digest, "entity": entry["entity_id"],
                                        "kind": entry["kind"]}
                if include_bodies:
                    item["body"] = entry.get("body")
                entries.append(item)
            except (OSError, sley2_codecs.CodecError, KeyError):
                continue
    return {"objects": entries, "count": len(entries)}


def _assemble(session: Session, ops: list[dict[str, Any]], *,
              nonce: str | None = None) -> str:
    """Assemble a candidate record from structured operations.

    The tool fills every envelope field mechanically from live session
    state (base ids from the accepted head, empty capability projection
    over the fixed trial principal, the frozen validation profile, the
    candidate nonce, the fixed expiry bound) and derives
    ExactEntityVersion preconditions from live reads. It interprets
    nothing: op classes, targets, and payloads are the agent's, encoded
    verbatim, and the server judges the result.

    A fresh nonce starts a record (`propose`, `append` additions);
    passing a base record's own nonce reassembles its full op list
    (`compose`), so CreateEntity identities re-derive deterministically
    and earlier identities are preserved byte-for-byte. Ordinals always
    run contiguously from zero: the frozen record rules admit no other
    shape, so there are no offset parameters.
    """

    if not isinstance(ops, list) or not ops or len(ops) > 64:
        _fail("operations")
    head = session.head
    if nonce is None:
        nonce = _fresh_nonce()
    else:
        _hex(nonce, 64)
    assembled_ops: list[dict[str, Any]] = []
    preconditions: list[dict[str, Any]] = []
    read_targets: list[tuple[int, str, int, str, Any]] = []
    create_ordinal = 0
    for index, op in enumerate(ops):
        ordinal = index
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
            "ordinal": ordinal,
            "class": class_name,
            "target_kind": kind,
            "field_tag": field_tag,
            "target_entity": target,
            "precondition_ordinal": ordinal,
            "payload": payload,
        }
        assembled_ops.append(entry)
        if class_name == "CreateEntity":
            preconditions.append({
                "operation_ordinal": ordinal,
                "requirement": "ExpectedIdentityAbsent",
                "payload": {"entity_id": target},
            })
            continue
        read_targets.append((ordinal, class_name, kind, target, field_tag))
    # Precondition bindings resolve through live version reads in small
    # chunks across fresh sessions (frozen per-session request limit):
    # each chunk shares the invocation transcript, so every mechanical
    # read stays in the chained evidence.
    bindings: dict[int, str] = {}
    for chunk_start in range(0, len(read_targets), 8):
        probe = Session(session._sley, session._workspace,
                        session._transcript, seed_pack=False)
        try:
            for ordinal, _class, _kind, target, _tag in read_targets[chunk_start:chunk_start + 8]:
                current = probe.call("entity.version", _entity_body(probe, target))
                if current["flags"].get("failed"):
                    _fail("precondition read")
                [decoded] = sley2_codecs.run_batch([{
                    "op": "decode_response", "method": "entity.version",
                    "body": current["body"],
                }])
                entries = decoded["decoded"].get("entries") or []
                if len(entries) != 1:
                    _fail("precondition read")
                bindings[ordinal] = entries[0]["object_id"]
        finally:
            probe.close()
    for ordinal, class_name, kind, target, field_tag in read_targets:
        object_id = bindings.get(ordinal, "")
        if not object_id:
            _fail("precondition read")
        precondition: dict[str, Any] = {
            "operation_ordinal": ordinal,
            "payload": {"entity_id": target, "object_id": object_id},
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
                "object_id": object_id,
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


def _decide(result_body_hex: str) -> tuple[bool, dict[str, Any]]:
    """Read the validation verdict: valid only when the decision tag is
    1 (Valid). A delivered result object is not acceptance — negative
    decisions arrive with failed=false at the protocol level."""

    [decoded] = sley2_codecs.run_batch([{"op": "decode_result", "body": result_body_hex}])
    decision = {"tag": decoded["decision_tag"], "failed_phase": decoded["failed_phase"]}
    return decoded["decision_tag"] == 1, decision


def _created_identities(record_hex: str) -> list[dict[str, Any]]:
    """Created-entity identities in an assembled record, for later phases
    to reference in new payloads: deterministic derivations from the
    record's own nonce, reported on every created record whether or not
    the record validates — derivation is not a validity claim (the
    `valid`/`decision` fields carry that), and only Valid records can
    finish. The server re-checks everything at each create and at
    finish time."""

    [described] = sley2_codecs.run_batch([{"op": "describe_record", "record": record_hex}])
    return [{"ordinal": ordinal, "entity": entity}
            for ordinal, class_name, entity in zip(described["ordinals"],
                                                  described["classes"],
                                                  described["targets"])
            if class_name == "CreateEntity"]


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
    if command == "open" and not rest:
        # The agent's own accepted-head opener (contract revision 5): a
        # guarded, transcript-captured workspace.open. The harness head
        # read in Session.__init__ stays harness bookkeeping.
        return _view(session, "workspace.open", "")
    if command == "revision" and len(rest) == 1:
        # The transaction id is agent-supplied (for instance the tx of a
        # prior `open`); no harness-held head is read here.
        return _view(session, "revision.read", _hex(rest[0], 64).hex())
    if command == "caps" and not rest:
        return _view(session, "session.capabilities", "")
    if command == "budgets" and not rest:
        return _view(session, "session.budgets", "")
    if command == "raw" and len(rest) == 2 and rest[0] in TOOL_METHODS:
        reply = session.call(rest[0], _hex(rest[1]).hex())
        return {"failed": bool(reply["flags"].get("failed")), "body": reply.get("body") or ""}
    if command in ("inspect", "validate") and len(rest) == 1:
        return _view(session, "candidate." + command, _hex(rest[0]).hex())
    if command == "append" and len(rest) == 2:
        # Thin adapter over the server's candidate.append, following its
        # actual contract: the base is server-stored bytes, the addition
        # is a standalone record, and the server rebuilds the
        # concatenation. Composition is proposal-only; the accepted head
        # moves only through finish. (Finding, evidenced in tests: under
        # frozen ordinal rules the concatenated record cannot validate —
        # both sides must number from zero, so the join always collides.
        # Admissible multi-phase composition goes through `compose`.)
        base_record = _hex(rest[0]).hex()
        try:
            ops = json.loads(rest[1])
        except json.JSONDecodeError as error:
            raise Sley2ToolError(f"LIVE_SLEY2_TOOL_INVALID: ops JSON: {error}") from error
        [described] = sley2_codecs.run_batch([{"op": "describe_record", "record": base_record}])
        if described["ordinals"] != list(range(described["op_count"])):
            _fail("base ordinals")
        addition = _assemble(session, ops)
        [stored_base] = sley2_codecs.run_batch([{"op": "stored_from_record", "record": base_record}])
        append_body = _encode_record([
            (1, bytes.fromhex(stored_base["stored"])),
            (2, bytes.fromhex(addition)),
        ]).hex()
        head_before = session.head["tx"]
        appended = session.call("candidate.append", append_body)
        if appended["flags"].get("failed"):
            if session.head["tx"] != head_before:
                _fail("head moved on rejected append")
            return {"appended": False, "body": appended.get("body") or ""}
        stored = appended.get("body") or ""
        [unwrapped] = sley2_codecs.run_batch([{"op": "record_from_stored", "stored": stored}])
        record_hex = unwrapped["record"]
        validated = session.call("candidate.validate", _validate_body(session, stored))
        if validated["flags"].get("failed"):
            return {"appended": True, "record": record_hex, "stored": stored,
                    "valid": False, "body": validated.get("body") or ""}
        valid, decision = _decide(validated.get("body") or "")
        return {"appended": True, "record": record_hex, "stored": stored,
                "valid": valid, "decision": decision,
                "body": validated.get("body") or ""}
    if command == "compose" and len(rest) == 2:
        # Contract-valid multi-phase composition: reassemble the FULL op
        # list (earlier phases resupplied verbatim, new ops appended)
        # under the BASE record's own nonce, then create + validate
        # through the real production admission path. Create identities
        # re-derive deterministically under the same nonce and counter,
        # so earlier identities are preserved byte-for-byte, never
        # silently changed; the server re-checks everything, including
        # that preservation, at create and validate time.
        base_record = _hex(rest[0]).hex()
        try:
            ops = json.loads(rest[1])
        except json.JSONDecodeError as error:
            raise Sley2ToolError(f"LIVE_SLEY2_TOOL_INVALID: ops JSON: {error}") from error
        [described] = sley2_codecs.run_batch([{"op": "describe_record", "record": base_record}])
        if described["ordinals"] != list(range(described["op_count"])):
            _fail("base ordinals")
        record_hex = _assemble(session, ops, nonce=described["nonce"])
        created = session.call("candidate.create", record_hex)
        if created["flags"].get("failed"):
            return {"created": False, "record": record_hex, "body": created.get("body") or ""}
        stored = _stored_from_record(record_hex)
        validated = session.call("candidate.validate", _validate_body(session, stored))
        if validated["flags"].get("failed"):
            return {"created": True, "record": record_hex,
                    "valid": False, "body": validated.get("body") or ""}
        valid, decision = _decide(validated.get("body") or "")
        composed: dict[str, Any] = {"created": True, "record": record_hex, "stored": stored,
                                    "valid": valid, "decision": decision,
                                    "body": validated.get("body") or "",
                                    "identities": _created_identities(record_hex)}
        return composed
    if command == "propose" and len(rest) == 1:
        try:
            ops = json.loads(rest[0])
        except json.JSONDecodeError as error:
            raise Sley2ToolError(f"LIVE_SLEY2_TOOL_INVALID: ops JSON: {error}") from error
        record_hex = _assemble(session, ops)
        created = session.call("candidate.create", record_hex)
        if created["flags"].get("failed"):
            return {"created": False, "record": record_hex, "body": created.get("body") or ""}
        stored = _stored_from_record(record_hex)
        validated = session.call("candidate.validate", _validate_body(session, stored))
        if validated["flags"].get("failed"):
            return {"created": True, "record": record_hex,
                    "valid": False, "body": validated.get("body") or ""}
        valid, decision = _decide(validated.get("body") or "")
        report: dict[str, Any] = {"created": True, "record": record_hex, "stored": stored,
                                  "valid": valid, "decision": decision,
                                  "body": validated.get("body") or "",
                                  "identities": _created_identities(record_hex)}
        return report
    if command == "finish" and len(rest) == 1:
        record_hex = _hex(rest[0]).hex()
        stored = _stored_from_record(record_hex)
        validated = session.call("candidate.validate", _validate_body(session, stored))
        if validated["flags"].get("failed"):
            return {"finished": False, "body": validated.get("body") or ""}
        valid, decision = _decide(validated.get("body") or "")
        if not valid:
            return {"finished": False, "decision": decision,
                    "body": validated.get("body") or ""}
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


def _validate_body(session: Session, stored_hex: str) -> str:
    """The candidate.validate request body: base, principal, now, stored
    bytes. Field 4 carries STORED bytes (magic, version, sized record,
    digest trailer) — the same bytes the commit path checks — never the
    bare record: validating record bytes always decides InvalidEncoding.

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
        (4, bytes.fromhex(stored_hex)),
    ]).hex()


def _record_usage(workspace: Path, command: str, ok: bool,
                  wall_ms: int, response_bytes: int) -> dict[str, int]:
    """Cumulative trial-wide accounting beside (never inside) the served
    repository: every invocation appends its cost, so later phases cannot
    reset context/action/wall accounting. Supporting evidence only — the
    campaign's own budgets still gate the trial, and the judge
    re-derives usage from transcripts."""

    ledger = workspace / USAGE_NAME
    totals = {"invocations": 0, "wall_ms": 0, "response_bytes": 0}
    try:
        if ledger.is_file() and not ledger.is_symlink():
            loaded = json.loads(ledger.read_text(encoding="utf-8"))
            prior = loaded.get("totals") if isinstance(loaded, dict) else None
            if isinstance(prior, dict):
                for key in totals:
                    value = prior.get(key)
                    if isinstance(value, int) and value >= 0:
                        totals[key] = value
    except (OSError, ValueError):
        totals = {"invocations": 0, "wall_ms": 0, "response_bytes": 0}
    totals["invocations"] += 1
    totals["wall_ms"] += wall_ms
    totals["response_bytes"] += response_bytes
    entry = {"command": command, "ok": ok, "wall_ms": wall_ms,
             "response_bytes": response_bytes, "totals": dict(totals)}
    try:
        ledger.write_text(json.dumps(entry, sort_keys=True), encoding="utf-8")
    except OSError as error:
        raise Sley2ToolError(f"LIVE_SLEY2_TOOL_INVALID: usage: {error}") from error
    return totals


def _pack_digest(workspace: Path) -> str:
    """Exact staged fixture binding: sha256 of the base pack bytes, or
    "none" for pack-less trials (CREATE starts blank)."""

    pack = workspace / PACK_NAME
    try:
        if pack.is_file() and not pack.is_symlink():
            return hashlib.sha256(pack.read_bytes()).hexdigest()
    except OSError as error:
        raise Sley2ToolError(f"LIVE_SLEY2_TOOL_INVALID: pack digest: {error}") from error
    return "none"


def _binary_digest() -> str:
    """Bound sley binary identity: sha256 of the executable bytes.

    Every transcript entry binds the exact binary that served the trial,
    so evidence from any other build is unverifiable. Fails closed when
    the binary is unbound.
    """

    path = resolve_binary()
    try:
        return hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError as error:
        raise Sley2ToolError(f"LIVE_SLEY2_TOOL_INVALID: binary digest: {error}") from error


def _command_evidence(argv: list[str], result: dict[str, Any] | None) -> tuple[dict[str, Any], dict[str, str | None]]:
    """Agent-visible shape of one command for the chained evidence: what
    was inspected or authored, and which candidate records moved."""

    args: dict[str, Any] = {}
    records: dict[str, str | None] = {"in": None, "out": None}
    if not argv:
        return args, records
    command, rest = argv[0], argv[1:]
    if command in ("read", "sig") and len(rest) == 1:
        args = {"entities": [rest[0]]}
    elif command == "inventory" and not rest:
        args = {"entities": ["all"]}
    elif command == "raw" and len(rest) == 2:
        args = {"method": rest[0], "body_bytes": len(rest[1]) // 2}
    elif command == "propose" and len(rest) == 1 and result is not None:
        try:
            ops = json.loads(rest[0])
            args = {"targets": [op.get("target") if isinstance(op, dict) else None
                                for op in ops] if isinstance(ops, list) else [],
                    "classes": [op.get("class") if isinstance(op, dict) else None
                                for op in ops] if isinstance(ops, list) else [],
                    "op_count": len(ops) if isinstance(ops, list) else 0}
        except json.JSONDecodeError:
            args = {"targets": [], "classes": [], "op_count": 0}
        record = result.get("record")
        records["out"] = hashlib.sha256(record.encode()).hexdigest() if isinstance(record, str) else None
    elif command == "compose" and len(rest) == 2 and result is not None:
        # Compose takes (base_record_hex, ops_json): the input candidate,
        # the resupplied full op list, and the output candidate are all
        # bound here. Nonce reuse alone is NOT preservation evidence: the
        # supplied op list must resupply earlier creates verbatim, and
        # preservation is established only by target-identity equality
        # across the transition (tested), never by nonce equality.
        try:
            ops = json.loads(rest[1])
            args = {"targets": [op.get("target") if isinstance(op, dict) else None
                                for op in ops] if isinstance(ops, list) else [],
                    "classes": [op.get("class") if isinstance(op, dict) else None
                                for op in ops] if isinstance(ops, list) else [],
                    "op_count": len(ops) if isinstance(ops, list) else 0}
        except json.JSONDecodeError:
            args = {"targets": [], "classes": [], "op_count": 0}
        records["in"] = hashlib.sha256(rest[0].encode()).hexdigest()
        record = result.get("record")
        records["out"] = hashlib.sha256(record.encode()).hexdigest() if isinstance(record, str) else None
    elif command == "append" and len(rest) == 2 and result is not None:
        try:
            ops = json.loads(rest[1])
            args = {"targets": [op.get("target") if isinstance(op, dict) else None
                                for op in ops] if isinstance(ops, list) else []}
        except json.JSONDecodeError:
            args = {"targets": []}
        records["in"] = hashlib.sha256(rest[0].encode()).hexdigest()
        record = result.get("record")
        records["out"] = hashlib.sha256(record.encode()).hexdigest() if isinstance(record, str) else None
    elif command == "finish" and len(rest) == 1:
        records["in"] = hashlib.sha256(rest[0].encode()).hexdigest()
    elif command == "side" and len(rest) == 1:
        args = {"side": rest[0]}
    elif command == "open" and not rest:
        args = {"opened": True}
    elif command == "revision" and len(rest) == 1:
        args = {"tx": rest[0]}
    return args, records


def _session_summary(transcript: list[dict[str, Any]]) -> dict[str, int]:
    """Aggregate one invocation's session transcript: requests, refusals,
    returned bytes, continuation use with omitted/truncated accounting."""

    summary = {"requests": 0, "failed": 0, "returned_bytes": 0,
               "continuations": 0, "omitted": 0, "truncated": 0}
    for entry in transcript:
        if not isinstance(entry, dict):
            continue
        if entry.get("direction") == "request":
            summary["requests"] += 1
            if entry.get("method") in CONTINUATION_METHODS:
                summary["continuations"] += 1
        elif entry.get("direction") == "response":
            if entry.get("failed"):
                summary["failed"] += 1
            for key in ("returned_bytes", "omitted"):
                try:
                    summary[key] += int(entry.get(key, 0) or 0)
                except (TypeError, ValueError):
                    pass
            if entry.get("truncated"):
                summary["truncated"] += 1
    return summary


def _chain_entry(entry: dict[str, Any], previous: str) -> dict[str, Any]:
    """Seal one chained evidence entry: hash covers the canonical body
    plus the previous hash, so reordering, deletion, or edits break
    verification."""

    body = json.dumps(entry, sort_keys=True)
    digest = hashlib.sha256((previous + body).encode("utf-8")).hexdigest()
    sealed = dict(entry)
    sealed["hash"] = digest
    return sealed


def append_transcript(workspace: Path, argv: list[str], ok: bool,
                      wall_ms: int, report_bytes: int,
                      transcript: list[dict[str, Any]],
                      result: dict[str, Any] | None = None) -> dict[str, Any]:
    """Append one invocation to the hash-chained agent transcript beside
    (never inside) the served repository. The chain is the complete,
    ordered, tamper-evident record of everything the agent saw and did;
    the judge verifies it before deriving any access evidence.

    Three claims stay separated: the hash chain proves ordering and
    tamper-evidence only. Completeness (every agent-visible operation
    and response is recorded) comes from durable-before-release in main
    plus transition/final linkage checks in the judge. Absence of an
    unrecorded route to protected state comes from provider confinement
    and tool-allowlist enforcement, not from the chain. The chain alone
    establishes none of the other two.
    """

    chain = workspace / CHAIN_NAME
    previous = "0" * 64
    seq = 0
    try:
        if chain.is_file() and not chain.is_symlink():
            lines = chain.read_text(encoding="utf-8").splitlines()
            if lines:
                last = json.loads(lines[-1])
                if not isinstance(last, dict) or not isinstance(last.get("hash"), str):
                    _fail("transcript chain")
                previous = last["hash"]
                seq = len(lines)
    except (OSError, ValueError) as error:
        raise Sley2ToolError(f"LIVE_SLEY2_TOOL_INVALID: transcript chain: {error}") from error
    args, records = _command_evidence(argv, result)
    entry = {
        "seq": seq,
        "tool_version": TOOL_VERSION,
        "sley_binary_sha256": _binary_digest(),
        "pack_sha256": _pack_digest(workspace),
        "command": argv[0] if argv else "",
        "args": args,
        "ok": ok,
        "wall_ms": wall_ms,
        "report_bytes": report_bytes,
        "records": records,
        "summary": _session_summary(transcript),
        "session": transcript,
        "prev": previous,
    }
    sealed = _chain_entry(entry, previous)
    try:
        with open(chain, "a", encoding="utf-8") as handle:
            handle.write(json.dumps(sealed, sort_keys=True) + "\n")
    except OSError as error:
        raise Sley2ToolError(f"LIVE_SLEY2_TOOL_INVALID: transcript chain: {error}") from error
    return sealed


def verify_transcript_chain(path: Path) -> tuple[list[dict[str, Any]], str | None]:
    """Verify a chained transcript file: contiguous sequence from zero,
    intact prev links, intact entry hashes. Returns (entries, None) when
    valid, ([], reason) otherwise. Used by the trial judge; never by the
    agent surface."""

    try:
        lines = Path(path).read_text(encoding="utf-8").splitlines()
    except OSError as error:
        return [], f"unreadable: {error}"
    entries: list[dict[str, Any]] = []
    previous = "0" * 64
    for number, line in enumerate(lines):
        try:
            entry = json.loads(line)
        except ValueError:
            return [], f"line {number} not JSON"
        if not isinstance(entry, dict):
            return [], f"line {number} shape"
        if entry.get("seq") != number:
            return [], f"line {number} seq"
        if entry.get("prev") != previous:
            return [], f"line {number} prev link"
        claimed = entry.get("hash")
        body = {key: value for key, value in entry.items() if key != "hash"}
        if not isinstance(claimed, str) or _chain_entry(body, previous)["hash"] != claimed:
            return [], f"line {number} hash"
        previous = claimed
        entries.append(entry)
    return entries, None


def main(argv: list[str] | None = None) -> int:
    arguments = sys.argv[1:] if argv is None else argv
    transcript: list[dict[str, Any]] = []
    workspace = Path.cwd()
    import time

    started = time.monotonic()
    try:
        session = Session(resolve_binary(), workspace, transcript)
        try:
            result = dispatch(session, workspace, arguments)
        finally:
            session.close()
        digest = hashlib.sha256(json.dumps(transcript, sort_keys=True).encode("utf-8")).hexdigest()
        printed = json.dumps({"command": arguments[:1], "outcome": "completed", "return_code": 0,
                              "report": result, "transcript_sha256": digest,
                              "stderr_text": "", "stdout_bytes": "", "stderr_bytes": "",
                              "truncated": False}, sort_keys=True)
        # Durable before release: the agent-visible response is retained
        # in the chained transcript (and usage ledger) BEFORE it is
        # printed. Evidence loss is therefore a terminal retained
        # failure (exit 2, no successful output released), never a
        # printed success with a silently lost record.
        wall_ms = int((time.monotonic() - started) * 1000)
        _record_usage(workspace, arguments[0] if arguments else "",
                      True, wall_ms, len(printed))
        append_transcript(workspace, arguments, True, wall_ms, len(printed),
                          transcript, result)
        print(printed)
        return 0
    except (Sley2ToolError, sley2_codecs.CodecError, OSError, ValueError) as error:
        print(json.dumps({"code": "LIVE_SLEY2_TOOL_INVALID", "detail": str(error)[:500]}, sort_keys=True))
        try:
            wall_ms = int((time.monotonic() - started) * 1000)
            _record_usage(workspace, arguments[0] if arguments else "",
                          False, wall_ms, 0)
            append_transcript(workspace, arguments, False, wall_ms, 0, transcript, None)
        except Sley2ToolError:
            pass
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
