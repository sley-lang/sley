"""Characterize live lifecycle evidence and its source inventory."""
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts'))
from r2_execution_evidence import LIB_SUITES, REQUIRED_INPUTS, REQUIRED_TESTS, SUCCESSOR_SUITES, lifecycle_output_problems, source_digest, test_output_problems


class LifecycleEvidence(unittest.TestCase):
    def output(self):
        return '\n'.join(f'test {name} ... ok' for name in sorted(REQUIRED_TESTS)) + (
            '\nRW060_EVIDENCE genesis_root=' + 'a' * 64 + '\n'
            'test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s\n'
        )

    def test_completed_run_and_emitted_identity_are_both_required(self):
        ids = {'genesis_root': 'a' * 64}
        self.assertEqual(lifecycle_output_problems(self.output(), ids), [])
        for output in [self.output().replace('0 failed', '1 failed'),
                       self.output().replace('0 ignored', '1 ignored'),
                       self.output().replace('test pack_export', 'test different'),
                       self.output().replace('a' * 64, 'b' * 64),
                       self.output().replace('test result:', 'incomplete result:')]:
            self.assertTrue(lifecycle_output_problems(output, ids))

    def test_conflicting_identity_is_refused(self):
        output = self.output() + 'RW060_EVIDENCE genesis_root=' + 'b' * 64 + '\n'
        self.assertIn('lifecycle-identity:genesis_root', lifecycle_output_problems(output, {'genesis_root': 'a' * 64}))

    def test_partial_and_filtered_runs_cannot_substitute_for_complete_suites(self):
        output = self.output()
        self.assertTrue(test_output_problems(output.replace('5 passed', '15 passed'), REQUIRED_TESTS))
        self.assertTrue(test_output_problems(output.replace('0 filtered out', '10 filtered out'), REQUIRED_TESTS))
        self.assertEqual(test_output_problems(output.replace('0 filtered out', '10 filtered out'), REQUIRED_TESTS, filtered=True), [])

    def test_successful_expected_panic_counts_as_a_completed_test(self):
        output = 'test expected_refusal - should panic ... ok\n'
        output += 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s\n'
        self.assertEqual(test_output_problems(output, {'expected_refusal'}), [])
        self.assertTrue(test_output_problems(output.replace('... ok', '... FAILED'), {'expected_refusal'}))
        self.assertTrue(test_output_problems(output.replace('... ok', '... ignored'), {'expected_refusal'}))


class BoundSuites(unittest.TestCase):
    def test_the_sley_owned_ar05_replay_is_bound_by_the_gate(self):
        # AR-05 closure evidence is the Sley-owned replay of the closure
        # workloads through the v2 path with per-metric attribution; the
        # gate binds it to the source digest like the Rust-driven suites,
        # so a revision that regresses it cannot read READY (Nabu P3).
        group = 'bootstrap_closure::closure_workloads_replay_through_v2_with_attribution'
        self.assertIn(group, LIB_SUITES)
        self.assertEqual(LIB_SUITES[group], {group})
        self.assertIn('rw075_hydration_workloads', SUCCESSOR_SUITES)
        self.assertIn('admission_authority::tests', LIB_SUITES)


class SourceInventory(unittest.TestCase):
    def test_code_addition_edit_and_deletion_invalidate_but_summary_does_not(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(['git', 'init', '--quiet', str(root)], check=True)
            for name in REQUIRED_INPUTS | {'crates/example/lib.rs'}:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('initial\n')
            subprocess.run(['git', 'add', '.'], cwd=root, check=True)
            initial = source_digest(root)
            (root / 'machine-summary.json').write_text('{"status":"PASS"}')
            self.assertEqual(source_digest(root), initial)
            for name in REQUIRED_INPUTS:
                path = root / name
                path.write_text('changed\n')
                self.assertNotEqual(source_digest(root), initial, name)
                path.write_text('initial\n')
            source = root / 'crates/example/lib.rs'
            source.write_text('changed\n')
            changed = source_digest(root)
            self.assertNotEqual(changed, initial)
            extra = root / 'crates/example/new.rs'
            extra.write_text('new source\n')
            self.assertNotEqual(source_digest(root), changed)
            source.unlink()
            with self.assertRaises(ValueError):
                source_digest(root)

    def test_inventory_requires_build_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(['git', 'init', '--quiet', str(root)], check=True)
            with self.assertRaises(ValueError):
                source_digest(root)


if __name__ == '__main__':
    unittest.main()
