"""Exercise the R2 verdict functions without executing the aggregate gate."""
from __future__ import annotations

import ast
import re
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def verdict_functions(directory):
    tree = ast.parse((ROOT / 'scripts/check_r2_exit.py').read_text())
    names = {'review_verdict', 'lane_round', 'latest_lane_verdict', 'bound_review_text'}
    definitions = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name in names]
    namespace = {'Path': Path, 're': re, 'REVIEWS': directory}
    exec(compile(ast.Module(body=definitions, type_ignores=[]), 'check_r2_exit.py', 'exec'), namespace)
    return namespace


class ReviewTokenTests(unittest.TestCase):
    def test_latest_review_requires_exact_current_source_binding(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            functions = verdict_functions(root)
            current = 'a' * 64
            older = root / 'lane-r1-2026-09-14.log'
            newer = root / 'lane-r2-2026-09-15.log'
            older.write_text(f'SOURCE_R2_SHA256: {current}\nVERDICT: PASS\n')
            for binding in ['', 'b' * 64, current + '0']:
                newer.write_text(f'SOURCE_R2_SHA256: {binding}\nVERDICT: PASS\n')
                self.assertEqual(functions['latest_lane_verdict']('lane', current), 'PENDING')
            newer.write_text(f'SOURCE_R2_SHA256: {current}\nVERDICT: PASS\n')
            self.assertEqual(functions['latest_lane_verdict']('lane', current), 'PASS')
            newer.write_text(f'SOURCE_R2_SHA256: {current}\nSOURCE_R2_SHA256: {current}\nVERDICT: PASS\n')
            self.assertEqual(functions['latest_lane_verdict']('lane', current), 'PENDING')

    def test_longer_identifiers_are_not_verdicts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            functions = verdict_functions(root)
            path = root / 'lane-r1-2026-09-15.log'
            for token in ['PASSIVE', 'PASS_EXTRA', 'PASS1', 'FAILURE', 'FAIL_EXTRA']:
                path.write_text('VERDICT: ' + token + '\n')
                self.assertEqual(functions['review_verdict']('lane*.log'), 'PENDING', token)
                self.assertEqual(functions['latest_lane_verdict']('lane'), 'PENDING', token)

    def test_exact_verdict_notes_and_failure_precedence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            functions = verdict_functions(root)
            path = root / 'lane-r1-2026-09-15.log'
            path.write_text('VERDICT: PASS (scoped review)\n')
            self.assertEqual(functions['review_verdict']('lane*.log'), 'PASS')
            path.write_text('VERDICT: PASS\nVERDICT: FAIL (open finding)\n')
            self.assertEqual(functions['review_verdict']('lane*.log'), 'FAIL')
            self.assertEqual(functions['latest_lane_verdict']('lane'), 'FAIL')


if __name__ == '__main__':
    unittest.main()
