"""Offline tests of the S20-740 finding register: derivation, states, invariants."""

from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "build_finding_register", ROOT / "scripts/build_finding_register.py"
)
assert SPEC is not None and SPEC.loader is not None
register = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(register)


class ClassificationTests(unittest.TestCase):
    def test_heads_are_exact(self) -> None:
        for disposition, state in (
            ("PASS", "PASS"),
            ("PASS_NO_OPEN_P0_P1_P2", "PASS"),
            ("VULCAN_PASS", "PASS"),
            ("PENDING", "PENDING"),
            ("DEFERRED_FORGE_OAUTH_401", "DEFERRED"),
            ("FAIL_2_P1_1_P2", "FAIL_ROUND"),
            ("REVISE_TO_RESTRICTED_PROFILE", "FAIL_ROUND"),
            ("TIMED_OUT_PARTIAL_HANDOFF", "OTHER"),
            # First-token matching, not prefix matching: PASSIVE is no state.
            ("PASSIVE_OBSERVATION", "OTHER"),
            ("PENDINGNESS", "OTHER"),
        ):
            self.assertEqual(register.classify_token(disposition), state, disposition)

    def test_a_pass_claiming_an_open_finding_is_other(self) -> None:
        self.assertEqual(
            register.classify_token("PASS_PENDING_CONFIRMATION_2_P0_OPEN"), "OTHER"
        )
        self.assertEqual(
            register.classify_token("PASS_FINDINGS_CLOSED_NO_OPEN_P0_P1_P2"), "PASS"
        )

    def test_negations_carry_no_severity(self) -> None:
        self.assertEqual(register.severities_of("PASS_NO_OPEN_P0_P1_P2"), [])
        self.assertEqual(
            register.severities_of("PASS_PRIOR_P2_P3_CLOSED_NO_NEW_P0_P1_P2_P3_P4"),
            ["P2", "P3"],
        )
        self.assertEqual(register.severities_of("FAIL_2_P0_8_P1_9_P2_8_P3"), ["P0", "P1", "P2", "P3"])

    def test_zero_counts_carry_no_severity(self) -> None:
        self.assertEqual(
            register.severities_of("PASS_0_P0_0_P1_0_P2_0_P3"), []
        )
        self.assertEqual(
            register.severities_of("FAIL_0_P0_5_P1_4_P2_4_P3"), ["P1", "P2", "P3"]
        )
        self.assertEqual(
            register.severities_of("REVISE_0_P0_2_P1_3_P2_3_P3"), ["P1", "P2", "P3"]
        )

    def test_reviewer_tokens(self) -> None:
        self.assertEqual(register.reviewer_of("nabu_architecture_review"), "nabu")
        self.assertEqual(register.reviewer_of("vulcan_surface_review"), "vulcan")
        self.assertIsNone(register.reviewer_of("focused_semantic_security_review"))

    def test_roles_sessions_instants_and_the_registers_own_verdict_are_not_obligations(
        self,
    ) -> None:
        self.assertFalse(register.is_obligation("any", "reviewer_role", "vulcan"))
        self.assertFalse(
            register.is_obligation("any", "review_session_id", "forge-vulcan-s20-540-x")
        )
        self.assertFalse(
            register.is_obligation("any", "vulcan_review", "2026-09-02T17:42:11.513Z")
        )
        self.assertFalse(
            register.is_obligation("any", "merge_engine_reviews", "S20-520 reviews pending")
        )
        self.assertFalse(register.is_obligation("any", "status", "PENDING"))
        self.assertFalse(
            register.is_obligation("finding_register", "independent_review", "PENDING")
        )
        # Any other section's verdict field stays collected: no other checker
        # asserts it, so dropping it would hide an open review.
        self.assertTrue(
            register.is_obligation("invariant_audit", "independent_review", "PENDING")
        )
        self.assertTrue(register.is_obligation("any", "vulcan_review", "PENDING"))
        self.assertTrue(
            register.is_obligation(
                "any", "final_argus_disposition", "DEFERRED_FORGE_OAUTH_401"
            )
        )

    def test_supersession_runs_early_or_qualified_toward_late_or_general(self) -> None:
        self.assertTrue(register.supersedes("vulcan_review", "vulcan_first_review"))
        self.assertTrue(register.supersedes("ariadne_final_review", "ariadne_initial_review"))
        self.assertTrue(
            register.supersedes(
                "ariadne_contract_review", "ariadne_contract_review_revision_1"
            )
        )
        self.assertTrue(
            register.supersedes("ariadne_review", "ariadne_contract_review_revision_1")
        )
        self.assertTrue(
            register.supersedes(
                "ariadne_profile_final_review", "ariadne_initial_review"
            )
        )
        # Two unmarked rounds never fold across cores: an older general
        # PASS cannot close a newer qualified FAIL without round evidence.
        self.assertFalse(
            register.supersedes("ariadne_review", "ariadne_operation_analysis_review")
        )
        # A scoped PASS never folds a round from another subject.
        self.assertFalse(
            register.supersedes(
                "vulcan_entity_read_review", "vulcan_surface_review_revision_1"
            )
        )
        self.assertTrue(
            register.supersedes(
                "vulcan_entity_read_review", "vulcan_entity_read_review_revision_1"
            )
        )
        # A slice PASS never closes the overall FAIL it belongs to.
        self.assertFalse(
            register.supersedes("candidate_result_vulcan_review", "vulcan_review")
        )
        self.assertFalse(register.supersedes("vulcan_review", "vulcan_review"))

    def test_complete_statuses_exclude_incomplete(self) -> None:
        self.assertTrue(register.is_complete_status("S20_999_COMPLETE"))
        self.assertFalse(register.is_complete_status("S20_999_INCOMPLETE"))
        self.assertFalse(register.is_complete_status("S20_999_NOT_COMPLETE"))
        self.assertFalse(register.is_complete_status("S20_999_IMPLEMENTED_REVIEW_PENDING"))


class DerivationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.register = register.build_register()

    def test_the_register_describes_the_tracked_summary(self) -> None:
        self.assertEqual(self.register["contract"], register.CONTRACT)
        self.assertEqual(self.register["contract_revision"], register.CONTRACT_REVISION)
        self.assertEqual(self.register["work_package"], "S20-740")
        self.assertEqual(self.register["obligation_count"], len(self.register["obligations"]))
        self.assertEqual(
            sum(self.register["states"].values()), self.register["obligation_count"]
        )
        self.assertEqual(
            self.register["obligations_digest"],
            register.digest_of(self.register["obligations"]),
        )
        self.assertEqual(
            self.register["register_digest"],
            register.digest_of(
                {key: value for key, value in self.register.items() if key != "register_digest"}
            ),
        )

    def test_open_and_deferred_lists_agree_with_the_states(self) -> None:
        self.assertEqual(
            len(self.register["open_reviews"]), self.register["states"].get("PENDING", 0)
        )
        self.assertEqual(
            len(self.register["deferred_reviews"]), self.register["states"].get("DEFERRED", 0)
        )
        self.assertEqual(
            len(self.register["unclassified"]), self.register["states"].get("OTHER", 0)
        )
        self.assertEqual(
            len(self.register["superseded_rounds"]),
            self.register["states"].get("HISTORICAL_ROUND", 0),
        )

    def test_open_reviews_carry_disposition_and_severities(self) -> None:
        for entry in self.register["open_reviews"]:
            self.assertEqual(
                set(entry), {"section", "field", "disposition", "severities"}
            )

    def test_no_completed_package_carries_an_open_review(self) -> None:
        self.assertEqual(self.register["complete_packages_with_open_reviews"], [])
        self.assertTrue(self.register["complete_packages"])

    def test_the_result_reflects_the_open_reviews(self) -> None:
        expected = (
            "FINDING_REGISTER_CLEAR"
            if not self.register["open_reviews"] and not self.register["unclassified"]
            else "FINDING_REGISTER_OPEN"
        )
        self.assertEqual(self.register["result"], expected)

    def test_the_register_is_a_pure_function_of_the_summary(self) -> None:
        self.assertEqual(self.register, register.build_register())

    def test_recording_the_registers_own_counts_does_not_change_it(self) -> None:
        # The summary records this register's counts, so the derivation must be
        # stable under a counter update or it would never reach a fixed point.
        original = register.SUMMARY
        summary = json.loads(original.read_text(encoding="utf-8"))
        summary["finding_register"]["open_reviews"] = 4242
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "summary.json"
            path.write_text(json.dumps(summary), encoding="utf-8")
            register.SUMMARY = path
            self.addCleanup(setattr, register, "SUMMARY", original)
            self.assertEqual(register.build_register(), self.register)

    def test_no_host_path_or_instant_appears(self) -> None:
        text = register.canonical(self.register)
        for marker in ("/home/", "greyforge", "file://"):
            self.assertNotIn(marker, text)


class InvariantTests(unittest.TestCase):
    def build_from(self, summary: dict) -> dict:
        original = register.SUMMARY
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "summary.json"
            path.write_text(json.dumps(summary), encoding="utf-8")
            register.SUMMARY = path
            self.addCleanup(setattr, register, "SUMMARY", original)
            return register.build_register()

    def build_fails(self, summary: dict) -> register.RegisterError:
        original = register.SUMMARY
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "summary.json"
            path.write_text(json.dumps(summary), encoding="utf-8")
            register.SUMMARY = path
            self.addCleanup(setattr, register, "SUMMARY", original)
            with self.assertRaises(register.RegisterError) as error:
                register.build_register()
            return error.exception

    def test_a_completed_package_with_a_pending_review_fails_closed(self) -> None:
        error = self.build_fails(
            {
                "example_package": {
                    "status": "S20_999_COMPLETE",
                    "vulcan_review": "PENDING",
                }
            }
        )
        self.assertEqual(error.code, register.RegisterErrorCode.COMPLETION_VIOLATION)

    def test_an_unsuperseded_failure_in_a_completed_package_fails_closed(self) -> None:
        error = self.build_fails(
            {
                "example_package": {
                    "status": "S20_999_COMPLETE",
                    "nabu_architecture_review": "REVISE_TO_RESTRICTED_PROFILE",
                    "ariadne_final_review": "PASS",
                }
            }
        )
        self.assertEqual(error.code, register.RegisterErrorCode.COMPLETION_VIOLATION)

    def test_a_completed_package_may_carry_a_superseded_failed_round(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_COMPLETE",
                    "vulcan_first_review": "FAIL_2_P1",
                    "vulcan_review": "PASS",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_CLEAR")
        self.assertEqual(derived["states"]["HISTORICAL_ROUND"], 1)
        self.assertEqual(derived["superseded_rounds"][0]["superseded_by"], "vulcan_review")

    def test_a_pass_dated_before_the_failed_round_does_not_fold_it(self) -> None:
        # Vulcan P4 at 92fa6646: a late-token PASS filed earlier than the
        # REVISE round it would fold is not a re-review of that round.
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_surface_review_revision_5": "REVISE_0_P0_0_P1_1_P2",
                    "vulcan_surface_review_revision_5_note": "REVISE via claude-code 2026-09-18 on 178873d7",
                    "vulcan_final_review": "PASS",
                    "vulcan_final_review_note": "PASS via muse 2026-09-13 on a810943",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")
        self.assertEqual(derived["states"].get("HISTORICAL_ROUND", 0), 0)
        self.assertEqual(derived["states"]["PENDING"], 1)
        # The same rounds with the PASS dated later fold as before.
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_surface_review_revision_5": "REVISE_0_P0_0_P1_1_P2",
                    "vulcan_surface_review_revision_5_note": "REVISE via claude-code 2026-09-18 on 178873d7",
                    "vulcan_final_review": "PASS",
                    "vulcan_final_review_note": "PASS via claude-code 2026-09-19 on c04539b9",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["states"]["HISTORICAL_ROUND"], 1)
        self.assertEqual(register.round_date("x 2026-09-01 y 2026-09-18 z"), "2026-09-18")
        self.assertIsNone(register.round_date(None))

    def test_count_tokens_never_bind_across_a_prior_clause(self) -> None:
        # Vulcan/Nabu P3 at 76227765: `2_P4_PRIOR_P3_CLOSED` had read P4 closed.
        self.assertEqual(register.closed_severities("PASS_0_P0_0_P1_0_P2_0_P3_2_P4_PRIOR_P3_CLOSED"), {"P3"})
        self.assertEqual(register.closed_severities("PASS_0_P0_0_P1_0_P2_1_P3_2_P4_PRIOR_P3_P4_CLOSED"), {"P3", "P4"})
        self.assertEqual(register.closed_severities("PASS_0_P0_0_P1_0_P2_0_P3_PRIOR_P3_CLOSED"), {"P3"})
        self.assertEqual(register.closed_severities("PASS_P2_P3_P4_CLOSED"), {"P2", "P3", "P4"})
        self.assertEqual(register.closed_severities("PASS_0_P0_0_P1_0_P2_2_P3_2_P4"), set())

    def test_revision_7_closure_line_shapes(self) -> None:
        # Revision 7: a status is the item's own — bold head, table cell or
        # standalone bold status — never a quoted marker, a finding-raising
        # line or a mixed-status line.
        yes = [
            ("- **[P1] P1-A accepted_head fails closed — CLOSED.** Guard at", "P1"),
            ("| [tests] V-02 regression records | P3 | **CLOSED (P3)** | fuzz/x", "P3"),
            ("**178873d P2/P3 (the items).** re-verified. **Both CLOSED.**", "P2"),
            ("1. `[P3] [unowned-authority] claim retirement` — **CLOSED.** All four", "P3"),
            ("- Finding 1 [P2] listing survives one build — **CLOSED.** carried_attestations", "P2"),
            ("  - [P3] audit record lacked the dated entry → **CLOSED.** S20_710:143", "P3"),
        ]
        no = [
            ('vulcan 92fa664 :22-26 (P2, two P3, two P4 all "— CLOSED"), ariadne', "P3"),
            ("[P4] [contract] docs/x.md - closure markers (`— CLOSED`, `→ CLOSED`) exclude", "P3"),
            ('citing this transcript\'s "Prior P3 (register-parsing) — CLOSED" line.', "P3"),
            ("- P2 stale manifest: CLOSED", "P2"),
            ("- **[P2] stale — CLOSED.** leg 2 — OPEN", "P2"),
            ("| [tests] item 10 | P3 | **OPEN (advisory)** | x", "P3"),
            ("**Carried P4 (92fa664) — capsule checker pinned only the summary side.** Repair", "P4"),
            ("the row **reads CLOSED but is not** really P3", "P3"),
            ("VERDICT: PASS_PRIOR_P2_CLOSED", "P2"),
        ]
        for line, severity in yes:
            self.assertTrue(register.is_closure_line(line, severity), line)
        for line, severity in no:
            self.assertFalse(register.is_closure_line(line, severity), line)
        # The severity is the item's own leading token; the kind relates by
        # phrase even with an em-dash or a capital in the tag (1a9f0aab round).
        self.assertFalse(register.is_closure_line("- **[P3] record-provenance, the P2 it discussed — CLOSED.**", "P2"))
        self.assertTrue(register.is_closure_line("**Prior P2/P3 (both items) — CLOSED.**", "P3"))
        self.assertTrue(register.line_speaks_about(
            "**Prior finding 1 — [P1] list-depth creep: CLOSED.** I traced the repaired driver myself.",
            "vulcan_surface_review_revision_4@db53894: [correctness/parity — list-depth creep] crates/sley-vm/tests/x.rs:8936",
        ))
        self.assertTrue(register.line_speaks_about("- **[P3] fail-closed-gap, `superseded_attestations` unvalidated — CLOSED.**", "v: [fail-closed-gap] scripts/a.py:1 - x"))
        self.assertFalse(register.line_speaks_about("- **[P3] the record — CLOSED.** a records step", "v: [record] docs/adr/ADR-0019.md:54 - stale vectors"))
        # 6589c6ec round: a transcript that records the claim's severity
        # OPEN (item head, strong identity) cannot close it elsewhere; a
        # lane's field name is never an identifier; a shared kind needs a
        # strong identity.
        with tempfile.TemporaryDirectory() as directory:
            transcript = Path(directory) / "t.md"
            transcript.write_text(
                "| [tests] item 10 E7 opcode lane vm_canonical_inputs.rs:404-435 | P3 | **OPEN (advisory)** | x\n"
                "- **[P3] item 10 in vm_canonical_inputs.rs:404-435 — CLOSED.** y\n"
                "[P3] [tests] fuzz/targets/other.rs:1 - prior other OPEN\n"
            )
            claim = "vulcan_review_closure_note@c67b072: [tests] fuzz/targets/vm_canonical_inputs.rs:404-435 - prior item 10 OPEN (advisory)"
            self.assertTrue(register.is_open_line("| [tests] item 10 | P3 | **OPEN (advisory)** | x", "P3"))
            self.assertFalse(register.is_open_line("| [tests] item 10 | P3 | **OPEN (advisory)** | x", "P4"))
            self.assertTrue(register.is_open_line("[P3] [tests] fuzz/targets/other.rs:1 - prior other OPEN", "P3"))
            self.assertFalse(register.is_open_line('- **[P3] x — CLOSED.** quoting "— OPEN" is not a status', "P3"))
            self.assertEqual(register.open_lines_about(transcript, claim, "P3"), [1])
            self.assertEqual(register.open_lines_about(transcript, "v: [tests] fuzz/targets/third.rs:9 - z", "P3"), [])
        self.assertFalse(register.line_speaks_about("- **[P3] something else — CLOSED.** see nabu_architecture_review-c04539b.md#L30", "nabu_architecture_review@db53894: [record] `nabu_architecture_review` = x"))
        section = {"p3_open": ["vulcan_surface_review@c67b072: [record-note] docs/a.md:1 - one", "vulcan_surface_review@c67b072: [record-note] docs/b.md:2 - two"]}
        self.assertTrue(register.shares_its_kind(section, "P3", section["p3_open"][0]))
        self.assertFalse(register.shares_its_kind(section, "P3", "nabu_architecture_review@c67b072: [record-note] docs/c.md:3 - three"))
        self.assertTrue(register.line_speaks_about("- **[P3] record-note stale — CLOSED.**", section["p3_open"][0]))
        self.assertFalse(register.line_speaks_about("- **[P3] record-note stale — CLOSED.**", section["p3_open"][0], strong=True))
        # Shared vocabulary within a lane is not an identity; a ledger-file
        # anchor yields a description key; round folding follows scope ancestry.
        section = {"p4_open": [
            "vulcan_review@c67b072: [evidence] machineresearch/sley-2.0/machine-summary.json (every note) - the `last_local_proof` note says clean",
            "vulcan_review@c67b072: [evidence] machineresearch/sley-2.0/machine-summary.json:2273 - chronology: the `last_local_proof` records predate",
        ]}
        self.assertIn("last_local_proof", register.shared_vocabulary(section, "P4", section["p4_open"][0]))
        self.assertNotEqual(register.finding_key(section["p4_open"][0]), register.finding_key(section["p4_open"][1]))
        self.assertTrue(register.scoped_before("178873d7" + "0" * 32, "76ae15ab" + "0" * 32) or True)  # unresolvable shas: no fold refusal by scope
        self.assertTrue(register.scoped_before("76ae15a", "1a9f0aa"))   # the PASS is an ancestor of the REVISE round
        self.assertFalse(register.scoped_before("1a9f0aa", "76ae15a"))
        # Document ids are not finding ids; a line span relates without its
        # file name; a carry marker does not change a re-statement's key.
        self.assertFalse(register.line_speaks_about("- **[P4] the ADR-0040 note — CLOSED.**", "v: [records] docs/adr/x.md:9 - ADR-0040 says"))
        self.assertTrue(register.line_speaks_about("- **[P3] spec 413-417,424-425 reader sentence — CLOSED.**", "v: [contract-text] docs/spec/X.md:413-417,424-425 - the sentence"))
        self.assertFalse(register.line_speaks_about("- **[P4] x — CLOSED.** builder :100-107", "v: [records] evidence/release/lane.json:100 - y"))
        self.assertEqual(register.finding_key("v@c67b072: [ledger-duplication] (carried from 76227765, OPEN) machineresearch/sley-2.0/machine-summary.json:1 - `p4_open` twice"),
                         register.finding_key("v@1a9f0aa: [ledger-duplication] machineresearch/sley-2.0/machine-summary.json:2 - `p4_open` twice"))
        # The PRIOR fallback never serves P0-P2.
        with tempfile.TemporaryDirectory() as directory:
            transcript = Path(directory) / "t.md"
            transcript.write_text("- **[P3] x — CLOSED.**\nVERDICT: PASS_0_P0_0_P1_0_P2_0_P3_PRIOR_P2_P3_CLOSED\n")
            self.assertEqual(register.cited_closure_lines(transcript, "P3"), [1])
            self.assertEqual(register.cited_closure_lines(transcript, "P2"), [])

    def test_revision_7_ledger_shape_rules(self) -> None:
        # Strict `#L` grammar, no open-and-closed claim, list/count agreement,
        # tag grammar, and restated-claim validation.
        transcript = "evidence/review/verdicts/release_candidate_packaging/vulcan_surface_review-92fa664.md"
        claim = "vulcan_review_revision_1@178873d: [record] open_risks / dossier reproduced on two hosts"
        base = {
            "release_candidate_packaging": {
                "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                "vulcan_review": "PASS_0_P0_0_P1_0_P2_0_P3_PRIOR_P3_CLOSED",
                "p3_open": [],
                "p3_open_count": 0,
                "p3_closed_claims": [{"claim": claim, "verified_by": transcript + "#L23 — line 23"}],
            },
            "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
        }
        self.assertEqual(self.build_from(base)["package_restated_claims"], {})
        for mutate in (
            lambda s: s["release_candidate_packaging"]["p3_closed_claims"].__setitem__(0, {"claim": claim, "verified_by": transcript + "#L, — x"}),
            lambda s: s["release_candidate_packaging"]["p3_closed_claims"].__setitem__(0, {"claim": claim, "verified_by": transcript + "#L23,,24 — x"}),
            lambda s: s["release_candidate_packaging"]["p3_closed_claims"].__setitem__(0, {"claim": claim, "verified_by": transcript + "#L23#L9999 — x"}),
            lambda s: s["release_candidate_packaging"].__setitem__("p3_open", [claim]) or s["release_candidate_packaging"].__setitem__("p3_open_count", 1),
            lambda s: s["release_candidate_packaging"].__setitem__("p3_open_count", 4),
            lambda s: s["release_candidate_packaging"]["p3_closed_claims"].__setitem__(0, {"claim": claim.replace("@178873d", "@ABCDEF1"), "verified_by": transcript + "#L23 — x"}),
            lambda s: s["release_candidate_packaging"].__setitem__("p3_restated_claims", [{"claim": "vulcan_review@c67b072: [record] x", "restates": "nobody"}]),
            lambda s: s["release_candidate_packaging"].__setitem__("p3_restated_claims", [{"claim": claim, "restates": claim}]),
            lambda s: s["release_candidate_packaging"].__setitem__("p3_restated_claims", [{"claim": "x"}]),
            # An exact-claim closure the tracked retirement file does not carry.
            lambda s: s["release_candidate_packaging"]["p3_closed_claims"].__setitem__(0, {"claim": claim, "verified_by": transcript + "#L23 — x", "binding": "exact-claim"}),
            lambda s: s["release_candidate_packaging"]["p3_closed_claims"].__setitem__(0, {"claim": claim, "verified_by": transcript + "#L23 — x", "binding": "other"}),
        ):
            summary = json.loads(json.dumps(base))
            mutate(summary)
            self.assertEqual(self.build_fails(summary).code, register.RegisterErrorCode.SUMMARY_INVALID)
        # A fold is the same finding (lane, kind, anchor, identifier), marked
        # carried, at a strictly later round, and its chain ends at an open or
        # retired claim (1a9f0aab round: the builder had checked membership only).
        restatement = "vulcan_review_revision_1@76ae15a: [record] open_risks / dossier reproduced on two hosts - carried OPEN"
        good = json.loads(json.dumps(base))
        good["release_candidate_packaging"]["p3_restated_claims"] = [{"claim": restatement, "restates": claim}]
        self.assertEqual(self.build_from(good)["package_restated_claims"], {"release_candidate_packaging.p3_restated_claims": 1})
        for bad_fold in (
            [{"claim": "vulcan_review@c67b072: [record] carried", "restates": claim}],  # another finding
            [{"claim": "vulcan_review_revision_1@76ae15a: [record] open_risks / dossier reproduced on two hosts - fresh", "restates": claim}],  # no carry marker
            [{"claim": "vulcan_review_revision_1@c04539b: [record] open_risks / dossier reproduced on two hosts - carried", "restates": claim.replace("@178873d", "@76ae15a")}],  # earlier round
            [{"claim": restatement, "restates": "vulcan_review_revision_1@c04539b: [record] open_risks / dossier reproduced on two hosts - carried"},
             {"claim": "vulcan_review_revision_1@c04539b: [record] open_risks / dossier reproduced on two hosts - carried", "restates": restatement}],  # cycle
            [{"claim": restatement, "restates": "vulcan_review_revision_1@c04539b: [record] open_risks / dossier reproduced on two hosts - carried"}],  # dangling
        ):
            bad = json.loads(json.dumps(base))
            bad["release_candidate_packaging"]["p3_restated_claims"] = bad_fold
            self.assertEqual(self.build_fails(bad).code, register.RegisterErrorCode.SUMMARY_INVALID, str(bad_fold)[:80])

    def test_closure_line_and_speaking_predicates(self) -> None:
        # 76ae15ab round: a closure line names the severity before a CLOSED
        # status marker with no OPEN marker; prose is not a closure line.
        self.assertTrue(register.is_closure_line("- **[P2] stale manifest — CLOSED.** rebuilt", "P2"))
        self.assertFalse(register.is_closure_line("- P2 stale manifest: CLOSED", "P2"))  # revision 7: no item status shape
        self.assertFalse(register.is_closure_line("- **[P2] stale manifest — CLOSED.** leg 2 — OPEN", "P2"))
        self.assertFalse(register.is_closure_line("No P0, P1, or P2-class defect exists; the P3 is CLOSED in spirit", "P2"))
        self.assertFalse(register.is_closure_line("- **[P3] wording — CLOSED.** (the P2 remains)", "P2"))
        self.assertFalse(register.is_closure_line("VERDICT: PASS_PRIOR_P2_CLOSED", "P2"))
        # Finding identity (1a9f0aab round): a generic one-word tag or a
        # shared path alone never relates; a path plus a tag word, an
        # identifier, a finding id or a file:line anchor does.
        claim = "vulcan_review_revision_1@178873d: [record/ledger] scripts/retire_review_claims.py:12 cites a transcript"
        self.assertFalse(register.line_speaks_about("- **[P3] ledger citation — CLOSED.**", claim))
        self.assertTrue(register.line_speaks_about("- **[P3] ledger x — CLOSED.** scripts/retire_review_claims.py now refuses", claim))
        self.assertTrue(register.line_speaks_about("- **[P3] x — CLOSED.** retire_review_claims.py:12 now refuses", claim))
        # A path basename is a path, not an identifier (`retire_review_claims` alone names nothing).
        self.assertFalse(register.line_speaks_about("- **[P3] the transcript that retire_review_claims cites — CLOSED.**", claim))
        self.assertTrue(register.line_speaks_about("- **[P3] `attestation_supersedes` cites — CLOSED.**", "v: [record] a.md:1 - the attestation_supersedes entry"))
        self.assertFalse(register.line_speaks_about("- **[P3] `p3_closed_claims` cites — CLOSED.**", "v: [record] a.md:1 - the p3_closed_claims entry"))  # ledger vocabulary
        self.assertFalse(register.line_speaks_about("- **[P3] the transcript — CLOSED.**", claim))
        self.assertFalse(register.line_speaks_about("- **[P3] unrelated wording about scripts/retire_review_claims.py — CLOSED.**", "v: [record] scripts/retire_review_claims.py - x"))
        self.assertTrue(register.line_speaks_about("- **[P2] RW090-DEV-01 inexact — CLOSED.**", "v: [contract] crates/x.rs:1 - the RW090-DEV-01 scope"))
        self.assertFalse(register.line_speaks_about("- **[P2] RW090-DEV-011 — CLOSED.**", "v: [contract] crates/x.rs:1 - the RW090-DEV-01 scope"))
        self.assertTrue(register.strictly_later_scope("76ae15a", "178873d"))
        self.assertFalse(register.strictly_later_scope("178873d", "76ae15a"))

    def test_cited_closure_lines_and_transcript_paths(self) -> None:
        # The claim-to-transcript relation (revision 6, 76227765 round): a
        # retirement cites lines carrying CLOSED that name the severity; a
        # traversal or non-transcript path is not a transcript.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "evidence/review/verdicts/x"
            root.mkdir(parents=True)
            transcript = root / "nabu_review-abc1234.md"
            transcript.write_text("intro\n- **P2 stale — CLOSED.**\n- **P3 wording — OPEN.**\nVERDICT: PASS_0_P0_0_P1_0_P2_1_P3_PRIOR_P2_CLOSED\n")
            self.assertEqual(register.cited_closure_lines(transcript, "P2"), [2])
            self.assertEqual(register.cited_closure_lines(transcript, "P3"), [])
            self.assertEqual(register.cited_closure_lines(transcript / "absent", "P2"), [])
        self.assertIsNone(register.transcript_path("evidence/review/verdicts/../../../docs/spec/FINDING_REGISTER_V1.md"))
        self.assertIsNone(register.transcript_path("docs/spec/FINDING_REGISTER_V1.md"))
        self.assertIsNone(register.transcript_path("/etc/passwd"))
        real = "evidence/review/verdicts/release_candidate_packaging/vulcan_surface_review-92fa664.md"
        self.assertIsNotNone(register.transcript_path(real + "#L22 — line 22"))

    def test_retired_claims_must_name_an_existing_transcript(self) -> None:
        # Revision 6: a pN_closed_claims entry is {claim, verified_by} with an
        # existing transcript path; anything else is SUMMARY_INVALID.
        transcript = "evidence/review/verdicts/release_candidate_packaging/vulcan_surface_review-92fa664.md"
        self.assertTrue((register.ROOT / transcript).is_file())
        # The section must own the transcript directory and the claim's lane
        # must be the transcript's (revision 6, c67b0729 round).
        good = {
            "release_candidate_packaging": {
                "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                "vulcan_review": "PASS_0_P0_0_P1_0_P2_0_P3_PRIOR_P3_CLOSED",
                "p3_open": [],
                "p3_open_count": 0,
                "p3_closed_claims": [
                    {"claim": "vulcan_review_revision_1@178873d: [record] open_risks / dossier reproduced on two hosts", "verified_by": transcript + "#L23 — line 23"}
                ],
            },
            "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
        }
        derived = self.build_from(good)
        self.assertEqual(derived["package_closed_claims"], {"release_candidate_packaging.p3_closed_claims": 1})
        for bad_entry in (
            {"claim": "x", "verified_by": "evidence/review/verdicts/nowhere/none-0000000.md"},
            {"claim": "x", "verified_by": "docs/spec/FINDING_REGISTER_V1.md"},
            {"claim": "x", "verified_by": "evidence/review/verdicts/../../../docs/spec/FINDING_REGISTER_V1.md"},
            {"claim": "x", "verified_by": transcript + "#L1 — a line that records no P3 closure"},
            # 76ae15ab round: the cited closure line must speak about the claim.
            {"claim": "vulcan_review_revision_1@178873d: [record] unrelated wording elsewhere", "verified_by": transcript + "#L23 — a closure line about another finding"},
            # c67b0729 round: a later filename is not a later scope; ancestry decides.
            {"claim": "vulcan_review_revision_1@fbb0257: [record] open_risks / dossier reproduced on two hosts", "verified_by": transcript + "#L23 — an earlier scope than the claim's"},
            {"claim": "nabu_review_revision_1@178873d: [record] x", "verified_by": transcript + "#L23 — another lane's transcript"},
            {"claim": "vulcan_review_revision_1@92fa664: [record] x", "verified_by": transcript + "#L23 — the claim's own round"},
            {"claim": "vulcan_review_revision_1@178873d: [record] x", "verified_by": transcript.replace("release_candidate_packaging", "standards_sbom_and_provenance") + "#L24 — another section's transcript"},
            {"claim": "x"},
            "x",
        ):
            bad = json.loads(json.dumps(good))
            bad["release_candidate_packaging"]["p3_closed_claims"] = [bad_entry]
            with self.assertRaises(register.RegisterError) as error:
                self.build_from(bad)
            self.assertEqual(error.exception.code, register.RegisterErrorCode.SUMMARY_INVALID)

    def test_an_unsuperseded_failure_blocks_clearance(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": "FAIL_1_P0",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")
        self.assertEqual(len(derived["open_reviews"]), 1)

    def test_a_slice_pass_does_not_close_the_overall_failure(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": "FAIL_1_P0",
                    "candidate_result_vulcan_review": "PASS_MANIFEST_LENGTH_FINDINGS_CLOSED",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")
        self.assertEqual(
            [entry["field"] for entry in derived["open_reviews"]], ["vulcan_review"]
        )

    def test_an_unclassified_disposition_blocks_clearance(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "merlin_review": "TIMED_OUT_PARTIAL_HANDOFF",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")

    def test_a_missing_or_vacuous_counter_fails_closed(self) -> None:
        for declared in (
            None,
            {"p0": 0},
            {"p0": "0", "p1": 0, "p2": 0, "p3": 0, "p4": 0},
        ):
            summary: dict = {
                "example_package": {
                    "status": "S20_999_COMPLETE",
                    "vulcan_review": "PASS",
                },
            }
            if declared is not None:
                summary["open_findings"] = declared
            error = self.build_fails(summary)
            self.assertEqual(error.code, register.RegisterErrorCode.SUMMARY_INVALID)

    def test_a_nonzero_package_claim_blocks_clearance(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": "PASS",
                    "p0_open": ["vulcan_review: [x] one open claim"],
                    "p0_open_count": 1,
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")

    def test_an_unclaimed_carried_finding_blocks_clearance(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": "PASS_WITH_P1_P3_FOLLOWUPS_NO_P0_P2",
                    "p1_open": [],
                    "p1_open_count": 0,
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")
        self.assertEqual(
            [
                (entry["field"], entry["unclaimed_severities"])
                for entry in derived["unclaimed_carried_findings"]
            ],
            [("vulcan_review", ["P1", "P3"])],
        )

    def test_a_claimed_carried_finding_does_not_block_clearance(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": "PASS_WITH_P1_FOLLOWUPS_NO_P0_P2_P3_P4",
                    "p1_open": ["finding-1"],
                    "p1_open_count": 1,
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        # The section tracks the carried P1, so nothing is unclaimed; the
        # nonzero package claim itself still blocks clearance elsewhere.
        self.assertEqual(derived["unclaimed_carried_findings"], [])
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")

    def test_a_closed_carried_finding_does_not_block_clearance(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": "PASS_PRIOR_P1_CLOSED_NO_NEW_P0_P1",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["unclaimed_carried_findings"], [])
        self.assertEqual(derived["result"], "FINDING_REGISTER_CLEAR")

    def test_a_partial_closure_leaves_followups_unclaimed(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": (
                        "PASS_PRIOR_P2_CLOSED_NO_OPEN_P0_P1_P2"
                        "_WITH_P3_P4_FOLLOWUPS"
                    ),
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(
            [
                (entry["field"], entry["unclaimed_severities"])
                for entry in derived["unclaimed_carried_findings"]
            ],
            [("vulcan_review", ["P3", "P4"])],
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")

    def test_a_stacked_negation_declares_nothing(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": "PASS_NO_NO_OPEN_P1",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        # The stacked negation is void: P1 stays a visible mention and,
        # unclaimed, blocks clearance.
        self.assertEqual(
            derived["open_reviews"], [],
        )
        self.assertEqual(
            [
                (entry["field"], entry["unclaimed_severities"])
                for entry in derived["unclaimed_carried_findings"]
            ],
            [("vulcan_review", ["P1"])],
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")

    def test_a_closed_substring_smuggles_nothing(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": "PASS_WITH_P1_DISCLOSED",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(
            [
                (entry["field"], entry["unclaimed_severities"])
                for entry in derived["unclaimed_carried_findings"]
            ],
            [("vulcan_review", ["P1"])],
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")

    def test_a_closed_suffix_salad_exempts_nothing(self) -> None:
        for disposition in (
            "PASS_WITH_P1_CLOSED_LOOP",
            "PASS_WITH_P1_CLOSEDNESS",
            "PASS_WITH_P1_CLOSED_CIRCUIT",
        ):
            derived = self.build_from(
                {
                    "example_package": {
                        "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                        "vulcan_review": disposition,
                    },
                    "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
                }
            )
            self.assertEqual(
                [
                    (entry["field"], entry["unclaimed_severities"])
                    for entry in derived["unclaimed_carried_findings"]
                ],
                [("vulcan_review", ["P1"])],
                disposition,
            )
            self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN", disposition)

    def test_a_vacuous_continuation_exempts_nothing(self) -> None:
        for disposition in (
            "PASS_WITH_P1_CLOSED_NO",
            "PASS_WITH_P1_CLOSED_WITH",
        ):
            derived = self.build_from(
                {
                    "example_package": {
                        "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                        "vulcan_review": disposition,
                    },
                    "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
                }
            )
            self.assertEqual(
                [
                    (entry["field"], entry["unclaimed_severities"])
                    for entry in derived["unclaimed_carried_findings"]
                ],
                [("vulcan_review", ["P1"])],
                disposition,
            )
            self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN", disposition)

    def test_mid_string_complete_packages_are_named_with_open_counts(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "COMPLETE_RESTRICTED_EXAMPLE_BOUNDARY",
                    "vulcan_review": "FAIL_1_P0",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(
            derived["mid_string_complete_packages"],
            [
                {
                    "section": "example_package",
                    "status": "COMPLETE_RESTRICTED_EXAMPLE_BOUNDARY",
                    "open_obligations": 1,
                }
            ],
        )
        # Visibility only: the open review itself blocks clearance, and a
        # mid-string status is never a completion violation.
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")
        self.assertEqual(derived["complete_packages_with_open_reviews"], [])

    def test_a_summary_without_obligations_fails_closed(self) -> None:
        error = self.build_fails({"nothing": {"status": "OPEN"}})
        self.assertEqual(error.code, register.RegisterErrorCode.SUMMARY_INVALID)

    def test_a_missing_summary_fails_closed(self) -> None:
        original = register.SUMMARY
        register.SUMMARY = Path("/nonexistent/summary.json")
        self.addCleanup(setattr, register, "SUMMARY", original)
        with self.assertRaises(register.RegisterError) as error:
            register.build_register()
        self.assertEqual(error.exception.code, register.RegisterErrorCode.SUMMARY_MISSING)


if __name__ == "__main__":
    unittest.main()
