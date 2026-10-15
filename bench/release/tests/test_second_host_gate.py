"""The real second-host preflight must distinguish status from diagnostics."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[3]


class SecondHostGateTests(unittest.TestCase):
    def preflight(self, status: str, code: int) -> subprocess.CompletedProcess:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "scripts").mkdir()
            script = root / "scripts/second_host_attest.sh"
            script.write_bytes((ROOT / "scripts/second_host_attest.sh").read_bytes())
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                            "commit", "-q", "--allow-empty", "-m", "fixture"], cwd=root, check=True)
            commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
            files = {
                "evidence/release/reproducibility-report.json": {
                    "commits": {commit: {"hosts": ["primary"], "artifact_sha256": "a" * 64}}},
                "machineresearch/sley-2.0/machine-summary.json": {
                    "release_candidate_packaging": {"candidate_commit": commit,
                                                    "candidate_artifact_sha256": "a" * 64}},
            }
            for name, value in files.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(json.dumps(value))
            (root / ".gitignore").write_text("/evidence/runtime/\n")
            subprocess.run(["git", "add", "."], cwd=root, check=True)
            subprocess.run(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                            "commit", "-q", "-m", "records"], cwd=root, check=True)
            tools = root / ".git/fixture-tools"
            tools.mkdir()
            lease = tools / "gf-lab-lease"
            lease.write_text('#!/bin/sh\nprintf "transport diagnostic\\n" >&2\n'
                             'printf "%s\\n" "$LEASE_FIXTURE_STATUS"\nexit "$LEASE_FIXTURE_CODE"\n')
            lease.chmod(0o755)
            env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ["PATH"],
                       LEASE_FIXTURE_STATUS=status, LEASE_FIXTURE_CODE=str(code))
            return subprocess.run(["bash", str(script), "--phase", "check"], cwd=root,
                                  env=env, capture_output=True, text=True, timeout=15)

    def test_free_status_ignores_stderr_diagnostics(self):
        result = self.preflight("free", 0)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("transport diagnostic", result.stderr)
        self.assertIn("check OK", result.stdout)

    def test_held_unreachable_and_unrecognized_statuses_refuse(self):
        for status, code in [('{"gate": "held"}', 10), ("free", 1), ("unknown", 0), ("", 0)]:
            with self.subTest(status=status, code=code):
                result = self.preflight(status, code)
                self.assertEqual(result.returncode, 3, result.stdout + result.stderr)
                self.assertNotIn("check OK", result.stdout)


if __name__ == "__main__":
    unittest.main()
