"""Deterministic initial-state staging for executable succession tasks."""

from __future__ import annotations

import hashlib
import json
import os
import stat
import tempfile
from functools import lru_cache
from pathlib import Path

from bench.live.manifest import canonical_json_bytes
from bench.live.snapshot import encode_snapshot, snapshot_directory


ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "bench" / "fixtures"
INITIAL = ROOT / "bench" / "live" / "initial"
CORPUS = ROOT / "bench" / "corpus" / "v1" / "tasks.json"
IMPLEMENTED_ARMS = frozenset({"raw_files", "sley_1_2_0"})
ARM_DIRECTORIES = {"raw_files": "raw", "sley_1_2_0": "legacy"}
TRANSIENT_NAMES = frozenset({"__pycache__", ".pytest_cache", ".mypy_cache"})

_corpus = json.loads(CORPUS.read_text(encoding="utf-8"))
TASK_IDS = tuple(task["id"] for task in _corpus["tasks"])

# Every non-CREATE start is a named, already oracle-proven failing control.
# CREATE starts from the explicit blank directories under bench/live/initial.
START_VARIANTS: dict[str, dict[str, str | None]] = {
    "raw_files": {
        "S2B-CREATE-001": None,
        "S2B-REPAIR-001": "upper_returns_low",
        "S2B-SIG-001": "missing_caller",
        "S2B-MODULE-001": "stale_import",
        "S2B-TYPE-001": "bool_compat_field",
        "S2B-EFFECT-001": "undeclared_effect",
        "S2B-CAP-001": "wildcard_scope",
        "S2B-DEAD-001": "reachable_changed",
        "S2B-TEST-001": "case_missing",
        "S2B-STALE-001": "guard_disabled",
        "S2B-MERGE-001": "overlapping_change",
        "S2B-PERF-001": "faster_but_wrong",
        "S2B-CONTEXT-001": "unbounded_read",
        "S2B-ADVERSARY-001": "wrong_repair",
        "S2B-CORRUPT-001": "unflipped",
    },
    "sley_1_2_0": {
        "S2B-CREATE-001": None,
        "S2B-REPAIR-001": "upper_returns_low",
        "S2B-SIG-001": "missing_caller",
        "S2B-MODULE-001": "stale_import",
        "S2B-TYPE-001": "bool_compat_field",
        "S2B-EFFECT-001": "undeclared_caller",
        "S2B-CAP-001": "effect_dropped",
        "S2B-DEAD-001": "reachable_changed",
        "S2B-TEST-001": "case_missing",
        "S2B-STALE-001": "guard_disabled",
        "S2B-MERGE-001": "overlapping_change",
        "S2B-PERF-001": "faster_but_wrong",
        "S2B-CONTEXT-001": "unbounded_read",
        "S2B-ADVERSARY-001": "wrong_repair",
        "S2B-CORRUPT-001": "unflipped",
    },
}


def _source(arm_id: str, task_id: str) -> Path:
    if arm_id not in IMPLEMENTED_ARMS or task_id not in TASK_IDS:
        raise ValueError("LIVE_TASKPACK_UNKNOWN")
    variant = START_VARIANTS[arm_id][task_id]
    if variant is None:
        return INITIAL / ARM_DIRECTORIES[arm_id] / task_id
    return FIXTURES / ARM_DIRECTORIES[arm_id] / task_id / "negative" / variant


def _copy_tree(source: Path, destination: Path) -> None:
    try:
        if destination.exists() or destination.is_symlink():
            raise ValueError("LIVE_TASKPACK_DESTINATION")
        destination.mkdir(mode=0o700, parents=True)
    except OSError as error:
        raise ValueError(f"LIVE_TASKPACK_DESTINATION: {error}") from error

    def copy_directory(input_directory: Path, output_directory: Path) -> None:
        for child in sorted(os.scandir(input_directory), key=lambda item: item.name):
            if child.name in TRANSIENT_NAMES or child.name.endswith((".pyc", ".pyo")):
                continue
            metadata = child.stat(follow_symlinks=False)
            target = output_directory / child.name
            if stat.S_ISLNK(metadata.st_mode):
                raise ValueError(f"LIVE_TASKPACK_UNSAFE: {child.path}")
            if stat.S_ISDIR(metadata.st_mode):
                target.mkdir(mode=0o700)
                copy_directory(Path(child.path), target)
                target.chmod(stat.S_IMODE(metadata.st_mode))
            elif stat.S_ISREG(metadata.st_mode):
                content = Path(child.path).read_bytes()
                flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
                if hasattr(os, "O_NOFOLLOW"):
                    flags |= os.O_NOFOLLOW
                descriptor = os.open(target, flags, 0o600)
                try:
                    written = 0
                    while written < len(content):
                        count = os.write(descriptor, content[written:])
                        if count <= 0:
                            raise ValueError("LIVE_TASKPACK_WRITE_FAILED")
                        written += count
                    os.fchmod(descriptor, stat.S_IMODE(metadata.st_mode))
                finally:
                    os.close(descriptor)
            else:
                raise ValueError(f"LIVE_TASKPACK_UNSAFE: {child.path}")

    source = source.resolve(strict=True)
    if not source.is_dir() or source.is_symlink():
        raise ValueError("LIVE_TASKPACK_SOURCE")
    copy_directory(source, destination)


def stage_initial(arm_id: str, task_id: str, destination: Path) -> None:
    """Create a disposable copy of one frozen starting state."""

    if arm_id == "sley_1_2_0" and task_id == "S2B-CREATE-001":
        target = Path(destination)
        if target.exists() or target.is_symlink():
            raise ValueError("LIVE_TASKPACK_DESTINATION")
        target.mkdir(mode=0o700, parents=True)
        return
    _copy_tree(_source(arm_id, task_id), Path(destination))


@lru_cache(maxsize=None)
def arm_fixture_digest(arm_id: str) -> str:
    """Digest the exact 15 initial snapshots and their selected variants."""

    if arm_id not in IMPLEMENTED_ARMS:
        raise ValueError("LIVE_TASKPACK_UNKNOWN")
    tasks: list[dict[str, str | None]] = []
    with tempfile.TemporaryDirectory(prefix="sley2-live-taskpack-") as temporary:
        root = Path(temporary)
        for task_id in TASK_IDS:
            workspace = root / task_id
            stage_initial(arm_id, task_id, workspace)
            snapshot = encode_snapshot(snapshot_directory(workspace))
            tasks.append(
                {
                    "snapshot_sha256": hashlib.sha256(snapshot).hexdigest(),
                    "task_id": task_id,
                    "variant": START_VARIANTS[arm_id][task_id],
                }
            )
    payload = {
        "arm_id": arm_id,
        "contract": "sley2.live-taskpack.v1",
        "tasks": tasks,
    }
    return hashlib.sha256(b"sley2.live-taskpack.v1\0" + canonical_json_bytes(payload)).hexdigest()
