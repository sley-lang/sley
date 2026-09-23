"""Scratch workspaces are removed on every path, or removal fails loudly.

The judge's `sley2-judge-*` copy was never removed, and read-only store
and tool files defeated `shutil.rmtree(..., ignore_errors=True)` in the
pristine/corrupt/merge helpers and the witnesses; about fifty leaked runs
exhausted the inodes of a 1M-inode `/tmp`. These cases run in a private
temporary root and assert that nothing named `sley2-*` survives.
"""

from __future__ import annotations

import io
import contextlib
import json
import os
import shutil
import stat
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from bench.fixtures import sley2_live_judge as judge
from bench.live import scratch
from bench.live.scratch import ScratchRemovalError, remove_scratch, scratch_root


def read_only_tree(root: Path) -> None:
    """A repository-like tree with the modes the tooling leaves behind."""
    store = root / "objects" / "ab"
    store.mkdir(parents=True)
    (store / "record").write_bytes(b"sealed")
    (root / "HEAD").write_text("head\n")
    locked = root / "locked"
    locked.mkdir()
    (locked / "inside").write_text("x")
    for path in (store / "record", root / "HEAD", locked / "inside"):
        path.chmod(0o400)
    store.chmod(0o500)
    (root / "objects").chmod(0o500)
    locked.chmod(0o000)


def leftovers(root: Path) -> list[str]:
    return sorted(entry.name for entry in root.iterdir() if entry.name.startswith("sley2-"))


class RemoveScratch(unittest.TestCase):
    def setUp(self):
        self.base = Path(tempfile.mkdtemp(prefix="scratch-test-"))

    def tearDown(self):
        remove_scratch(self.base)

    def test_read_only_and_unlistable_tree_is_removed(self):
        tree = self.base / "sley2-judge-x"
        tree.mkdir()
        read_only_tree(tree / "ws")
        tree.chmod(0o500)
        remove_scratch(tree)
        self.assertFalse(os.path.lexists(tree))

    def test_ignore_errors_removal_would_have_leaked(self):
        tree = self.base / "sley2-judge-y"
        tree.mkdir()
        read_only_tree(tree)
        shutil.rmtree(tree, ignore_errors=True)
        self.assertTrue(os.path.lexists(tree), "the old removal left the tree behind")
        remove_scratch(tree)
        self.assertFalse(os.path.lexists(tree))

    def test_a_tree_that_survives_removal_fails_loudly(self):
        tree = self.base / "sley2-judge-z"
        tree.mkdir()
        with mock.patch.object(scratch.shutil, "rmtree", lambda *a, **k: None):
            with self.assertRaises(ScratchRemovalError):
                remove_scratch(tree)

    def test_scratch_root_is_removed_on_an_exception_and_restores_tmpdir(self):
        before = os.environ.get("TMPDIR")
        with self.assertRaises(RuntimeError):
            with scratch_root("sley2-witness-test-run-") as root:
                self.assertEqual(os.environ["TMPDIR"], str(root))
                inner = Path(tempfile.mkdtemp(prefix="sley2-context-witness-"))
                self.assertEqual(inner.parent, root)
                read_only_tree(inner / "ws")
                raise RuntimeError("witness failed")
        self.assertFalse(os.path.lexists(root))
        self.assertEqual(os.environ.get("TMPDIR"), before)


class JudgeScratch(unittest.TestCase):
    """The judge removes its scratch copy on reject and harness-error paths."""

    TASK = "S2B-CONTEXT-001"

    def setUp(self):
        self.private = Path(tempfile.mkdtemp(prefix="judge-scratch-test-"))
        self.saved = tempfile.tempdir
        tempfile.tempdir = str(self.private)
        self.workspace = self.private / "trial"
        (self.workspace / judge.REPO_DIR).mkdir(parents=True)
        read_only_tree(self.workspace / judge.REPO_DIR / "store")
        # The copy keeps the unlistable directory out (copytree cannot read
        # it); the rest of the read-only modes are copied into the scratch.
        (self.workspace / judge.REPO_DIR / "store" / "locked").chmod(0o700)
        (self.workspace / "final_candidate.hex").write_text("00\n")

    def tearDown(self):
        tempfile.tempdir = self.saved
        remove_scratch(self.private)

    def run_judge(self, binary: str) -> dict:
        out = io.StringIO()
        with mock.patch.object(sys, "argv", ["judge", str(self.workspace)]), \
                mock.patch.dict(os.environ, {"SLEY2_SLEY_BINARY": binary}), \
                contextlib.redirect_stdout(out):
            judge.main(self.TASK)
        return json.loads(out.getvalue().strip().splitlines()[-1])

    def test_harness_error_path_leaves_no_scratch(self):
        # A binary that exits at once: the session fails right after the
        # scratch copy is made (this path leaked a read-only sley2-judge-*).
        binary = self.private / "dead-binary"
        binary.write_text("#!/bin/sh\nexit 3\n")
        binary.chmod(0o755)
        record = self.run_judge(str(binary))
        self.assertEqual(record["status"], "harness_error", record)
        self.assertEqual(leftovers(self.private), [])

    @unittest.skipUnless(os.environ.get("SLEY2_SLEY_BINARY"), "SLEY2_SLEY_BINARY unbound")
    def test_real_binary_path_leaves_no_scratch(self):
        record = self.run_judge(os.environ["SLEY2_SLEY_BINARY"])
        self.assertIn(record["status"], ("rejected", "harness_error"), record)
        self.assertEqual(leftovers(self.private), [])

    def test_a_scratch_that_cannot_be_removed_is_a_harness_error(self):
        binary = self.private / "dead-binary"
        binary.write_text("#!/bin/sh\nexit 3\n")
        binary.chmod(0o755)
        with mock.patch.object(judge, "remove_scratch",
                               side_effect=ScratchRemovalError("stuck")):
            record = self.run_judge(str(binary))
        self.assertEqual(record["status"], "harness_error", record)
        self.assertIn("scratch removal", record["detail"])
        for name in leftovers(self.private):
            remove_scratch(self.private / name)


if __name__ == "__main__":
    unittest.main()
