"""Unit tests for S2B-CONTEXT-001 (stdlib unittest)."""

import unittest

import program


class ContextTest(unittest.TestCase):
    def test_minimum_entities(self):
        store = program.Store()
        self.assertGreaterEqual(len(store.entities), 10000)

    def test_complete_impact_closure_updated(self):
        store = program.Store()
        closure = store.impact_closure(program.TARGET_TYPE)
        self.assertGreater(len(closure), 0)
        updated = store.add_required_field(program.TARGET_TYPE,
                                           program.FIELD,
                                           program.DEFAULT_VALUE)
        self.assertEqual(updated, len(closure))
        for eid in closure:
            self.assertIn(program.FIELD, store.entities[eid])

    def test_bounds_never_exceeded(self):
        store = program.Store()
        store.add_required_field(program.TARGET_TYPE, program.FIELD,
                                 program.DEFAULT_VALUE)
        self.assertLessEqual(store.max_chunk, program.CHUNK_MAX)
        self.assertGreater(len(store.continuations), 0)
        # Continuation records are explicit (start offsets recorded).
        self.assertTrue(store.continuations[0].startswith(
            "Continuation(start=0,"))

    def test_no_whole_reads_no_invalid_commits(self):
        store = program.Store()
        store.add_required_field(program.TARGET_TYPE, program.FIELD,
                                 program.DEFAULT_VALUE)
        self.assertEqual(store.whole_store_reads, 0)
        self.assertEqual(store.invalid_commits, 0)

    def test_unbounded_read_refused(self):
        store = program.Store()
        with self.assertRaises(program.UnboundedReadRefused) as cm:
            store.read_all()
        self.assertEqual(cm.exception.code, "ORACLE_UNBOUNDED_READ_REFUSED")


if __name__ == "__main__":
    unittest.main()
