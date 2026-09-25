"""Witness log provenance: dirty paths are named, witness outputs are not."""

from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

from bench.live.witness_provenance import source_identity


def git(root: Path, *argv: str) -> str:
    env = dict(os.environ, GIT_AUTHOR_NAME="w", GIT_AUTHOR_EMAIL="w@example.invalid",
               GIT_COMMITTER_NAME="w", GIT_COMMITTER_EMAIL="w@example.invalid")
    return subprocess.run(["git", *argv], cwd=root, env=env, check=True,
                          capture_output=True, text=True).stdout.strip()


class SourceIdentity(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        git(self.root, "init", "-q")
        (self.root / "code.py").write_text("x = 1\n")
        log = self.root / "bench/live/succ-trials-20260923/trial.log"
        log.parent.mkdir(parents=True)
        log.write_text("old\n")
        git(self.root, "add", ".")
        git(self.root, "commit", "-q", "-m", "base")
        self.commit = git(self.root, "rev-parse", "HEAD")

    def tearDown(self):
        self.temporary.cleanup()

    def test_clean_tree(self):
        self.assertEqual(source_identity(self.root),
                         f"source {self.commit} (clean apart from witness outputs)")

    def test_rewritten_witness_output_is_not_code_drift(self):
        (self.root / "bench/live/succ-trials-20260923/trial.log").write_text("new\n")
        self.assertEqual(source_identity(self.root),
                         f"source {self.commit} (clean apart from witness outputs)")

    def test_code_drift_names_the_dirty_paths(self):
        (self.root / "code.py").write_text("x = 2\n")
        (self.root / "bench/live/succ-trials-20260923/trial.log").write_text("new\n")
        self.assertEqual(source_identity(self.root),
                         f"source {self.commit} (tracked changes: code.py)")

    def test_outside_a_repository_is_unknown(self):
        with tempfile.TemporaryDirectory() as bare:
            self.assertEqual(source_identity(Path(bare)), "source unknown")


if __name__ == "__main__":
    unittest.main()
