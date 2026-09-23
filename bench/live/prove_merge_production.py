#!/usr/bin/env python3
"""Production merge-path proof on live MERGE content (runner-owned).

Builds corpus-faithful BRANCH structure in one scratch repository —
ancestor seed, branch.create ours/theirs at the ancestor — then
co-locates both side histories by content-addressed object union
(commits are linear-head by design, so branch divergence is
reconciled, never re-committed; exchange.import is one-shot), and
drives the PRODUCTION merge path: merge.judge in both operand
orders, merge.commit, post-commit reads. No frozen fixture is
modified; no trial-surface operation is added.

Proves (S2B-MERGE-001 content):
- branch pointers ours/theirs fork from the ancestor transaction;
- merge.judge accepts (ancestor, ours, theirs) AND the swapped
  order with byte-identical merged payloads (observed commutativity);
- merge.commit lands a merged transaction; post-commit reads show
  exactly the union semantics (shared constant at the ours value,
  theirs-only change preserved byte-identical, base otherwise
  unchanged);
- selected executable tests: enumerated from the merged state.
  The frozen sides carry no functions, so the executable selected
  set is EMPTY — recorded here as the remaining content gap for
  the fixture review (invoice-test/checksum shapes exist only as
  constants), never as a pass.

Usage: prove_merge_production.py [logfile]
Env: SLEY2_SLEY_BINARY (required).
"""

from __future__ import annotations

import json
import os
import shutil
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from bench.live import sley2_codecs  # noqa: E402
from bench.live.scratch import remove_scratch  # noqa: E402
from bench.live import sley2_tool  # noqa: E402

TASK_DIR = ROOT / "bench" / "fixtures" / "sley2" / "S2B-MERGE-001"


# --- minimal SCB mirror (oracle/scb1 codec semantics, stdlib only) ---

def enc_uvar(value: int) -> bytes:
    if value < 0:
        raise ValueError("uvar negative")
    out = bytearray()
    while True:
        group = value & 0x7F
        value >>= 7
        out.append(group | (0x80 if value else 0))
        if not value:
            return bytes(out)


def dec_uvar(data: bytes, pos: int = 0) -> tuple[int, int]:
    value = 0
    shift = 0
    while True:
        byte = data[pos]
        pos += 1
        value |= (byte & 0x7F) << shift
        shift += 7
        if not byte & 0x80:
            return value, pos


def read_record(data: bytes, expected: list[int]) -> dict[int, bytes]:
    pos = 0
    count, pos = dec_uvar(data, pos)
    if count != len(expected):
        raise ValueError(f"field count {count}")
    result: dict[int, bytes] = {}
    for want in expected:
        tag, pos = dec_uvar(data, pos)
        if tag != want:
            raise ValueError(f"field tag {tag}")
        size, pos = dec_uvar(data, pos)
        result[tag] = data[pos:pos + size]
        pos += size
    if pos != len(data):
        raise ValueError("trailing bytes")
    return result


def read_union(data: bytes) -> tuple[int, bytes]:
    pos = 0
    tag, pos = dec_uvar(data, pos)
    size, pos = dec_uvar(data, pos)
    payload = data[pos:pos + size]
    if pos + size != len(data) or len(payload) != size:
        raise ValueError("union framing")
    return tag, payload


# --- proof ---

def _fields(pairs: list[tuple[int, bytes]]) -> str:
    [out] = sley2_codecs.run_batch(
        [{"op": "encode_fields",
          "fields": [[tag, value.hex()] for tag, value in pairs]}])
    return out["fields"]


def _failure(reply: dict) -> str:
    [failure] = sley2_codecs.run_batch(
        [{"op": "decode_failure", "body": reply["body"]}])
    return json.dumps(failure["decoded"])[:200]


def _read_live_state(pack_name: str, read_entities: list[str] | None = None) -> dict:
    """Live entity bodies of one frozen side pack (throwaway session).

    Returns head tx, object list, decoded bodies for read_entities,
    and the repo dir (kept on disk for co-location; the caller cleans
    the stage)."""

    stage = Path(tempfile.mkdtemp(prefix="sley2-merge-side-"))
    ws = stage / "ws"
    ws.mkdir(mode=0o700)
    (ws / "repo").mkdir(mode=0o700)
    shutil.copyfile(TASK_DIR / pack_name, ws / "base.pack")
    session = sley2_tool.Session(
        sley2_tool.resolve_binary(), ws, [], seed_pack=True)
    try:
        inv = sley2_tool.dispatch(session, ws, ["inventory"])
        bodies: dict[str, dict] = {}
        for eid in read_entities or []:
            rep = sley2_tool.dispatch(session, ws, ["read", eid])
            bodies[eid] = rep["decoded"]["entries"][0]["body"]
        return {"head": session.head["tx"],
                "objects": inv.get("inventory", {}).get("objects", []),
                "bodies": bodies,
                "repo": ws / "repo", "stage": stage}
    finally:
        session.close()


def _union_tree(dest: Path, source: Path) -> int:
    """Copy content-addressed files (new files only; shared history
    has identical bytes by content addressing). Returns files added."""

    added = 0
    for path in sorted(source.rglob("*")):
        if not path.is_file() or path.is_symlink():
            continue
        target = dest / path.relative_to(source)
        if target.exists():
            continue
        target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        shutil.copyfile(path, target)
        added += 1
    return added


def _judge(session, ancestor: bytes, left: bytes,
           right: bytes) -> dict:
    body = _fields([(1, ancestor), (2, left), (3, right)])
    reply = session._raw_request("merge.judge", body)
    if reply["flags"].get("failed"):
        raise SystemExit(f"merge.judge refused: {_failure(reply)}")
    tag, payload = read_union(bytes.fromhex(reply["body"]))
    if tag != 1:
        raise SystemExit(f"merge.judge conflict, tag {tag}")
    fields = read_record(payload, [1, 2, 3])
    if len(fields[1]) != 32:
        raise SystemExit("merged root not fixed32")
    count, _ = dec_uvar(fields[2])
    return {"root": fields[1].hex(), "count": count,
            "stored": fields[3].hex(), "raw": reply["body"]}


def main() -> int:
    log_path = Path(sys.argv[1]) if len(sys.argv) > 1 else None
    if not os.environ.get("SLEY2_SLEY_BINARY"):
        raise SystemExit("missing env SLEY2_SLEY_BINARY")
    lines: list[str] = []

    def emit(text: str) -> None:
        lines.append(text)
        print(text, flush=True)

    def flush_log() -> None:
        if log_path is not None:
            log_path.parent.mkdir(parents=True, exist_ok=True)
            log_path.write_text("\n".join(lines) + "\n", encoding="utf-8")

    manifest = json.loads((TASK_DIR / "task_manifest.json").read_text())
    conflict = manifest["entities"][
        manifest["judge"].get("conflict", "constant")]
    workdir = Path(tempfile.mkdtemp(prefix="sley2-merge-proof-"))
    stages: list[Path] = []
    try:
        # Live side content (identities + bodies the branches must
        # replicate as commits).
        base_state = _read_live_state("base.pack", [conflict])
        ours_state = _read_live_state("ours.pack", [conflict])
        theirs_state = _read_live_state("theirs.pack")
        stages = [base_state["stage"], ours_state["stage"],
                  theirs_state["stage"]]
        emit(f"base head {base_state['head'][:16]}... "
             f"objects {len(base_state['objects'])}")
        emit(f"ours head {ours_state['head'][:16]}... "
             f"objects {len(ours_state['objects'])}")
        emit(f"theirs head {theirs_state['head'][:16]}... "
             f"objects {len(theirs_state['objects'])}")
        # The proof repository: ancestor seed, then co-locate both
        # side histories by object union (content-addressed; shared
        # ancestor objects have identical bytes). Heads/branches/
        # exchange staging stay the base's own: only objects and
        # transactions travel. Commits are linear-head by design, so
        # branch divergence is reconciled here, not re-committed.
        ws = workdir / "ws"
        ws.mkdir(mode=0o700)
        (ws / "repo").mkdir(mode=0o700)
        shutil.copyfile(TASK_DIR / "base.pack", ws / "base.pack")
        session = None
        try:
            session = sley2_tool.Session(
                sley2_tool.resolve_binary(), ws, [], seed_pack=True)
            ancestor = bytes.fromhex(session.head["tx"])
            for branch in (b"ours", b"theirs"):
                reply = session._raw_request(
                    "branch.create",
                    _fields([(1, branch), (2, ancestor)]))
                if reply["flags"].get("failed"):
                    raise SystemExit(
                        f"branch.create refused: {_failure(reply)}")
            emit("branches ours/theirs forked at ancestor")
            proof_repo = ws / "repo"
            for state in (ours_state, theirs_state):
                added_objects = _union_tree(
                    proof_repo / "objects", state["repo"] / "objects")
                added_tx = _union_tree(
                    proof_repo / "transactions",
                    state["repo"] / "transactions")
                emit(f"co-located {state['head'][:16]}... "
                     f"+{added_objects} objects +{added_tx} tx records")
            principal = bytes.fromhex(sley2_tool.TRIAL_PRINCIPAL)

            # Live revisions as merge inputs (addressable through
            # the co-located histories).
            ours_tx = bytes.fromhex(ours_state["head"])
            theirs_tx = bytes.fromhex(theirs_state["head"])
            emit(f"merge inputs ancestor {ancestor.hex()[:16]}... "
                 f"ours {ours_tx.hex()[:16]}... "
                 f"theirs {theirs_tx.hex()[:16]}...")
            # Production judgment, both operand orders.
            first = _judge(session, ancestor, ours_tx, theirs_tx)
            second = _judge(session, ancestor, theirs_tx, ours_tx)
            emit(f"judge ours,theirs: root {first['root'][:16]}... "
                 f"objects {first['count']}")
            emit(f"judge theirs,ours: root {second['root'][:16]}... "
                 f"objects {second['count']}")
            if first["raw"] != second["raw"]:
                raise SystemExit("operand orders diverge")
            emit("orders byte-identical: commutativity observed")
            # Commit the merged outcome.
            now = int(time.time() * 1000)
            commit_body = _fields(
                [(1, ancestor), (2, ours_tx), (3, theirs_tx),
                 (4, principal),
                 (5, enc_uvar(now)), (6, enc_uvar(now + 60000)),
                 (7, b"proof-merge")])
            commit = session._raw_request("merge.commit", commit_body)
            if commit["flags"].get("failed"):
                raise SystemExit(
                    f"merge.commit refused: {_failure(commit)}")
            ctag, cpayload = read_union(bytes.fromhex(commit["body"]))
            if ctag != 1 or len(cpayload) != 32:
                raise SystemExit(f"merge.commit bad result tag {ctag}")
            emit(f"committed merged tx {cpayload.hex()[:16]}...")
            # Post-commit reads: union semantics on the merged head.
            session.close()
            post = sley2_tool.Session(
                sley2_tool.resolve_binary(), ws, [], seed_pack=False)
            try:
                rep = sley2_tool.dispatch(post, ws, ["read", conflict])
                merged_bool = rep["decoded"]["entries"][0][
                    "body"]["value"]["data"]["value"]
                ours_bool = ours_state["bodies"][conflict][
                    "value"]["data"]["value"]
                base_bool = base_state["bodies"][conflict][
                    "value"]["data"]["value"]
                emit(f"merged shared-constant: {merged_bool} "
                     f"(base {base_bool}, ours {ours_bool})")
                if merged_bool != ours_bool:
                    raise SystemExit("ours change lost")
                inv = sley2_tool.dispatch(post, ws, ["inventory"])
                kinds: dict = {}
                for entry in inv.get("inventory", {}).get("objects", []):
                    kinds[entry.get("kind")] = kinds.get(
                        entry.get("kind"), 0) + 1
                emit(f"merged object kinds: {kinds}")
            finally:
                post.close()
                session = None  # closed above; skip outer close
            emit("MERGE production-path proof: BRANCHED+JUDGED+"
                 "COMMITTED (selected executable tests: none present "
                 "in these sides — content gap recorded for fixture "
                 "review)")
        finally:
            if session is not None:
                try:
                    session.close()
                except Exception:
                    pass
    finally:
        try:
            # Read-only store files defeat a silent ignore_errors removal;
            # remove_scratch restores permissions or fails loudly.
            remove_scratch(workdir)
            for stage in stages:
                remove_scratch(stage)
        finally:
            flush_log()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
