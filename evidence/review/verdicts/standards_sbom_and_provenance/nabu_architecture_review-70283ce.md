# Nabu architecture review — standards_sbom_and_provenance, revision 5 clean-pass round (SCOPE_SHA 70283ce)

## Scope verification

`git rev-parse HEAD` = `70283ce1932d4182f206f0d09e6129af3976a4c2`, matching the
dispatched SCOPE_SHA. `git status --short` was empty at dispatch and remains empty;
no tracked file was modified, formatted, staged, or committed. The only file written
by this review is this transcript.

## Note on the stated prior baseline

The task described the prior lane result as "a zero-finding PASS at revision 3"
filed at `evidence/review/verdicts/standards_sbom_and_provenance/nabu_architecture_review-a4b6029.md`.
I read that file in full and it does not match that description: it is dated
2026-09-13, its own title says "records-closure amendment (rev 5, SCOPE_SHA a4b6029)",
and its footer is `PASS-3` (three P3/P4 findings, not zero). I also read the three
earlier transcripts in the same directory (`e464ed4` 09-11 09:10 REVISE, `abf4ff0`
09-11 11:56 REVISE, `3320ca9` 09-11 18:28 PASS with five P4 follow-ups) to locate a
zero-finding revision-3 pass; none exists under this field. "Revision 3" is the
spec's own internal counter (`docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md:6-16`:
revision 3 closed five P0s; revision 5 added the records-closure model) and is not
the git-round label attached to any nabu_architecture_review transcript. I am
treating `a4b6029` (PASS-3, three carried observations, already reviewing this same
records-closure model) as the actual prior baseline, since that is what the
evidence supports, per the brief's instruction to re-derive claims from primary
sources rather than trust a summary. This discrepancy is reported, not corrected —
correcting labeling conventions is not this lane's authority.

## Inputs read in full

- `evidence/review/verdicts/standards_sbom_and_provenance/nabu_architecture_review-a4b6029.md`
  (prior baseline, see above) and the three earlier transcripts in the same directory
  (skimmed for the zero-finding-at-rev-3 search above).
- `scripts/records_closure.py` (full, 141 lines) at HEAD.
- `scripts/build_standards_sbom.py`, `scripts/build_release_provenance.py`,
  `scripts/check_standards_sbom_and_provenance.py` (the closure-relevant sections;
  full diff against `a4b6029` read for all four plus the spec and the test file).
- `docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md` sections 1, 5, 7, 9, current text
  and the `a4b6029..70283ce` diff.
- `bench/release/tests/test_standards_sbom.py` `RecordsClosureTests` (full diff).
- `docs/audits/S20_710_PRE_RELEASE_AUDIT.md` (status header, to check for a stale
  claim adjacent to the license change bundled in this window) and
  `docs/audits/S20_710_STANDARDS_SBOM_CLOSEOUT.md` (header, for revision history).
- `git show f7df74f --stat` / commit message (the commit named in the task).
- `git log -1 --format=... <sha>` for the four prior-transcript SHAs, to place them
  in chronological order.

## Tool results

`SLEY2_MASTER_GOAL=/home/dev/machineresearch/Sley2.0mastergoal.md python3 scripts/check_standards_sbom_and_provenance.py`:

```
{
  "codes": [...8 codes...],
  "contract": "s20-710-full-standards-sbom-and-provenance-v1",
  "implementation_complete": false,
  "problems": [
    "closure:ineligible"
  ],
  "records_closure": {
    "advanced": true,
    "attested_source_commit": "7a94a4a31272a6dc7588aff902dcde81f7d10a4e",
    "reason": "records-closure-ineligible: bench/release/tests/test_standards_sbom.py, crates/sley-query/src/root_query.rs, docs/WORK_PACKAGES.md, docs/audits/S20_LOCAL_COMPLETION_FRONTIER.md, docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md, docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md, scripts/build_decision_dossier.py, scripts/build_ga_acceptance_report.py, scripts/check_standards_sbom_and_provenance.py",
    "records_closure_head": "70283ce1932d4182f206f0d09e6129af3976a4c2"
  },
  "result": "FAIL",
  "s20_710_audit_complete": false,
  "status": "S20_710_FULL_SBOM_AND_PROVENANCE_IMPLEMENTED_REVIEW_PENDING"
}
```
Exit code 1.

`SLEY2_MASTER_GOAL=... python3 -m unittest bench.release.tests.test_standards_sbom -v`:
52 tests, all `ok`, `Ran 52 tests in 0.083s / OK`. Includes the new/renamed
`RecordsClosureTests` (`test_builders_admit_exactly_the_live_closure_verdict`,
`test_builders_admit_an_eligible_closure_keeping_the_candidate_binding`,
`test_attestation_bound_change_is_ineligible`, `test_bound_input_change_refuses`,
`test_builders_refuse_a_bound_artifact_change`, `test_builders_refuse_an_ineligible_closure`,
`test_unverifiable_git_refuses_closed`) and the license-relabeled tests
(`test_spdx_is_2_3_with_a_fixed_instant_and_an_extracted_apache_reference`,
`test_plain_and_approved_declarations_pass_through`).

## Independently re-derived claims

**1. "One cause on an ineligible closure" (f7df74f).** Confirmed by direct diff
(`git diff a4b6029 70283ce -- scripts/check_standards_sbom_and_provenance.py`) and
by the live run above: before this window, an ineligible closure produced three
problems (`sbom:drift`, `provenance:drift`, `closure:ineligible`) because both
builders' `--check` refuses non-zero once the closure predicate refuses derivation,
which the checker separately counted as drift. The new code collects the drift
labels into `drift_problems`, computes `closure_ineligible` first, and only extends
`problems` with the drift labels `if not closure_ineligible`. Live output above shows
exactly one problem, `closure:ineligible`, confirming the fold. `git show f7df74f`'s
own commit message states "checker reports one cause on an ineligible closure",
matching the diff.

**2. Section 5 bound-inputs sentence rewrite.** Prior text (`a4b6029`): "none of the
bound inputs changed (the T52 inventory the SPDX namespace binds; the emitted
documents are validated instead by byte-identical re-derivation, and the candidate
still needs its clean `REPRODUCIBLE` attestation 4-tuple)." Current text: "none of
the bound inputs changed. The bound input is the T52 inventory the SPDX namespace
binds. The emitted documents (the SBOM pair and the provenance statement) and the
reproducibility report are not bound inputs: they are validated by the second layer,
byte-identical re-derivation over the closure HEAD plus the attestation 4-tuple gate
... A reader needs both layers: the first says what may not move, the second says
how the documents that may be re-derived are checked." This is a prose clarification,
not a code or predicate change — `records_closure.py`'s `BOUND_PATHS` tuple and
`ELIGIBLE_PREFIXES` tuple are byte-identical between `a4b6029` and `70283ce`
(confirmed by the diff producing no hunks against `records_closure.py`). The rewrite
directly answers my own prior-round P3 observation (`a4b6029` transcript, finding 1)
that the reproducibility report's non-bound status relied on the second-layer 4-tuple
gate rather than positional binding — the spec text now says this explicitly instead
of leaving it implicit. Architecturally this closes a documentation gap, not a code
gap; the underlying two-layer design was already sound and is unchanged.

**3. Records-closure ownership: is authority for the closure invariant held in one
place?** Grepped the full tree for `ELIGIBLE_PREFIXES` and `BOUND_PATHS`: both
symbols are defined exactly once, in `scripts/records_closure.py:35,46`, and used
only inside that file's own `closure_status()` function (lines 130, 132). Both
builders (`build_standards_sbom.py:478`, `build_release_provenance.py:214`) and the
checker (`check_standards_sbom_and_provenance.py:376`) import `records_closure` and
call `closure_status()` — none of the three callers has its own copy of the
eligibility or boundedness rule, and none re-implements the diff-classification
logic. This is single ownership of the invariant (one module decides "is this HEAD a
records-closure of that candidate"), consumed identically by three callers. It is
architecturally sound: three call sites cannot disagree because there is only one
implementation to disagree with.

**4. Is the triple invocation itself a defect?** The checker still shells out to git
independently and calls `records_closure.closure_status()` a third time
(`check_standards_sbom_and_provenance.py:376`) rather than reusing a verdict the
builders already computed when the checker invoked them as subprocesses two lines
earlier (`:352-356`). This is unchanged since `a4b6029` (I flagged it there as a P3
"note only; no change requested" because all three invocations read the same live
git state within one process run and would only disagree if HEAD moved mid-run, in
which case every caller fails closed anyway). It remains a minor, non-actionable
coupling note: three call sites, one authority, redundant computation but not
redundant *semantics*. I am not re-raising it as an actionable finding this round
since nothing changed about it and my prior characterization ("note only") stands.

**5. Enforcement placement moved from checker to builder.** `build_release_provenance.py`
now raises `ProvenanceError(EVIDENCE_INVALID, ...)` directly inside `build_statement()`
when `working_tree_clean` is false (new code, `:244-256` per diff), rather than
relying on the checker to catch a dirty derivation downstream. The commit's own
comment states this explicitly ("Enforcement lives at derivation, not downstream in
the checker ... the builder emitted what only the checker refused"). This is an
ownership improvement in the same direction as the records-closure model: the
invariant ("no statement from a dirty tree") is now enforced at the one place that
derives the statement, not duplicated as a second check in the checker. I checked the
checker for a competing dirty-tree decision (`grep -n "working_tree_clean\|clean"
check_standards_sbom_and_provenance.py`) — its only uses of that field are attestation
*filtering* for subject-binding eligibility (a different invariant: "which historical
attestations may bind the subject", not "may this build derive a statement"), so
there is no duplicated authority here either.

**6. Bundled, adjacent, non-closure change: license relabeling.** In the same
`a4b6029..70283ce` window, `PROPRIETARY`/`PROPRIETARY_TEXT` were renamed to
`ROOT_LICENSE = "Apache-2.0"`/`ROOT_LICENSE_TEXT`, the CycloneDX/SPDX emitters now
emit `Apache-2.0`, and provenance's `BLOCKERS` dropped `root_license_text_operator_approval`
and `release_candidate_history_reanchor`. This is outside the records-closure model
proper but is part of the same revision-5 window and worth a staging-honesty check:
`git log --all --oneline | grep -i license` shows a recorded operator decision chain
(`7804f66` "licensed-candidate L1: Apache-2.0 root license...", `bc2a3d8` "T52 PASS
with approved Apache-2.0 dispositions", `82ad532` "Argus + Vulcan finals PASS on
licensed-candidate L2 ... (0 findings)") and `LICENSE` exists at HEAD
(`git cat-file -e 70283ce:LICENSE` succeeds, contents are the real Apache License
text). The change is evidenced, not asserted. One adjacent document,
`docs/audits/S20_710_PRE_RELEASE_AUDIT.md:3`, still reads "Status: BLOCKED -
operator-approved root license text required" — but that document is explicitly
self-described as frozen at an older anchor (`db1bc62`) with re-anchoring at the
release candidate named as a tracked, disclosed precondition, not a hidden gap
(`:7-12`), and the governing contract itself (`STANDARDS_SBOM_AND_PROVENANCE_V1.md`
Boundary section) already discloses that these documents do not complete the S20-710
audit and stay draft/unapproved regardless. I do not treat the frozen audit's stale
header as a finding in this lane: it is a different document with its own disclosed
freeze semantics, not a claim this contract or this round makes.

## Per-item analysis (per the dispatched task)

- **Records-closure model as an ownership arrangement.** One module
  (`records_closure.py`) owns the eligibility/boundedness predicate; both builders
  and the checker consume it by call, not by reimplementation. No duplicated
  authority. The one redundancy (checker recomputing a verdict the builders already
  computed) is a coupling/performance note carried since last round, not a semantic
  split, and remains non-actionable.
- **Checker's one-cause fold.** Verified live and by diff: an ineligible closure now
  surfaces exactly one problem, `closure:ineligible`, instead of three overlapping
  ones. This is a genuine improvement to staging honesty (one root cause, one label)
  and directly closes what a prior round (attributed to Vulcan, 2026-09-13, in the
  code comment) flagged as "two labels for one cause."
- **Section 5 rewrite.** A prose-only clarification that answers my own prior P3
  observation about the reproducibility report's non-bound status; no predicate or
  builder behavior changed. Consistent with the code before and after.
- **Expected `closure:ineligible` at this HEAD.** Confirmed this is the correct,
  disclosed, fail-closed posture: the live run's `reason` field names nine ineligible
  paths, all attestation-bound (crates, scripts, specs, WORK_PACKAGES, an audit doc),
  none records-only, and both builders and the checker refuse rather than silently
  admitting or silently relabeling the state as something else. `implementation_complete`
  stays `false`, `result` stays `FAIL`. This is staging honesty holding under the
  exact transitional condition the task describes as expected pre-re-mint.

## Findings

None that require owner action. The one carried observation from `a4b6029` about
triplicated (but not divergent) `closure_status()` invocation remains true and
remains non-actionable for the same reason given then; I am not listing it as a
finding since nothing regressed and no owner action is implied beyond the optional
refactor already noted last round.

## Summary

The records-closure model's ownership arrangement is sound: `records_closure.py` is
the single owner of the eligibility and boundedness predicate, both builders and the
checker consume that one predicate without any parallel reimplementation, and the two
changes since the last review of this model (the checker's one-cause fold for an
ineligible closure, and the section 5 bound-inputs sentence rewrite) are both
narrowing/clarifying rather than loosening: the fold removes a duplicate-label
symptom without changing the underlying refusal, and the rewrite states in the spec
text what the code already did. Live execution confirms the checker reports exactly
`closure:ineligible` (the nine paths named are all genuinely attestation-bound: three
scripts, two specs, a crate source file, a WORK_PACKAGES doc, an audit doc, and the
test file itself) and all 52 `bench/release/tests/test_standards_sbom` tests pass.
The bundled, adjacent Apache-2.0 relabeling is evidenced by a recorded operator
decision chain and an installed LICENSE file, and does not conflict with this
contract's own disclosed boundary (draft, unapproved, S20-710 audit not complete).
No architectural defect found in the closure model or its ownership boundaries at
this HEAD; the pre-re-mint `closure:ineligible` state is the correct, honestly
reported, fail-closed posture the task described as expected.

```
VERDICT: PASS
SECTION: standards_sbom_and_provenance
FIELD: nabu_architecture_review
SCOPE_SHA: 70283ce1932d4182f206f0d09e6129af3976a4c2
FINDINGS:
SUMMARY: The records-closure model's ownership is single-authority (records_closure.py owns the eligibility/boundedness predicate; both builders and the checker consume it without reimplementation). Since the last review of this model, the checker's drift-label fold and the section 5 bound-inputs rewrite are both narrowing/clarifying, not loosening. Live run confirms the checker reports exactly one problem, `closure:ineligible`, with nine genuinely attestation-bound paths named, and all 52 bench/release/tests/test_standards_sbom tests pass. This is the correct, honestly-reported, fail-closed pre-re-mint posture; no architectural defect found.
```
