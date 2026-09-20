#!/usr/bin/env python3
"""End-to-end proof of the protected live-capture boundary (no model).

Deterministic adapters run CONFINED (bwrap mount+user namespace) with
no protected filesystem; their only channel is frames over pipes to
the trusted gateway, which executes the real sley2_tool Session
machinery on the runner-owned protected workspace inside
TrustedCapture.exchange. The same containment + mediation
configuration is intended for actual trials (deterministic adapters
stand in for the provider process; the provider path adds --share-net
and the frozen provider command).

Steps (argv[1], default `all`):
  access    confined open/read/write probes on protected prefixes
            (asserts actual errno outcomes + host digests unchanged)
  refusal   confined allowed-read + denied-command through the gateway
            (asserts captured refusal, budget counting, forged
            candidate-side diagnostics ignored)
  type_pos  confined bounded-read / propose / resolve / compose /
            finish through the gateway + trusted judge + adjudicate
  inject    failure injections on capture COPIES (gap, torn suffix,
            missing completion, final mismatch, budget corrupt):
            every variant fails closed with evidence preserved
  storage   storage-failure fail-closed (read-only capture dir)
  all       the above in order (type_pos last; slowest)

Env: SLEY2_SLEY_BINARY (required for refusal/type_pos),
SUCC_JUDGE_TEST_BINARY (required for type_pos oracle step).
Log: argv[2] optional logfile (all steps append human-readable lines).

Acceptance of a mediated attempt requires BOTH a reconciled
runner-owned capture AND the trusted oracle verdict (adjudicate);
candidate-side files never participate.
"""

from __future__ import annotations

import hashlib
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from bench.fixtures import sley2_live_judge as judge  # noqa: E402
from bench.live import mediated_sley as gw  # noqa: E402
from bench.live import trusted_capture as tc  # noqa: E402
from bench.live import confined as cf  # noqa: E402
from bench.live.taskpacks import stage_initial  # noqa: E402

TASK_ID = "S2B-TYPE-001"
TASK_DIR = ROOT / "bench" / "fixtures" / "sley2" / TASK_ID
TOOL_VERSION = "1"

# Adapter-conventional member labels (deterministic, consistent across
# the attempt; NOT frozen task content — the judge checks typedef
# shape and case coverage, never these literals).
MEMBERS = {"queued": "51" * 32, "running": "52" * 32,
           "succeeded": "53" * 32, "failed": "54" * 32}

lines: list[str] = []


def emit(text: str) -> None:
    lines.append(text)
    print(text, flush=True)


def sha256_file(path: Path) -> str:
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def require_env(name: str) -> str:
    value = os.environ.get(name, "")
    if not value:
        raise SystemExit(f"missing env {name}")
    return value


def trial_inputs(manifest: dict) -> dict:
    entities = manifest["entities"]
    param = ""
    for candidate in manifest.get("targets", []):
        if isinstance(candidate, str) and candidate.startswith("6c"):
            param = candidate
    if not param:
        raise SystemExit("no 6c param in manifest targets")
    return {
        "task_id": TASK_ID,
        "entities": {
            "status": entities["status"],
            "switch": entities["switch"],
            "param": param,
            "switch_entry": entities["switch_entry"],
            "switch_leaf": entities["switch_leaf"],
            "typedef_hint": entities["status"],
        },
        "members": MEMBERS,
    }


def frozen_bindings(protected_ws: Path, manifest: dict) -> dict:
    return {
        "pack_sha256": sha256_file(protected_ws / "base.pack"),
        "task_manifest_sha256": hashlib.sha256(
            json.dumps(manifest, sort_keys=True).encode()).hexdigest(),
        "tool_version": TOOL_VERSION,
        "binary_sha256": sha256_file(Path(require_env("SLEY2_SLEY_BINARY"))),
    }


def setup_run(root: Path, with_binary: bool) -> dict:
    """Stage runner-owned protected state + agent scratch (no tooling in
    either: the gateway drives Sessions directly; the agent holds only
    trial inputs and the client script copy)."""

    protected_ws = root / "protected" / "ws"
    stage_initial("sley_2_0", TASK_ID, protected_ws)
    try:
        os.chmod(root / "protected", 0o700)
        os.chmod(protected_ws, 0o700)
    except OSError:
        pass
    manifest = json.loads((TASK_DIR / "task_manifest.json").read_text())
    scratch = root / "scratch"
    scratch.mkdir(mode=0o700, parents=True)
    (scratch / "trial_inputs.json").write_text(
        json.dumps(trial_inputs(manifest), sort_keys=True), encoding="utf-8")
    shutil.copyfile(ROOT / "bench" / "live" / "mediated_client.py",
                    scratch / "mediated_client.py")
    capture_dir = root / "capture"
    frozen = frozen_bindings(protected_ws, manifest) if with_binary else None
    return {"protected_ws": protected_ws, "manifest": manifest,
            "scratch": scratch, "capture_dir": capture_dir,
            "frozen": frozen}


def mask_paths(env: dict) -> list[Path]:
    """Every prefix the confined trial must not observe (data AND code
    with expected answers: fixtures, live witnesses/logs, oracle
    project, Rust test vectors). The client copy in scratch is the
    only bench code visible."""

    root = env["capture_dir"].parent
    return [root / "protected", env["capture_dir"],
            ROOT / "bench", ROOT / "oracle", ROOT / "crates"]


def spawn_client(env: dict, sequence: str,
                 extra_args: list[str]) -> subprocess.Popen[bytes]:
    spec = cf.SandboxSpec(scratch_dir=env["scratch"],
                          mask_paths=tuple(mask_paths(env)),
                          share_net=False)
    argv = cf.confinement_argv(
        spec, ["python3", "/scratch/mediated_client.py", sequence,
               *extra_args])
    return subprocess.Popen(argv, stdin=subprocess.PIPE,
                            stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE)


def summary_of(diagnostics: str) -> dict:
    for line in diagnostics.splitlines():
        if line.startswith("SUMMARY "):
            return json.loads(line[len("SUMMARY "):])
        if line.startswith("ACCESS_PROBE_RESULT "):
            return json.loads(line[len("ACCESS_PROBE_RESULT "):])
    return {}


def step_access() -> int:
    emit("== capture_demo/access: confined open/read/write probes ==")
    if not cf.bwrap_available():
        emit("CONFINEMENT_UNAVAILABLE: bwrap missing (failing preflight kept)")
        return 2
    tmp = Path(tempfile.mkdtemp(prefix="sley2-capture-access-"))
    protected = tmp / "protected"
    capture = tmp / "capture"
    protected.mkdir(mode=0o700)
    capture.mkdir(mode=0o700)
    secret = protected / "secret.txt"
    secret.write_bytes(b"SECRET-PROTECTED-CONTENT")
    before = sha256_file(secret)
    scratch = tmp / "scratch"
    scratch.mkdir(mode=0o700)
    # Low-level probe (actual syscalls, errno outcomes).
    probe = cf.probe_access(scratch, [protected, capture])
    for path in (str(protected), str(capture)):
        read = probe["reads"][path]
        emit(f"probe read {path}: ok={read['ok']} "
             f"error={read.get('error')} errno={read.get('errno')}")
        if read["ok"]:
            emit("FAIL: protected path readable inside sandbox")
            return 1
    emit(f"probe scratch writable: {probe['scratch_ok']}")
    if not probe["scratch_ok"]:
        emit("FAIL: scratch not writable")
        return 1
    # Writes to masked prefixes land in the private tmpfs (or fail);
    # either way the host files are unchanged.
    after = sha256_file(secret)
    emit(f"host secret unchanged: {before == after}")
    if before != after:
        emit("FAIL: host protected file modified from sandbox")
        return 1
    # Gateway-level access probe (confined client, no gateway needed:
    # it exits before any frame; run it unconsumed to show the client
    # itself cannot exfiltrate by path).
    env = {"capture_dir": capture, "scratch": scratch}
    (scratch / "trial_inputs.json").write_text(
        json.dumps({"task_id": TASK_ID, "entities": {}, "members": {}}),
        encoding="utf-8")
    client_src = ROOT / "bench" / "live" / "mediated_client.py"
    shutil.copyfile(client_src, scratch / "mediated_client.py")
    spec = cf.SandboxSpec(scratch_dir=scratch,
                          mask_paths=(protected, capture))
    argv = cf.confinement_argv(
        spec, ["python3", "/scratch/mediated_client.py", "access_probe",
               str(protected), str(capture)])
    completed = subprocess.run(argv, capture_output=True, timeout=120,
                               check=False)
    if completed.returncode != 0:
        emit(f"FAIL: access_probe client exit {completed.returncode}: "
             f"{completed.stderr.decode()[:300]}")
        return 1
    result = summary_of(completed.stderr.decode())
    emit(f"client access_probe: {json.dumps(result, sort_keys=True)[:600]}")
    for path in (str(protected), str(capture)):
        if result.get("reads", {}).get(path, {}).get("ok"):
            emit(f"FAIL: client read {path}")
            return 1
    if sha256_file(secret) != before:
        emit("FAIL: host secret modified by client")
        return 1
    emit("access: PASS (protected unreadable; host unchanged; scratch works)")
    return 0


def step_refusal() -> int:
    emit("== capture_demo/refusal: allowed read + denied command ==")
    sley = Path(require_env("SLEY2_SLEY_BINARY"))
    tmp = Path(tempfile.mkdtemp(prefix="sley2-capture-refusal-"))
    env = setup_run(tmp, with_binary=True)
    capture_dir = env["capture_dir"]
    assert env["frozen"] is not None
    cap = tc.TrustedCapture.create(capture_dir, attempt_id="demo.refusal.0",
                                   frozen=env["frozen"])
    endpoint = gw.MediatedSleyEndpoint(sley, env["protected_ws"], cap)
    proc = spawn_client(env, "refusal_probe", [])
    try:
        pumped = gw.pump_client(proc, endpoint, deadline_s=600)
    except (gw.GatewayError, tc.CaptureError) as error:
        emit(f"FAIL: pump error: {error}")
        return 1
    emit(f"pump frames={pumped['frames']} returncode={pumped['returncode']}")
    outcome = summary_of(pumped["diagnostics"])
    emit(f"client summary: {json.dumps(outcome, sort_keys=True)[:300]}")
    if not outcome.get("ok"):
        emit("FAIL: refusal_probe outcome not ok")
        return 1
    # Forged candidate-side diagnostics exist but must never count.
    forged_final = env["scratch"] / "final_candidate.hex"
    forged_chain = env["scratch"] / ".sley-live-transcript.jsonl"
    emit(f"forged scratch files present: {forged_final.is_file()} "
         f"{forged_chain.is_file()}")
    final = endpoint.protected_final()
    emit(f"protected final present: {final is not None} (expect None)")
    if final is not None:
        emit("FAIL: unexpected protected final")
        return 1
    completion = cap.complete(b"no-final-bytes")
    _ = completion
    result = tc.reconcile(capture_dir, b"no-final-bytes")
    emit(f"reconcile: {result['code']} exchanges="
         f"{result.get('complete_exchanges')} totals="
         f"{json.dumps(result.get('totals', {}), sort_keys=True)}")
    if not result["reconciled"]:
        emit("FAIL: refusal capture did not reconcile")
        return 1
    if result["totals"]["failed"] < 1:
        emit("FAIL: denial not counted as failed response")
        return 1
    # The forged scratch bytes are nowhere in the capture.
    forged = forged_final.read_bytes()
    captured = (capture_dir / "exchanges.jsonl").read_bytes()
    if forged.strip() in captured:
        emit("FAIL: forged bytes appear in capture")
        return 1
    status, code = gw.adjudicate(capture_dir, None,
                                 {"status": "rejected",
                                  "code": "ORACLE_NOT_RUN"})
    emit(f"adjudicate (no final): {status} {code}")
    if status != "harness_failure":
        emit("FAIL: missing final must not release any verdict")
        return 1
    emit("refusal: PASS (denial captured+counted; forgeries ignored; "
         "no final, no acceptance)")
    return 0


def step_type_pos() -> int:
    emit("== capture_demo/type_pos: confined full migration ==")
    sley = Path(require_env("SLEY2_SLEY_BINARY"))
    require_env("SUCC_JUDGE_TEST_BINARY")
    tmp = Path(tempfile.mkdtemp(prefix="sley2-capture-typepos-"))
    emit(f"run root kept at: {tmp}")
    env = setup_run(tmp, with_binary=True)
    capture_dir = env["capture_dir"]
    assert env["frozen"] is not None
    cap = tc.TrustedCapture.create(capture_dir, attempt_id="demo.typepos.0",
                                   frozen=env["frozen"])
    endpoint = gw.MediatedSleyEndpoint(sley, env["protected_ws"], cap)
    proc = spawn_client(env, "type_pos", [])
    try:
        pumped = gw.pump_client(proc, endpoint, deadline_s=1800)
    except (gw.GatewayError, tc.CaptureError) as error:
        emit(f"FAIL: pump error: {error}")
        return 1
    emit(f"pump frames={pumped['frames']} returncode={pumped['returncode']}")
    outcome = summary_of(pumped["diagnostics"])
    emit(f"client summary: {json.dumps(outcome, sort_keys=True)[:500]}")
    if not outcome.get("ok"):
        emit(f"FAIL: mediated migration not finished: "
             f"{outcome.get('error')}")
        return 1
    final = endpoint.protected_final()
    if final is None:
        emit("FAIL: no protected final after finish")
        return 1
    emit(f"protected final bytes: {len(final)}")
    completion = cap.complete(final)
    emit(f"completion exchanges={completion['exchanges']} "
         f"sessions={completion['sessions']} phases={completion['phases']}")
    result = tc.reconcile(capture_dir, final)
    emit(f"reconcile: {result['code']} exchanges="
         f"{result.get('complete_exchanges')}")
    if not result["reconciled"]:
        emit("FAIL: migration capture did not reconcile")
        return 1
    # Trusted judge over the protected workspace (runner-side).
    saved_argv = sys.argv
    sys.argv = ["sley2_live_judge", str(env["protected_ws"])]
    out = io.StringIO()
    saved_cwd = os.getcwd()
    os.chdir(env["protected_ws"])
    try:
        with mock.patch.object(judge.sys, "stdout", out):
            exit_code = judge.main(TASK_ID)
    finally:
        sys.argv = saved_argv
        os.chdir(saved_cwd)
    emit(f"judge exit: {exit_code} {out.getvalue()[:300]}")
    try:
        printed = json.loads(out.getvalue().strip().splitlines()[-1])
        oracle_verdict = {"status": printed.get("status"),
                          "code": printed.get("code")}
    except (ValueError, IndexError):
        oracle_verdict = ({"status": "accepted", "code": None}
                          if exit_code == 0 else
                          {"status": "rejected", "code": "ORACLE_UNKNOWN"})
    status, code = gw.adjudicate(capture_dir, final, oracle_verdict)
    emit(f"adjudicate: {status} {code}")
    if status != "accepted":
        emit("FAIL: reconciled + accepted oracle must adjudicate accepted")
        return 1
    emit("type_pos: PASS (confined migration reconciled; judge accepted; "
         "adjudicated accepted)")
    return 0


def step_inject() -> int:
    emit("== capture_demo/inject: failure injections fail closed ==")
    tmp = Path(tempfile.mkdtemp(prefix="sley2-capture-inject-"))
    base = tmp / "base"
    cap = tc.TrustedCapture.create(base, attempt_id="demo.inject.0",
                                   frozen={
                                       "pack_sha256": "a" * 64,
                                       "task_manifest_sha256": "b" * 64,
                                       "tool_version": "1",
                                       "binary_sha256": "c" * 64})
    cap.exchange(phase="read", session_id="s1", method="revision",
                 request=b"{}",
                 handler=lambda: (b"r1", {"failed": False}))
    cap.exchange(phase="compose", session_id="s1", method="propose",
                 request=b"{}",
                 handler=lambda: (b"r2", {"failed": False,
                                          "continued": True, "omitted": 2}))
    final = b"final-inject"
    cap.complete(final)
    good = tc.reconcile(base, final)
    if not good["reconciled"]:
        emit(f"FAIL: baseline capture broken: {good}")
        return 1
    emit(f"baseline: {good['code']} exchanges=2")

    def variant(name: str, mutate) -> dict:
        target = tmp / name
        shutil.copytree(base, target)
        mutate(target)
        return tc.reconcile(target, final)

    def drop_response(target: Path) -> None:
        path = target / "exchanges.jsonl"
        lines = path.read_text(encoding="utf-8").splitlines()
        path.write_text("\n".join([lines[0], lines[1], lines[2]]) + "\n",
                        encoding="utf-8")

    def drop_completion(target: Path) -> None:
        (target / "completion.json").unlink()

    def tamper_final_file(_: Path) -> None:
        pass  # final mismatch is passed as expected_final below

    def corrupt_ledger(target: Path) -> None:
        (target / "budgets.json").write_text("not json{",
                                             encoding="utf-8")

    def torn_suffix(target: Path) -> None:
        path = target / "exchanges.jsonl"
        path.write_bytes(path.read_bytes() + b'{"torn": tru')

    cases = [
        ("missing_records", drop_response,
         ("CAPTURE_ORPHAN_REQUEST", "CAPTURE_COMPLETION_INVALID",
          "CAPTURE_TOTALS_MISMATCH")),
        ("incomplete_completion", drop_completion,
         ("CAPTURE_COMPLETION_MISSING",)),
        ("storage_ledger_corrupt", corrupt_ledger,
         ("CAPTURE_BUDGET_CORRUPT",)),
        ("interrupted_collection", torn_suffix, ("CAPTURE_TORN_SUFFIX",)),
    ]
    failed = 0
    for name, mutate, codes in cases:
        if name == "storage_ledger_corrupt":
            result = variant(name, mutate)
        else:
            result = variant(name, mutate)
        emit(f"inject {name}: reconciled={result['reconciled']} "
             f"code={result['code']} evidence_exchanges="
             f"{result.get('complete_exchanges')}")
        if result["reconciled"] or result["code"] not in codes:
            emit(f"FAIL: {name} must fail closed with {codes}")
            failed += 1
    mismatch = tc.reconcile(tmp / "base", b"different-final")
    emit(f"inject final_mismatch: reconciled={mismatch['reconciled']} "
         f"code={mismatch['code']}")
    if mismatch["reconciled"] or mismatch["code"] != "CAPTURE_FINAL_MISMATCH":
        emit("FAIL: final mismatch must fail closed")
        failed += 1
    # Ledger bytes preserved (not zeroed) on the corrupt variant.
    ledger_raw = (tmp / "storage_ledger_corrupt" / "budgets.json"
                  ).read_bytes()
    emit(f"corrupt ledger preserved bytes: {ledger_raw[:20]!r}")
    if ledger_raw != b"not json{":
        emit("FAIL: corrupt ledger bytes not preserved")
        failed += 1
    # Original baseline untouched by the injections.
    again = tc.reconcile(base, final)
    if not again["reconciled"]:
        emit("FAIL: baseline mutated by injection copies")
        failed += 1
    if failed:
        return 1
    emit("inject: PASS (all variants fail closed; evidence preserved)")
    return 0


def step_storage() -> int:
    emit("== capture_demo/storage: unavailable storage fails closed ==")
    tmp = Path(tempfile.mkdtemp(prefix="sley2-capture-storage-"))
    capture_dir = tmp / "cap"
    cap = tc.TrustedCapture.create(capture_dir, attempt_id="demo.storage.0",
                                   frozen={
                                       "pack_sha256": "a" * 64,
                                       "task_manifest_sha256": "b" * 64,
                                       "tool_version": "1",
                                       "binary_sha256": "c" * 64})
    os.chmod(capture_dir, 0o555)
    try:
        try:
            cap.exchange(phase="read", session_id="s1", method="a",
                         request=b"{}",
                         handler=lambda: (b"r", {"failed": False}))
        except tc.CaptureError as error:
            emit(f"exchange refused as expected: {str(error)[:100]}")
        else:
            # Running as owner, chmod 555 still permits writes; treat
            # a successful write as a skipped simulation, not proof.
            emit("NOTE: owner-write succeeded despite 555 (running as "
                 "owner); storage-failure path covered by unit tests + "
                 "read-only-file variant below")
        # Stronger simulation independent of ownership: replace the
        # exchanges file with a directory so append fails.
        os.chmod(capture_dir, 0o700)
        (capture_dir / "exchanges.jsonl").mkdir()
        try:
            cap.exchange(phase="read", session_id="s1", method="b",
                         request=b"{}",
                         handler=lambda: (b"r", {"failed": False}))
            emit("FAIL: exchange over directory-backed log succeeded")
            return 1
        except tc.CaptureError as error:
            emit(f"exchange refused as expected: {str(error)[:100]}")
    finally:
        os.chmod(capture_dir, 0o700)
    try:
        result = tc.reconcile(capture_dir, b"f")
    except tc.CaptureError as error:
        # Unreadable storage is itself a fail-closed non-acceptance.
        emit(f"reconcile refused on broken storage: {str(error)[:100]}")
        emit("storage: PASS (unavailable storage fails closed)")
        return 0
    emit(f"reconcile after storage failure: reconciled="
         f"{result['reconciled']} code={result['code']}")
    if result["reconciled"]:
        emit("FAIL: storage failure must not reconcile")
        return 1
    emit("storage: PASS (unavailable storage fails closed)")
    return 0


def main() -> int:
    which = sys.argv[1] if len(sys.argv) > 1 else "all"
    log_path = Path(sys.argv[2]) if len(sys.argv) > 2 else None
    steps = {"access": step_access, "refusal": step_refusal,
             "type_pos": step_type_pos, "inject": step_inject,
             "storage": step_storage}
    order = (["access", "refusal", "inject", "storage", "type_pos"]
             if which == "all" else [which])
    code = 0
    for name in order:
        if name not in steps:
            emit(f"unknown step {name}")
            return 2
        try:
            step_code = steps[name]()
        except Exception as error:  # noqa: BLE001 - demo reports failures
            emit(f"step {name} raised {type(error).__name__}: {error}")
            step_code = 1
        emit(f"step {name}: {'PASS' if step_code == 0 else 'FAIL'}")
        if step_code != 0:
            code = step_code
            if which == "all":
                emit(f"stopping after failed step {name}")
                break
    if log_path is not None:
        log_path.parent.mkdir(parents=True, exist_ok=True)
        log_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return code


if __name__ == "__main__":
    raise SystemExit(main())
