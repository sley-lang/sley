"""Unit tests for S2B-TYPE-001 (stdlib unittest)."""

import json
import unittest

import program
from program import Queued, Running, Succeeded, Failed


class TypeTest(unittest.TestCase):
    def test_four_variants_constructed(self):
        states = [Queued(), Running(), Succeeded(), Failed("E_TIMEOUT")]
        self.assertEqual(len(states), 4)

    def test_exhaustive_handling(self):
        self.assertEqual(program.handle(Queued()), "waiting")
        self.assertEqual(program.handle(Running()), "working")
        self.assertEqual(program.handle(Succeeded()), "done")
        self.assertEqual(program.handle(Failed("E_TIMEOUT")),
                         "failed:E_TIMEOUT")

    def test_failed_round_trips_code(self):
        s = Failed("E_CONN")
        self.assertEqual(program.deserialize(program.serialize(s)), s)
        self.assertEqual(program.deserialize(program.serialize(s)).error_code,
                         "E_CONN")

    def test_serialization_deterministic(self):
        states = [Queued(), Running(), Succeeded(), Failed("E_X")]
        first = [json.dumps(program.serialize(s), sort_keys=True)
                 for s in states]
        second = [json.dumps(program.serialize(s), sort_keys=True)
                  for s in states]
        self.assertEqual(first, second)

    def test_no_boolean_status_binding(self):
        for s in (Queued(), Running(), Succeeded(), Failed("E_B")):
            for attr, val in vars(s).items():
                self.assertNotIsInstance(val, bool,
                                         "parallel boolean field: " + attr)


if __name__ == "__main__":
    unittest.main()
