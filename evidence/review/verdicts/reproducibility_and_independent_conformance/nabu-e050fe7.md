# Nabu Council review — reproducibility_and_independent_conformance

Harness: claude-code
Observed model: claude-fable-5-1
Reviewed checkpoint: e050fe75a86c1b2f0bf779561ac2bc70bce34289

Delta since a809906: b9ae9a9 (contract revision 7, exported helpers, checker bindings, embeds, env scrub, tests), then the integration commits that consume the helpers (a10d871), the Makefile build/verify split (9b4f064 `Makefile:225-253`), the repro-field sync (`scripts/sync_evidence_counters.py:33-51`), and the 9b4f064 two-host re-mint of `evidence/release/reproducibility-report.json`.

Prior a809906 findings, all three lanes, traced against e050fe7:
- Nabu P3 `BOUND_PATHS` restated: closed. `scripts/records_closure.py:47-51` derives it from `ARTIFACT_INPUT_PATHS` filtered by `ELIGIBLE_PREFIXES`; equality test `bench/release/tests/test_standards_sbom.py:712-728`.
- Nabu/Ariadne/Vulcan P3 summary fields contradicting the report: closed. `machine-summary.json:4031-4049` restate MULTI_HOST/ATTESTED/2/2/three blockers; checker binds them at `check_reproducibility_and_independent_conformance.py:347-357`; sync writes them at `sync_evidence_counters.py:40-51`; regression `test_summary_mirrors_follow_single_and_two_host_reports`.
- Nabu P3 lineage: closed; revision_2 = 0bcc9c6 `REVISE_0_P0_1_P1_0_P2_2_P3_3_P4`, revision_3 = 9ae09a1 `REVISE_0_P0_1_P1_0_P2_1_P3_3_P4`, revision_4 = 246d5c4 (`:4066-4071`), matching the transcripts.
- Nabu P4 exported owner: closed. `admissible_attestation` (:182), `admissible_attestations` (:200), `binds_candidate` (:221), `select_attestation` (:234, majority-of-hosts, tie fails to None, 4-tuple binding with a candidate record). Imported by provenance (`build_release_provenance.py:171-178,224-226`), SBOM (`build_standards_sbom.py:486`), standards checker (:306-309,:385-388), packaging checker (:205), content report (:33), GA (:418), dossier (:157).
- Nabu P4 static blockers: closed by contract wording (spec :160-166) and the constant's comment (:30-37).
- Nabu P4 section 1 cross-reference/indent: closed (spec :78-81).
- Ariadne P4 WORK_PACKAGES row, section 9, uncoded `runner_label`, duplicate `ariadne_review` leaf: closed (row 66 at revision 7; spec :390-395; `test_a_declared_oracle_script_missing_from_disk_is_coded`; leaf removed per `:4053`).
- Vulcan P2 embedded inputs: closed. `EMBEDDED_INPUT_PATHS` in the surface (`build_release_candidate.py:50-72`), scan test `test_reproducibility.py:543-586`.
- Vulcan P3 lane record: table added (`S20_730_REPRODUCIBILITY_CLOSEOUT.md:52-67`) — but see finding 1.
- Vulcan P3 bare PASS tokens: base fields still carry the held forms (`:4052,:4061,:4072`); the notes say the dispatcher files this round's verdicts. That is an acceptance-time record step, not a code defect; not counted.
- Vulcan P4 env scrub, LICENSE/NOTICE enumeration, a810943 note: closed (`scrub_build_env`, spec :138-148, `:4088`).

Candidate lifecycle traced: `local_attestation` copies the S20-720 record (:77-128); `build_report` refuses duplicate labels and digest conflicts (:266-314); `carried_attestations` re-validates and carries every label not re-attested (:351-385). During a primary re-mint with a carried older secondary the selector without a candidate returns None (tie), the content/SBOM builders still bind via the candidate 4-tuple, and the checker's `history_problems` fails stale on the older commit until the lab attestation is merged — the documented fail-closed window (RECOVERY :49-53). At e050fe7 both attestations name 9b4f064 with identical artifact digest `18ee1ecf…`, size 2190457, manifest `15a76816…`, 15 members, matching toolchains; `report_digest 3417b3ac…` is recorded. Dirty/unreproduced/toolchain-less/tied/absent cases have unit coverage (:193-253) and cross-consumer coverage (`test_ga_acceptance_report.py:303-331`, `test_candidate_content.py:68-78`).

Non-actionable observations: `release-candidate-build` (`Makefile:232`) runs the report builder without `--host-label`, so a lab run of the same target labels the lab build `primary` in the lab's local tracked report; the runbook's separate `--emit-attestation --host-label secondary` step is what enters the merge, so the tracked evidence is unaffected. Spec section 6 still says the report is rebuilt by `release-candidate-smoke`; still true through the target dependency.

Not executed: checker, `--check`, tests (sandbox).

VERDICT: REVISE_0_P0_0_P1_0_P2_1_P3_2_P4
SECTION: reproducibility_and_independent_conformance
FIELD: current_delta_review.nabu
SCOPE_SHA: e050fe75a86c1b2f0bf779561ac2bc70bce34289
FINDINGS:
[P3] [record] docs/audits/S20_730_REPRODUCIBILITY_CLOSEOUT.md:61-63 - contract section 5.1 (revision 7, spec :335-342) requires one lane-record row per exercise of the second-host runbook, and the closeout itself says "the next mint fills its row completely" (:65-67); the tracked report's MULTI_HOST_REPRODUCIBLE claim now rests on the 9b4f064 lab mint (secondary attestation at 9b4f0648…, report_digest 3417b3ac…), yet the table carries only the 7a94a4a row and `grep 9b4f064` over the closeout, RESUME.md, docs/status, WORK_PACKAGES, REQ-02, and the acceptance map finds no record; the summary pointer `second_host_lane_record` (machine-summary.json:4035) still says "7a94a4a row filed". The recovery validation JSON records hosts 2 but none of the row fields (transport lane, lab checkout, lab evidence.json sha256, attestation-file sha256, bundle commit, merge commit), so the current multi-host claim is not auditable the way the contract requires. Repair: file the 9b4f064 row with the fields the table defines and update the summary pointer.
[P4] [record] machineresearch/sley-2.0/machine-summary.json:4112 - `candidate_consumers_note` states build_decision_dossier.py:135 and build_ga_acceptance_report.py:159 "still select attestations[0] and are queued"; at e050fe7 both import `select_attestation` (build_decision_dossier.py:157, build_ga_acceptance_report.py:418), so the note contradicts the code and the RECOVERY record. Restate.
[P4] [record] docs/WORK_PACKAGES.md:66 - the S20-730 row still says `MULTI_HOST_REPRODUCIBLE` at `7a94a4a` and "mechanics complete pending the queued dual-host re-mint"; the re-mint landed at 9b4f064 (both attestations, report and provenance re-derived). Align the row with the current candidate.
SUMMARY: Revision 7 closes every actionable a809906 item across the three lanes and the integration commits consume the exported selector everywhere the contract names; the candidate lifecycle (derive, carry, merge, select, bind, stale-check, sync) is single-owner and fail-closed, and the current two-host report at 9b4f064 is internally consistent. What remains is record accuracy: the contract-mandated lane-record row for the very mint the current claim rests on is missing (P3), and two pointers still describe the pre-re-mint world (P4). No mechanics defect found.
