# Independent review-record audit — 2026-09-15

Baseline inspected: `4846bb6a2333efa6360bec3ed2296368943a338b`.
Reviewer: separate Codex subagent `remaining_review_audit`; read-only source review, not an author of the inspected implementation. This is a scoped record/classifier and R2 evidence-binding review, not a full semantic, security, GA, or release acceptance.

## Verdict

**REVISE — do not clear the register by relabeling all remaining records.** Two of the three OTHER rows are incorrectly named metadata. The third preserves genuine incomplete review work. The 41 unclaimed PASS rows mix historical closed findings, duplicate old dispositions, and real residual work. Zero PENDING rows is not evidence of zero unresolved findings.

No execution or test success is claimed here. Reads included the register builder, finding-register contract, GA/dossier consumers, original RW075 native/premium records, current R2 checker, S20-350/540 closeouts, legacy reclassification record, current summary, and relevant later review notes. The targeted admission-boundary source reads below support only the stated narrow observations.

## Three unclassified dispositions

### 1. mutation_value_profile.merlin_review — metadata, not a verdict

Current value: `TIMED_OUT_PARTIAL_HANDOFF_INTEGRATED_BY_CODEX`.

Evidence:

- Its introducing commit is `cbf878d0dade22307ea6cfca41023666cecce8c9` (`docs: close S20-350 proposal construction`; equivalent earlier-history commit `8f416fed` also exists).
- `machineresearch/sley-2.0/s20-740-finding-register-campaign-2026-09-03.md` explicitly describes this as a timed-out handoff that Codex integrated.
- `docs/WORK_PACKAGES.md` assigns Merlin implementation ownership; the S20-350 closeout describes the independent-review outage separately from local implementation and semantic review.
- No original Merlin transcript for this specific timeout was located in the tracked tree. Do not invent a PASS or claim that original partial work was independently accepted.

Correction: preserve the exact text as `merlin_handoff_note` (with introducing commit/campaign pointer), and remove the misnamed verdict field. This is an independent **PASS for this metadata reclassification only**. Current S20-350 acceptance must continue to rely on genuine scoped reviews, conformance and tests; this note supplies none of them.

### 2. s20_600_frozen_legacy_adapter.review_lane — metadata

Current value: `legacy_artifact_adapter`.

`evidence/review/reclassification/s20-600-item10.md` calls this the governing lane, in contrast to the incorrectly assigned persistent-fuzz lane. The section separately carries `legacy_adapter_contract_review` and `vulcan_review` with actual dispositions. The string is a lane identifier, not acceptance.

Correction: rename to `review_lane_note` or `lane`, preserving its value and reclassification pointer. Prefer the existing contract's suffix exception over weakening the global obligation matcher. Independent **PASS for this metadata reclassification only**. This does not close the 5 P3 follow-ups in either actual review field.

### 3. rw075_correction.native_review_r12_2026_09_07 — historical self-review plus a real open obligation

Original: `machineresearch/sley-2.0/reviews/reweave-rw075-native-r12-2026-09-07.log`.

It explicitly states same author line, SELF-REVIEW, provisional, not independent acceptance, not Nabu, not premium round 2, and not gate input. It records AR-06 PARTIALLY_CLOSED: no revision binding of review evidence to implementation; RW060 still summary plus hardcoded IDs. Its later closure caveats say formal qualifying Nabu round12 and premium round2 are still required. `rw-075-continuation-2026-09-07.md` confirms the pending source `f5fd457...` and repair candidate `df89a062...`.

The current `scripts/check_r2_exit.py` still has these gaps:

- `rw060_ok` accepts a summary status, hardcoded IDs and existence of the markdown record; it does not authenticate lifecycle execution evidence against the reviewed implementation.
- Latest-lane reviews are selected by filename round/date and verdict text without candidate revision or bound source digest. Latest existing Nabu r11 cannot independently qualify later r12 implementation.
- Nabu/Ariadne exact-token claims are not implemented like premium: their PASS/FAIL regexes lack premium's identifier-boundary guard. This is a defensive parsing correction, not a reason to relabel existing evidence.
- Premium round1 still supplies R2_ARCHITECTURE_FAIL. The existing retained negative record is correct and must remain.

Correction: preserve self-review verbatim in a `*_note` field, introduce an explicit PENDING independent RW075 completion obligation, repair evidence binding, then obtain qualifying independent reviews against the actual candidate. File exact source/record hashes and command evidence, preserving the original failures. Do not use an unscoped replacement PASS or present this subagent as the missing Nabu/premium lane.

Independent **REVISE for current R2 evidence binding**. No new severity is assigned beyond the original AR-06 architecture-review obligation; it must be tracked explicitly rather than hidden by renaming. The separate prefix parsing discrepancy also needs correction and positive/negative unit coverage before R2 readiness.

### Narrow current source observations (not full RW075 acceptance)

I read `crates/sley-vm/src/admission_authority.rs` `admit_v2_package`, `verify_structural_correspondence`, `tables_match`, `type_tables_match`, the sealed `SleyAdmissionEvidence`, and the reserved admission route, plus the carried-Bytes check in `bootstrap.rs`.

The current code places complete table/image correspondence before receipt minting, compares entry/epoch/root/constants/type definitions/imports/globals/contracts, and reserves the post-C1 route as always refusing `SleyEvidenceUnavailable`. V2 carried Bytes are bounded by RAW_HASH_MAX_BYTES. These observations support the original review's AR-02/03/07 design descriptions. They do not replace execution evidence, review the full semantic/lowering pipeline, close AR-06, or establish C1 exclusion. No full RW075 PASS is issued.

## Why 41 PASS rows remain blocking

`build_finding_register.py` correctly distinguishes verdict state from clearance:

- PASS with nonzero severity mentions stays PASS.
- A severity must be explicitly closed in that disposition, genuinely declared absent, or tracked by positive package open claims.
- A positive package claim still prevents CLEAR; simply adding it is accounting, not closure.
- General FAIL/REVISE supersession runs only for same-reviewer compatible subjects and round ordering. It does not supersede PASS-with-findings.
- A PASS in nested `current_delta_review` is in a different dotted section and does not automatically discharge older top-level scope findings.
- `_note` fields are excluded intentionally; moving real unresolved findings into notes without an active obligation would hide them.
- GA `lane_clear` requires all lane rows PASS/HISTORICAL plus no flagged unclaimed/unclassified row. GA's complete independent PASS is narrower than ordinary classifier PASS.

Do not change the classifier to silently erase all historical PASS severities. Prefer an explicit per-row closure record linking original transcript, finding IDs, corrective commit and independent disposition. Preserve original verdicts in the record history. If a normalized current disposition is written, keep the original verbatim in its note and cite the actual closure evidence; never convert unresolved work into `*_CLOSED` by interpretation alone.

### Evidence-backed classifications already established

1. `repository_exchange.ariadne_implementation_review`: the phrase `PASS_IMPLEMENTATION_AFTER_P1_REMEDIATION` is historical repair wording. `docs/audits/S20_540_REPOSITORY_EXCHANGE_CLOSEOUT.md` lines 146–150 records the independent re-review: every first-review item closed, residual P3 notes applied. Normalize the current record to PASS with a preserved original and exact closeout/session pointer. This is a record correction, not a new full implementation review.
2. `mutation_value_profile.epoch1_reanchor_review`: ADR-0019 records the initial P2 stale-vector defect then PASS after all vectors/identities re-anchored. The wording mentions historical P2, not an asserted current P2. Normalize the closure wording with ADR/session evidence, preserving the original. I have not independently rerun the whole re-anchor.
3. Reproducibility base Ariadne/Nabu dispositions duplicate rounds already represented by `ariadne_contract_review_revision_3 = PASS_P2_P3_P4_CLOSED` and `nabu_architecture_review_revision_4 = PASS_P1_P3_P4_CLOSED`, whose notes itemize closure commits. Final 97b9117 review is retained. Reconcile the duplicate current field against those exact records; do not demand a new build solely for old base prose.
4. Packaging revision2/3 and standards older PASS-with-findings have final later review evidence, but each old follow-up still needs explicit linkage to the final review's scope. Existing unclaimed behavior is mechanically correct; it is not grounds to re-run accepted builds.
5. Legacy adapter, semantic-checker fuzz and VM-fuzz notes claim particular post-review repairs. These are concrete review inputs, not by themselves independent confirmation of every residual. Obtain narrow re-review or verify original residual IDs directly.
6. Pack-fuzz note explicitly leaves VR-C7-02/03 for the next target round. Its PASS is not zero findings.
7. VM extended Nabu's original transcript really enumerates 7 P1, 6 P2 and 5 P3. No later Nabu transcript was located in `evidence/review/verdicts/vm_extended_opcode_profile`. Ariadne/Vulcan closure records and broad pN_closed lists cannot impersonate Nabu closure; item-level matching plus independent Nabu/current reviewer closure is needed.
8. CLI `current_delta_review_note` itself records P4 notes despite bare current PASSs; complete-entity notes likewise carry P4 follow-ups and a Vulcan P3 that its zero-count field omits. These should be inventoried, not assumed absent because the selected count string is zero.

### Exact unclaimed inventory at reviewed baseline

| Section | Field | Current disposition |
|---|---|---|
| `cli` | `vulcan_surface_review` | `PASS_0_P0_0_P1_0_P2_2_P3` |
| `complete_entity_impact_profile` | `ariadne_contract_review` | `PASS_WITH_P4_NOTES_NO_P0_P1_P2_P3` |
| `complete_entity_impact_profile` | `ariadne_review` | `PASS_WITH_P4_NOTES_NO_P0_P1_P2_P3` |
| `complete_entity_impact_profile` | `nabu_architecture_review` | `PASS_WITH_P4_NOTES_NO_P0_P1_P2_P3` |
| `mutation_value_profile` | `epoch1_reanchor_review` | `PASS_AFTER_P2_VECTOR_REANCHOR` |
| `mutation_value_profile` | `vulcan_review` | `PASS_WITH_P3_P4_FOLLOWUPS_NO_P0_P1_P2` |
| `release_candidate_packaging` | `ariadne_contract_review_revision_3` | `PASS_0_P0_0_P1_0_P2_1_P3` |
| `release_candidate_packaging` | `nabu_architecture_review_revision_2` | `PASS_0_P0_0_P1_0_P2_3_P3` |
| `release_candidate_packaging` | `nabu_architecture_review_revision_3` | `PASS_0_P0_0_P1_0_P2_3_P3` |
| `release_candidate_packaging` | `vulcan_surface_review_revision_2` | `PASS_0_P0_0_P1_0_P2_2_P3` |
| `release_candidate_packaging` | `vulcan_surface_review_revision_3` | `PASS_0_P0_0_P1_0_P2_2_P3` |
| `repository_exchange` | `ariadne_implementation_review` | `PASS_IMPLEMENTATION_AFTER_P1_REMEDIATION` |
| `reproducibility_and_independent_conformance` | `ariadne_contract_review` | `PASS_WITH_P2_P3_FOLLOWUPS_NO_P0_P1` |
| `reproducibility_and_independent_conformance` | `nabu_architecture_review` | `PASS_WITH_P1_P3_P4_FOLLOWUPS_NO_P0_P2` |
| `root_backed_query_profile` | `ariadne_contract_review` | `PASS_WITH_P3_P4_FOLLOWUPS_NO_P0_P1_P2` |
| `root_backed_query_profile` | `ariadne_entity_read_review` | `PASS_WITH_P3_P4_FOLLOWUPS_NO_P0_P1_P2` |
| `root_backed_query_profile` | `nabu_architecture_review` | `PASS_WITH_P3_P4_FOLLOWUPS_NO_P0_P1_P2` |
| `root_backed_query_profile` | `nabu_entity_read_review` | `PASS_WITH_P3_P4_FOLLOWUPS_NO_P0_P1_P2` |
| `root_backed_query_profile` | `vulcan_entity_read_review` | `PASS_WITH_P3_P4_FOLLOWUPS_NO_P0_P1_P2` |
| `root_backed_query_profile` | `vulcan_surface_review` | `PASS_0_P0_0_P1_0_P2_2_P3_1_P4` |
| `s20_600_frozen_legacy_adapter` | `legacy_adapter_contract_review` | `PASS_0_P0_0_P1_0_P2_5_P3` |
| `s20_600_frozen_legacy_adapter` | `vulcan_review` | `PASS_0_P0_0_P1_0_P2_5_P3` |
| `s20_700_adapter_responses_persistent_fuzz_slice` | `vulcan_review` | `PASS_0_P0_0_P1_0_P2_4_P3` |
| `s20_700_complete_root_snapshot_persistent_fuzz_slice` | `vulcan_review` | `PASS_0_P0_0_P1_0_P2_3_P3` |
| `s20_700_merge_persistent_fuzz_slice` | `vulcan_review` | `PASS_0_P0_0_P1_0_P2_3_P3` |
| `s20_700_pack_persistent_fuzz_slice` | `vulcan_review` | `PASS_0_P0_0_P1_0_P2_3_P3_4_P4` |
| `s20_700_query_persistent_fuzz_slice` | `vulcan_final_review` | `PASS_0_P0_0_P1_0_P2_2_P3` |
| `s20_700_query_persistent_fuzz_slice` | `vulcan_review_revision_2` | `PASS_0_P0_0_P1_0_P2_5_P3` |
| `s20_700_remaining_surface_audit` | `vulcan_review` | `PASS_0_P0_0_P1_0_P2_3_P3_4_P4` |
| `s20_700_scb1_persistent_fuzz_slice` | `vulcan_review` | `PASS_PRIOR_P2_CLOSED_NO_OPEN_P0_P1_P2_WITH_P3_P4_FOLLOWUPS` |
| `s20_700_schema_persistent_fuzz_slice` | `vulcan_final_review` | `PASS_0_P0_0_P1_0_P2_4_P3_3_P4` |
| `s20_700_schema_persistent_fuzz_slice` | `vulcan_review_revision_2` | `PASS_0_P0_0_P1_0_P2_4_P3` |
| `s20_700_semantic_checkers_persistent_fuzz_slice` | `vulcan_review` | `PASS_0_P0_0_P1_0_P2_2_P3_3_P4` |
| `s20_700_smp1_json_bridge_persistent_fuzz_slice` | `vulcan_review` | `PASS_0_P0_0_P1_0_P2_6_P3_5_P4` |
| `s20_700_smp1_persistent_fuzz_slice` | `vulcan_review` | `PASS_0_P0_0_P1_0_P2_3_P3_3_P4` |
| `s20_700_vm_persistent_fuzz_slice` | `vulcan_review` | `PASS_0_P0_0_P1_0_P2_0_P3_5_P4` |
| `standards_sbom_and_provenance` | `ariadne_contract_review` | `PASS_0_P0_0_P1_0_P2_4_P3` |
| `standards_sbom_and_provenance` | `ariadne_contract_review_revision_3` | `PASS_0_P0_0_P1_0_P2_4_P3` |
| `standards_sbom_and_provenance` | `vulcan_surface_review_revision_4` | `PASS_0_P0_0_P1_0_P2_0_P3_1_P4` |
| `threat_coverage` | `independent_security_review` | `PASS_0_P0_0_P1_0_P2_3_P3_5_P4` |
| `vm_extended_opcode_profile` | `nabu_architecture_review` | `PASS_0_P0_7_P1_6_P2_5_P3` |

## GA implications / concrete completion order

The 19 unevidenced GA entries are not all review-label repairs. Several test terminal statuses directly: candidate validation/commit, mutation descriptors/construction, policy/capability, native refs, packaging. Current names deliberately carry restricted/proposal/narrow/boundary scope. A package should gain a terminal status only when its governing profile and completion gate truly permit it, not because a string suffix makes the GA report green.

1. Apply the two metadata corrections and evidence-backed historical wording repairs, preserving original text and pointers.
2. Create a residual ledger for each real item behind the 41 rows, including the hidden CLI/complete-entity follow-ups noted above. Match duplicate rows to the same original finding IDs to avoid duplicate work.
3. Repair/review AR-06 and exact verdict parsing; keep RW075 self-review and premium FAIL historical evidence intact.
4. Close residuals with targeted implementation/record fixes and independent verification. Parent is handling threat/security closure.
5. Review package terminal transitions against actual owner contracts and stage checkers; do not broaden technical promises as a bookkeeping fix.
6. Complete the real succession campaign and all thresholds, then S20-740 independent review over a genuinely CLEAR register; record the final-candidate release decision only when its candidate/build identity is current.

This report does not authorize a release, spend money, dispatch a harness, run a campaign, or claim checks executed. The operator's broader authorization is held by the parent; this subtask contributes independently inspected evidence and corrective recommendations.
