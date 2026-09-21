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
    """TYPE task: complete JobState migration (frozen corpus S2B-TYPE-001).

    Predicate provenance (three tiers — alternatives reject ONLY where
    the governing tier requires it):

    CORPUS-FROZEN (bench/corpus/v1/tasks.json S2B-TYPE-001, immutable):
    - no boolean status binding remains (status/param/switch-result
      never Bool; no parallel bool compat field);
    - all four cases exhaustively handled: VariantSwitch whose Member
      keys cover exactly the typedef members, every member targeting a
      Required arm (a Trap arm does not handle its case), no implicit
      default;
    - Failed carries an explicit SInt error code: the typedef holds
      exactly one Some(SInt) member, and any live Failed value carries
      a Some(SInt) payload (forbidden "null error" — a Failed member
      with a null payload rejects);
    - constructors, switches, and tests updated (closure roles below;
      the base holds no Test entities so test update is established
      through this verification, documented);
    - serialized values deterministic (explicit fixed SInt values; live
      served-state reads, never bare-record parse alone).

    FIXTURE-SPECIFIED (task_manifest.json v2 closure on the work
    branch, review-gated before main adoption):
    - closure roles status/switch/param/switch_entry/switch_leaf (the
      block count follows from the roles, not from a corpus number);
    - switch result SInt with SInt-constant leaves (the fixture's
      concrete observable; the corpus requires non-Bool + exhaustive,
      the fixture pins SInt);
    - Failed arm forwards CasePayload and the Failed leaf returns its
      Block param (this is how the fixture requires the error code to
      be handled rather than dropped).

    RETIRED witness choices (0b26c39c proved them; this correction
    retires them as restrictions — historical logs preserved):
    - literal code 7: the corpus requires an EXPLICIT code, not 7.
      Failed(7) was the positive witness's choice (matching the S3
      test-vector literal). Failed(8) with an explicit SInt payload
      satisfies the corpus and accepts. The wrong-code negative now
      demonstrates loss/corruption of the required payload (Failed
      member with null payload → ORACLE_FAILED_CODE), not
      disagreement with the witness literal.
    - status-is-Failed: the corpus pins no status value. Any typedef
      member value accepts; the explicit-payload rule applies when the
      value IS the Failed member.
    - distinct leaf constants: the corpus requires deterministic, not
      distinct. Leaves sharing an SInt constant accept.

    Structural vs behavioral (kept distinct):
    - structural (parse-level): typedef shape/member counts, sorted
      Member-key coverage, Required/no-Trap arms, closure membership.
    - behavioral (execution machinery): the submitted candidate passes
      server-side candidate.validate (every arm typechecks at
      validation time, including CasePayload forwarding); the judged
      status/param/switch bodies are read LIVE from the served
      post-commit head through entity.version (a live tagged state,
      not record bytes); SInt round-trip determinism is executed by
      the frozen S3 suite (real VM: u8 codes + Failed payload
      round-trip, re-run with the re-proofs).
    """

    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    entities = manifest.get("entities", {}) if isinstance(manifest.get("entities"), dict) else {}
    switch = entities.get(judge.get("switch", "switch"), "")
    status = entities.get(judge.get("status", "status"), "")
    param_expect = entities.get("switch_param", entities.get("param", ""))
    # Back-compat: original manifest named only switch/status; the
    # corrected closure exposes switch_entry/switch_leaf/param via
    # targets. Resolve param through live targets when not named.
    if not switch or not status:
        _harness_fail("type entities")
    want_cases = int(judge.get("variant_cases", 4))
    # --- status constructor ---
    status_body = _decode_body(session, scratch_ws, status, current)
    value = status_body.get("value") or {}
    value_type = value.get("value_type") or {}
    if value_type.get("variant") == "Bool":
        _reject("ORACLE_BOOL_COMPAT_FIELD", "status still Bool")
    if value_type.get("variant") != "Named":
        _reject("ORACLE_TYPE_NOT_MIGRATED", "status not Named")
    named = value_type.get("value") or {}
    typedef_id = named.get("definition", "")
    if not typedef_id or not isinstance(typedef_id, str):
        _reject("ORACLE_TYPE_NOT_MIGRATED", "status typedef")
    # --- typedef shape: exactly 4 members, one Some SInt, three None ---
    typedef_body = _decode_body(session, scratch_ws, typedef_id, current)
    form = (typedef_body.get("form") or {}).get("value") if isinstance(
        typedef_body.get("form"), dict) else None
    if not isinstance(form, list) or len(form) != want_cases:
        _reject("ORACLE_TYPE_NOT_MIGRATED",
                f"typedef members {len(form) if isinstance(form, list) else '?'} != 4")
    some_members: list[str] = []
    none_members: list[str] = []
    for member in form:
        if not isinstance(member, dict):
            _reject("ORACLE_TYPE_NOT_MIGRATED", "typedef member shape")
        mid = member.get("member_id", "")
        payload = member.get("payload_type") or {}
        variant = payload.get("variant")
        if variant == "Some":
            inner = payload.get("value") or {}
            if inner.get("variant") != "SInt":
                _reject("ORACLE_TYPE_NOT_MIGRATED", "Failed payload not SInt")
            some_members.append(str(mid))
        elif variant == "None":
            none_members.append(str(mid))
        else:
            _reject("ORACLE_TYPE_NOT_MIGRATED", "typedef payload shape")
    if len(some_members) != 1 or len(none_members) != want_cases - 1:
        _reject("ORACLE_TYPE_NOT_MIGRATED", "need 3xNone + 1xSome(SInt)")
    failed_member = some_members[0]
    member_ids = {str(m.get("member_id", "")) for m in form}
    # --- status value: a live typedef member; explicit code required
    # iff the value IS the Failed member (corpus: explicit code + no
    # null error; the corpus pins no literal and no status value) ---
    data = value.get("data") or {}
    if data.get("variant") != "Variant":
        _reject("ORACLE_TYPE_NOT_MIGRATED", "status data not Variant")
    variant = data.get("value") or {}
    if variant.get("definition") != typedef_id:
        _reject("ORACLE_TYPE_NOT_MIGRATED", "status typedef mismatch")
    status_member = variant.get("member_id", "")
    if status_member not in member_ids:
        _reject("ORACLE_TYPE_NOT_MIGRATED", "status member outside typedef")
    payload = variant.get("payload") or {}
    if status_member == failed_member:
        if payload.get("variant") != "Some":
            _reject("ORACLE_FAILED_CODE", "Failed null payload")
        inner = payload.get("value") or {}
        inner_data = inner.get("data") or {}
        if inner_data.get("variant") != "SInt" or not isinstance(
                inner_data.get("value"), int):
            _reject("ORACLE_FAILED_CODE", "Failed code not explicit SInt")
    elif payload.get("variant") != "None":
        _reject("ORACLE_TYPE_NOT_MIGRATED", "unit member carries payload")
    # --- param migrates to Named typedef (no Bool compat) ---
    targets = manifest.get("targets", []) if isinstance(
        manifest.get("targets"), list) else []
    # Param entity: prefer named manifest role, else discover the switch's
    # sole parameter through the function body (live binding, not guess).
    switch_body_pre = _decode_body(session, scratch_ws, switch, current)
    params = switch_body_pre.get("parameters") or []
    param_id = ""
    if param_expect and isinstance(param_expect, str):
        param_id = param_expect
    elif len(params) == 1 and isinstance(params[0], str):
        param_id = params[0]
    else:
        _harness_fail("type param")
    param_body = _decode_body(session, scratch_ws, param_id, current)
    param_type = param_body.get("value_type") or {}
    if param_type.get("variant") == "Bool":
        _reject("ORACLE_BOOL_COMPAT_FIELD", "param still Bool")
    if param_type.get("variant") != "Named" or (
            param_type.get("value") or {}).get("definition") != typedef_id:
        _reject("ORACLE_TYPE_NOT_MIGRATED", "param not Named typedef")
    # --- switch function ---
    switch_body = switch_body_pre
    result_type = switch_body.get("result_type") or {}
    if result_type.get("variant") == "Bool":
        _reject("ORACLE_BOOL_COMPAT_FIELD", "switch result still Bool")
    if result_type.get("variant") != "SInt":
        _reject("ORACLE_TYPE_NOT_MIGRATED", "switch result not SInt")
    blocks = switch_body.get("blocks") or []
    if len(blocks) < want_cases:
        _reject("ORACLE_MISSING_CASE", f"{len(blocks)} blocks < {want_cases} variants")
    if not judge.get("exhaustive", True):
        _harness_fail("exhaustive spec")
    entry_id = switch_body.get("entry_block", "")
    if not entry_id:
        _harness_fail("switch entry")
    # All blocks Required, no Trap terminators.
    block_bodies: dict[str, dict] = {}
    for block_id in blocks:
        block = _decode_body(session, scratch_ws, block_id, current)
        block_bodies[block_id] = block
        if block.get("reachability") != "Required":
            _reject("ORACLE_MISSING_CASE", f"block {block_id[:16]} not required")
        terminator = block.get("terminator") or {}
        if terminator.get("variant") == "Trap":
            _reject("ORACLE_TRAP_ARM", f"block {block_id[:16]} trap")
    # Entry must be an exhaustive VariantSwitch (not the frozen Bool
    # CondBranch). Typedef-only migration keeps CondBranch and rejects
    # here, never as acceptance.
    entry = block_bodies.get(entry_id)
    if entry is None:
        _harness_fail("entry body")
    terminator = entry.get("terminator") or {}
    if terminator.get("variant") != "VariantSwitch":
        _reject("ORACLE_SWITCH_NOT_MIGRATED", "entry not VariantSwitch")
    sw = terminator.get("value") or {}
    selector = sw.get("value") or {}
    if selector.get("variant") != "Parameter" or selector.get("value") != param_id:
        _reject("ORACLE_SWITCH_NOT_MIGRATED", "switch not on param")
    cases = sw.get("cases") or []
    if len(cases) != want_cases:
        _reject("ORACLE_MISSING_CASE", f"{len(cases)} cases != 4")
    # Case keys must be Member keys covering exactly the typedef members,
    # strictly sorted (production SwitchCases rule).
    keys = []
    for case in cases:
        key = (case.get("case_key") or {}) if isinstance(case, dict) else {}
        if key.get("variant") != "Member":
            _reject("ORACLE_MISSING_CASE", "non-Member case")
        keys.append(str(key.get("value", "")))
    if keys != sorted(keys):
        _reject("ORACLE_MISSING_CASE", "cases not sorted")
    if set(keys) != {str(m.get("member_id", "")) for m in form}:
        _reject("ORACLE_MISSING_CASE", "cases != typedef members")
    # Every function block is entry or a case target (no unreachable
    # arms, no hidden trap blocks). Every target is a listed block.
    targets_of_switch = set()
    failed_uses_payload = False
    for case in cases:
        edge = case.get("edge") or {}
        target = edge.get("target", "")
        args = edge.get("arguments") or []
        if target not in block_bodies:
            _reject("ORACLE_MISSING_CASE", f"case target {str(target)[:16]} not in blocks")
        targets_of_switch.add(target)
        key_val = str(((case.get("case_key") or {}).get("value", "")))
        if key_val == failed_member:
            # Failed arm must forward the explicit payload.
            if not any(isinstance(a, dict) and a.get("variant") == "CasePayload"
                       for a in args):
                _reject("ORACLE_FAILED_CODE", "Failed arm drops payload")
            failed_uses_payload = True
    if not failed_uses_payload:
        _reject("ORACLE_FAILED_CODE", "no Failed payload arm")
    if set(blocks) != targets_of_switch | {entry_id}:
        # Entry may also be a case target (self-loop designs); allow
        # entry in targets but forbid any block outside entry+targets.
        if not set(blocks).issubset(targets_of_switch | {entry_id}):
            _reject("ORACLE_MISSING_CASE", "unreachable block present")
        if not targets_of_switch.issubset(set(blocks)):
            _reject("ORACLE_MISSING_CASE", "case target outside blocks")
    # Leaf returns: Failed leaf returns its Block param (SInt payload);
    # other leaves return SInt constants via ConstantRef (explicit,
    # deterministic; distinctness was a retired witness choice — shared
    # constants accept). Trap already rejected above.
    seen_const_values: list[int] = []
    for case in cases:
        edge = case.get("edge") or {}
        target = edge.get("target", "")
        leaf = block_bodies[target]
        # Entry-as-target (self-loop) cannot be a returning leaf; such
        # designs reject as non-exhaustive handling.
        if target == entry_id:
            _reject("ORACLE_MISSING_CASE", "entry self-loop arm")
        term = leaf.get("terminator") or {}
        if term.get("variant") != "Return":
            _reject("ORACLE_MISSING_CASE", f"leaf {target[:16]} not Return")
        ret = (term.get("value") or {}).get("value") or {}
        key_val = str(((case.get("case_key") or {}).get("value", "")))
        if key_val == failed_member:
            if ret.get("variant") != "Parameter":
                _reject("ORACLE_FAILED_CODE", "Failed leaf not param return")
            # Param must be a Block param of this leaf with SInt type.
            leaf_params = leaf.get("parameters") or []
            if ret.get("value") not in leaf_params:
                _reject("ORACLE_FAILED_CODE", "Failed return not leaf param")
            pb = _decode_body(session, scratch_ws, ret.get("value"), current)
            if pb.get("role") != "Block":
                _reject("ORACLE_FAILED_CODE", "Failed param role")
            ptype = pb.get("value_type") or {}
            if ptype.get("variant") != "SInt":
                _reject("ORACLE_FAILED_CODE", "Failed param not SInt")
        else:
            if ret.get("variant") != "OperationResult":
                _reject("ORACLE_MISSING_CASE", f"leaf {target[:16]} not op return")
            op_ref = ret.get("value") or {}
            op_id = op_ref.get("operation", "")
            if not op_id:
                _reject("ORACLE_MISSING_CASE", "leaf op ref")
            op_body = _decode_body(session, scratch_ws, op_id, current)
            if op_body.get("opcode") != 1:
                _reject("ORACLE_MISSING_CASE", "leaf op not ConstantRef")
            imm = op_body.get("immediate") or {}
            if imm.get("variant") != "Entity":
                _reject("ORACLE_MISSING_CASE", "leaf const ref")
            const_body = _decode_body(session, scratch_ws, imm.get("value", ""), current)
            const_val = (const_body.get("value") or {})
            const_data = const_val.get("data") or {}
            if const_data.get("variant") != "SInt":
                _reject("ORACLE_MISSING_CASE", "leaf const not SInt")
            seen_const_values.append(int(const_data.get("value")))
    _ = targets
    _ = seen_const_values


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
                             current, task_dir)
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
        _judge_merge(session, manifest, corpus, scratch_ws, workspace, candidate)
    elif flow == "perf":
        _judge_perf(session, manifest, corpus, scratch_ws, task_dir)
    elif flow == "bounded-maintenance":
        suffix = _judge_bounded(session, manifest, corpus, scratch_ws, transcript,
                                task_dir, workspace)
    elif flow == "create":
        _judge_create(session, manifest, corpus, scratch_ws, current)
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

def _judge_corrupt_exchange(task_dir: Path) -> None:
    """Exchange-pack corruption and rejection path (CORRUPT acceptance
    evidence): a bit-flipped pack must fail import with the exact
    frozen digest symbol, and the destination ref must not move.

    Two independent single-bit corruptions (middle, last byte) both
    map to EXCHANGE_DIGEST_MISMATCH (frozen S3 s3_g2_corrupt parity
    for the exchange layer; the README-normative PACK_DIGEST_MISMATCH
    lives one layer up, on repository-bundle import, which the trial
    surface never drives). The destination head transaction and live
    object count are identical before and after each rejected import.
    The constant-value restoration checked beside this is a separate
    smoke test, never this task's acceptance evidence."""

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
        shutil.rmtree(workdir, ignore_errors=True)


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


def _create_run(repo: Path, func: str, pairs: list) -> list:
    """Drive one function on SInt64 input pairs through the native driver."""

    cases = [{"inputs": [{"type": "SInt", "bits": 64, "value": value}
                         for value in pair]} for pair in pairs]
    results = _run_driver(repo, func, cases).get("cases")
    if not isinstance(results, list) or len(results) != len(pairs):
        _harness_fail("create case shape")
    return results


def _create_callees(session: Session, scratch_ws: Path,
                    current: set[str]) -> dict[str, set[str]]:
    """Caller -> CallDirect callees over CURRENT operations (owner
    bindings; historical ops excluded). Block ownership resolves
    through live bindings, never file order."""

    repo = scratch_ws / REPO_DIR
    callees: dict[str, set[str]] = {}
    for entry in _decode_paths(session, _current_paths(scratch_ws, current)):
        try:
            if entry.get("kind") != 8:
                continue
            body = entry.get("body")
            if not isinstance(body, dict) or body.get("opcode") != 112:
                continue
            target = _immediate_function(body.get("immediate"))
            block = body.get("block", "")
            if not target or not block:
                continue
            block_body = _decode_bound_body(session, repo, str(block))
            caller = block_body.get("function", "")
            if caller:
                callees.setdefault(str(caller), set()).add(str(target))
        except (JudgeRejection, JudgeHarnessError):
            raise
        except (AttributeError, TypeError, KeyError):
            continue
    return callees


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


def _reject_ceil_near_miss(candidates: list, checks: list) -> None:
    """Frozen ceiling-division negative (S3 round_up_tax parity): a
    candidate computing ceil(x/y) instead of the required floor on
    every check row is ORACLE_WRONG_CENTS, not a generic mismatch.
    `candidates` holds (entity, produced-or-None list) in check order;
    `checks` are the frozen [x, y, floor] rows."""

    try:
        ceil = []
        floors = []
        for row in checks:
            num, den, floor = int(row[0]), int(row[1]), int(row[2])
            ceil.append((num + den - 1) // den if den > 0 else None)
            floors.append(floor)
    except (IndexError, TypeError, ValueError):
        return
    for _, produced in candidates:
        if produced == ceil and produced != floors:
            _reject("ORACLE_WRONG_CENTS",
                    f"ceiling division {produced} contradicts floor"[:120])


def _judge_create(session: Session, manifest: dict, corpus: dict, scratch_ws: Path,
                  current: set[str]) -> None:
    """CREATE task: a blank-started program authored through the trial
    surface. Roles are discovered behaviorally (executor, not names):
    three checked scalar primitives matching the frozen checks, overflow
    signaling checked arithmetic, and a wiring entry calling all three.

    Strict corpus cases are covered by observed executions: empty (0,0
    primitives compose to 0), one-line (observed subtotal/tax/total
    compose to 2681, cross-checked against observed total() and the
    corpus expectation), overflow (observed Err Arithmetic/1).
    Execution-shape failures map to the frozen negative codes: an
    overflow case returning a value is ORACLE_UNCHECKED_ARITHMETIC
    (specified rule; provably unreachable with the checked-only opcode
    set — no authorable primitive matches its value checks while
    returning Ok on overflow); a ceiling-division tax triple is
    ORACLE_WRONG_CENTS via near-miss classification; anything else that
    matches no role is ORACLE_CREATE_MISMATCH.

    The submitted wiring entry itself must execute natively on the
    frozen one-line intermediates (see below): the program's own
    end-to-end decoded result, not only its primitives', decides the
    corpus total. No acceptance exists without that execution
    evidence: unmappable entries reject (ORACLE_CREATE_UNMAPPED),
    unexecutable ones reject as an explicit readiness status
    (ORACLE_CREATE_UNEXECUTABLE), and only the correctly decoded
    expected value accepts.
    """

    _ = corpus
    judge = manifest.get("judge", {}) if isinstance(manifest.get("judge"), dict) else {}
    spec = judge.get("primitives", {}) if isinstance(judge.get("primitives"), dict) else {}
    if not spec:
        _harness_fail("create primitives")
    repo = scratch_ws / REPO_DIR
    functions = _create_functions(session, scratch_ws, current)
    if not functions:
        _reject("ORACLE_CREATE_INCOMPLETE", "no live functions")

    def observations(func: str, pairs: list) -> list:
        return _create_run(repo, func, pairs)

    def want_ok(want: int) -> dict:
        return _driver_want("Ok", want)

    roles: dict[str, str] = {}
    seen: dict[str, dict[tuple, int]] = {}
    near: dict[str, list[tuple[str, list]]] = {}
    # Deterministic candidate order (entity-sorted): readdir order must
    # never decide role labels. Behaviorally tied roles (frozen
    # subtotal/tax_mul are both MUL by spec) are disambiguated below
    # by cross-checks, never by scan order.
    for role, cfg in spec.items():
        if not isinstance(cfg, dict):
            _harness_fail("create primitive spec")
        params = int(cfg.get("params", 2))
        checks = cfg.get("checks", [])
        match = None
        for entity, body in sorted(functions):
            if entity in roles.values():
                continue
            parameters = body.get("parameters")
            if not isinstance(parameters, list) or len(parameters) != params:
                continue
            pairs = [tuple(c[:params]) for c in checks]
            wants = [c[params] for c in checks]
            try:
                results = observations(entity, pairs)
            except JudgeHarnessError:
                raise
            except (JudgeRejection, Exception):
                continue
            ok = True
            values: dict[tuple, int] = {}
            produced: list = []
            for result, want, pair in zip(results, wants, pairs):
                if not result.get("ok"):
                    ok = False
                    produced.append(None)
                    continue
                value = result.get("value") or {}
                try:
                    got = int(value["Result"]["Ok"]["SInt"])
                except (KeyError, TypeError, ValueError):
                    ok = False
                    produced.append(None)
                    continue
                produced.append(got)
                if value != want_ok(want):
                    ok = False
                    continue
                values[pair] = want
            if ok:
                match = entity
                seen[str(role)] = values
                break
            near.setdefault(str(role), []).append((entity, produced))
        if match is None:
            ceiling = judge.get("ceiling_roles", [])
            if isinstance(ceiling, list) and str(role) in ceiling:
                _reject_ceil_near_miss(near.get(str(role), []), checks)
            _reject("ORACLE_CREATE_MISMATCH", f"no primitive behaves as {role}"[:64])
        roles[str(role)] = match
    if len(set(roles.values())) != len(roles):
        _reject("ORACLE_CREATE_MISMATCH", "primitives not distinct")
    # Composition identity over OBSERVED values (one-line corpus case):
    # the frozen composition spec names the input pairs per role; the
    # host only threads already-observed integers through the invoice
    # shape (subtotal; product; quotient; sum) and checks the corpus
    # total. Every cent below was produced by native execution above.
    composition = judge.get("composition", {})
    if not isinstance(composition, dict):
        _harness_fail("create composition spec")
    try:
        sub = seen["subtotal"][tuple(composition["subtotal"])]
        product = seen["tax_mul"][tuple(composition["tax_mul"])]
        tax = seen["tax_div"][tuple(composition["tax_div"])]
        total = seen["total"][tuple(composition["total"])]
        want_total = int(composition["result_cents"])
    except (KeyError, TypeError, ValueError):
        _harness_fail("create observed values")
    if total != want_total:
        _reject("ORACLE_CREATE_MISMATCH", "corpus one-line total")
    if sub + tax != total:
        _reject("ORACLE_CREATE_MISMATCH",
                f"composition {sub}+{tax}!={total}"[:120])
    if (sub, product, tax) != (2500, 1812500, 181):
        _reject("ORACLE_CREATE_MISMATCH", "corpus one-line values")

    def require_overflow(spec_key: str, code: str) -> None:
        cfg = judge.get(spec_key, {})
        if not isinstance(cfg, dict):
            return
        primitive = cfg.get("primitive", "")
        func = roles.get(primitive, "")
        if not func:
            _harness_fail("create overflow role")
        inputs = cfg.get("inputs", [])
        results = observations(func, [tuple(inputs)])
        _check_overflow_result(results[0], cfg, code)

    require_overflow("overflow", "ORACLE_UNCHECKED_ARITHMETIC")
    require_overflow("overflow_add", "ORACLE_UNCHECKED_ARITHMETIC")

    wiring = judge.get("wiring", {}) if isinstance(judge.get("wiring"), dict) else {}
    need = wiring.get("entry_calls", [])
    callees = _create_callees(session, scratch_ws, current)
    primitives = set(roles.values())
    entry = None
    entry_body: dict = {}
    for entity, body in functions:
        if entity in primitives:
            continue
        if set(need and [roles[r] for r in need if r in roles]) <= callees.get(entity, set()):
            result_type = body.get("result_type") or {}
            if isinstance(result_type, dict) and result_type.get("variant") == "Result":
                entry = entity
                entry_body = body
                break
    if entry is None and need:
        _reject("ORACLE_CREATE_UNWIRED", "no entry calls all primitives")
    # Native end-to-end entry execution (the program's own invoice
    # result on the frozen one-line intermediates): each entry CALL
    # operand position carries its role's frozen input value (shared
    # params must agree — a conflict means the wiring cannot compute
    # the corpus total); the full param vector then executes natively
    # and its decoded value must equal Ok(result_cents).
    # Fail-closed: unmappable shapes reject as ORACLE_CREATE_UNMAPPED,
    # unexecutable mappings reject as ORACLE_CREATE_UNEXECUTABLE
    # (readiness status, never success, never candidate blame), and
    # only a correctly decoded expected value accepts. See
    # `_judge_create_entry` for the classification.
    if entry is not None and need:
        ties = _create_role_ties(roles, spec, observations, want_ok)
        _judge_create_entry(entry, entry_body, roles, ties, composition,
                            want_total, want_ok(want_total), session,
                            scratch_ws, current, observations)


def _create_role_ties(roles: dict, spec: dict, observations: object,
                      want_ok: object) -> list[tuple[str, str]]:
    """Behaviorally interchangeable role pairs (frozen subtotal/tax_mul
    are both MUL by spec: each matched function satisfies the other's
    checks too). Entry-mapping tries each labeling; the program's own
    end-to-end result decides, never scan order. Driver-verified."""

    ties: list[tuple[str, str]] = []
    names = sorted(roles)
    for index, first in enumerate(names):
        for second in names[index + 1:]:
            cfg_first = spec.get(first, {})
            cfg_second = spec.get(second, {})
            if (not isinstance(cfg_first, dict)
                    or not isinstance(cfg_second, dict)
                    or cfg_first.get("opcode") != cfg_second.get("opcode")):
                continue
            try:
                cross_second = observations(
                    roles[first],
                    [tuple(c[:2]) for c in cfg_second.get("checks", [])])
                cross_first = observations(
                    roles[second],
                    [tuple(c[:2]) for c in cfg_first.get("checks", [])])
            except (JudgeRejection, JudgeHarnessError):
                raise
            except Exception:
                continue
            wants_second = [c[2] for c in cfg_second.get("checks", [])]
            wants_first = [c[2] for c in cfg_first.get("checks", [])]
            if (len(cross_second) == len(wants_second)
                    and len(cross_first) == len(wants_first)
                    and all(isinstance(r, dict) and r.get("ok")
                            and r.get("value") == want_ok(w)
                            for r, w in zip(cross_second, wants_second))
                    and all(isinstance(r, dict) and r.get("ok")
                            and r.get("value") == want_ok(w)
                            for r, w in zip(cross_first, wants_first))):
                ties.append((first, second))
    return ties


def _judge_create_entry(entry: str, entry_body: dict, roles: dict,
                        ties: list[tuple[str, str]], composition: dict,
                        want_total: int, want: dict,
                        session: Session, scratch_ws: Path,
                        current: set[str],
                        observations: object) -> None:
    """Execute the submitted wiring entry natively on the frozen
    one-line intermediates (see `_judge_create`). Role-labeled
    assignment only: every CALL operand must be a Parameter whose
    positions match the role's frozen input pair.

    Behaviorally tied roles (frozen subtotal/tax_mul are both MUL)
    admit interchangeable labelings: every tied labeling whose
    demands are consistent executes, and the program's own decoded
    Ok(2681) decides — never scan order, never a conventional label.

    Fail-closed classification (no silent pass):
    - malformed frozen composition spec → harness failure (judge-side
      config, never candidate blame);
    - undecodable entry/block/op bodies → harness failure (required
      evidence unverifiable, never candidate blame);
    - entry shape the helper cannot map to input-consuming execution
      (no params, non-112 calls, non-Parameter operands, no mapped
      calls) → ORACLE_CREATE_UNMAPPED rejection (the program as
      submitted does not expose the required input-consuming
      wiring; a valid alternative that DOES map is never rejected
      for structural differences);
    - mapped but the engine cannot execute → ORACLE_CREATE_UNEXECUTABLE
      rejection (explicit non-accepting readiness status: production
      limitation, not candidate wrongness, never reported as success);
    - executed with a wrong decoded value → ORACLE_CREATE_MISMATCH;
    - executed with the correctly decoded expected value → accept.
    The driver result envelope is validated and only its decoded
    value is compared to the expected payload.
    """

    role_of = {entity: role for role, entity in roles.items()}
    frozen_inputs: dict[str, list] = {}
    for role in roles:
        pair = composition.get(role, None) if isinstance(
            composition, dict) else None
        if (not isinstance(pair, list) or len(pair) != 2
                or any(isinstance(v, bool) or not isinstance(v, int)
                       for v in pair)):
            _harness_fail("create composition spec")
        frozen_inputs[role] = pair
    entry_params = entry_body.get("parameters") or []
    if not isinstance(entry_params, list) or not entry_params:
        _reject("ORACLE_CREATE_UNMAPPED", "entry has no parameters")
    calls: list[tuple[str, list]] = []
    for block_id in entry_body.get("blocks") or []:
        try:
            block = _decode_body(session, scratch_ws, str(block_id),
                                 current)
        except (JudgeRejection, JudgeHarnessError):
            raise
        except Exception as error:
            _harness_fail(f"create entry decode: {error}"[:120])
        for op_id in block.get("operations") or []:
            try:
                op = _decode_body(session, scratch_ws, str(op_id),
                                  current)
            except (JudgeRejection, JudgeHarnessError):
                raise
            except Exception as error:
                _harness_fail(f"create entry decode: {error}"[:120])
            if op.get("opcode") != 112:
                continue
            callee = _immediate_function(op.get("immediate"))
            operands = op.get("operands") or []
            if not role_of.get(callee, ""):
                _reject("ORACLE_CREATE_UNMAPPED",
                        "entry calls outside role set"[:120])
            if (len(operands) != 2
                    or any(not isinstance(o, dict)
                           or o.get("variant") != "Parameter"
                           or not isinstance(o.get("value"), str)
                           for o in operands)):
                _reject("ORACLE_CREATE_UNMAPPED",
                        "entry operands not parameters"[:120])
            calls.append((callee, [str(o["value"]) for o in operands]))
    if not calls:
        _reject("ORACLE_CREATE_UNMAPPED", "no mapped entry calls")
    labelings = [dict(role_of)]
    for first, second in ties:
        swapped = dict(role_of)
        first_entities = [entity for entity, role in role_of.items()
                          if role == first]
        second_entities = [entity for entity, role in role_of.items()
                           if role == second]
        for entity in first_entities:
            swapped[entity] = second
        for entity in second_entities:
            swapped[entity] = first
        labelings.append(swapped)
    conflict_detail = ""
    mismatch_detail = ""
    unexecutable_detail = ""
    for labeling in labelings:
        assigned: dict[str, int] = {}
        conflict = ""
        for callee, operands in calls:
            role = labeling.get(callee, "")
            pair = frozen_inputs.get(role, None)
            if pair is None:
                _harness_fail("create labeling invariant")
            for pid, value in zip(operands, pair):
                if pid in assigned and assigned[pid] != value:
                    conflict = (f"entry param {pid[:8]} "
                                f"{assigned[pid]} != {value} ({role})"[:120])
                    break
                assigned[pid] = value
            if conflict:
                break
        if conflict:
            conflict_detail = conflict_detail or conflict
            continue
        if not assigned:
            _harness_fail("create assignment invariant")
        vector = [assigned.get(str(param), 0) for param in entry_params]
        try:
            results = observations(entry, [tuple(vector)])
        except (JudgeRejection, JudgeHarnessError):
            raise
        except Exception as error:
            unexecutable_detail = unexecutable_detail or (
                f"entry driver failed: {type(error).__name__}"[:120])
            continue
        first = results[0] if isinstance(results, list) and results else None
        if not isinstance(first, dict) or first.get("ok") is not True:
            # Mapped but not executed: no execution evidence exists,
            # so no acceptance is possible (production limitation,
            # never candidate blame, never success).
            unexecutable_detail = unexecutable_detail or (
                "entry did not execute natively"[:120])
            continue
        value = first.get("value")
        if not isinstance(value, dict):
            _harness_fail("create entry envelope")
        if value == want:
            return
        mismatch_detail = mismatch_detail or (
            f"entry end-to-end {value} != "
            f"Ok({want_total})"[:120])
    if mismatch_detail:
        _reject("ORACLE_CREATE_MISMATCH", mismatch_detail)
    if unexecutable_detail:
        _reject("ORACLE_CREATE_UNEXECUTABLE", unexecutable_detail)
    if conflict_detail:
        _reject("ORACLE_CREATE_MISMATCH", conflict_detail)
    _harness_fail("create entry undecided")


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
        for cleanup in cleanups:
            try:
                cleanup()
            except Exception:
                continue


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
    method name. Each such response must fit MAX_RESPONSE_BYTES; any
    response carrying omitted>0 or truncated must be followed by a
    query.continue in the same session scope (explicit continuation;
    the corpus requires bounded operation WITH omissions and
    continuations, not their absence); a query.continue with no
    preceding truncated response is an inconsistent continuation and
    rejects; cumulative agent-visible bytes across the trial must fit
    AGENT_CUMULATIVE_RESPONSE_BUDGET. Whole-store reads are only
    inventory/side (full file enumeration). Hidden truncation,
    exceeded bounds, inconsistent continuations, and unbounded access
    reject under the frozen unbounded-read code; semantic-only success
    is noted in the rejection detail, never mislabeled as a pass.

    Limitation retained: transcripts record response bounds and body
    digests, not requested limit values or continuation tokens, so the
    judge verifies actual returned content, per-response limits,
    continuation discipline, and cumulative budgets — not the literal
    requested limit parameters or after==prev-next_after token
    equality. Judge-only inspection (judge transcript) stays separate
    from agent-visible context and cost.

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
        pending_truncated = False
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
                if method in BOUNDED_QUERY_METHODS:
                    if method == "query.continue":
                        entry_continuations += 1
                        if not pending_truncated:
                            _reject("QUERY_REQUIRED_FACT_OMITTED",
                                    "inconsistent continuation; semantics held")
                        pending_truncated = False
                    elif trunc or omit > 0:
                        # Bounded page with more to fetch: an explicit
                        # query.continue must follow in this scope.
                        pending_truncated = True
                else:
                    if trunc or omit > 0:
                        _reject("QUERY_REQUIRED_FACT_OMITTED",
                                f"hidden truncation on {method}; semantics held")
            else:
                # Non-call transcript markers (hello/greeting/seed/scope):
                # ordering-relevant but not agent-visible calls; the
                # hash chain covers them, the call audit skips them.
                continue
        if pending is not None:
            _reject("QUERY_REQUIRED_FACT_OMITTED",
                    "agent session order; semantics held")
        if pending_truncated:
            _reject("QUERY_REQUIRED_FACT_OMITTED",
                    "truncated page without continuation; semantics held")
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
            "operations={operations} continuations={continuations} "
            "bounded_reads={bounded_reads} refusals={refusals} "
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
