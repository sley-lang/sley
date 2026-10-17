"""Bounds tests for the local-only scaling runner (no Sley build needed)."""

from pathlib import Path
import sys
import time
import unittest

from measure_scaling import capture


class CaptureTests(unittest.TestCase):
    def child(self, source, timeout=2):
        return capture([sys.executable, "-c", source], Path.cwd(), timeout)

    def test_collects_both_streams_and_nonzero_status(self):
        code, out, err = self.child("import sys; print(42); print('error',file=sys.stderr); sys.exit(7)")
        self.assertEqual((code, out, err), (7, b"42\n", b"error\n"))

    def test_timeout_is_bounded_and_reaps_child(self):
        before = time.monotonic()
        with self.assertRaisesRegex(RuntimeError, "deadline"):
            self.child("import time; time.sleep(60)", 0.05)
        self.assertLess(time.monotonic() - before, 3)

    def test_each_stream_has_output_limit(self):
        for fd in (1, 2):
            with self.subTest(fd=fd), self.assertRaisesRegex(RuntimeError, "stream cap"):
                self.child(f"import os; data=b'x'*65536\nwhile True: os.write({fd},data)")

    def test_closed_output_does_not_bypass_deadline(self):
        before = time.monotonic()
        with self.assertRaises((RuntimeError, TimeoutError)):
            self.child("import os,time; os.close(1); os.close(2); time.sleep(60)", 0.05)
        self.assertLess(time.monotonic() - before, 3)


if __name__ == "__main__":
    unittest.main()
