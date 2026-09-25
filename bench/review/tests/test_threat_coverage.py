"""Regression checks for the boundaries of mechanically located evidence."""
from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts'))
import build_threat_coverage_report as coverage


class EvidenceBoundaryTests(unittest.TestCase):
    def test_production_after_test_module_is_not_test_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'crates/example/src/lib.rs'
            source.parent.mkdir(parents=True)
            source.write_text('''#[cfg(test)] mod checks {
    #[test] fn sample() { assert_eq!("TEST_CONTROL", "TEST_CONTROL"); }
}
fn production() { let _ = "PRODUCTION_CONTROL"; }
''')
            with patch.object(coverage, 'ROOT', root):
                sources = coverage.exercise_sources()
            self.assertTrue(coverage.exercised_in(['TEST_CONTROL'], sources))
            self.assertEqual(coverage.exercised_in(['PRODUCTION_CONTROL'], sources), [])

    def test_symbol_name_is_not_a_substring_match(self):
        sources = [('fixture', '"PREFIX_CONTROL_DENIED" "CONTROL_DENIED_SUFFIX"')]
        self.assertEqual(coverage.exercised_in(['CONTROL_DENIED'], sources), [])
        self.assertEqual(coverage.exercised_in(['CONTROL_DENIED'], [('fixture', '"CONTROL_DENIED"')]), ['fixture'])

    def test_test_marker_in_literal_is_not_a_test_module(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'crates/example/src/lib.rs'
            source.parent.mkdir(parents=True)
            source.write_text('const HELP: &str = "#[cfg(test)] mod tests { }";\nfn production() { let _ = "PRODUCTION_CONTROL"; }')
            with patch.object(coverage, 'ROOT', root):
                self.assertEqual(coverage.exercised_in(['PRODUCTION_CONTROL'], coverage.exercise_sources()), [])


class RustRegionsTests(unittest.TestCase):
    def test_partition_preserves_production_on_both_sides_and_between_modules(self):
        from rust_source_regions import rust_production_text, rust_test_text
        source = '''#[cfg(test)] mod helper;
fn before() {}
#[cfg(test)] mod first {
    const BRACES: &str = r#" } { #[cfg(test)] mod fake { "#;
    /* } /* nested { */ } */
    #[test] fn first_test() { let _ = '}'; }
}
fn between() {}
#[cfg(test)] #[allow(dead_code)] mod second { fn second_test() {} }
fn after() {}
'''
        production = rust_production_text(source)
        tests = rust_test_text(source)
        self.assertEqual(len(production), len(source))
        self.assertEqual(production.count('\n'), source.count('\n'))
        for name in ['before', 'between', 'after']:
            self.assertIn('fn ' + name, production)
            self.assertNotIn('fn ' + name, tests)
        for name in ['first_test', 'second_test']:
            self.assertNotIn('fn ' + name, production)
            self.assertIn('fn ' + name, tests)

    def test_lexical_mask_matches_frozen_isolated_checker(self):
        import ast
        import re
        from rust_source_regions import rust_code_mask, RUST_RAW_STRING_PREFIX
        original = ast.parse((ROOT / 'scripts/check_s20_530_crash_recovery.py').read_text())
        names = {'rust_code_mask', 'rust_char_literal_end'}
        functions = [n for n in original.body if isinstance(n, ast.FunctionDef) and n.name in names]
        namespace = {'re': re, 'RUST_RAW_STRING_PREFIX': RUST_RAW_STRING_PREFIX}
        exec(compile(ast.Module(body=functions, type_ignores=[]), 'frozen-scanner', 'exec'), namespace)
        for relative in ['crates/sley-id/src/lib.rs', 'crates/sley-vm/src/lib.rs', 'crates/sley-repo/src/gc.rs']:
            source = (ROOT / relative).read_text()
            self.assertEqual(rust_code_mask(source), namespace['rust_code_mask'](source), relative)

    def test_unterminated_module_is_not_silently_classified(self):
        from rust_source_regions import rust_test_text
        with self.assertRaises(ValueError):
            rust_test_text('#[cfg(test)] mod tests { fn test() {}')


if __name__ == '__main__':
    unittest.main()
