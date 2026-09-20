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


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


sys.path.insert(0, str(_repo_root()))
from bench.live import sley2_codecs  # noqa: E402
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
                return json.loads(line[len("LIVE_JUDGE_RESULT "):])
            except json.JSONDecodeError as error:
                raise JudgeHarnessError(
                    f"LIVE_SLEY2_JUDGE_INVALID: verdict: {error}") from error
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
                         scratch_ws: Path, candidate: bytes, current: set[str]) -> None:
    """Execution-kind tasks: strict corpus cases against named functions.

    Clamp-combine tasks (REPAIR, REPAIR-shaped ADVERSARY) run below/above
    predicates per the corpus triples (the S3 clamp driver pattern) with
    task-specific mismatch codes; SIG checks its caller/call records;
    CORRUPT checks the restored constant; every other execution task
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
                 versions_post: dict[str, str]) -> None:
    """Graph-kind tasks: absent roles tombstoned, expected roles present,
    non-target bindings byte-identical."""

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
    for entity, before in versions_pre.items():
        if entity in targets:
            continue
        if entity in [entities.get(role, "") for role in absent]:
            continue
        if versions_post.get(entity) != before:
            _reject("ORACLE_COLLATERAL_TOUCHED", entity[:32])


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
    ws = workdir / "ws"
    ws.mkdir(mode=0o700)
    (ws / REPO_DIR).mkdir(mode=0o700)
    try:
        shutil.copyfile(pack, ws / "base.pack")
    except OSError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: base pack: {error}") from error
    session = Session(_resolve_binary(), ws, [], seed_pack=True)

    def cleanup() -> None:
        try:
            session.close()
        finally:
            shutil.rmtree(workdir, ignore_errors=True)

    return ws / REPO_DIR, session, cleanup


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


def _judge_type_variant(session: Session, manifest: dict, scratch_ws: Path,
                        current: set[str]) -> None:
    """TYPE task: the switch migrates off Bool (four variant arms need at
    least four blocks, all required); the status constant is no longer a
    parallel Bool binding."""

    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    switch = entities.get(judge.get("switch", "switch"), "")
    status = entities.get(judge.get("status", "status"), "")
    if not switch or not status:
        _harness_fail("type entities")
    want_cases = int(judge.get("variant_cases", 4))
    status_body = _decode_body(session, scratch_ws, status, current)
    value_type = (status_body.get("value") or {}).get("value_type") or {}
    if value_type.get("variant") == "Bool":
        _reject("ORACLE_BOOL_COMPAT_FIELD", "status still Bool")
    switch_body = _decode_body(session, scratch_ws, switch, current)
    blocks = switch_body.get("blocks") or []
    if len(blocks) < want_cases:
        _reject("ORACLE_MISSING_CASE", f"{len(blocks)} blocks < {want_cases} variants")
    if not judge.get("exhaustive", True):
        _harness_fail("exhaustive spec")
    for block_id in blocks:
        block = _decode_body(session, scratch_ws, block_id, current)
        if block.get("reachability") != "Required":
            _reject("ORACLE_MISSING_CASE", f"block {block_id[:16]} not required")


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
        needs_versions = flow in ("graph", "test-entity", "type-variant") or (
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

    Returns an optional access-evidence suffix for the verdict detail
    (bounded-maintenance only); every other flow returns None."""

    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    flow = judge.get("flow", "")
    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    targets = manifest.get("targets", []) if isinstance(manifest.get("targets"), list) else []
    current = set(versions_post.values())
    suffix: str | None = None
    if flow == "execute-cases":
        _judge_execute_cases(session, manifest, corpus, task_id, scratch_ws, candidate,
                             current)
    elif flow == "graph":
        # Observation first: behavior-change negatives must report
        # before absent checks (a trial that keeps dead code and
        # changes behavior is reachable_changed, not unexpected).
        _judge_graph_observation(session, manifest, task_id, scratch_ws, task_dir,
                                 versions_post, current)
        _judge_graph(session, manifest, versions_pre, versions_post)
        _judge_reference_count(session, manifest, scratch_ws, current)
    elif flow == "test-entity":
        _judge_test_entity(session, manifest, corpus, scratch_ws, task_dir, current)
    elif flow == "stale-sequence":
        _judge_stale_sequence(session, manifest, corpus, scratch_ws, candidate, pre_tx)
    elif flow == "merge":
        _judge_merge(session, manifest, corpus, scratch_ws, workspace)
    elif flow == "perf":
        _judge_perf(session, manifest, corpus, scratch_ws, task_dir)
    elif flow == "bounded-maintenance":
        suffix = _judge_bounded(session, manifest, corpus, scratch_ws, transcript,
                                task_dir, workspace)
    elif flow == "type-variant":
        _judge_type_variant(session, manifest, scratch_ws, current)
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

def _judge_corrupt_value(session: Session, manifest: dict) -> None:
    """The corrupted constant reads the manifest-expected value post-fix."""

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
    _judge_impl_unchanged(scratch_ws, task_dir, func,
                          session.head.get("epoch", ""))
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


def _judge_impl_unchanged(scratch_ws: Path, task_dir: Path, func: str,
                          epoch: str) -> None:
    """The implementation entity's object bytes are byte-identical
    between the pristine base pack and the committed outcome (frozen
    S3 implementation_changes == 0)."""

    if not epoch:
        _harness_fail("epoch")
    repo_pre, pre_session, cleanup = _seeded_repo(task_dir)
    try:
        pre_raw = _entity_stored_bytes(repo_pre, func, pre_session.head.get("epoch", ""))
        post_raw = _entity_stored_bytes(scratch_ws / REPO_DIR, func, epoch)
    finally:
        cleanup()
    if pre_raw is None or post_raw is None:
        _harness_fail("impl objects")
    if pre_raw != post_raw:
        _reject("ORACLE_IMPL_TOUCHED", func[:32])


def _entity_stored_bytes(repo: Path, entity: str, epoch: str) -> bytes | None:
    """Raw stored bytes of one entity's object file."""

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
    permitted stale rejection with no partial second write, re-query,
    and construction of a NEW candidate that genuinely validates against
    the new base.

    Resubmitting the original candidate while changing only the outer
    request binding is NOT rebase evidence: the candidate's embedded
    base binding still names the old head, so outer-tx substitution
    alone proves nothing. The judge therefore requires a freshly
    assembled candidate against H1 with a Valid decision.
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
    if "STALE" not in code:
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
    # Genuine rebase: assemble a NEW candidate against H1 (identity
    # Replace of the guard's current body) and require Valid through the
    # correct validation shape.
    _require_rebased_candidate_valid(session, scratch_ws, manifest, head_h1)


def _require_rebased_candidate_valid(session: Session, scratch_ws: Path,
                                     manifest: dict, head_h1: str) -> None:
    """Build a fresh identity candidate against the new base and require
    a Valid decision. Proves the new base is writable, independent of
    the original bytes."""

    from bench.live import sley2_tool as _tool

    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    principal = manifest.get("principal", "")
    guard = entities.get("guard", "")
    if not guard:
        _harness_fail("stale entities")
    reply = session._raw_request("entity.version", _tool_entity_body(session, guard))
    if reply["flags"].get("failed"):
        _reject("ORACLE_REBASE_INVALID", "guard unreadable at H1")
    try:
        [decoded] = sley2_codecs.run_batch([{
            "op": "decode_response", "method": "entity.version", "body": reply["body"],
        }])
    except (sley2_codecs.CodecError, KeyError) as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: rebase read: {error}") from error
    entries = decoded["decoded"].get("entries") or []
    if len(entries) != 1:
        _reject("ORACLE_REBASE_INVALID", "guard entries at H1")
    entry = entries[0]
    ops = [{"class": "ReplaceEntityVersion", "kind": entry["kind"],
            "target": guard, "field_tag": None, "payload": entry["body"]}]
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
                 workspace: Path) -> None:
    """MERGE task: the committed outcome carries exactly the union
    semantics — the shared constant at the ours value, the theirs-only
    constant present, nothing else added or changed. Order-independence
    (commutativity) holds by construction: the expectation is derived
    per side, never in side order; determinism is rechecked by reread."""

    _ = corpus
    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    conflict = entities.get(judge.get("conflict", "constant"), "")
    if not conflict:
        _harness_fail("merge conflict entity")
    repos: dict[str, Path] = {}
    epochs: dict[str, str] = {}
    cleanups = []
    try:
        for name in ("base.pack", "ours.pack", "theirs.pack"):
            path = workspace / name
            try:
                if not path.is_file() or path.is_symlink():
                    _harness_fail(f"side {name}")
            except OSError as error:
                raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: sides: {error}") from error
            repo, side_session, cleanup = _seeded_pack(path)
            repos[name] = repo
            epochs[name] = side_session.head.get("epoch", "")
            cleanups.append(cleanup)
        post_epoch = session.head.get("epoch", "")
        base_ids = _repo_entity_ids(repos["base.pack"], epochs["base.pack"])
        post_ids = _repo_entity_ids(scratch_ws / REPO_DIR, post_epoch)
        ours_conflict = _entity_stored_bytes(
            repos["ours.pack"], conflict, epochs["ours.pack"])
        post_conflict = _entity_stored_bytes(
            scratch_ws / REPO_DIR, conflict, post_epoch)
        if ours_conflict is None or post_conflict is None:
            _harness_fail("merge conflict objects")
        if post_conflict != ours_conflict:
            _reject("ORACLE_MERGE_CONFLICT", "shared constant not at merged value")
        theirs_ids = _repo_entity_ids(repos["theirs.pack"], epochs["theirs.pack"])
        fresh_theirs = [e for e in theirs_ids if e not in base_ids]
        fresh_post = [e for e in post_ids if e not in base_ids]
        if len(fresh_theirs) != 1 or len(fresh_post) != 1:
            _reject("ORACLE_MERGE_CONFLICT",
                    f"theirs={len(fresh_theirs)} post={len(fresh_post)}")
        theirs_body = _decode_body_at(repos["theirs.pack"], fresh_theirs[0],
                                      epochs["theirs.pack"])
        post_body = _decode_fresh_body(session, scratch_ws, fresh_post[0])
        if theirs_body != post_body:
            _reject("ORACLE_MERGE_CONFLICT", "theirs-only change not preserved")
        for entity in post_ids:
            if entity in base_ids or entity == fresh_post[0]:
                continue
            if _repo_version_of(scratch_ws / REPO_DIR, post_epoch, entity) != \
                    _repo_version_of(repos["base.pack"], epochs["base.pack"], entity):
                _reject("ORACLE_MERGE_CONFLICT", f"extra {entity[:16]}")
        if _repo_entity_ids(scratch_ws / REPO_DIR, post_epoch) != post_ids:
            _reject("ORACLE_MERGE_UNSTABLE", "recompute diverged")
    finally:
        for cleanup in cleanups:
            try:
                cleanup()
            except Exception:
                continue


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


def _repo_version_of(repo: Path, epoch: str, entity: str) -> bytes | None:
    """Stored bytes of one entity in a repo dir (None when absent)."""

    return _entity_stored_bytes(repo, entity, epoch)


def _decode_fresh_body(session: Session, scratch_ws: Path, entity: str) -> dict:
    """Typed body of a fresh (single-version) entity from the repo."""

    for entry in _decode_paths(session, _store_files(scratch_ws / REPO_DIR)):
        if entry.get("entity_id") == entity:
            body = entry.get("body")
            if not isinstance(body, dict):
                _harness_fail("entity body")
            return body
    _reject("ORACLE_MISSING_TARGET", entity[:32])
    raise AssertionError("unreachable")


def _decode_body_at(repo: Path, entity: str, epoch: str) -> dict:
    """Typed body of one entity from a repo dir (not the trial repo)."""

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


def _seeded_pack(pack: Path) -> tuple[Path, object, object]:
    """Seeded throwaway session for one side pack (merge determinism)."""

    workdir = Path(tempfile.mkdtemp(prefix="sley2-merge-"))
    ws = workdir / "ws"
    ws.mkdir(mode=0o700)
    (ws / REPO_DIR).mkdir(mode=0o700)
    try:
        shutil.copyfile(pack, ws / "base.pack")
    except OSError as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: side pack: {error}") from error
    session = Session(_resolve_binary(), ws, [], seed_pack=True)

    def cleanup() -> None:
        try:
            session.close()
        finally:
            shutil.rmtree(workdir, ignore_errors=True)

    return ws / REPO_DIR, session, cleanup


def _repo_stems(repo: Path) -> set[str]:
    """Object ids of a repo dir (any layout)."""

    found = set()
    for item in _store_files(repo):
        digest = _object_id_of(item)
        if digest:
            found.add(digest)
    return found


def _judge_perf(session: Session, manifest: dict, corpus: dict, scratch_ws: Path,
                task_dir: Path) -> None:
    """PERF task: identical outputs pre (pristine base pack) vs post on
    the fixed inputs; instruction reduction at/above the frozen
    threshold."""

    _ = (session, corpus)
    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    func = entities.get("func", "")
    inputs = judge.get("fixed_inputs", [[9223372036854775800, 7]])
    threshold = judge.get("minimum_instruction_reduction_percent", 30)
    if not func:
        _harness_fail("perf entry")
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
    if instr_pre <= 0:
        _harness_fail("perf baseline")
    reduction = 100.0 * (instr_pre - instr_post) / instr_pre
    if reduction < float(threshold):
        _reject("ORACLE_PERF_UNIMPROVED", f"{reduction:.1f}% < {threshold}%")


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

    Verified (order, hashes, tool/binary/fixture binding), then derived
    across every session and phase: whole-store reads under the frozen
    definition applied to every exposed read route (inventory, side,
    and any query/refs enumeration, never only commands named
    inventory), targeted reads, operations, continuation use with
    omitted/truncated accounting, refusals, per-response byte caps, and
    the absence of any commit path.

    Missing, truncated, mismatched, or unverifiable evidence prevents
    full CONTEXT acceptance under the frozen unbounded-read code;
    semantic-only success is noted in the rejection detail, never
    mislabeled as a pass. whole_store_reads is derived, never
    defaulted: an empty but valid chain derives zero only because every
    call is enumerated. Unknowns are never reported as zero: incomplete
    evidence rejects instead of returning counts.
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
    continuations = 0
    omitted = 0
    truncated = 0
    refusals = 0
    max_response = 0
    calls = 0
    binary_ids: set[str] = set()
    try:
        judge_binary = hashlib.sha256(_resolve_binary().read_bytes()).hexdigest()
    except Exception:
        judge_binary = ""
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
            continuations += int(summary.get("continuations", 0) or 0)
            omitted += int(summary.get("omitted", 0) or 0)
            refused_calls = int(summary.get("failed", 0) or 0)
            returned = int(summary.get("returned_bytes", 0) or 0)
            report_bytes = int(entry.get("report_bytes", 0) or 0)
        except (TypeError, ValueError):
            _reject("QUERY_REQUIRED_FACT_OMITTED", "agent summary shape; semantics held")
        refusals += refused_calls
        max_response = max(max_response, returned, report_bytes)
        if returned > MAX_RESPONSE_BYTES or report_bytes > MAX_RESPONSE_BYTES:
            _reject("QUERY_REQUIRED_FACT_OMITTED",
                    f"response over bound {max(max_response, returned, report_bytes)}; semantics held")
        session = entry.get("session") if isinstance(entry.get("session"), list) else []
        for item in session:
            if isinstance(item, dict) and item.get("direction") == "request":
                calls += 1
                if item.get("method") == "commit":
                    _reject("QUERY_REQUIRED_FACT_OMITTED",
                            "agent commit path; semantics held")
                # Frozen whole-store-read definition applied to every
                # exposed read route, not only commands named inventory:
                # any query/refs enumeration reads served state beyond a
                # targeted entity version/signature.
                if item.get("method") in ("query.root", "query.restricted",
                                          "query.continue", "refs.list"):
                    whole_store += 1
        if summary.get("truncated"):
            truncated += 1
    if len(binary_ids) != 1 or (judge_binary and next(iter(binary_ids)) != judge_binary):
        _reject("QUERY_REQUIRED_FACT_OMITTED",
                "tool/binary identity mismatch; semantics held")
    if omitted > 0 or truncated > 0:
        _reject("QUERY_REQUIRED_FACT_OMITTED",
                f"omitted={omitted} truncated={truncated}; semantics held")
    if whole_store > 0:
        _reject("QUERY_REQUIRED_FACT_OMITTED",
                f"whole_store_reads={whole_store}; semantics held")
    _verify_candidate_transitions(entries)
    _verify_final_linkage(trial_ws, entries)
    return {"whole_store_reads": whole_store, "targeted_reads": len(targeted),
            "operations": operations, "continuations": continuations,
            "omitted": omitted, "truncated": truncated, "refusals": refusals,
            "max_response_bytes": max_response, "agent_requests": calls,
            "invocations": len(entries)}


def _is_hash64(value: object) -> bool:
    return (isinstance(value, str) and len(value) == 64
            and all(ch in "0123456789abcdef" for ch in value))


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


def _judge_bounded(session: Session, manifest: dict, corpus: dict, scratch_ws: Path,
                   transcript: list, task_dir: Path, trial_ws: Path) -> str:
    """CONTEXT task: minimum entity count, required field present, impact
    closure coherent, collateral clean (generic path already ran), and the
    judge's own transcript shows bounded usage only (no whole-store
    dump, no failed calls, per-reply byte caps held)."""

    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    minimum = int(judge.get("minimum_entities", 10000))
    count = len(_repo_stems(scratch_ws / REPO_DIR))
    if count < minimum:
        _reject("ORACLE_BELOW_MINIMUM", f"{count} < {minimum}")
    typedef = entities.get("typedef", "")
    if not typedef:
        _harness_fail("typedef entity")
    reply = session._raw_request("entity.version", _tool_entity_body(session, typedef))
    if reply["flags"].get("failed"):
        _reject("ORACLE_TYPEDEF_UNREADABLE", typedef[:32])
    add = judge.get("add_member", {}) if isinstance(judge.get("add_member"), dict) else {}
    if add:
        _judge_added_member(session, scratch_ws, typedef, add)
    impact = judge.get("impact", []) if isinstance(judge.get("impact"), list) else []
    if impact and add:
        _judge_impact_consts(session, scratch_ws, entities, impact, add)
    _audit_transcript_bounds(transcript)
    access = _audit_agent_access(task_dir, trial_ws)
    _ = corpus
    return ("whole_store_reads={whole_store_reads} targeted_reads={targeted_reads} "
            "operations={operations} continuations={continuations} refusals={refusals} "
            "max_response_bytes={max_response_bytes}").format(**access)


def _judge_impact_consts(session: Session, scratch_ws: Path, entities: dict,
                         impact: list, add: dict) -> None:
    """Every impact-closure record constant carries the added member
    (the closure update is complete, not just the typedef)."""

    member = add.get("member", "")
    for role in impact:
        if not isinstance(role, str):
            _harness_fail("impact role")
        target = entities.get(role, "")
        if not target:
            _harness_fail("impact entity")
        reply = session._raw_request("entity.version", _tool_entity_body(session, target))
        if reply["flags"].get("failed"):
            _reject("ORACLE_IMPACT_INCOMPLETE", role[:32])
        try:
            [decoded] = sley2_codecs.run_batch([{
                "op": "decode_response", "method": "entity.version",
                "body": reply["body"],
            }])
        except (sley2_codecs.CodecError, KeyError) as error:
            raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: impact: {error}") from error
        entries = decoded["decoded"].get("entries") or []
        if len(entries) != 1:
            _reject("ORACLE_IMPACT_INCOMPLETE", role[:32])
        path = _object_path(scratch_ws / REPO_DIR, entries[0].get("object_id", ""))
        if path is None:
            _reject("ORACLE_IMPACT_INCOMPLETE", role[:32])
        try:
            [obj] = sley2_codecs.run_batch([{
                "op": "decode_object", "stored": path.read_bytes().hex(),
                "epoch": session.head["epoch"],
            }])
        except (OSError, sley2_codecs.CodecError, KeyError) as error:
            raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: impact: {error}") from error
        body = obj["decoded"].get("body")
        if not isinstance(body, dict):
            _harness_fail("impact body")
        value = body.get("value") or {}
        data = value.get("data") or {} if isinstance(value, dict) else {}
        record = data.get("value") or {} if isinstance(data, dict) else {}
        fields = record.get("fields") if isinstance(record, dict) else None
        if not isinstance(fields, list) or not any(
                isinstance(f, dict) and f.get("member_id") == member for f in fields):
            _reject("ORACLE_IMPACT_INCOMPLETE", role[:32])


def _judge_added_member(session: Session, scratch_ws: Path, typedef: str,
                        add: dict) -> None:
    """The required record field is present on the typedef's CURRENT
    version with the frozen member id and value type."""

    member = add.get("member", "")
    want_type = add.get("type", "")
    if not member or not want_type:
        _harness_fail("add_member spec")
    reply = session._raw_request("entity.version", _tool_entity_body(session, typedef))
    if reply["flags"].get("failed"):
        _reject("ORACLE_TYPEDEF_UNREADABLE", typedef[:32])
    try:
        [decoded] = sley2_codecs.run_batch([{
            "op": "decode_response", "method": "entity.version", "body": reply["body"],
        }])
    except (sley2_codecs.CodecError, KeyError) as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: typedef: {error}") from error
    entries = decoded["decoded"].get("entries") or []
    if len(entries) != 1:
        _reject("ORACLE_TYPEDEF_UNREADABLE", typedef[:32])
    object_id = entries[0].get("object_id", "")
    path = _object_path(scratch_ws / REPO_DIR, object_id)
    if path is None:
        _reject("ORACLE_TYPEDEF_UNREADABLE", typedef[:32])
    try:
        [obj] = sley2_codecs.run_batch([{
            "op": "decode_object", "stored": path.read_bytes().hex(),
            "epoch": session.head["epoch"],
        }])
    except (OSError, sley2_codecs.CodecError, KeyError) as error:
        raise JudgeHarnessError(f"LIVE_SLEY2_JUDGE_INVALID: typedef: {error}") from error
    body = obj["decoded"].get("body")
    if not isinstance(body, dict):
        _harness_fail("typedef body")
    form = body.get("form") or {}
    fields = form.get("value") if isinstance(form, dict) else None
    for field in fields if isinstance(fields, list) else []:
        if not isinstance(field, dict):
            continue
        if field.get("member_id") == member:
            value_type = field.get("value_type") or {}
            if isinstance(value_type, dict) and value_type.get("variant") == want_type:
                return
            _reject("ORACLE_MEMBER_MISMATCH", want_type[:32])
    _reject("ORACLE_MEMBER_MISSING", member[:32])


def _audit_transcript_bounds(transcript: list) -> None:
    """Bounded-usage audit over the judge transcript: every response
    succeeded, no reply exceeds the per-reply byte cap, request count
    stays within the frozen cap. A whole-store dump would trip the byte
    cap; the harness surface offers no dump/export method at all."""

    allowed = {"session.open", "workspace.open", "exchange.import", "commit",
               "candidate.validate", "entity.version", "entity.signature"}
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
