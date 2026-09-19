"""The shared persistent-fuzz proof validator refuses every fail-open shape."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
from fuzz_proof_record import _all_empty, proof_record_problems  # noqa: E402


class ProofRecordTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-q")
        (self.root / "fuzz/targets").mkdir(parents=True)
        (self.root / "crates/sley-vm").mkdir(parents=True)
        (self.root / "scripts").mkdir()
        (self.root / "fuzz/targets/vm_canonical_inputs.rs").write_text("use sley_vm::x;\n")
        (self.root / "fuzz/Cargo.toml").write_text("[package]\n")
        (self.root / "fuzz/Cargo.lock").write_text("")
        (self.root / "Cargo.lock").write_text('[[package]]\nname = "sley-vm"\nversion = "0"\ndependencies = [\n "sley-check",\n]\n\n[[package]]\nname = "sley-check"\nversion = "0"\n')
        (self.root / "crates/sley-vm/lib.rs").write_text("a\n")
        (self.root / "scripts/run_vm_persistent_fuzz.py").write_text("# runner\n")
        self.commit()
        self.head = self.git("rev-parse", "HEAD").strip()

    def git(self, *args: str) -> str:
        return subprocess.check_output(
            ["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", *args],
            cwd=self.root, text=True,
        )

    def commit(self) -> None:
        self.git("add", ".")
        self.git("commit", "-qm", "fixture")

    def good(self) -> dict:
        return {
            "result": "PASS", "executed_runs": 512, "runs_floor": 512, "new_crash_artifacts": [],
            "owner_lib_sancov": 40, "source_commit": self.head, "worktree_dirty_files": [],
        }

    def problems(self, proof: dict) -> list[str]:
        return proof_record_problems(self.root, proof, "scripts/run_vm_persistent_fuzz.py", ["vm_canonical_inputs"])

    def test_a_valid_fresh_pass_has_no_problems(self) -> None:
        self.assertEqual(self.problems(self.good()), [])

    def test_every_fail_open_shape_is_refused(self) -> None:
        cases = {
            "proof-record-not-pass": dict(self.good(), result="FAIL"),
            "proof-record-below-floor": dict(self.good(), executed_runs=511),
            "proof-record-not-int:executed_runs": dict(self.good(), executed_runs="512"),
            "proof-record-new-crashes": dict(self.good(), new_crash_artifacts=["crash-1"]),
            "proof-record-missing:new_crash_artifacts": {k: v for k, v in self.good().items() if k != "new_crash_artifacts"},
            "proof-record-no-owner-sancov": {k: v for k, v in self.good().items() if k != "owner_lib_sancov"},
            "proof-record-missing:worktree_dirty_files": {k: v for k, v in self.good().items() if k != "worktree_dirty_files"},
            "proof-record-dirty-worktree": dict(self.good(), worktree_dirty_files=[" M crates/sley-vm/lib.rs"]),
            "proof-record-bad-source-commit": dict(self.good(), source_commit="abc"),
            "proof-record-not-ancestor": dict(self.good(), source_commit="f" * 40),
            # 76ae15ab round: the crash list is a list (or dict of lists) and
            # counted integers are never booleans.
            "proof-record-new-crashes": dict(self.good(), new_crash_artifacts=None),
            "proof-record-no-owner-sancov": dict(self.good(), owner_lib_sancov=True),
        }
        for value in (None, "", 0, False):
            self.assertIn("proof-record-new-crashes", self.problems(dict(self.good(), new_crash_artifacts=value)), repr(value))
        self.assertIn("proof-record-not-int:executed_runs", self.problems(dict(self.good(), executed_runs=True)))
        self.assertIn("proof-record-not-int:runs_floor", self.problems(dict(self.good(), runs_floor=True)))
        self.assertFalse(_all_empty(None))
        self.assertFalse(_all_empty({"a": ""}))
        self.assertTrue(_all_empty({"a": [], "b": []}))
        for expected, proof in cases.items():
            self.assertIn(expected, self.problems(proof), expected)
        self.assertEqual(proof_record_problems(self.root, None, "x", []), ["proof-record-missing"])

    def test_regression_records_are_bound_and_crashes_refused(self) -> None:
        # Vulcan P4 at 8966da2e: the shared validator binds a proof's
        # `regression_records` to tracked files and refuses `still_crashes`.
        (self.root / "fuzz/regressions").mkdir(parents=True)
        (self.root / "fuzz/regressions/S20_700_VM_001.json").write_text("{}")
        self.commit()
        bound = dict(self.good(), source_commit=self.git("rev-parse", "HEAD").strip(),
                     regression_records=["fuzz/regressions/S20_700_VM_001.json"],
                     retested_regressions=[{"artifact": "x", "returncode": 0, "still_crashes": False}])
        self.assertEqual(self.problems(bound), [])
        self.assertEqual(self.problems(dict(bound, regression_records={"t": ["fuzz/regressions/S20_700_VM_001.json"]})), [])
        self.assertIn("proof-record-unbound-regression:fuzz/regressions/MISSING.json",
                      self.problems(dict(bound, regression_records=["fuzz/regressions/MISSING.json"])))
        self.assertIn("proof-record-not-list:regression_records",
                      self.problems(dict(bound, regression_records="fuzz/regressions/S20_700_VM_001.json")))
        self.assertIn("proof-record-still-crashes",
                      self.problems(dict(bound, retested_regressions=[{"artifact": "x", "returncode": 1, "still_crashes": True}])))
        self.assertIn("proof-record-still-crashes",
                      self.problems(dict(bound, retested_prior_crashes=[{"artifact": "y", "still_crashes": True}])))

    def test_multi_target_records_are_judged_per_target(self) -> None:
        proof = dict(self.good(), executed_runs={"a": 700, "b": 792}, targets={"a": {"runs_floor": 792}, "b": {"runs_floor": 792}},
                     new_crash_artifacts={"a": [], "b": []})
        self.assertIn("proof-record-below-floor:a", self.problems(proof))
        proof["executed_runs"]["a"] = 792
        self.assertEqual(self.problems(proof), [])
        self.assertFalse(_all_empty({"a": [], "b": ["crash"]}))

    def test_a_change_to_a_transitive_crate_or_the_runner_stales_the_proof(self) -> None:
        for path in ("crates/sley-vm/lib.rs", "scripts/run_vm_persistent_fuzz.py", "fuzz/Cargo.lock"):
            (self.root / path).write_text("changed " + path + "\n")
            self.commit()
            self.assertIn("proof-record-predates-lane-change", self.problems(self.good()))
            self.assertNotIn("proof-record-predates-lane-change", self.problems(dict(self.good(), source_commit=self.git("rev-parse", "HEAD").strip())))
        (self.root / "crates/sley-check").mkdir()
        (self.root / "crates/sley-check/lib.rs").write_text("dep\n")
        self.commit()
        # sley-check is reached transitively through Cargo.lock.
        self.assertIn("proof-record-predates-lane-change", self.problems(self.good()))


if __name__ == "__main__":
    unittest.main()
