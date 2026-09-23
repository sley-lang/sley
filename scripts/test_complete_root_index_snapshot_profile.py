#!/usr/bin/env python3
"""Regressions for the S20-300 stage checker's revision-5 gates.

Completion binding: a COMPLETE status backed only by the historical
base-field PASS values (revision 3) is refused with
`completion-unbound-review`, and admitted only when every lane carries a
PASS bound to the current contract revision by field name. Probe gate: the
identity probe has exactly one reader; aliased imports, same-named files in
other crates, `tests` components that are not crate integration tests,
extra references in the consumer file, fuzz targets, and a waiting or
initializing consumer are each refused. Every case serves inputs from
memory or a scratch tree; nothing here mutates the repository.
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location(
    "complete_root_index_snapshot_checker",
    ROOT / "scripts/check_complete_root_index_snapshot_profile.py",
)
CHECKER = importlib.util.module_from_spec(spec)
sys.modules["complete_root_index_snapshot_checker"] = CHECKER
spec.loader.exec_module(CHECKER)

SUMMARY_TEXT = (ROOT / "machineresearch/sley-2.0/machine-summary.json").read_text(
    encoding="utf-8"
)
LANES = ("ariadne_contract_review", "nabu_architecture_review", "vulcan_surface_review")


def run_with_summary(summary: dict) -> tuple[int, dict]:
    original = CHECKER.read

    def read(path):
        if Path(path) == CHECKER.SUMMARY:
            return json.dumps(summary)
        return original(path)

    CHECKER.read = read
    try:
        buffer = io.StringIO()
        with contextlib.redirect_stdout(buffer):
            code = CHECKER.main()
        return code, json.loads(buffer.getvalue())
    finally:
        CHECKER.read = original


def complete_flip(summary: dict) -> dict:
    section = summary["complete_root_index_snapshot"]
    section["status"] = CHECKER.COMPLETE_STATUS
    section["implementation_complete"] = True
    return section


class CompletionBinding(unittest.TestCase):
    def test_baseline_passes(self):
        code, payload = run_with_summary(json.loads(SUMMARY_TEXT))
        self.assertEqual((code, payload["result"]), (0, "PASS"), payload["problems"])

    def test_complete_flip_on_historical_base_pass_is_refused(self):
        summary = json.loads(SUMMARY_TEXT)
        section = complete_flip(summary)
        for lane in LANES:
            self.assertTrue(str(section[lane]).startswith("PASS"))
            section.pop(f"{lane}_revision_{CHECKER.SPEC_REVISION}", None)
        code, payload = run_with_summary(summary)
        self.assertEqual(code, 1)
        for lane in LANES:
            self.assertIn(f"completion-unbound-review:{lane}", payload["problems"])

    def test_complete_flip_on_previous_revision_fields_is_refused(self):
        summary = json.loads(SUMMARY_TEXT)
        section = complete_flip(summary)
        for lane in LANES:
            section[f"{lane}_revision_{CHECKER.SPEC_REVISION - 1}"] = "PASS_0_P0_0_P1_0_P2_0_P3"
            section.pop(f"{lane}_revision_{CHECKER.SPEC_REVISION}", None)
        code, payload = run_with_summary(summary)
        self.assertEqual(code, 1)
        self.assertIn("completion-unbound-review:nabu_architecture_review", payload["problems"])

    def test_complete_with_bound_current_pass_is_admitted(self):
        summary = json.loads(SUMMARY_TEXT)
        section = complete_flip(summary)
        for lane in LANES:
            section[f"{lane}_revision_{CHECKER.SPEC_REVISION}"] = "PASS_0_P0_0_P1_0_P2_0_P3"
        code, payload = run_with_summary(summary)
        self.assertEqual((code, payload["result"]), (0, "PASS"), payload["problems"])


def scratch_tree() -> tuple[tempfile.TemporaryDirectory, Path]:
    temporary = tempfile.TemporaryDirectory()
    root = Path(temporary.name)
    for relative in (CHECKER.PROBE_DEFINITION, CHECKER.PROBE_CONSUMER):
        target = root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text((ROOT / relative).read_text(encoding="utf-8"), encoding="utf-8")
    (root / "fuzz/targets").mkdir(parents=True)
    return temporary, root


class ProbeGate(unittest.TestCase):
    def setUp(self):
        self.temporary, self.root = scratch_tree()

    def tearDown(self):
        self.temporary.cleanup()

    def write(self, relative: str, text: str) -> None:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def test_real_tree_passes(self):
        self.assertEqual(CHECKER.probe_gate_problems(ROOT), [])
        self.assertEqual(CHECKER.probe_gate_problems(self.root), [])

    def test_aliased_import_elsewhere_is_refused(self):
        self.write("crates/sley-cli/src/peek.rs",
                   "use sley_repo::cached_complete_root_snapshot_id as peek;\nfn f() { peek(); }\n")
        self.assertIn("probe-caller:crates/sley-cli/src/peek.rs",
                      CHECKER.probe_gate_problems(self.root))

    def test_same_named_file_in_another_crate_is_refused(self):
        self.write("crates/sley-protocol/src/index_cache.rs",
                   "fn f() { sley_repo::cached_complete_root_snapshot_id(a, b, c); }\n")
        self.assertIn("probe-caller:crates/sley-protocol/src/index_cache.rs",
                      CHECKER.probe_gate_problems(self.root))

    def test_non_integration_tests_component_is_refused(self):
        self.write("crates/sley-protocol/src/tests/helper.rs",
                   "fn f() { sley_repo::cached_complete_root_snapshot_id(a, b, c); }\n")
        self.assertIn("probe-caller:crates/sley-protocol/src/tests/helper.rs",
                      CHECKER.probe_gate_problems(self.root))

    def test_crate_integration_tests_are_exempt(self):
        self.write("crates/sley-repo/tests/probe.rs",
                   "fn f() { sley_repo::cached_complete_root_snapshot_id(a, b, c); }\n")
        self.assertEqual(CHECKER.probe_gate_problems(self.root), [])

    def test_fuzz_target_reference_is_refused(self):
        self.write("fuzz/targets/probe.rs",
                   "fn f() { let g = sley_repo::cached_complete_root_snapshot_id; }\n")
        self.assertIn("probe-caller:fuzz/targets/probe.rs",
                      CHECKER.probe_gate_problems(self.root))

    def test_second_reference_in_the_consumer_file_is_refused(self):
        path = self.root / CHECKER.PROBE_CONSUMER
        text = path.read_text(encoding="utf-8")
        path.write_text(text + "\nfn leak() { let _ = cached_complete_root_snapshot_id; }\n",
                        encoding="utf-8")
        problems = CHECKER.probe_gate_problems(self.root)
        self.assertTrue(any(p.startswith("probe-caller:crates/sley-protocol/src/server.rs:references=2")
                            for p in problems), problems)

    def test_aliased_import_in_the_consumer_is_refused(self):
        path = self.root / CHECKER.PROBE_CONSUMER
        text = path.read_text(encoding="utf-8")
        path.write_text("use sley_repo::cached_complete_root_snapshot_id as peek;\n" + text,
                        encoding="utf-8")
        self.assertIn("probe-caller:crates/sley-protocol/src/server.rs:aliased",
                      CHECKER.probe_gate_problems(self.root))

    def test_waiting_consumer_is_refused(self):
        path = self.root / CHECKER.PROBE_CONSUMER
        text = path.read_text(encoding="utf-8").replace(
            "acquire_shared_repository_maintenance_nonblocking(&self.repository)",
            "self.maintenance()",
        )
        path.write_text(text, encoding="utf-8")
        problems = CHECKER.probe_gate_problems(self.root)
        self.assertIn("probe-consumer:not-non-waiting", problems)
        self.assertIn("probe-consumer:initializes-or-waits", problems)


if __name__ == "__main__":
    unittest.main()
