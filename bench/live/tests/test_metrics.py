from __future__ import annotations

import json
import unittest

from bench.live.metrics import derive_provider_observation
from bench.live.provider import parse_codex_jsonl


def line(value: dict) -> bytes:
    return json.dumps(value, sort_keys=True).encode("utf-8") + b"\n"


class LiveMetricTests(unittest.TestCase):
    def test_derives_only_observable_provider_quantities(self) -> None:
        prompt = b"repair the program\n"
        payload = b"".join(
            [
                line({"type": "thread.started", "thread_id": "t1"}),
                line({"type": "turn.started"}),
                line(
                    {
                        "type": "item.completed",
                        "item": {
                            "id": "i1",
                            "type": "command_execution",
                            "command": "cat program.py && python3 -m unittest",
                            "aggregated_output": "FAILED test_upper\n",
                            "exit_code": 1,
                            "status": "failed",
                        },
                    }
                ),
                line(
                    {
                        "type": "item.completed",
                        "item": {
                            "id": "i2",
                            "type": "file_change",
                            "changes": [{"path": "program.py", "kind": "update"}],
                        },
                    }
                ),
                line(
                    {
                        "type": "item.completed",
                        "item": {
                            "id": "i3",
                            "type": "command_execution",
                            "command": "python3 -m unittest",
                            "aggregated_output": "OK\n",
                            "exit_code": 0,
                            "status": "completed",
                        },
                    }
                ),
                line(
                    {
                        "type": "item.completed",
                        "item": {"id": "i4", "type": "agent_message", "text": "done"},
                    }
                ),
                line(
                    {
                        "type": "turn.completed",
                        "usage": {
                            "input_tokens": 50,
                            "cached_input_tokens": 5,
                            "output_tokens": 10,
                            "reasoning_output_tokens": 2,
                        },
                    }
                ),
            ]
        )
        events = parse_codex_jsonl(payload)
        observation = derive_provider_observation(events, prompt)
        self.assertEqual(observation["context_bytes"], len(prompt) + len(b"FAILED test_upper\nOK\n"))
        self.assertEqual(observation["compile_or_check_attempts"], 2)
        self.assertEqual(observation["repair_loops"], 1)
        self.assertEqual(observation["invalid_candidates"], 1)
        self.assertEqual(observation["files_inspected"], 1)
        self.assertEqual(observation["model_input_tokens"], 50)
        self.assertEqual(observation["model_output_tokens"], 10)
        self.assertEqual(observation["total_observable_tokens"], 60)
        self.assertEqual(observation["tool_calls"], 3)

    def test_unrecognized_commands_do_not_invent_file_or_check_counts(self) -> None:
        payload = b"".join(
            [
                line(
                    {
                        "type": "item.completed",
                        "item": {
                            "type": "command_execution",
                            "command": "date",
                            "aggregated_output": "now\n",
                            "exit_code": 0,
                        },
                    }
                ),
                line(
                    {
                        "type": "turn.completed",
                        "usage": {
                            "input_tokens": 1,
                            "cached_input_tokens": 0,
                            "output_tokens": 1,
                            "reasoning_output_tokens": 0,
                        },
                    }
                ),
            ]
        )
        observation = derive_provider_observation(parse_codex_jsonl(payload), b"p")
        self.assertEqual(observation["files_inspected"], 0)
        self.assertEqual(observation["compile_or_check_attempts"], 0)
        self.assertEqual(observation["repair_loops"], 0)


if __name__ == "__main__":
    unittest.main()
