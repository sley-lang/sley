"""Removal of scratch workspaces that hold read-only store and tool files.

A staged workspace carries files and directories the tooling makes
read-only (the frozen tool, TOOLING.md, sealed store objects). A plain
`shutil.rmtree(path, ignore_errors=True)` then leaves them behind without
a word, and each leaked judge or witness run left ~20k inodes on a tmpfs
(about fifty runs filled a 1M-inode `/tmp`). `remove_scratch` restores
owner read, write, and search on the directories inside the tree that
block removal (never on the tree's parent and never on a file), retries,
and raises `ScratchRemovalError` if the tree still exists: removal either
succeeds or fails loudly.

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


def _grant_directory(path: Path, root: Path) -> None:
    """Restore owner read/write/search on a directory inside the tree.

    Only real directories at or below `root` are touched: never the root's
    parent, never a symlink, and never a non-directory (a file needs no
    mode change to be unlinked, and a hard-linked file's inode is shared
    with whatever else links it)."""
    if path != root and root not in path.parents:
        return
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return
    if not stat.S_ISDIR(mode):
        return
    wanted = stat.S_IRUSR | stat.S_IWUSR | stat.S_IXUSR
    if stat.S_IMODE(mode) & wanted != wanted:
        os.chmod(path, stat.S_IMODE(mode) | wanted)


def _strict(function, failing, excinfo) -> None:
    raise excinfo


def _handler(root: Path, depth: int):
    def handle(function, failing, excinfo) -> None:
        if isinstance(excinfo, FileNotFoundError):
            return
        if depth >= 2:
            raise excinfo
        path = Path(failing)
        _grant_directory(path.parent, root)
        _grant_directory(path, root)
        try:
            mode = path.lstat().st_mode
        except FileNotFoundError:
            return
        if stat.S_ISDIR(mode):
            # A directory that could not be listed or removed had its
            # contents skipped; remove it whole now that it is accessible.
            shutil.rmtree(path, onexc=_handler(root, depth + 1))
        else:
            os.unlink(path)
    return handle


def remove_scratch(path: Path | str) -> None:
    """Remove `path` entirely, restoring owner permissions inside it.

    Raises `ScratchRemovalError` when the tree cannot be removed.

    Precondition: no other process writes into the tree while it is being
    removed (true for every caller: the judge, the witnesses, and the
    merge prover remove their own scratch after its last user exited).
    The retry path works on paths, not directory descriptors, so a
    concurrent writer swapping a directory for a symlink could redirect
    it; `shutil.rmtree` itself stays symlink-safe for the first pass."""
    root = Path(path)
    try:
        if not os.path.lexists(root):
            return
        if root.is_symlink() or not root.is_dir():
            root.unlink()
        else:
            _grant_directory(root, root)
            shutil.rmtree(root, onexc=_handler(root, 0))
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
