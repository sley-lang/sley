from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import re

from bench.live import mediated_shim, mediated_sley, sley2_tool
from bench.live.tooling import (
    SLEY2_TOOLING,
    build_prompt,
    prompt_template_digest,
    stage_tooling,
    tooling_digest,
)


class ToolingTests(unittest.TestCase):
    def test_prompt_is_identical_across_arms_and_binds_task_and_seed(self) -> None:
        prompts = {
            build_prompt("S2B-REPAIR-001", 17, arm)
            for arm in ("raw_files", "sley_1_2_0", "sley_2_0")
        }
        self.assertEqual(len(prompts), 1)
        prompt = prompts.pop()
        self.assertIn('"id":"S2B-REPAIR-001"', prompt.decode())
        self.assertIn("Trial seed: 17", prompt.decode())
        self.assertEqual(len(prompt_template_digest()), 64)

    def test_raw_and_legacy_tooling_are_deterministic_and_legacy_launcher_is_executable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for arm in ("raw_files", "sley_1_2_0", "sley_2_0"):
                first = root / f"{arm}-one"
                second = root / f"{arm}-two"
                first.mkdir()
                second.mkdir()
                stage_tooling(arm, first)
                stage_tooling(arm, second)
                self.assertEqual(tooling_digest(arm), tooling_digest(arm))
                self.assertEqual(
                    (first / ".sley-live/TOOLING.md").read_bytes(),
                    (second / ".sley-live/TOOLING.md").read_bytes(),
                )
            launcher = root / "sley_1_2_0-one/.sley-live/sley-tool"
            self.assertTrue(launcher.stat().st_mode & 0o111)
            sley2_launcher = root / "sley_2_0-one/.sley-live/sley-tool"
            self.assertTrue(sley2_launcher.stat().st_mode & 0o111)

    def test_unknown_task_arm_and_existing_control_directory_refuse(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.mkdir(exist_ok=True)
            with self.assertRaises(ValueError):
                build_prompt("NOPE", 1, "raw_files")
            with self.assertRaises(ValueError):
                stage_tooling("sley_9_9", root)
            (root / ".sley-live").mkdir()
            with self.assertRaises(ValueError):
                stage_tooling("raw_files", root)


class Sley2CommandSurfacePinTests(unittest.TestCase):
    """The agent-facing TOOLING.md is the surface the live model reads: its
    documented command lines, the mediated gateway's allowed commands, the
    shim's phase sets, and the tool dispatcher's commands move together."""

    def documented(self) -> list[str]:
        return re.findall(r"^\.sley-live/sley-tool (\S+)", SLEY2_TOOLING, flags=re.M)

    def test_every_allowed_command_is_documented_and_nothing_else(self) -> None:
        documented = self.documented()
        self.assertEqual(len(documented), len(set(documented)), documented)
        for name in sorted(mediated_sley.ALLOWED_COMMANDS):
            with self.subTest(name=name):
                self.assertIn(f".sley-live/sley-tool {name}", SLEY2_TOOLING)
        self.assertEqual(set(documented), set(mediated_sley.ALLOWED_COMMANDS))

    def test_documented_argument_forms_for_open_and_revision(self) -> None:
        lines = set(re.findall(r"^\.sley-live/sley-tool .*$", SLEY2_TOOLING, flags=re.M))
        self.assertIn(".sley-live/sley-tool open", lines)
        self.assertIn(".sley-live/sley-tool revision TX_HEX", lines)
        self.assertIn(".sley-live/sley-tool sig ENTITY_HEX", lines)

    def test_shim_phases_enumerate_every_allowed_command(self) -> None:
        phased = (set(mediated_shim.READ_COMMANDS)
                  | set(mediated_shim.COMPOSE_COMMANDS) | {"finish"})
        self.assertEqual(set(mediated_sley.ALLOWED_COMMANDS) - phased, set())
        self.assertIn("open", mediated_shim.READ_COMMANDS)

    def test_tool_module_docstring_lists_open_and_revision_argument(self) -> None:
        doc = sley2_tool.__doc__ or ""
        self.assertRegex(doc, r"(?m)^  open\s")
        self.assertRegex(doc, r"(?m)^  revision <tx-hex>\s")


# --- Root-query contract: the documented layout, checked three ways -------
#
# The helpers below implement ONLY what the "Bounded root queries" block of
# SLEY2_TOOLING states (classes 2, 4, 14; the field order, widths, and
# ranges it lists). They share no code with the stand-in or the server.
# 1. the documented block is pinned line for line, so a doc edit forces a
#    review of these helpers;
# 2. requests encoded to the documented layout reproduce the conformance
#    vectors' request identities (blake3 over the engine's domain) and the
#    vectors' response records decode under the documented layout, so the
#    doc matches the engine;
# 3. every request body the scripted CONTEXT stand-in sends (all modes) is
#    parsed strictly under the documented grammar, and the stand-in decodes
#    responses built to the documented layout.

import json
import subprocess
from pathlib import Path as _Path

from bench.live import mediated_client, sley2_codecs

ROOT = _Path(__file__).resolve().parents[3]
DOCUMENTED_LAYOUT = """request body:
  "SLEYRQQ1"                     8 bytes
  u32 1, u32 1                   format, profile
  snapshot, epoch, root, workspace   32 bytes each, from `open`
  u32 2, u32 1                   completeness, limits profile
  u64 max_entities               1..65535 entries per page
  u64 max_edges                  1..400000
  u32 max_depth                  0..65535
  u64 max_response_bytes         1..67108864
  u64 max_work                   1..100000000
  u32 paging                     1 = single page, 2 = allow continuation
  cursor                         u32 1 (none) | u32 2, u32 1, entity id
  u32 class, class body:
    2   entity                   id -> its kind and object id
    4   entities of one kind     u32 kind (4 = type definition, 9 = constant)
    14  reverse impact closure   u64 n (1..65535), then n seed ids, strictly ascending
response body:
  "SLEYRQR1", u32 1, u32 1, query id, snapshot, epoch, root, workspace,
  u32 2, u32 1, the five limits, u32 paging, cursor (echoed request)
  u32 class, u64 total (whole result), u64 returned (this page),
  u32 truncated (1 no, 2 yes),
  next cursor (same encoding), u32 depth, u64 work, u64 bytes, u32 class
  result, class 2: u32 kind, object id, fingerprint (u32 1 | u32 2, 32 bytes)
  result, classes 4 and 14: u64 count, then count entity ids
"""
LIMIT_RANGES = (("max_entities", 8, 1, 65535), ("max_edges", 8, 1, 400000),
                ("max_depth", 4, 0, 65535),
                ("max_response_bytes", 8, 1, 67108864),
                ("max_work", 8, 1, 100000000))


class _Reader:
    def __init__(self, data: bytes) -> None:
        self.data, self.at = data, 0

    def take(self, size: int) -> bytes:
        if self.at + size > len(self.data):
            raise ValueError("short")
        chunk = self.data[self.at:self.at + size]
        self.at += size
        return chunk

    def u(self, width: int) -> int:
        return int.from_bytes(self.take(width), "big")

    def ident(self) -> str:
        return self.take(32).hex()

    def cursor(self) -> str | None:
        tag = self.u(4)
        if tag == 1:
            return None
        if tag != 2 or self.u(4) != 1:
            raise ValueError("cursor")
        return self.ident()

    def done(self) -> None:
        if self.at != len(self.data):
            raise ValueError("trailing bytes")


def doc_parse_request(body: bytes) -> dict:
    r = _Reader(body)
    if r.take(8) != b"SLEYRQQ1" or r.u(4) != 1 or r.u(4) != 1:
        raise ValueError("header")
    req = {name: r.ident() for name in ("snapshot", "epoch", "root", "workspace")}
    if r.u(4) != 2 or r.u(4) != 1:
        raise ValueError("profile")
    for name, width, low, high in LIMIT_RANGES:
        req[name] = r.u(width)
        if not low <= req[name] <= high:
            raise ValueError(name)
    req["paging"] = r.u(4)
    if req["paging"] not in (1, 2):
        raise ValueError("paging")
    req["cursor"] = r.cursor()
    req["class"] = r.u(4)
    if req["class"] == 2:
        req["entity"] = r.ident()
        if req["cursor"] is not None:
            raise ValueError("class 2 takes no cursor")
    elif req["class"] == 4:
        req["kind"] = r.u(4)
    elif req["class"] == 14:
        count = r.u(8)
        if not 1 <= count <= 65535:
            raise ValueError("seed count")
        req["seeds"] = [r.ident() for _ in range(count)]
        if req["seeds"] != sorted(set(req["seeds"])):
            raise ValueError("seeds not strictly ascending")
    else:
        raise ValueError(f"undocumented class {req['class']}")
    r.done()
    return req


def doc_encode_request(req: dict) -> bytes:
    def cursor(value):
        return (1).to_bytes(4, "big") if value is None else (
            (2).to_bytes(4, "big") + (1).to_bytes(4, "big") + bytes.fromhex(value))

    out = bytearray(b"SLEYRQQ1" + (1).to_bytes(4, "big") * 2)
    for name in ("snapshot", "epoch", "root", "workspace"):
        out += bytes.fromhex(req[name])
    out += (2).to_bytes(4, "big") + (1).to_bytes(4, "big")
    for name, width, _, _ in LIMIT_RANGES:
        out += req[name].to_bytes(width, "big")
    out += req["paging"].to_bytes(4, "big") + cursor(req["cursor"])
    out += req["class"].to_bytes(4, "big")
    if req["class"] == 2:
        out += bytes.fromhex(req["entity"])
    elif req["class"] == 4:
        out += req["kind"].to_bytes(4, "big")
    else:
        out += len(req["seeds"]).to_bytes(8, "big")
        out += b"".join(bytes.fromhex(seed) for seed in req["seeds"])
    return bytes(out)


def doc_encode_response(req: dict, *, total: int, entities=None, kind=None,
                        truncated: bool = False, next_after=None) -> bytes:
    def cursor(value):
        return (1).to_bytes(4, "big") if value is None else (
            (2).to_bytes(4, "big") + (1).to_bytes(4, "big") + bytes.fromhex(value))

    head = doc_encode_request(req)
    # Echo: magic swap, query id (opaque 32 bytes here), then the request
    # fields from snapshot through the echoed cursor and class.
    echo = head[16:len(head) - _class_body_len(req)]
    out = bytearray(b"SLEYRQR1" + (1).to_bytes(4, "big") * 2 + b"\x11" * 32)
    out += echo
    returned = 1 if req["class"] == 2 else len(entities)
    out += total.to_bytes(8, "big") + returned.to_bytes(8, "big")
    out += (2 if truncated else 1).to_bytes(4, "big") + cursor(next_after)
    out += (0).to_bytes(4, "big") + (1).to_bytes(8, "big") + (0).to_bytes(8, "big")
    out += req["class"].to_bytes(4, "big")
    if req["class"] == 2:
        out += kind.to_bytes(4, "big") + b"\x22" * 32 + (1).to_bytes(4, "big")
    else:
        out += len(entities).to_bytes(8, "big")
        out += b"".join(bytes.fromhex(e) for e in entities)
    return bytes(out)


def _class_body_len(req: dict) -> int:
    return {2: 32, 4: 4}.get(req["class"], 8 + 32 * len(req.get("seeds", [])))


def doc_decode_response(body: bytes) -> dict:
    r = _Reader(body)
    if r.take(8) != b"SLEYRQR1" or r.u(4) != 1 or r.u(4) != 1:
        raise ValueError("header")
    out = {"query_id": r.ident()}
    for name in ("snapshot", "epoch", "root", "workspace"):
        out[name] = r.ident()
    if r.u(4) != 2 or r.u(4) != 1:
        raise ValueError("profile")
    for name, width, _, _ in LIMIT_RANGES:
        out[name] = r.u(width)
    out["paging"], out["cursor"], out["class"] = r.u(4), r.cursor(), r.u(4)
    out["total"], out["returned"] = r.u(8), r.u(8)
    out["truncated"] = {1: False, 2: True}[r.u(4)]
    out["next_after"] = r.cursor()
    r.u(4), r.u(8), r.u(8)
    if r.u(4) != out["class"]:
        raise ValueError("class echo")
    if out["class"] == 2:
        out["kind"], out["object_id"] = r.u(4), r.ident()
        if r.u(4) == 2:
            r.take(32)
    else:
        out["entities"] = [r.ident() for _ in range(r.u(8))]
    r.done()
    return out


class RootQueryContractTests(unittest.TestCase):
    VECTORS = ("class-02", "class-04", "class-14",
               "page-namespaces-1", "page-namespaces-2")

    def test_documented_layout_is_pinned(self) -> None:
        start = SLEY2_TOOLING.index("```text\nrequest body:") + len("```text\n")
        block = SLEY2_TOOLING[start:SLEY2_TOOLING.index("```", start)]
        self.assertEqual(block, DOCUMENTED_LAYOUT)
        for sentence in ("QUERY_SNAPSHOT_MISMATCH",
                         "with 32 zero bytes as the snapshot",
                         "cursor set to that\npage's next cursor",
                         "no single reply may exceed 1048576 bytes",
                         "stay within 4194304 bytes per trial"):
            self.assertIn(sentence, SLEY2_TOOLING)

    def test_documented_raw_methods_are_the_tool_methods(self) -> None:
        start = SLEY2_TOOLING.index("response body as hex")
        listed = SLEY2_TOOLING[SLEY2_TOOLING.index("):", start) + 2:
                               SLEY2_TOOLING.index(".\n", start)]
        names = tuple(name.strip() for name in listed.split(","))
        self.assertEqual(names, tuple(sley2_tool.TOOL_METHODS))

    def _vectors(self) -> tuple[dict, dict]:
        data = json.loads((ROOT / "conformance/root-backed-query/v1/accepted.json")
                          .read_text(encoding="utf-8"))
        return data["context"], {v["id"]: v for v in data["vectors"]}

    @staticmethod
    def _request_for(context: dict, vector: dict) -> dict:
        req = {"snapshot": context["snapshot_id"],
               "epoch": context["schema_epoch_hex"],
               "root": context["root_hex"], "workspace": context["workspace_id"],
               "paging": 2 if vector["allow_continuation"] else 1,
               "cursor": (vector["after"] or {}).get("entity"),
               **{k: v for k, v in vector["query"].items() if k != "class"}}
        limits = vector["limits"]
        req.update(max_entities=limits["max_returned_entities"],
                   max_edges=limits["max_returned_edges"],
                   max_depth=limits["max_depth"],
                   max_response_bytes=limits["max_response_bytes"],
                   max_work=limits["max_work"])
        req["class"] = vector["query"]["class"]
        return req

    def test_documented_request_layout_reproduces_engine_query_ids(self) -> None:
        context, vectors = self._vectors()
        bodies = []
        for name in self.VECTORS:
            req = self._request_for(context, vectors[name])
            body = doc_encode_request(req)
            self.assertEqual(doc_parse_request(body)["class"], req["class"])
            bodies.append(body.hex())
        try:
            uv = sley2_codecs._uv()
        except sley2_codecs.CodecError:
            self.skipTest("uv unavailable")
        script = ("import blake3, json, sys\n"
                  "print(json.dumps([blake3.blake3(b'sley2.root-query.v1' + "
                  "bytes.fromhex(h)).hexdigest() for h in json.load(sys.stdin)]))")
        completed = subprocess.run(
            [uv, "run", "--offline", "--frozen", "--project",
             str(sley2_codecs.ORACLE_PROJECT), "python", "-c", script],
            input=json.dumps(bodies).encode(), capture_output=True,
            timeout=120, check=False)
        self.assertEqual(completed.returncode, 0, completed.stderr[-300:])
        digests = json.loads(completed.stdout)
        for name, digest in zip(self.VECTORS, digests):
            with self.subTest(vector=name):
                self.assertEqual(digest, vectors[name]["query_id"])

    def test_engine_response_records_decode_under_documented_layout(self) -> None:
        context, vectors = self._vectors()
        for name in self.VECTORS:
            vector = vectors[name]
            with self.subTest(vector=name):
                record = bytes.fromhex(vector["record_hex"])
                doc = doc_decode_response(record)
                self.assertEqual(doc["query_id"], vector["query_id"])
                self.assertEqual(doc["snapshot"], context["snapshot_id"])
                self.assertEqual(doc["class"], vector["query"]["class"])
                self.assertEqual(doc["next_after"],
                                 (vector["next_after"] or {}).get("entity"))
                self.assertEqual(doc["truncated"], vector["next_after"] is not None)
                stand_in = mediated_client.decode_root_response(record.hex())
                for key in ("snapshot", "class", "total", "returned",
                            "truncated", "next_after"):
                    self.assertEqual(stand_in[key], doc[key], key)
                if doc["class"] == 2:
                    self.assertEqual(stand_in["kind"], doc["kind"])
                else:
                    self.assertEqual(stand_in["entities"], doc["entities"])

    def test_every_stand_in_request_follows_the_documented_layout(self) -> None:
        for mode in mediated_client.CONTEXT_MODES:
            with self.subTest(mode=mode):
                server = _DocumentedFakeServer()
                outcome = mediated_client.context_discover_and_repair(
                    server.call, mode)
                self.assertTrue(server.bodies, mode)
                for method, body in server.bodies:
                    req = doc_parse_request(bytes.fromhex(body))
                    self.assertIn(method, ("query.root", "query.continue"))
                    if method == "query.continue":
                        self.assertEqual(req["paging"], 2)
                        self.assertIsNotNone(req["cursor"])
                # The fake accepts every proposal (validation is the
                # server's job, proved in test_mediated_context); here only
                # the request shapes and the response decoding matter.
                self.assertTrue(outcome.get("finished"), outcome)
                discovery = outcome["discovery"]
                self.assertEqual(discovery["impact_seen"],
                                 7 if mode == "stop_early" else 10)
                self.assertEqual(discovery["impacted_constants"], 3)


class _DocumentedFakeServer:
    """A stand-in for the gateway that answers root queries strictly under
    the documented request/response layout (and nothing else), so the
    stand-in's encoder and decoder are exercised against the contract."""

    HEAD = {"tx": "10" * 32, "root": "20" * 32, "policy": "30" * 32,
            "workspace": "40" * 32, "epoch": "50" * 32, "objects": 20,
            "tombstones": 0, "receipt": "60" * 32}
    SNAPSHOT = "70" * 32
    TYPEDEF = "c4" + "04" * 31
    CONSTS = ["c4" + f"{n:02x}" * 31 for n in (5, 6, 7)]
    CLOSURE = sorted(["c4" + f"{n:02x}" * 31 for n in range(11) if n != 2])

    def __init__(self) -> None:
        self.warm = False
        self.bodies: list[tuple[str, str]] = []

    def kind_of(self, entity: str) -> int:
        if entity == self.TYPEDEF:
            return 4
        return 9 if entity in self.CONSTS else 3

    def call(self, name, phase, command, args):
        if command == "open" and not args:
            head = dict(self.HEAD)
            if self.warm:
                head["snapshot"] = self.SNAPSHOT
            return {"ok": True, "report": {"failed": False, "decoded": head}}
        if command == "revision":
            if len(args) != 1:
                return {"ok": False, "error": "Sley2ToolError"}
            return {"ok": True, "report": {"failed": False, "decoded": {}}}
        if command == "read":
            entity = args[0]
            kind = self.kind_of(entity)
            if kind == 4:
                body = {"form": {"variant": "Record", "value": []}}
            elif kind == 9:
                body = {"value": {"value_type": {"variant": "Named"},
                                  "data": {"variant": "Record", "value": {
                                      "definition": self.TYPEDEF,
                                      "fields": []}}}}
            else:
                body = {"members": []}
            return {"ok": True, "report": {"failed": False, "decoded": {
                "entries": [{"kind": kind, "body": body}]}}}
        if command == "propose":
            return {"ok": True, "report": {"valid": True, "record": "ab",
                                           "decision": {"tag": 1}}}
        if command == "finish":
            return {"ok": True, "report": {"finished": True}}
        if command != "raw":
            raise AssertionError(f"unexpected command {command}")
        method, body = args
        self.bodies.append((method, body))
        req = doc_parse_request(bytes.fromhex(body))
        if req["snapshot"] != self.SNAPSHOT:
            self.warm = True
            return {"ok": True, "report": {"failed": True, "body": "00"}}
        if req["class"] == 2:
            reply = doc_encode_response(req, total=1,
                                        kind=self.kind_of(req["entity"]))
            return {"ok": True, "report": {"failed": False, "body": reply.hex()}}
        if req["class"] == 4:
            items = ([self.TYPEDEF] if req["kind"] == 4 else
                     self.CONSTS if req["kind"] == 9 else [])
        else:
            items = list(self.CLOSURE) if req["seeds"] == [self.TYPEDEF] else []
        remaining = [e for e in items if req["cursor"] is None or e > req["cursor"]]
        page = remaining[:req["max_entities"]]
        truncated = len(remaining) > len(page)
        if truncated and req["paging"] == 1:
            return {"ok": True, "report": {"failed": True, "body": "00"}}
        reply = doc_encode_response(req, total=len(items), entities=page,
                                    truncated=truncated,
                                    next_after=page[-1] if truncated else None)
        return {"ok": True, "report": {"failed": False, "body": reply.hex()}}


if __name__ == "__main__":
    unittest.main()
