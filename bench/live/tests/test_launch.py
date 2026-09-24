from __future__ import annotations

import json
import os
import tempfile
import unittest
from collections import Counter
from pathlib import Path
from unittest import mock

from bench.live import launch
from bench.live.artifacts import ArtifactStore
from bench.live.tests.test_attempts import manifest


class ScheduleTests(unittest.TestCase):
    def test_every_task_arm_pair_once_per_seed_in_rotated_order(self) -> None:
        value = manifest()
        slots = launch.schedule(value)
        self.assertEqual(len(slots), value["scheduled_attempts"])
        self.assertEqual(len(set(slots)), len(slots))
        first_arms = Counter(slots[index][1] for index in range(0, len(slots), 3))
        self.assertEqual(set(first_arms.values()), {5})

    def test_requested_slots_run_in_preregistered_order(self) -> None:
        value = manifest()
        chosen = launch._parse_slots(
            "S2B-TEST-001:raw_files:17,S2B-REPAIR-001:sley_2_0:17", value)
        self.assertEqual(chosen[0][0], "S2B-REPAIR-001")
        with self.assertRaises(launch.LaunchError):
            launch._parse_slots("S2B-REPAIR-001:raw_files:99", value)


class UnavailableTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR"))
        self.store = ArtifactStore(Path(self.temporary.name) / "a")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _record(self, events: list[dict]) -> dict:
        payload = b"".join(json.dumps(e).encode() + b"\n" for e in events)
        return {"artifacts": {"provider_events_sha256": self.store.put(payload).sha256}}

    def test_provider_error_event_halts(self) -> None:
        record = self._record([
            {"type": "turn.started"},
            {"type": "error", "message": "You’ve hit your usage limit. Visit ..."},
            {"type": "turn.failed", "error": {"message": "You’ve hit your usage limit."}},
        ])
        self.assertTrue(launch.provider_unavailable(self.store, record))

    def test_model_text_about_rate_limits_does_not_halt(self) -> None:
        record = self._record([
            {"type": "item.completed", "item": {"type": "agent_message",
                                                "text": "added a rate limit guard"}},
            {"type": "turn.completed", "usage": {}},
        ])
        self.assertFalse(launch.provider_unavailable(self.store, record))


class BindingTests(unittest.TestCase):
    def test_any_drift_refuses_the_slot(self) -> None:
        value = manifest()
        value["environment_manifest"] = dict(value["environment_manifest"],
                                             provider_sandbox={"provider_root": "/opt/p"},
                                             binaries={"x": 1}, provider_files={"y": 2})
        bound = {field: value[field] for field in (
            "arm_fixture_digests", "tool_description_digests", "oracle_digest",
            "prompt_template_digest", "provider_executable_sha256", "provider_version")}
        bound.update(binaries={"x": 1}, provider_files={"y": 2})
        with mock.patch.object(launch, "repo_state", return_value=(value["repo_commit"], True)), \
                mock.patch.object(launch, "derive_bindings", return_value=bound):
            launch.verify_binding(value)
        for field, drifted in (("oracle_digest", "0" * 64), ("binaries", {"x": 2})):
            changed = dict(bound, **{field: drifted})
            with mock.patch.object(launch, "repo_state", return_value=(value["repo_commit"], True)), \
                    mock.patch.object(launch, "derive_bindings", return_value=changed):
                with self.assertRaisesRegex(launch.LaunchError, field):
                    launch.verify_binding(value)
        with mock.patch.object(launch, "repo_state", return_value=(value["repo_commit"], False)):
            with self.assertRaisesRegex(launch.LaunchError, "dirty"):
                launch.verify_binding(value)
        with mock.patch.object(launch, "repo_state", return_value=("f" * 40, True)):
            with self.assertRaisesRegex(launch.LaunchError, "repo_commit"):
                launch.verify_binding(value)


if __name__ == "__main__":
    unittest.main()
