from __future__ import annotations

import sys
import unittest

from bench.live.process import ProcessError, run_provider_process


class ProviderProcessTests(unittest.TestCase):
    def test_captures_exact_streams_exit_and_wall_time(self) -> None:
        capture = run_provider_process(
            [
                sys.executable,
                "-c",
                "import sys; data=sys.stdin.buffer.read(); sys.stdout.buffer.write(data); sys.stderr.write('note')",
            ],
            b"prompt\n",
            timeout_ms=5_000,
            max_output_bytes=1024,
        )
        self.assertEqual(capture.stdout, b"prompt\n")
        self.assertEqual(capture.stderr, b"note")
        self.assertEqual(capture.exit_code, 0)
        self.assertFalse(capture.timed_out)
        self.assertGreaterEqual(capture.wall_time_ms, 0)

    def test_timeout_is_retained_as_a_capture(self) -> None:
        capture = run_provider_process(
            [sys.executable, "-c", "import time; time.sleep(10)"],
            b"",
            timeout_ms=10,
            max_output_bytes=1024,
        )
        self.assertTrue(capture.timed_out)
        self.assertEqual(capture.exit_code, 124)

    def test_output_limit_and_invalid_configuration_fail_closed(self) -> None:
        with self.assertRaisesRegex(ProcessError, "LIVE_PROVIDER_OUTPUT_LIMIT"):
            run_provider_process(
                [sys.executable, "-c", "print('x' * 1000)"],
                b"",
                timeout_ms=5_000,
                max_output_bytes=100,
            )
        for argv, prompt, timeout in (([], b"", 1), ([sys.executable], "bad", 1), ([sys.executable], b"", 0)):
            with self.subTest(argv=argv, timeout=timeout):
                with self.assertRaisesRegex(ProcessError, "LIVE_PROVIDER_PROCESS_INVALID"):
                    run_provider_process(argv, prompt, timeout_ms=timeout, max_output_bytes=1024)


if __name__ == "__main__":
    unittest.main()
