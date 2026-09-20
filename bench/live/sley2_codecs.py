"""Batched SCB1 codec service for the live sley_2_0 trial tool.

The trial tool runs under system python3, which has no blake3 and must not
duplicate the oracle's codecs. Every digest, encode, and semantic decode
goes through the pinned oracle/scb1 project over one `uv run --offline
--frozen` subprocess per batch (stdin JSON in, stdout JSON out). No
judgment happens here: this module transports bytes and structures only.
The strict trial oracle never trusts it.

Protocol: the caller passes a list of {"op": ..., ...} dicts; the service
returns a same-length list of {"ok": bool, ...} dicts. Unknown ops and any
malformed input fail the whole batch without partial results.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
ORACLE_PROJECT = ROOT / "oracle" / "scb1"

_SERVICE = r"""
import json, struct, sys
import blake3
from sley2_scb1_oracle import candidate as C
from sley2_scb1_oracle import entity_read as E
from sley2_scb1_oracle import mutation_value as M
from sley2_scb1_oracle.codec import Cursor, decode_uvar

def h(b):
    return bytes.fromhex(b)

def out_ok(**kw):
    out = {"ok": True}
    out.update(kw)
    return out

def _sized(cursor):
    size = decode_uvar(cursor, 64)
    if size > 16777216 or size > cursor.remaining:
        raise ValueError("sized bound")
    return cursor.read(size)

def _to_json(declared_type, cursor, depth=0):
    # Mechanical mirror of mutation_value._encode_type (fixture JSON
    # conventions: variant dicts, name-keyed enums, hex ids, plain
    # scalars). Display and agent authoring only; the trial oracle
    # re-validates everything through its own pinned decoders.
    if depth > 16:
        raise ValueError("type depth")
    if declared_type.startswith("Option<") and declared_type.endswith(">"):
        tag = decode_uvar(cursor, 32)
        payload = _sized(cursor)
        if tag == 0:
            if payload:
                raise ValueError("option payload")
            return {"variant": "None"}
        if tag != 1:
            raise ValueError("option tag")
        sub = Cursor(payload)
        value = _to_json(declared_type[7:-1], sub, depth + 1)
        sub.finish()
        return {"variant": "Some", "value": value}
    if declared_type.startswith("List<") and declared_type.endswith(">"):
        inner = declared_type[5:-1]
        count = decode_uvar(cursor, 64)
        if count > 65535:
            raise ValueError("list bound")
        return [_to_json(inner, Cursor(_sized(cursor)), depth + 1) for _ in range(count)]
    if declared_type in M.RECORDS:
        fields = M.RECORDS[declared_type]
        count = decode_uvar(cursor, 64)
        if count != len(fields):
            raise ValueError("record arity")
        out = {}
        for tag, name, field_type in fields:
            rtag = decode_uvar(cursor, 32)
            if rtag != tag:
                raise ValueError("record tag")
            sub = Cursor(_sized(cursor))
            out[name] = _to_json(field_type, sub, depth + 1)
            sub.finish()
        return out
    if declared_type in M.UNIONS:
        tag = decode_uvar(cursor, 32)
        payload = _sized(cursor)
        for variant, (vtag, payload_type) in M.UNIONS[declared_type].items():
            if vtag == tag:
                if payload_type is None:
                    if payload:
                        raise ValueError("union payload")
                    return {"variant": variant}
                sub = Cursor(payload)
                value = _to_json(payload_type, sub, depth + 1)
                sub.finish()
                return {"variant": variant, "value": value}
        raise ValueError("union tag")
    if declared_type in M.SIMPLE_ENUMS:
        width = 16 if declared_type == "BuiltinFailureKind" else 32
        tag = decode_uvar(cursor, width)
        for name, vtag in M.SIMPLE_ENUMS[declared_type].items():
            if vtag == tag:
                return name
        raise ValueError("enum tag")
    if declared_type in ("EntityId", "StateRoot", "MemberId", "ObjectId",
                         "WorkspaceId", "TransactionId", "SchemaEpochId",
                         "PolicyRootId", "PrincipalId", "CapabilitySummaryDigest",
                         "ValidationProfileId", "CandidateNonce", "FixedBytes32"):
        return cursor.read(32).hex()
    if declared_type in ("UInt16", "IntegerWidth", "UInt32", "UInt64", "UInt128"):
        width = {"UInt16": 16, "IntegerWidth": 16, "UInt32": 32,
                 "UInt64": 64, "UInt128": 128}[declared_type]
        return decode_uvar(cursor, width)
    if declared_type in ("SInt64", "SInt128"):
        width = 64 if declared_type == "SInt64" else 128
        raw = decode_uvar(cursor, width)
        return (raw >> 1) ^ -(raw & 1)
    if declared_type == "Unit":
        return None
    if declared_type == "Bool":
        raw = cursor.read(1)
        if raw not in (b"\x00", b"\x01"):
            raise ValueError("bool byte")
        return raw == b"\x01"
    if declared_type == "Bytes":
        return _sized(cursor).hex()
    if declared_type == "Text":
        return _sized(cursor).decode("utf-8")
    if declared_type in ("F32Bits", "F64Bits"):
        size = 4 if declared_type == "F32Bits" else 8
        return cursor.read(size).hex()
    if declared_type in ("Set<EntityId>", "EntityIdSet"):
        count = decode_uvar(cursor, 64)
        if count > 65535:
            raise ValueError("set bound")
        return [Cursor(_sized(cursor)).read(32).hex() for _ in range(count)]
    if declared_type == "CanonicalMapEntries":
        count = decode_uvar(cursor, 64)
        if count > 65535:
            raise ValueError("map bound")
        return [_to_json("MapEntryConst", Cursor(_sized(cursor)), depth + 1)
                for _ in range(count)]
    raise ValueError(f"unsupported type {declared_type}")

def h(b):
    return bytes.fromhex(b)

def out_ok(**kw):
    out = {"ok": True}
    out.update(kw)
    return out

def handle(call):
    op = call.get("op")
    if op == "capability_digest":
        ws = h(call["workspace"]); pol = h(call["policy_root"])
        root = h(call["state_root"]); princ = h(call["principal"])
        rec = C.encode_record([(1, C.encode_uvar(1)), (2, princ), (3, ws),
                               (4, pol), (5, root), (6, C.encode_uvar(0))])
        pre = b"SLEYCAS1" + C.encode_uvar(1) + C.encode_uvar(len(rec)) + rec
        return out_ok(digest=blake3.blake3(b"sley2.capability-summary.v1" + pre).hexdigest())
    if op == "derive_entity":
        d = C.derive_entity_id(h(call["workspace"]), h(call["nonce"]),
                               int(call["kind"]), int(call["ordinal"]))
        return out_ok(entity=d.hex())
    if op == "validation_profile":
        return out_ok(profile=C.validation_profile_id().hex())
    if op == "assemble_record":
        record = C.encode_candidate_record(call["record"])
        return out_ok(record=record.hex())
    if op == "decode_response":
        body = h(call["body"])
        method = call["method"]
        if method in ("entity.version", "entity.signature"):
            decoded = E.decode_response_body(body)
            objects = []
            for entry in decoded["entries"]:
                stored = E.decode_response_entry(entry)
                record = _parse_object_record(stored["stored"])
                unbound = E.decode_stored_record(record)
                typed = _to_json(E.BODY_TYPES[stored["kind"]], Cursor(unbound["body"]))
                objects.append({"entity": stored["entity"],
                                "kind": stored["kind"],
                                "object_id": stored["object_id"],
                                "body_type": E.BODY_TYPES[stored["kind"]],
                                "body": typed})
            decoded["entries"] = objects
        else:
            return {"ok": False, "error": "no decoder for method"}
        return out_ok(decoded=json.loads(json.dumps(decoded, default=_default)))
    if op == "decode_object":
        decoded = E.decode_stored_unbound(h(call["stored"]), h(call["epoch"]))
        raw_body = bytes(decoded["body"]).hex()
        typed = _to_json(E.BODY_TYPES[decoded["kind"]], Cursor(decoded["body"]))
        decoded["body"] = typed
        decoded["body_type"] = E.BODY_TYPES[decoded["kind"]]
        decoded["body_hex"] = raw_body
        return out_ok(decoded=json.loads(json.dumps(decoded, default=_default)))
    if op == "decode_value":
        cursor = Cursor(h(call["data"]))
        value = _to_json(call["type"], cursor)
        cursor.finish()
        return out_ok(value=json.loads(json.dumps(value, default=_default)))
    if op == "decode_failure":
        return out_ok(decoded=json.loads(json.dumps(E.decode_failure(h(call["body"])), default=_default)))
    if op == "encode_fields":
        return out_ok(fields=E.encode_fields([(int(tag), h(val)) for tag, val in call["fields"]]).hex())
    if op == "encode_value":
        from sley2_scb1_oracle import mutation_value as MV
        return out_ok(value=MV.encode_mutation_value(call["type"], call["value"]).hex())
    if op == "stored_from_record":
        return out_ok(stored=C.stored_from_record_bytes(h(call["record"])).hex())
    return {"ok": False, "error": "unknown op"}

def _parse_object_record(stored: bytes) -> bytes:
    # Framing mirror of the stored-object envelope (magic, version,
    # contract tag, epoch, sized record, digest trailer): display parsing
    # only, no digest verification — the server is the source of truth.
    cursor = Cursor(stored)
    if cursor.read(8) != b"SLEYSCB1":
        raise ValueError("object magic")
    decode_uvar(cursor, 64)
    decode_uvar(cursor, 32)
    cursor.read(32)
    size = decode_uvar(cursor, 64)
    if size > 16777216 or size > cursor.remaining:
        raise ValueError("object bound")
    return cursor.read(size)


def _default(value):
    # Raw bytes surface as plain lowercase hex: exactly the form the
    # fixture encoders accept back (_bytes_from_hex), so decoded views
    # round-trip into proposals.
    if isinstance(value, (bytes, bytearray)):
        return bytes(value).hex()
    raise TypeError(f"non-JSON value {type(value).__name__}")

def main():
    calls = json.load(sys.stdin)
    if not isinstance(calls, list):
        print(json.dumps([{"ok": False, "error": "batch must be a list"}]))
        return 2
    results = []
    for call in calls:
        try:
            results.append(handle(call))
        except Exception as error:
            results.append({"ok": False, "error": f"{type(error).__name__}: {str(error)[:200]}"})
    print(json.dumps(results))
    return 0

sys.exit(main())
"""


class CodecError(ValueError):
    """A codec service call was refused or failed."""


def _fail(detail: str) -> None:
    raise CodecError(f"LIVE_SLEY2_CODEC_INVALID: {detail}")


_UV = None


def _uv() -> str:
    """The uv binary by absolute path: trial and oracle subprocesses run
    under restricted PATHs without it, so it is resolved once per
    process from the ambient environment plus pinned install roots and
    then frozen. A move fails closed with a named error, never a PATH
    search at call time."""

    global _UV
    if _UV is None:
        import shutil

        found = shutil.which("uv")
        if found is None:
            for candidate in (
                "/home/gfarch/.local/share/mise/installs/uv/latest/.mise-bins/uv",
            ):
                try:
                    if Path(candidate).is_file() and os.access(candidate, os.X_OK):
                        found = candidate
                        break
                except OSError:
                    continue
        if found is None:
            _fail("uv unavailable")
        _UV = found
    return _UV


def run_batch(calls: list[dict], *, timeout_seconds: int = 120) -> list[dict]:
    """Run one batch of codec ops through the pinned oracle project."""

    if not isinstance(calls, list) or not calls:
        _fail("batch")
    try:
        payload = json.dumps(calls).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise CodecError(f"LIVE_SLEY2_CODEC_INVALID: input: {error}") from error
    try:
        completed = subprocess.run(
            [_uv(), "run", "--offline", "--frozen", "--project", str(ORACLE_PROJECT),
             "python", "-c", _SERVICE],
            input=payload,
            capture_output=True,
            timeout=timeout_seconds,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise CodecError(f"LIVE_SLEY2_CODEC_INVALID: spawn: {error}") from error
    if completed.returncode != 0:
        _fail(f"service exit {completed.returncode}: {completed.stderr.decode('utf-8', 'replace')[:300]}")
    try:
        results = json.loads(completed.stdout.decode("utf-8"))
    except (UnicodeError, json.JSONDecodeError) as error:
        raise CodecError(f"LIVE_SLEY2_CODEC_INVALID: output: {error}") from error
    if not isinstance(results, list) or len(results) != len(calls):
        _fail("shape")
    for result in results:
        if not isinstance(result, dict) or result.get("ok") is not True:
            _fail(str(result.get("error", "unknown"))[:200] if isinstance(result, dict) else "shape")
    return results
