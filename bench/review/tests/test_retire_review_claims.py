"""The claim-retirement tool: automatic and explicit rules, replay, folding."""

import json
import subprocess
import sys
import tempfile
import unittest
import unittest.mock
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
import build_finding_register as register  # noqa: E402
import retire_review_claims as retire  # noqa: E402


class RetireReviewClaimsTests(unittest.TestCase):
    """A fixture repository with two rounds of one lane's transcripts."""

    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-q")
        (self.root / "x").write_text("a\n")
        self.commit("first")
        self.first = self.git("rev-parse", "HEAD").strip()
        (self.root / "x").write_text("b\n")
        self.commit("second")
        self.second = self.git("rev-parse", "HEAD").strip()
        self.section = self.root / "evidence/review/verdicts/release_candidate_packaging"
        self.section.mkdir(parents=True)
        (self.section / f"vulcan_surface_review-{self.first[:7]}.md").write_text(
            "FINDINGS: [P3] [record-note] scripts/a.py:1 - stale note\nVERDICT: PASS_0_P0_0_P1_0_P2_1_P3\n"
        )
        (self.section / f"vulcan_surface_review-{self.second[:7]}.md").write_text(
            "- **[P3] record-note stale in scripts/a.py — CLOSED.** repaired\n"
            "VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_PRIOR_P3_CLOSED\n"
        )
        self.git("add", ".")  # transcripts must be in the index (tracked or staged)
        for module in (register, retire):
            self.addCleanup(setattr, module, "ROOT", module.ROOT)
            module.ROOT = self.root
        register._tracked = None
        self.addCleanup(setattr, register, "_tracked", None)
        register._ledger_words = None
        self.addCleanup(setattr, register, "_ledger_words", None)
        register._verdict_fields = None
        self.addCleanup(setattr, register, "_verdict_fields", None)
        retire._ancestry.clear()
        retire._resolved.clear()
        register._scope_order.clear()
        register._raising.clear()
        register._generation.clear()

    def git(self, *args: str) -> str:
        return subprocess.check_output(
            ["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", *args],
            cwd=self.root, text=True,
        )

    def commit(self, message: str) -> None:
        self.git("add", ".")
        self.git("commit", "-qm", message)

    def summary(self, claim: str) -> dict:
        return {
            "release_candidate_packaging": {
                "vulcan_surface_review": "PASS_0_P0_0_P1_0_P2_0_P3_PRIOR_P3_CLOSED",
                "vulcan_surface_review_note": f"on {self.second}: transcript evidence/review/verdicts/release_candidate_packaging/vulcan_surface_review-{self.second[:7]}.md",
                "p3_open": [claim],
                "p3_open_count": 1,
            }
        }

    def test_the_automatic_rule_retires_a_strictly_earlier_claim_only(self) -> None:
        summary = self.summary(f"vulcan_surface_review@{self.first[:7]}: [record-note] scripts/a.py:1 - stale note")
        self.assertEqual(retire.retire(summary, []), 1)
        closed = summary["release_candidate_packaging"]["p3_closed_claims"][0]
        self.assertIn(f"vulcan_surface_review-{self.second[:7]}.md#L1", closed["verified_by"])
        self.assertEqual(summary["release_candidate_packaging"]["p3_open"], [])
        # The claim's own round never retires; an untagged claim retires only
        # through its raising transcript's scope (79fdcc63 round) — here the
        # first-round FINDINGS line raises it, so the second round closes it.
        summary = self.summary(f"vulcan_surface_review@{self.second[:7]}: [record-note] scripts/a.py:1 - stale note")
        self.assertEqual(retire.retire(summary, []), 0)
        summary = self.summary("vulcan_surface_review: [record-note] scripts/a.py:1 - stale note")
        self.assertEqual(retire.retire(summary, []), 1)
        summary = self.summary("vulcan_surface_review: [record-note] scripts/a.py:1 - a note no transcript raised")
        self.assertEqual(retire.retire(summary, []), 0)
        # A closure line that does not speak about the claim retires nothing.
        summary = self.summary(f"vulcan_surface_review@{self.first[:7]}: [wording] docs/other.md:9 - unrelated")
        self.assertEqual(retire.retire(summary, []), 0)

    def test_transcript_closers_come_from_filed_verdict_lines(self) -> None:
        closers = retire.transcript_closers("release_candidate_packaging")
        self.assertEqual(len(closers), 1)
        _, lane, severities, scope, path = closers[0]
        self.assertEqual((lane, severities, scope), ("vulcan", {"P3"}, self.second))
        self.assertTrue(path.endswith(f"vulcan_surface_review-{self.second[:7]}.md"))

    def test_the_explicit_rule_needs_speaking_closure_lines_and_a_field_prefix(self) -> None:
        claim = "vulcan_surface_review: [record-note] scripts/a.py:1 - stale note"
        path = f"evidence/review/verdicts/release_candidate_packaging/vulcan_surface_review-{self.second[:7]}.md"
        entry = {"section": "release_candidate_packaging", "severities": [3], "prefixes": ["vulcan_surface_review: [record-note]"],
                 "verified_by": path, "reason": "closed at the second round"}
        summary = self.summary(claim)
        self.assertEqual(retire.retire(summary, [entry]), 1)
        self.assertIn("#L1 — closed at the second round", summary["release_candidate_packaging"]["p3_closed_claims"][0]["verified_by"])
        for bad in (
            dict(entry, lines=[2]),
            dict(entry, prefixes=["vulcan_surface_review"]),
            dict(entry, prefixes=[""]),
        ):
            with self.assertRaises(SystemExit):
                retire.retire(self.summary(claim), [bad])

    def test_a_lane_less_field_retires_through_the_explicit_rule(self) -> None:
        # 1a9f0aab round: a lane-less field (a closure note) had no retirement path.
        section = self.root / "evidence/review/verdicts/mutation_value_profile"
        section.mkdir(parents=True)
        (section / f"epoch1_reanchor_review_closure-{self.second[:7]}.md").write_text(
            "- **[P4] capsule checker pinned only the summary side — CLOSED.** scripts/check_capsule.py:32\nVERDICT: PASS\n"
        )
        self.git("add", ".")
        register._tracked = None
        summary = {"mutation_value_profile": {
            # The claim was raised by the closure review at `first`; the review
            # at `second` closes it (an own-round line never retires a claim).
            "epoch1_reanchor_review_closure_note": f"on {self.first}: verified",
            "p4_open": ["epoch1_reanchor_review_closure_note: [checker] scripts/check_capsule.py:24 - pinned only the summary side"],
            "p4_open_count": 1,
        }}
        entry = {"section": "mutation_value_profile", "severities": [4],
                 "prefixes": ["epoch1_reanchor_review_closure_note: [checker]"],
                 "verified_by": f"evidence/review/verdicts/mutation_value_profile/epoch1_reanchor_review_closure-{self.second[:7]}.md",
                 "reason": "closed by the closure review"}
        self.assertEqual(retire.retire(summary, [entry]), 1)
        with self.assertRaises(SystemExit):
            retire.retire({"mutation_value_profile": {"p4_open": ["x_note: [checker] y"], "p4_open_count": 1}},
                          [dict(entry, prefixes=["x_note: [checker]"])])

    def test_a_one_line_prior_transcript_retires_exactly_the_claim_it_names(self) -> None:
        # Nabu P2 at 1a9f0aab: three same-lane claims sharing paths; the
        # single closure line names one finding identity.
        (self.section / f"vulcan_surface_review-{self.second[:7]}.md").write_text(
            "- **[P3] chain derivation fail-open in scripts/sync.py — CLOSED.** fixed\n"
            "VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_PRIOR_P3_CLOSED\n"
        )
        self.git("add", ".")
        register._tracked = None
        claims = [
            f"vulcan_surface_review@{self.first[:7]}: [chain-derivation] scripts/sync.py:10 - the chain is derived fail-open",
            f"vulcan_surface_review@{self.first[:7]}: [record-wording] scripts/sync.py:20 - a note reads wrongly",
            f"vulcan_surface_review@{self.first[:7]}: [test-coverage] scripts/sync.py:30 - no test for the derivation",
        ]
        summary = {"release_candidate_packaging": {"p3_open": list(claims), "p3_open_count": 3}}
        self.assertEqual(retire.retire(summary, []), 1)
        self.assertEqual(summary["release_candidate_packaging"]["p3_open"], claims[1:])
        self.assertEqual(summary["release_candidate_packaging"]["p3_closed_claims"][0]["claim"], claims[0])

    def test_an_exact_claim_binding_needs_the_tracked_entry_and_a_closure_line(self) -> None:
        # The reviewer's own per-finding closure line, bound by the whole
        # claim string where the identity relation cannot see it.
        claim = f"vulcan_surface_review@{self.first[:7]}: [wording] docs/other.md:9 - phrased differently"
        path = f"evidence/review/verdicts/release_candidate_packaging/vulcan_surface_review-{self.second[:7]}.md"
        entry = {"section": "release_candidate_packaging", "verified_by": path,
                 "claims": [{"claim": claim, "lines": [1], "reason": "the reviewer's line 1 closes this finding"}]}
        summary = self.summary(claim)
        self.assertEqual(retire.retire(summary, [entry]), 1)
        closed = summary["release_candidate_packaging"]["p3_closed_claims"][0]
        self.assertEqual(closed["binding"], "exact-claim")
        with self.assertRaises(SystemExit):
            retire.retire(self.summary(claim), [dict(entry, claims=[{"claim": claim, "lines": [2]}])])
        # A prefix of the claim is not an exact binding.
        self.assertEqual(retire.retire(self.summary(claim), [dict(entry, claims=[{"claim": claim[:30], "lines": [1]}])]), 0)
        # The builder refuses an exact-claim closure the tracked file does not carry.
        with unittest.mock.patch.object(register, "RETIREMENTS", self.root / "absent.json"):
            self.assertFalse(register.exact_claim_binding("release_candidate_packaging", claim, self.root / path, [1]))
        (self.root / "retirements.json").write_text(json.dumps([entry]))
        with unittest.mock.patch.object(register, "RETIREMENTS", self.root / "retirements.json"):
            self.assertTrue(register.exact_claim_binding("release_candidate_packaging", claim, self.root / path, [1]))
            self.assertFalse(register.exact_claim_binding("release_candidate_packaging", claim, self.root / path, [2]))

    def test_a_shared_kind_needs_a_strong_identity(self) -> None:
        # Ariadne P2 at 6589c6ec: one closure line's kind matched three open
        # claims of its lane; only the claim it identifies strongly retires.
        (self.section / f"vulcan_surface_review-{self.second[:7]}.md").write_text(
            "- **[P3] [record-note] the `attestation_supersedes` note — CLOSED.** fixed\n"
            "VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_PRIOR_P3_CLOSED\n"
        )
        self.git("add", ".")
        register._tracked = None
        claims = [
            f"vulcan_surface_review@{self.first[:7]}: [record-note] docs/a.md:1 - the `attestation_supersedes` note is stale",
            f"vulcan_surface_review@{self.first[:7]}: [record-note] docs/b.md:2 - another note is stale",
        ]
        summary = {"release_candidate_packaging": {"p3_open": list(claims), "p3_open_count": 2}}
        self.assertEqual(retire.retire(summary, []), 1)
        self.assertEqual(summary["release_candidate_packaging"]["p3_open"], claims[1:])

    def test_a_filed_transcript_closes_by_its_status_lines_without_a_prior_token(self) -> None:
        # Ariadne P3 at 6589c6ec: five closures had no closer because the
        # verdict tokens omitted the PRIOR suffix.
        (self.section / f"vulcan_surface_review-{self.second[:7]}.md").write_text(
            "- **[P3] record-note stale in scripts/a.py — CLOSED.** repaired\n"
            "[P4] [wording] docs/new.md:1 - a new finding\n"
            "VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_1_P4\n"
        )
        self.git("add", ".")
        register._tracked = None
        closers = retire.transcript_closers("release_candidate_packaging")
        self.assertEqual([(c[1], c[2]) for c in closers], [("vulcan", {"P3"})])
        summary = self.summary(f"vulcan_surface_review@{self.first[:7]}: [record-note] scripts/a.py:1 - stale note")
        self.assertEqual(retire.retire(summary, []), 1)

    def test_an_untagged_claim_takes_its_raising_transcripts_scope(self) -> None:
        # 79fdcc63 round: an own-round line never retires an untagged claim;
        # the raising transcript (its [Pn] line) gives the claim its scope.
        claim = "vulcan_surface_review: [record-note] scripts/a.py:1 - stale note"
        self.assertEqual(register.raising_scope("release_candidate_packaging", claim, {}), self.first[:7])
        summary = self.summary(claim)
        self.assertEqual(retire.retire(summary, []), 1)  # the second-round transcript closes it
        own = {"section": "release_candidate_packaging", "severities": [3], "prefixes": ["vulcan_surface_review: [record-note]"],
               "verified_by": f"evidence/review/verdicts/release_candidate_packaging/vulcan_surface_review-{self.first[:7]}.md", "reason": "x"}
        (self.section / f"vulcan_surface_review-{self.first[:7]}.md").write_text(
            "FINDINGS: [P3] [record-note] scripts/a.py:1 - stale note\n- **[P3] record-note stale in scripts/a.py — CLOSED.**\nVERDICT: PASS\n"
        )
        self.git("add", ".")
        register._tracked = None
        register._raising.clear()
        with self.assertRaises(SystemExit):
            retire.retire(self.summary(claim), [own])

    def test_closers_bind_the_earliest_transcript_and_prefer_own_head_lines(self) -> None:
        (self.root / "x").write_text("c\n")
        self.commit("third")
        third = self.git("rev-parse", "HEAD").strip()
        (self.section / f"vulcan_surface_review-{third[:7]}.md").write_text(
            "- **[P3] something else — CLOSED.** the record-note stale item in scripts/a.py was handled earlier\n"
            "VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_PRIOR_P3_CLOSED\n"
        )
        self.git("add", ".")
        register._tracked = None
        retire._resolved.clear()
        claim = f"vulcan_surface_review@{self.first[:7]}: [record-note] scripts/a.py:1 - stale note"
        summary = self.summary(claim)
        retire.retire(summary, [])
        self.assertIn(f"-{self.second[:7]}.md#L1", summary["release_candidate_packaging"]["p3_closed_claims"][0]["verified_by"])

    def test_one_generic_head_closes_no_finding_under_a_shared_kind(self) -> None:
        # Nabu/Vulcan P2 at b58ac1e0: a head naming only the kind and the
        # file must not retire the lane's several findings of that kind
        # against that file; an identifier in the head retires exactly one.
        (self.section / f"vulcan_surface_review-{self.second[:7]}.md").write_text(
            "- **[P3] [robustness] scripts/build_finding_register.py — CLOSED.** generic\n"
            "- **[P3] [robustness] `finding_key` in scripts/build_finding_register.py — CLOSED.** specific\n"
            "VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_PRIOR_P3_CLOSED\n"
        )
        self.git("add", ".")
        register._tracked = None
        claims = [
            f"vulcan_surface_review@{self.first[:7]}: [robustness] scripts/build_finding_register.py:10 - the `finding_key` anchor is coarse",
            f"vulcan_surface_review@{self.first[:7]}: [robustness] scripts/build_finding_register.py:20 - the `closure_head` read is partial",
        ]
        summary = {"release_candidate_packaging": {"p3_open": list(claims), "p3_open_count": 2}}
        self.assertEqual(retire.retire(summary, []), 1)
        self.assertEqual(summary["release_candidate_packaging"]["p3_open"], claims[1:])
        self.assertIn("#L2", summary["release_candidate_packaging"]["p3_closed_claims"][0]["verified_by"])
        # 8f774d0c round: an identifier-less original or a prose carry word
        # does not make the kind unshared — the generic head still closes nothing.
        for shapes in (
            [claims[0].replace("`finding_key`", "the key"), claims[1]],
            [claims[0] + " again", claims[1] + ", unchanged"],
        ):
            summary = {"release_candidate_packaging": {"p3_open": list(shapes), "p3_open_count": 2}}
            retired = retire.retire(summary, [])
            self.assertEqual(retired, 1 if "`finding_key`" in shapes[0] else 0, shapes)

    def test_regeneration_must_reproduce_the_tracked_ledger(self) -> None:
        claim = f"vulcan_surface_review@{self.first[:7]}: [record-note] scripts/a.py:1 - stale note"
        summary = self.summary(claim)
        retire.retire(summary, [])
        self.assertEqual(retire.regeneration_divergence(summary, []), [])
        tampered = json.loads(json.dumps(summary))
        tampered["release_candidate_packaging"]["p3_closed_claims"].append({"claim": "vulcan_surface_review@0000000: [x] y", "verified_by": "z#L1 — w"})
        self.assertEqual(len(retire.regeneration_divergence(tampered, [])), 1)

    def test_replay_refuses_a_stale_or_duplicated_closure(self) -> None:
        claim = f"vulcan_surface_review@{self.first[:7]}: [record-note] scripts/a.py:1 - stale note"
        summary = self.summary(claim)
        retire.retire(summary, [])
        self.assertEqual(retire.replay_problems(summary), [])
        stale = json.loads(json.dumps(summary))
        stale["release_candidate_packaging"]["p3_closed_claims"][0]["verified_by"] = stale["release_candidate_packaging"]["p3_closed_claims"][0]["verified_by"].replace("#L1", "#L2")
        self.assertEqual(len(retire.replay_problems(stale)), 1)
        duplicated = json.loads(json.dumps(summary))
        duplicated["release_candidate_packaging"]["p3_open"] = [claim]
        self.assertTrue(any("open and closed" in problem for problem in retire.replay_problems(duplicated)))

    def test_an_untracked_transcript_is_not_a_filed_transcript(self) -> None:
        # Vulcan P3 at 1a9f0aab: an untracked concurrent-lane file had flipped --check.
        untracked = self.section / f"vulcan_surface_review_extra-{self.second[:7]}.md"
        untracked.write_text("- **[P3] record-note stale in scripts/a.py — CLOSED.**\nVERDICT: PASS_PRIOR_P3_CLOSED\n")
        claim = "vulcan_surface_review: [record-note] scripts/a.py:1 - stale note"
        entry = {"section": "release_candidate_packaging", "severities": [3], "prefixes": ["vulcan_surface_review: [record-note]"],
                 "verified_by": untracked.relative_to(self.root).as_posix(), "reason": "x"}
        with self.assertRaises(SystemExit):
            retire.retire(self.summary(claim), [entry])
        self.assertIsNone(register.transcript_path(untracked.relative_to(self.root).as_posix()))

    def test_transcript_resolution_fails_closed_on_ambiguity(self) -> None:
        (self.section / f"vulcan_surface_review_final-{self.second[:7]}.md").write_text("VERDICT: PASS\n")
        with self.assertRaises(SystemExit):
            retire.transcript_for("release_candidate_packaging", "vulcan", self.second, None)
        self.assertEqual(retire.role("current_delta_review.nabu"), "nabu")

    def test_restatements_fold_into_the_named_root(self) -> None:
        # 8966da2e round: the root is the claim at the round the
        # re-statement names — never the earliest listed nor the farthest
        # ancestor. A re-statement whose named sha matches nothing stays open.
        earliest = f"vulcan_surface_review@{self.first[:7]}: [record] scripts/a.py:1 - `note` stale"
        carried = f"vulcan_surface_review@{self.second[:7]}: [record] (carried from {self.first[:7]}, OPEN) scripts/a.py:3 - `note` still stale"
        fresh = f"vulcan_surface_review@{self.second[:7]}: [record] scripts/a.py:3 - a new `note` finding"
        other = f"vulcan_surface_review@{self.second[:7]}: [record] (carried from {self.first[:7]}, OPEN) scripts/b.py:3 - another `note`"
        summary = {"s": {"p4_open": [earliest, carried, fresh, other], "p4_open_count": 4}}
        self.assertEqual(retire.fold_restatements(summary), 1)
        self.assertEqual(summary["s"]["p4_open"], [earliest, fresh, other])
        self.assertEqual(summary["s"]["p4_restated_claims"], [{"claim": carried, "restates": earliest}])
        self.assertEqual(retire.fold_restatements(summary), 0)

    def test_restatement_naming_an_absent_root_stays_open(self) -> None:
        # The 8966da2e P2: five `[predicate-precision]` re-statements naming
        # later rounds had folded into a closed root at an earlier round.
        root = f"vulcan_surface_review@{self.first[:7]}: [record] scripts/a.py:1 - `alpha_guard` stale"
        carried = f"vulcan_surface_review@{self.second[:7]}: [record] (carried from {self.second[:7]}, OPEN) scripts/a.py:3 - `alpha_guard` still stale"
        summary = {"s": {"p4_open": [root, carried], "p4_open_count": 2}}
        self.assertEqual(retire.fold_restatements(summary), 0)
        self.assertEqual(summary["s"]["p4_open"], [root, carried])
        self.assertNotIn("p4_restated_claims", summary["s"])

    def test_fold_prefers_the_open_named_root_over_a_closed_twin(self) -> None:
        # Vulcan P2 at 8966da2e: two same-kind same-file originals, one
        # closed, and a carried re-statement of the OPEN one folds into
        # the open root — never into the closed twin.
        shutter = self.section / f"vulcan_surface_review-{self.first[:7]}.md"
        shutter.write_text("- **[P4] record `alpha_guard` stale in scripts/a.py — CLOSED.**\n")
        self.git("add", ".")
        alpha = f"vulcan_surface_review@{self.first[:7]}: [record] scripts/a.py:1 - `alpha_guard` stale"
        beta = f"vulcan_surface_review@{self.first[:7]}: [record] scripts/a.py:1 - `beta_guard` stale"
        beta_copy = f"vulcan_surface_review@{self.second[:7]}: [record] (carried from {self.first[:7]}, OPEN) scripts/a.py:3 - `beta_guard` still stale"
        summary = {"s": {
            "p4_open": [beta, beta_copy], "p4_open_count": 2,
            "p4_closed_claims": [{"claim": alpha, "verified_by": f"{shutter.relative_to(self.root).as_posix()}#L1 — shutter"}],
        }}
        self.assertEqual(retire.fold_restatements(summary), 1)
        restated = summary["s"]["p4_restated_claims"]
        self.assertEqual(len(restated), 1)
        self.assertEqual(restated[0]["claim"], beta_copy)
        self.assertEqual(restated[0]["restates"], beta)

    def test_postdating_closer_refuses_the_fold_until_a_later_line_speaks(self) -> None:
        # Nabu P2 at 8966da2e: a re-statement filed after its root's closer
        # inherits nothing; a closing transcript that postdates the
        # re-statement and names it folds it.
        (self.root / "x").write_text("c\n")
        self.commit("third")
        third = self.git("rev-parse", "HEAD").strip()
        early = self.section / f"vulcan_surface_review-{self.first[:7]}.md"
        early.write_text("- **[P4] record `beta_guard` stale in scripts/a.py — CLOSED.**\n")
        self.git("add", ".")
        shut = f"vulcan_surface_review@{self.first[:7]}: [record] scripts/a.py:1 - `beta_guard` shut"
        copy = f"vulcan_surface_review@{self.second[:7]}: [record] (carried from {self.first[:7]}, OPEN) scripts/a.py:3 - `beta_guard` still stale"
        summary = {"s": {
            "p4_open": [copy], "p4_open_count": 1,
            "p4_closed_claims": [{"claim": shut, "verified_by": f"{early.relative_to(self.root).as_posix()}#L1 — early"}],
        }}
        self.assertEqual(retire.fold_restatements(summary), 0)
        self.assertEqual(summary["s"]["p4_open"], [copy])
        late = self.section / f"vulcan_surface_review-{third[:7]}.md"
        late.write_text("- **[P4] record `beta_guard` stale in scripts/a.py — CLOSED.**\n")
        self.git("add", ".")
        summary["s"]["p4_closed_claims"] = [
            {"claim": shut, "verified_by": f"{late.relative_to(self.root).as_posix()}#L1 — late"}
        ]
        self.assertEqual(retire.fold_restatements(summary), 1)
        self.assertEqual(summary["s"]["p4_restated_claims"], [{"claim": copy, "restates": shut}])

    def test_runbook_tracking_carry_folds_into_its_named_root(self) -> None:
        # The runbook-tracking fold test (Ariadne/Nabu P2 at 8966da2e): a
        # named carry of the runbook-tracking family folds into the claim
        # at the round it names.
        root = f"vulcan_surface_review@{self.first[:7]}: [runbook-tracking] scripts/second_host_attest.sh - `attest_scope` pinned"
        copy = f"vulcan_surface_review@{self.second[:7]}: [runbook-tracking] (carried from {self.first[:7]}, OPEN) scripts/second_host_attest.sh - `attest_scope` still pinned"
        summary = {"s": {"p4_open": [root, copy], "p4_open_count": 2}}
        self.assertEqual(retire.fold_restatements(summary), 1)
        self.assertEqual(summary["s"]["p4_restated_claims"], [{"claim": copy, "restates": root}])

    def test_one_finding_receives_one_status(self) -> None:
        # Ariadne P2 at 8966da2e: an original beside its own named copy,
        # judged under a head naming the finding, ends in one status —
        # never open in one copy and closed in another.
        closer = self.section / f"vulcan_surface_review-{self.second[:7]}.md"
        closer.write_text("- **[P4] record finding_key stale in scripts/a.py — CLOSED.**\n")
        self.git("add", ".")
        orig = f"vulcan_surface_review@{self.first[:7]}: [record] scripts/a.py:1 - `finding_key` stale"
        copy = f"vulcan_surface_review@{self.second[:7]}: [record] (carried from {self.first[:7]}, OPEN) scripts/a.py:3 - `finding_key` still stale"
        summary = {"s": {"p4_open": [orig, copy], "p4_open_count": 2}}
        retired = retire.retire(summary, [])
        folded = retire.fold_restatements(summary)
        self.assertGreaterEqual(retired + folded, 1)
        self.assertEqual(retire.split_status_problems(summary), [])
        survivors = set(summary["s"]["p4_open"])
        closed = {item["claim"] for item in summary["s"].get("p4_closed_claims", [])}
        restated = {item["claim"] for item in summary["s"].get("p4_restated_claims", [])}
        self.assertEqual(len(survivors | closed | restated), 2)
        self.assertEqual(survivors & closed, set())

    def test_split_status_refusal(self) -> None:
        # Ariadne P2 at 8966da2e: a key open in one copy and closed in
        # another is refused by `--check` until explicitly reconciled.
        claim = f"vulcan_surface_review@{self.first[:7]}: [record] scripts/a.py:1 - `note` stale"
        split = {"s": {
            "p4_open": [claim], "p4_open_count": 1,
            "p4_closed_claims": [{"claim": claim, "verified_by": "evidence/review/verdicts/x.md#L1 — y"}],
        }}
        problems = retire.split_status_problems(split)
        self.assertEqual(len(problems), 1)
        self.assertIn("open and closed", problems[0])
        clean = {"s": {"p4_open": [claim], "p4_open_count": 1}}
        self.assertEqual(retire.split_status_problems(clean), [])

    def test_split_status_refusal(self) -> None:
        # Ariadne P2 at 8966da2e: a key open in one copy and closed in
        # another is refused by `--check` until explicitly reconciled.
        claim = f"vulcan_surface_review@{self.first[:7]}: [record] scripts/a.py:1 - `note` stale"
        split = {"s": {
            "p4_open": [claim], "p4_open_count": 1,
            "p4_closed_claims": [{"claim": claim, "verified_by": "evidence/review/verdicts/x.md#L1 — y"}],
        }}
        problems = retire.split_status_problems(split)
        self.assertEqual(len(problems), 1)
        self.assertIn("open and closed", problems[0])
        clean = {"s": {"p4_open": [claim], "p4_open_count": 1}}
        self.assertEqual(retire.split_status_problems(clean), [])

    def test_equal_key_open_claim_inherits_a_later_closure(self) -> None:
        # Nabu P4 at 8966da2e: exact identity needs no carry phrase — an
        # open claim whose key matches a retired claim restates it when
        # the closure postdates the claim.
        shutter = self.section / f"vulcan_surface_review-{self.second[:7]}.md"
        shutter.write_text("- **[P4] record `alpha_guard` stale in scripts/a.py — CLOSED.**\n")
        self.git("add", ".")
        shut = f"vulcan_surface_review@{self.first[:7]}: [record] scripts/a.py:1 - `alpha_guard` shut"
        echo = f"vulcan_surface_review@{self.first[:7]}: [record] scripts/a.py:9 - `alpha_guard` again"
        summary = {"s": {
            "p4_open": [echo], "p4_open_count": 1,
            "p4_closed_claims": [{"claim": shut, "verified_by": f"{shutter.relative_to(self.root).as_posix()}#L1 — shutter"}],
        }}
        self.assertEqual(retire.fold_restatements(summary), 1)
        self.assertEqual(summary["s"]["p4_restated_claims"], [{"claim": echo, "restates": shut}])
        self.assertEqual(retire.split_status_problems(summary), [])

    def test_duplicated_authority_reads_agree_with_and_without_the_link(self) -> None:
        # Nabu P4 at 8966da2e: the shared-vocabulary exemption is derived
        # from the claims (same finding key plus a carry), never from the
        # ledger's popped `restates` link — both paths read alike.
        root = f"vulcan_surface_review@{self.first[:7]}: [record] scripts/a.py:1 - `note` stale"
        copy = f"vulcan_surface_review@{self.second[:7]}: [record] (carried from {self.first[:7]}, OPEN) scripts/a.py:3 - `note` still stale"
        linked = {"p4_open": [root, copy],
                  "p4_restated_claims": [{"claim": copy, "restates": root}]}
        unlinked = {"p4_open": [root, copy]}
        self.assertEqual(register.shares_its_kind(linked, "P4", root),
                         register.shares_its_kind(unlinked, "P4", root))
        self.assertEqual(register.shared_vocabulary(linked, "P4", root),
                         register.shared_vocabulary(unlinked, "P4", root))

    def test_closers_union_closes_both_severities(self) -> None:
        # Nabu P4 at b58ac1e0: a live field whose token carries
        # `_PRIOR_P3_CLOSED` and whose transcript records a `[P4] … —
        # CLOSED.` own-head line retires the P4; the union is asserted.
        (self.section / f"vulcan_surface_review-{self.second[:7]}.md").write_text(
            "- **[P4] record `alpha_guard` stale in scripts/a.py — CLOSED.**\n"
            "VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_PRIOR_P3_CLOSED\n")
        self.git("add", ".")
        claim = f"vulcan_surface_review@{self.first[:7]}: [record] scripts/a.py:1 - `alpha_guard` stale"
        summary = {"release_candidate_packaging": {
            "vulcan_surface_review": "PASS_0_P0_0_P1_0_P2_0_P3_PRIOR_P3_CLOSED",
            "vulcan_surface_review_note": f"on {self.second}: transcript evidence/review/verdicts/release_candidate_packaging/vulcan_surface_review-{self.second[:7]}.md",
            "p4_open": [claim], "p4_open_count": 1,
        }}
        closers = retire.closers(summary["release_candidate_packaging"], "release_candidate_packaging")
        union = {sev for _, lane, sevs, _, _ in closers if lane == "vulcan" for sev in sevs}
        self.assertEqual(union, {"P3", "P4"})
        self.assertEqual(retire.retire(summary, []), 1)
        self.assertEqual(summary["release_candidate_packaging"]["p4_open"], [])

    def test_closer_disagreement_is_reported(self) -> None:
        # Vulcan P4 at 8966da2e: a PRIOR field whose transcript records a
        # severity the token does not close is reported, never silent.
        (self.section / f"vulcan_surface_review-{self.second[:7]}.md").write_text(
            "- **[P4] record `alpha_guard` stale in scripts/a.py — CLOSED.**\n"
            "VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_PRIOR_P3_CLOSED\n")
        self.git("add", ".")
        summary = {"release_candidate_packaging": {
            "vulcan_surface_review": "PASS_0_P0_0_P1_0_P2_0_P3_PRIOR_P3_CLOSED",
            "vulcan_surface_review_note": f"on {self.second}: transcript evidence/review/verdicts/release_candidate_packaging/vulcan_surface_review-{self.second[:7]}.md",
        }}
        problems = retire.closer_disagreements(summary)
        self.assertEqual(len(problems), 1)
        self.assertIn("P4", problems[0])

    def test_replay_exact_claim_branch(self) -> None:
        # Nabu P4 at 79fdcc63: the replay checks the exact-claim branch —
        # a cited line outside the claim's closure fails, the bound line
        # passes.
        path = self.section / f"vulcan_surface_review-{self.second[:7]}.md"
        path.write_text("- **[P3] record-note stale in scripts/a.py — CLOSED.**\n"
                        "- **[P3] other wording — CLOSED.**\n")
        self.git("add", ".")
        rel = path.relative_to(self.root).as_posix()
        claim = f"vulcan_surface_review@{self.first[:7]}: [record-note] scripts/a.py:1 - stale note"
        bindings = self.root / "claim-retirements.json"
        bindings.write_text(json.dumps([{"section": "release_candidate_packaging", "verified_by": rel,
                                         "claims": [{"claim": claim, "lines": [1]}]}]))
        original = register.RETIREMENTS
        register.RETIREMENTS = bindings
        self.addCleanup(setattr, register, "RETIREMENTS", original)
        good = {"release_candidate_packaging": {"p3_open": [], "p3_open_count": 0,
                      "p3_closed_claims": [{"claim": claim, "verified_by": rel + "#L1 — x", "binding": "exact-claim"}]}}
        self.assertEqual(retire.replay_problems(good), [])
        bad = {"release_candidate_packaging": {"p3_open": [], "p3_open_count": 0,
                     "p3_closed_claims": [{"claim": claim, "verified_by": rel + "#L2 — x", "binding": "exact-claim"}]}}
        self.assertTrue(retire.replay_problems(bad))

    def test_regeneration_keys_on_the_full_reference(self) -> None:
        # Nabu P4 at 79fdcc63: the divergence gate compares the full
        # reference (path and `#L` lines) — a two-line transcript citing
        # different lines for one claim diverges, where path-only keying
        # had stayed silent.
        import json as _json

        claim = f"vulcan_surface_review@{self.first[:7]}: [record] scripts/a.py:1 - `note` stale"

        def replay_as(closed: list) -> object:
            def close(summary: dict, retirements: list) -> int:
                summary["s"]["p4_closed_claims"] = _json.loads(_json.dumps(closed))
                summary["s"]["p4_open"] = []
                return 1
            return close

        tracked = {"s": {"p4_open": [claim], "p4_open_count": 1,
                         "p4_closed_claims": [{"claim": claim, "verified_by": "evidence/review/verdicts/x.md#L1 — x"}]}}
        with unittest.mock.patch.object(retire, "retire", side_effect=replay_as(tracked["s"]["p4_closed_claims"])):
            with unittest.mock.patch.object(retire, "fold_restatements", return_value=0):
                self.assertEqual(retire.regeneration_divergence(tracked, []), [])
        relined = {"s": {"p4_open": [claim], "p4_open_count": 1,
                         "p4_closed_claims": [{"claim": claim, "verified_by": "evidence/review/verdicts/x.md#L1,L2 — x"}]}}
        with unittest.mock.patch.object(retire, "retire", side_effect=replay_as(relined["s"]["p4_closed_claims"])):
            with unittest.mock.patch.object(retire, "fold_restatements", return_value=0):
                self.assertEqual(retire.regeneration_divergence(relined, []), [])
        with unittest.mock.patch.object(retire, "retire", side_effect=replay_as(relined["s"]["p4_closed_claims"])):
            with unittest.mock.patch.object(retire, "fold_restatements", return_value=0):
                problems = retire.regeneration_divergence(tracked, [])
                self.assertEqual(len(problems), 1)
                self.assertIn("p4_closed_claims", problems[0])

    def test_undated_claims_never_fold(self) -> None:
        # Vulcan P4 at 8966da2e: refuse undated and unscoped folds — a
        # claim with no scope cannot be ordered, so it stays open.
        bare = "vulcan_surface_review: [record] scripts/a.py - `note` stale"
        summary = {"s": {"p4_open": [bare], "p4_open_count": 1}}
        self.assertEqual(retire.fold_restatements(summary), 0)
        self.assertEqual(summary["s"]["p4_open"], [bare])

    def test_own_head_lines_are_preferred_over_trailing_prose(self) -> None:
        # Ariadne P4 at 79fdcc63 (carried OPEN at 8966da2e): a line whose
        # own head names the finding is that finding's closure; a line
        # naming it only in trailing prose is used only when no head
        # names it.
        path = self.section / f"vulcan_surface_review-{self.second[:7]}.md"
        path.write_text("- **[P4] record stale — CLOSED.** trailing prose names `alpha_guard` only\n"
                        "- **[P4] [`alpha_guard`] stale in scripts/a.py — CLOSED.**\n")
        self.git("add", ".")
        headed = f"vulcan_surface_review@{self.first[:7]}: [record] scripts/a.py:1 - `alpha_guard` open here"
        section = {"p4_open": [headed]}
        self.assertEqual(retire.speaking_lines(path, "P4", headed, section, "s"), [2])

    def test_exact_claim_open_refusal(self) -> None:
        # Ariadne P4 at 79fdcc63 (carried OPEN at 8966da2e): an exact-claim
        # retirement whose transcript records the claim OPEN is refused.
        path = self.section / f"vulcan_surface_review-{self.second[:7]}.md"
        path.write_text("- **[P4] record `alpha_guard` stale — CLOSED.**\n"
                        "- **[P4] [`alpha_guard`] stale — OPEN.**\n")
        self.git("add", ".")
        rel = path.relative_to(self.root).as_posix()
        claim = f"vulcan_surface_review@{self.first[:7]}: [record] scripts/a.py:1 - `alpha_guard` stale"
        item = {"section": "release_candidate_packaging", "severities": [4],
                "claims": [{"claim": claim, "lines": [1]}],
                "verified_by": rel, "reason": "x"}
        with self.assertRaises(SystemExit):
            retire.retire({"release_candidate_packaging": {"p4_open": [claim], "p4_open_count": 1}}, [item])


if __name__ == "__main__":
    unittest.main()
