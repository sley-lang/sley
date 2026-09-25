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
                "import sys,time; data=sys.stdin.buffer.read(); time.sleep(.05); sys.stdout.buffer.write(data); sys.stderr.write('note')",
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
        self.assertGreater(capture.peak_memory_bytes, 0)

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

    def test_launched_process_sees_only_the_frozen_environment(self) -> None:
        # The actual launched process's access is checked, not merely
        # the provider flag list: only allowlisted keys are visible and
        # nothing is inherited from the runner environment.
        import json
        import os

        probe = ("import json,os; print(json.dumps(dict(os.environ), sort_keys=True))")
        environment = {"HOME": "/home/benchmark", "PATH": "/usr/bin:/bin",
                       "LANG": "C.UTF-8"}
        capture = run_provider_process(
            [sys.executable, "-c", probe],
            b"",
            timeout_ms=5_000,
            max_output_bytes=65536,
            environment=environment,
        )
        self.assertEqual(capture.exit_code, 0)
        seen = json.loads(capture.stdout)
        self.assertEqual(seen, environment)
        self.assertNotIn("SLEY2_SLEY_BINARY", seen)

    def test_argv_is_literal_without_a_shell(self) -> None:
        import json

        probe = "import json,sys; print(json.dumps(sys.argv[1:]))"
        tricky = "$(touch /tmp/pwned)"
        capture = run_provider_process(
            [sys.executable, "-c", probe, tricky],
            b"",
            timeout_ms=5_000,
            max_output_bytes=65536,
        )
        self.assertEqual(json.loads(capture.stdout), [tricky])

    def test_timed_out_process_group_is_killed(self) -> None:
        import time

        started = time.monotonic()
        capture = run_provider_process(
            [sys.executable, "-c",
             "import subprocess,sys,time; "
             "subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(30)']); "
             "time.sleep(30)"],
            b"",
            timeout_ms=200,
            max_output_bytes=1024,
        )
        self.assertTrue(capture.timed_out)
        self.assertEqual(capture.exit_code, 124)
        self.assertLess(time.monotonic() - started, 25)


if __name__ == "__main__":
    unittest.main()
