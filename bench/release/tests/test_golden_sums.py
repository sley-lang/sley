"""The sley-tests golden files are checked against their SHA256SUMS."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import shutil
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location("check_golden_sha256sums", ROOT / "scripts/check_golden_sha256sums.py")
assert SPEC is not None and SPEC.loader is not None
golden = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(golden)


class GoldenSumsTests(unittest.TestCase):
    def test_the_golden_set_matches_and_drift_fails_loudly(self) -> None:
        sums = ROOT / "crates/sley-tests/golden/SHA256SUMS"
        self.assertEqual(golden.sums_problems(sums), [])
        with tempfile.TemporaryDirectory() as tmp:
            copy = Path(tmp) / "golden"
            shutil.copytree(sums.parent, copy)
            first = sorted(path for path in copy.iterdir() if path.name != "SHA256SUMS")[0]
            first.write_bytes(first.read_bytes() + b" ")
            (copy / "extra.json").write_text("{}\n", encoding="utf-8")
            problems = golden.sums_problems(copy / "SHA256SUMS")
            self.assertEqual([problem.split(":")[0] for problem in problems], ["unlisted", "mismatch"])
            self.assertIn(first.name, problems[1])
            first.unlink()
            self.assertIn(f"missing:{first.name}", golden.sums_problems(copy / "SHA256SUMS"))
            stderr = io.StringIO()
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(stderr):
                self.assertEqual(golden.main([str(copy / "SHA256SUMS")]), 1)
            self.assertIn("FAIL", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
