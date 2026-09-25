from __future__ import annotations

import copy
import unittest

from bench.live.environment import EnvironmentError, provider_environment
from bench.live.tests.test_attempts import manifest


class ProviderEnvironmentTests(unittest.TestCase):
    def test_returns_exact_manifest_environment(self) -> None:
        expected = {
            "HOME": "/home/benchmark",
            "LANG": "C.UTF-8",
            "PATH": "/usr/bin:/bin",
        }
        self.assertEqual(provider_environment(manifest()), expected)

    def test_missing_required_or_unallowlisted_variables_fail_closed(self) -> None:
        for environment in (
            {"PATH": "/usr/bin:/bin"},
            {"HOME": "/home/benchmark", "PATH": "/usr/bin:/bin", "OPENAI_API_KEY": "secret"},
            {"HOME": "/home/benchmark", "PATH": ""},
        ):
            with self.subTest(environment=environment):
                candidate = copy.deepcopy(manifest())
                candidate["environment_manifest"]["provider_environment"] = environment
                with self.assertRaisesRegex(EnvironmentError, "LIVE_PROVIDER_ENVIRONMENT_INVALID"):
                    provider_environment(candidate)


if __name__ == "__main__":
    unittest.main()
