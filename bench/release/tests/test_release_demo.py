"""The canonical demo resolves its defaults from its own location and cleans up."""

from __future__ import annotations

import contextlib
import io
import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from bench.release.tests.test_packaging import demo

ROOT = Path(__file__).resolve().parents[3]


class DemoDefaultsTests(unittest.TestCase):
    def test_defaults_resolve_from_the_script_not_the_working_directory(self) -> None:
        seen = {}

        def capture(sley, fixture, work, timeout):
            seen.update(sley=sley, fixture=fixture, work=work)
            return {"contract": "s20-720-release-demo-v1", "problems": [], "result": "PASS"}

        with tempfile.TemporaryDirectory() as elsewhere, patch.object(demo, "run", side_effect=capture):
            previous = os.getcwd()
            os.chdir(elsewhere)
            try:
                with contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(demo.main([]), 0)
            finally:
                os.chdir(previous)
        script_root = (ROOT / "bench/release/run_demo.py").resolve().parent.parent
        self.assertEqual(seen["sley"], script_root / "bin/sley")
        self.assertEqual(seen["fixture"], script_root / "conformance/release-demo/v1/demo.json")
        self.assertFalse(seen["work"].exists(), "the auto-created work directory is removed")

    def test_the_temporary_work_directory_is_removed_on_failure_and_kept_when_named(self) -> None:
        def failing(sley, fixture, work, timeout):
            (work / "first").mkdir()
            raise demo.DemoFailure("endpoint closed")

        with tempfile.TemporaryDirectory() as parent:
            created = []
            real_mkdtemp = tempfile.mkdtemp

            def tracking_mkdtemp(**kwargs):
                path = real_mkdtemp(dir=parent, **kwargs)
                created.append(Path(path))
                return path

            named = Path(parent) / "named"
            output = io.StringIO()
            with patch.object(demo, "run", side_effect=failing), \
                    patch.object(demo.tempfile, "mkdtemp", side_effect=tracking_mkdtemp):
                with contextlib.redirect_stdout(output):
                    self.assertEqual(demo.main([]), 1)
                with contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(demo.main(["--work", str(named)]), 1)
            self.assertEqual(len(created), 1)
            self.assertFalse(created[0].exists())
            self.assertTrue((named / "first").is_dir(), "a named work directory is kept")
            # The failure report keeps its shape.
            self.assertEqual(
                json.loads(output.getvalue()),
                {"contract": "s20-720-release-demo-v1", "problems": ["DemoFailure:endpoint closed"], "result": "FAIL"},
            )


if __name__ == "__main__":
    unittest.main()
