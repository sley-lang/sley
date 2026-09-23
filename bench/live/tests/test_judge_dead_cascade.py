"""Regressions for the graph judge's ownership cascade (S2B-DEAD-001).

The frozen DEAD manifest says the helper's "parameter and block follow
their function": structure owned (pre-state) by an absent role must be
removed with it. The judge derives that ownership from decoded pre-state
bodies and still rejects unowned collateral, kept or modified owned
structure, and a kept helper. Pure unit tests run everywhere; the
end-to-end witness runs are gated on the sley and driver binaries.
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from bench.fixtures import sley2_live_judge as judge

ROOT = Path(__file__).resolve().parents[3]


def _e(byte: str) -> str:
    return byte * 32


NS, LIVE, LIVE_BLOCK, LIVE_OP = _e("88"), _e("89"), _e("8a"), _e("8d")
HELPER, HELPER_BLOCK, DEAD_BLOCK, HELPER_PARAM = _e("8e"), _e("8f"), _e("90"), _e("91")
CONST, DEAD_OP = _e("92"), _e("93")

MANIFEST = {
    "task_id": "S2B-DEAD-001",
    "entities": {"dead_block": DEAD_BLOCK, "dead_helper": HELPER,
                 "live_func": LIVE, "namespace": NS},
    "targets": [NS, LIVE, LIVE_OP, DEAD_BLOCK, HELPER],
    "judge": {"flow": "graph", "absent": ["dead_block", "dead_helper"]},
}

# Pre-state owner edges exactly as the frozen base pack decodes them.
OWNERS = {LIVE_BLOCK: LIVE, LIVE_OP: LIVE_BLOCK, _e("8b"): LIVE, _e("8c"): LIVE,
          HELPER_BLOCK: HELPER, HELPER_PARAM: HELPER,
          DEAD_BLOCK: LIVE, DEAD_OP: DEAD_BLOCK}

PRE = {entity: f"{index:064x}" for index, entity in enumerate(
    [NS, LIVE, LIVE_BLOCK, _e("8b"), _e("8c"), LIVE_OP, HELPER, HELPER_BLOCK,
     DEAD_BLOCK, HELPER_PARAM, CONST, DEAD_OP])}


def _post_after_fix() -> dict[str, str]:
    post = dict(PRE)
    for gone in (HELPER, HELPER_BLOCK, HELPER_PARAM, DEAD_BLOCK, DEAD_OP):
        post.pop(gone)
    post[NS] = "f" * 64
    post[LIVE] = "e" * 64
    return post


def _present_from(post: dict[str, str]):
    return lambda _session, entity: entity in post


class OwnerClosureTests(unittest.TestCase):
    def test_transitive_ownership_reaches_roots(self) -> None:
        owned = judge._owner_closure(OWNERS, {DEAD_BLOCK, HELPER})
        self.assertEqual(owned, {DEAD_OP, HELPER_BLOCK, HELPER_PARAM})

    def test_survivor_structure_is_not_owned(self) -> None:
        owned = judge._owner_closure(OWNERS, {DEAD_BLOCK, HELPER})
        for survivor in (LIVE_BLOCK, LIVE_OP, _e("8b"), _e("8c"), CONST, NS, LIVE):
            self.assertNotIn(survivor, owned)

    def test_deep_chain_is_followed(self) -> None:
        # Parameter of a block of a deleted function: two hops.
        owners = {"p": "b", "b": "f", "x": "y"}
        self.assertEqual(judge._owner_closure(owners, {"f"}), {"p", "b"})

    def test_cycles_terminate_without_ownership(self) -> None:
        owners = {"a": "b", "b": "a"}
        self.assertEqual(judge._owner_closure(owners, {"root"}), set())

    def test_no_roots_owns_nothing(self) -> None:
        self.assertEqual(judge._owner_closure(OWNERS, set()), set())


class OwnedByDecodeTests(unittest.TestCase):
    def test_owner_fields_by_kind(self) -> None:
        entries = [
            {"entity_id": HELPER_PARAM, "kind": 6, "body": {"owner": HELPER}},
            {"entity_id": HELPER_BLOCK, "kind": 7, "body": {"function": HELPER}},
            {"entity_id": DEAD_OP, "kind": 8, "body": {"block": DEAD_BLOCK}},
            # A constant has no structural owner; a kind-7 body with the
            # wrong field is not ownership either.
            {"entity_id": CONST, "kind": 9, "body": {"owner": HELPER}},
            {"entity_id": LIVE_BLOCK, "kind": 7, "body": {"owner": HELPER}},
        ]
        pre = {entry["entity_id"]: "0" * 64 for entry in entries}
        with mock.patch.object(judge, "_object_path", return_value=Path("/x")), \
                mock.patch.object(judge, "_decode_paths", return_value=entries):
            owned = judge._owned_by(None, Path("/ws"), pre, {HELPER, DEAD_BLOCK})
        self.assertEqual(owned, {HELPER_PARAM, HELPER_BLOCK, DEAD_OP})

    def test_missing_pre_object_is_harness_failure(self) -> None:
        with mock.patch.object(judge, "_object_path", return_value=None):
            with self.assertRaises(judge.JudgeHarnessError):
                judge._owned_by(None, Path("/ws"), {HELPER_PARAM: "0" * 64}, {HELPER})


class GraphCascadeTests(unittest.TestCase):
    def _judge(self, post: dict[str, str], scratch_ws: Path | None = Path("/ws"),
               manifest: dict | None = None) -> None:
        cascade = judge._owner_closure(OWNERS, {DEAD_BLOCK, HELPER})
        with mock.patch.object(judge, "_entity_present", side_effect=_present_from(post)), \
                mock.patch.object(judge, "_owned_by", return_value=cascade) as owned_by:
            try:
                judge._judge_graph(None, manifest or MANIFEST, PRE, post, scratch_ws)
            finally:
                self.owned_by_calls = owned_by.call_count

    def assertRejects(self, code: str, post: dict[str, str], **kwargs) -> judge.JudgeRejection:
        with self.assertRaises(judge.JudgeRejection) as raised:
            self._judge(post, **kwargs)
        self.assertEqual(raised.exception.code, code)
        return raised.exception

    def test_correct_deletion_with_cascade_passes(self) -> None:
        self._judge(_post_after_fix())

    def test_kept_helper_is_unexpected_entity_first(self) -> None:
        post = _post_after_fix()
        post[HELPER] = PRE[HELPER]
        error = self.assertRejects("ORACLE_UNEXPECTED_ENTITY", post)
        self.assertEqual(error.detail, "dead_helper")

    def test_owned_structure_kept_modified_is_rejected(self) -> None:
        post = _post_after_fix()
        post[HELPER_PARAM] = "d" * 64
        error = self.assertRejects("ORACLE_UNEXPECTED_ENTITY", post)
        self.assertIn("owned by absent role", error.detail)

    def test_unowned_collateral_deletion_is_still_touched(self) -> None:
        post = _post_after_fix()
        post.pop(CONST)
        error = self.assertRejects("ORACLE_COLLATERAL_TOUCHED", post)
        self.assertEqual(error.detail, CONST[:32])

    def test_survivor_structure_change_is_still_touched(self) -> None:
        post = _post_after_fix()
        post[LIVE_BLOCK] = "c" * 64
        self.assertRejects("ORACLE_COLLATERAL_TOUCHED", post)

    def test_deleted_survivor_parameter_is_still_touched(self) -> None:
        # A deleted non-target entity (the shape of a deleted test or
        # survivor structure) never rides the cascade.
        post = _post_after_fix()
        post.pop(_e("8b"))
        self.assertRejects("ORACLE_COLLATERAL_TOUCHED", post)

    def test_without_scratch_no_cascade_is_granted(self) -> None:
        self.assertRejects("ORACLE_COLLATERAL_TOUCHED", _post_after_fix(), scratch_ws=None)

    def test_manifest_without_absent_roles_never_derives_ownership(self) -> None:
        manifest = dict(MANIFEST, judge={"flow": "graph"})
        post = dict(PRE)
        post.pop(HELPER_PARAM)
        self.assertRejects("ORACLE_COLLATERAL_TOUCHED", post, manifest=manifest)
        self.assertEqual(self.owned_by_calls, 0)

    def test_unchanged_state_never_derives_ownership(self) -> None:
        # The helper is still present, so the absent check rejects before
        # any ownership decode.
        self.assertRejects("ORACLE_UNEXPECTED_ENTITY", dict(PRE))
        self.assertEqual(self.owned_by_calls, 0)


def _binaries() -> tuple[str, str] | None:
    sley = os.environ.get("SLEY2_SLEY_BINARY", "")
    driver = os.environ.get("SUCC_JUDGE_TEST_BINARY", "")
    if sley and driver and os.access(sley, os.X_OK) and os.access(driver, os.X_OK):
        return sley, driver
    return None


@unittest.skipUnless(_binaries(), "needs SLEY2_SLEY_BINARY and SUCC_JUDGE_TEST_BINARY")
class DeadWitnessEndToEndTests(unittest.TestCase):
    """The frozen deletion through the real tool, commit and judge."""

    def _run(self, variant: str) -> str:
        with tempfile.TemporaryDirectory() as tmp:
            log = Path(tmp) / f"{variant}.log"
            completed = subprocess.run(
                [sys.executable, str(ROOT / "bench" / "live" / "succ_witness_dead.py"),
                 variant, str(log)], capture_output=True, text=True, timeout=900,
                check=False)
            self.assertEqual(completed.returncode, 0, completed.stderr[-2000:])
            return log.read_text(encoding="utf-8")

    def test_frozen_deletion_is_accepted(self) -> None:
        text = self._run("pos")
        self.assertIn("valid True", text)
        self.assertIn('"status": "accepted"', text)
        self.assertIn("judge exit: 0", text)

    def test_unowned_extra_deletion_is_collateral(self) -> None:
        text = self._run("neg_extra_delete")
        self.assertIn('"code": "ORACLE_COLLATERAL_TOUCHED"', text)
        self.assertIn("judge exit: 1", text)


if __name__ == "__main__":
    unittest.main()
