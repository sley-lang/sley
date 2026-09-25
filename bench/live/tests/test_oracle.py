from __future__ import annotations

import json
import unittest

from bench.live.oracle import OracleError, parse_fixture_oracle


def payload(**changes) -> bytes:
    value = {
        "arm": "raw",
        "code": None,
        "detail": "checked",
        "status": "accepted",
        "task_id": "S2B-REPAIR-001",
    }
    value.update(changes)
    return json.dumps(value, sort_keys=True).encode() + b"\n"


class LiveOracleTests(unittest.TestCase):
    def test_accepted_and_rejected_verdicts_are_normalized(self) -> None:
        accepted = parse_fixture_oracle(
            payload(), exit_code=0, expected_arm="raw_files", expected_task="S2B-REPAIR-001"
        )
        self.assertEqual(accepted["arm_id"], "raw_files")
        self.assertEqual(accepted["status"], "accepted")
        rejected = parse_fixture_oracle(
            payload(status="rejected", code="ORACLE_CLAMP_UPPER"),
            exit_code=1,
            expected_arm="raw_files",
            expected_task="S2B-REPAIR-001",
        )
        self.assertEqual(rejected["code"], "ORACLE_CLAMP_UPPER")

    def test_identity_exit_status_extra_lines_and_harness_errors_fail_closed(self) -> None:
        cases = (
            (payload(), 1, "raw_files", "S2B-REPAIR-001"),
            (payload(task_id="S2B-SIG-001"), 0, "raw_files", "S2B-REPAIR-001"),
            (payload(arm="legacy"), 0, "raw_files", "S2B-REPAIR-001"),
            (payload() + b"extra\n", 0, "raw_files", "S2B-REPAIR-001"),
            (payload(status="harness_error", code="HARNESS_CHILD"), 2, "raw_files", "S2B-REPAIR-001"),
        )
        for value, code, arm, task in cases:
            with self.subTest(value=value, code=code):
                with self.assertRaisesRegex(OracleError, "LIVE_ORACLE_INVALID"):
                    parse_fixture_oracle(
                        value, exit_code=code, expected_arm=arm, expected_task=task
                    )

    def test_unknown_fields_and_missing_rejection_code_fail_closed(self) -> None:
        with self.assertRaisesRegex(OracleError, "LIVE_ORACLE_INVALID"):
            parse_fixture_oracle(
                payload(extra=True),
                exit_code=0,
                expected_arm="raw_files",
                expected_task="S2B-REPAIR-001",
            )
        with self.assertRaisesRegex(OracleError, "LIVE_ORACLE_INVALID"):
            parse_fixture_oracle(
                payload(status="rejected"),
                exit_code=1,
                expected_arm="raw_files",
                expected_task="S2B-REPAIR-001",
            )


if __name__ == "__main__":
    unittest.main()
