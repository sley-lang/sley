"""Scripted end-to-end proof for the live sley_2_0 arm (REPAIR task).

No model, no campaign: the trial workspace is staged exactly as the
campaign stages it (initial pack + `.sley-live` tooling), a scripted
agent performs the mechanical fix through the launched tool only (opcode
swap on the above-comparison operation), and the live oracle entry
judges the workspace accepted against the frozen corpus triples.

Skipped without a sley binary and driver binary (integration-gated).
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from bench.live.taskpacks import stage_initial
from bench.live.tooling import stage_tooling

ROOT = Path(__file__).resolve().parents[3]


def _executable(path: str) -> Path | None:
    if not path:
        return None
    candidate = Path(path)
    try:
        if candidate.is_file() and not candidate.is_symlink() and os.access(candidate, os.X_OK):
            return candidate
    except OSError:
        pass
    return None


def _sley_binary() -> Path | None:
    for candidate in (
        os.environ.get("SLEY2_SLEY_BINARY", ""),
        os.path.join(os.environ.get("CARGO_TARGET_DIR", ""), "debug", "sley"),
        str(ROOT / "target" / "debug" / "sley"),
    ):
        found = _executable(candidate)
        if found is not None:
            return found
    return None


def _driver_binary() -> Path | None:
    found = _executable(os.environ.get("SUCC_JUDGE_TEST_BINARY", ""))
    if found is not None:
        return found
    for base in (os.environ.get("CARGO_TARGET_DIR", ""), str(ROOT / "target")):
        if not base:
            continue
        try:
            for candidate in sorted(Path(base, "debug", "deps").glob("succ_live_judge_cases-*")):
                found = _executable(str(candidate))
                if found is not None:
                    return found
        except OSError:
            continue
    return None


class Sley2EndToEndTests(unittest.TestCase):
    def test_repair_fix_judged_accepted(self) -> None:
        sley = _sley_binary()
        driver = _driver_binary()
        if sley is None or driver is None:
            self.skipTest("sley or driver binary unavailable")
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        ws = Path(temp.name) / "ws"
        stage_initial("sley_2_0", "S2B-REPAIR-001", ws)
        stage_tooling("sley_2_0", ws)
        env = dict(os.environ)
        env["SLEY2_SLEY_BINARY"] = str(sley)
        env["SUCC_JUDGE_TEST_BINARY"] = str(driver)
        tool = ws / ".sley-live" / "sley-tool"

        def run(*argv: str) -> dict:
            completed = subprocess.run(
                [str(tool), *argv], cwd=ws, capture_output=True,
                timeout=180, check=False, env=env)
            self.assertEqual(completed.returncode, 0, completed.stderr.decode()[:400])
            return json.loads(completed.stdout.decode())

        manifest = json.loads(
            (ROOT / "bench/fixtures/sley2/S2B-REPAIR-001/task_manifest.json")
            .read_text(encoding="utf-8"))
        above_op = manifest["entities"]["above_op"]
        # Scripted fix: read the buggy operation (LessThan tag 98), swap
        # to GreaterThan (100) in its typed body, propose, validate,
        # finish. A model would infer the swap from the decoded view, the
        # task statement, and the opcode table; the script pins it.
        read = run("read", above_op)
        self.assertFalse(read["report"]["failed"])
        entry = read["report"]["decoded"]["entries"][0]
        body = entry["body"]
        self.assertEqual(body["opcode"], 98)
        body["opcode"] = 100
        ops = [{"class": "ReplaceEntityVersion", "kind": entry["kind"],
                "target": above_op, "field_tag": None, "payload": body}]
        proposed = run("propose", json.dumps(ops))
        self.assertTrue(proposed["report"].get("created"), json.dumps(proposed)[:400])
        self.assertTrue(proposed["report"].get("valid"), json.dumps(proposed)[:400])
        record = proposed["report"]["record"]
        stored = proposed["report"]["stored"]
        done = run("finish", record)
        self.assertTrue(done["report"].get("finished"))
        self.assertEqual((ws / "final_candidate.hex").read_text(encoding="utf-8").strip(), stored)
        # Judge through the live oracle entry, exactly as run_fixture_oracle
        # invokes it (workspace argv + capability env + binary passthrough).
        oracle = (ROOT / "bench/fixtures/sley2/S2B-REPAIR-001/live_oracle.py")
        oracle_env = {"LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "PATH": "/usr/bin:/bin",
                      "PYTHONHASHSEED": "0", "TZ": "UTC",
                      "SLEY2_LIVE_ORACLE_CANDIDATE": str(ws),
                      "SLEY2_SLEY_BINARY": str(sley),
                      "SUCC_JUDGE_TEST_BINARY": str(driver)}
        completed = subprocess.run(
            [sys.executable, str(oracle), str(ws)], capture_output=True,
            timeout=300, check=False, env=oracle_env, cwd=str(ROOT))
        self.assertEqual(completed.returncode, 0, completed.stderr.decode()[:500])
        verdict = json.loads(completed.stdout.decode().strip().splitlines()[-1])
        self.assertEqual(verdict["status"], "accepted", json.dumps(verdict)[:400])
        self.assertIsNone(verdict["code"])

    def test_corrupt_restore_judged_accepted(self) -> None:
        sley = _sley_binary()
        driver = _driver_binary()
        if sley is None or driver is None:
            self.skipTest("sley or driver binary unavailable")
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        ws = Path(temp.name) / "ws"
        stage_initial("sley_2_0", "S2B-CORRUPT-001", ws)
        stage_tooling("sley_2_0", ws)
        env = dict(os.environ)
        env["SLEY2_SLEY_BINARY"] = str(sley)
        env["SUCC_JUDGE_TEST_BINARY"] = str(driver)
        tool = ws / ".sley-live" / "sley-tool"

        def run(*argv: str) -> dict:
            completed = subprocess.run(
                [str(tool), *argv], cwd=ws, capture_output=True,
                timeout=180, check=False, env=env)
            self.assertEqual(completed.returncode, 0, completed.stderr.decode()[:400])
            return json.loads(completed.stdout.decode())

        manifest = json.loads(
            (ROOT / "bench/fixtures/sley2/S2B-CORRUPT-001/task_manifest.json")
            .read_text(encoding="utf-8"))
        constant = manifest["entities"]["constant"]
        read = run("read", constant)
        self.assertFalse(read["report"]["failed"])
        entry = read["report"]["decoded"]["entries"][0]
        body = json.loads(json.dumps(entry["body"]))
        self.assertTrue(body["value"]["data"]["value"])
        body["value"]["data"]["value"] = False
        ops = [{"class": "ReplaceEntityVersion", "kind": entry["kind"],
                "target": constant, "field_tag": None, "payload": body}]
        proposed = run("propose", json.dumps(ops))
        self.assertTrue(proposed["report"].get("valid"), json.dumps(proposed)[:400])
        done = run("finish", proposed["report"]["record"])
        self.assertTrue(done["report"].get("finished"))
        oracle = (ROOT / "bench/fixtures/sley2/S2B-CORRUPT-001/live_oracle.py")
        oracle_env = {"LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "PATH": "/usr/bin:/bin",
                      "PYTHONHASHSEED": "0", "TZ": "UTC",
                      "SLEY2_LIVE_ORACLE_CANDIDATE": str(ws),
                      "SLEY2_SLEY_BINARY": str(sley),
                      "SUCC_JUDGE_TEST_BINARY": str(driver)}
        completed = subprocess.run(
            [sys.executable, str(oracle), str(ws)], capture_output=True,
            timeout=300, check=False, env=oracle_env, cwd=str(ROOT))
        self.assertEqual(completed.returncode, 0, completed.stderr.decode()[:500])
        verdict = json.loads(completed.stdout.decode().strip().splitlines()[-1])
        self.assertEqual(verdict["status"], "accepted", json.dumps(verdict)[:400])
        self.assertIsNone(verdict["code"])
