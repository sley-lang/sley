"""Removal of scratch workspaces that hold read-only store and tool files.

A staged workspace carries files and directories the tooling makes
read-only (the frozen tool, TOOLING.md, sealed store objects). A plain
`shutil.rmtree(path, ignore_errors=True)` then leaves them behind without
a word, and each leaked judge or witness run left ~20k inodes on a tmpfs
(about fifty runs filled a 1M-inode `/tmp`). `remove_scratch` restores
owner write (and search, for directories) on the failing path's parent
and on the path itself, retries, and raises `ScratchRemovalError` if the
tree still exists: removal either succeeds or fails loudly.

`scratch_root` scopes a whole process run: every `tempfile` directory the
run creates, including those of child processes that honour `TMPDIR`,
lands under one private root that is removed on every exit path.
"""

from __future__ import annotations

import contextlib
import os
import shutil
import stat
import tempfile
from pathlib import Path
from typing import Iterator


class ScratchRemovalError(OSError):
    """A scratch tree could not be removed."""


def _grant(path: Path) -> None:
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return
    if stat.S_ISLNK(mode):
        return
    wanted = stat.S_IRUSR | stat.S_IWUSR
    if stat.S_ISDIR(mode):
        wanted |= stat.S_IXUSR
    if stat.S_IMODE(mode) & wanted != wanted:
        os.chmod(path, stat.S_IMODE(mode) | wanted)


def _strict(function, failing, excinfo) -> None:
    raise excinfo


def _restore_and_retry(failing, excinfo, nested) -> None:
    if isinstance(excinfo, FileNotFoundError):
        return
    path = Path(failing)
    _grant(path.parent)
    _grant(path)
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return
    if stat.S_ISDIR(mode):
        # A directory that could not be listed or removed had its contents
        # skipped; remove it whole now that it is accessible.
        shutil.rmtree(path, onexc=nested)
    else:
        os.unlink(path)


def _retry_once(function, failing, excinfo) -> None:
    _restore_and_retry(failing, excinfo, _strict)


def _retry(function, failing, excinfo) -> None:
    _restore_and_retry(failing, excinfo, _retry_once)


def remove_scratch(path: Path | str) -> None:
    """Remove `path` entirely, restoring owner permissions as needed.

    Raises `ScratchRemovalError` when the tree cannot be removed."""
    root = Path(path)
    try:
        if not os.path.lexists(root):
            return
        if root.is_symlink() or not root.is_dir():
            root.unlink()
        else:
            _grant(root.parent)
            _grant(root)
            shutil.rmtree(root, onexc=_retry)
    except OSError as error:
        raise ScratchRemovalError(f"scratch removal failed for {root}: {error}") from error
    if os.path.lexists(root):
        raise ScratchRemovalError(f"scratch removal left {root} behind")


@contextlib.contextmanager
def scratch_root(prefix: str) -> Iterator[Path]:
    """Run a block with a private temporary root, removed on every path.

    `tempfile.tempdir` and `TMPDIR` point into the root for the duration,
    so directories made by this process and by child processes that
    honour `TMPDIR` all fall under it."""
    root = Path(tempfile.mkdtemp(prefix=prefix))
    saved_tempdir = tempfile.tempdir
    saved_env = os.environ.get("TMPDIR")
    tempfile.tempdir = str(root)
    os.environ["TMPDIR"] = str(root)
    try:
        yield root
    finally:
        tempfile.tempdir = saved_tempdir
        if saved_env is None:
            os.environ.pop("TMPDIR", None)
        else:
            os.environ["TMPDIR"] = saved_env
        remove_scratch(root)
