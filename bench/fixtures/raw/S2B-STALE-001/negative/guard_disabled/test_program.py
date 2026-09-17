"""Unit tests for S2B-STALE-001 (stdlib unittest)."""

import unittest

import program
from program import Store, StaleRoot


class StaleTest(unittest.TestCase):
    def test_scripted_sequence(self):
        store = Store()
        a = store.open_session()
        b = store.open_session()
        a.stage_replace("net_total", "v1-a")
        self.assertEqual(a.commit(), 1)
        b.stage_replace("net_total", "v1-b")
        with self.assertRaises(StaleRoot) as cm:
            b.commit()
        self.assertEqual(cm.exception.code, "STALE_ROOT")
        # One commit accepted before requery, zero stale accepts.
        self.assertEqual(store.version, 1)
        self.assertEqual(store.funcs["net_total"], "v1-a")
        b2 = b.requery()
        b2.stage_replace("net_total", "v2-b")
        self.assertEqual(b2.commit(), 2)
        self.assertTrue(program.validate(store.funcs))

    def test_symbol_spelling(self):
        self.assertEqual(program.CODE_STALE, "STALE_ROOT")
        self.assertEqual(StaleRoot("x").code, "STALE_ROOT")

    def test_no_last_write_wins(self):
        store = Store()
        a = store.open_session()
        b = store.open_session()
        a.stage_replace("net_total", "v1-a")
        a.commit()
        b.stage_replace("net_total", "v1-b")
        try:
            b.commit()
            stale_raised = False
        except StaleRoot:
            stale_raised = True
        self.assertTrue(stale_raised,
                        "B commit must be STALE_ROOT; funcs=%r version=%d"
                        % (store.funcs, store.version))
        self.assertNotEqual(store.funcs["net_total"], "v1-b")


if __name__ == "__main__":
    unittest.main()
