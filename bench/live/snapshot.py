"""Canonical, reconstructable directory snapshots for live campaign evidence."""

from __future__ import annotations

import base64
import hashlib
import json
import os
import re
import stat
from pathlib import Path, PurePosixPath
from typing import Any, Mapping

from bench.live.manifest import canonical_json_bytes


CONTRACT = "sley2.live-workspace-snapshot.v1"
HEX_64 = re.compile(r"[0-9a-f]{64}\Z")
ENTRY_FIELDS = frozenset({"content_base64", "mode", "path", "sha256", "size", "type"})
SNAPSHOT_FIELDS = frozenset({"contract", "entries", "root_mode", "total_bytes"})
MAX_ENTRIES = 100_000
MAX_TOTAL_BYTES = 512 * 1024 * 1024


class SnapshotError(ValueError):
    """A workspace cannot be safely or canonically snapshotted/restored."""


def _fail(symbol: str, detail: str = "") -> None:
    raise SnapshotError(symbol if not detail else f"{symbol}: {detail}")


def _mode(value: Any, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= 0o777:
        _fail("LIVE_SNAPSHOT_INVALID", field)
    return value


def _path(value: Any) -> str:
    if not isinstance(value, str) or not value or "\x00" in value or "\\" in value:
        _fail("LIVE_SNAPSHOT_INVALID", "path")
    parsed = PurePosixPath(value)
    if parsed.is_absolute() or str(parsed) != value or any(part in {"", ".", ".."} for part in parsed.parts):
        _fail("LIVE_SNAPSHOT_INVALID", "path")
    if len(value.encode("utf-8")) > 4096:
        _fail("LIVE_SNAPSHOT_INVALID", "path length")
    return value


def _read_regular(path: Path) -> tuple[bytes, int]:
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise SnapshotError(f"LIVE_SNAPSHOT_UNSAFE: {error}") from error
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode):
            _fail("LIVE_SNAPSHOT_UNSAFE", str(path))
        chunks: list[bytes] = []
        total = 0
        while True:
            chunk = os.read(descriptor, 65_536)
            if not chunk:
                return b"".join(chunks), stat.S_IMODE(metadata.st_mode)
            total += len(chunk)
            if total > MAX_TOTAL_BYTES:
                _fail("LIVE_SNAPSHOT_LIMIT", "single file")
            chunks.append(chunk)
    finally:
        os.close(descriptor)


def snapshot_directory(root: Path) -> dict[str, Any]:
    """Capture every directory and regular file below *root* in stable order."""

    source = Path(root)
    try:
        root_metadata = source.lstat()
    except OSError as error:
        raise SnapshotError(f"LIVE_SNAPSHOT_UNSAFE: {error}") from error
    if not stat.S_ISDIR(root_metadata.st_mode) or source.is_symlink():
        _fail("LIVE_SNAPSHOT_UNSAFE", "root")
    entries: list[dict[str, Any]] = []
    total_bytes = 0

    def visit(directory: Path, relative: PurePosixPath | None) -> None:
        nonlocal total_bytes
        try:
            children = sorted(os.scandir(directory), key=lambda item: item.name)
        except OSError as error:
            raise SnapshotError(f"LIVE_SNAPSHOT_UNSAFE: {error}") from error
        for child in children:
            child_relative = PurePosixPath(child.name) if relative is None else relative / child.name
            normalized = _path(child_relative.as_posix())
            try:
                metadata = child.stat(follow_symlinks=False)
            except OSError as error:
                raise SnapshotError(f"LIVE_SNAPSHOT_UNSAFE: {error}") from error
            if stat.S_ISLNK(metadata.st_mode):
                _fail("LIVE_SNAPSHOT_UNSAFE", normalized)
            if stat.S_ISDIR(metadata.st_mode):
                entries.append(
                    {
                        "content_base64": None,
                        "mode": stat.S_IMODE(metadata.st_mode),
                        "path": normalized,
                        "sha256": None,
                        "size": 0,
                        "type": "directory",
                    }
                )
                visit(Path(child.path), child_relative)
            elif stat.S_ISREG(metadata.st_mode):
                content, mode = _read_regular(Path(child.path))
                total_bytes += len(content)
                if total_bytes > MAX_TOTAL_BYTES:
                    _fail("LIVE_SNAPSHOT_LIMIT", "total bytes")
                entries.append(
                    {
                        "content_base64": base64.b64encode(content).decode("ascii"),
                        "mode": mode,
                        "path": normalized,
                        "sha256": hashlib.sha256(content).hexdigest(),
                        "size": len(content),
                        "type": "file",
                    }
                )
            else:
                _fail("LIVE_SNAPSHOT_UNSAFE", normalized)
            if len(entries) > MAX_ENTRIES:
                _fail("LIVE_SNAPSHOT_LIMIT", "entries")

    visit(source, None)
    entries.sort(key=lambda item: item["path"])
    snapshot = {
        "contract": CONTRACT,
        "entries": entries,
        "root_mode": stat.S_IMODE(root_metadata.st_mode),
        "total_bytes": total_bytes,
    }
    validate_snapshot(snapshot)
    return snapshot


def validate_snapshot(snapshot: Mapping[str, Any]) -> None:
    if not isinstance(snapshot, Mapping) or set(snapshot) != SNAPSHOT_FIELDS:
        _fail("LIVE_SNAPSHOT_INVALID", "field set")
    if snapshot["contract"] != CONTRACT:
        _fail("LIVE_SNAPSHOT_INVALID", "contract")
    _mode(snapshot["root_mode"], "root_mode")
    total = snapshot["total_bytes"]
    if isinstance(total, bool) or not isinstance(total, int) or not 0 <= total <= MAX_TOTAL_BYTES:
        _fail("LIVE_SNAPSHOT_INVALID", "total_bytes")
    entries = snapshot["entries"]
    if not isinstance(entries, list) or len(entries) > MAX_ENTRIES:
        _fail("LIVE_SNAPSHOT_INVALID", "entries")
    paths: list[str] = []
    calculated_total = 0
    directory_paths: set[str] = set()
    for entry in entries:
        if not isinstance(entry, dict) or set(entry) != ENTRY_FIELDS:
            _fail("LIVE_SNAPSHOT_INVALID", "entry field set")
        path = _path(entry["path"])
        paths.append(path)
        _mode(entry["mode"], f"mode {path}")
        kind = entry["type"]
        size = entry["size"]
        if isinstance(size, bool) or not isinstance(size, int) or size < 0:
            _fail("LIVE_SNAPSHOT_INVALID", f"size {path}")
        parent = PurePosixPath(path).parent.as_posix()
        if parent != "." and parent not in directory_paths:
            _fail("LIVE_SNAPSHOT_INVALID", f"missing parent {path}")
        if kind == "directory":
            if entry["content_base64"] is not None or entry["sha256"] is not None or size != 0:
                _fail("LIVE_SNAPSHOT_INVALID", f"directory {path}")
            directory_paths.add(path)
            continue
        if kind != "file" or not isinstance(entry["content_base64"], str):
            _fail("LIVE_SNAPSHOT_INVALID", f"type {path}")
        try:
            content = base64.b64decode(entry["content_base64"], validate=True)
        except (ValueError, TypeError) as error:
            raise SnapshotError(f"LIVE_SNAPSHOT_INVALID: base64 {path}") from error
        if base64.b64encode(content).decode("ascii") != entry["content_base64"]:
            _fail("LIVE_SNAPSHOT_INVALID", f"base64 canonical {path}")
        digest = entry["sha256"]
        if not isinstance(digest, str) or HEX_64.fullmatch(digest) is None:
            _fail("LIVE_SNAPSHOT_INVALID", f"sha256 {path}")
        if len(content) != size or hashlib.sha256(content).hexdigest() != digest:
            _fail("LIVE_SNAPSHOT_INVALID", f"content {path}")
        calculated_total += size
    if paths != sorted(paths) or len(paths) != len(set(paths)):
        _fail("LIVE_SNAPSHOT_INVALID", "path order")
    if calculated_total != total:
        _fail("LIVE_SNAPSHOT_INVALID", "total_bytes")
    canonical_json_bytes(dict(snapshot))


def encode_snapshot(snapshot: Mapping[str, Any]) -> bytes:
    validate_snapshot(snapshot)
    return canonical_json_bytes(dict(snapshot))


def decode_snapshot(payload: bytes) -> dict[str, Any]:
    if not isinstance(payload, bytes) or not payload:
        _fail("LIVE_SNAPSHOT_INVALID", "bytes")
    try:
        value = json.loads(payload)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise SnapshotError("LIVE_SNAPSHOT_INVALID: JSON") from error
    if not isinstance(value, dict):
        _fail("LIVE_SNAPSHOT_INVALID", "object")
    validate_snapshot(value)
    if canonical_json_bytes(value) != payload:
        _fail("LIVE_SNAPSHOT_INVALID", "noncanonical")
    return value


def restore_snapshot(snapshot: Mapping[str, Any], destination: Path) -> None:
    """Restore a validated snapshot into an absent or empty directory."""

    validate_snapshot(snapshot)
    target = Path(destination)
    try:
        if target.exists() or target.is_symlink():
            if target.is_symlink() or not target.is_dir() or any(target.iterdir()):
                _fail("LIVE_SNAPSHOT_DESTINATION", str(target))
        else:
            target.mkdir(mode=0o700, parents=True)
    except OSError as error:
        raise SnapshotError(f"LIVE_SNAPSHOT_DESTINATION: {error}") from error
    directories: list[tuple[Path, int]] = []
    for entry in snapshot["entries"]:
        output = target.joinpath(*PurePosixPath(entry["path"]).parts)
        if entry["type"] == "directory":
            try:
                output.mkdir(mode=0o700)
            except OSError as error:
                raise SnapshotError(f"LIVE_SNAPSHOT_DESTINATION: {error}") from error
            directories.append((output, entry["mode"]))
            continue
        content = base64.b64decode(entry["content_base64"], validate=True)
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        try:
            descriptor = os.open(output, flags, 0o600)
            try:
                written = 0
                while written < len(content):
                    count = os.write(descriptor, content[written:])
                    if count <= 0:
                        _fail("LIVE_SNAPSHOT_DESTINATION", "short write")
                    written += count
                os.fchmod(descriptor, entry["mode"])
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
        except OSError as error:
            raise SnapshotError(f"LIVE_SNAPSHOT_DESTINATION: {error}") from error
    for directory, mode in reversed(directories):
        directory.chmod(mode)
    target.chmod(snapshot["root_mode"])
