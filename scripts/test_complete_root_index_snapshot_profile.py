#!/usr/bin/env python3
"""Regressions for the S20-300 stage checker's revision-5 and revision-6 gates.

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


class TokenAwareGate(unittest.TestCase):
    """Revision 6: literals and block comments never hide or fake a token,
    and the consumer wrapper has exactly one caller."""

    def setUp(self):
        self.temporary, self.root = scratch_tree()
        (self.root / "crates/sley-protocol/src/lib.rs").write_text(
            "#[cfg(test)]\nmod server_tests;\n", encoding="utf-8")

    def tearDown(self):
        self.temporary.cleanup()

    def consumer(self) -> Path:
        return self.root / CHECKER.PROBE_CONSUMER

    def test_real_tree_passes(self):
        self.assertEqual(CHECKER.probe_gate_problems(self.root), [])

    def test_a_url_string_does_not_hide_a_same_line_reference(self):
        path = self.root / "crates/sley-cli/src/leak.rs"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text('fn f() { let u = "http://x"; sley_repo::cached_complete_root_snapshot_id(a, b, c); }\n',
                        encoding="utf-8")
        self.assertIn("probe-caller:crates/sley-cli/src/leak.rs", CHECKER.probe_gate_problems(self.root))

    def test_a_block_comment_does_not_hide_an_alias(self):
        text = self.consumer().read_text(encoding="utf-8")
        self.consumer().write_text(
            "pub(crate) use sley_repo::cached_complete_root_snapshot_id/**/as peek;\n" + text,
            encoding="utf-8")
        self.assertIn("probe-caller:crates/sley-protocol/src/server.rs:aliased",
                      CHECKER.probe_gate_problems(self.root))

    def test_a_use_string_does_not_hide_a_second_reference(self):
        text = self.consumer().read_text(encoding="utf-8")
        self.consumer().write_text(
            text + '\nfn leak() { let s = "use "; let _ = cached_complete_root_snapshot_id; let t = ";"; }\n',
            encoding="utf-8")
        problems = CHECKER.probe_gate_problems(self.root)
        self.assertTrue(any(p.startswith("probe-caller:crates/sley-protocol/src/server.rs:references=2")
                            for p in problems), problems)

    def test_a_second_wrapper_caller_is_refused(self):
        text = self.consumer().read_text(encoding="utf-8")
        self.consumer().write_text(
            text + "\nfn disclose(&self, head: &H) { let _ = self.materialized_head_snapshot(head); }\n",
            encoding="utf-8")
        problems = CHECKER.probe_gate_problems(self.root)
        self.assertTrue(any(p.startswith("probe-wrapper-caller:crates/sley-protocol/src/server.rs")
                            for p in problems), problems)

    def test_the_wrapper_named_in_another_file_is_refused(self):
        path = self.root / "crates/sley-protocol/src/other.rs"
        path.write_text("fn f(s: &S) { s.materialized_head_snapshot(h); }\n", encoding="utf-8")
        self.assertIn("probe-wrapper-caller:crates/sley-protocol/src/other.rs",
                      CHECKER.probe_gate_problems(self.root))

    def test_the_server_test_module_may_name_the_wrapper_only_while_cfg_test(self):
        path = self.root / CHECKER.SERVER_TEST_MODULE
        path.write_text("fn t(s: &S) { s.materialized_head_snapshot(h); }\n", encoding="utf-8")
        self.assertEqual(CHECKER.probe_gate_problems(self.root), [])
        (self.root / "crates/sley-protocol/src/lib.rs").write_text("mod server_tests;\n", encoding="utf-8")
        self.assertIn(f"probe-wrapper-caller:{CHECKER.SERVER_TEST_MODULE}",
                      CHECKER.probe_gate_problems(self.root))

    def test_a_blocking_acquire_beside_the_non_waiting_one_is_refused(self):
        text = self.consumer().read_text(encoding="utf-8").replace(
            "let guard = acquire_shared_repository_maintenance_nonblocking(&self.repository).ok()?;",
            "let _hold = acquire_shared_repository_maintenance(&self.repository).ok()?;\n"
            "        let guard = acquire_shared_repository_maintenance_nonblocking(&self.repository).ok()?;",
        )
        self.assertIn("acquire_shared_repository_maintenance(&self.repository).ok()?;\n", text)
        self.consumer().write_text(text, encoding="utf-8")
        self.assertIn("probe-consumer:initializes-or-waits", CHECKER.probe_gate_problems(self.root))


if __name__ == "__main__":
    unittest.main()
