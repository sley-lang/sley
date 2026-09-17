from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from bench.live.legacy_tool import LegacyToolError, dispatch


class FakeSession:
    def __init__(self) -> None:
        self.calls = []

    def check(self, target):
        self.calls.append(("check", Path(target)))
        return {
            "command": "check",
            "outcome": "completed",
            "return_code": 0,
            "report": {"status": "ok"},
            "stderr_text": "",
            "stdout_bytes": 10,
            "stderr_bytes": 0,
            "truncated": False,
        }

    def machine_invoke(self, source, task, request):
        self.calls.append(("machine", Path(source), task, request))
        return {
            "command": "machine",
            "outcome": "completed",
            "return_code": 0,
            "report": {"status": "ok", "result": 7},
            "stderr_text": "",
            "stdout_bytes": 10,
            "stderr_bytes": 0,
            "truncated": False,
        }


class LegacyToolTests(unittest.TestCase):
    def test_dispatches_relative_candidate_paths_and_emits_json_safe_result(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "program.sley").write_text("task main -> Int { return 1 }\n")
            session = FakeSession()
            result = dispatch(session, ["check", "program.sley"], root)
            self.assertEqual(result["report"], {"status": "ok"})
            self.assertEqual(session.calls, [("check", root / "program.sley")])
            result = dispatch(
                session,
                ["machine", "program.sley", "app.main", '{"value":7}'],
                root,
            )
            self.assertEqual(result["report"]["result"], 7)

    def test_absolute_parent_symlink_unknown_and_bad_json_are_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "root"
            root.mkdir()
            source = root / "program.sley"
            source.write_text("x")
            link = root / "link.sley"
            link.symlink_to(source)
            cases = (
                ["check", str(source)],
                ["check", "../outside.sley"],
                ["check", "link.sley"],
                ["unknown", "program.sley"],
                ["machine", "program.sley", "task", "[]"],
            )
            for argv in cases:
                with self.subTest(argv=argv):
                    with self.assertRaisesRegex(LegacyToolError, "LIVE_LEGACY_TOOL_INVALID"):
                        dispatch(FakeSession(), argv, root)


if __name__ == "__main__":
    unittest.main()
