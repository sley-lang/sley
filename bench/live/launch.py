#!/usr/bin/env python3
"""S20-640 live campaign launcher: freeze a run, then execute its slots.

    python3 -m bench.live.launch freeze --prereg PREREG --tier small|large \
        --run-id ID --run-dir DIR --label CAMPAIGN|PILOT
    python3 -m bench.live.launch run --run-dir DIR [--slots TASK:ARM:SEED,...]
        [--continue-on-provider-unavailable]
    python3 -m bench.live.launch status --run-dir DIR

``freeze`` copies the committed preregistration's tier model, budgets,
seeds, and retry policy unchanged into one create-once run manifest and
fills only what the checkout determines: the commit (clean tree
required), the recomputed fixture, tool, oracle, and prompt digests, the
provider and binary identities, and the hardware. ``run`` re-derives
every one of those bindings before each slot and refuses on drift, runs
slots in the preregistered order through ``execute_attempt`` with the
provider sandbox, never re-runs a recorded slot, and halts after a slot
whose provider stream reports the account unavailable (that slot stays
recorded as the harness failure it is).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

from bench.live.manifest import (
    build_manifest,
    canonical_json_bytes,
    read_manifest,
    write_manifest_once,
)


ROOT = Path(__file__).resolve().parents[2]
CORPUS = ROOT / "bench" / "corpus" / "v1" / "tasks.json"
PREREG_CONTRACT = "sley2.s20-640-preregistration.v1"
ARM_ORDER = ("raw_files", "sley_1_2_0", "sley_2_0")
LABELS = frozenset({"CAMPAIGN", "PILOT"})
# Per sandbox profile: the release-directory files a run binds by digest,
# the first being the provider executable.
PROVIDER_FILES = {
    "codex": ("bin/codex", "bin/codex-code-mode-host"),
    "claude-code": ("claude",),
}
# Judge-side inputs whose bytes decide acceptance (git-tracked files).
ORACLE_ROOTS = ("bench/fixtures", "oracle")
ORACLE_FILES = (
    "bench/live/oracle.py",
    "bench/live/mediated_sley.py",
    "bench/live/trusted_capture.py",
    "bench/live/sley2_tool.py",
    "bench/live/sley2_codecs.py",
    "bench/live/scratch.py",
    "bench/live/snapshot.py",
    "bench/sley2/runner.py",
    "crates/sley-repo/tests/succ_live_judge_cases.rs",
)
UNAVAILABLE_MARKERS = (
    "hit your usage limit",
    "hit your limit",
    "usage limit",
    "usage_limit_reached",
    "rate limit",
    "rate_limit",
    "overloaded",
)


class LaunchError(ValueError):
    """The campaign cannot be frozen or a slot cannot be run as bound."""


def _fail(symbol: str, detail: str = "") -> None:
    raise LaunchError(symbol if not detail else f"{symbol}: {detail}")


def _sha_file(path: Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def _git(*args: str) -> str:
    completed = subprocess.run(["git", "-C", str(ROOT), *args],
                               capture_output=True, text=True, check=False)
    if completed.returncode != 0:
        _fail("LIVE_LAUNCH_GIT", completed.stderr.strip()[:200])
    return completed.stdout


def repo_state() -> tuple[str, bool]:
    head = _git("rev-parse", "HEAD").strip()
    clean = _git("status", "--porcelain", "--untracked-files=all").strip() == ""
    return head, clean


def oracle_digest() -> str:
    tracked = set(_git("ls-files", "-z", *ORACLE_ROOTS).split("\0")) - {""}
    tracked |= set(ORACLE_FILES)
    inventory = [{"path": path, "sha256": _sha_file(ROOT / path)}
                 for path in sorted(tracked)]
    return hashlib.sha256(
        b"sley2.live-oracle-inputs.v1\0"
        + canonical_json_bytes({"files": inventory})).hexdigest()


def tool_description_digests() -> dict[str, str]:
    from bench.live.mediated_attempt import (
        MEDIATED_TOOL_VERSION,
        production_staging_digest,
        stage_mediated_scratch,
    )
    from bench.live.tooling import tooling_digest

    digests = {arm: tooling_digest(arm) for arm in ("raw_files", "sley_1_2_0")}
    with tempfile.TemporaryDirectory(prefix="tooldigest-") as temporary:
        scratch = Path(temporary)
        stage_mediated_scratch(scratch)
        staged = production_staging_digest(scratch)
    digests["sley_2_0"] = hashlib.sha256(
        b"sley2.live-mediated-tooling.v1\0"
        + canonical_json_bytes({"files": staged,
                                "tool_version": MEDIATED_TOOL_VERSION})).hexdigest()
    return digests


def arm_fixture_digests() -> dict[str, str]:
    from bench.live.taskpacks import arm_fixture_digest

    return {arm: arm_fixture_digest(arm) for arm in ARM_ORDER}


def hardware_manifest() -> dict[str, Any]:
    cpu = ""
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                cpu = line.split(":", 1)[1].strip()
                break
        memory = next(line.split()[1] for line in Path("/proc/meminfo").read_text().splitlines()
                      if line.startswith("MemTotal:"))
    except (OSError, StopIteration):
        memory = "unknown"
    return {"cpu_model": cpu, "kernel": platform.release(),
            "logical_cpus": os.cpu_count() or 0, "mem_total_kb": str(memory),
            "node": platform.node()}


def load_preregistration(path: Path) -> dict[str, Any]:
    value = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(value, dict) or value.get("contract") != PREREG_CONTRACT:
        _fail("LIVE_LAUNCH_PREREG_INVALID", "contract")
    # A preregistration another committed preregistration supersedes can
    # never be frozen again (it stays as history).
    digest = _sha_file(Path(path))
    for other in sorted(Path(path).resolve().parent.glob("PREREGISTRATION*.json")):
        try:
            superseding = json.loads(other.read_text(encoding="utf-8")).get("supersedes") or {}
        except (OSError, ValueError):
            continue
        if isinstance(superseding, dict) and superseding.get("sha256") == digest:
            _fail("LIVE_LAUNCH_PREREG_SUPERSEDED", other.name)
    return value


def _binary(env_name: str) -> Path:
    raw = os.environ.get(env_name, "")
    path = Path(raw)
    if not raw or not path.is_file() or path.is_symlink():
        _fail("LIVE_LAUNCH_BINARY_UNBOUND", env_name)
    return path


def provider_executable(provider_root: Path, profile: str) -> Path:
    return Path(provider_root) / PROVIDER_FILES[profile][0]


def provider_identity(provider_root: Path, profile: str = "codex") -> dict[str, Any]:
    files = {name: _sha_file(provider_root / name) for name in PROVIDER_FILES[profile]}
    with tempfile.TemporaryDirectory(prefix="provider-version-") as home:
        version = subprocess.run([str(provider_executable(provider_root, profile)), "--version"],
                                 capture_output=True, text=True, check=False,
                                 env={"PATH": "/usr/bin:/bin", "HOME": home,
                                      "CLAUDE_CONFIG_DIR": home, "CODEX_HOME": home})
    return {"executable_sha256": files[PROVIDER_FILES[profile][0]], "files": files,
            "version": version.stdout.strip()}


def derive_bindings(provider_root: Path, profile: str = "codex") -> dict[str, Any]:
    """Every checkout-determined value a run binds (recomputed per slot)."""

    from bench.live.tooling import prompt_template_digest

    identity = provider_identity(provider_root, profile)
    sley = _binary("SLEY2_SLEY_BINARY")
    judge = _binary("SUCC_JUDGE_TEST_BINARY")
    return {
        "arm_fixture_digests": arm_fixture_digests(),
        "binaries": {
            "judge_driver": {"path": str(judge), "sha256": _sha_file(judge)},
            "sley": {"path": str(sley), "sha256": _sha_file(sley)},
        },
        "oracle_digest": oracle_digest(),
        "prompt_template_digest": prompt_template_digest(),
        "provider_executable_sha256": identity["executable_sha256"],
        "provider_files": identity["files"],
        "provider_version": identity["version"],
        "tool_description_digests": tool_description_digests(),
    }


def schedule(manifest: dict[str, Any]) -> list[tuple[str, str, int]]:
    """Preregistered order: seed-major, corpus task order, arms rotated by
    task index so no arm always runs first or last within a task."""

    tasks = [task["id"] for task in json.loads(CORPUS.read_text())["tasks"]]
    slots = []
    for seed in manifest["random_seeds"]:
        for index, task in enumerate(tasks):
            shift = index % len(ARM_ORDER)
            for arm in ARM_ORDER[shift:] + ARM_ORDER[:shift]:
                slots.append((task, arm, seed))
    return slots


def freeze(args: argparse.Namespace) -> int:
    prereg_path = Path(args.prereg).resolve()
    prereg = load_preregistration(prereg_path)
    if args.label not in LABELS:
        _fail("LIVE_LAUNCH_LABEL")
    head, clean = repo_state()
    if not clean:
        _fail("LIVE_LAUNCH_TREE_DIRTY")
    tier = prereg["tiers"][args.tier]
    provider = prereg["provider"]
    sandbox_inputs = provider["sandbox"]
    from bench.live.provider_sandbox import ProviderSandbox

    profile = sandbox_inputs.get("profile", "codex")
    sandbox = ProviderSandbox(provider_root=Path(sandbox_inputs["provider_root"]),
                              auth_source=Path(sandbox_inputs["auth_source"]),
                              allowed_hosts=tuple(sandbox_inputs["allowed_hosts"]),
                              profile=profile)
    if provider["model_provider"] != sandbox.spec.model_provider:
        _fail("LIVE_LAUNCH_PREREG_MISMATCH", "model_provider")
    from bench.live.manifest import CORPUS as _CORPUS, PLAN as _PLAN
    from bench.raw.runner import task_statement_digest

    for field, actual in (("benchmark_plan_digest", _sha_file(_PLAN)),
                          ("corpus_digest", _sha_file(_CORPUS)),
                          ("task_statement_digest", task_statement_digest())):
        if prereg[field] != actual:
            _fail("LIVE_LAUNCH_PREREG_MISMATCH", field)
    if prereg["scheduled_attempts_per_tier"] != 15 * 3 * prereg["trial_count_per_tier"]:
        _fail("LIVE_LAUNCH_PREREG_MISMATCH", "scheduled_attempts_per_tier")
    bindings = derive_bindings(Path(sandbox_inputs["provider_root"]), profile)
    command: dict[str, Any] = {
        "executable": str(provider_executable(Path(sandbox_inputs["provider_root"]), profile)),
        "flags_source": ("bench/live/provider.py ClaudeCodeAdapter.command" if profile == "claude-code"
                         else "bench/live/provider.py CodexExecAdapter.command"),
    }
    if profile == "codex":
        command["command_network_access"] = True
    environment = {
        "binaries": bindings["binaries"],
        "label": args.label,
        "preregistration": {
            "path": str(prereg_path.relative_to(ROOT)),
            "sha256": _sha_file(prereg_path),
        },
        "halt_rule": prereg["halt_rule"],
        "provider_command": command,
        "provider_environment": dict(prereg["provider_environment"]),
        "provider_files": bindings["provider_files"],
        "provider_sandbox": sandbox.describe(),
        "schedule_order": prereg["schedule_order"],
    }
    manifest = build_manifest(
        run_id=args.run_id,
        created_at_utc=datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        repo_commit=head,
        model_exact_version=tier["model_exact_version"],
        model_tier=args.tier,
        reasoning_effort=tier["reasoning_effort"],
        trial_count=prereg["trial_count_per_tier"],
        random_seeds=list(prereg["random_seeds"]),
        context_budget=prereg["budgets"]["context_budget"],
        action_budget=prereg["budgets"]["action_budget"],
        wall_time_budget=prereg["budgets"]["wall_time_budget"],
        retry_policy=dict(prereg["retry_policy"]),
        hardware_manifest=hardware_manifest(),
        cache_state=prereg["cache_state"],
        environment_manifest=environment,
        arm_fixture_digests=bindings["arm_fixture_digests"],
        tool_description_digests=bindings["tool_description_digests"],
        oracle_digest=bindings["oracle_digest"],
        prompt_template_digest=bindings["prompt_template_digest"],
        provider_executable_sha256=bindings["provider_executable_sha256"],
        provider_version=bindings["provider_version"],
        model_provider=sandbox.spec.model_provider,
    )
    run_dir = Path(args.run_dir)
    write_manifest_once(run_dir / "run_manifest.json", manifest)
    print(json.dumps({"frozen": str(run_dir / "run_manifest.json"),
                      "repo_commit": head, "scheduled_attempts": manifest["scheduled_attempts"]}))
    return 0


def verify_binding(manifest: dict[str, Any]) -> None:
    """Refuse a slot unless the checkout still is what the run froze."""

    head, clean = repo_state()
    if head != manifest["repo_commit"]:
        _fail("LIVE_LAUNCH_BINDING_DRIFT", "repo_commit")
    if not clean:
        _fail("LIVE_LAUNCH_BINDING_DRIFT", "tree dirty")
    environment = manifest["environment_manifest"]
    provider_root = Path(environment["provider_sandbox"]["provider_root"])
    bindings = derive_bindings(provider_root, environment["provider_sandbox"].get("profile", "codex"))
    for field in ("arm_fixture_digests", "tool_description_digests", "oracle_digest",
                  "prompt_template_digest", "provider_executable_sha256",
                  "provider_version"):
        if bindings[field] != manifest[field]:
            _fail("LIVE_LAUNCH_BINDING_DRIFT", field)
    for field in ("binaries", "provider_files"):
        if bindings[field] != environment[field]:
            _fail("LIVE_LAUNCH_BINDING_DRIFT", field)


def _recorded(run_dir: Path, store: Any) -> set[tuple[str, str, int]]:
    from bench.live.attempts import verify_attempts

    return {(record["task_id"], record["arm_id"], record["seed"])
            for record in verify_attempts(run_dir, store)}


def _provider_events(store: Any, record: dict[str, Any]) -> list[dict[str, Any]]:
    digest = record["artifacts"].get("provider_events_sha256")
    if not digest:
        return []
    events = []
    for line in store.read(digest).splitlines():
        try:
            event = json.loads(line)
        except (UnicodeError, ValueError):
            continue
        if isinstance(event, dict):
            events.append(event)
    return events


def provider_unavailable(store: Any, record: dict[str, Any]) -> bool:
    """True when the provider itself (an ``error``/``turn.failed`` event,
    an error ``result``, or a rejected ``rate_limit_event``; never model
    text) reported the account unavailable."""

    for event in _provider_events(store, record):
        kind = event.get("type")
        if kind == "rate_limit_event":
            info = event.get("rate_limit_info")
            if isinstance(info, dict) and info.get("status") not in (None, "allowed", "allowed_warning"):
                return True
            continue
        if kind == "result" and event.get("is_error") is True:
            message = json.dumps(event.get("result", ""), ensure_ascii=False).lower()
        elif kind in {"error", "turn.failed"}:
            message = json.dumps(event.get("message", event.get("error", "")), ensure_ascii=False).lower()
        else:
            continue
        if any(marker in message for marker in UNAVAILABLE_MARKERS):
            return True
    return False


def usage_pressure(store: Any, record: dict[str, Any]) -> float | None:
    """Highest provider-reported usage-window utilization (0..1) in the
    attempt's stream, or None when the provider reports none."""

    highest = None
    for event in _provider_events(store, record):
        info = event.get("rate_limit_info") if event.get("type") == "rate_limit_event" else None
        windows = info.get("unifiedWindows") if isinstance(info, dict) else None
        if not isinstance(windows, dict):
            continue
        for window in windows.values():
            value = window.get("utilization") if isinstance(window, dict) else None
            if isinstance(value, (int, float)) and not isinstance(value, bool):
                highest = value if highest is None else max(highest, value)
    return highest


def _parse_slots(text: str, manifest: dict[str, Any]) -> list[tuple[str, str, int]]:
    if text == "all":
        return schedule(manifest)
    wanted = []
    for item in text.split(","):
        task, arm, seed = item.split(":")
        wanted.append((task, arm, int(seed)))
    order = schedule(manifest)
    unknown = [slot for slot in wanted if slot not in order]
    if unknown:
        _fail("LIVE_LAUNCH_SLOT_UNKNOWN", str(unknown[0]))
    # Requested slots still run in the preregistered order.
    return [slot for slot in order if slot in wanted]


def run(args: argparse.Namespace) -> int:
    from bench.live.artifacts import ArtifactStore
    from bench.live.campaign import execute_attempt
    from bench.live.provider import ClaudeCodeAdapter, CodexExecAdapter
    from bench.live.provider_sandbox import ProviderSandbox, ProviderSandboxError

    run_dir = Path(args.run_dir)
    manifest = read_manifest(run_dir / "run_manifest.json")
    environment = manifest["environment_manifest"]
    binaries = environment["binaries"]
    os.environ["SLEY2_SLEY_BINARY"] = binaries["sley"]["path"]
    os.environ["SUCC_JUDGE_TEST_BINARY"] = binaries["judge_driver"]["path"]
    sandbox_description = environment["provider_sandbox"]
    profile = sandbox_description.get("profile", "codex")
    sandbox = ProviderSandbox(
        provider_root=Path(sandbox_description["provider_root"]),
        auth_source=Path(sandbox_description["auth_source"]),
        allowed_hosts=tuple(sandbox_description["allowed_hosts"]),
        profile=profile)
    halt_rule = environment.get("halt_rule") or {}
    utilization_limit = halt_rule.get("usage_utilization_halt_percent") if isinstance(halt_rule, dict) else None
    if profile == "claude-code":
        adapter: Any = ClaudeCodeAdapter(
            executable=environment["provider_command"]["executable"],
            model=manifest["model_exact_version"],
            reasoning_effort=manifest["model_configuration"]["reasoning_effort"])
    else:
        adapter = CodexExecAdapter(
            executable=environment["provider_command"]["executable"],
            model=manifest["model_exact_version"],
            reasoning_effort=manifest["model_configuration"]["reasoning_effort"],
            command_network_access=bool(environment["provider_command"]["command_network_access"]))
    store = ArtifactStore(run_dir / "artifacts")
    workspace_parent = Path(args.workspace_parent)
    for task, arm, seed in _parse_slots(args.slots, manifest):
        if (task, arm, seed) in _recorded(run_dir, store):
            print(json.dumps({"slot": [task, arm, seed], "skipped": "already recorded"}))
            continue
        verify_binding(manifest)
        try:
            record = execute_attempt(
                run_directory=run_dir, store=store, adapter=adapter,
                task_id=task, arm_id=arm, seed=seed,
                workspace_parent=workspace_parent, provider_sandbox=sandbox)
        except ProviderSandboxError as error:
            # Raised before launch (no slot consumed): e.g. the credential
            # copy would expire inside the attempt.
            print(json.dumps({"halt": str(error), "slot": [task, arm, seed]}), flush=True)
            return 4
        metrics = record["metrics"]
        print(json.dumps({
            "slot": [task, arm, seed], "status": record["status"],
            "failure_code": record["failure_code"],
            "provider_exit_code": record["provider_exit_code"],
            "wall_time_ms": metrics["wall_time"],
            "model_input_tokens": metrics["model_input_tokens"],
            "model_output_tokens": metrics["model_output_tokens"],
            "tool_calls": metrics["tool_calls"]}), flush=True)
        if provider_unavailable(store, record) and not args.continue_on_provider_unavailable:
            print(json.dumps({"halt": "provider unavailable",
                              "slot": [task, arm, seed]}), flush=True)
            return 3
        pressure = usage_pressure(store, record)
        if (isinstance(utilization_limit, int) and pressure is not None
                and pressure * 100 >= utilization_limit):
            # Stop before the next slot rather than burn it on a limit.
            print(json.dumps({"halt": "provider usage window near its limit",
                              "slot": [task, arm, seed], "utilization": pressure}), flush=True)
            return 5
    return 0


def status(args: argparse.Namespace) -> int:
    from bench.live.artifacts import ArtifactStore
    from bench.live.attempts import verify_attempts

    run_dir = Path(args.run_dir)
    manifest = read_manifest(run_dir / "run_manifest.json")
    records = verify_attempts(run_dir, ArtifactStore(run_dir / "artifacts"))
    print(json.dumps({"label": manifest["environment_manifest"]["label"],
                      "recorded": len(records),
                      "scheduled": manifest["scheduled_attempts"],
                      "statuses": sorted({r["status"] for r in records})}))
    return 0


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="bench.live.launch")
    commands = parser.add_subparsers(dest="command", required=True)
    frozen = commands.add_parser("freeze")
    frozen.add_argument("--prereg", required=True)
    frozen.add_argument("--tier", required=True, choices=("small", "large"))
    frozen.add_argument("--run-id", required=True)
    frozen.add_argument("--run-dir", required=True)
    frozen.add_argument("--label", required=True, choices=sorted(LABELS))
    running = commands.add_parser("run")
    running.add_argument("--run-dir", required=True)
    running.add_argument("--slots", default="all")
    running.add_argument("--workspace-parent", default=os.environ.get("TMPDIR", "/home/gfarch/Work/tc") + "/ws")
    running.add_argument("--continue-on-provider-unavailable", action="store_true")
    state = commands.add_parser("status")
    state.add_argument("--run-dir", required=True)
    args = parser.parse_args(list(argv) if argv is not None else None)
    try:
        return {"freeze": freeze, "run": run, "status": status}[args.command](args)
    except LaunchError as error:
        print(json.dumps({"error": str(error)}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
