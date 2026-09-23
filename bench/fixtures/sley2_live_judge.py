"""Shared live-trial judge for the sley_2_0 arm (per-task entries route here).

The judge runs as the trial oracle subprocess (stdlib system python; the
pinned codecs are reached through the same uv service the trial tool
uses, and strict cases execute in-process through the frozen Rust case
driver). It judges the agent-produced workspace WITHOUT trusting it:

1. Loads the frozen corpus task, task manifest, and exclusion flag.
2. Reads final_candidate.hex (missing/malformed is rejected, never a
   lost slot).
3. Copies workspace/repo to scratch (the trial tree is never mutated).
4. Validates the candidate on a privileged scratch session, commits it,
   and runs the frozen strict cases through the Rust driver pre/post.
5. Applies the per-task judge spec (task_manifest.json) against frozen
   corpus cases: strict execution, collateral (added files decode to
   target entities only; nothing deleted), forbidden outcomes.
6. Prints exactly one JSON verdict; exit 0 accepted, 1 rejected,
   2 harness error — the run_fixture_oracle protocol.

Exclusions (EFFECT/CAP E7) reject with the frozen exclusion code, exactly
as their expect files do. Kind-specific flows (stale sequences, merge
judging, fuel, test entities, bounded maintenance) follow the judge spec;
unfinished extensions fail closed with explicit codes, never silent
acceptance.
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Callable


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


sys.path.insert(0, str(_repo_root()))
from bench.live import sley2_codecs  # noqa: E402
from bench.live.scratch import ScratchRemovalError, remove_scratch  # noqa: E402
from bench.live.sley2_tool import (  # noqa: E402
    _entity_body as _tool_entity_body,
    CHAIN_NAME,
    MAX_RESPONSE_BYTES,
    PROTOCOL_VERSION,
    REPORT_NAME,
    REPO_DIR,
    SESSION_TIMEOUT,
    PROFILE_ARGS,
    TOOL_VERSION,
    Session,
    Sley2ToolError,
    _hex,
    _parse_record_fields,
    _encode_record,
    _encode_uvar,
    _uvar,
    verify_transcript_chain,
)
from bench.sley2.runner import Endpoint, endpoint_offer  # noqa: E402

JUDGE_NOW_MILLIS = 1780000000000
TASK_DIR = _repo_root() / "bench" / "fixtures" / "sley2"
CORPUS = _repo_root() / "bench" / "corpus" / "v1" / "tasks.json"
JUDGE_CASE_BINARY = "succ_live_judge_cases"


class JudgeHarnessError(ValueError):
    """The trial cannot be judged (harness error, exit 2)."""


class JudgeRejection(ValueError):
    """The trial fails (rejected with a code, exit 1)."""

    def __init__(self, code: str, detail: str = "") -> None:
        super().__init__(code)
        self.code = code
        self.detail = detail


def _reject(code: str, detail: str = "") -> None:
    raise JudgeRejection(code, detail)


def _harness_fail(detail: str) -> None:
    raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: {detail}")


def _emit(task_id: str, status: str, code: str | None, detail: str, exit_code: int) -> int:
    print(json.dumps({"status": status, "code": code, "detail": detail[:200],
                      "task_id": task_id, "arm": "sley2"}, sort_keys=True))
    return exit_code


def _load_json(path: Path) -> dict:
    try:
        value = json.loads(path.read_bytes().decode("utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: read: {error}") from error
    if not isinstance(value, dict):
        _harness_fail("object")
    return value


def _task_corpus(task_id: str) -> dict:
    tasks = _load_json(CORPUS).get("tasks") or []
    for task in tasks:
        if isinstance(task, dict) and task.get("id") == task_id:
            return task
    _harness_fail("corpus task")
    raise AssertionError("unreachable")


def _resolve_binary() -> Path:
    raw = os.environ.get("SLEY2_SLEY_BINARY", "")
    if not raw or "\x00" in raw:
        _harness_fail("binary unbound (SLEY2_SLEY_BINARY)")
    path = Path(raw)
    try:
        if not path.is_file() or path.is_symlink() or not os.access(path, os.X_OK):
            _harness_fail("binary unbound")
    except OSError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: binary: {error}") from error
    return path


def _exclusion(task_dir: Path, expect: dict) -> None:
    """Frozen task exclusions reject exactly as their expect files do."""

    if expect.get("exclusion") is True or (task_dir / "excluded.json").is_file():
        code = "ORACLE_EXCLUDED"
        try:
            detail = json.loads((task_dir / "excluded.json").read_text(encoding="utf-8")).get("reason", "")
        except (OSError, ValueError, AttributeError):
            detail = "frozen exclusion"
        _reject(code, str(detail)[:200])


def _candidate_bytes(workspace: Path) -> bytes:
    path = workspace / "final_candidate.hex"
    try:
        text = path.read_text(encoding="utf-8").strip()
    except OSError:
        _reject("ORACLE_FINAL_MISSING", "final_candidate.hex absent")
        raise AssertionError("unreachable")
    if len(text) % 2 or not text or any(
        ch not in "0123456789abcdef" for ch in text
    ):
        _reject("ORACLE_FINAL_MALFORMED", "final_candidate.hex not lowercase hex")
        raise AssertionError("unreachable")
    if len(text) > 4_000_000:
        _reject("ORACLE_FINAL_MALFORMED", "final_candidate.hex oversize")
        raise AssertionError("unreachable")
    return bytes.fromhex(text)


def _scratch_copy(repo: Path, workdir: Path) -> Path:
    if not repo.is_dir() or repo.is_symlink():
        _harness_fail("repo")
    scratch = workdir / "scratch"
    try:
        shutil.copytree(repo, scratch, symlinks=False)
    except OSError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: copy: {error}") from error
    return scratch


def _run_driver(repo: Path, function: str, cases: list) -> dict:
    """Strict cases through the frozen Rust driver (values, codes, fuel)."""

    binary = _cargo_test_binary()
    env = dict(os.environ)
    env["SUCC_JUDGE_REPO"] = str(repo)
    env["SUCC_JUDGE_FUNCTION"] = function
    env["SUCC_JUDGE_CASES"] = json.dumps(cases)
    try:
        completed = subprocess.run(
            [str(binary), "--exact", "live_case_driver", "--nocapture"],
            capture_output=True, timeout=240, check=False, env=env,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: driver: {error}") from error
    for line in completed.stdout.decode("utf-8", "replace").splitlines():
        if line.startswith("LIVE_JUDGE_RESULT "):
            try:
                verdict = json.loads(line[len("LIVE_JUDGE_RESULT "):])
            except json.JSONDecodeError as error:
                raise JudgeHarnessError(
                    f"LIVE_SLEY2_JUDGE_INVALID: verdict: {error}") from error
            if isinstance(verdict, dict):
                # Fail-closed driver boundary: harness-side failures
                # (malformed inputs, unrenderable values, top-level
                # driver errors) are never candidate evidence.
                if verdict.get("harness_error"):
                    raise JudgeHarnessError(
                        f"LIVE_SLEY2_JUDGE_INVALID: driver harness: "
                        f"{verdict['harness_error']}"[:200])
                for case in verdict.get("cases") or []:
                    if (isinstance(case, dict)
                            and isinstance(case.get("code"), str)
                            and case["code"].startswith("DRIVER_")):
                        raise JudgeHarnessError(
                            f"LIVE_SLEY2_JUDGE_INVALID: {case['code']}"[:200])
            return verdict
    raise JudgeHarnessError("LIVE_SLEY2_JUDGE_INVALID: no verdict")


def _cargo_test_binary() -> Path:
    # Explicit binding only (passed through the oracle boundary by the
    # router): never guess a target directory.
    raw = os.environ.get("SUCC_JUDGE_TEST_BINARY", "")
    if not raw or "\x00" in raw:
        _harness_fail("driver binary unbound (SUCC_JUDGE_TEST_BINARY)")
    candidate = Path(raw)
    try:
        if candidate.is_file() and not candidate.is_symlink() and os.access(candidate, os.X_OK):
            return candidate
    except OSError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: driver: {error}") from error
    _harness_fail("driver binary unbound (SUCC_JUDGE_TEST_BINARY)")
    raise AssertionError("unreachable")


def _commit_candidate(session: Session, principal: str, record_hex: str) -> str:
    head_tx = session.head["tx"]
    body = _encode_commit_body(head_tx, principal, record_hex)
    reply = session._raw_request("commit", body)
    if reply["flags"].get("failed"):
        raise JudgeRejection("ORACLE_COMMIT_REJECTED", (reply.get("body") or "")[:120])
    fields = _parse_record_fields(bytes.fromhex(reply["body"]))
    return fields[1].hex()


def _encode_commit_body(head_tx: str, principal: str, stored_hex: str) -> str:
    """Commit request: (parent tx, principal, now, STORED bytes).

    Field 4 carries STORED bytes (magic, version, sized record, digest
    trailer), never the bare record. The parameter is named stored_hex
    to keep that binding explicit.
    """

    return _encode_record([
        (1, bytes.fromhex(head_tx)),
        (2, bytes.fromhex(principal)),
        (3, _encode_uvar(JUDGE_NOW_MILLIS)),
        (4, bytes.fromhex(stored_hex)),
    ]).hex()


def _encode_validate_body(head_tx: str, principal: str, stored_hex: str) -> str:
    """Validation request: (base tx, principal, now, STORED bytes).

    Byte layout coincides with the commit encoder by protocol design,
    but the method semantics differ (validate never advances the head).
    Callers must use this encoder for candidate.validate; the commit
    encoder is not interchangeable, even where bytes coincide.
    """

    return _encode_record([
        (1, bytes.fromhex(head_tx)),
        (2, bytes.fromhex(principal)),
        (3, _encode_uvar(JUDGE_NOW_MILLIS)),
        (4, bytes.fromhex(stored_hex)),
    ]).hex()


def _require_valid_decision(result_body_hex: str, code: str = "ORACLE_REBASE_INVALID") -> dict:
    """Decode the owner validation result and require the Valid decision.

    A successful candidate.validate response envelope (failed=false) is
    not sufficient: negative decisions arrive with failed=false at the
    protocol level. Only decision tag 1 (Valid) counts; anything else
    rejects with the given code. A successful envelope carrying Invalid
    therefore cannot produce task acceptance.
    """

    try:
        [decoded] = sley2_codecs.run_batch(
            [{"op": "decode_result", "body": result_body_hex}])
    except sley2_codecs.CodecError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: result: {error}") from error
    if decoded.get("decision_tag") != 1:
        _reject(code, f"decision={decoded.get('decision_tag')} "
                      f"phase={decoded.get('failed_phase')}")
    return decoded


def _check_cases(cases: list, results: list, expected: list) -> None:
    if len(results) != len(expected):
        _harness_fail("case count")
    for case, result, want in zip(cases, results, expected):
        if not isinstance(result, dict) or "ok" not in result:
            _harness_fail("case shape")
        if "result" in want:
            if not result.get("ok"):
                _reject("ORACLE_CASE_MISMATCH", json.dumps(result)[:160])
            if result.get("value") != {"SInt": str(want["result"])}:
                _reject("ORACLE_CASE_MISMATCH", json.dumps(result)[:160])
        elif "failure" in want:
            if result.get("ok") or result.get("code") != want["failure"]:
                _reject("ORACLE_CASE_MISMATCH", json.dumps(result)[:160])
        else:
            _harness_fail("case expectation")


def _combine_clamp(below: bool, above: bool, inputs: list) -> int:
    x, lo, hi = inputs
    if below:
        return lo
    if above:
        return hi
    return x


def _judge_execute_cases(session: Session, manifest: dict, corpus: dict, task_id: str,
                         scratch_ws: Path, candidate: bytes, current: set[str],
                         task_dir: Path | None = None) -> None:
    """Execution-kind tasks: strict corpus cases against named functions.

    Clamp-combine tasks (REPAIR, REPAIR-shaped ADVERSARY) run below/above
    predicates per the corpus triples (the S3 clamp driver pattern) with
    task-specific mismatch codes; SIG checks its caller/call records;
    CORRUPT checks the exchange-pack rejection path (acceptance) beside
    the restored-constant smoke check; every other execution task
    judges the manifest entry function directly."""

    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    repo = scratch_ws / REPO_DIR
    if judge.get("combine") == "clamp" or task_id == "S2B-REPAIR-001":
        functions = judge.get("functions", {}) if isinstance(judge.get("functions"), dict) else {}
        below_role = functions.get("below", "below_func")
        above_role = functions.get("above", "above_func")
        below = entities.get(below_role, "")
        above = entities.get(above_role, "")
        if not below or not above:
            _harness_fail("clamp functions")
        cases = corpus.get("strict_oracle", {}).get("cases") or []
        if not cases:
            # ADVERSARY is REPAIR-shaped per the frozen emitter: it reuses
            # the REPAIR corpus triples through the same clamp combination.
            cases = _corpus_cases("S2B-REPAIR-001")
        mismatch = ("ORACLE_CLAMP_MISMATCH" if task_id == "S2B-REPAIR-001"
                    else "ORACLE_REPAIR_MISMATCH")
        below_cases = [{"inputs": [{"type": "SInt", "bits": 64, "value": c["input"][0]},
                                    {"type": "SInt", "bits": 64, "value": c["input"][1]}]}
                       for c in cases]
        above_cases = [{"inputs": [{"type": "SInt", "bits": 64, "value": c["input"][0]},
                                    {"type": "SInt", "bits": 64, "value": c["input"][2]}]}
                       for c in cases]
        below_out = _run_driver(repo, below, below_cases).get("cases")
        above_out = _run_driver(repo, above, above_cases).get("cases")
        if not isinstance(below_out, list) or not isinstance(above_out, list):
            _harness_fail("case shape")
        if len(below_out) != len(cases) or len(above_out) != len(cases):
            _harness_fail("case count")
        for case, below_r, above_r in zip(cases, below_out, above_out):
            for result in (below_r, above_r):
                if not result.get("ok"):
                    # Signature changes surface the engine code verbatim
                    # (frozen S3 SIG-family: VM_EXEC_INPUT_COUNT_MISMATCH).
                    _reject(str(result.get("code") or mismatch)[:64],
                            json.dumps(result)[:160])
                if result.get("value", {}).get("Bool") is None:
                    _reject(mismatch, json.dumps(result)[:160])
            got = _combine_clamp(bool(below_r["value"]["Bool"]), bool(above_r["value"]["Bool"]),
                                 case["input"])
            if got != case["result"]:
                _reject(mismatch,
                        f"triple {case['input']}: got {got}, want {case['result']}")
        _judge_adversarial(session, manifest, candidate)
        return
    if task_id == "S2B-SIG-001":
        _judge_sig(session, manifest, scratch_ws, current)
        return
    entry = judge.get("entry", "")
    function = entities.get(entry, "")
    if not function:
        _harness_fail("entry function")
    if task_id == "S2B-CORRUPT-001":
        expected = manifest.get("judge", {}).get("corrupt", {}).get("expected", False)
        post = _run_driver(repo, function, []).get("cases")
        if not isinstance(post, list):
            _harness_fail("case shape")
        _check_cases([], post, [])
        _judge_corrupt_value(session, manifest)
        if task_dir is None:
            _harness_fail("corrupt pack dir")
        _judge_corrupt_exchange(task_dir)
        _judge_corrupt_pack_resealed(task_dir)
        return
    post = _run_driver(repo, function, _driver_cases(corpus)).get("cases")
    if not isinstance(post, list):
        _harness_fail("case shape")
    cases = corpus.get("strict_oracle", {}).get("cases") or []
    _check_cases(cases, post, cases)


def _corpus_cases(task_id: str) -> list:
    """Strict-oracle cases of another frozen corpus task (ADVERSARY
    reuses the REPAIR triples per the frozen emitter)."""

    tasks = _load_json(CORPUS).get("tasks") or []
    for task in tasks:
        if isinstance(task, dict) and task.get("id") == task_id:
            return task.get("strict_oracle", {}).get("cases") or []
    _harness_fail("corpus task")
    raise AssertionError("unreachable")




def _read_manifest(task_dir: Path) -> dict:
    try:
        manifest = _load_json(task_dir / "task_manifest.json")
    except JudgeHarnessError as error:
        # No frozen manifest (e.g. CREATE-001): live judging is not
        # implemented for this task. Harness error, never a verdict.
        raise JudgeHarnessError(
            f"LIVE_SLEY2_JUDGE_INVALID: no live manifest: {error}") from error
    for key in ("principal", "workspace", "policy_root", "entities", "targets", "judge"):
        if key not in manifest:
            _reject("ORACLE_MANIFEST_MISSING", f"task_manifest.json lacks {key}")
    return manifest




def _snapshot_versions(connect: object, workspace: Path) -> dict[str, str]:
    """entity -> current object bindings through entity.version calls.

    Object files are immutable: repos hold stale versions beside
    current ones, so currency comes from the server (tombstones fail
    and drop out), never from file order. Sessions cap requests
    (frozen protocol limit), so entities are queried in small chunks
    across fresh sessions."""

    probe = connect()
    try:
        entities = _repo_entities(probe, workspace)
    finally:
        probe.close()
    versions: dict[str, str] = {}
    for index in range(0, len(entities), 8):
        session = connect()
        try:
            for entity in entities[index:index + 8]:
                try:
                    reply = session._raw_request(
                        "entity.version", _tool_entity_body(session, entity))
                except Exception:
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
                entries = decoded["decoded"].get("entries") or []
                if len(entries) != 1:
                    continue
                versions[entity] = entries[0].get("object_id", "")
        finally:
            session.close()
    return versions


def _repo_entities(session: Session, workspace: Path) -> list[str]:
    """Distinct entity ids across served object files (any version)."""

    calls = []
    for path in _store_files(workspace / REPO_DIR):
        try:
            calls.append({"op": "decode_object", "stored": path.read_bytes().hex(),
                          "epoch": session.head["epoch"]})
        except OSError as error:
            raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: inventory: {error}") from error
    try:
        results = sley2_codecs.run_batch(calls) if calls else []
    except sley2_codecs.CodecError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: inventory: {error}") from error
    found = set()
    for result in results:
        entity = result["decoded"].get("entity_id", "")
        if entity:
            found.add(entity)
    return sorted(found)


def _judge_graph(session: Session, manifest: dict, versions_pre: dict[str, str],
                 versions_post: dict[str, str], scratch_ws: Path | None = None) -> None:
    """Graph-kind tasks: absent roles tombstoned, expected roles present,
    non-target bindings byte-identical.

    Structure owned by an absent role in the PRE state (a function's
    parameters and blocks, a block's operations, transitively) follows
    its owner, as the frozen DEAD manifest notes ("helper parameter and
    block follow their function"): it must be removed with its owner,
    never kept, modified or left orphaned. Ownership comes from the
    decoded pre-state bodies, never from a literal list."""

    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    targets = manifest.get("targets", []) if isinstance(manifest.get("targets"), list) else []
    absent = judge.get("absent", []) or []
    for role in absent:
        if role in entities and _entity_present(session, entities[role]):
            _reject("ORACLE_UNEXPECTED_ENTITY", role)
    if "expect_target" in judge:
        want = entities.get(judge["expect_target"], judge["expect_target"])
        if not _entity_present(session, want):
            _reject("ORACLE_MISSING_TARGET", str(judge["expect_target"])[:64])
    absent_ids = {entities[role] for role in absent if entities.get(role)}
    cascade: set[str] | None = None
    for entity, before in versions_pre.items():
        if entity in targets:
            continue
        if entity in absent_ids:
            continue
        if versions_post.get(entity) == before:
            continue
        if absent_ids and scratch_ws is not None:
            if cascade is None:
                cascade = _owned_by(session, scratch_ws, versions_pre, absent_ids)
            if entity in cascade:
                if entity in versions_post:
                    _reject("ORACLE_UNEXPECTED_ENTITY", f"owned by absent role: {entity[:32]}")
                continue
        _reject("ORACLE_COLLATERAL_TOUCHED", entity[:32])


# Structural owner field per entity kind: Parameter.owner, Block.function,
# Operation.block (frozen entity-body layouts).
_OWNER_FIELD = {6: "owner", 7: "function", 8: "block"}


def _owned_by(session: Session, scratch_ws: Path, versions_pre: dict[str, str],
              roots: set[str]) -> set[str]:
    """Pre-state entities whose structural owner chain reaches a root.

    Bodies decode from the PRE-commit object of each entity (objects are
    immutable, so the scratch repo still holds them); an unreadable pre
    object is a harness failure, never a silent pass."""

    repo = scratch_ws / REPO_DIR
    paths = []
    for entity in sorted(versions_pre):
        path = _object_path(repo, versions_pre[entity])
        if path is None:
            _harness_fail(f"pre object {entity[:16]}")
        paths.append(path)
    owner: dict[str, str] = {}
    for entry in _decode_paths(session, paths):
        field = _OWNER_FIELD.get(entry.get("kind"))
        body = entry.get("body")
        entity = entry.get("entity_id", "")
        if field and entity and isinstance(body, dict) and isinstance(body.get(field), str):
            owner[entity] = body[field]
    return _owner_closure(owner, roots)


def _owner_closure(owner: dict[str, str], roots: set[str]) -> set[str]:
    """Entities whose owner chain (child -> owner) reaches any root."""

    owned: set[str] = set()
    for entity in owner:
        seen = {entity}
        node = owner[entity]
        while True:
            if node in roots:
                owned.add(entity)
                break
            if node in seen or node not in owner:
                break
            seen.add(node)
            node = owner[node]
    return owned


def _entity_present(session: Session, entity: str) -> bool:
    """Whether entity.version resolves post-commit (tombstones fail)."""

    if not entity:
        return False
    try:
        reply = session._raw_request("entity.version", _tool_entity_body(session, entity))
    except Exception:
        return False
    return not reply["flags"].get("failed")


def _object_id_of(path: Path) -> str | None:
    """Object id for a store file: flat 64-hex stems, or sharded
    prefix dirs (object stores shard large repos by digest prefix)."""

    import string as _string

    hexdigits = set(_string.hexdigits)
    parts = [path.stem]
    node = path.parent
    while sum(len(part) for part in parts) < 64 and node.name != "scb1":
        if not node.name or any(c not in hexdigits for c in node.name):
            return None
        parts.append(node.name)
        node = node.parent
    digest = "".join(reversed(parts))
    if len(digest) != 64 or any(c not in hexdigits for c in digest):
        return None
    return digest.lower()


def _object_path(repo: Path, object_id: str) -> Path | None:
    """File holding an object id (flat or sharded layout; sharded
    files keep their full digest names under prefix dirs)."""

    base = repo / "objects" / "scb1"
    candidates = [base / f"{object_id}.scb1"]
    for depth in (1, 2, 3):
        node = base
        for width in range(depth):
            node = node / object_id[width * 2:width * 2 + 2]
        candidates.append(node / f"{object_id}.scb1")
    for candidate in candidates:
        try:
            if candidate.is_file() and not candidate.is_symlink():
                return candidate
        except OSError:
            return None
    return None


def _store_files(repo: Path) -> list[Path]:
    """All object files under a repo, any layout."""

    base = repo / "objects" / "scb1"
    try:
        paths = sorted(base.rglob("*.scb1")) if base.is_dir() else []
    except OSError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: inventory: {error}") from error
    out = []
    for path in paths:
        try:
            if path.is_symlink() or not path.is_file():
                continue
            if _object_id_of(path) is None:
                continue
            out.append(path)
        except OSError as error:
            raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: inventory: {error}") from error
    return out


def _current_paths(scratch_ws: Path, current: set[str]) -> list[Path]:
    """Object files holding current versions (stale versions excluded)."""

    return [p for p in _store_files(scratch_ws / REPO_DIR)
            if (_object_id_of(p) or "") in current]


def _decode_paths(session: Session, paths: list[Path]) -> list[dict]:
    """decode_object over file paths (verified envelopes)."""

    calls = []
    for path in paths:
        try:
            calls.append({"op": "decode_object", "stored": path.read_bytes().hex(),
                          "epoch": session.head["epoch"]})
        except OSError as error:
            raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: inventory: {error}") from error
    try:
        results = sley2_codecs.run_batch(calls) if calls else []
    except sley2_codecs.CodecError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: inventory: {error}") from error
    return [r["decoded"] for r in results]


def _decode_body(session: Session, scratch_ws: Path, entity: str,
                 current: set[str]) -> dict:
    """Typed body of one entity's CURRENT version from the served repo."""

    for entry in _decode_paths(session, _current_paths(scratch_ws, current)):
        if entry.get("entity_id") == entity:
            body = entry.get("body")
            if not isinstance(body, dict):
                _harness_fail("entity body")
            return body
    _reject("ORACLE_MISSING_TARGET", entity[:32])
    raise AssertionError("unreachable")


def _seeded_repo(task_dir: Path) -> tuple[Path, object, object]:
    """Pristine H1-equivalent repo seeded from the task base pack.

    Returns (repo_path, session, cleanup). The caller runs the driver
    against repo_path (pre-state behavior) then calls cleanup."""

    pack = task_dir / "base.pack"
    try:
        if not pack.is_file() or pack.is_symlink():
            _harness_fail("base pack")
    except OSError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: base pack: {error}") from error
    workdir = Path(tempfile.mkdtemp(prefix="sley2-pristine-"))
    ws, session = _staged_session(workdir, pack, "base pack")

    def cleanup() -> None:
        try:
            session.close()
        finally:
            _remove_workdir(workdir)

    return ws / REPO_DIR, session, cleanup


def _remove_workdir(workdir: Path) -> None:
    """Remove a judge scratch workspace or fail the judgment loudly.

    Scratch trees hold read-only store and tool files; a silent
    `ignore_errors` removal leaked ~20k inodes per run, so removal
    restores owner permissions and retries, and a tree that still cannot
    be removed is a harness error, never a quiet leak."""
    try:
        remove_scratch(workdir)
    except ScratchRemovalError as error:
        raise JudgeHarnessError(
            f"LIVE_SLEY2_JUDGE_INVALID: scratch removal: {error}") from error


def _staged_session(workdir: Path, pack: Path, what: str) -> tuple[Path, object]:
    """Stage `pack` into a fresh workspace under `workdir` and seed a
    session; `workdir` is removed if staging fails."""
    try:
        ws = workdir / "ws"
        ws.mkdir(mode=0o700)
        (ws / REPO_DIR).mkdir(mode=0o700)
        try:
            shutil.copyfile(pack, ws / "base.pack")
        except OSError as error:
            raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: {what}: {error}") from error
        return ws, Session(_resolve_binary(), ws, [], seed_pack=True)
    except BaseException:
        _remove_workdir(workdir)
        raise


def _sint_cases(inputs: list) -> list:
    """Driver cases for fixed SInt64 input lists (pairs or wider)."""

    cases = []
    for values in inputs:
        if (not isinstance(values, list) or not values
                or not all(isinstance(v, int) for v in values)):
            _harness_fail("fixed inputs")
        cases.append({"inputs": [{"type": "SInt", "bits": 64, "value": value}
                                 for value in values]})
    return cases


def _judge_sig(session: Session, manifest: dict, scratch_ws: Path,
               current: set[str]) -> None:
    """SIG task: three explicit caller records bound to the migrated
    callee. Callers are CallDirect operations naming the callee with
    two explicit SInt operands each (frozen S3 shape: test-case records
    bound to the migrated signature); the callee keeps its 2-arity; the
    callee executes on the fixed inputs."""

    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    callee = entities.get(judge.get("callee", "callee_func"), "")
    if not callee:
        _harness_fail("sig callee")
    arity = int(judge.get("expect_arity", 2))
    calls = _call_direct_ops(session, scratch_ws, callee, current)
    if len(calls) < 3:
        # Frozen S3 family: an omitted caller is a TYPE_ERROR rejection.
        _reject("TYPE_ERROR", f"found {len(calls)} caller records")
    blocks = set()
    for call in calls:
        operands = call.get("operands") or []
        if len(operands) < arity:
            _reject("ORACLE_AMBIENT_DEFAULT",
                    f"{len(operands)} explicit inputs < arity {arity}")
        blocks.add(call.get("block", ""))
    if len(blocks) < 3:
        _reject("TYPE_ERROR", "callers share blocks")
    body = _decode_body(session, scratch_ws, callee, current)
    if len(body.get("parameters") or []) != arity:
        _reject("ORACLE_SIG_ARITY", f"want {arity}")
    fixed = judge.get("fixed_inputs", [[7, 10]])
    results = _run_driver(scratch_ws / REPO_DIR, callee, _sint_cases(fixed)).get("cases")
    if not isinstance(results, list) or len(results) != len(fixed):
        _harness_fail("case shape")
    for result in results:
        if not result.get("ok"):
            _reject(str(result.get("code") or "ORACLE_SIG_FAILED")[:64],
                    json.dumps(result)[:160])


def _call_direct_ops(session: Session, scratch_ws: Path, callee: str,
                     current: set[str]) -> list[dict]:
    """Decoded CURRENT kind-8 OperationBodies that are CallDirect
    (opcode 112) naming the callee through the function immediate."""

    found = []
    for entry in _decode_paths(session, _current_paths(scratch_ws, current)):
        if entry.get("kind") != 8:
            continue
        body_hex = entry.get("body_hex", "")
        if not isinstance(body_hex, str) or not body_hex:
            continue
        try:
            [typed] = sley2_codecs.run_batch([{
                "op": "decode_value", "type": "OperationBody", "data": body_hex,
            }])
        except (sley2_codecs.CodecError, KeyError):
            continue
        body = typed["value"]
        if not isinstance(body, dict) or body.get("opcode") != 112:
            continue
        if _immediate_function(body.get("immediate")) == callee:
            found.append(body)
    return found


def _immediate_function(immediate: object) -> str:
    """Callee hex when an Immediate names a function, else empty."""

    if not isinstance(immediate, dict):
        return ""
    if immediate.get("variant") == "Function":
        value = immediate.get("value") or {}
        if isinstance(value, dict) and isinstance(value.get("function"), str):
            return value["function"]
    if isinstance(immediate.get("function"), str):
        return immediate["function"]
    return ""


# Fixed explicit Failed code the execution check drives through the
# migrated switch. The frozen corpus pins no literal; 7 is the frozen S3
# test-vector code (s3_g1_type.rs `JobState::Failed(7)`) and fits every
# signed/unsigned integer width a code may declare.
TYPE_EXEC_FAILED_CODE = 7
_TYPE_CODE_VARIANTS = ("SInt", "UInt")
# Entity kinds (sley-mutate EntityBodyValueKind).
_KIND_TYPEDEF, _KIND_FUNCTION, _KIND_PARAMETER = 4, 5, 6
_KIND_CONSTANT, _KIND_GLOBAL = 9, 10


def _judge_type_variant(session: Session, manifest: dict, scratch_ws: Path,
                        versions_pre: dict[str, str],
                        versions_post: dict[str, str]) -> str:
    """TYPE task: JobState migration (frozen corpus S2B-TYPE-001).

    Frozen text (bench/corpus/v1/tasks.json S2B-TYPE-001): replace a
    running boolean with a JobState tagged variant (Queued, Running,
    Succeeded, Failed(error_code)) and update constructors, switches,
    and tests; required outcomes: no boolean status binding remains,
    all four cases exhaustively handled, Failed carries an explicit
    error code, serialized semantic values deterministic; forbidden:
    implicit default case, null error, parallel boolean compatibility
    field; strict oracle `type_graph_and_execution`, variant_cases 4,
    exhaustive.

    Every predicate below is derived from that text; none is a
    witness-shape choice (the switch result type, how arms compute
    their values, whether the Failed arm returns the code raw, maps it, or discards
    it, and how many blocks / join blocks the switch uses are all
    free):

    - status constructor: the status constant is a Named value of a
      tagged variant with exactly four members, three unit members and
      one member carrying an integer (SInt or UInt, any width) error
      code; a value of the coded member carries an explicit code
      (null payload -> ORACLE_FAILED_CODE; no coded member at all ->
      ORACLE_FAILED_CODE, the "null error" outcome). The corpus pins
      no status value and no code literal.
    - no boolean status binding / no parallel boolean compatibility
      field: every Constant, Parameter (function or block), GlobalValue
      and TypeDef member among the fresh and target entities, plus the
      switch result type, is Bool-free (recursively) ->
      ORACLE_BOOL_COMPAT_FIELD. Operation results (transient values,
      e.g. an arm-internal comparison) are not bindings.
    - switches updated: the switch function is a function of the
      status alone (one parameter, Named JobState; the base switch has
      exactly one status input) and dispatches on it through a
      VariantSwitch on that parameter, in any reachable block
      (ORACLE_SWITCH_NOT_MIGRATED).
    - exhaustive handling: every such VariantSwitch has Member keys
      exactly covering the four members (no default key exists in the
      terminator form, so no implicit default); every block reachable
      from the entry is Required and none is a Trap
      (ORACLE_MISSING_CASE / ORACLE_TRAP_ARM). How the Failed arm uses
      its code is free: it may bind it (CasePayload), map it, or
      discard it (the frozen S3 reference switch is
      `JobState::Failed(_) => "failed"`, s3_g1_type.rs:199). "Failed
      carries an explicit error code" / "null error" govern the
      variant type and its values (typedef + status rules above), not
      the consumer shape (revision 3; Ariadne r2 P1).
    - execution (strict oracle `type_graph_and_execution`): the
      migrated switch executes through the frozen case driver over all
      four member values (Failed carrying the fixed explicit code
      TYPE_EXEC_FAILED_CODE, payload required by the driver, never
      defaulted); every case returns a value (a trap, fuel exhaustion
      or engine error -> ORACLE_MISSING_CASE) and a second execution
      returns identical values (-> ORACLE_NONDETERMINISTIC).

    Tests: the base holds no TestCase entities and the frozen text
    names no test vectors; the judge-driven execution above is the
    test evidence. Closure (FIXTURE tier, review-gated): the
    task_manifest targets (status 0x65, switch 0x6b, its parameter
    0x6c, switch_entry 0x6d, switch_leaf 0x6e) bound which base
    entities may change; fresh entities are always allowed and are
    judged by the Bool scan above. The named `switch_param` role
    (0x6c) is a required manifest contract and is always in the Bool
    scan scope, so a switch that gains a second (Bool) parameter is
    judged (ORACLE_BOOL_COMPAT_FIELD), never a harness error.

    Returns the verdict suffix naming the executed per-member values.
    """

    current = set(versions_post.values())
    fresh = set(versions_post) - set(versions_pre)
    bodies = _type_current_bodies(session, scratch_ws, current)
    plan = _type_structure(manifest, bodies, fresh)
    values = _type_execute(scratch_ws / REPO_DIR, plan)
    return "type exec " + ",".join(
        f"{member[:4]}={json.dumps(value, sort_keys=True, separators=(',', ':'))}"
        for member, value in zip(plan["members"], values))


def _type_current_bodies(session: Session, scratch_ws: Path,
                         current: set[str]) -> dict[str, dict]:
    """entity -> {"kind", "body"} for every CURRENT served version."""

    found: dict[str, dict] = {}
    for entry in _decode_paths(session, _current_paths(scratch_ws, current)):
        entity = entry.get("entity_id", "")
        body = entry.get("body")
        if isinstance(entity, str) and entity and isinstance(body, dict):
            found[entity] = {"kind": entry.get("kind"), "body": body}
    return found


def _type_has_bool(type_expr: object) -> bool:
    """Whether a decoded TypeExpr is or contains Bool (Option(Bool),
    Tuple(.., Bool), Vector(Bool), Result arms, map keys/values)."""

    if isinstance(type_expr, dict):
        if type_expr.get("variant") == "Bool":
            return True
        return any(_type_has_bool(value) for value in type_expr.values())
    if isinstance(type_expr, list):
        return any(_type_has_bool(value) for value in type_expr)
    return False


def _type_body(bodies: dict[str, dict], entity: object, what: str) -> dict:
    item = bodies.get(entity) if isinstance(entity, str) else None
    if item is None:
        _reject("ORACLE_MISSING_TARGET", f"{what} {str(entity)[:32]}")
        raise AssertionError("unreachable")
    return item["body"]


def _type_bool_scan(bodies: dict[str, dict], scope: list[str], switch: str) -> None:
    """Parallel boolean compatibility field: no Bool-typed binding among
    the fresh and target entities (constants, function and block
    parameters, globals, typedef members) and no Bool switch result."""

    for entity in scope:
        item = bodies.get(entity)
        if item is None:
            continue  # tombstoned target: nothing bound
        kind, body = item.get("kind"), item.get("body") or {}
        found: list[tuple[str, object]] = []
        if kind == _KIND_CONSTANT:
            found.append(("constant", (body.get("value") or {}).get("value_type")))
        elif kind == _KIND_PARAMETER:
            found.append((f"{str(body.get('role', '')).lower() or 'parameter'} parameter",
                          body.get("value_type")))
        elif kind == _KIND_GLOBAL:
            found.append(("global", body.get("value_type")))
        elif kind == _KIND_TYPEDEF:
            form = body.get("form") or {}
            for member in (form.get("value") or []) if isinstance(form, dict) else []:
                if isinstance(member, dict):
                    found.append(("typedef member", member.get("value_type")))
                    found.append(("typedef member", member.get("payload_type")))
        elif kind == _KIND_FUNCTION and entity == switch:
            found.append(("switch result", body.get("result_type")))
        for where, type_expr in found:
            if _type_has_bool(type_expr):
                _reject("ORACLE_BOOL_COMPAT_FIELD", f"{where} {entity[:16]} Bool-typed")


def _type_edge_targets(terminator: object) -> list[str]:
    """Every edge target named by a terminator (Branch, CondBranch,
    VariantSwitch cases), in terminator order."""

    targets: list[str] = []

    def walk(node: object) -> None:
        if isinstance(node, dict):
            if isinstance(node.get("target"), str) and "arguments" in node:
                targets.append(node["target"])
            for value in node.values():
                walk(value)
        elif isinstance(node, list):
            for value in node:
                walk(value)

    walk(terminator)
    return targets


def _type_structure(manifest: dict, bodies: dict[str, dict], fresh: set[str]) -> dict:
    """Graph half of `type_graph_and_execution` over decoded CURRENT
    bodies (pure; unit-tested over decoded-body fixtures). Returns the
    execution plan: switch function, typedef, members in definition
    order, and the Failed member."""

    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    targets = manifest.get("targets", []) if isinstance(manifest.get("targets"), list) else []
    switch = entities.get(judge.get("switch", "switch"), "")
    status = entities.get(judge.get("status", "status"), "")
    # The named switch_param role (the base switch's status input,
    # 0x6c) is a required manifest contract: its Bool type is the
    # boolean status binding the task removes, so it is always in the
    # Bool scan scope, independent of the targets list.
    param_role = entities.get("switch_param", "")
    if not switch or not status or not param_role:
        _harness_fail("type entities")
    if not judge.get("exhaustive", True):
        _harness_fail("exhaustive spec")
    want_cases = int(judge.get("variant_cases", 4))
    # --- status constructor: Named tagged variant ---
    value = _type_body(bodies, status, "status").get("value") or {}
    value_type = value.get("value_type") or {}
    if _type_has_bool(value_type):
        _reject("ORACLE_BOOL_COMPAT_FIELD", "status still Bool")
    if value_type.get("variant") != "Named":
        _reject("ORACLE_TYPE_NOT_MIGRATED", "status not Named")
    typedef_id = (value_type.get("value") or {}).get("definition", "")
    if not typedef_id or not isinstance(typedef_id, str):
        _reject("ORACLE_TYPE_NOT_MIGRATED", "status typedef")
    # --- typedef: four members, three unit + one integer-coded ---
    form = _type_body(bodies, typedef_id, "typedef").get("form") or {}
    members = form.get("value") if isinstance(form, dict) else None
    if not isinstance(form, dict) or form.get("variant") != "Variant":
        _reject("ORACLE_TYPE_NOT_MIGRATED", "status type not a tagged variant")
    if not isinstance(members, list) or len(members) != want_cases:
        _reject("ORACLE_TYPE_NOT_MIGRATED",
                f"typedef members {len(members) if isinstance(members, list) else '?'}"
                f" != {want_cases}")
    member_ids: list[str] = []
    coded: list[tuple[str, dict]] = []
    for member in members:
        if not isinstance(member, dict) or not isinstance(member.get("member_id"), str):
            _reject("ORACLE_TYPE_NOT_MIGRATED", "typedef member shape")
        member_ids.append(member["member_id"])
        payload_type = member.get("payload_type") or {}
        if payload_type.get("variant") == "Some":
            coded.append((member["member_id"], payload_type.get("value") or {}))
        elif payload_type.get("variant") != "None":
            _reject("ORACLE_TYPE_NOT_MIGRATED", "typedef payload shape")
    if len(set(member_ids)) != want_cases:
        _reject("ORACLE_TYPE_NOT_MIGRATED", "duplicate typedef members")
    if not coded:
        _reject("ORACLE_FAILED_CODE", "no member carries an error code (null error)")
    if len(coded) != 1:
        _reject("ORACLE_TYPE_NOT_MIGRATED",
                f"need {want_cases - 1} unit members + 1 coded member")
    failed_member, code_type = coded[0]
    if code_type.get("variant") not in _TYPE_CODE_VARIANTS:
        _reject("ORACLE_FAILED_CODE", "Failed payload not an integer error code")
    # --- status value: a member of the typedef; explicit code iff it
    # is the coded (Failed) member ---
    data = value.get("data") or {}
    variant = data.get("value") or {}
    if data.get("variant") != "Variant" or variant.get("definition") != typedef_id:
        _reject("ORACLE_TYPE_NOT_MIGRATED", "status data not a JobState value")
    status_member = variant.get("member_id", "")
    if status_member not in member_ids:
        _reject("ORACLE_TYPE_NOT_MIGRATED", "status member outside typedef")
    payload = variant.get("payload") or {}
    if status_member == failed_member:
        if payload.get("variant") != "Some":
            _reject("ORACLE_FAILED_CODE", "Failed null payload")
        inner_data = (payload.get("value") or {}).get("data") or {}
        if (inner_data.get("variant") not in _TYPE_CODE_VARIANTS
                or isinstance(inner_data.get("value"), bool)
                or not isinstance(inner_data.get("value"), int)):
            _reject("ORACLE_FAILED_CODE", "Failed code not an explicit integer")
    elif payload.get("variant") != "None":
        _reject("ORACLE_TYPE_NOT_MIGRATED", "unit member carries payload")
    # --- no Bool binding among fresh + target entities ---
    _type_bool_scan(bodies, sorted(fresh | {param_role}
                                   | {t for t in targets if isinstance(t, str)}), switch)
    # --- switch: a function of the status alone ---
    switch_body = _type_body(bodies, switch, "switch")
    params = switch_body.get("parameters") or []
    if len(params) != 1:
        _reject("ORACLE_SWITCH_NOT_MIGRATED",
                f"switch takes {len(params)} parameters, not the status alone")
    param_id = params[0]
    param_type = _type_body(bodies, param_id, "switch param").get("value_type") or {}
    if param_type.get("variant") != "Named" or (
            param_type.get("value") or {}).get("definition") != typedef_id:
        _reject("ORACLE_TYPE_NOT_MIGRATED", "switch param not Named JobState")
    blocks = [b for b in (switch_body.get("blocks") or []) if isinstance(b, str)]
    entry_id = switch_body.get("entry_block", "")
    if not entry_id or entry_id not in blocks:
        _harness_fail("switch entry")
    # --- reachable blocks: Required, never Trap ---
    reachable: list[str] = []
    pending = [entry_id]
    terminators: dict[str, dict] = {}
    while pending:
        block_id = pending.pop(0)
        if block_id in terminators:
            continue
        if block_id not in blocks:
            _reject("ORACLE_MISSING_CASE", f"edge target {block_id[:16]} outside switch")
        block = _type_body(bodies, block_id, "block")
        if block.get("reachability") != "Required":
            _reject("ORACLE_MISSING_CASE", f"block {block_id[:16]} not Required")
        terminator = block.get("terminator") or {}
        if terminator.get("variant") == "Trap":
            _reject("ORACLE_TRAP_ARM", f"block {block_id[:16]} trap")
        terminators[block_id] = terminator
        reachable.append(block_id)
        pending.extend(_type_edge_targets(terminator))
    # --- dispatch on the status input, exhaustively ---
    switches = []
    for block_id in reachable:
        terminator = terminators[block_id]
        if terminator.get("variant") != "VariantSwitch":
            continue
        selector = (terminator.get("value") or {}).get("value") or {}
        if selector.get("variant") == "Parameter" and selector.get("value") == param_id:
            switches.append(terminator.get("value") or {})
    if not switches:
        _reject("ORACLE_SWITCH_NOT_MIGRATED", "no VariantSwitch on the status input")
    for sw in switches:
        cases = [c for c in (sw.get("cases") or []) if isinstance(c, dict)]
        keys = []
        for case in cases:
            key = case.get("case_key") or {}
            if key.get("variant") != "Member":
                _reject("ORACLE_MISSING_CASE", "non-Member case key")
            keys.append(str(key.get("value", "")))
        if len(keys) != want_cases or set(keys) != set(member_ids):
            _reject("ORACLE_MISSING_CASE", f"{len(keys)} cases != the {want_cases} members")
        if keys != sorted(keys):
            _reject("ORACLE_MISSING_CASE", "cases not sorted")
    return {"switch": switch, "typedef": typedef_id, "members": member_ids,
            "failed": failed_member}


def _type_execute(repo: Path, plan: dict) -> list:
    """Execution half of `type_graph_and_execution`: all four member
    values through the frozen case driver, twice; every case returns a
    value and both runs agree. Returns the per-member values."""

    cases = []
    for member in plan["members"]:
        item: dict = {"member": member}
        if member == plan["failed"]:
            item["payload"] = {"value": TYPE_EXEC_FAILED_CODE}
        cases.append({"inputs": [item]})
    runs = []
    for _ in range(2):
        results = _run_driver(repo, plan["switch"], cases).get("cases")
        if not isinstance(results, list) or len(results) != len(cases):
            _harness_fail("type case shape")
        values = []
        for member, result in zip(plan["members"], results):
            value = result.get("value") if isinstance(result, dict) else None
            if (not isinstance(result, dict) or not result.get("ok") or value is None
                    or (isinstance(value, dict) and "non_success" in value)):
                _reject("ORACLE_MISSING_CASE",
                        f"member {member[:16]} returned no value: "
                        f"{json.dumps(result, sort_keys=True)[:120]}")
            values.append(value)
        runs.append(values)
    if runs[0] != runs[1]:
        _reject("ORACLE_NONDETERMINISTIC", "switch values differ between runs")
    return runs[0]


def _judge_graph_observation(session: Session, manifest: dict, task_id: str,
                             scratch_ws: Path, task_dir: Path,
                             versions_post: dict[str, str], current: set[str]) -> None:
    """MODULE/DEAD observation stability: the observable function runs the
    fixed inputs identically pre (pristine base pack) and post; MODULE
    additionally refuses stale imports and duplicate implementations,
    DEAD refuses a deleted public entry."""

    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    fixed = judge.get("fixed_inputs")
    if not fixed:
        return
    function = entities.get("checksum", "") or entities.get("live_func", "")
    if not function:
        _harness_fail("observable function")
    if function not in versions_post:
        code = ("ORACLE_PUBLIC_DELETED" if task_id == "S2B-DEAD-001"
                else "ORACLE_STALE_IMPORT")
        _reject(code, function[:32])
    post = _run_driver(scratch_ws / REPO_DIR, function, _sint_cases(fixed)).get("cases")
    if not isinstance(post, list) or len(post) != len(fixed):
        _harness_fail("case shape")
    for result in post:
        if not result.get("ok"):
            _reject("ORACLE_OBSERVATION_FAILED", json.dumps(result)[:160])
    repo_pre, _, cleanup = _seeded_repo(task_dir)
    try:
        pre = _run_driver(repo_pre, function, _sint_cases(fixed)).get("cases")
    finally:
        cleanup()
    if not isinstance(pre, list) or len(pre) != len(fixed):
        _harness_fail("pristine cases")
    pre_values = [c.get("value") if c.get("ok") else None for c in pre]
    post_values = [c.get("value") if c.get("ok") else None for c in post]
    if None in pre_values:
        _harness_fail("pristine execution")
    if pre_values != post_values:
        code = ("ORACLE_REACHABLE_CHANGED" if task_id == "S2B-DEAD-001"
                else "ORACLE_OBSERVATION_CHANGED")
        _reject(code, "observation digest diverged")
    if task_id == "S2B-MODULE-001":
        _judge_module_extras(session, manifest, scratch_ws, versions_post, function,
                             _sint_cases(fixed), post_values, current)
    if task_id == "S2B-DEAD-001" and not _entity_present(session, function):
        _reject("ORACLE_PUBLIC_DELETED", function[:32])


def _judge_module_extras(session: Session, manifest: dict, scratch_ws: Path,
                         versions_post: dict[str, str],
                         checksum: str, cases: list, witness_values: list,
                         current: set[str]) -> None:
    """MODULE extras: the integrity package exports the checksum (a
    stale import leaves it unexported), and no other live function
    reproduces the checksum on the fixed inputs (a copy-pasted
    implementation instead of an import would agree everywhere the
    checksum does)."""

    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    checksum_entity = entities.get("checksum", "")
    new_package = entities.get("new_package", "")
    if not checksum_entity or not new_package:
        _harness_fail("module roles")
    package = _decode_body(session, scratch_ws, new_package, current)
    exports = package.get("exports") or []
    if checksum_entity not in exports:
        _reject("ORACLE_STALE_IMPORT", f"exports={len(exports)}")
    functions = _function_entities(session, scratch_ws, current)
    for entity in functions:
        if entity == checksum or entity not in versions_post:
            continue
        try:
            results = _run_driver(scratch_ws / REPO_DIR, entity, cases).get("cases")
        except (JudgeRejection, JudgeHarnessError):
            raise
        except Exception:
            continue
        if not isinstance(results, list) or len(results) != len(cases):
            continue
        values = [c.get("value") if isinstance(c, dict) and c.get("ok") else None
                  for c in results]
        if None not in values and values == witness_values:
            _reject("ORACLE_DUPLICATE_IMPL", entity[:32])


def _function_entities(session: Session, scratch_ws: Path, current: set[str]) -> list[str]:
    """Entity ids of CURRENT kind-5 FunctionBody objects in the repo."""

    return [entry.get("entity_id", "")
            for entry in _decode_paths(session, _current_paths(scratch_ws, current))
            if entry.get("kind") == 5 and entry.get("entity_id")]


def main(task_id: str) -> int:
    try:
        return _main(task_id)
    except JudgeRejection as rejection:
        return _emit(task_id, "rejected", rejection.code, rejection.detail, 1)
    except JudgeHarnessError as error:
        return _emit(task_id, "harness_error", None, str(error)[:200], 2)
    except Exception as error:
        return _emit(task_id, "harness_error", None, f"internal {type(error).__name__}", 2)


def _main(task_id: str) -> int:
    if len(sys.argv) != 2:
        _harness_fail("argv")
    workspace = Path(sys.argv[1])
    try:
        if not workspace.is_dir() or workspace.is_symlink():
            _harness_fail("workspace")
    except OSError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: workspace: {error}") from error
    task_dir = TASK_DIR / task_id
    manifest = _read_manifest(task_dir)
    corpus = _task_corpus(task_id)
    _exclusion(task_dir, corpus.get("strict_oracle", {}))
    if manifest.get("task_id", task_id) != task_id:
        _reject("ORACLE_MANIFEST_MISSING", "task id mismatch")
    candidate = _candidate_bytes(workspace)
    sley = _resolve_binary()
    workdir = Path(tempfile.mkdtemp(prefix="sley2-judge-"))
    # Every path (accept, reject, harness error) removes the scratch copy.
    try:
        scratch_ws = workdir / "ws"
        scratch_ws.mkdir(mode=0o700)
        shutil.copytree(workspace / REPO_DIR, scratch_ws / REPO_DIR, symlinks=False)
        transcript: list = []

        def connect() -> Session:
            return Session(sley, scratch_ws, transcript, seed_pack=False)

        session = connect()
        try:
            judge_cfg = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
            flow = judge_cfg.get("flow", "")
            needs_versions = flow in ("graph", "test-entity", "type-variant", "create") or (
                flow == "execute-cases" and task_id == "S2B-SIG-001")
            versions_pre = _snapshot_versions(connect, scratch_ws) if needs_versions else {}
            pre_tx = session.head.get("tx", "")
            if not pre_tx:
                _harness_fail("pre head")
            _commit_candidate(session, manifest["principal"], candidate.hex())
            # Post-commit reads need a fresh session: commit advances the
            # head every token was minted under, and the endpoint refuses
            # new opens on the committed process.
            session.close()
            session = connect()
            versions_post = _snapshot_versions(connect, scratch_ws) if needs_versions else {}
            _collateral_files(session, scratch_ws, workspace, manifest, flow)
            suffix = _judge_flows(session, manifest, corpus, task_id, versions_pre, versions_post,
                                  scratch_ws, candidate, pre_tx, workspace, task_dir, transcript)
        finally:
            session.close()
    finally:
        _remove_workdir(workdir)
    detail = "all flows held" + (f"; {suffix}" if suffix else "")
    return _emit(task_id, "accepted", None, detail, 0)


def _collateral_files(session: Session, scratch_ws: Path, trial_ws: Path, manifest: dict,
                      flow: str = "") -> None:
    """Added files decode to target entities only; trial files persist.

    Fresh entities (absent pre-commit: new tests, parameters, blocks,
    fields, branch additions) are the agent's own additions and always
    allowed; the per-flow behavior gates judge them. Modified base
    entities must be targets."""

    _ = flow
    targets = manifest.get("targets", []) if isinstance(manifest.get("targets"), list) else []
    target_set = set(targets)

    def objects(ws: Path) -> dict[str, Path]:
        found: dict[str, Path] = {}
        for item in _store_files(ws / REPO_DIR):
            digest = _object_id_of(item) or ""
            if digest:
                found[digest] = item
        return found

    before = objects(trial_ws)
    after = objects(scratch_ws)
    for digest in before:
        if digest not in after:
            _reject("ORACLE_COLLATERAL_DELETED", digest[:16])
    added = [digest for digest in after if digest not in before]
    trial_entities = set(_repo_entities(session, trial_ws)) if added else set()
    for digest in added:
        path = after[digest]
        try:
            [decoded] = sley2_codecs.run_batch([{
                "op": "decode_object", "stored": path.read_bytes().hex(),
                "epoch": session.head["epoch"],
            }])
        except (OSError, sley2_codecs.CodecError, KeyError) as error:
            raise JudgeRejection("ORACLE_COLLATERAL_UNDECODABLE", str(error)[:120]) from error
        entry = decoded["decoded"]
        entity = entry.get("entity_id", "")
        if entity in target_set:
            continue
        if entity and entity not in trial_entities:
            continue
        _reject("ORACLE_COLLATERAL_TOUCHED", str(entity)[:32])


def _judge_flows(session: Session, manifest: dict, corpus: dict, task_id: str,
                 versions_pre: dict[str, str], versions_post: dict[str, str],
                 scratch_ws: Path, candidate: bytes, pre_tx: str,
                 workspace: Path, task_dir: Path, transcript: list) -> str | None:
    """Dispatch the frozen per-kind judging flow from the manifest.

    Returns an optional evidence suffix for the verdict detail
    (bounded-maintenance access counts; type-variant executed values);
    every other flow returns None."""

    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    flow = judge.get("flow", "")
    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    targets = manifest.get("targets", []) if isinstance(manifest.get("targets"), list) else []
    current = set(versions_post.values())
    suffix: str | None = None
    if flow == "execute-cases":
        _judge_execute_cases(session, manifest, corpus, task_id, scratch_ws, candidate,
                             current, task_dir)
    elif flow == "graph":
        # Observation first: behavior-change negatives must report
        # before absent checks (a trial that keeps dead code and
        # changes behavior is reachable_changed, not unexpected).
        _judge_graph_observation(session, manifest, task_id, scratch_ws, task_dir,
                                 versions_post, current)
        _judge_graph(session, manifest, versions_pre, versions_post, scratch_ws)
        _judge_reference_count(session, manifest, scratch_ws, current)
    elif flow == "test-entity":
        _judge_test_entity(session, manifest, corpus, scratch_ws, task_dir, current)
    elif flow == "stale-sequence":
        _judge_stale_sequence(session, manifest, corpus, scratch_ws, candidate, pre_tx)
    elif flow == "merge":
        _judge_merge(session, manifest, corpus, scratch_ws, workspace, candidate)
    elif flow == "perf":
        _judge_perf(session, manifest, corpus, scratch_ws, task_dir)
    elif flow == "bounded-maintenance":
        suffix = _judge_bounded(session, manifest, corpus, scratch_ws, transcript,
                                task_dir, workspace)
    elif flow == "create":
        _judge_create(session, manifest, corpus, scratch_ws, current)
    elif flow == "type-variant":
        suffix = _judge_type_variant(session, manifest, scratch_ws, versions_pre,
                                     versions_post)
    elif flow == "excluded-e7":
        _reject("ORACLE_EXCLUDED", "frozen E7 exclusion")
    else:
        _harness_fail(f"judge flow {flow!r}")
    _ = targets
    return suffix


def _driver_cases(corpus: dict) -> list:
    cases = corpus.get("strict_oracle", {}).get("cases") or []
    out = []
    for case in cases:
        if not isinstance(case, dict) or "input" not in case:
            _harness_fail("cases")
        out.append({"inputs": [
            {"type": "SInt", "bits": 64, "value": value} for value in case["input"]
        ]})
    return out


def _judge_reference_count(session: Session, manifest: dict, scratch_ws: Path,
                           current: set[str]) -> None:
    """Count CURRENT CallDirect operations naming the checksum function:
    decoded op bodies, not file counts."""

    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    if "reference_count" not in judge:
        return
    checksum = entities.get("checksum", "")
    if not checksum:
        _harness_fail("checksum entity")
    calls = [{"op": "decode_object", "stored": path.read_bytes().hex(),
              "epoch": session.head["epoch"]}
             for path in _current_paths(scratch_ws, current)]
    count = 0
    if calls:
        try:
            results = sley2_codecs.run_batch(calls)
        except sley2_codecs.CodecError as error:
            raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: inventory: {error}") from error
        for result in results:
            entry = result["decoded"]
            if entry.get("kind") != 8:
                continue
            try:
                [typed] = sley2_codecs.run_batch([{
                    "op": "decode_value", "type": "OperationBody",
                    "data": entry.get("body_hex", ""),
                }])
            except (sley2_codecs.CodecError, KeyError):
                continue
            body = typed["value"]
            if not isinstance(body, dict):
                continue
            # Opcode 112 is CallDirect; the immediate names the callee.
            if body.get("opcode") != 112:
                continue
            if _immediate_function(body.get("immediate")) == checksum:
                count += 1
    if count != judge["reference_count"]:
        # Frozen MODULE negative: the new package must export the
        # checksum; a wrong count means the import is stale/missing.
        code = ("ORACLE_STALE_IMPORT"
                if manifest.get("task_id", "") == "S2B-MODULE-001"
                else "ORACLE_REFERENCE_COUNT")
        _reject(code, f"got {count}, want {judge['reference_count']}")


def _stored_record_bytes(stored: bytes) -> bytes:
    """Inner candidate record from stored bytes (magic, version, sized
    record, digest     trailer): framing only, no digest verification (the
    server verified on commit)."""

    magic = b"SLEYCAN1"
    if not stored.startswith(magic):
        _harness_fail("stored magic")
    position = len(magic)
    try:
        version, width = _uvar(stored, position)
    except ValueError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: stored: {error}") from error
    position += width
    if version != 1:
        _harness_fail("stored version")
    try:
        size, width = _uvar(stored, position)
    except ValueError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: stored: {error}") from error
    position += width
    record = stored[position:position + size]
    if len(record) != size or position + size + 32 != len(stored):
        _harness_fail("stored framing")
    return record


def _judge_adversarial(session: Session, manifest: dict, candidate: bytes) -> None:
    """Adversarial metadata must not become authority: the committed
    record carries no capabilities beyond the empty trial projection
    over its own base context, and the policy root is unchanged."""

    adv = manifest.get("judge", {}).get("adversarial")
    if not isinstance(adv, dict):
        return
    fields = _parse_record_fields(_stored_record_bytes(candidate))
    recorded = fields.get(8, b"").hex()
    envelope = {}
    for tag, key in ((2, "workspace"), (4, "base_root"), (6, "policy_root"), (7, "principal")):
        raw = fields.get(tag, b"")
        if len(raw) != 32:
            _reject("ORACLE_CAPABILITY_GRANTED", f"envelope {tag}")
        envelope[key] = raw.hex()
    [capability] = sley2_codecs.run_batch([{
        "op": "capability_digest", "principal": envelope["principal"],
        "workspace": envelope["workspace"], "policy_root": envelope["policy_root"],
        "state_root": envelope["base_root"],
    }])
    if recorded != capability["digest"]:
        _reject("ORACLE_CAPABILITY_GRANTED", recorded[:32])
    if session.head.get("policy", "") != manifest.get("policy_root", ""):
        _reject("ORACLE_POLICY_CHANGED", session.head.get("policy", "")[:32])

def _judge_corrupt_exchange(task_dir: Path) -> None:
    """Exchange-trailer regression (judge-side; kept as a regression,
    never as the corpus PACK obligation): a bit-flipped exchange whose
    trailer is NOT resealed must fail import with the exact exchange
    digest symbol, and the destination ref must not move.

    Two independent single-bit corruptions (middle, last byte) both
    map to EXCHANGE_DIGEST_MISMATCH: the unkeyed exchange trailer gate
    (crates/sley-repo/src/exchange.rs:674-677) fires first only because
    the trailer is left stale. The corpus code PACK_DIGEST_MISMATCH is
    reached through the same `exchange.import` route once the trailer
    is resealed (the exchange owner runs the PACK owner's preflight on
    the embedded pack, exchange.rs:1384, and returns PACK_* verbatim,
    exchange.rs:273-281); that vector is `_judge_corrupt_pack_resealed`.
    The judge corrupts and imports these bytes itself through the
    privileged `_raw_request`; the trial agent never touches them. The
    destination head transaction and live object count are identical
    before and after each rejected import. The constant-value
    restoration checked beside this is a separate smoke test, never
    this task's acceptance evidence."""

    pack_path = task_dir / "base.pack"
    try:
        pack = pack_path.read_bytes()
    except OSError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: pack: {error}") from error
    if len(pack) < 64:
        _harness_fail("pack size")
    corrupted = []
    for offset in (len(pack) // 2, len(pack) - 1):
        broken = bytearray(pack)
        broken[offset] ^= 0x01
        corrupted.append(bytes(broken).hex())
    workdir = Path(tempfile.mkdtemp(prefix="sley2-corrupt-"))
    try:
        ws = workdir / "ws"
        ws.mkdir(mode=0o700)
        (ws / REPO_DIR).mkdir(mode=0o700)
        try:
            shutil.copyfile(pack_path, ws / "base.pack")
        except OSError as error:
            raise JudgeHarnessError(
                f"LIVE_SLEY2_JUDGE_INVALID: stage pack: {error}") from error
        session = Session(_resolve_binary(), ws, [], seed_pack=True)
        try:
            head_before = session.head.get("tx", "")
            count_before = _live_object_count(session)
            if not head_before or count_before <= 0:
                _harness_fail("pristine head")
            for broken_hex in corrupted:
                reply = session._raw_request("exchange.import", broken_hex)
                if not reply["flags"].get("failed"):
                    _reject("ORACLE_CORRUPT_ACCEPTED", "bit-flip imported")
                try:
                    [failure] = sley2_codecs.run_batch(
                        [{"op": "decode_failure", "body": reply["body"]}])
                except sley2_codecs.CodecError as error:
                    raise JudgeHarnessError(
                        f"LIVE_SLEY2_JUDGE_INVALID: corrupt code: {error}") from error
                symbol = failure["decoded"].get("symbol", "")
                if symbol != "EXCHANGE_DIGEST_MISMATCH":
                    _reject("ORACLE_CORRUPT_UNREFUSED", symbol[:64])
            fresh = Session(_resolve_binary(), ws, [], seed_pack=False)
            try:
                if fresh.head.get("tx", "") != head_before:
                    _reject("ORACLE_CORRUPT_REF_MOVED", "destination head moved")
                if _live_object_count(fresh) != count_before:
                    _reject("ORACLE_CORRUPT_REF_MOVED", "destination store changed")
            finally:
                fresh.close()
        finally:
            session.close()
    finally:
        _remove_workdir(workdir)


# Resealed embedded-pack vector (S2B-CORRUPT-001 corpus code). The exchange
# trailer is the unkeyed digest blake3(domain || envelope-without-trailer)
# (crates/sley-repo/src/exchange.rs:674; crates/sley-id/src/lib.rs:167-171,501),
# so the judge recomputes it through the pinned oracle project's blake3 and
# proves the recomputation equals the owner's on the untouched staged
# exchange before using it. The Rust pin reseals with the owner's own
# `RepositoryExchangeId::derive` (crates/sley-repo/tests/s3_g2_corrupt.rs,
# `s3_corrupt_exchange_resealed_embedded_pack`) and flips the same offsets.
EXCHANGE_DIGEST_DOMAIN = b"sley2.repository-exchange.v1"
EXCHANGE_CONTRACT_TAG = 540
PACK_CONTRACT_TAG = 170
_SCB_MAGIC = b"SLEYSCB1"
_TRAILER_LEN = 32
CORRUPT_PACK_SYMBOL = "PACK_DIGEST_MISMATCH"
_RESEAL_SERVICE = (
    "import json, sys, blake3\n"
    "domain = bytes.fromhex(sys.argv[1])\n"
    "items = json.loads(sys.stdin.read())\n"
    "print(json.dumps([blake3.blake3(domain + bytes.fromhex(i)).hexdigest() for i in items]))\n"
)


def _scb_uvar(data: bytes, position: int) -> tuple[int, int]:
    try:
        value, width = _uvar(data, position)
    except (ValueError, IndexError) as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: exchange uvar: {error}") from error
    return value, position + width


def _scb_envelope(data: bytes, tag: int) -> tuple[int, int]:
    """(payload_start, payload_len) of one SCB1 envelope spanning `data`
    exactly: magic, version 1, contract tag, epoch, sized payload,
    32-byte trailer."""

    if not data.startswith(_SCB_MAGIC):
        _harness_fail("exchange magic")
    version, position = _scb_uvar(data, len(_SCB_MAGIC))
    contract, position = _scb_uvar(data, position)
    if version != 1 or contract != tag:
        _harness_fail(f"envelope tag {contract}")
    length, position = _scb_uvar(data, position + 32)
    if position + length + _TRAILER_LEN != len(data):
        _harness_fail("envelope span")
    return position, length


def _scb_fields(data: bytes, start: int, length: int) -> dict[int, tuple[int, int]]:
    """Record field tag -> absolute (start, length) inside data[start:start+length]."""

    count, position = _scb_uvar(data, start)
    fields: dict[int, tuple[int, int]] = {}
    for _ in range(count):
        tag, position = _scb_uvar(data, position)
        size, position = _scb_uvar(data, position)
        fields[tag] = (position, size)
        position += size
    if position != start + length:
        _harness_fail("record span")
    return fields


def _scb_elements(data: bytes, start: int, length: int) -> list[tuple[int, int]]:
    count, position = _scb_uvar(data, start)
    elements = []
    for _ in range(count):
        size, position = _scb_uvar(data, position)
        elements.append((position, size))
        position += size
    if position != start + length:
        _harness_fail("list span")
    return elements


def _embedded_objects(exchange: bytes) -> list[tuple[int, int]]:
    """Absolute (start, length) of every canonical object's stored bytes
    inside the embedded tag-170 pack: exchange payload field 2
    (exchange.rs:788) -> pack payload field 5 -> entry field 3, whose
    declared length is entry field 2 (crates/sley-repo/src/lib.rs:800-811)."""

    payload_start, payload_len = _scb_envelope(exchange, EXCHANGE_CONTRACT_TAG)
    fields = _scb_fields(exchange, payload_start, payload_len)
    if 2 not in fields:
        _harness_fail("exchange pack field")
    pack_start, pack_len = fields[2]
    pack = exchange[pack_start:pack_start + pack_len]
    inner_start, inner_len = _scb_envelope(pack, PACK_CONTRACT_TAG)
    pack_fields = _scb_fields(pack, inner_start, inner_len)
    if 5 not in pack_fields:
        _harness_fail("pack objects field")
    objects = []
    for entry_start, entry_len in _scb_elements(pack, *pack_fields[5]):
        entry = _scb_fields(pack, entry_start, entry_len)
        if 2 not in entry or 3 not in entry:
            _harness_fail("pack object entry")
        declared, _ = _scb_uvar(pack, entry[2][0])
        stored_start, stored_len = entry[3]
        if declared != stored_len:
            _harness_fail("pack object length")
        objects.append((pack_start + stored_start, stored_len))
    if len(objects) < 2:
        _harness_fail("pack object count")
    return objects


def _exchange_digests(prefixes: list[bytes]) -> list[bytes]:
    """blake3(EXCHANGE_DIGEST_DOMAIN || prefix) per prefix, through the
    pinned oracle project (system python has no blake3)."""

    try:
        completed = subprocess.run(
            [sley2_codecs._uv(), "run", "--offline", "--frozen", "--project",
             str(sley2_codecs.ORACLE_PROJECT), "python", "-c", _RESEAL_SERVICE,
             EXCHANGE_DIGEST_DOMAIN.hex()],
            input=json.dumps([prefix.hex() for prefix in prefixes]).encode("utf-8"),
            capture_output=True, timeout=120, check=False)
    except (OSError, subprocess.TimeoutExpired, sley2_codecs.CodecError) as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: reseal: {error}") from error
    if completed.returncode != 0:
        _harness_fail(f"reseal exit {completed.returncode}")
    try:
        digests = [bytes.fromhex(item) for item in json.loads(completed.stdout.decode("utf-8"))]
    except (UnicodeError, ValueError, TypeError) as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: reseal output: {error}") from error
    if len(digests) != len(prefixes) or any(len(d) != _TRAILER_LEN for d in digests):
        _harness_fail("reseal shape")
    return digests


def _resealed_pack_vectors(exchange: bytes) -> dict:
    """The resealed corpus vectors over the staged exchange: two
    independent single-byte flips inside canonical object bytes (first
    object at 3/8, last object at 5/8 of its stored bytes), each with
    the exchange trailer recomputed; plus the resealed-unflipped control,
    which must equal the staged exchange byte for byte (proof that the
    recomputation is the owner's digest on this input)."""

    objects = _embedded_objects(exchange)
    flips = []
    for label, (start, length), eighths in (("object-first", objects[0], 3),
                                             ("object-last", objects[-1], 5)):
        offset = start + length * eighths // 8
        if not start < offset < start + length - 1:
            _harness_fail("flip offset")
        body = bytearray(exchange[:-_TRAILER_LEN])
        body[offset] ^= 0x01
        flips.append((label, offset, bytes(body)))
    digests = _exchange_digests([exchange[:-_TRAILER_LEN]] + [body for _, _, body in flips])
    control = exchange[:-_TRAILER_LEN] + digests[0]
    if control != exchange:
        _harness_fail("reseal diverges from the owner trailer")
    vectors = [{"label": label, "offset": offset, "hex": (body + digest).hex()}
               for (label, offset, body), digest in zip(flips, digests[1:])]
    return {"control_hex": control.hex(), "vectors": vectors}


def _failure_symbol(reply: dict) -> str:
    try:
        [failure] = sley2_codecs.run_batch([{"op": "decode_failure", "body": reply["body"]}])
    except sley2_codecs.CodecError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: corrupt code: {error}") from error
    return str(failure["decoded"].get("symbol", ""))


def _pre_head_import(ws: Path, body_hex: str) -> dict:
    """One sessionless `exchange.import` on a repository with no accepted
    head (the only state in which the method travels without a session,
    crates/sley-protocol/src/server.rs:1132-1149): the same dispatch the
    trial tool's privileged seeding uses."""

    from bench.sley2.runner import request_frame

    sley = _resolve_binary()
    endpoint = Endpoint(sley, ws / REPO_DIR, ws / REPORT_NAME, SESSION_TIMEOUT, PROFILE_ARGS)
    try:
        hello, _, _ = endpoint_offer(sley)
        greeting = endpoint.send(hello)
        if not greeting or greeting[-1].get("kind") != "hello":
            _harness_fail("corrupt negotiation")
        replies = endpoint.send(request_frame("exchange.import", body_hex, None, 0,
                                              protocol_version=PROTOCOL_VERSION))
    finally:
        endpoint.close()
    if not replies:
        _harness_fail("corrupt import reply")
    return replies[-1]


def _tree_bytes(path: Path) -> dict[str, str]:
    out: dict[str, str] = {}
    for item in sorted(path.rglob("*")):
        if item.is_file() and not item.is_symlink():
            out[str(item.relative_to(path))] = hashlib.sha256(item.read_bytes()).hexdigest()
    return out


def _require_pack_refusal(reply: dict, label: str) -> str:
    if not reply["flags"].get("failed"):
        _reject("ORACLE_CORRUPT_ACCEPTED", f"{label}: corrupted exchange imported")
    symbol = _failure_symbol(reply)
    if symbol != CORRUPT_PACK_SYMBOL:
        _reject("ORACLE_CORRUPT_UNREFUSED", f"{label}: {symbol}"[:120])
    return symbol


def _judge_corrupt_pack_resealed(task_dir: Path, vectors: dict | None = None) -> dict:
    """Corpus vector (judge-side): one canonical object byte altered in
    the staged exchange's embedded pack, exchange trailer resealed,
    `exchange.import` must refuse with exact PACK_DIGEST_MISMATCH (the
    PACK owner's code, never remapped), and the destination must not
    move.

    One destination, two states. Fresh (no head): each resealed flip,
    plus a retry of the first, is refused and the repository files are
    byte-identical; the resealed-unflipped control then imports into
    that same destination with no cleanup (clean re-import accepted,
    deterministic recovery). Populated (the recovered head): each flip
    is refused twice with the same failure body, and the head
    transaction and live object count are unchanged on a fresh session
    and the repository files are byte-identical across every refusal.
    The judge drives these bytes through the privileged route; no agent
    path can attempt an import (exchange.import is denied to every arm,
    bench/sley2/runner.py:126). Whether that satisfies the corpus
    "attempt import" is an open owner question
    (bench/live/CORRUPT-SURFACE-DECISION.md). Returns the observed
    evidence for the witness log."""

    pack_path = task_dir / "base.pack"
    try:
        exchange = pack_path.read_bytes()
    except OSError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: pack: {error}") from error
    built = vectors if vectors is not None else _resealed_pack_vectors(exchange)
    flips = built["vectors"]
    if not flips:
        _harness_fail("corrupt vectors")
    evidence: dict = {"route": "exchange.import", "resealed": True, "fresh": [], "populated": []}
    workdir = Path(tempfile.mkdtemp(prefix="sley2-corrupt-pack-"))
    try:
        ws = workdir / "ws"
        ws.mkdir(mode=0o700)
        repo = ws / REPO_DIR
        repo.mkdir(mode=0o700)
        for vector in flips + flips[:1]:
            before = _tree_bytes(repo)
            symbol = _require_pack_refusal(_pre_head_import(ws, vector["hex"]), vector["label"])
            if _tree_bytes(repo) != before:
                _reject("ORACLE_CORRUPT_REF_MOVED", f"{vector['label']}: fresh destination written")
            evidence["fresh"].append({"label": vector["label"], "offset": vector.get("offset"),
                                      "symbol": symbol})
        recovered = _pre_head_import(ws, built["control_hex"])
        if recovered["flags"].get("failed"):
            _reject("ORACLE_CORRUPT_UNRECOVERED", _failure_symbol(recovered)[:120])
        evidence["control"] = "accepted"
        session = Session(_resolve_binary(), ws, [], seed_pack=False)
        try:
            head_before = session.head.get("tx", "")
            count_before = _live_object_count(session)
            if not head_before or count_before <= 0:
                _harness_fail("recovered head")
            for vector in flips:
                bodies = []
                for _ in range(2):
                    before = _tree_bytes(repo)
                    reply = session._raw_request("exchange.import", vector["hex"])
                    symbol = _require_pack_refusal(reply, vector["label"])
                    if _tree_bytes(repo) != before:
                        _reject("ORACLE_CORRUPT_REF_MOVED",
                                f"{vector['label']}: populated destination written")
                    bodies.append(reply["body"])
                if bodies[0] != bodies[1]:
                    _reject("ORACLE_CORRUPT_UNREFUSED", f"{vector['label']}: retry diverged")
                evidence["populated"].append({"label": vector["label"], "symbol": symbol,
                                              "attempts": 2})
        finally:
            session.close()
        fresh = Session(_resolve_binary(), ws, [], seed_pack=False)
        try:
            if fresh.head.get("tx", "") != head_before:
                _reject("ORACLE_CORRUPT_REF_MOVED", "destination head moved")
            if _live_object_count(fresh) != count_before:
                _reject("ORACLE_CORRUPT_REF_MOVED", "destination store changed")
        finally:
            fresh.close()
        evidence["head_tx"] = head_before
        evidence["live_objects"] = count_before
    finally:
        # Read-only store files defeat an ignore_errors removal; the scratch
        # is removed on every path or the judgment fails loudly.
        _remove_workdir(workdir)
    return evidence


def _judge_corrupt_value(session: Session, manifest: dict) -> None:
    """The corrupted constant reads the manifest-expected value post-fix
    (smoke only: restoring a constant's value is a different operation
    from rejecting a corrupted exchange pack)."""

    corrupt = manifest.get("judge", {}).get("corrupt", {})
    entities = manifest.get("entities", {})
    if not isinstance(corrupt, dict) or "entity" not in corrupt or "expected" not in corrupt:
        _harness_fail("corrupt spec")
    target = entities.get(corrupt["entity"], corrupt["entity"])
    reply = session._raw_request("entity.version", _tool_entity_body(session, target))
    if reply["flags"].get("failed"):
        _reject("ORACLE_CORRUPT_UNREADABLE", target[:32])
    [decoded] = sley2_codecs.run_batch([{
        "op": "decode_response", "method": "entity.version", "body": reply["body"],
    }])
    entries = decoded["decoded"].get("entries") or []
    if len(entries) != 1:
        _reject("ORACLE_CORRUPT_UNREADABLE", target[:32])
    body = entries[0].get("body") or {}
    try:
        value = body["value"]["data"]["value"]
    except (KeyError, TypeError):
        _reject("ORACLE_CORRUPT_UNRESTORED", json.dumps(body)[:120])
    if value != corrupt["expected"]:
        _reject("ORACLE_CORRUPT_UNRESTORED", json.dumps(value)[:120])


def _judge_test_entity(session: Session, manifest: dict, corpus: dict, scratch_ws: Path,
                       task_dir: Path, current: set[str]) -> None:
    """TEST task: judge the actual submitted TestCase entities.

    The implementation stays byte-identical (frozen S3
    implementation_changes == 0); only the test set may change. Each
    submitted TestCase bound to the target is decoded structurally; the
    set must cover success (7/2 -> Ok 3), divide-by-zero (7/0 -> Err
    code 2), and signed overflow (i64::MIN/-1 -> Err code 1) with exact
    expected outcomes. Coverage is verified from the submitted inputs
    and expected outcomes, and each submitted boundary is executed
    through the native driver against the unmodified implementation.
    Count alone never suffices: three or more tests with a missing,
    duplicated, or wrong-expected boundary are rejected.
    """

    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    func = entities.get("func", "")
    if not func:
        _harness_fail("test target")
    try:
        tests = _test_entities(session, scratch_ws, func, current)
    except JudgeHarnessError as error:
        raise JudgeRejection("ORACLE_CASE_MISSING", str(error)[:120]) from error
    _judge_impl_unchanged(session, scratch_ws, task_dir, func)
    if len(tests) < 3:
        _reject("ORACLE_CASE_MISSING", f"found {len(tests)} test entities")
    required = _require_test_boundaries(tests)
    # Execute each submitted boundary through the native driver and
    # require the actual outcome to match the submitted expectation.
    for (first, second), (kind, detail) in required.items():
        driver_case = {"inputs": [{"type": "SInt", "bits": 64, "value": first},
                                  {"type": "SInt", "bits": 64, "value": second}]}
        results = _run_driver(scratch_ws / REPO_DIR, func, [driver_case]).get("cases")
        if not isinstance(results, list) or len(results) != 1:
            _harness_fail("case shape")
        result = results[0]
        if not result.get("ok"):
            _reject("ORACLE_TEST_MISMATCH", json.dumps(result)[:160])
        want = _driver_want(kind, detail)
        if result.get("value") != want:
            _reject("ORACLE_TEST_MISMATCH",
                    f"boundary {(first, second)}: got "
                    f"{json.dumps(result.get('value'))[:120]}")


def _create_functions(session: Session, scratch_ws: Path,
                      current: set[str]) -> list[tuple[str, dict]]:
    """Live (current) kind-5 Function entities with decoded bodies,
    owner-resolved (stale duplicates excluded by the current filter)."""

    found = []
    for decoded in _decode_paths(session, _current_paths(scratch_ws, current)):
        try:
            if decoded.get("kind") != 5:
                continue
            body = decoded.get("body")
            if not isinstance(body, dict):
                continue
            entity = decoded.get("entity_id", "")
            if entity:
                found.append((str(entity), body))
        except (AttributeError, TypeError):
            continue
    return found






def _check_overflow_result(result: dict, cfg: dict, ok_code: str) -> None:
    """Overflow-case outcome rule (pure): a value where checked
    arithmetic must signal overflow is unchecked (wrapping) semantics
    under the frozen code, however embodied; a well-formed Err must
    carry the frozen failure code, else the shape mismatches.

    Reachability note: with the checked-only opcode set, no
    trial-surface-authorable single primitive returns Ok on the frozen
    overflow inputs while matching its value checks (only the correct
    checked op matches, and it signals). The rule is specified judge
    behavior with frozen-code parity regardless."""

    if not isinstance(result, dict):
        _harness_fail("create overflow shape")
    want = _driver_want("Err", int(cfg.get("code", 1)))
    value = result.get("value") if result.get("ok") else None
    if value == want:
        return
    # Native execution succeeded but produced a VALUE where checked
    # arithmetic must signal overflow: unchecked (wrapping) semantics
    # under the frozen code, however embodied.
    if isinstance(value, dict) and (
            value.get("Result", {}).get("Ok") is not None
            or value.get("SInt") is not None):
        _reject(ok_code, json.dumps(value)[:120])
    _reject("ORACLE_CREATE_MISMATCH",
            f"overflow shape {json.dumps(result)[:120]}")




def _judge_create(session: Session, manifest: dict, corpus: dict, scratch_ws: Path,
                  current: set[str]) -> None:
    """CREATE task: a genuine typed invoice program authored through the
    trial surface. Type definitions, checked helpers, a chained entry
    returning Result<Money,ArithmeticError>, and submitted deterministic
    tests are all discovered structurally and proven by native execution.

    The submitted entry itself executes natively on the frozen invoice
    inputs (empty, one-line, overflow): the program's own end-to-end
    decoded result decides the corpus outcome. No precomputed subtotal,
    tax product, rounded tax, or final total crosses as an entry input:
    the entry takes only the line vector and the basis-point rate, and
    Python only threads frozen inputs through and compares decoded
    execution results with frozen expectations.

    Money/LineItem recognition is by shape (one/two SInt64 record
    fields) against the submitted accepted state, never by fixed
    identities or role names; member meaning follows typedef field
    order (quantity, then unit_cents). Correct implementations need not
    use any particular helper count, role names, or operand shape: any
    entry with the required contract that computes the frozen cases
    accepts.

    Fail-closed classification (no silent pass):
    - malformed frozen spec -> harness failure (judge-side config,
      never candidate blame);
    - no Money/LineItem typedefs, or no entry with the required
      contract -> ORACLE_CREATE_UNMAPPED rejection (the program as
      submitted does not expose the required typed wiring);
    - mapped entry that the engine cannot execute ->
      ORACLE_CREATE_UNEXECUTABLE (explicit non-accepting readiness
      status: production limitation, not candidate wrongness, never
      reported as success);
    - executed with a wrong decoded value -> ORACLE_CREATE_MISMATCH;
    - overflow inputs yielding a value instead of the required
      overflow signal -> ORACLE_UNCHECKED_ARITHMETIC;
    - executed with the correctly decoded expected value on every
      frozen case, plus covering submitted tests executed natively ->
      accept.
    """

    _ = corpus
    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    if judge.get("flow") != "create":
        _harness_fail("create flow")
    spec_cases = _invoice_spec_cases(judge)
    repo = scratch_ws / REPO_DIR
    typedefs = _create_typedefs(session, scratch_ws, current)
    money_ids = [entity for entity, body in typedefs.items()
                 if _sint_record_shape(body, 1)]
    line_ids = [entity for entity, body in typedefs.items()
                if _sint_record_shape(body, 2)]
    if not money_ids or not line_ids:
        _reject("ORACLE_CREATE_UNMAPPED", "no Money/LineItem typedefs")
    functions = _create_functions(session, scratch_ws, current)
    if not functions:
        _reject("ORACLE_CREATE_UNMAPPED", "no live functions")
    params = _create_param_types(session, scratch_ws, current)
    mismatch_detail = ""
    unexecutable_detail = ""
    accepted: tuple[str, str, str] | None = None
    for money_td in sorted(money_ids):
        cents = _record_members(typedefs[money_td])[0]
        for line_td in sorted(line_ids):
            members = _record_members(typedefs[line_td])
            quantity, unit = members[0], members[1]
            for entity, body in functions:
                if not _entry_shape(body, params, line_td, money_td):
                    continue
                try:
                    _check_invoice_entry(repo, entity, money_td, cents,
                                         quantity, unit, spec_cases)
                except JudgeHarnessError:
                    raise
                except JudgeRejection as rejected:
                    if rejected.code == "ORACLE_CREATE_MISMATCH":
                        mismatch_detail = (
                            mismatch_detail or rejected.detail[:120])
                        continue
                    if rejected.code == "ORACLE_CREATE_UNEXECUTABLE":
                        unexecutable_detail = (
                            unexecutable_detail or rejected.detail[:120])
                        continue
                    raise
                accepted = (entity, money_td, line_td)
                break
            if accepted is not None:
                break
        if accepted is not None:
            break
    if accepted is None:
        if mismatch_detail:
            _reject("ORACLE_CREATE_MISMATCH", mismatch_detail)
        if unexecutable_detail:
            _reject("ORACLE_CREATE_UNEXECUTABLE", unexecutable_detail)
        _reject("ORACLE_CREATE_UNMAPPED", "no entry matches contract")
    entry, money_td, line_td = accepted
    cents = _record_members(typedefs[money_td])[0]
    members = _record_members(typedefs[line_td])
    _judge_invoice_tests(session, scratch_ws, repo, entry, money_td,
                         cents, line_td, members[0], members[1],
                         current)


def _invoice_spec_cases(judge: dict) -> list:
    """Frozen invoice cases from the manifest judge spec (judge-side
    config): each names lines, the basis-point rate, and the exact
    expected outcome. Malformed specs are harness failures."""

    raw = judge.get("cases", None)
    if not isinstance(raw, list) or not raw:
        _harness_fail("create cases")
    cases = []
    for item in raw:
        if not isinstance(item, dict):
            _harness_fail("create cases")
        name = item.get("name", "")
        lines = item.get("lines", None)
        rate = item.get("tax_bp", None)
        expect = item.get("expect", None)
        if (not isinstance(name, str) or not name
                or not isinstance(lines, list)
                or isinstance(rate, bool) or not isinstance(rate, int)
                or not isinstance(expect, dict)):
            _harness_fail("create cases")
        parsed_lines = []
        for line in lines:
            if (not isinstance(line, dict)
                    or isinstance(line.get("quantity"), bool)
                    or not isinstance(line.get("quantity"), int)
                    or isinstance(line.get("unit_cents"), bool)
                    or not isinstance(line.get("unit_cents"), int)):
                _harness_fail("create cases")
            parsed_lines.append((line["quantity"], line["unit_cents"]))
        if set(expect) == {"Ok"}:
            cents = expect["Ok"].get("cents", None) if isinstance(
                expect["Ok"], dict) else None
            if isinstance(cents, bool) or not isinstance(cents, int):
                _harness_fail("create cases")
            want: tuple[str, object] = ("Ok", cents)
        elif set(expect) == {"Err"}:
            failure = expect["Err"]
            if (not isinstance(failure, dict)
                    or failure.get("kind") != "Arithmetic"
                    or isinstance(failure.get("code"), bool)
                    or not isinstance(failure.get("code"), int)):
                _harness_fail("create cases")
            want = ("Err", failure["code"])
        else:
            _harness_fail("create cases")
        cases.append({"name": name, "lines": parsed_lines,
                      "tax_bp": rate, "want": want})
    names = [case["name"] for case in cases]
    if sorted(names) != ["empty", "one-line", "overflow"]:
        _harness_fail("create cases")
    return cases


def _sint_record_shape(body: dict, fields: int) -> bool:
    """Record typedef with exactly `fields` SInt64 members (Money = 1,
    LineItem = 2), resolved from the decoded submitted body."""

    try:
        form = body.get("form") or {}
        if form.get("variant") != "Record":
            return False
        members = form.get("value") or []
        if not isinstance(members, list) or len(members) != fields:
            return False
        return all(isinstance(field, dict)
                   and field.get("value_type") == {"variant": "SInt",
                                                   "value": 64}
                   for field in members)
    except (AttributeError, TypeError):
        return False


def _record_members(body: dict) -> list:
    """Typedef record member identities in definition order."""

    try:
        members = (body.get("form") or {}).get("value") or []
        return [str(field["member_id"]) for field in members]
    except (AttributeError, TypeError, KeyError):
        _harness_fail("create typedef members")
        raise AssertionError("unreachable")


def _named_definition(typed: object, definition: str) -> bool:
    return (isinstance(typed, dict)
            and typed.get("variant") == "Named"
            and isinstance(typed.get("value"), dict)
            and typed["value"].get("definition") == definition
            and typed["value"].get("arguments") == [])


def _entry_shape(body: dict, params: dict[str, dict], line_td: str,
                 money_td: str) -> bool:
    """Entry contract: (Vector(LineItem), SInt64) ->
    Result<Money,ArithmeticError>. No names, no helper count, no
    operand-shape requirements beyond the callable contract."""

    try:
        parameters = body.get("parameters") or []
        if not isinstance(parameters, list) or len(parameters) != 2:
            return False
        first = params.get(str(parameters[0])) or {}
        second = params.get(str(parameters[1])) or {}
        if (not isinstance(first, dict) or not isinstance(second, dict)):
            return False
        if (first.get("variant") != "Vector"
                or not _named_definition(first.get("value"), line_td)):
            return False
        if second != {"variant": "SInt", "value": 64}:
            return False
        result = body.get("result_type") or {}
        if not isinstance(result, dict):
            return False
        if result.get("variant") != "Result":
            return False
        value = result.get("value") or {}
        return (_named_definition(value.get("ok"), money_td)
                and value.get("error") == {"variant": "BuiltinFailure",
                                           "value": "ArithmeticError"})
    except (AttributeError, TypeError):
        return False


def _create_typedefs(session: Session, scratch_ws: Path,
                     current: set[str]) -> dict[str, dict]:
    """Live (current) kind-4 TypeDef entities with decoded bodies."""

    found: dict[str, dict] = {}
    for decoded in _decode_paths(session, _current_paths(scratch_ws, current)):
        try:
            if decoded.get("kind") != 4:
                continue
            body = decoded.get("body")
            if not isinstance(body, dict):
                continue
            entity = decoded.get("entity_id", "")
            if entity:
                found[str(entity)] = body
        except (AttributeError, TypeError):
            continue
    return found


def _create_param_types(session: Session, scratch_ws: Path,
                        current: set[str]) -> dict[str, dict]:
    """Live (current) kind-6 Parameter value types by entity."""

    found: dict[str, dict] = {}
    for decoded in _decode_paths(session, _current_paths(scratch_ws, current)):
        try:
            if decoded.get("kind") != 6:
                continue
            body = decoded.get("body")
            if not isinstance(body, dict):
                continue
            entity = decoded.get("entity_id", "")
            typed = body.get("value_type")
            if entity and isinstance(typed, dict):
                found[str(entity)] = typed
        except (AttributeError, TypeError):
            continue
    return found


def _invoice_driver_case(lines: list, rate: int, quantity: str,
                         unit: str) -> dict:
    """One typed driver case: the line vector plus the rate, decoded by
    the driver against the entry's declared parameter types. Only
    frozen inputs cross; no computed intermediate does."""

    return {"inputs": [
        {"values": [{"fields": {quantity: {"value": quantity_value},
                                unit: {"value": unit_value}}}
                    for quantity_value, unit_value in lines]},
        {"value": rate}]}


def _invoice_want_ok(money_td: str, cents: str, value: int) -> dict:
    return {"Result": {"Ok": {"Record": {
        "definition": money_td,
        "fields": {cents: {"SInt": str(value)}}}}}}


def _invoice_want_err(code: int) -> dict:
    return {"Result": {"Err": {"BuiltinFailure": {"kind": "Arithmetic",
                                                 "code": code}}}}


def _check_invoice_entry(repo: Path, entry: str, money_td: str,
                         cents: str, quantity: str, unit: str,
                         cases: list) -> None:
    """Execute one entry candidate on every frozen invoice case.

    Wrong decoded values reject as ORACLE_CREATE_MISMATCH (candidate
    wrongness); mapped-but-unexecuted cases reject as
    ORACLE_CREATE_UNEXECUTABLE (readiness, never blame); overflow
    inputs yielding a value reject as ORACLE_UNCHECKED_ARITHMETIC via
    the frozen overflow rule."""
    for case in cases:
        name = case["name"]
        driver_case = _invoice_driver_case(case["lines"], case["tax_bp"],
                                          quantity, unit)
        try:
            results = _run_driver(repo, entry, [driver_case]).get("cases")
        except JudgeHarnessError:
            raise
        except (JudgeRejection, Exception) as error:
            raise JudgeRejection(
                "ORACLE_CREATE_UNEXECUTABLE",
                f"entry driver failed: {type(error).__name__}"[:120]) from error
        if not isinstance(results, list) or len(results) != 1:
            _harness_fail("create case shape")
        result = results[0]
        if not isinstance(result, dict):
            _harness_fail("create case shape")
        kind, detail = case["want"]
        if kind == "Err":
            _check_overflow_result(
                result, {"code": detail}, "ORACLE_UNCHECKED_ARITHMETIC")
            continue
        if not result.get("ok"):
            raise JudgeRejection(
                "ORACLE_CREATE_UNEXECUTABLE",
                f"{name}: entry did not execute natively"[:120])
        value = result.get("value")
        if not isinstance(value, dict):
            _harness_fail("create entry envelope")
        want = _invoice_want_ok(money_td, cents, detail)
        if value != want:
            raise JudgeRejection(
                "ORACLE_CREATE_MISMATCH",
                f"{name}: end-to-end {json.dumps(value)[:80]} != "
                f"Ok({detail})"[:120])


def _judge_invoice_tests(session: Session, scratch_ws: Path, repo: Path,
                         entry: str, money_td: str, cents: str,
                         line_td: str, quantity: str, unit: str,
                         current: set[str]) -> None:
    """Submitted deterministic tests for the accepted entry: the three
    frozen boundaries (empty, one-line, overflow) with exact submitted
    expectations, each executed natively through the driver against the
    unmodified implementation. Count alone never suffices."""

    try:
        tests = _test_entities(session, scratch_ws, entry, current)
    except JudgeHarnessError as error:
        raise JudgeRejection("ORACLE_CASE_MISSING", str(error)[:120]) from error
    if len(tests) < 3:
        _reject("ORACLE_CASE_MISSING", f"found {len(tests)} test entities")
    required = _require_invoice_boundaries(tests, money_td, cents,
                                           line_td, quantity, unit)
    for key, (kind, detail) in required.items():
        lines, rate = key
        driver_case = _invoice_driver_case(list(lines), rate, quantity,
                                          unit)
        results = _run_driver(repo, entry, [driver_case]).get("cases")
        if not isinstance(results, list) or len(results) != 1:
            _harness_fail("case shape")
        result = results[0]
        if not result.get("ok"):
            _reject("ORACLE_TEST_MISMATCH", json.dumps(result)[:160])
        if kind == "Ok":
            want = _invoice_want_ok(money_td, cents, detail)
        else:
            want = _invoice_want_err(detail)
        if result.get("value") != want:
            _reject("ORACLE_TEST_MISMATCH",
                    f"boundary {(lines, rate)}: got "
                    f"{json.dumps(result.get('value'))[:120]}")


def _require_invoice_boundaries(tests: list, money_td: str, cents: str,
                                line_td: str, quantity: str,
                                unit: str) -> dict:
    """Require the three frozen CREATE boundaries from submitted test
    entities. Missing boundaries, duplicated inputs standing in for
    another boundary, or wrong expected outcomes all reject: count
    alone never suffices."""

    i64max = 9223372036854775807
    required = {
        ((), 725): ("Ok", 0),
        (((2, 1250),), 725): ("Ok", 2681),
        (((i64max, 2),), 725): ("Err", 1),
    }
    seen: dict = {}
    for test in tests:
        pair = _invoice_test_inputs(test, line_td, quantity, unit)
        norm = _invoice_test_expected(test, money_td, cents)
        if pair is None or norm is None:
            _reject("ORACLE_TEST_MISMATCH",
                    f"undecodable test inputs/expected {str(test)[:120]}")
        assert pair is not None and norm is not None
        if pair in seen:
            continue
        seen[pair] = norm
    for key, want_norm in required.items():
        got = seen.get(key)
        if got is None:
            _reject("ORACLE_CASE_MISSING", f"missing boundary {key}"[:120])
        if got != want_norm:
            _reject("ORACLE_TEST_MISMATCH",
                    f"boundary {key}: submitted {got} != want "
                    f"{want_norm}"[:120])
    return required


def _invoice_test_inputs(test: dict, line_td: str, quantity: str,
                         unit: str) -> tuple | None:
    """Submitted (lines, rate) from a decoded TestCaseBody, or None."""

    try:
        inputs = test.get("inputs")
        if not isinstance(inputs, list) or len(inputs) != 2:
            return None
        vector, rate_item = inputs
        if not isinstance(vector, dict) or not isinstance(rate_item, dict):
            return None
        data = vector.get("data")
        if not isinstance(data, dict) or data.get("variant") != "Sequence":
            return None
        members = data.get("value")
        if not isinstance(members, list):
            return None
        lines = []
        for item in members:
            if not isinstance(item, dict):
                return None
            if item.get("value_type") != {
                    "variant": "Named",
                    "value": {"definition": line_td, "arguments": []}}:
                return None
            record = item.get("data")
            if (not isinstance(record, dict)
                    or record.get("variant") != "Record"):
                return None
            const = record.get("value")
            if not isinstance(const, dict):
                return None
            if const.get("definition") != line_td:
                return None
            fields = const.get("fields")
            if not isinstance(fields, list):
                return None
            by_member = {field.get("member_id"): field.get("value")
                         for field in fields if isinstance(field, dict)}
            first = by_member.get(quantity)
            second = by_member.get(unit)
            if (not isinstance(first, dict) or not isinstance(second, dict)):
                return None
            first_data = first.get("data") or {}
            second_data = second.get("data") or {}
            if (first_data.get("variant") != "SInt"
                    or second_data.get("variant") != "SInt"):
                return None
            first_value = first_data.get("value")
            second_value = second_data.get("value")
            if (isinstance(first_value, bool)
                    or not isinstance(first_value, int)
                    or isinstance(second_value, bool)
                    or not isinstance(second_value, int)):
                return None
            lines.append((first_value, second_value))
        rate_data = (rate_item.get("data") or {}) if isinstance(
            rate_item, dict) else {}
        if rate_data.get("variant") != "SInt":
            return None
        rate = rate_data.get("value")
        if isinstance(rate, bool) or not isinstance(rate, int):
            return None
        return (tuple(lines), rate)
    except Exception:
        return None


def _invoice_test_expected(test: dict, money_td: str,
                           cents: str) -> tuple[str, int] | None:
    """Submitted invoice expectation as ("Ok", cents) or ("Err", code),
    or None."""

    try:
        expected = test.get("expected")
        if not isinstance(expected, dict):
            return None
        variant = expected.get("variant")
        value = expected.get("value")
        if variant == "FailureCode":
            if isinstance(value, int) and not isinstance(value, bool):
                return ("Err", value)
            return None
        if variant != "Value" or not isinstance(value, dict):
            return None
        data = value.get("data")
        if not isinstance(data, dict) or data.get("variant") != "Result":
            return None
        inner = data.get("value")
        if not isinstance(inner, dict):
            return None
        if inner.get("variant") == "Ok":
            result = inner.get("value")
            if not isinstance(result, dict):
                return None
            result_data = result.get("data")
            if (not isinstance(result_data, dict)
                    or result_data.get("variant") != "Record"):
                return None
            const = result_data.get("value")
            if not isinstance(const, dict):
                return None
            if const.get("definition") != money_td:
                return None
            fields = const.get("fields")
            if not isinstance(fields, list):
                return None
            for field in fields:
                if not isinstance(field, dict):
                    return None
                if field.get("member_id") != cents:
                    continue
                amount = field.get("value") or {}
                amount_data = amount.get("data") or {}
                if amount_data.get("variant") != "SInt":
                    return None
                amount_value = amount_data.get("value")
                if (isinstance(amount_value, bool)
                        or not isinstance(amount_value, int)):
                    return None
                return ("Ok", amount_value)
            return None
        if inner.get("variant") == "Err":
            result = inner.get("value")
            if not isinstance(result, dict):
                return None
            result_data = result.get("data")
            if (not isinstance(result_data, dict)
                    or result_data.get("variant") != "BuiltinFailure"):
                return None
            failure = result_data.get("value")
            if not isinstance(failure, dict):
                return None
            if failure.get("kind") != "ArithmeticError":
                return None
            code = failure.get("code")
            if isinstance(code, bool) or not isinstance(code, int):
                return None
            return ("Err", code)
        return None
    except Exception:
        return None


def _test_input_pair(test: dict) -> tuple[int, int] | None:
    """Submitted (a, b) inputs from a decoded TestCaseBody, or None."""

    try:
        inputs = test.get("inputs")
        if not isinstance(inputs, list) or len(inputs) != 2:
            return None
        values = []
        for item in inputs:
            if not isinstance(item, dict):
                return None
            data = item.get("data")
            if not isinstance(data, dict) or data.get("variant") != "SInt":
                return None
            value = data.get("value")
            if not isinstance(value, int):
                return None
            values.append(value)
        return (values[0], values[1])
    except Exception:
        return None


def _test_expected_norm(test: dict) -> tuple[str, int] | None:
    """Submitted expectation as ("Ok", value) or ("Err", code), or None."""

    try:
        expected = test.get("expected")
        if not isinstance(expected, dict):
            return None
        variant = expected.get("variant")
        value = expected.get("value")
        if variant == "FailureCode":
            if isinstance(value, int) and value in (1, 2):
                return ("Err", value)
            return None
        if variant != "Value" or not isinstance(value, dict):
            return None
        data = value.get("data")
        if not isinstance(data, dict) or data.get("variant") != "Result":
            return None
        inner = data.get("value")
        if not isinstance(inner, dict):
            return None
        if inner.get("variant") == "Ok":
            result = inner.get("value")
            if not isinstance(result, dict):
                return None
            result_data = result.get("data")
            if not isinstance(result_data, dict) or result_data.get("variant") != "SInt":
                return None
            result_value = result_data.get("value")
            if not isinstance(result_value, int):
                return None
            return ("Ok", result_value)
        if inner.get("variant") == "Err":
            result = inner.get("value")
            if not isinstance(result, dict):
                return None
            result_data = result.get("data")
            if not isinstance(result_data, dict) or result_data.get("variant") != "BuiltinFailure":
                return None
            failure = result_data.get("value")
            if not isinstance(failure, dict):
                return None
            if failure.get("kind") != "ArithmeticError":
                return None
            code = failure.get("code")
            if not isinstance(code, int):
                return None
            return ("Err", code)
        return None
    except Exception:
        return None


def _driver_want(kind: str, detail: int) -> dict:
    if kind == "Ok":
        return {"Result": {"Ok": {"SInt": str(detail)}}}
    return {"Result": {"Err": {"BuiltinFailure": {"kind": "Arithmetic", "code": detail}}}}


def _require_test_boundaries(tests: list) -> dict[tuple[int, int], tuple[str, int]]:
    """Require the three frozen TEST boundaries from submitted entities.

    Returns the required map on success; rejects otherwise. Three or
    more tests with a missing boundary, a duplicated input pair standing
    in for another boundary, or a wrong expected failure are all
    rejected: count alone never suffices.
    """

    required = {
        (7, 2): ("Ok", 3),
        (7, 0): ("Err", 2),
        (-9223372036854775808, -1): ("Err", 1),
    }
    seen: dict[tuple[int, int], tuple[str, int]] = {}
    for test in tests:
        pair = _test_input_pair(test) if isinstance(test, dict) else None
        norm = _test_expected_norm(test) if isinstance(test, dict) else None
        if pair is None or norm is None:
            _reject("ORACLE_TEST_MISMATCH",
                    f"undecodable test inputs/expected {str(test)[:120]}")
        assert pair is not None and norm is not None
        if pair in seen:
            continue
        seen[pair] = norm
    for pair, want_norm in required.items():
        got = seen.get(pair)
        if got is None:
            _reject("ORACLE_CASE_MISSING", f"missing boundary {pair}")
        if got != want_norm:
            _reject("ORACLE_TEST_MISMATCH",
                    f"boundary {pair}: submitted {got} != want {want_norm}")
    return required


def _judge_impl_unchanged(session: Session, scratch_ws: Path, task_dir: Path,
                           func: str) -> None:
    """The implementation entity's CURRENT object bytes are byte-identical
    between the pristine base pack and the committed outcome (frozen
    S3 implementation_changes == 0).

    Both sides resolve through live entity.version bindings under
    their accepted heads; retained historical object files never
    decide (a stale duplicate beside the current version cannot flip
    this check either way)."""

    repo_pre, pre_session, cleanup = _seeded_repo(task_dir)
    try:
        pre_raw = _entity_bound_bytes(pre_session, repo_pre, func)
    finally:
        cleanup()
    post_raw = _entity_bound_bytes(session, scratch_ws / REPO_DIR, func)
    if pre_raw is None or post_raw is None:
        _harness_fail("impl objects")
    if pre_raw != post_raw:
        _reject("ORACLE_IMPL_TOUCHED", func[:32])


def _live_object_id(session: Session, entity: str) -> str | None:
    """Owner-derived current ObjectId for one entity under the session's
    accepted head (entity.version binding). Tombstoned/absent entities
    fail and yield None; multi-entry or undecodable replies yield None.
    Historical object files never decide currency."""

    if not entity:
        return None
    try:
        reply = session._raw_request("entity.version", _tool_entity_body(session, entity))
    except Exception:
        return None
    if reply["flags"].get("failed"):
        return None
    try:
        [decoded] = sley2_codecs.run_batch([{
            "op": "decode_response", "method": "entity.version",
            "body": reply["body"],
        }])
    except (sley2_codecs.CodecError, KeyError):
        return None
    entries = decoded["decoded"].get("entries") or []
    if len(entries) != 1:
        return None
    object_id = entries[0].get("object_id", "")
    if not _is_hash64(object_id):
        return None
    return str(object_id)


def _bound_object_bytes(repo: Path, object_id: str) -> bytes | None:
    """Raw stored bytes for one owner-derived ObjectId (exact path)."""

    if not _is_hash64(object_id):
        return None
    path = _object_path(repo, object_id)
    if path is None:
        return None
    try:
        return path.read_bytes()
    except OSError:
        return None


def _entity_bound_bytes(session: Session, repo: Path, entity: str) -> bytes | None:
    """Raw stored bytes of one entity's CURRENT version: resolve the
    live ObjectId under the accepted head, then read that exact object.
    Stale versions beside it never win by filename order."""

    object_id = _live_object_id(session, entity)
    if object_id is None:
        return None
    return _bound_object_bytes(repo, object_id)


def _entity_stored_bytes(repo: Path, entity: str, epoch: str) -> bytes | None:
    """Raw stored bytes of one entity's object file (FILE SCAN ONLY).

    Currency WARNING: this scans immutable object files in filename
    order and returns the first file decoding to the entity. Repos
    retain stale versions beside current ones, so this MUST NOT decide
    current-state facts (presence, impl identity, counts, merge
    conflict values). Acceptance-critical paths must use
    _entity_bound_bytes (live entity.version binding -> exact object)
    instead. Retained only for inventory/enumeration helpers that
    explicitly handle currency elsewhere."""

    if not epoch:
        return None
    for path in _store_files(repo):
        try:
            raw = path.read_bytes()
        except OSError:
            continue
        try:
            [decoded] = sley2_codecs.run_batch([{
                "op": "decode_object", "stored": raw.hex(), "epoch": epoch,
            }])
        except (sley2_codecs.CodecError, KeyError):
            continue
        try:
            if decoded["decoded"].get("entity_id") == entity:
                return raw
        except (KeyError, TypeError):
            continue
    return None


def _test_entities(session: Session, scratch_ws: Path, func: str,
                   current: set[str]) -> list:
    """CURRENT TestCase entities targeting a function (kind 14 with
    matching target field), decoded structurally."""

    found = []
    for decoded in _decode_paths(session, _current_paths(scratch_ws, current)):
        try:
            entry = decoded
            if entry.get("kind") != 14:
                continue
            if not isinstance(entry.get("body_hex"), str):
                continue
            [typed] = sley2_codecs.run_batch([{
                "op": "decode_value", "type": "TestCaseBody",
                "data": entry["body_hex"],
            }])
            if typed["value"].get("target") == func:
                found.append(typed["value"])
        except (sley2_codecs.CodecError, KeyError):
            continue
    return found


def _judge_stale_sequence(session: Session, manifest: dict, corpus: dict, scratch_ws: Path,
                          candidate: bytes, pre_tx: str) -> None:
    """STALE task frozen sequence: competing candidates against the same
    base, first acceptance (already committed to H1 on entry), exact
    permitted stale rejection (STALE_ROOT, never a substring match)
    with no partial second write, re-query, and genuine rebase of the
    contender's own operations against the new base.

    Resubmitting the original candidate while changing only the outer
    request binding is NOT rebase evidence: the candidate's embedded
    base binding still names the old head, so outer-tx substitution
    alone proves nothing. An identity replacement would only prove H1
    is writable. The judge therefore decodes the contender record and
    re-assembles the SAME change against H1 with fresh bindings,
    requiring a Valid decision on task-relevant targets.
    """

    _ = corpus
    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    if "constant" not in entities or "guard" not in entities:
        _harness_fail("stale entities")
    principal = manifest.get("principal", "")
    head_h1 = session.head.get("tx", "")
    if not head_h1 or head_h1 == pre_tx:
        _reject("ORACLE_STALE_UNCHANGED", "head did not advance past H1")
    # Competing write against the old base must fail stale: same stored
    # bytes, old parent. No last-write-wins.
    competing = _encode_commit_body(pre_tx, principal, candidate.hex())
    stale = session._raw_request("commit", competing)
    if not stale["flags"].get("failed"):
        _reject("ORACLE_LAST_WRITE_WINS", "competing commit applied")
    try:
        [decoded] = sley2_codecs.run_batch([{"op": "decode_failure", "body": stale["body"]}])
    except sley2_codecs.CodecError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: stale code: {error}") from error
    code = decoded["decoded"].get("symbol", "")
    # Exact frozen mapping (S3 s3_g2_stale pins STALE_ROOT): a symbol
    # merely containing the substring STALE never counts.
    if code != "STALE_ROOT":
        _reject("ORACLE_STALE_UNREFUSED", code[:64])
    # No partial second write: a fresh session must still read H1.
    fresh = Session(_resolve_binary(), scratch_ws, [], seed_pack=False)
    try:
        if fresh.head.get("tx", "") != head_h1:
            _reject("ORACLE_STALE_PARTIAL_WRITE",
                    f"{fresh.head.get('tx', '')[:16]} != H1")
    finally:
        fresh.close()
    # Outer-binding-only resubmission is not rebase evidence. Record its
    # outcome for diagnosis, but never accept on it: even a successful
    # envelope must still decode to Valid, and even then a fresh
    # candidate is required below.
    probe = session._raw_request(
        "candidate.validate",
        _encode_validate_body(head_h1, principal, candidate.hex()))
    if not probe["flags"].get("failed"):
        try:
            [probe_decoded] = sley2_codecs.run_batch(
                [{"op": "decode_result", "body": probe["body"]}])
        except sley2_codecs.CodecError:
            pass
        else:
            # Deliberately ignored as acceptance evidence: resubmission
            # with only the outer tx changed does not rebase the
            # candidate's embedded base binding.
            _ = probe_decoded.get("decision_tag")
    # Genuine rebase: replay the contender's own operations against
    # H1 with fresh bindings and require Valid through the correct
    # validation shape.
    _require_rebased_candidate_valid(session, scratch_ws, manifest, head_h1,
                                      candidate)


def _require_rebased_candidate_valid(session: Session, scratch_ws: Path,
                                      manifest: dict, head_h1: str,
                                      candidate: bytes) -> None:
    """Genuine rebase of the contender onto the new base: decode the
    contender's own operations (classes, targets, payloads) from its
    record, re-assemble the SAME change against H1 with fresh bindings
    through the normal assembly path, and require a Valid decision.

    An identity replacement (re-stating H1's current bytes) would only
    prove the new base is writable; it never counts here. The rebased
    change must touch at least one manifest task entity, and every
    Replace target must re-resolve at H1 (re-query of the new base).
    Outer-binding-only resubmission never counts (checked by the
    caller); only this same-change-new-base validation does."""

    _ = scratch_ws
    from bench.live import sley2_tool as _tool

    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    principal = manifest.get("principal", "")
    if not entities or not principal:
        _harness_fail("stale entities")
    try:
        [unwrapped] = sley2_codecs.run_batch(
            [{"op": "record_from_stored", "stored": candidate.hex()}])
        [decoded] = sley2_codecs.run_batch(
            [{"op": "record_operations", "record": unwrapped["record"]}])
    except sley2_codecs.CodecError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: rebase decode: {error}") from error
    operations = decoded.get("operations") or []
    if not operations:
        _reject("ORACLE_REBASE_INVALID", "contender has no operations")
    task_ids = set(entities.values())
    ops = []
    touched = False
    for item in operations:
        if not isinstance(item, dict):
            _reject("ORACLE_REBASE_INVALID", "contender op shape")
        class_name = item.get("class", "")
        kind = item.get("kind")
        target = item.get("target", "")
        payload = item.get("payload")
        if class_name not in ("CreateEntity", "ReplaceEntityVersion"):
            _reject("ORACLE_REBASE_INVALID",
                    f"contender class {class_name}"[:64])
        if not isinstance(kind, int) or not _is_hash64(target) or payload is None:
            _reject("ORACLE_REBASE_INVALID", "contender op shape")
        if target in task_ids:
            touched = True
        if class_name == "ReplaceEntityVersion":
            # Re-query the new base: the rebased target must resolve
            # at H1 (deleted targets cannot rebase).
            if _live_object_id(session, str(target)) is None:
                _reject("ORACLE_REBASE_INVALID",
                        f"target unresolved at H1 {str(target)[:16]}")
            ops.append({"class": class_name, "kind": kind, "target": target,
                        "field_tag": item.get("field_tag"), "payload": payload})
        else:
            # Fresh creation under the new base nonce: identities
            # re-derive (never carried across bases).
            ops.append({"class": class_name, "kind": kind, "target": None,
                        "field_tag": item.get("field_tag"), "payload": payload})
    if not touched:
        _reject("ORACLE_REBASE_INVALID", "contender touches no task entity")
    try:
        record_hex = _tool._assemble(session, ops)
    except Exception as error:
        raise JudgeRejection("ORACLE_REBASE_INVALID", f"assemble: {error}"[:120]) from error
    try:
        [stored] = sley2_codecs.run_batch(
            [{"op": "stored_from_record", "record": record_hex}])
    except sley2_codecs.CodecError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: rebase stored: {error}") from error
    rebased = session._raw_request(
        "candidate.validate",
        _encode_validate_body(head_h1, principal, stored["stored"]))
    if rebased["flags"].get("failed"):
        _reject("ORACLE_REBASE_INVALID", (rebased.get("body") or "")[:120])
    _require_valid_decision(rebased.get("body") or "", code="ORACLE_REBASE_INVALID")


def _judge_merge(session: Session, manifest: dict, corpus: dict, scratch_ws: Path,
                 workspace: Path, candidate: bytes) -> None:
    """MERGE task: the committed outcome carries exactly the union
    semantics — the shared constant at the ours value, the theirs-only
    constant present, nothing else added or changed — AND the union
    re-applies cleanly from either side.

    Order-independence is OBSERVED, not asserted: the contender record
    is rebased onto the ours head and onto the theirs head (same
    classes/targets/payloads, fresh bindings; already-satisfied
    replaces skipped), and production validation must accept both.
    Either order rejecting is ORACLE_MERGE_UNSTABLE. The side packs
    share ancestor transaction history (base head tx bytes present in
    both packs — verified by identity 2026-09-21), but arrive as
    separate pack files rather than branch pointers in one repo, so
    the frozen production merge-judge path (merge.judge over
    co-resident revisions) is not directly drivable here; the
    applicable production path exercised is candidate validation
    itself, in both orders, against both heads. A branch-pointer
    production-path proof is tracked separately
    (bench/live/prove_merge_production.py).

    Executable-function tests: the MERGE fixture carries no functions
    (namespace + bool constants only), so there are no selected tests
    to execute; preservation of both changes plus dual-order validation
    plus the generic collateral checks are the applicable evidence."""

    _ = corpus
    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    conflict = entities.get(judge.get("conflict", "constant"), "")
    if not conflict:
        _harness_fail("merge conflict entity")
    repos: dict[str, Path] = {}
    side_ws: dict[str, Path] = {}
    epochs: dict[str, str] = {}
    sides: dict[str, Session] = {}
    cleanups = []
    try:
        for name in ("base.pack", "ours.pack", "theirs.pack"):
            path = workspace / name
            try:
                if not path.is_file() or path.is_symlink():
                    _harness_fail(f"side {name}")
            except OSError as error:
                raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: sides: {error}") from error
            repo, ws, side_session, cleanup = _seeded_pack(path)
            repos[name] = repo
            side_ws[name] = ws
            epochs[name] = side_session.head.get("epoch", "")
            sides[name] = side_session
            cleanups.append(cleanup)

        def side_connect(name: str) -> object:
            ws = side_ws[name]
            def connect() -> Session:
                return Session(_resolve_binary(), ws, [], seed_pack=False)
            return connect

        def post_connect() -> Session:
            return Session(_resolve_binary(), scratch_ws, [], seed_pack=False)

        # Live entity sets under each accepted head: stale object files
        # beside current versions never count (deleted entities drop out
        # via failed entity.version, never via filename presence).
        base_ids = _live_entity_set(side_connect("base.pack"), side_ws["base.pack"],
                                    repos["base.pack"], epochs["base.pack"])
        post_ids = _live_entity_set(post_connect, scratch_ws,
                                    scratch_ws / REPO_DIR,
                                    session.head.get("epoch", ""))
        ours_conflict = _entity_bound_bytes(
            sides["ours.pack"], repos["ours.pack"], conflict)
        post_conflict = _entity_bound_bytes(
            session, scratch_ws / REPO_DIR, conflict)
        if ours_conflict is None or post_conflict is None:
            _harness_fail("merge conflict objects")
        if post_conflict != ours_conflict:
            _reject("ORACLE_MERGE_CONFLICT", "shared constant not at merged value")
        theirs_ids = _live_entity_set(side_connect("theirs.pack"),
                                      side_ws["theirs.pack"],
                                      repos["theirs.pack"],
                                      epochs["theirs.pack"])
        fresh_theirs = [e for e in theirs_ids if e not in base_ids]
        fresh_post = [e for e in post_ids if e not in base_ids]
        if len(fresh_theirs) != 1 or len(fresh_post) != 1:
            _reject("ORACLE_MERGE_CONFLICT",
                    f"theirs={len(fresh_theirs)} post={len(fresh_post)}")
        theirs_body = _decode_bound_body(sides["theirs.pack"], repos["theirs.pack"],
                                         fresh_theirs[0])
        post_body = _decode_bound_body(session, scratch_ws / REPO_DIR,
                                       fresh_post[0])
        if theirs_body != post_body:
            _reject("ORACLE_MERGE_CONFLICT", "theirs-only change not preserved")
        for entity in post_ids:
            if entity in base_ids or entity == fresh_post[0]:
                continue
            if _entity_bound_bytes(session, scratch_ws / REPO_DIR, entity) != \
                    _entity_bound_bytes(sides["base.pack"], repos["base.pack"], entity):
                _reject("ORACLE_MERGE_CONFLICT", f"extra {entity[:16]}")
        if _live_entity_set(post_connect, scratch_ws,
                            scratch_ws / REPO_DIR,
                            session.head.get("epoch", "")) != post_ids:
            _reject("ORACLE_MERGE_UNSTABLE", "recompute diverged")
        _judge_merge_orders(candidate, manifest, sides, repos,
                            session, scratch_ws)
    finally:
        # Every side workspace is released. A session-close error stays
        # ignored as before; a failed scratch removal is a harness error,
        # raised once every cleanup has run.
        removal_failure: JudgeHarnessError | None = None
        for cleanup in cleanups:
            try:
                cleanup()
            except JudgeHarnessError as error:
                removal_failure = removal_failure or error
            except Exception:
                continue
        if removal_failure is not None:
            raise removal_failure


def _judge_merge_orders(candidate: bytes, manifest: dict,
                        sides: dict[str, Session], repos: dict[str, Path],
                        session: Session, scratch_ws: Path) -> None:
    """Rebase the contender onto each side head and require production
    validation both ways (observed order-independence). Replaces
    already satisfied on a side (side bytes equal post bytes) are
    skipped as vacuous; an empty remainder is vacuously valid."""

    from bench.live import sley2_tool as _tool

    principal = manifest.get("principal", "")
    if not principal:
        _harness_fail("merge principal")
    try:
        [unwrapped] = sley2_codecs.run_batch(
            [{"op": "record_from_stored", "stored": candidate.hex()}])
        [decoded] = sley2_codecs.run_batch(
            [{"op": "record_operations", "record": unwrapped["record"]}])
    except sley2_codecs.CodecError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: merge decode: {error}") from error
    operations = decoded.get("operations") or []
    if not operations:
        _reject("ORACLE_MERGE_UNSTABLE", "contender has no operations")
    post_repo = scratch_ws / REPO_DIR
    for name in ("ours.pack", "theirs.pack"):
        side = sides[name]
        repo = repos[name]
        ops = []
        for item in operations:
            if not isinstance(item, dict):
                _reject("ORACLE_MERGE_UNSTABLE", "contender op shape")
            class_name = item.get("class", "")
            kind = item.get("kind")
            target = item.get("target", "")
            payload = item.get("payload")
            if class_name not in ("CreateEntity", "ReplaceEntityVersion"):
                _reject("ORACLE_MERGE_UNSTABLE",
                        f"contender class {class_name}"[:64])
            if not isinstance(kind, int) or not _is_hash64(target) or payload is None:
                _reject("ORACLE_MERGE_UNSTABLE", "contender op shape")
            if class_name == "ReplaceEntityVersion":
                side_bytes = _entity_bound_bytes(side, repo, str(target))
                post_bytes = _entity_bound_bytes(session, post_repo, str(target))
                if side_bytes is not None and side_bytes == post_bytes:
                    continue
                ops.append({"class": class_name, "kind": kind, "target": target,
                            "field_tag": item.get("field_tag"),
                            "payload": payload})
            else:
                # Fresh creation under the new base nonce: identities
                # re-derive (never carried across bases).
                ops.append({"class": class_name, "kind": kind, "target": None,
                            "field_tag": item.get("field_tag"),
                            "payload": payload})
        if not ops:
            continue
        try:
            record_hex = _tool._assemble(side, ops)
        except Exception as error:
            raise JudgeRejection("ORACLE_MERGE_UNSTABLE",
                                 f"rebase assemble {name}: {error}"[:120]) from error
        try:
            [stored] = sley2_codecs.run_batch(
                [{"op": "stored_from_record", "record": record_hex}])
        except sley2_codecs.CodecError as error:
            raise JudgeHarnessError(
                f"LIVE_SLEY2_JUDGE_INVALID: merge stored: {error}") from error
        reply = side._raw_request(
            "candidate.validate",
            _encode_validate_body(side.head.get("tx", ""), principal,
                                  stored["stored"]))
        if reply["flags"].get("failed"):
            _reject("ORACLE_MERGE_UNSTABLE",
                    f"rebase {name} refused"[:64])
        _require_valid_decision(reply.get("body") or "",
                                code="ORACLE_MERGE_UNSTABLE")


def _repo_entity_ids(repo: Path, epoch: str) -> set[str]:
    """Entity ids present as object files in a repo dir (decoded)."""

    if not epoch:
        _harness_fail("epoch")
    calls = []
    for path in _store_files(repo):
        try:
            calls.append({"op": "decode_object", "stored": path.read_bytes().hex(),
                          "epoch": epoch})
        except OSError as error:
            raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: inventory: {error}") from error
    try:
        results = sley2_codecs.run_batch(calls) if calls else []
    except sley2_codecs.CodecError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: inventory: {error}") from error
    found = set()
    for result in results:
        entity = result["decoded"].get("entity_id", "")
        if entity:
            found.add(entity)
    return found


def _live_entity_set(connect: object, workspace: Path, repo: Path,
                      epoch: str) -> set[str]:
    """Live entity ids under an accepted head: enumerate distinct file
    entities, keep only those whose entity.version resolves now.
    Tombstoned/deleted entities drop out; stale duplicates collapse to
    one live identity (currency from the server, never file order).
    Sessions cap requests (frozen protocol limit), so liveness is
    probed in small chunks across fresh sessions."""

    if not epoch:
        _harness_fail("epoch")
    candidates = sorted(_repo_entity_ids(repo, epoch))
    live: set[str] = set()
    for index in range(0, len(candidates), 8):
        session = connect()
        try:
            for entity in candidates[index:index + 8]:
                if _live_object_id(session, entity) is not None:
                    live.add(entity)
        finally:
            session.close()
    return live


def _decode_bound_body(session: Session, repo: Path, entity: str) -> dict:
    """Typed body of one entity's CURRENT version (live binding ->
    exact object -> decode). Historical duplicates never win."""

    raw = _entity_bound_bytes(session, repo, entity)
    if raw is None:
        _reject("ORACLE_MISSING_TARGET", entity[:32])
        raise AssertionError("unreachable")
    try:
        [decoded] = sley2_codecs.run_batch([{
            "op": "decode_object", "stored": raw.hex(),
            "epoch": session.head.get("epoch", ""),
        }])
    except (sley2_codecs.CodecError, KeyError) as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: bound decode: {error}") from error
    body = decoded["decoded"].get("body")
    if not isinstance(body, dict):
        _harness_fail("entity body")
    return body


def _repo_version_of(repo: Path, epoch: str, entity: str) -> bytes | None:
    """Stored bytes of one entity in a repo dir (FILE SCAN, not currency).

    See _entity_stored_bytes warning: acceptance-critical comparisons
    must use _entity_bound_bytes instead."""

    return _entity_stored_bytes(repo, entity, epoch)


def _decode_fresh_body(session: Session, scratch_ws: Path, entity: str) -> dict:
    """Typed body of a fresh entity from the trial repo (CURRENT version)."""

    return _decode_bound_body(session, scratch_ws / REPO_DIR, entity)


def _decode_body_at(repo: Path, entity: str, epoch: str) -> dict:
    """Typed body of one entity from a side repo dir (FILE SCAN, not
    currency). Merge judging uses _decode_bound_body; this remains for
    non-acceptance inventory use."""

    raw = _entity_stored_bytes(repo, entity, epoch)
    if raw is None:
        _harness_fail("side entity")
    try:
        [decoded] = sley2_codecs.run_batch([{
            "op": "decode_object", "stored": raw.hex(), "epoch": epoch,
        }])
    except (sley2_codecs.CodecError, KeyError) as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: side decode: {error}") from error
    body = decoded["decoded"].get("body")
    if not isinstance(body, dict):
        _harness_fail("side body")
    return body


def _seeded_pack(pack: Path) -> tuple[Path, Path, object, object]:
    """Seeded throwaway session for one side pack (merge determinism).

    Returns (repo_path, workspace_path, session, cleanup): the
    workspace outlives single sessions so liveness probes can open
    fresh chunked sessions (frozen per-session request limit)."""

    workdir = Path(tempfile.mkdtemp(prefix="sley2-merge-"))
    ws, session = _staged_session(workdir, pack, "side pack")

    def cleanup() -> None:
        try:
            session.close()
        finally:
            _remove_workdir(workdir)

    return ws / REPO_DIR, ws, session, cleanup


def _repo_stems(repo: Path) -> set[str]:
    """Object ids of a repo dir (any layout)."""

    found = set()
    for item in _store_files(repo):
        digest = _object_id_of(item)
        if digest:
            found.add(digest)
    return found


# The enforced VM memory ceiling the strict-case driver passes with
# every request (ExecutionLimits.max_value_units): peak monotonic
# semantic value units above this refuse at the execution boundary.
# The PERF judge reports the measured peak against this ceiling;
# provider-process RSS, fuel, and constant zeros are never evidence.
DRIVER_MAX_VALUE_UNITS = 100_000


def _judge_perf(session: Session, manifest: dict, corpus: dict, scratch_ws: Path,
                task_dir: Path) -> None:
    """PERF task: identical outputs pre (pristine base pack) vs post on
    the governing fixed large inputs; instruction reduction at/above
    the frozen threshold; no effects; fuel never regresses; no memory
    ceiling breach. All measured on the actual submitted candidate (pre
    numbers come from the pristine pack through the same driver).

    The contract's memory quantity is peak monotonic semantic value
    units (ExecutionOutcome.peak_value_units), and its limit is the
    enforced max_value_units ceiling (DRIVER_MAX_VALUE_UNITS, the
    bound the driver passes with every request). Telemetry comes from
    the driver per case: a missing peak is a harness failure (absent
    telemetry is never a measured zero); a post peak above the
    enforced ceiling rejects. Provider-process RSS, fuel, and
    constant zeros are never substituted for VM value units."""

    _ = corpus
    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    func = entities.get("func", "")
    inputs = judge.get("fixed_inputs", [[9223372036854775800, 7]])
    threshold = judge.get("minimum_instruction_reduction_percent", 30)
    if not func:
        _harness_fail("perf entry")
    body = _decode_bound_body(session, scratch_ws / REPO_DIR, func)
    effects = body.get("effects")
    if not isinstance(effects, list) or effects:
        _reject("ORACLE_PERF_EFFECTS", "submitted entry must be effect-free")
    repo_pre, _, cleanup = _seeded_repo(task_dir)
    try:
        pre = _run_driver(repo_pre, func, _sint_cases(inputs)).get("cases")
    finally:
        cleanup()
    post = _run_driver(scratch_ws / REPO_DIR, func, _sint_cases(inputs)).get("cases")
    if not isinstance(pre, list) or not isinstance(post, list) or len(pre) != len(post):
        _harness_fail("perf cases")
    for before, after in zip(pre, post):
        if not before.get("ok") or not after.get("ok"):
            _reject("ORACLE_PERF_FAILED", json.dumps(after)[:160])
        if before.get("value") != after.get("value"):
            _reject("ORACLE_OUTPUT_MISMATCH", json.dumps(after.get("value"))[:120])
    instr_pre = sum(int(case.get("instructions", 0)) for case in pre)
    instr_post = sum(int(case.get("instructions", 0)) for case in post)
    fuel_pre = sum(int(case.get("fuel", 0)) for case in pre)
    fuel_post = sum(int(case.get("fuel", 0)) for case in post)
    if instr_pre <= 0:
        _harness_fail("perf baseline")
    reduction = 100.0 * (instr_pre - instr_post) / instr_pre
    if reduction < float(threshold):
        _reject("ORACLE_PERF_UNIMPROVED", f"{reduction:.1f}% < {threshold}%")
    if fuel_post > fuel_pre:
        _reject("ORACLE_PERF_UNIMPROVED",
                f"fuel regressed {fuel_post} > {fuel_pre}")
    for side, cases in (("pre", pre), ("post", post)):
        for case in cases:
            peak = case.get("peak_value_units")
            if (isinstance(peak, bool) or not isinstance(peak, int)
                    or peak < 0):
                _harness_fail(f"perf memory telemetry {side}")
    peak_post = max(int(case["peak_value_units"]) for case in post)
    if peak_post > DRIVER_MAX_VALUE_UNITS:
        _reject("ORACLE_PERF_MEMORY_CEILING",
                f"peak {peak_post} > ceiling {DRIVER_MAX_VALUE_UNITS}"[:120])


# Cumulative agent-visible response budget across a trial (mirrors the
# judge-transcript 4 MiB bound): bounded paging may accumulate pages, but
# unbounded accumulation rejects. Per-response cap is MAX_RESPONSE_BYTES.
AGENT_CUMULATIVE_RESPONSE_BUDGET = 4 * 1024 * 1024
# Query/refs methods are bounded paging routes under the governing
# contracts (never whole-store by name): each response must fit the
# per-response cap, and any omitted/truncated response must be followed
# by a query.continue (explicit continuation chain). Hidden truncation,
# inconsistent continuations, and over-cap responses reject.
BOUNDED_QUERY_METHODS = frozenset({"query.root", "query.restricted",
                                   "query.continue", "refs.list"})


class _ContinuationLedger:
    """Trial-wide continuation discharge, bound to the page it continues.

    Every answered root-query page carries its continuation binding
    (`chain`: the query key — the request with its cursor elided —, the
    request cursor, the page's own truncation flag, and its next cursor),
    derived on the trusted side from the exact bodies. A truncated
    `query.root` page opens (query, next cursor). Only a successful
    `query.continue` of that same query whose request cursor equals that
    next cursor discharges it; a continuation page that is itself truncated
    opens its own next cursor. A refused continue (or any failed bounded
    response) opens and discharges nothing. A successful continue that
    matches no open page — a skipped, past-the-end, repeated, or foreign
    cursor, or another query — is an inconsistent continuation. Any other
    bounded page that reports omissions or truncation (a cursor-bearing
    `query.root`, `query.restricted`, `refs.list`) has no continuation route
    and stays open. Scope is the trial, not the invocation or session:
    binding is by query and cursor, so continuation works across one-shot
    invocations while forged, refused, or unrelated continues never close
    a page. A page still open when the trial ends rejects.
    """

    def __init__(self, reject: Callable[[str], None]) -> None:
        self._reject = reject
        self._open: dict[tuple[str, str], int] = {}
        self._unbound = 0
        self.continuations = 0

    def _binding(self, chain: object) -> dict | None:
        if not isinstance(chain, dict):
            return None
        if not (isinstance(chain.get("query"), str)
                and isinstance(chain.get("truncated"), bool)
                and (chain.get("after") is None
                     or isinstance(chain.get("after"), str))
                and (chain.get("next") is None
                     or isinstance(chain.get("next"), str))):
            return None
        return chain

    def _open_next(self, chain: dict) -> None:
        following = chain.get("next")
        if not isinstance(following, str):
            self._reject("continuation binding; semantics held")
        key = (chain["query"], following)
        self._open[key] = self._open.get(key, 0) + 1

    def page(self, method: str, failed: bool, trunc: bool, omit: int,
             chain: object) -> None:
        if method not in BOUNDED_QUERY_METHODS:
            if trunc or omit > 0:
                self._reject(f"hidden truncation on {method}; semantics held")
            return
        if failed:
            return
        binding = self._binding(chain)
        if binding is not None and binding["truncated"] != trunc:
            self._reject("continuation binding; semantics held")
        if method == "query.continue":
            if binding is None or binding.get("after") is None:
                self._reject("inconsistent continuation; semantics held")
            key = (binding["query"], binding["after"])
            if self._open.get(key, 0) < 1:
                self._reject("inconsistent continuation; semantics held")
            self._open[key] -= 1
            if not self._open[key]:
                del self._open[key]
            self.continuations += 1
            if trunc:
                self._open_next(binding)
            return
        if not (trunc or omit > 0):
            return
        if (method == "query.root" and trunc and binding is not None
                and binding.get("after") is None):
            self._open_next(binding)
        else:
            self._unbound += 1

    def finish(self) -> None:
        if self._open or self._unbound:
            self._reject("truncated page without continuation; semantics held")


def _audit_agent_access(task_dir: Path, trial_ws: Path) -> dict[str, int]:
    """CONTEXT agent-read evidence from the trusted tool boundary.

    Three claims stay separated: the hash chain proves ordering and
    tamper-evidence only. Completeness (the record contains every
    agent-visible operation and response) is established by
    durable-before-release, per-entry response accounting, transition
    linkage, and final linkage below. Absence of an unrecorded route to
    protected state is established by provider confinement and the
    tool-allowlist/repo-untouched checks, not by the chain. The chain
    alone establishes neither of the other two.

    Bounded-query semantics (governing ROOT_BACKED / RESTRICTED / CAPSULE
    contracts): query.root, query.restricted, query.continue, and
    refs.list are bounded paging routes, NOT whole-store reads by
    method name. Each such response must fit MAX_RESPONSE_BYTES;
    cumulative agent-visible bytes across the trial must fit
    AGENT_CUMULATIVE_RESPONSE_BUDGET. Continuation discipline is the
    trial-wide `_ContinuationLedger` (explicit continuation; the corpus
    requires bounded operation WITH omissions and continuations, not
    their absence): a truncated query.root page opens (query key, next
    cursor), and only a successful query.continue of the same query
    whose request cursor equals that next cursor discharges it, so
    after == the previous page's next is verified from the exact
    bodies. Scope is the trial, bound by query and cursor, not by
    session. A continue that matches no open page is inconsistent; a
    page still open at trial end, or an omitting page with no
    continuation route, rejects. Whole-store reads are only
    inventory/side (full file enumeration). Hidden truncation,
    exceeded bounds, inconsistent continuations, and unbounded access
    reject under the frozen unbounded-read code; semantic-only success
    is noted in the rejection detail, never mislabeled as a pass.

    Limitation retained: requested limit values are not recorded, so
    the judge verifies actual returned content and per-response limits,
    not the literal requested limit parameters. Judge-only inspection
    (judge transcript) stays separate from agent-visible context and
    cost.

    whole_store_reads is derived, never defaulted. Unknowns are never
    reported as zero: incomplete evidence rejects instead of returning
    counts.
    """

    chain = trial_ws / CHAIN_NAME
    if not chain.is_file() or chain.is_symlink():
        _reject("QUERY_REQUIRED_FACT_OMITTED",
                "agent access evidence missing; semantics held")
    entries, reason = verify_transcript_chain(chain)
    if reason is not None:
        _reject("QUERY_REQUIRED_FACT_OMITTED",
                f"agent access evidence {reason}; semantics held")
    if not entries:
        _reject("QUERY_REQUIRED_FACT_OMITTED",
                "agent access evidence empty; semantics held")
    try:
        pack_digest = hashlib.sha256((task_dir / "base.pack").read_bytes()).hexdigest()
    except OSError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: fixture pack: {error}") from error
    whole_store = 0
    targeted: set[str] = set()
    operations = 0
    bounded_reads = 0
    continuations = 0
    omitted = 0
    truncated = 0
    refusals = 0
    max_response = 0
    cumulative = 0
    calls = 0
    binary_ids: set[str] = set()
    try:
        judge_binary = hashlib.sha256(_resolve_binary().read_bytes()).hexdigest()
    except Exception:
        judge_binary = ""
    ledger = _ContinuationLedger(
        lambda detail: _reject("QUERY_REQUIRED_FACT_OMITTED", detail))
    for entry in entries:
        if not isinstance(entry, dict):
            _reject("QUERY_REQUIRED_FACT_OMITTED", "agent entry shape; semantics held")
        if entry.get("tool_version") != TOOL_VERSION:
            _reject("QUERY_REQUIRED_FACT_OMITTED",
                    f"tool version {entry.get('tool_version')}; semantics held")
        binary_id = entry.get("sley_binary_sha256", "")
        if not _is_hash64(binary_id):
            _reject("QUERY_REQUIRED_FACT_OMITTED",
                    "tool/binary identity missing; semantics held")
        binary_ids.add(str(binary_id))
        if entry.get("pack_sha256") != pack_digest:
            _reject("QUERY_REQUIRED_FACT_OMITTED",
                    "fixture binding mismatch; semantics held")
        command = entry.get("command", "")
        if command in ("inventory", "side"):
            # Full file enumeration of every served object: the only
            # whole-store routes. Bounded query methods below are NOT
            # whole-store by name.
            whole_store += 1
        args = entry.get("args") if isinstance(entry.get("args"), dict) else {}
        for entity in args.get("entities", []) if isinstance(args.get("entities"), list) else []:
            if entity != "all":
                targeted.add(str(entity))
        if command in ("propose", "compose", "append", "finish"):
            operations += 1
        if not entry.get("ok"):
            refusals += 1
        summary = entry.get("summary") if isinstance(entry.get("summary"), dict) else {}
        try:
            report_bytes = int(entry.get("report_bytes", 0) or 0)
        except (TypeError, ValueError):
            _reject("QUERY_REQUIRED_FACT_OMITTED", "agent summary shape; semantics held")
        max_response = max(max_response, report_bytes)
        if report_bytes > MAX_RESPONSE_BYTES:
            _reject("QUERY_REQUIRED_FACT_OMITTED",
                    f"response over bound {report_bytes}; semantics held")
        session = entry.get("session") if isinstance(entry.get("session"), list) else []
        # Per-call bounded accounting over the ordered session items:
        # pair each request with its following response.
        pending: dict | None = None
        entry_continuations = 0
        entry_omitted = 0
        entry_truncated = 0
        entry_returned = 0
        entry_failed = 0
        for item in session:
            if not isinstance(item, dict):
                _reject("QUERY_REQUIRED_FACT_OMITTED",
                        "agent session shape; semantics held")
            direction = item.get("direction")
            if direction == "request":
                calls += 1
                method = item.get("method", "")
                if method == "commit":
                    _reject("QUERY_REQUIRED_FACT_OMITTED",
                            "agent commit path; semantics held")
                if method in BOUNDED_QUERY_METHODS:
                    bounded_reads += 1
                pending = item
            elif direction == "response":
                if pending is None:
                    _reject("QUERY_REQUIRED_FACT_OMITTED",
                            "agent session order; semantics held")
                method = pending.get("method", "") if isinstance(pending, dict) else ""
                pending = None
                try:
                    returned = int(item.get("returned_bytes", 0) or 0)
                    omit = int(item.get("omitted", 0) or 0)
                except (TypeError, ValueError):
                    _reject("QUERY_REQUIRED_FACT_OMITTED",
                            "agent response shape; semantics held")
                trunc = bool(item.get("truncated"))
                if item.get("failed"):
                    entry_failed += 1
                entry_returned += returned
                entry_omitted += omit
                if trunc:
                    entry_truncated += 1
                max_response = max(max_response, returned)
                cumulative += returned
                if returned > MAX_RESPONSE_BYTES:
                    _reject("QUERY_REQUIRED_FACT_OMITTED",
                            f"response over bound {returned}; semantics held")
                before = ledger.continuations
                ledger.page(method, bool(item.get("failed")), trunc, omit,
                            item.get("chain"))
                entry_continuations += ledger.continuations - before
            else:
                # Non-call transcript markers (hello/greeting/seed/scope):
                # ordering-relevant but not agent-visible calls; the
                # hash chain covers them, the call audit skips them.
                continue
        if pending is not None:
            _reject("QUERY_REQUIRED_FACT_OMITTED",
                    "agent session order; semantics held")
        # Reconcile the entry summary against the enumerated session
        # items (completeness: summaries must equal recorded calls —
        # overstated or understated accounting both reject, naming the
        # field, so summaries cannot hide or invent response bounds).
        try:
            sum_cont = int(summary.get("continuations", 0) or 0)
            sum_omit = int(summary.get("omitted", 0) or 0)
            sum_fail = int(summary.get("failed", 0) or 0)
            sum_ret = int(summary.get("returned_bytes", 0) or 0)
            sum_trunc = 1 if summary.get("truncated") else 0
        except (TypeError, ValueError):
            _reject("QUERY_REQUIRED_FACT_OMITTED", "agent summary shape; semantics held")
        if sum_omit != entry_omitted:
            _reject("QUERY_REQUIRED_FACT_OMITTED",
                    f"agent summary omitted mismatch; semantics held")
        if sum_fail != entry_failed:
            _reject("QUERY_REQUIRED_FACT_OMITTED",
                    "agent summary failed mismatch; semantics held")
        if sum_ret != entry_returned:
            _reject("QUERY_REQUIRED_FACT_OMITTED",
                    "agent summary returned_bytes mismatch; semantics held")
        if sum_trunc != (1 if entry_truncated else 0):
            _reject("QUERY_REQUIRED_FACT_OMITTED",
                    "agent summary truncated mismatch; semantics held")
        # Summary continuations counts every CONTINUATION_METHODS
        # request (root/restricted/continue); the audit counts
        # query.continue follows only — so the summary must at least
        # cover the enumerated follows (it may legitimately exceed).
        if sum_cont < entry_continuations:
            _reject("QUERY_REQUIRED_FACT_OMITTED",
                    "agent summary continuations understated; semantics held")
        continuations += entry_continuations
        omitted += entry_omitted
        truncated += entry_truncated
        refusals += entry_failed
        if cumulative > AGENT_CUMULATIVE_RESPONSE_BUDGET:
            _reject("QUERY_REQUIRED_FACT_OMITTED",
                    f"cumulative {cumulative} over budget; semantics held")
    ledger.finish()
    if len(binary_ids) != 1 or (judge_binary and next(iter(binary_ids)) != judge_binary):
        _reject("QUERY_REQUIRED_FACT_OMITTED",
                "tool/binary identity mismatch; semantics held")
    if whole_store > 0:
        _reject("QUERY_REQUIRED_FACT_OMITTED",
                f"whole_store_reads={whole_store}; semantics held")
    _verify_candidate_transitions(entries)
    _verify_final_linkage(trial_ws, entries)
    return {"whole_store_reads": whole_store, "targeted_reads": len(targeted),
            "operations": operations, "continuations": continuations,
            "bounded_reads": bounded_reads,
            "omitted": omitted, "truncated": truncated, "refusals": refusals,
            "max_response_bytes": max_response, "agent_requests": calls,
            "invocations": len(entries)}


def _is_hash64(value: object) -> bool:
    return (isinstance(value, str) and len(value) == 64
            and all(ch in "0123456789abcdef" for ch in value))


def _mediated_capture_dir() -> Path | None:
    """Runner-controlled reconciled capture for this attempt, when the
    trial ran through the mediated gateway (set by the runner, never
    by the agent). Unset on the direct tool path, which keeps the
    legacy chain-file route."""

    raw = os.environ.get("SLEY2_MEDIATED_CAPTURE_DIR", "")
    if not raw or "\x00" in raw:
        return None
    return Path(raw)


def _capture_canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"),
                      ensure_ascii=True).encode("utf-8")


def _capture_seal_digest(previous: str, record: dict) -> str:
    body = {key: value for key, value in record.items()
            if key not in ("prev", "hash")}
    return hashlib.sha256(
        (previous + _capture_canonical(body).decode("ascii")
         ).encode("ascii")).hexdigest()


def _audit_mediated_access(capture_dir: Path, task_dir: Path,
                           trial_ws: Path) -> dict[str, int]:
    """CONTEXT access/budget evidence from the runner-owned reconciled
    capture (mediated trials): hash-chained request/response pairs with
    per-call bytes, failure, omission, truncation, and continuation
    accounting — never candidate-side files.

    Binding verification (fail-closed, never defaulted):
    - start.json contract, attempt match, frozen pack/manifest/tool/
      binary bindings against judge-computed values;
    - exchanges.jsonl hash chain from genesis, request/response seq
      pairing, per-record attempt match;
    - completion.json contract/attempt, exchange count, chain head,
      and final_sha256 against the trial final artifact when one
      exists (read-only probes link nothing).

    Audit rules mirror the legacy chain audit at captured-method
    granularity (`raw:<server method>` exposes the inner query
    method; gateway-local `resolve` is neutral): bounded query
    methods are not whole-store reads by name; continuation is the
    trial-wide `_ContinuationLedger`, bound by query key and cursor
    (a truncated query.root page is discharged only by a successful
    query.continue of the same query whose request cursor equals the
    page's next cursor, across invocations and sessions); a
    query.continue that matches no open page is inconsistent, and a
    page left open at trial end rejects; cumulative and per-response
    budgets bind; only
    inventory/side count as whole-store; commit and hidden truncation
    reject. Scope/root consistency comes from the capture's session
    scopes and frozen pack/manifest bindings, not method names alone.
    """

    def fail(detail: str) -> None:
        _reject("QUERY_REQUIRED_FACT_OMITTED", detail)

    if not capture_dir.is_dir() or capture_dir.is_symlink():
        fail("mediated capture missing; semantics held")
    try:
        start = json.loads((capture_dir / "start.json").read_bytes()
                           .decode("utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError):
        fail("mediated start unreadable; semantics held")
    if (not isinstance(start, dict)
            or start.get("contract") != "sley2.trusted-capture.v1"
            or not isinstance(start.get("attempt_id"), str)
            or not start["attempt_id"]):
        fail("mediated start shape; semantics held")
    attempt_id = start["attempt_id"]
    frozen = start.get("frozen")
    if not isinstance(frozen, dict):
        fail("mediated frozen bindings missing; semantics held")
    try:
        pack_digest = hashlib.sha256(
            (task_dir / "base.pack").read_bytes()).hexdigest()
        manifest_digest = hashlib.sha256(
            (task_dir / "task_manifest.json").read_bytes()).hexdigest()
    except OSError as error:
        raise JudgeHarnessError(
            f"LIVE_SLEY2_JUDGE_INVALID: fixture: {error}") from error
    if frozen.get("pack_sha256") != pack_digest:
        fail("mediated fixture binding mismatch; semantics held")
    if frozen.get("task_manifest_sha256") != manifest_digest:
        fail("mediated manifest binding mismatch; semantics held")
    if frozen.get("tool_version") != TOOL_VERSION:
        fail("mediated tool version mismatch; semantics held")
    try:
        judge_binary = hashlib.sha256(_resolve_binary().read_bytes()
                                      ).hexdigest()
    except Exception:
        judge_binary = ""
    if (not _is_hash64(frozen.get("binary_sha256"))
            or (judge_binary and frozen.get("binary_sha256")
                != judge_binary)):
        fail("mediated tool/binary identity mismatch; semantics held")
    try:
        lines = (capture_dir / "exchanges.jsonl").read_bytes().decode(
            "utf-8").splitlines()
    except OSError:
        fail("mediated exchanges missing; semantics held")
    if not lines:
        fail("mediated exchanges empty; semantics held")
    pairs: list[tuple[dict, dict]] = []
    previous = "0" * 64
    pending: dict | None = None
    seq = 0
    for number, line in enumerate(lines):
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            fail(f"mediated exchange line {number} shape; semantics held")
        if not isinstance(record, dict):
            fail(f"mediated exchange line {number} shape; semantics held")
        if record.get("attempt_id") != attempt_id:
            fail(f"mediated exchange line {number} attempt; semantics held")
        if record.get("prev") != previous:
            fail(f"mediated exchange line {number} prev link; semantics held")
        if _capture_seal_digest(previous, record) != record.get("hash"):
            fail(f"mediated exchange line {number} hash; semantics held")
        previous = str(record.get("hash"))
        kind = record.get("kind")
        if kind == "request":
            if pending is not None:
                fail("mediated exchange order; semantics held")
            if record.get("seq") != seq:
                fail("mediated exchange seq; semantics held")
            pending = record
        elif kind == "response":
            if pending is None:
                fail("mediated exchange order; semantics held")
            if record.get("seq") != seq:
                fail("mediated exchange seq; semantics held")
            for key in ("phase", "method", "session_id"):
                if record.get(key) != pending.get(key):
                    fail("mediated request/response binding; semantics held")
            pairs.append((pending, record))
            pending = None
            seq += 1
        else:
            fail(f"mediated exchange line {number} kind; semantics held")
    if pending is not None:
        fail("mediated exchange order; semantics held")
    try:
        completion = json.loads((capture_dir / "completion.json")
                                .read_bytes().decode("utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError):
        fail("mediated completion missing; semantics held")
    if (not isinstance(completion, dict)
            or completion.get("contract")
            != "sley2.trusted-capture-completion.v1"
            or completion.get("attempt_id") != attempt_id):
        fail("mediated completion shape; semantics held")
    if completion.get("exchanges") != len(pairs):
        fail("mediated completion count; semantics held")
    if completion.get("chain_head") != previous:
        fail("mediated completion head; semantics held")
    final = trial_ws / "final_candidate.hex"
    try:
        if final.is_file() and not final.is_symlink():
            final_bytes = final.read_bytes()
            if final_bytes:
                # Completion binds the exact protected final bytes
                # (the file as the runner held it).
                final_digest = hashlib.sha256(final_bytes).hexdigest()
                if final_digest != completion.get("final_sha256"):
                    fail("mediated final binding mismatch; semantics held")
    except OSError:
        fail("mediated final binding unverifiable; semantics held")
    whole_store = 0
    operations = 0
    bounded_reads = 0
    continuations = 0
    omitted = 0
    truncated = 0
    refusals = 0
    max_response = 0
    cumulative = 0
    calls = 0
    ledger = _ContinuationLedger(fail)
    from bench.live.mediated_sley import AUDIT_LABELS

    def inner_method(method: object) -> str:
        if not isinstance(method, str):
            return ""
        if method.startswith("raw:"):
            return method[len("raw:"):]
        return method

    for request, response in pairs:
        calls += 1
        label = request.get("method")
        if label not in AUDIT_LABELS:
            # Closed label vocabulary: the gateway records denied or unknown
            # commands as `denied`, never under an agent-chosen string, so
            # an out-of-set label is not evidence of a routed call.
            fail(f"mediated capture label {str(label)[:40]}; semantics held")
        method = inner_method(label)
        scope = request.get("session_id")
        if not isinstance(scope, str) or not scope:
            fail("mediated session scope; semantics held")
        if method in ("inventory", "side"):
            # Full file enumeration of every served object: the only
            # whole-store routes. Bounded query methods below are NOT
            # whole-store by name.
            whole_store += 1
        if method == "commit":
            fail("agent commit path; semantics held")
        if method in ("propose", "compose", "append", "finish"):
            operations += 1
        try:
            returned = int(response.get("response_bytes", 0) or 0)
            omit = int(response.get("omitted", 0) or 0)
        except (TypeError, ValueError):
            fail("mediated response shape; semantics held")
        trunc = bool(response.get("truncated"))
        if response.get("failed"):
            refusals += 1
        omitted += omit
        if trunc:
            truncated += 1
        max_response = max(max_response, returned)
        cumulative += returned
        if returned > MAX_RESPONSE_BYTES:
            fail(f"response over bound {returned}; semantics held")
        if method in BOUNDED_QUERY_METHODS:
            bounded_reads += 1
        ledger.page(method, bool(response.get("failed")), trunc, omit,
                    response.get("chain"))
        if cumulative > AGENT_CUMULATIVE_RESPONSE_BUDGET:
            fail(f"cumulative {cumulative} over budget; semantics held")
    ledger.finish()
    continuations = ledger.continuations
    if whole_store > 0:
        fail(f"whole_store_reads={whole_store}; semantics held")
    return {"whole_store_reads": whole_store,
            "operations": operations, "continuations": continuations,
            "bounded_reads": bounded_reads, "omitted": omitted,
            "truncated": truncated, "refusals": refusals,
            "max_response_bytes": max_response, "agent_requests": calls,
            "invocations": len(pairs)}


def _verify_candidate_transitions(entries: list) -> None:
    """Compose/append/finish linkage for access-evidence claims.

    Each successful propose/compose/append must carry a non-null output
    transition binding; each successful compose/append must carry a
    non-null input binding equal to the most recent prior output; each
    successful finish must carry a non-null input binding equal to the
    most recent prior output. A missing or inconsistent transition cannot
    support an accepted access-evidence claim: it rejects under the
    frozen unbounded-read code with semantics-held detail, never as a
    silent zero.
    """

    last_out: str | None = None
    for entry in entries:
        if not isinstance(entry, dict):
            continue
        command = entry.get("command", "")
        records = entry.get("records") if isinstance(entry.get("records"), dict) else {}
        args = entry.get("args") if isinstance(entry.get("args"), dict) else {}
        if command not in ("propose", "compose", "append", "finish"):
            continue
        if not entry.get("ok"):
            continue
        if command == "propose":
            out = records.get("out")
            if not _is_hash64(out):
                _reject("QUERY_REQUIRED_FACT_OMITTED",
                        "propose transition missing out; semantics held")
            if not isinstance(args.get("op_count"), int) or args["op_count"] <= 0:
                _reject("QUERY_REQUIRED_FACT_OMITTED",
                        "propose transition missing ops; semantics held")
            last_out = str(out)
        elif command in ("compose", "append"):
            inp = records.get("in")
            out = records.get("out")
            if not _is_hash64(inp) or not _is_hash64(out):
                _reject("QUERY_REQUIRED_FACT_OMITTED",
                        f"{command} transition missing in/out; semantics held")
            if not isinstance(args.get("op_count"), int) or args["op_count"] <= 0:
                _reject("QUERY_REQUIRED_FACT_OMITTED",
                        f"{command} transition missing ops; semantics held")
            if last_out is None or inp != last_out:
                _reject("QUERY_REQUIRED_FACT_OMITTED",
                        f"{command} transition mismatch; semantics held")
            last_out = str(out)
        elif command == "finish":
            inp = records.get("in")
            if not _is_hash64(inp):
                _reject("QUERY_REQUIRED_FACT_OMITTED",
                        "finish transition missing in; semantics held")
            if last_out is None or inp != last_out:
                _reject("QUERY_REQUIRED_FACT_OMITTED",
                        "finish transition mismatch; semantics held")


def _verify_final_linkage(trial_ws: Path, entries: list) -> None:
    """The finished record hash must match the submitted artifact.

    final_candidate.hex holds STORED bytes; the transcript binds RECORD
    hashes. The judge digest-verifies stored -> record through the pinned
    codecs and requires the resulting record hash to equal the finish
    input and the last propose/compose/append output. When no final
    artifact exists (read-only evidence probes) there is nothing to link.
    """

    final = trial_ws / "final_candidate.hex"
    try:
        if not final.is_file() or final.is_symlink():
            return
        stored_hex = final.read_text(encoding="utf-8").strip()
    except OSError:
        return
    if not stored_hex or len(stored_hex) % 2:
        return
    try:
        [unwrapped] = sley2_codecs.run_batch(
            [{"op": "record_from_stored", "stored": stored_hex}])
        record_hex = unwrapped["record"]
    except Exception:
        _reject("QUERY_REQUIRED_FACT_OMITTED",
                "final linkage unverifiable; semantics held")
        raise AssertionError("unreachable")
    want = hashlib.sha256(record_hex.encode()).hexdigest()
    last_out: str | None = None
    finish_in: str | None = None
    for entry in entries:
        if not isinstance(entry, dict) or not entry.get("ok"):
            continue
        command = entry.get("command", "")
        records = entry.get("records") if isinstance(entry.get("records"), dict) else {}
        if command in ("propose", "compose", "append"):
            out = records.get("out")
            if _is_hash64(out):
                last_out = str(out)
        elif command == "finish":
            inp = records.get("in")
            if _is_hash64(inp):
                finish_in = str(inp)
    if last_out is None or finish_in is None:
        _reject("QUERY_REQUIRED_FACT_OMITTED",
                "final linkage missing transitions; semantics held")
    if finish_in != last_out or finish_in != want:
        _reject("QUERY_REQUIRED_FACT_OMITTED",
                "final linkage mismatch; semantics held")


def _live_object_count(session: Session) -> int:
    """Live object count under the accepted head via revision.read.

    Single owner-derived count (summary field 6 = objects().len()):
    retained historical files never inflate it, and a deleted entity
    stops counting the moment its tombstone commits. File scans must
    not substitute for this in acceptance-critical counts."""

    head_tx = session.head.get("tx", "")
    if not head_tx:
        _harness_fail("head for revision count")
    reply = session._raw_request("revision.read", head_tx)
    if reply["flags"].get("failed"):
        _harness_fail("revision count unreadable")
    try:
        fields = _parse_record_fields(bytes.fromhex(reply["body"]))
        count, _ = _uvar(fields[6], 0)
    except Exception as error:
        raise JudgeHarnessError(
            f"LIVE_SLEY2_JUDGE_INVALID: revision count: {error}") from error
    return int(count)


def _judge_bounded(session: Session, manifest: dict, corpus: dict, scratch_ws: Path,
                   transcript: list, task_dir: Path, trial_ws: Path) -> str:
    """CONTEXT task: minimum live-entity count, required field present,
    impact closure coherent, collateral clean (generic path already ran),
    and the judge's own transcript shows bounded usage only (no
    whole-store dump, no failed calls, per-reply byte caps held)."""

    _ = scratch_ws
    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    minimum = int(judge.get("minimum_entities", 10000))
    count = _live_object_count(session)
    if count < minimum:
        _reject("ORACLE_BELOW_MINIMUM", f"live {count} < {minimum}")
    typedef = entities.get("typedef", "")
    if not typedef:
        _harness_fail("typedef entity")
    reply = session._raw_request("entity.version", _tool_entity_body(session, typedef))
    if reply["flags"].get("failed"):
        _reject("ORACLE_TYPEDEF_UNREADABLE", typedef[:32])
    added = _typedef_added_members(task_dir, session, scratch_ws, typedef)
    if not added:
        _reject("ORACLE_IMPACT_INCOMPLETE", "no added member")
    _verify_impact_closure(session, scratch_ws, typedef, added)
    _audit_transcript_bounds(transcript)
    mediated_dir = _mediated_capture_dir()
    if mediated_dir is not None:
        # Mediated trials: access/budget evidence comes from the
        # runner-owned reconciled capture, never the obsolete
        # candidate-workspace chain file (the gateway path writes
        # none, by design).
        access = _audit_mediated_access(mediated_dir, task_dir, trial_ws)
        return ("mediated whole_store_reads={whole_store_reads} "
                "operations={operations} continuations={continuations} "
                "bounded_reads={bounded_reads} refusals={refusals} "
                "max_response_bytes={max_response_bytes}").format(**access)
    access = _audit_agent_access(task_dir, trial_ws)
    _ = corpus
    return ("whole_store_reads={whole_store_reads} targeted_reads={targeted_reads} "
            "operations={operations} continuations={continuations} "
            "bounded_reads={bounded_reads} refusals={refusals} "
            "max_response_bytes={max_response_bytes}").format(**access)


def _typedef_record_fields(body: dict) -> list:
    """Record fields of a decoded TypeDef body in definition order."""

    try:
        form = body.get("form") or {}
        if form.get("variant") != "Record":
            _harness_fail("typedef form")
        fields = form.get("value") or []
        if not isinstance(fields, list):
            _harness_fail("typedef fields")
        return fields
    except AttributeError:
        _harness_fail("typedef body")
        raise AssertionError("unreachable")


def _decode_object_file(session: Session, path: Path) -> dict:
    """One stored object decoded through the pinned codecs."""

    try:
        [obj] = sley2_codecs.run_batch([{
            "op": "decode_object", "stored": path.read_bytes().hex(),
            "epoch": session.head["epoch"],
        }])
    except (OSError, sley2_codecs.CodecError, KeyError, IndexError) as error:
        raise JudgeHarnessError(
            f"LIVE_SLEY2_JUDGE_INVALID: decode: {error}") from error
    decoded = obj["decoded"]
    if not isinstance(decoded, dict):
        _harness_fail("decoded shape")
    return decoded


def _read_current_typedef(session: Session, scratch_ws: Path,
                          typedef: str) -> dict:
    """Current TypeDef body through its live binding (a single
    entity.version read, not a store-wide snapshot)."""

    reply = session._raw_request(
        "entity.version", _tool_entity_body(session, typedef))
    if reply["flags"].get("failed"):
        _reject("ORACLE_TYPEDEF_UNREADABLE", typedef[:32])
    try:
        [decoded] = sley2_codecs.run_batch([{
            "op": "decode_response", "method": "entity.version",
            "body": reply["body"],
        }])
    except (sley2_codecs.CodecError, KeyError) as error:
        raise JudgeHarnessError(
            f"LIVE_SLEY2_JUDGE_INVALID: typedef: {error}") from error
    entries = decoded["decoded"].get("entries") or []
    if len(entries) != 1:
        _reject("ORACLE_TYPEDEF_UNREADABLE", typedef[:32])
    path = _object_path(scratch_ws / REPO_DIR,
                        entries[0].get("object_id", ""))
    if path is None:
        _reject("ORACLE_TYPEDEF_UNREADABLE", typedef[:32])
    body = _decode_object_file(session, path).get("body")
    if not isinstance(body, dict):
        _harness_fail("typedef body")
    return body


def _read_base_typedef(task_dir: Path, typedef: str) -> dict:
    """Pristine pre-image TypeDef body from the task base pack (the
    same seeded-repo helper the impl-unchanged check uses)."""

    repo_pre, pre_session, cleanup = _seeded_repo(task_dir)
    try:
        raw = _entity_bound_bytes(pre_session, repo_pre, typedef)
        if raw is None:
            _harness_fail("base typedef")
        try:
            [obj] = sley2_codecs.run_batch([{
                "op": "decode_object", "stored": raw.hex(),
                "epoch": pre_session.head["epoch"],
            }])
        except (sley2_codecs.CodecError, KeyError, IndexError) as error:
            raise JudgeHarnessError(
                f"LIVE_SLEY2_JUDGE_INVALID: base typedef: {error}") from error
    finally:
        cleanup()
    body = obj["decoded"].get("body")
    if not isinstance(body, dict):
        _harness_fail("base typedef body")
    return body


def _typedef_added_members(task_dir: Path, session: Session,
                           scratch_ws: Path, typedef: str) -> list:
    """Members present on the current typedef but absent from the
    pristine base pre-image, each with its declared type. The
    governing task names no member identity or type: ANY added member
    counts, and every impact-closure constant must carry each of them
    with the declared type (no private-manifest literal)."""

    base_fields = _typedef_record_fields(_read_base_typedef(task_dir, typedef))
    current_fields = _typedef_record_fields(
        _read_current_typedef(session, scratch_ws, typedef))
    base_members = {str(field.get("member_id")) for field in base_fields
                    if isinstance(field, dict)}
    added = []
    for field in current_fields:
        if not isinstance(field, dict):
            _harness_fail("typedef fields")
        member = str(field.get("member_id"))
        if member not in base_members:
            decltype = field.get("value_type")
            if not isinstance(decltype, dict):
                _harness_fail("typedef fields")
            added.append((member, decltype))
    return added


def _live_bound_object(session: Session, scratch_ws: Path,
                         entity: str) -> dict:
    """Current body of one entity through its live binding."""

    reply = session._raw_request(
        "entity.version", _tool_entity_body(session, entity))
    if reply["flags"].get("failed"):
        raise JudgeHarnessError(
            f"LIVE_SLEY2_JUDGE_INVALID: live binding: {entity[:16]}")
    try:
        [decoded] = sley2_codecs.run_batch([{
            "op": "decode_response", "method": "entity.version",
            "body": reply["body"],
        }])
    except (sley2_codecs.CodecError, KeyError) as error:
        raise JudgeHarnessError(
            f"LIVE_SLEY2_JUDGE_INVALID: live binding: {error}") from error
    entries = decoded["decoded"].get("entries") or []
    if len(entries) != 1:
        raise JudgeHarnessError("LIVE_SLEY2_JUDGE_INVALID: live binding shape")
    path = _object_path(scratch_ws / REPO_DIR,
                        entries[0].get("object_id", ""))
    if path is None:
        raise JudgeHarnessError("LIVE_SLEY2_JUDGE_INVALID: live binding path")
    body = _decode_object_file(session, path).get("body")
    if not isinstance(body, dict):
        _harness_fail("live binding body")
    return body


def _impact_consts(session: Session, scratch_ws: Path,
                   typedef: str) -> list:
    """Live record constants whose value type names the typedef: the
    complete impact closure, discovered structurally from stored
    state (never from manifest roles). Every stored object decodes
    (chunked batches); entities with several stored versions resolve
    through live bindings, so no store-wide version snapshot is
    needed."""

    groups: dict[str, list] = {}
    paths = _store_files(scratch_ws / REPO_DIR)
    for start in range(0, len(paths), 1000):
        chunk = paths[start:start + 1000]
        calls = []
        for path in chunk:
            try:
                calls.append({"op": "decode_object",
                              "stored": path.read_bytes().hex(),
                              "epoch": session.head["epoch"]})
            except OSError as error:
                raise JudgeHarnessError(
                    f"LIVE_SLEY2_JUDGE_INVALID: inventory: {error}"
                ) from error
        try:
            results = sley2_codecs.run_batch(calls) if calls else []
        except sley2_codecs.CodecError as error:
            raise JudgeHarnessError(
                f"LIVE_SLEY2_JUDGE_INVALID: decode: {error}") from error
        for result in results:
            decoded = result["decoded"]
            if not isinstance(decoded, dict):
                continue
            try:
                if decoded.get("kind") != 9:
                    continue
                body = decoded.get("body")
                if not isinstance(body, dict):
                    continue
                value = body.get("value")
                if not isinstance(value, dict):
                    continue
                typed = value.get("value_type")
                if not (isinstance(typed, dict)
                        and typed.get("variant") == "Named"
                        and isinstance(typed.get("value"), dict)
                        and typed["value"].get("definition") == typedef):
                    continue
                entity = (decoded.get("entity_id", "")
                          or decoded.get("entity", ""))
                if entity:
                    groups.setdefault(str(entity), []).append(body)
            except (AttributeError, TypeError):
                continue
    found = []
    for entity, bodies in groups.items():
        if len(bodies) == 1:
            found.append((entity, bodies[0]))
            continue
        try:
            found.append((entity, _live_bound_object(
                session, scratch_ws, entity)))
        except JudgeHarnessError:
            raise
        except Exception as error:
            raise JudgeHarnessError(
                f"LIVE_SLEY2_JUDGE_INVALID: impact binding: {error}"
            ) from error
    return found


def _verify_impact_closure(session: Session, scratch_ws: Path, typedef: str,
                           added: list) -> None:
    """Every impact-closure record constant carries every added member
    with its declared type (the closure update is complete, not just
    the typedef)."""

    consts = _impact_consts(session, scratch_ws, typedef)
    if not consts:
        _reject("ORACLE_IMPACT_INCOMPLETE", "no impact constants")
    for member, decltype in added:
        for entity, body in consts:
            try:
                value = body.get("value") or {}
                data = value.get("data") or {}
                record = data.get("value") or {}
                fields = record.get("fields")
            except AttributeError:
                _harness_fail("impact body")
            if not isinstance(fields, list):
                _harness_fail("impact body")
            if not any(isinstance(field, dict)
                       and field.get("member_id") == member
                       and isinstance(field.get("value"), dict)
                       and field["value"].get("value_type") == decltype
                       for field in fields):
                _reject("ORACLE_IMPACT_INCOMPLETE", entity[:32])
def _audit_transcript_bounds(transcript: list) -> None:
    """Bounded-usage audit over the judge transcript: every response
    succeeded, no reply exceeds the per-reply byte cap, request count
    stays within the frozen cap. A whole-store dump would trip the byte
    cap; the harness surface offers no dump/export method at all."""

    allowed = {"session.open", "workspace.open", "exchange.import", "commit",
               "candidate.validate", "entity.version", "entity.signature",
               "revision.read"}
    requests = 0
    returned_total = 0
    for entry in transcript:
        if not isinstance(entry, dict):
            _harness_fail("transcript shape")
        direction = entry.get("direction")
        if direction == "request":
            requests += 1
            if entry.get("method") not in allowed:
                _reject("QUERY_REQUIRED_FACT_OMITTED",
                        f"unbounded method {entry.get('method')}"[:64])
        elif direction == "response":
            if entry.get("failed"):
                _reject("QUERY_REQUIRED_FACT_OMITTED", "failed call")
            try:
                returned_total += int(entry.get("returned_bytes", 0) or 0)
            except (TypeError, ValueError):
                _harness_fail("transcript bounds")
    if requests > 100:
        _reject("QUERY_REQUIRED_FACT_OMITTED", f"{requests} requests")
    if returned_total > 4 * 1024 * 1024:
        _reject("QUERY_REQUIRED_FACT_OMITTED", f"{returned_total} bytes")
