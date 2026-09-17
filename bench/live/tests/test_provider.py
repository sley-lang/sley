from __future__ import annotations

import json
import unittest

from bench.live.provider import (
    CodexExecAdapter,
    ProviderError,
    parse_codex_jsonl,
)


def line(value: dict) -> bytes:
    return json.dumps(value, sort_keys=True).encode("utf-8") + b"\n"


class CodexProviderTests(unittest.TestCase):
    def test_command_freezes_noninteractive_surface(self) -> None:
        adapter = CodexExecAdapter(
            executable="/opt/codex",
            model="gpt-5.6-sol",
            reasoning_effort="medium",
        )
        argv = adapter.command("/trial/workspace")
        self.assertEqual(argv[0], "/opt/codex")
        self.assertIn("exec", argv)
        self.assertIn("--json", argv)
        self.assertIn("--ephemeral", argv)
        self.assertIn("--ignore-user-config", argv)
        self.assertIn("--ignore-rules", argv)
        self.assertIn("--strict-config", argv)
        self.assertIn("workspace-write", argv)
        self.assertIn("gpt-5.6-sol", argv)
        self.assertEqual(argv[-1], "-")
        self.assertNotIn("--dangerously-bypass-approvals-and-sandbox", argv)

    def test_unhashable_reasoning_effort_fails_as_provider_error(self) -> None:
        with self.assertRaisesRegex(ProviderError, "LIVE_PROVIDER_CONFIG_INVALID"):
            CodexExecAdapter(
                executable="/opt/codex",
                model="gpt-5.6-sol",
                reasoning_effort=[],  # type: ignore[arg-type]
            )

    def test_jsonl_parser_preserves_events_and_usage(self) -> None:
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
                            "command": "python3 -m unittest",
                            "status": "completed",
                            "exit_code": 0,
                        },
                    }
                ),
                line(
                    {
                        "type": "item.completed",
                        "item": {"id": "i2", "type": "agent_message", "text": "done"},
                    }
                ),
                line(
                    {
                        "type": "turn.completed",
                        "usage": {
                            "input_tokens": 100,
                            "cached_input_tokens": 40,
                            "output_tokens": 25,
                            "reasoning_output_tokens": 5,
                        },
                    }
                ),
            ]
        )
        parsed = parse_codex_jsonl(payload)
        self.assertEqual(parsed.thread_id, "t1")
        self.assertEqual(parsed.input_tokens, 100)
        self.assertEqual(parsed.output_tokens, 25)
        self.assertEqual(parsed.reasoning_output_tokens, 5)
        self.assertEqual(parsed.tool_calls, 1)
        self.assertEqual(parsed.final_message, "done")
        self.assertEqual(len(parsed.events), 5)

    def test_missing_terminal_duplicate_terminal_and_unknown_usage_refuse(self) -> None:
        good_terminal = line(
            {
                "type": "turn.completed",
                "usage": {
                    "input_tokens": 1,
                    "cached_input_tokens": 0,
                    "output_tokens": 1,
                    "reasoning_output_tokens": 0,
                },
            }
        )
        for payload in (
            line({"type": "turn.started"}),
            good_terminal + good_terminal,
            line({"type": "turn.completed", "usage": {"input_tokens": -1}}),
        ):
            with self.subTest(payload=payload):
                with self.assertRaises(ProviderError):
                    parse_codex_jsonl(payload)

    def test_noncanonical_or_nonobject_event_refuses(self) -> None:
        terminal = {
            "type": "turn.completed",
            "usage": {
                "input_tokens": 1,
                "cached_input_tokens": 0,
                "output_tokens": 1,
                "reasoning_output_tokens": 0,
            },
        }
        with self.assertRaisesRegex(ProviderError, "LIVE_PROVIDER_EVENT_INVALID"):
            parse_codex_jsonl(b"[]\n" + line(terminal))
        with self.assertRaisesRegex(ProviderError, "LIVE_PROVIDER_EVENT_INVALID"):
            parse_codex_jsonl(b"not-json\n")


if __name__ == "__main__":
    unittest.main()
