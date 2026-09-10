#!/usr/bin/env python3
"""Discriminating static tests for the S20-430 rule audit (contract section 5).

Each mutation case serves one adulterated production source from memory
while the method truth stays derived from the real frozen `Method` table,
runs the real audit entrypoint, and requires a specific refusal. The
previous hand-written ranges missed tags 306/307 and every `entity.*`
literal (Vulcan R6-P1-1); the mutation cases fail before source repair and
pass after it. The valid controls pass before and after. Nothing here
mutates the repository under test.
"""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def load_module(name: str, relative: str):
    path = ROOT / relative
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


CHECKER = load_module("cli_rules_checker", "scripts/check_cli_rules.py")

PROTOCOL_TEXT = (ROOT / "crates/sley-protocol/src/lib.rs").read_text(encoding="utf-8")
TAGS, NAMES = CHECKER.derive_method_truth(PROTOCOL_TEXT)
PATTERNS = CHECKER.judgment_patterns(TAGS)
NAME_PATTERN = CHECKER.method_name_pattern(NAMES)
REAL_SOURCE = (ROOT / "crates/sley-cli/src/lib.rs").read_text(encoding="utf-8").split("#[cfg(test)]", 1)[0]


def audit(production: str) -> tuple[list, dict]:
    problems: list = []
    counters = {"frame_literals": 0, "encode_calls": 0}
    CHECKER.audit_production_source("lib.rs", production, PATTERNS, NAME_PATTERN, problems, counters)
    if counters["frame_literals"] > 1:
        problems.append(f"frame-literals:{counters['frame_literals']}")
    if counters["encode_calls"] > 1:
        problems.append(f"encode-frame-calls:{counters['encode_calls']}")
    return problems, counters


class TruthDerivationControl(unittest.TestCase):
    def test_version_two_tags_and_names_are_audited(self) -> None:
        self.assertIn(306, TAGS)
        self.assertIn(307, TAGS)
        self.assertIn("entity.version", NAMES)
        self.assertIn("entity.signature", NAMES)
        self.assertEqual((len(TAGS), len(NAMES)), (43, 43))

    def test_real_source_passes(self) -> None:
        problems, counters = audit(REAL_SOURCE)
        self.assertEqual(problems, [])
        self.assertEqual(counters["frame_literals"], 1)
        self.assertEqual(counters["encode_calls"], 1)


class TagArmCases(unittest.TestCase):
    def test_306_match_arm_refused(self) -> None:
        problems, _ = audit(REAL_SOURCE + '\nmatch tag {\n306 => {}\n_ => {}\n}\n')
        self.assertTrue(any(item.startswith("judgment:") for item in problems), problems)

    def test_307_match_arm_refused(self) -> None:
        problems, _ = audit(REAL_SOURCE + '\nmatch tag {\n307 => {}\n_ => {}\n}\n')
        self.assertTrue(any(item.startswith("judgment:") for item in problems), problems)

    def test_legacy_305_arm_still_refused(self) -> None:
        problems, _ = audit(REAL_SOURCE + '\nmatch tag {\n305 => {}\n_ => {}\n}\n')
        self.assertTrue(any(item.startswith("judgment:") for item in problems), problems)


class MethodNameCases(unittest.TestCase):
    def test_entity_version_literal_refused(self) -> None:
        problems, _ = audit(REAL_SOURCE + '\nlet _ = "entity.version";\n')
        self.assertIn("method-name:lib.rs", problems)

    def test_entity_signature_literal_refused(self) -> None:
        problems, _ = audit(REAL_SOURCE + '\nlet _ = "entity.signature";\n')
        self.assertIn("method-name:lib.rs", problems)

    def test_bare_capsule_literal_refused(self) -> None:
        problems, _ = audit(REAL_SOURCE + '\nlet _ = "capsule";\n')
        self.assertIn("method-name:lib.rs", problems)


class EncodeCallCases(unittest.TestCase):
    def test_second_versioned_encode_call_refused(self) -> None:
        problems, counters = audit(
            REAL_SOURCE + "\nfn extra() { let _ = encode_frame_for_version(&f, 1); }\n"
        )
        self.assertEqual(counters["encode_calls"], 2)
        self.assertIn("encode-frame-calls:2", problems)


if __name__ == "__main__":
    unittest.main()
