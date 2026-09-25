from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from bench.fixtures.live_boundary import BoundaryError, resolve_candidate


class OracleBoundaryTests(unittest.TestCase):
    def test_frozen_fixture_roles_remain_local(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            task = Path(temporary) / "task"
            (task / "fixture").mkdir(parents=True)
            (task / "negative" / "broken").mkdir(parents=True)
            self.assertEqual(resolve_candidate(task, "fixture"), (task / "fixture", "positive", None))
            self.assertEqual(
                resolve_candidate(task, "negative/broken"),
                (task / "negative" / "broken", "negative", "broken"),
            )

    def test_exact_runner_authorized_live_candidate_is_positive(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            task = Path(temporary) / "task"
            task.mkdir()
            live = Path(temporary) / "run" / "candidate"
            live.mkdir(parents=True)
            with mock.patch.dict(os.environ, {"SLEY2_LIVE_ORACLE_CANDIDATE": str(live)}):
                self.assertEqual(resolve_candidate(task, live), (live, "positive", None))

    def test_parent_child_symlink_and_unauthorized_paths_are_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            task = Path(temporary) / "task"
            task.mkdir()
            live = Path(temporary) / "run" / "candidate"
            child = live / "child"
            child.mkdir(parents=True)
            link = Path(temporary) / "link"
            link.symlink_to(live, target_is_directory=True)
            with mock.patch.dict(os.environ, {"SLEY2_LIVE_ORACLE_CANDIDATE": str(live)}):
                for candidate in (live.parent, child, link):
                    with self.subTest(candidate=candidate):
                        with self.assertRaisesRegex(BoundaryError, "LIVE_ORACLE_PATH_REFUSED"):
                            resolve_candidate(task, candidate)


if __name__ == "__main__":
    unittest.main()
