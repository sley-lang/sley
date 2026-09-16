"""Required frozen VM outcomes remain distinct from codec-only acceptance."""
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


class RequiredOutcomeVectors(unittest.TestCase):
    def test_depth_exhaustion_is_explicit_and_distinct_from_boundary_success(self):
        vectors = {v['id']: v for v in json.loads((ROOT / 'conformance/vm-extended/v1/accepted.json').read_text())['vectors']}
        self.assertIn('call-direct-depth-exceeded', set(vectors))
        self.assertEqual(vectors['call-direct-depth-exceeded']['termination'], {'kind': 'ResourceLimit', 'resource': 'CallDepth', 'tag': 5})
        self.assertNotIn('success_value_hash_hex', vectors['call-direct-depth-exceeded'])
        self.assertIn('success_value_hash_hex', vectors['call-direct-depth-ceiling'])

    def test_duplicate_key_is_a_failure_value_under_successful_execution(self):
        vectors = {v['id']: v for v in json.loads((ROOT / 'conformance/vm-extended/v1/accepted.json').read_text())['vectors']}
        self.assertIn('map-new-duplicate-key', set(vectors))
        self.assertEqual(vectors['map-new-duplicate-key']['expected_failure_value'], {'kind': 'DuplicateKey', 'code': 1})
        self.assertEqual(len(vectors['map-new-duplicate-key']['success_value_hash_hex']), 64)


class CurrentRecordConsistency(unittest.TestCase):
    def test_current_metadata_mismatches_are_independent(self):
        import sys
        sys.path.insert(0, str(ROOT / 'scripts'))
        from check_vm_extended_opcode_profile import current_record_problems
        spec = 'Status: contract revision 15\n'
        adr = 'Status: accepted current revision 15\n'
        campaign = 'Status: current revision 15\nCurrent conformance vectors: 31.\n'
        self.assertEqual(current_record_problems(spec, adr, campaign, 15, 31), [])
        for name, changed in [('spec', spec.replace('15', '16')), ('adr', adr.replace('15', '16')), ('campaign', campaign.replace('15', '16'))]:
            args = {'spec': spec, 'adr': adr, 'campaign': campaign}
            args[name] = changed
            self.assertIn('current-revision:' + name, current_record_problems(**args, revision=15, count=31))
        self.assertIn('current-vector-count:campaign', current_record_problems(spec, adr, campaign, 15, 30))
        self.assertEqual(current_record_problems(spec, adr, campaign + 'Historical revision 2: 19 vectors\n', 15, 31), [])

    def test_adr_current_revision_mismatch_fails_the_stage_checker(self):
        import contextlib
        import io
        import sys
        import tempfile
        from unittest.mock import patch
        sys.path.insert(0, str(ROOT / 'scripts'))
        import check_vm_extended_opcode_profile as checker
        with tempfile.TemporaryDirectory() as directory:
            adr = Path(directory) / 'adr.md'
            adr.write_text(checker.ADR.read_text().replace('current contract revision 15', 'current contract revision 16'))
            with patch.object(checker, 'ADR', adr), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(checker.main(), 1)


if __name__ == '__main__':
    unittest.main()
