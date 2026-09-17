from __future__ import annotations

import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from bench.live.manifest import (
    ManifestError,
    build_manifest,
    canonical_json_bytes,
    manifest_digest,
    read_manifest,
    write_manifest_once,
)


def digest(label: str) -> str:
    return hashlib.sha256(label.encode("utf-8")).hexdigest()


class LiveManifestTests(unittest.TestCase):
    def manifest(self) -> dict:
        return build_manifest(
            run_id="sley2-small-r1",
            created_at_utc="2026-09-17T12:00:00Z",
            repo_commit="1" * 40,
            model_exact_version="gpt-5.6-sol",
            model_tier="small",
            reasoning_effort="medium",
            trial_count=2,
            random_seeds=[17, 29],
            context_budget=131_072,
            action_budget=64,
            wall_time_budget=1_800_000,
            retry_policy={"provider_attempts": 1, "retryable_failures": []},
            hardware_manifest={"node": "greyarch", "cpu": "frozen"},
            cache_state="cold-per-trial",
            environment_manifest={
                "locale": "C.UTF-8",
                "timezone": "UTC",
                "provider_environment": {
                    "HOME": "/home/benchmark",
                    "LANG": "C.UTF-8",
                    "PATH": "/usr/bin:/bin",
                },
            },
            arm_fixture_digests={
                "raw_files": digest("raw"),
                "sley_1_2_0": digest("legacy"),
                "sley_2_0": digest("sley2"),
            },
            tool_description_digests={
                "raw_files": digest("raw-tools"),
                "sley_1_2_0": digest("legacy-tools"),
                "sley_2_0": digest("sley2-tools"),
            },
            oracle_digest=digest("oracle"),
            prompt_template_digest=digest("prompt"),
            provider_executable_sha256=digest("codex"),
            provider_version="codex-cli 0.148.0",
        )

    def test_round_trip_and_digest_are_canonical(self) -> None:
        manifest = self.manifest()
        self.assertEqual(manifest["scheduled_attempts"], 90)
        self.assertEqual(manifest["model_provider"], "openai-chatgpt-oauth")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "manifest.json"
            write_manifest_once(path, manifest)
            self.assertEqual(read_manifest(path), manifest)
            self.assertEqual(path.read_bytes(), canonical_json_bytes(manifest) + b"\n")
            self.assertEqual(manifest_digest(manifest), manifest_digest(read_manifest(path)))

    def test_manifest_is_create_once(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "manifest.json"
            write_manifest_once(path, self.manifest())
            with self.assertRaisesRegex(ManifestError, "LIVE_MANIFEST_EXISTS"):
                write_manifest_once(path, self.manifest())

    def test_model_tier_version_and_schedule_fail_closed(self) -> None:
        for field, value in (
            ("model_tier", "medium"),
            ("model_exact_version", "latest"),
            ("scheduled_attempts", 89),
        ):
            with self.subTest(field=field):
                manifest = copy.deepcopy(self.manifest())
                manifest[field] = value
                with tempfile.TemporaryDirectory() as directory:
                    path = Path(directory) / "manifest.json"
                    path.write_text(json.dumps(manifest), encoding="utf-8")
                    with self.assertRaises(ManifestError):
                        read_manifest(path)

    def test_arm_maps_are_exact(self) -> None:
        manifest = self.manifest()
        del manifest["arm_fixture_digests"]["sley_2_0"]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "manifest.json"
            path.write_bytes(canonical_json_bytes(manifest) + b"\n")
            with self.assertRaisesRegex(ManifestError, "LIVE_MANIFEST_INVALID"):
                read_manifest(path)

    def test_unhashable_enum_values_fail_as_manifest_errors(self) -> None:
        for field, value in (
            ("model_tier", []),
            ("model_configuration", {"reasoning_effort": []}),
        ):
            with self.subTest(field=field):
                candidate = copy.deepcopy(self.manifest())
                candidate[field] = value
                with self.assertRaisesRegex(ManifestError, "LIVE_MANIFEST_INVALID"):
                    manifest_digest(candidate)


if __name__ == "__main__":
    unittest.main()
