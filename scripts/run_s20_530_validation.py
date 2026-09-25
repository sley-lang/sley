#!/usr/bin/env python3
"""Run and bind the authoritative S20-530 Tier 2 closeout evidence."""

from __future__ import annotations

import errno
import ctypes
import hashlib
import io
import json
import os
import resource
import secrets
import select
import selectors
import shlex
import signal
import site
import stat
import subprocess
import sys
import tarfile
import tempfile
import time
import types
from collections.abc import Mapping
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / "scripts/check_s20_530_crash_recovery.py"


def stop(message: str) -> None:
    print(f"S20-530 validation runner failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def require_isolated_python_startup() -> None:
    flags = sys.flags
    if not (
        flags.isolated == 1
        and flags.ignore_environment == 1
        and flags.no_user_site == 1
        and flags.safe_path
        and flags.dont_write_bytecode == 1
        and site.ENABLE_USER_SITE is False
    ):
        stop("invoke with exact /usr/bin/python3 -I -B")
    try:
        user_site = Path(site.getusersitepackages()).resolve(strict=False)
    except (AttributeError, OSError, TypeError, ValueError) as error:
        stop(f"cannot resolve Python user-site boundary: {error}")
    for entry in sys.path:
        try:
            if Path(entry).resolve(strict=False) == user_site:
                stop("Python user site is present on sys.path")
        except (OSError, TypeError, ValueError) as error:
            stop(f"cannot resolve Python import path: {error}")


def require_linux_subreaper() -> None:
    pr_set_child_subreaper = 36
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.prctl(pr_set_child_subreaper, 1, 0, 0, 0) != 0:
        error_number = ctypes.get_errno()
        stop(f"cannot establish Linux child-subreaper boundary: errno {error_number}")


def require_pidfd_support() -> None:
    if not hasattr(os, "pidfd_open") or not hasattr(signal, "pidfd_send_signal"):
        stop("Linux pidfd support is required for identity-safe command cleanup")
    try:
        descriptor = os.pidfd_open(os.getpid(), 0)
    except OSError as error:
        stop(f"cannot open runner pidfd: {error}")
    try:
        signal.pidfd_send_signal(descriptor, 0, None, 0)
    except OSError as error:
        stop(f"cannot signal runner pidfd: {error}")
    finally:
        os.close(descriptor)


def load_contract() -> types.ModuleType:
    try:
        source = CHECKER.read_bytes()
        code = compile(source, str(CHECKER), "exec", dont_inherit=True)
    except (OSError, SyntaxError) as error:
        stop(f"cannot compile frozen checker source: {error}")
    module = types.ModuleType("s20_530_frozen_contract")
    module.__file__ = str(CHECKER)
    exec(code, module.__dict__)
    return module


require_isolated_python_startup()
require_linux_subreaper()
require_pidfd_support()
contract = load_contract()
EVIDENCE = contract.CLOSEOUT_EVIDENCE
TEST_PLAN = contract.TEST_PLAN
LOG_DIR = contract.VALIDATION_LOG_DIR
OUTPUT_READ_LIMIT_BYTES = contract.COMMAND_OUTPUT_LIMIT_BYTES + 64 * 1024
COMMAND_TREE_EMPTY_PASSES = 3
CLEANUP_RESERVE_DESCRIPTORS = 8


def load_test_plan() -> dict[str, object]:
    try:
        payload = contract.read_bounded_repository_file(TEST_PLAN)
        value = contract.strict_json_object(
            payload,
            str(TEST_PLAN.relative_to(ROOT)),
        )
    except (OSError, ValueError) as error:
        stop(f"cannot load {TEST_PLAN.relative_to(ROOT)}: {error}")
    if value.get("contract") != "s20-530-crash-recovery-test-plan-v1":
        stop("test-plan identity differs")
    frozen_hashes = contract.require_frozen_contract_integrity()
    expected_contract = contract.canonical_json_sha256(frozen_hashes)
    if value.get("contract_set_sha256") != expected_contract:
        stop("test plan is bound to a stale S20-530 contract")
    reconciler = contract.limit_exception_reconciler_manifest()
    if value.get("limit_exception_reconciler") != reconciler:
        stop("test plan exception-reconciler manifest differs")
    reconciler_sha256 = contract.canonical_json_sha256(reconciler)
    if value.get("limit_exception_reconciler_sha256") != reconciler_sha256:
        stop("test plan exception-reconciler manifest digest differs")
    for manifest_field, digest_field in (
        (
            "limit_event_control_ancestries",
            "limit_event_control_ancestries_sha256",
        ),
        (
            "limit_event_control_exception_ledger",
            "limit_event_control_exception_ledger_sha256",
        ),
        ("limit_event_entry_call_paths", "limit_event_entry_call_paths_sha256"),
        (
            "limit_event_entry_exception_ledger",
            "limit_event_entry_exception_ledger_sha256",
        ),
        ("limit_shared_state_authority", "limit_shared_state_authority_sha256"),
    ):
        manifest = value.get(manifest_field)
        if not isinstance(manifest, dict):
            stop(f"test plan {manifest_field} is not an object")
        if value.get(digest_field) != contract.canonical_json_sha256(manifest):
            stop(f"test plan {digest_field} differs")
    return value


def mapped_tests(
    test_plan: dict[str, object],
) -> tuple[list[str], dict[str, str], dict[str, str], str]:
    row_map = test_plan.get("matrix_test_map")
    if not isinstance(row_map, dict) or tuple(row_map) != contract.MATRIX_IDS:
        stop("matrix test map does not have the exact row order")
    sources = {
        relative: (ROOT / relative).read_text(encoding="utf-8")
        for relative in contract.OWNER_SOURCES
    }
    used: list[str] = []
    owners: dict[str, str] = {}
    qualified: dict[str, str] = {}
    for row_id, entry in row_map.items():
        if row_id in contract.EVIDENCE_FAMILIES:
            if not isinstance(entry, dict):
                stop(f"{row_id} evidence is not an object")
            subcases = entry.get("subcases")
            if (
                not isinstance(subcases, dict)
                or tuple(subcases) != contract.EVIDENCE_FAMILIES[row_id]
            ):
                stop(f"{row_id} subcase order differs")
            if row_id in contract.GROUPED_ERROR_CASES:
                grouped_entries: list[tuple[str, str, object]] = []
                for group_id, group_entry in subcases.items():
                    if not isinstance(group_entry, dict):
                        stop(f"{row_id}/{group_id} evidence is not an object")
                    cases = group_entry.get("cases")
                    expected = tuple(
                        leaf_id
                        for leaf_id, _code, _variant, _chain in (
                            contract.GROUPED_ERROR_CASES[row_id][group_id]
                        )
                    )
                    if not isinstance(cases, dict) or tuple(cases) != expected:
                        stop(f"{row_id}/{group_id} leaf order differs")
                    grouped_entries.extend(
                        (group_id, leaf_id, leaf_entry)
                        for leaf_id, leaf_entry in cases.items()
                    )
                entries = tuple(grouped_entries)
            else:
                entries = tuple(
                    (subcase_id, None, entry) for subcase_id, entry in subcases.items()
                )
        else:
            entries = ((None, None, entry),)
        for subcase_id, leaf_id, subcase in entries:
            owner, exact_qualified = contract.require_evidence_tests(
                row_id, subcase_id, subcase, sources, used, leaf_id
            )
            owners[used[-1]] = contract.crate_for_source(owner)
            qualified[used[-1]] = exact_qualified
    if len(used) != 419:
        stop(f"matrix test map has {len(used)} tests instead of 419")
    body_manifest = contract.mapped_test_body_manifest(used, sources, qualified)
    recorded_body_manifest = test_plan.get("mapped_test_bodies")
    if (
        not isinstance(recorded_body_manifest, dict)
        or tuple(recorded_body_manifest) != tuple(used)
        or recorded_body_manifest != body_manifest
    ):
        stop("mapped test-body review manifest differs")
    for test, record in recorded_body_manifest.items():
        if not isinstance(record, dict) or tuple(record) != (
            "source",
            "qualified_test",
            "body_sha256",
        ):
            stop(f"mapped test-body review fields/order differ for {test}")
    body_digest = contract.canonical_json_sha256(body_manifest)
    recorded_body_digest = contract.canonical_json_sha256(recorded_body_manifest)
    if (
        test_plan.get("mapped_test_bodies_sha256") != body_digest
        or recorded_body_digest != body_digest
    ):
        stop("mapped test-body review-manifest digest differs")
    return used, owners, qualified, body_digest


def tool_file_problem(metadata: os.stat_result) -> str | None:
    if not stat.S_ISREG(metadata.st_mode):
        return "not a regular file"
    if stat.S_IMODE(metadata.st_mode) & 0o022:
        return "group- or world-writable"
    return None


def tool_record(path: Path) -> dict[str, object]:
    resolved = path.resolve(strict=True)
    metadata = resolved.lstat()
    if problem := tool_file_problem(metadata):
        stop(f"tool file is {problem}: {resolved}")
    return {
        "path": str(resolved),
        "uid": metadata.st_uid,
        "gid": metadata.st_gid,
        "mode": f"{stat.S_IMODE(metadata.st_mode):04o}",
        "sha256": hashlib.sha256(resolved.read_bytes()).hexdigest(),
    }


def tool_targets_problem(tools: Mapping[str, object]) -> str | None:
    for name, value in tools.items():
        if not isinstance(value, dict):
            return f"tool record is not an object: {name}"
        path = value.get("path")
        if not isinstance(path, str):
            return f"tool path is not text: {name}"
        try:
            current = tool_record(Path(path))
        except (OSError, RuntimeError) as error:
            return f"cannot revalidate tool target {name}: {error}"
        if current != value:
            return f"tool target differs from its recorded bytes/mode: {name}"
    return None


def require_tool_targets(tools: Mapping[str, object], phase: str) -> None:
    if problem := tool_targets_problem(tools):
        stop(f"{phase}: {problem}")


def rustup_which(rustup: Path, rustup_record: dict[str, object], tool: str) -> Path:
    require_tool_targets({"rustup": rustup_record}, f"before rustup which {tool}")
    result = subprocess.run(
        (str(rustup), "which", tool),
        cwd=ROOT,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env={"RUSTUP_HOME": "/home/dev/.rustup"},
    )
    require_tool_targets({"rustup": rustup_record}, f"after rustup which {tool}")
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        stop(f"cannot resolve {tool} through rustup: {detail}")
    try:
        return Path(result.stdout.decode("utf-8", errors="strict").strip())
    except UnicodeDecodeError as error:
        stop(f"rustup returned a non-UTF-8 {tool} path: {error}")


def execution_tools() -> dict[str, object]:
    rustup = Path("/home/dev/.cargo/bin/rustup").resolve(strict=True)
    rustup_record = tool_record(rustup)
    candidates = {
        "git": contract.GIT,
        "rustup": rustup,
        "cargo": rustup_which(rustup, rustup_record, "cargo"),
        "rustc": rustup_which(rustup, rustup_record, "rustc"),
        "rustdoc": rustup_which(rustup, rustup_record, "rustdoc"),
        "cargo-fmt": rustup_which(rustup, rustup_record, "cargo-fmt"),
        "rustfmt": rustup_which(rustup, rustup_record, "rustfmt"),
        "python3": Path("/usr/bin/python3"),
        "uv": Path("/home/dev/.local/bin/uv"),
        "make": Path("/usr/bin/make"),
        "sh": Path("/bin/sh"),
        "cc": Path("/usr/bin/cc"),
        "as": Path("/usr/bin/as"),
        "ar": Path("/usr/bin/ar"),
        "ld": Path("/usr/bin/ld"),
    }
    if tuple(candidates) != contract.EXECUTION_TOOL_NAMES:
        stop("runner tool-manifest order differs from frozen contract")
    records = {name: tool_record(path) for name, path in candidates.items()}
    require_tool_targets(records, "after tool resolution")
    return records


def dangerous_parent_environment_names(
    environment: Mapping[str, str],
) -> tuple[str, ...]:
    return tuple(
        sorted(
            {
                *contract.FORBIDDEN_PARENT_ENVIRONMENT.intersection(environment),
                *(name for name in environment if name.startswith("GIT_")),
            }
        )
    )


def reject_dangerous_parent_environment() -> None:
    present = dangerous_parent_environment_names(os.environ)
    if present:
        stop(f"forbidden inherited environment variables are present: {present!r}")


def create_tool_bin(runtime_root: Path, tools: dict[str, object]) -> Path:
    tool_bin = runtime_root / "tool-bin"
    tool_bin.mkdir(mode=0o700)
    for name in (
        "cargo",
        "rustc",
        "rustdoc",
        "cargo-fmt",
        "rustfmt",
        "uv",
        "make",
        "sh",
        "cc",
        "as",
        "ar",
        "ld",
    ):
        record_name = "python3" if name == "python" else name
        record = tools[record_name]
        assert isinstance(record, dict)
        path = record["path"]
        assert isinstance(path, str)
        os.symlink(path, tool_bin / name)
    for name in ("python3", "python"):
        path = tool_bin / name
        descriptor = os.open(
            path,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
            0o500,
        )
        try:
            view = memoryview(contract.PYTHON_LAUNCHER)
            while view:
                written = os.write(descriptor, view)
                if written <= 0:
                    stop(f"short write for isolated Python launcher {name}")
                view = view[written:]
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    return tool_bin


def runtime_environment(
    runtime_root: Path, tool_bin: Path, tools: dict[str, object]
) -> dict[str, str]:
    for directory in (
        "cargo-home",
        "cargo-target",
        "home",
        "tmp",
        "uv-cache",
        "uv-environment",
        "xdg-cache",
        "xdg-config",
        "xdg-data",
        "xdg-state",
    ):
        (runtime_root / directory).mkdir(mode=0o700)
    rustc = tools["rustc"]
    rustdoc = tools["rustdoc"]
    python3 = tools["python3"]
    assert isinstance(rustc, dict)
    assert isinstance(rustdoc, dict)
    assert isinstance(python3, dict)
    environment = {
        "CARGO_HOME": str(runtime_root / "cargo-home"),
        "CARGO_INCREMENTAL": "0",
        "CARGO_NET_GIT_FETCH_WITH_CLI": "false",
        "CARGO_TARGET_DIR": str(runtime_root / "cargo-target"),
        "CARGO_TERM_COLOR": "never",
        "HOME": str(runtime_root / "home"),
        "LC_ALL": "C",
        "PATH": str(tool_bin),
        "PYTHONDONTWRITEBYTECODE": "1",
        "PYTHONHASHSEED": "0",
        "PYTHONNOUSERSITE": "1",
        "PYTHONSAFEPATH": "1",
        "RUSTC": str(rustc["path"]),
        "RUSTDOC": str(rustdoc["path"]),
        "RUST_TEST_THREADS": "1",
        "SLEY_S20_530_SEED": "S20_530_DETERMINISTIC_V1",
        "SOURCE_DATE_EPOCH": "0",
        "TMPDIR": str(runtime_root / "tmp"),
        "TZ": "UTC",
        "UV_CACHE_DIR": str(runtime_root / "uv-cache"),
        "UV_FROZEN": "1",
        "UV_NO_CONFIG": "1",
        "UV_NO_PROGRESS": "1",
        "UV_PROJECT_ENVIRONMENT": str(runtime_root / "uv-environment"),
        "UV_PYTHON": str(python3["path"]),
        "XDG_CACHE_HOME": str(runtime_root / "xdg-cache"),
        "XDG_CONFIG_HOME": str(runtime_root / "xdg-config"),
        "XDG_DATA_HOME": str(runtime_root / "xdg-data"),
        "XDG_STATE_HOME": str(runtime_root / "xdg-state"),
    }
    canonical = {
        key: (
            value.replace(str(runtime_root), "<isolated>")
            .replace(str(tool_bin), "<isolated>/tool-bin")
            .replace(str(rustc["path"]), "<tool:rustc>")
            .replace(str(rustdoc["path"]), "<tool:rustdoc>")
            .replace(str(python3["path"]), "<tool:python3>")
        )
        for key, value in environment.items()
    }
    if canonical != contract.EXECUTION_ENVIRONMENT:
        stop("runner environment projection differs from frozen contract")
    return environment


def trusted_runtime_parent() -> Path:
    uid = os.getuid()
    parent = Path(f"/run/user/{uid}")
    for path, expected_uid, exact_mode in (
        (Path("/run"), 0, None),
        (Path("/run/user"), 0, None),
        (parent, uid, 0o700),
    ):
        try:
            metadata = path.lstat()
        except OSError as error:
            stop(f"cannot inspect trusted runtime parent {path}: {error}")
        mode = stat.S_IMODE(metadata.st_mode)
        if (
            not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid != expected_uid
            or mode & 0o022
            or (exact_mode is not None and mode != exact_mode)
        ):
            stop(f"trusted runtime parent is unsafe: {path}")
    return parent


def directory_manifest(root: Path) -> dict[str, dict[str, object]]:
    output: dict[str, dict[str, object]] = {}
    for path in sorted(root.rglob("*")):
        relative = str(path.relative_to(root))
        metadata = path.lstat()
        descriptor: dict[str, object] = {
            "mode": f"{stat.S_IMODE(metadata.st_mode):04o}",
            "uid": metadata.st_uid,
        }
        if stat.S_ISREG(metadata.st_mode):
            descriptor.update(
                kind="regular", sha256=hashlib.sha256(path.read_bytes()).hexdigest()
            )
        elif stat.S_ISLNK(metadata.st_mode):
            descriptor.update(kind="symlink", target=os.readlink(path))
        elif stat.S_ISDIR(metadata.st_mode):
            descriptor.update(kind="directory")
        else:
            stop(f"runtime authority contains a special entry: {path}")
        output[relative] = descriptor
    return output


def path_identity(path: Path) -> tuple[int, int, int, int]:
    metadata = path.lstat()
    if not stat.S_ISDIR(metadata.st_mode):
        stop(f"runtime path is not a directory: {path}")
    return (
        metadata.st_dev,
        metadata.st_ino,
        stat.S_IMODE(metadata.st_mode),
        metadata.st_uid,
    )


def runtime_path_identities(
    runtime_root: Path, snapshot: Path, tool_bin: Path
) -> dict[str, tuple[int, int, int, int]]:
    names = (
        "cargo-home",
        "cargo-target",
        "home",
        "tmp",
        "uv-cache",
        "uv-environment",
        "xdg-cache",
        "xdg-config",
        "xdg-data",
        "xdg-state",
    )
    output = {name: path_identity(runtime_root / name) for name in names}
    output["source"] = path_identity(snapshot)
    output["tool-bin"] = path_identity(tool_bin)
    return output


def cargo_configuration_problem(snapshot: Path, cargo_home: Path) -> str | None:
    candidates: list[Path] = [cargo_home / "config", cargo_home / "config.toml"]
    for ancestor in (snapshot, *snapshot.parents):
        cargo_dir = ancestor / ".cargo"
        try:
            metadata = cargo_dir.lstat()
        except FileNotFoundError:
            continue
        except OSError as error:
            return f"cannot inspect Cargo ancestor {cargo_dir}: {error}"
        if not stat.S_ISDIR(metadata.st_mode):
            return f"Cargo ancestor is not a real directory: {cargo_dir}"
        candidates.extend((cargo_dir / "config", cargo_dir / "config.toml"))
    for candidate in candidates:
        try:
            candidate.lstat()
        except FileNotFoundError:
            continue
        except OSError as error:
            return f"cannot inspect Cargo execution config {candidate}: {error}"
        return f"Cargo execution config is forbidden: {candidate}"
    return None


def snapshot_input_hashes(snapshot: Path) -> dict[str, dict[str, str]]:
    output: dict[str, dict[str, str]] = {}
    for path in sorted(snapshot.rglob("*")):
        relative = str(path.relative_to(snapshot))
        metadata = path.lstat()
        if stat.S_ISDIR(metadata.st_mode):
            continue
        if contract.is_validation_output_path(relative):
            continue
        if not stat.S_ISREG(metadata.st_mode):
            stop(f"snapshot contains a non-regular input: {relative}")
        output[relative] = contract.regular_input_descriptor(
            metadata.st_mode, path.read_bytes()
        )
    return output


def materialize_snapshot(
    revision: str,
    destination: Path,
    expected_inputs: dict[str, dict[str, str]],
    tools: Mapping[str, object],
) -> None:
    if problem := contract.git_archive_arguments_problem(
        contract.GIT_ARCHIVE_ARGUMENTS
    ):
        stop(problem)
    require_tool_targets({"git": tools["git"]}, "before Git archive")
    authority_before = contract.git_local_authority()
    result = subprocess.run(
        (str(contract.GIT), *contract.GIT_ARCHIVE_ARGUMENTS, revision),
        cwd=ROOT,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=contract.GIT_ENVIRONMENT,
    )
    authority_after = contract.git_local_authority()
    if authority_after != authority_before:
        stop("Git local authority changed during Git archive")
    require_tool_targets({"git": tools["git"]}, "after Git archive")
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        stop(f"cannot archive validated commit: {detail}")
    destination.mkdir(mode=0o700)
    try:
        with tarfile.open(fileobj=io.BytesIO(result.stdout), mode="r:") as archive:
            for member in archive.getmembers():
                pure = Path(member.name)
                if pure.is_absolute() or ".." in pure.parts:
                    stop(f"Git archive path escapes snapshot: {member.name!r}")
                if contract.is_validation_output_path(member.name):
                    continue
                target = destination / pure
                if member.isdir():
                    target.mkdir(mode=0o700, parents=True, exist_ok=True)
                    continue
                if not member.isfile():
                    stop(f"Git archive contains a non-regular member: {member.name}")
                target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                source = archive.extractfile(member)
                if source is None:
                    stop(f"cannot read Git archive member: {member.name}")
                target.write_bytes(source.read())
                target.chmod(member.mode & 0o777)
    except (OSError, tarfile.TarError) as error:
        stop(f"cannot materialize validated commit: {error}")
    if snapshot_input_hashes(destination) != expected_inputs:
        stop("materialized snapshot bytes/modes differ from validated commit")


ProcessIdentity = tuple[int, int]
PinnedProcess = tuple[ProcessIdentity, int]


def proc_stat_identity(raw: bytes) -> ProcessIdentity:
    closing = raw.rfind(b") ")
    if closing < 2:
        raise ValueError("missing process-name terminator")
    fields = raw[closing + 2 :].split()
    if len(fields) < 20 or len(fields[0]) != 1:
        raise ValueError("missing process state, parent pid, or start time")
    parent = int(fields[1])
    if parent < 0:
        raise ValueError("negative parent pid")
    start_time = int(fields[19])
    if start_time < 0:
        raise ValueError("negative process start time")
    return parent, start_time


def proc_stat_parent(raw: bytes) -> int:
    return proc_stat_identity(raw)[0]


def proc_snapshot() -> dict[int, ProcessIdentity]:
    identities: dict[int, ProcessIdentity] = {}
    try:
        entries = tuple(Path("/proc").iterdir())
    except OSError as error:
        stop(f"cannot enumerate procfs for descendant cleanup: {error}")
    for entry in entries:
        if not entry.name.isdigit():
            continue
        try:
            identities[int(entry.name)] = proc_stat_identity(
                (entry / "stat").read_bytes()
            )
        except FileNotFoundError:
            continue
        except (PermissionError, OSError, ValueError) as error:
            stop(f"cannot parse procfs identity for pid {entry.name}: {error}")
    return identities


def descendant_identities(
    identities: Mapping[int, ProcessIdentity], root_pid: int
) -> dict[int, ProcessIdentity]:
    descendants: dict[int, ProcessIdentity] = {}
    frontier = {root_pid}
    while frontier:
        found = {
            pid: identity
            for pid, identity in identities.items()
            if identity[0] in frontier and pid not in descendants and pid != root_pid
        }
        descendants.update(found)
        frontier = set(found)
    return dict(sorted(descendants.items()))


def stable_descendant_identities(
    candidates: Mapping[int, ProcessIdentity],
    observed: Mapping[int, ProcessIdentity],
    root_pid: int,
) -> dict[int, ProcessIdentity]:
    unchanged = {
        pid: identity
        for pid, identity in candidates.items()
        if observed.get(pid) == identity
    }
    return descendant_identities(unchanged, root_pid)


def descendant_pids() -> tuple[int, ...]:
    return tuple(descendant_identities(proc_snapshot(), os.getpid()))


def reap_children() -> None:
    while True:
        try:
            waited, _status = os.waitpid(-1, os.WNOHANG)
        except ChildProcessError:
            return
        if waited == 0:
            return


def signal_pidfd(descriptor: int, signal_number: int, identity: str) -> bool:
    try:
        signal.pidfd_send_signal(descriptor, signal_number, None, 0)
    except ProcessLookupError:
        return False
    except OSError as error:
        stop(f"cannot signal pinned {identity}: {error}")
    return True


def pin_descendants(
    excluded: frozenset[tuple[int, int]],
) -> dict[int, PinnedProcess]:
    before = proc_snapshot()
    candidates = {
        pid: identity
        for pid, identity in descendant_identities(before, os.getpid()).items()
        if (pid, identity[1]) not in excluded
    }
    opened: dict[int, PinnedProcess] = {}
    result: dict[int, PinnedProcess] = {}
    try:
        for pid, identity in candidates.items():
            try:
                descriptor = os.pidfd_open(pid, 0)
            except ProcessLookupError:
                continue
            except OSError as error:
                if error.errno in {errno.EMFILE, errno.ENFILE} and opened:
                    _last_pid, (_last_identity, last_descriptor) = opened.popitem()
                    os.close(last_descriptor)
                    break
                if error.errno in {errno.EMFILE, errno.ENFILE} and excluded:
                    return {}
                stop(f"cannot pin command descendant pid {pid}: {error}")
            opened[pid] = identity, descriptor
        retained = stable_descendant_identities(
            candidates, proc_snapshot(), os.getpid()
        )
        for pid in tuple(opened):
            identity, descriptor = opened.pop(pid)
            if retained.get(pid) == identity:
                result[pid] = identity, descriptor
            else:
                os.close(descriptor)
        return result
    except BaseException:
        for _identity, descriptor in (*opened.values(), *result.values()):
            os.close(descriptor)
        raise


def extend_pinned_descendants(
    pinned: dict[tuple[int, int], PinnedProcess],
) -> None:
    for pid, (identity, descriptor) in pin_descendants(frozenset(pinned)).items():
        key = pid, identity[1]
        if key in pinned:
            os.close(descriptor)
        else:
            pinned[key] = identity, descriptor


def signal_pinned_descendants(
    pinned: Mapping[tuple[int, int], PinnedProcess], signal_number: int
) -> None:
    for (pid, start_time), (_identity, descriptor) in pinned.items():
        signal_pidfd(
            descriptor,
            signal_number,
            f"command descendant pid {pid} start {start_time}",
        )


def close_pinned_descendants(
    pinned: Mapping[tuple[int, int], PinnedProcess],
) -> None:
    for _identity, descriptor in pinned.values():
        os.close(descriptor)


def pidfd_exited(descriptor: int) -> bool:
    poller = select.poll()
    poller.register(descriptor, select.POLLIN | select.POLLHUP | select.POLLERR)
    return bool(poller.poll(0))


def prune_exited_pinned_descendants(
    pinned: dict[tuple[int, int], PinnedProcess],
) -> None:
    exited = [
        key
        for key, (_identity, descriptor) in pinned.items()
        if pidfd_exited(descriptor)
    ]
    if not exited:
        return
    reap_children()
    for key in exited:
        _identity, descriptor = pinned.pop(key)
        os.close(descriptor)


def command_tree_empty_pass(
    process: subprocess.Popen[bytes],
    pinned: dict[tuple[int, int], PinnedProcess],
    signal_number: int,
) -> bool:
    if not pinned:
        extend_pinned_descendants(pinned)
    if pinned:
        signal_pinned_descendants(pinned, signal_number)
    process.poll()
    if process.returncode is None:
        return False
    reap_children()
    prune_exited_pinned_descendants(pinned)
    if not pinned:
        extend_pinned_descendants(pinned)
        signal_pinned_descendants(pinned, signal_number)
        reap_children()
        prune_exited_pinned_descendants(pinned)
    return not pinned and not descendant_pids()


def terminate_command_tree(
    process: subprocess.Popen[bytes],
    leader_pidfd: int | None,
    *,
    terminate_leader: bool,
) -> None:
    pinned: dict[tuple[int, int], PinnedProcess] = {}
    try:
        if terminate_leader and leader_pidfd is not None:
            signal_pidfd(leader_pidfd, signal.SIGTERM, "command leader")
        term_deadline = time.monotonic() + 0.25
        empty_passes = 0
        while True:
            if command_tree_empty_pass(process, pinned, signal.SIGTERM):
                empty_passes += 1
                if empty_passes >= COMMAND_TREE_EMPTY_PASSES:
                    return
            else:
                empty_passes = 0
            if time.monotonic() >= term_deadline:
                break
            time.sleep(0.01)

        if leader_pidfd is not None:
            signal_pidfd(leader_pidfd, signal.SIGKILL, "command leader")
        kill_deadline = time.monotonic() + 2
        empty_passes = 0
        while True:
            if command_tree_empty_pass(process, pinned, signal.SIGKILL):
                empty_passes += 1
                if empty_passes >= COMMAND_TREE_EMPTY_PASSES:
                    return
            else:
                empty_passes = 0
            if time.monotonic() >= kill_deadline:
                if process.returncode is None:
                    stop("pinned command leader survived forced cleanup")
                survivors = descendant_pids()
                pinned_identities = tuple(sorted(pinned))
                stop(
                    "command descendants survived forced cleanup: "
                    f"proc={survivors!r}, pinned={pinned_identities!r}"
                )
            time.sleep(0.01)
    finally:
        close_pinned_descendants(pinned)


def terminate_unpinned_unreaped_command(process: subprocess.Popen[bytes]) -> None:
    process.poll()
    if process.returncode is None:
        # A direct, unreaped child owns this numeric PID until wait() completes.
        process.send_signal(signal.SIGKILL)
        try:
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            stop("unreaped command leader survived SIGKILL")
    terminate_command_tree(process, None, terminate_leader=False)


def setup_failure_ready(setup_failure: tuple[str, Path]) -> None:
    mode, ready = setup_failure
    if mode not in {"pidfd-emfile", "selector-emfile"}:
        stop(f"unknown bounded-capture setup failure control: {mode}")
    deadline = time.monotonic() + 6
    while not ready.exists():
        if time.monotonic() >= deadline:
            stop(f"bounded-capture setup control did not become ready: {mode}")
        time.sleep(0.01)


def open_cleanup_reserve() -> list[int]:
    descriptors: list[int] = []
    try:
        for _index in range(CLEANUP_RESERVE_DESCRIPTORS):
            descriptors.append(os.open("/dev/null", os.O_RDONLY | os.O_CLOEXEC))
    except OSError as error:
        while descriptors:
            os.close(descriptors.pop())
        stop(f"cannot reserve command-cleanup descriptors before spawn: {error}")
    return descriptors


def close_descriptors(descriptors: list[int]) -> None:
    while descriptors:
        os.close(descriptors.pop())


def release_reserve_slot(descriptors: list[int], purpose: str) -> None:
    if not descriptors:
        stop(f"command-cleanup descriptor reserve exhausted before {purpose}")
    os.close(descriptors.pop())


def exhaust_file_descriptors() -> list[int]:
    descriptors: list[int] = []
    while True:
        try:
            descriptors.append(os.open("/dev/null", os.O_RDONLY | os.O_CLOEXEC))
        except OSError as error:
            if error.errno not in {errno.EMFILE, errno.ENFILE}:
                close_descriptors(descriptors)
                raise
            return descriptors


def open_fd_snapshot() -> dict[int, str]:
    snapshot: dict[int, str] = {}
    try:
        names = os.listdir("/proc/self/fd")
    except OSError as error:
        stop(f"cannot enumerate runner file descriptors: {error}")
    for name in names:
        if not name.isdigit():
            continue
        try:
            snapshot[int(name)] = os.readlink(f"/proc/self/fd/{name}")
        except FileNotFoundError:
            continue
        except OSError as error:
            stop(f"cannot inspect runner file descriptor {name}: {error}")
    return snapshot


def bounded_capture(
    arguments: list[str],
    cwd: Path,
    environment: dict[str, str],
    *,
    timeout_seconds: float,
    output_limit_bytes: int,
    setup_failure: tuple[str, Path] | None = None,
) -> tuple[int, float, bytes, str | None]:
    reap_children()
    if descendants := descendant_pids():
        stop(f"pre-existing runner descendants are forbidden: {descendants!r}")
    started = time.monotonic()
    cleanup_reserve = open_cleanup_reserve()
    try:
        process = subprocess.Popen(
            arguments,
            cwd=cwd,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            env=environment,
            start_new_session=True,
            close_fds=True,
        )
    except BaseException:
        close_descriptors(cleanup_reserve)
        raise
    leader_pidfd: int | None = None
    selector: selectors.BaseSelector | None = None
    exhausted_descriptors: list[int] = []
    cleanup_complete = False
    try:
        output = bytearray()
        eof = False
        problem: str | None = None
        if setup_failure is not None and setup_failure[0] == "pidfd-emfile":
            setup_failure_ready(setup_failure)
            exhausted_descriptors = exhaust_file_descriptors()
        else:
            release_reserve_slot(cleanup_reserve, "leader pidfd acquisition")
        leader_pidfd = os.pidfd_open(process.pid, 0)
        if setup_failure is not None and setup_failure[0] == "selector-emfile":
            setup_failure_ready(setup_failure)
            exhausted_descriptors = exhaust_file_descriptors()
        else:
            release_reserve_slot(cleanup_reserve, "selector acquisition")
        if process.stdout is None:
            stop("captured command lacks a stdout pipe")
        selector = selectors.DefaultSelector()
        selector.register(process.stdout, selectors.EVENT_READ)
        while process.poll() is None or not eof:
            elapsed = time.monotonic() - started
            if elapsed > timeout_seconds:
                problem = f"timeout after {timeout_seconds:g} seconds"
                break
            events = selector.select(timeout=min(0.1, timeout_seconds - elapsed))
            for key, _mask in events:
                allowance = output_limit_bytes - len(output)
                chunk = os.read(key.fd, max(1, min(65_536, allowance + 1)))
                if not chunk:
                    selector.unregister(key.fileobj)
                    eof = True
                    continue
                output.extend(chunk)
                if len(output) > output_limit_bytes:
                    del output[output_limit_bytes:]
                    problem = f"output exceeds {output_limit_bytes} bytes"
                    break
            if problem is not None:
                break
        if problem is not None:
            close_descriptors(cleanup_reserve)
            terminate_command_tree(process, leader_pidfd, terminate_leader=True)
            cleanup_complete = True
            return (
                process.returncode
                if process.returncode is not None
                else -signal.SIGKILL,
                round(time.monotonic() - started, 6),
                bytes(output),
                problem,
            )
        returncode = process.wait(timeout=5)
        close_descriptors(cleanup_reserve)
        terminate_command_tree(process, leader_pidfd, terminate_leader=False)
        cleanup_complete = True
        return returncode, round(time.monotonic() - started, 6), bytes(output), None
    finally:
        if selector is not None:
            selector.close()
        if process.stdout is not None:
            process.stdout.close()
        close_descriptors(cleanup_reserve)
        try:
            if not cleanup_complete:
                if leader_pidfd is None:
                    terminate_unpinned_unreaped_command(process)
                else:
                    terminate_command_tree(process, leader_pidfd, terminate_leader=True)
        finally:
            close_descriptors(exhausted_descriptors)
            if leader_pidfd is not None:
                os.close(leader_pidfd)


def output_directory_descriptor(path: Path, *, create: bool, root: Path = ROOT) -> int:
    try:
        relative = path.relative_to(root)
    except ValueError:
        raise ValueError(f"output directory escapes repository: {path}") from None
    if any(part in {"", ".", ".."} for part in relative.parts):
        raise ValueError(f"output directory has an unsafe component: {path}")
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    descriptor = os.open(root, flags)
    try:
        root_metadata = os.fstat(descriptor)
        if (
            not stat.S_ISDIR(root_metadata.st_mode)
            or root_metadata.st_uid != os.getuid()
            or root_metadata.st_gid != os.getgid()
            or stat.S_IMODE(root_metadata.st_mode) & 0o002
        ):
            raise ValueError(f"output root is unsafe: {root}")
        for part in relative.parts:
            try:
                child = os.open(part, flags, dir_fd=descriptor)
            except FileNotFoundError:
                if not create:
                    raise
                try:
                    os.mkdir(part, mode=0o700, dir_fd=descriptor)
                except FileExistsError:
                    pass
                child = os.open(part, flags, dir_fd=descriptor)
            metadata = os.fstat(child)
            if (
                not stat.S_ISDIR(metadata.st_mode)
                or metadata.st_uid != os.getuid()
                or metadata.st_gid != os.getgid()
                or stat.S_IMODE(metadata.st_mode) & 0o002
            ):
                os.close(child)
                raise ValueError(f"output directory component is unsafe: {part}")
            os.close(descriptor)
            descriptor = child
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def open_output_directory(
    path: Path, *, root: Path = ROOT
) -> tuple[int, tuple[int, int]]:
    try:
        descriptor = output_directory_descriptor(path, create=True, root=root)
    except (OSError, ValueError) as error:
        stop(f"cannot open output directory {path}: {error}")
    metadata = os.fstat(descriptor)
    return descriptor, (metadata.st_dev, metadata.st_ino)


def directory_handle_matches(
    path: Path, identity: tuple[int, int], *, root: Path = ROOT
) -> bool:
    try:
        descriptor = output_directory_descriptor(path, create=False, root=root)
    except (OSError, ValueError):
        return False
    try:
        metadata = os.fstat(descriptor)
        return (metadata.st_dev, metadata.st_ino) == identity
    finally:
        os.close(descriptor)


def safe_output_basename(name: str) -> bool:
    return Path(name).name == name and name not in {"", ".", ".."}


def regular_output_entry_problem(directory_fd: int, name: str) -> str | None:
    if not safe_output_basename(name):
        return f"unsafe output basename: {name!r}"
    try:
        metadata = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
    except FileNotFoundError:
        return None
    except OSError as error:
        return f"cannot inspect output {name}: {error}"
    if not stat.S_ISREG(metadata.st_mode):
        return f"output is not a regular file: {name}"
    if metadata.st_uid != os.getuid():
        return f"output is not owned by the validation uid: {name}"
    if metadata.st_gid != os.getgid():
        return f"output is not owned by the validation gid: {name}"
    if stat.S_IMODE(metadata.st_mode) & 0o002:
        return f"output is world-writable: {name}"
    if metadata.st_size > OUTPUT_READ_LIMIT_BYTES:
        return f"output exceeds {OUTPUT_READ_LIMIT_BYTES} bytes: {name}"
    return None


def read_at(directory_fd: int, name: str) -> bytes:
    if not safe_output_basename(name):
        stop(f"unsafe output basename: {name!r}")
    descriptor = os.open(
        name,
        os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC,
        dir_fd=directory_fd,
    )
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            stop(f"output is not a regular file: {name}")
        if before.st_uid != os.getuid():
            stop(f"output is not owned by the validation uid: {name}")
        if before.st_gid != os.getgid():
            stop(f"output is not owned by the validation gid: {name}")
        if stat.S_IMODE(before.st_mode) & 0o002:
            stop(f"output is world-writable: {name}")
        if before.st_size > OUTPUT_READ_LIMIT_BYTES:
            stop(f"output exceeds {OUTPUT_READ_LIMIT_BYTES} bytes: {name}")
        content = bytearray()
        while True:
            chunk = os.read(
                descriptor,
                OUTPUT_READ_LIMIT_BYTES + 1 - len(content),
            )
            if not chunk:
                break
            content.extend(chunk)
            if len(content) > OUTPUT_READ_LIMIT_BYTES:
                stop(f"output exceeds {OUTPUT_READ_LIMIT_BYTES} bytes: {name}")
        after = os.fstat(descriptor)
        stable_fields = (
            "st_dev",
            "st_ino",
            "st_mode",
            "st_uid",
            "st_gid",
            "st_size",
            "st_mtime_ns",
            "st_ctime_ns",
        )
        if any(
            getattr(before, field) != getattr(after, field) for field in stable_fields
        ):
            stop(f"output changed while it was read: {name}")
        if len(content) != before.st_size:
            stop(f"output length differs from its metadata: {name}")
        return bytes(content)
    finally:
        os.close(descriptor)


def atomic_write_at(directory_fd: int, name: str, payload: bytes) -> None:
    if not safe_output_basename(name):
        stop(f"unsafe output basename: {name!r}")
    if len(payload) > OUTPUT_READ_LIMIT_BYTES:
        stop(f"output payload exceeds {OUTPUT_READ_LIMIT_BYTES} bytes: {name}")
    if problem := regular_output_entry_problem(directory_fd, name):
        stop(problem)
    temporary = f".{name}.{secrets.token_hex(16)}.tmp"
    descriptor = os.open(
        temporary,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
        0o600,
        dir_fd=directory_fd,
    )
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                stop(f"short write for output {name}")
            view = view[written:]
        os.fsync(descriptor)
    except BaseException:
        try:
            os.unlink(temporary, dir_fd=directory_fd)
        except OSError:
            pass
        raise
    finally:
        os.close(descriptor)
    os.replace(
        temporary,
        name,
        src_dir_fd=directory_fd,
        dst_dir_fd=directory_fd,
    )
    os.fsync(directory_fd)


def preflight_log_directory(directory_fd: int) -> None:
    allowed = {
        Path(relative).name for relative in contract.expected_validation_log_paths()
    }
    for name in os.listdir(directory_fd):
        if name not in allowed:
            stop(f"unexpected validation-log output entry: {name}")
        if problem := regular_output_entry_problem(directory_fd, name):
            stop(problem)


def verify_completed_logs(directory_fd: int, expected: dict[str, str]) -> None:
    preflight_log_directory(directory_fd)
    for name, digest in expected.items():
        if problem := regular_output_entry_problem(directory_fd, name):
            stop(problem)
        try:
            payload = read_at(directory_fd, name)
        except FileNotFoundError:
            stop(f"completed validation log disappeared: {name}")
        if hashlib.sha256(payload).hexdigest() != digest:
            stop(f"completed validation log changed: {name}")


def run_captured(
    command: str,
    snapshot: Path,
    environment: dict[str, str],
    execution_profile_sha256: str,
    log_directory_fd: int,
    log_name: str,
) -> tuple[int, float, str, bytes]:
    exit_status, duration, output, problem = bounded_capture(
        shlex.split(command),
        snapshot,
        environment,
        timeout_seconds=contract.COMMAND_TIMEOUT_SECONDS,
        output_limit_bytes=contract.COMMAND_OUTPUT_LIMIT_BYTES,
    )
    if problem is not None:
        stop(f"bounded command failed before evidence recording: {command}: {problem}")
    header = (
        f"$ {command}\n"
        f"execution_profile_sha256: {execution_profile_sha256}\n"
        f"exit_status: {exit_status}\n"
    ).encode("utf-8")
    payload = header + output
    atomic_write_at(log_directory_fd, log_name, payload)
    return exit_status, duration, hashlib.sha256(payload).hexdigest(), output


def write_evidence(evidence: dict[str, object], directory_fd: int) -> None:
    encoded = (
        json.dumps(evidence, indent=2, ensure_ascii=False, allow_nan=False) + "\n"
    ).encode()
    atomic_write_at(directory_fd, EVIDENCE.name, encoded)


def run_in_fresh_runtime(
    command: str,
    revision: str,
    inputs: dict[str, dict[str, str]],
    tools: dict[str, object],
    execution_profile_sha256: str,
    runtime_parent: Path,
    log_directory_fd: int,
    log_name: str,
) -> tuple[int, float, str, bytes]:
    with tempfile.TemporaryDirectory(
        prefix=".sley-s20-530-", dir=runtime_parent
    ) as temporary_root:
        runtime_root = Path(temporary_root)
        metadata = runtime_root.lstat()
        if (
            not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid != os.getuid()
            or stat.S_IMODE(metadata.st_mode) != 0o700
        ):
            stop("per-command runtime root is unsafe")
        snapshot = runtime_root / "source"
        materialize_snapshot(revision, snapshot, inputs, tools)
        if problem := contract.workspace_execution_input_problem(snapshot, inputs):
            stop(f"materialized execution-input closure differs: {problem}")
        tool_bin = create_tool_bin(runtime_root, tools)
        environment = runtime_environment(runtime_root, tool_bin, tools)
        tool_manifest = directory_manifest(tool_bin)
        identities = runtime_path_identities(runtime_root, snapshot, tool_bin)
        if problem := cargo_configuration_problem(
            snapshot, runtime_root / "cargo-home"
        ):
            stop(problem)
        require_tool_targets(tools, f"before command {command}")
        result = run_captured(
            command,
            snapshot,
            environment,
            execution_profile_sha256,
            log_directory_fd,
            log_name,
        )
        require_tool_targets(tools, f"after command {command}")
        if snapshot_input_hashes(snapshot) != inputs:
            stop(f"source snapshot changed during command execution: {command}")
        if problem := contract.workspace_execution_input_problem(snapshot, inputs):
            stop(f"post-command execution-input closure differs: {problem}")
        if directory_manifest(tool_bin) != tool_manifest:
            stop(f"tool-bin authority changed during command execution: {command}")
        if runtime_path_identities(runtime_root, snapshot, tool_bin) != identities:
            stop(f"runtime path identity changed during command execution: {command}")
        if problem := cargo_configuration_problem(
            snapshot, runtime_root / "cargo-home"
        ):
            stop(f"command created a forbidden execution authority: {problem}")
        return result


def require_runner_negative_controls(runtime_parent: Path) -> None:
    if problem := contract.git_archive_arguments_problem(
        contract.GIT_ARCHIVE_ARGUMENTS
    ):
        stop(problem)
    for hostile_arguments in (
        ("archive", "--format=tar"),
        ("-c", "tar.umask=0002", "archive", "--format=tar"),
        ("archive", "-c", "tar.umask=0022", "--format=tar"),
        ("-c", "tar.umask=0022", "archive", "--format=zip"),
    ):
        if contract.git_archive_arguments_problem(hostile_arguments) is None:
            stop("runner self-test accepted hostile Git archive arguments")
    forbidden = dangerous_parent_environment_names(
        {"TMPDIR": "/attacker", "GIT_INDEX_FILE": "/attacker/index"}
    )
    if forbidden != ("GIT_INDEX_FILE", "TMPDIR"):
        stop("runner self-test did not reject inherited temp/Git substitution")
    try:
        proc_stat_parent(b"malformed proc stat")
    except ValueError:
        pass
    else:
        stop("runner self-test accepted malformed procfs ancestry")
    stale_candidates = {
        4_101: (4_000, 101),
        4_102: (4_101, 102),
        4_103: (4_000, 103),
    }
    changed_observations = {
        4_101: (4_000, 101),
        4_102: (4_101, 202),
        4_103: (1, 103),
    }
    if stable_descendant_identities(stale_candidates, changed_observations, 4_000) != {
        4_101: (4_000, 101)
    }:
        stop("runner self-test retained a reused or reparented process identity")

    environment = {"LC_ALL": "C"}
    timeout_result = bounded_capture(
        [
            str(contract.PYTHON),
            "-I",
            "-B",
            "-c",
            "import time; time.sleep(1)",
        ],
        ROOT,
        environment,
        timeout_seconds=0.05,
        output_limit_bytes=1_024,
    )
    if timeout_result[3] != "timeout after 0.05 seconds":
        stop("runner self-test did not terminate a timed-out process group")
    output_result = bounded_capture(
        [
            str(contract.PYTHON),
            "-I",
            "-B",
            "-c",
            "import os; os.write(1, b'x' * 129)",
        ],
        ROOT,
        environment,
        timeout_seconds=5,
        output_limit_bytes=128,
    )
    if output_result[3] != "output exceeds 128 bytes":
        stop("runner self-test did not terminate oversized output")

    with tempfile.TemporaryDirectory(
        prefix=".sley-s20-530-control-", dir=runtime_parent
    ) as temporary_root:
        fixture = Path(temporary_root)
        source = fixture / "source"
        source.mkdir(mode=0o700)
        cargo_dir = fixture / ".cargo"
        cargo_dir.mkdir(mode=0o700)
        (cargo_dir / "config.toml").write_text("[build]\nrustc-wrapper='fake'\n")
        cargo_home = fixture / "cargo-home"
        cargo_home.mkdir(mode=0o700)
        problem = cargo_configuration_problem(source, cargo_home)
        if problem is None or "config.toml" not in problem:
            stop("runner self-test accepted an ancestor Cargo config")

        import_root = fixture / "imports"
        import_root.mkdir(mode=0o700)
        sentinel = fixture / "user-site-sentinel"
        (import_root / "sitecustomize.py").write_text(
            f"from pathlib import Path\nPath({str(sentinel)!r}).write_text('bad')\n"
        )
        isolated_result = bounded_capture(
            [str(contract.PYTHON), "-I", "-B", "-c", "print('isolated')"],
            ROOT,
            {"LC_ALL": "C", "PYTHONPATH": str(import_root)},
            timeout_seconds=5,
            output_limit_bytes=1_024,
        )
        if isolated_result[0] != 0 or isolated_result[2] != b"isolated\n":
            stop("runner self-test rejected isolated Python execution")
        if sentinel.exists():
            stop("runner self-test executed injected Python user-site code")

        for setup_mode in ("pidfd-emfile", "selector-emfile"):
            setup_ready = fixture / f"{setup_mode}-ready"
            setup_late_sentinel = fixture / f"{setup_mode}-late-sentinel"
            child_readies = tuple(
                fixture / f"{setup_mode}-child-{index}-ready" for index in range(12)
            )
            setup_child_codes = tuple(
                "\n".join(
                    (
                        "import os, signal, time",
                        "from pathlib import Path",
                        "if os.fork(): os._exit(0)",
                        "os.setsid()",
                        "if os.fork(): os._exit(0)",
                        "signal.signal(signal.SIGTERM, signal.SIG_IGN)",
                        "for descriptor in range(256):",
                        "    try: os.close(descriptor)",
                        "    except OSError: pass",
                        f"Path({str(child_ready)!r}).write_text('ready')",
                        "time.sleep(0.6)",
                        f"Path({str(setup_late_sentinel)!r}).write_text('escaped')",
                    )
                )
                for child_ready in child_readies
            )
            spawn_statements = " ".join(
                (
                    "subprocess.Popen([sys.executable,'-I','-B','-c',"
                    f"{child_code!r}], stdin=subprocess.DEVNULL, "
                    "stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, "
                    "close_fds=True);"
                )
                for child_code in setup_child_codes
            )
            setup_leader_code = (
                "import subprocess,sys,time; from pathlib import Path; "
                f"readies={tuple(str(path) for path in child_readies)!r}; "
                f"{spawn_statements} "
                "deadline=time.monotonic()+5; "
                'exec("while not all(Path(path).exists() for path in readies) '
                "and time.monotonic() < deadline:\\n"
                '    time.sleep(0.01)"); '
                "assert all(Path(path).exists() for path in readies); "
                f"Path({str(setup_ready)!r}).write_text('ready'); time.sleep(10)"
            )
            descriptors_before = open_fd_snapshot()
            previous_limits = resource.getrlimit(resource.RLIMIT_NOFILE)
            previous_soft, previous_hard = previous_limits
            control_soft = (
                64
                if previous_soft == resource.RLIM_INFINITY
                else min(previous_soft, 64)
            )
            if control_soft < 32:
                stop("runner self-test lacks descriptor room for EMFILE control")
            resource.setrlimit(
                resource.RLIMIT_NOFILE,
                (control_soft, previous_hard),
            )
            try:
                try:
                    bounded_capture(
                        [
                            str(contract.PYTHON),
                            "-I",
                            "-B",
                            "-c",
                            setup_leader_code,
                        ],
                        ROOT,
                        environment,
                        timeout_seconds=5,
                        output_limit_bytes=1_024,
                        setup_failure=(setup_mode, setup_ready),
                    )
                except OSError as error:
                    if error.errno not in {errno.EMFILE, errno.ENFILE}:
                        stop(f"runner self-test received wrong {setup_mode} failure")
                else:
                    stop(f"runner self-test did not reach real {setup_mode}")
            finally:
                resource.setrlimit(resource.RLIMIT_NOFILE, previous_limits)
            time.sleep(0.65)
            if setup_late_sentinel.exists() or descendant_pids():
                stop(f"runner self-test leaked a {setup_mode} descendant")
            if open_fd_snapshot() != descriptors_before:
                stop(f"runner self-test leaked a {setup_mode} file descriptor")

        for control_name, leader_action, timeout_seconds, output_limit, expected in (
            ("normal-exit", "pass", 5.0, 1_024, None),
            ("timeout", "time.sleep(1)", 0.1, 1_024, "timeout after 0.1 seconds"),
            (
                "output-limit",
                "os.write(1, b'x' * 129)",
                5.0,
                128,
                "output exceeds 128 bytes",
            ),
        ):
            ready = fixture / f"{control_name}-ready"
            late_sentinel = fixture / f"{control_name}-late-sentinel"
            child_code = "\n".join(
                (
                    "import os, signal, time",
                    "from pathlib import Path",
                    "if os.fork(): os._exit(0)",
                    "os.setsid()",
                    "if os.fork(): os._exit(0)",
                    "signal.signal(signal.SIGTERM, signal.SIG_IGN)",
                    "for descriptor in range(256):",
                    "    try: os.close(descriptor)",
                    "    except OSError: pass",
                    f"Path({str(ready)!r}).write_text('ready')",
                    "time.sleep(0.6)",
                    f"Path({str(late_sentinel)!r}).write_text('escaped')",
                )
            )
            leader_code = (
                "import os,subprocess,sys,time; from pathlib import Path; "
                f"ready=Path({str(ready)!r}); "
                "subprocess.Popen([sys.executable,'-I','-B','-c',"
                f"{child_code!r}], stdin=subprocess.DEVNULL, "
                "stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, "
                "close_fds=True); "
                "deadline=time.monotonic()+2; "
                'exec("while not ready.exists() and time.monotonic() < deadline:\\n'
                '    time.sleep(0.01)"); '
                "assert ready.exists(); "
                f"{leader_action}"
            )
            descendant_result = bounded_capture(
                [str(contract.PYTHON), "-I", "-B", "-c", leader_code],
                ROOT,
                environment,
                timeout_seconds=timeout_seconds,
                output_limit_bytes=output_limit,
            )
            if expected is None and (
                descendant_result[0] != 0 or descendant_result[3] is not None
            ):
                stop("runner self-test rejected normal descendant cleanup")
            if expected is not None and descendant_result[3] != expected:
                stop(f"runner self-test rejected {control_name} descendant cleanup")
            time.sleep(0.65)
            if late_sentinel.exists() or descendant_pids():
                stop(f"runner self-test leaked a {control_name} descendant")

        fork_ready = fixture / "fork-on-term-ready"
        fork_late_sentinel = fixture / "fork-on-term-late-sentinel"
        fork_child_code = "\n".join(
            (
                "import os, signal, time",
                "from pathlib import Path",
                "if os.fork(): os._exit(0)",
                "os.setsid()",
                "if os.fork(): os._exit(0)",
                "def on_term(_signum, _frame):",
                "    if os.fork() == 0:",
                "        signal.signal(signal.SIGTERM, signal.SIG_IGN)",
                "        time.sleep(0.6)",
                f"        Path({str(fork_late_sentinel)!r}).write_text('escaped')",
                "        os._exit(0)",
                "    os._exit(0)",
                "signal.signal(signal.SIGTERM, on_term)",
                "for descriptor in range(256):",
                "    try: os.close(descriptor)",
                "    except OSError: pass",
                f"Path({str(fork_ready)!r}).write_text('ready')",
                "time.sleep(10)",
            )
        )
        fork_leader_code = (
            "import subprocess,sys,time; from pathlib import Path; "
            f"ready=Path({str(fork_ready)!r}); "
            "subprocess.Popen([sys.executable,'-I','-B','-c',"
            f"{fork_child_code!r}], stdin=subprocess.DEVNULL, "
            "stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, "
            "close_fds=True); "
            "deadline=time.monotonic()+2; "
            'exec("while not ready.exists() and time.monotonic() < deadline:\\n'
            '    time.sleep(0.01)"); '
            "assert ready.exists()"
        )
        fork_result = bounded_capture(
            [str(contract.PYTHON), "-I", "-B", "-c", fork_leader_code],
            ROOT,
            environment,
            timeout_seconds=5,
            output_limit_bytes=1_024,
        )
        if fork_result[0] != 0 or fork_result[3] is not None:
            stop("runner self-test rejected fork-on-TERM cleanup")
        time.sleep(0.65)
        if fork_late_sentinel.exists() or descendant_pids():
            stop("runner self-test leaked a fork-on-TERM descendant")

        output_dir = fixture / "outputs"
        output_dir.mkdir(mode=0o700)
        output_fd = os.open(output_dir, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            os.symlink("target", output_dir / "result.json")
            if regular_output_entry_problem(output_fd, "result.json") is None:
                stop("runner self-test accepted a symlink output target")
            os.mkfifo(output_dir / "pipe")
            if regular_output_entry_problem(output_fd, "pipe") is None:
                stop("runner self-test accepted a FIFO output target")
            oversized = output_dir / "oversized"
            oversized.touch()
            os.truncate(oversized, OUTPUT_READ_LIMIT_BYTES + 1)
            if regular_output_entry_problem(output_fd, "oversized") is None:
                stop("runner self-test accepted an oversized output target")
        finally:
            os.close(output_fd)

        symlink_target = fixture / "symlink-target"
        symlink_target.mkdir(mode=0o700)
        symlink_component = fixture / "symlink-component"
        symlink_component.symlink_to(symlink_target, target_is_directory=True)
        try:
            escaped_fd = output_directory_descriptor(
                symlink_component / "nested", create=True, root=fixture
            )
        except (OSError, ValueError):
            escaped_fd = None
        if escaped_fd is not None:
            os.close(escaped_fd)
            stop("runner self-test accepted an intermediate output symlink")
        if (symlink_target / "nested").exists():
            stop("runner self-test created output through an intermediate symlink")

        stable_output = fixture / "stable-output"
        stable_output.mkdir(mode=0o700)
        stable_fd, stable_identity = open_output_directory(stable_output, root=fixture)
        os.close(stable_fd)
        moved_output = fixture / "moved-output"
        stable_output.rename(moved_output)
        stable_output.mkdir(mode=0o700)
        if directory_handle_matches(stable_output, stable_identity, root=fixture):
            stop("runner self-test accepted a substituted output directory")

        authority_dir = fixture / "tool-authority"
        authority_dir.mkdir(mode=0o700)
        authority = authority_dir / "cargo"
        authority.write_bytes(b"first")
        authority.chmod(0o600)
        authority_record = tool_record(authority)
        before = directory_manifest(authority_dir)
        authority.write_bytes(b"second")
        if directory_manifest(authority_dir) == before:
            stop("runner self-test missed generated tool-authority mutation")
        if tool_targets_problem({"cargo": authority_record}) is None:
            stop("runner self-test accepted mutated recorded tool-target bytes")
        authority.chmod(0o664)
        if tool_file_problem(authority.lstat()) != "group- or world-writable":
            stop("runner self-test accepted a group-writable tool file")


def require_git_archive_materialization_control(runtime_parent: Path) -> None:
    revision, _tree = contract.current_git_revision()
    inputs = contract.commit_input_hashes(revision)
    git_tools = {"git": tool_record(contract.GIT)}
    with tempfile.TemporaryDirectory(
        prefix=".sley-s20-530-archive-control-", dir=runtime_parent
    ) as temporary_root:
        destination = Path(temporary_root) / "source"
        materialize_snapshot(revision, destination, inputs, git_tools)


def main() -> None:
    reject_dangerous_parent_environment()
    frozen_hashes = contract.require_frozen_contract_integrity()
    spec = contract.SPEC.read_text(encoding="utf-8")
    adr = contract.ADR.read_text(encoding="utf-8")
    contract.require_contract(spec, adr)
    contract.require_dependency_direction()
    contract_set_sha256 = contract.canonical_json_sha256(frozen_hashes)
    test_plan = load_test_plan()
    tests, owners, qualified, mapped_test_bodies_sha256 = mapped_tests(test_plan)

    tools = execution_tools()
    git_authority = contract.git_local_authority()
    require_tool_targets({"git": tools["git"]}, "before Git source binding")
    dirty = contract.workspace_input_dirtiness()
    if dirty:
        stop(f"commit every validation input before running: {dirty!r}")
    if flags := contract.index_flag_problems():
        stop(f"clear non-default Git index flags before running: {flags!r}")
    revision, tree = contract.current_git_revision()
    inputs = contract.commit_input_hashes(revision)
    if contract.workspace_input_hashes() != inputs:
        stop("working bytes/modes do not equal the exact validated commit tree")
    require_tool_targets({"git": tools["git"]}, "after Git source binding")
    source_set = contract.canonical_json_sha256(inputs)
    test_names_digest = contract.canonical_json_sha256(tests)
    tools_sha256 = contract.canonical_json_sha256(tools)
    environment_sha256 = contract.canonical_json_sha256(contract.EXECUTION_ENVIRONMENT)
    profile = contract.execution_profile_payload(
        source_set, revision, tree, tools, git_authority
    )
    execution_profile_sha256 = contract.canonical_json_sha256(profile)

    runtime_parent = trusted_runtime_parent()
    require_runner_negative_controls(runtime_parent)
    require_tool_targets(tools, "after runner hostile controls")
    log_directory_fd, log_directory_identity = open_output_directory(LOG_DIR)
    evidence_directory_fd, evidence_directory_identity = open_output_directory(
        EVIDENCE.parent
    )
    preflight_log_directory(log_directory_fd)
    if problem := regular_output_entry_problem(evidence_directory_fd, EVIDENCE.name):
        stop(problem)
    completed_logs: dict[str, str] = {}

    list_results: dict[str, object] = {}
    listed_by_crate: dict[str, list[str]] = {}
    for index, (command, crate) in enumerate(
        zip(contract.TEST_LIST_COMMANDS, contract.TEST_LIST_CRATES), start=1
    ):
        log_relative = contract.validation_log_relative("test-list", index, command)
        log_name = Path(log_relative).name
        exit_status, duration, output_digest, output = run_in_fresh_runtime(
            command,
            revision,
            inputs,
            tools,
            execution_profile_sha256,
            runtime_parent,
            log_directory_fd,
            log_name,
        )
        completed_logs[log_name] = output_digest
        verify_completed_logs(log_directory_fd, completed_logs)
        if not directory_handle_matches(LOG_DIR, log_directory_identity):
            stop("validation-log directory path was substituted")
        if not directory_handle_matches(EVIDENCE.parent, evidence_directory_identity):
            stop("validation-evidence directory path was substituted")
        if exit_status != 0:
            stop(f"test listing failed: {command}; see {log_relative}")
        listed = contract.cargo_listed_tests(output)
        if listed is None:
            stop(f"test listing is not strict UTF-8: {command}")
        listed_by_crate[crate] = listed
        list_results[command] = {
            "result": "PASS",
            "exit_status": exit_status,
            "crate": crate,
            "source_set_sha256": source_set,
            "validated_commit": revision,
            "validated_tree": tree,
            "output_log": log_relative,
            "output_sha256": output_digest,
            "listed_tests": listed,
            "listed_tests_sha256": contract.canonical_json_sha256(listed),
            "execution_profile_sha256": execution_profile_sha256,
            "duration_seconds": duration,
            "skipped_checks": [],
        }
    for test, crate in owners.items():
        exact_name = qualified[test]
        if exact_name not in listed_by_crate[crate]:
            stop(
                f"mapped test {test} is absent as exact {exact_name} from the "
                f"{crate} native lib test list"
            )

    command_results: dict[str, object] = {}
    for index, command in enumerate(contract.TIER_2_COMMANDS, start=1):
        log_relative = contract.validation_log_relative("tier2", index, command)
        log_name = Path(log_relative).name
        exit_status, duration, output_digest, output = run_in_fresh_runtime(
            command,
            revision,
            inputs,
            tools,
            execution_profile_sha256,
            runtime_parent,
            log_directory_fd,
            log_name,
        )
        completed_logs[log_name] = output_digest
        verify_completed_logs(log_directory_fd, completed_logs)
        if not directory_handle_matches(LOG_DIR, log_directory_identity):
            stop("validation-log directory path was substituted")
        if not directory_handle_matches(EVIDENCE.parent, evidence_directory_identity):
            stop("validation-evidence directory path was substituted")
        if exit_status != 0:
            stop(f"Tier 2 command failed: {command}; see {log_relative}")
        result: dict[str, object] = {
            "result": "PASS",
            "exit_status": exit_status,
            "source_set_sha256": source_set,
            "test_names_sha256": test_names_digest,
            "validated_commit": revision,
            "validated_tree": tree,
            "output_log": log_relative,
            "output_sha256": output_digest,
        }
        if index <= len(contract.TEST_LIST_CRATES):
            passed = contract.cargo_passed_tests(output)
            if passed is None:
                stop(f"native lib test output is not strict UTF-8: {command}")
            result["passed_tests"] = passed
            result["passed_tests_sha256"] = contract.canonical_json_sha256(passed)
        result.update(
            {
                "execution_profile_sha256": execution_profile_sha256,
                "duration_seconds": duration,
                "skipped_checks": [],
            }
        )
        command_results[command] = result

    if contract.current_git_revision() != (revision, tree):
        stop("Git revision changed during validation")
    current_non_outputs = {
        path: descriptor
        for path, descriptor in contract.workspace_input_hashes().items()
        if not contract.is_validation_output_path(path)
    }
    committed_non_outputs = {
        path: descriptor
        for path, descriptor in inputs.items()
        if not contract.is_validation_output_path(path)
    }
    if current_non_outputs != committed_non_outputs:
        stop("a non-output input changed during command execution")
    if contract.git_local_authority() != git_authority:
        stop("Git local authority changed during validation")
    require_tool_targets(tools, "before closeout evidence")

    evidence = {
        "contract": "s20-530-crash-recovery-closeout-v1",
        "result": "PASS_CRASH_RECOVERY_MATRIX",
        "implementation_complete": True,
        "contract_set_sha256": contract_set_sha256,
        "test_plan": str(TEST_PLAN.relative_to(ROOT)),
        "test_plan_sha256": contract.repository_file_sha256(TEST_PLAN),
        "mapped_test_bodies_sha256": mapped_test_bodies_sha256,
        "limit_event_control_ancestries_sha256": test_plan[
            "limit_event_control_ancestries_sha256"
        ],
        "limit_event_control_exception_ledger_sha256": test_plan[
            "limit_event_control_exception_ledger_sha256"
        ],
        "limit_event_entry_call_paths_sha256": test_plan[
            "limit_event_entry_call_paths_sha256"
        ],
        "limit_event_entry_exception_ledger_sha256": test_plan[
            "limit_event_entry_exception_ledger_sha256"
        ],
        "limit_exception_reconciler_sha256": test_plan[
            "limit_exception_reconciler_sha256"
        ],
        "limit_shared_state_authority_sha256": test_plan[
            "limit_shared_state_authority_sha256"
        ],
        "reviews": {},
        "review_receipt_verification": {},
        "validation": {
            "workspace_inputs": inputs,
            "source_set_sha256": source_set,
            "runner": str(Path(__file__).resolve().relative_to(ROOT)),
            "runner_sha256": contract.repository_file_sha256(Path(__file__).resolve()),
            "execution_trust_boundary": contract.EXECUTION_TRUST_BOUNDARY,
            "environment": contract.EXECUTION_ENVIRONMENT,
            "environment_sha256": environment_sha256,
            "git_local_authority": git_authority,
            "tools": tools,
            "tools_sha256": tools_sha256,
            "execution_profile_sha256": execution_profile_sha256,
            "executed_tests": tests,
            "test_names_sha256": test_names_digest,
            "test_owners": owners,
            "qualified_tests": qualified,
            "validated_commit": revision,
            "validated_tree": tree,
            "test_lists": list_results,
            "commands": command_results,
        },
    }
    verify_completed_logs(log_directory_fd, completed_logs)
    if not directory_handle_matches(LOG_DIR, log_directory_identity):
        stop("validation-log directory path was substituted before closeout")
    if not directory_handle_matches(EVIDENCE.parent, evidence_directory_identity):
        stop("validation-evidence directory path was substituted before closeout")
    write_evidence(evidence, evidence_directory_fd)
    os.close(log_directory_fd)
    os.close(evidence_directory_fd)
    print(
        "S20-530 Tier 2 validation: PASS "
        f"({len(tests)} mapped tests; source_set_sha256={source_set}; "
        f"contract_set_sha256={contract_set_sha256}; "
        f"evidence_payload_sha256={contract.review_payload_sha256(evidence)})"
    )


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        reject_dangerous_parent_environment()
        runtime_parent = trusted_runtime_parent()
        require_runner_negative_controls(runtime_parent)
        require_git_archive_materialization_control(runtime_parent)
        print("S20-530 validation runner self-test: PASS")
    elif not sys.argv[1:]:
        main()
    else:
        stop(
            "usage: /usr/bin/python3 -I -B scripts/run_s20_530_validation.py [--self-test]"
        )
