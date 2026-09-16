# Qualification historical closure audit

Date: 2026-09-15. Source inspected: `1fc327bb` plus the live files named below. Scope is the five accepted qualification sections only. This is a read-only historical reconciliation, not a new Council verdict, execution claim, security acceptance, or GA decision. No accepted build or test campaign was rerun.

## Result

The register has **16 carried PASS rows in these five sections**: root 6, reproducibility 2, packaging 5, standards 3, dossier 0. They are not 16 independent defects. Some duplicate the same historical round; some contain fully repaired findings; others carry real evidence or documentation work that the final qualification review did not cover.

**Three complete row corrections are justified without further implementation or acceptance:** the two reproducibility base aliases and standards Vulcan revision 4. Exact proposals are in `qualification-historical-closures.json`. Other rows need either the identified residual work or explicit, accurately scoped disposition; changing them all to zero-finding PASS would lose debt. Original transcripts should remain unchanged.

This audit uses anchors such as `P3[2]` to mean the second P3 in the original transcript's FINDINGS block. Those transcripts do not assign durable finding IDs; these are explicitly audit-local locators, not invented original IDs. Paths below are relative to the repository; role/commit references expand to `evidence/review/verdicts/<section>/<role-field>-<commit>.md` unless the newer form `<role>-<commit>.md` is specified.

## Final acceptance boundaries

The tracked `evidence/review/qualification-repair-closeout.{md,json}` records 15 actual scoped Council PASS transcripts, three per section. Root/repro/packaging/dossier scopes are `97b9117ce36bc0a8f9f8ecbb2514f4f97cea1deb`; standards scopes are `a7c69f8efa2267db944929288b2d508d40838fe4`. The accepted source candidate is `5313437afb5df66b5fa48b82c374746467d0a1ca`, archive `7da77f1b41b18845076ba1694e1163ed77301385417305f5eab00faf9be1ea3a`. Static reviewer checks and separately executed orchestrator validation are distinguished in those records.

The final root round closes the **wording-row record** after revision-7 implementation; its final delta does not review entity-read corpus/frame behavior. The final packaging round closes revision-5 stage-invariant tests and stale status/owner records. Final standards a7 closes risk/gap record prose; it changes no SBOM/provenance mechanism. A clean current delta is not universal retrospective closure of every P4 ever mentioned.

`build_finding_register.py` deliberately does not supersede a PASS-with-findings merely because another PASS exists. `closed_severities()` requires explicit severity closure. Same-lane FAIL supersession also checks subject cores: a root query review cannot silently become an entity-read review. Do not weaken either rule to obtain clearance. GA consumers depend on this clearance and section status; the accepted qualification artifact can remain valid while global readiness remains unevidenced.

## 1. root_backed_query_profile — six rows

### Rows R1/R2: ariadne_contract_review; nabu_architecture_review

Both current values are `PASS_WITH_P3_P4_FOLLOWUPS_NO_P0_P1_P2`. Their prior PASS is the a4b6029 transcript, with a809906 and c1d4177 subsequent scopes.

| Original anchor | Actual resolution / residual |
|---|---|
| Ariadne a4 P3[1] summary revision/vector count, P3[2] WORK_PACKAGES revision; Nabu a4 P3[1] same | Corrected through `f7df74f2`, `f02f8de9`, `c1d41778`; current revision 7 and 27 vectors agree. c1 transcripts independently verify these sources. |
| Ariadne a4 P3[3], Nabu a4 P3[2]: checker only reports revision | `f02f8de9` asserts checker/spec/summary revision and binds revision paragraphs. c1 Nabu verifies reversion fails and whitespace reflow passes. |
| Ariadne a4 P4[1], Nabu a4 P4[2]: duplicate page-union sentence | `f02f8de9`; closed explicitly at c1. |
| Ariadne a4 P4[3], Nabu a4 P4[1]: verify() doc lacks arm carve-out | `f02f8de9`; c1 Nabu verifies the arm-first entry points and updated comment. |
| Ariadne a4 P4[2], Nabu a4 P4[3]: rule 1 repeats completeness despite arm-gate preamble | **Still present**, `ROOT_BACKED_QUERY_PROFILE_V1.md:88`. c1 Nabu explicitly says held open by design; e050 Nabu preserves it. Neither final97 root round edits that rule. Outcome-consistent editorial debt, not an engine defect. |
| Nabu a4 P2: boundary-held entity-read/S20-310 wording | Final97 closes the authorized section3/7/9/10 wording packet, not every sentence in ENTITY_READ_PROFILE_V2. The a4 P2 bundles ownership-direction text with the separately recorded wording gate. Do not infer universal P2 closure from this row's abbreviated PASS token; section11 now describes composition, but ENTITY_READ_PROFILE_V2's unilateral ownership statement remains. |
| Nabu a809 P4: ADR-0030 lacks section11 composition acknowledgment | **Still not acknowledged** in ADR decision text. c1 explicitly held this open; `6cee454b` updates revision context, not section11 composition. This is later-lane debt associated with the base history. |

Subsequent actionable a809 defects (section7 audit attribution/class names/class1 count; section9 omitted paging arms; revision markers) close at `f02f8de9`/`c1d41778`. c1 closeout-count defects close at `ede5ef0f`, reviewed e050. Missing c1 lane history and doc-comment/packet-token records close through `27ad8d7e` and `5313437a`, with final97 closing the exact wording row. These legitimate closures do not remove the held rule1/ADR residues. **No complete-row value replacement proposed.**

### Row R3: vulcan_surface_review

Current `PASS_0_P0_0_P1_0_P2_2_P3_1_P4`; source `vulcan_surface_review-43f2f5b.md`.

| Original anchor | Actual resolution / residual |
|---|---|
| P3[1]: arm rejection independently adjudicated only by descriptor; generator stamps tamper by row ID | **Unresolved.** `check_root_backed_query_vector.py:603` still maps descriptor arm1 directly to the refusal; `generate_root_backed_query_fixtures.py:119` stamps descriptors. No source commit after the reviewed oracle repair changes this path. The Rust refusal is real; the residual is the depth of the independence claim. a809 explicitly says corpus/oracle unchanged. |
| P3[2]: degenerate roots/entry-point walks leave truncated arms unpinned | **Closed** by `f7df74f2` unit walk, extended at `f02f8de9`; c1 independently verifies Roots, EntryRows, DependencyRows and InventoryEntries. Revision7 section9 now states the actual bounded evidence rule. |
| P4[1]: union checker assumes disjoint pages | **Unchanged:** oracle lines546–571. It is correct for the frozen degenerate fixture; original residual concerned a future nondegenerate fixture with overlapping requests. Current accepted evidence does not exercise that future shape. A documented applicability limit is a possible disposition, not a repaired checker. |

Final97 records-only PASS does not claim to replace the arm oracle. **No complete-row replacement.**

### Rows R4/R5/R6: the three *_entity_read_review fields

Current values all `PASS_WITH_P3_P4_FOLLOWUPS_NO_P0_P1_P2`. Original sources: Ariadne/Vulcan246d5c4; Nabu9ae09a1. The original transcripts explicitly distinguish these fields from the root-query lane. No97 entity-read re-review exists.

| Source anchor | Closure / current source |
|---|---|
| Ariadne246 P3[1], Vulcan246 P3[1]: wrong sequence discharge table and missing request-id/exhausted-stale tests | Closed at `3704f3ee`: current ENTITY_READ_PROFILE_V2 section7 names correct methods; `server_tests.rs:3612,3651` contain the exact missing request-id reuse and exhausted-budget/stale-root tests. |
| Ariadne246 P3[2]: max_work bound rows not consumed | **Audit correction after direct corpus-key inspection:** `bound_work_exact`/`bound_work_below` are row IDs whose relation values are `work_exact`/`work_one_below`; the existing six-row loop already consumes both. The original review/audit inference of missing coverage was wrong. No owner repair is required; the follow-up correction adds explicit row-ID assertions so this coverage is visible. |
| Ariadne246 P4[1]: refresh HEAD names previous commit while encoder bytes came from dirty repair | Historical provenance discrepancy remains recorded (`refresh_head_revision` 9ae09a1, encoder hash63e82f). Current encoder matches that hash, so this is **not current encoder drift**; do not rewrite history to claim a clean refresh happened. Requires honest provenance annotation or an authorized new refresh if the contract requires one. |
| Ariadne246 P4[2]: checked_overflow uses different operands than the declared row | **Unresolved.** Current test uses ver_ws and u64::MAX ceilings, not the row's declared arithmetic operands (`entity_read.rs:2545`). It proves overflow refusal, not exact-row replay. |
| Ariadne246 P4[3]: target agree() precedes kind check | Carried unreachable-on-VerifiedRevision ordering note; no scoped closure located. It should be explicitly dispositioned, not erased by a records-only root PASS. |
| Nabu9ae P3[1]: repo projection bypassed by corpus test | Closed at `cff53963`; real imported corpus bytes feed production `view_object` in `projection_reproduces_every_corpus_stored_object` (`repo/entity_read.rs:404`). Ariadne246 independently accepts this repair. |
| Nabu9ae P3[2]: completion gate omits entity fields | Closed at `cff53963`; checker lines276+ binds all three entity-read fields; Ariadne246 confirms. |
| Nabu9ae P3[3]/[4]: WORK_PACKAGES and ERROR_CODES composition lag | Closed in the same repair series (`cff53963`, later revision pointers `f02f8de9`); Ariadne246 and current field note explicitly record closure. |
| Nabu9ae P4[1]: informational encoder hash stale | Closed by the subsequent corpus refresh at `cff53963`: current accepted manifest and encoder SHA256 both63e82fc8ee66a6b6d10ca18e978ee4cbc1aebef4998f24990b616d93470d9a35. Do not conflate this with Ariadne's later dirty-refresh provenance note. |
| Nabu9ae P4[2]: owning checker only lists accepted/SHA files; independent entity checker not named in section | **Residual.** `check_entity_read_vectors.py` does check accepted/rejected inputs when run from Make, but root profile ownership/registration still does not name it. Different question from executable conformance coverage. |
| Nabu9ae P4[3]: repo capture re-export/encode wrapper asymmetry and public audit views | Hygiene remains; re-export at repo/entity_read.rs:18 remains. No97 acceptance of this residual. |
| Vulcan246 P3[2]: corpus response wire bytes/frame ID/length never compared to Rust frame output | **Unresolved.** No `response_wire_hex`/`response_frame_id` consumption appears in Rust; accepted owner replay checks body/work/count. Generic protocol frame tests are not this corpus comparison. |
| Vulcan246 P4[1]: every-work_preflight doc claim exceeds six consumed rows | The comment overclaims the scope of this one test: six boundary rows run here and three additional work-preflight rows run in `rejected_relation_work_recomputes`. The bound_work rows themselves are already among the six. Follow-up documentation correction names both tests; no missing bound-row implementation. |
| Vulcan246 P4[2]: server.head clones full VerifiedRevision | Remains at `server.rs:1254`; bounded owner work does not mean bounded whole request. Performance/accounting limitation, not fixed by a test-only body comparison. |
| Vulcan246 P4[3]: finding-register supersession lane-blind | Closed by later subject-core-aware `field_core`/`supersedes`; current function explicitly prevents entity PASS from folding root-query FAIL. This repair does not grant cross-subject historical acceptance. |

**No whole entity row can be declared closed.** Proposed next implementation scope: exact positive response-frame replay; explicit relation-bound coverage and truthful overflow/doc claims; honest provenance/ownership annotations; explicit narrow disposition of remaining performance/hygiene limits. These are benign evidence/record corrections; no exploit reproduction is needed.

## 2. reproducibility_and_independent_conformance — two duplicate rows

### R7 ariadne_contract_review

Source `ariadne_contract_review-9ae09a1.md`: P2 stale84bfa9c attestation; P3[1] omitted0bcc9c6 round, [2] revision5 label with changed text, [3] smp1 v2 sums not refreshed; P4[1] unstated shape rules, [2] missing corpus-count/non-version tests, [3] hand-synchronized checker revision/commit, [4] uncoded missing runner script, [5] request history absent.

Closure map: P2 genuine two-host7a94a4a attestation filed `6a2eef7e`, then replaced by the fully bound5313437 campaign/final97 report; P3[1] `f7df74f2` plus lineage corrections `b9ae9a9b`; P3[2] and P4[1]/[2] revision6 `cff53963`/`3704f3ee`; P3[3] `05ecb86a`; P4[3]/[4]/[5] `b9ae9a9b` revision7 owner predicate, checker bindings, coded runner refusal and REQ-02 history. Later e050/400 receipt findings were repaired at `27ad8d7e`, `cd22b0ea`, `5313437a`, with final97 explicitly checking the actual receipt sequence.

The same9ae round is already represented by `ariadne_contract_review_revision_3 = PASS_P2_P3_P4_CLOSED` with a complete closure note. The base is an obsolete alias of that same round, not another outstanding review. **Replace base value by that exact existing disposition; append explicit alias/closure evidence note.**

### R8 nabu_architecture_review

Source `nabu_architecture_review-246d5c4.md`: P1 same stale attestation; P3 omitted9ae round; P4[1] silent-skip paths lacking tracked-set equality test, [2] missing whole-tree cross-reference/indent, [3] summary lacks tracked_corpus_directories.

Closure: P1 as above; P3 `b9ae9a9b` repairs unique round lineage; P4[1] `3704f3ee` adds `test_recorded_paths_equal_the_tracked_conformance_set`; P4[2] `b9ae9a9b`; P4[3] `f7df74f2`. Current revision4 field already says `PASS_P1_P3_P4_CLOSED` with these references. Final97 establishes current report/receipt alignment after the later integration changes. **Same safe alias correction.**

## 3. release_candidate_packaging — five rows

Read original abf4ff0/3320ca9 transcripts in full. Some stored count strings omit P2/P4 findings actually in those transcripts; raw transcripts, not abbreviated counters, govern this audit.

### R9 ariadne_contract_review_revision_3 (3320ca9)

Current `PASS_0_P0_0_P1_0_P2_1_P3`. P3 null/null toolchain binding closed at `43ea736b`, now strengthened by shared admissibility owner at `b9ae9a9b`. P4[1] stale base dispositions closed by filed revision fields `43ea736b` and rotations `6a2eef7e`; P4[2] old mint-worktree label was explicitly a note only; P4[3] stale ignored operator-tree artifact/evidence was outside tracked authority. Final97 validates new accepted archive and tracked evidence, but does not inspect disposition/retention of every old ignored local file. **Substantive counted P3 is closed; avoid inventing a disk-hygiene cleanup receipt.** Can declare P3 closed with a note preserving the original informational P4 scope, or leave row until an explicit record disposition is made.

### R10 nabu_architecture_review_revision_2 (abf4ff0)

Current string says 3P3 but transcript also contains twoP2 and fiveP4.

- P2[1] unreplayable --no-keep and P2[2] discarded partial failure evidence: `d01237f3`, verified3320ca9.
- P3[1] missing normative register-attestation binding: `d01237f3` plus `7ecde3d9`, verified3320ca9.
- P3[2] **smoke output owner map remains absent**. Make now splits build/verify and writes S20-710/730/750 records; section9 describes S20-720 evidence and section10 excludes those other responsibilities without an output-to-owner map. Final97 section15 stage/content scope does not close this older Nabu finding.
- P3[3] missing per-package claim shape: `d01237f3` adds p1_open shape. That is a record-shape repair, not proof all unclaimed lower-severity debt is tracked.
- P4[1] narrow/unclean attestation tuple: `d01237f3`, `43ea736b`, `b9ae9a9b`; P4[2] inferred make_target: narrowed `7ecde3d9` and confirmed3320; P4[3] one toolchain sample: `d01237f3`; P4[4] indent: **three-space continuation still present** in section7; P4[5] old retained-worktree/closeout pointer: live candidate pointer repaired `43ea736b` and later closeout updates, historical disk retention not reviewed here.

**Do not close whole row.**

### R11 nabu_architecture_review_revision_3 (3320ca9)

P3[1] closeout's hardcoded current candidate: `43ea736b` then subsequent pointer update makes summary candidate_* authoritative; final97 closeout marks old narrative historical. P3[2] missingabf round field: `43ea736b`. P3[3] same **owner map remains**. P4[1] FailureEvidenceTests below main guard and [2] offline_tests unbound: `43ea736b`, current checker pins count14; [3] section7 indent remains; [4] historical worktree retention not verified. **Do not close whole row.**

### R12 vulcan_surface_review_revision_2 (abf4ff0)

Original P2[1] incomplete/nonclean nine-tuple, P2[2] shifted severity strings: `d01237f3`, confirmed3320. P3[1] no-keep replay and P3[2] no whole-tree/invocation tests: `d01237f3`, confirmed3320. P4[1] dated paragraph/marker/indent: section13/header/markers repaired in `d01237f3`/`7ecde3d9`, indent remains; P4[2] FAIL invocation absent: `d01237f3`; P4[3] historical mint retention not verified. **Counted P3s close, whole transcript is not a zero-residual record.**

### R13 vulcan_surface_review_revision_3 (3320ca9)

P3[1] null toolchain and [2] closeout candidate pointer: `43ea736b`, with current owner predicate `b9ae9a9b` and historical closeout framing final97. P4[1] test main guard, [2] sibling null diagnostic, [3] absentabf round fields: `43ea736b`. P4[4] old review/mint worktree retention is outside final97 and not inspected here. **Counted substantive findings repaired; preserve informational old disk-hygiene limit unless independently dispositioned.**

Final97 does independently close the newer 400895e findings: real staging path tests, extra/missing fixed member refusal72007, count14, revision5/15-member status/closeout, license-blocker removal. Those are fixes at `5313437a`; none repairs the older Nabu owner map or section7 indentation.

## 4. standards_sbom_and_provenance — three rows

### R14/R15 ariadne_contract_review and ariadne_contract_review_revision_3

Both `PASS_0_P0_0_P1_0_P2_4_P3` represent **the same3320ca9 PASS**, not the a4b6029 harness amendment. The a809906 transcript explicitly identifies and audits both rounds separately.

Original3320 P3[1] missing revision fields: `43ea736b`; P3[2] unexercised SBOM result/manifest/size admission gates: tests at `43ea736b`; P3[3] tautological positive admission control: extracted predicate/direct-negative tests at `43ea736b`; P3[4] undefined/asymmetric names tuple: symmetric four-tuple and contract at `43ea736b`. **All four P3 closures explicitly independently confirmed by Ariadne a809906.**

Original P4[1] plural Makefile renderings: same43ea fix. P4[2] SBOM mismatch-state namespace accepts any attestation digest without clean/repro filter: **a809 explicitly says unchanged, consistent with section7 and moot under the report builder, not re-listed**. This is an explicit nonblocking disposition, not an implemented filter. P4[3] revision3 closeout: a809 explicitly accepts it as historical P0 closeout, not re-listed; later dated revision context further distinguishes history.

Later a4 amendment P2 live test, P3 revision pointer and P4 bound-input prose close at db1bc62 repair/`f7df74f2`; later a809 SPDX extracted-license defect and shape/record wording close `40dbbd04`; integration corrections at `27ad8d7e` and record findings at `5313437a`/`a7c69f8e`, followed by finala7 PASS. Thus **the four counted P3s are genuinely closed and the two remaining old P4 observations have an explicit later reviewer disposition**. A parent may normalize these duplicate P3-count tokens to `PASS_P3_CLOSED` only if its note explicitly preserves the unchanged P4 filter limitation and cites a809's not-relisted adjudication; do not write `P4_CLOSED` or claim finala7 changed that code. The attached JSON offers this narrowly worded optional correction separately from the three fully closed rows.

### R16 vulcan_surface_review_revision_4

Current `PASS_0_P0_0_P1_0_P2_0_P3_1_P4`. Actual source is **db1bc62**, not a4b6029: a4 was the earlier REVISE, corrected in the harness amendment note. db1 source has P3 held cosmetic duplicate-cause reporting and P4 admission-test name overclaim.

P4 test name changed at `f7df74f2` to `test_builders_admit_an_eligible_closure_keeping_the_candidate_binding` with scoped comment. Ariadne a809 explicitly verifies it. P3 duplicate-cause labels initially changed at f7 but created a separate70283ce P3; `a809906f` repairs the fold to use builder-attributed closure refusal with four tests. Vulcan a809 independently PASSes that exact fold. Later finala7 keeps that mechanism untouched and accepts the remaining record correction. **Use `PASS_P3_P4_CLOSED`; preserve db1 original verdict/held counts in the note and correct the mistaken a4 provenance.**

## 5. decision_dossier — zero rows

No old PASS-with-findings row appears in the register's unclaimed list for this section. Do not manufacture a closure edit. Final97 closes the current CLI ValueError/76001 boundary and revision9 record alignment; preserve that scoped acceptance.

## Apply guidance

1. Apply the three fully supported exact changes in the JSON, retaining prior note content and raw transcripts.
2. The two optional standards duplicate changes are **P3 closure plus explicit carried-limit disposition**, not all-severity closure. They cite an actual later review that declined to relist the P4, rather than treating user authorization as reviewer acceptance.
3. Keep all three entity rows and the other root/packaging residuals visible until their actual scoped work/disposition is complete. Do not replace status, global readiness or original reviewer identity.
4. Regenerate register/GA/dossier only through existing builders after parent changes; this audit neither ran those builders nor asserted final clearance. Do not launch another accepted build merely to correct these metadata fields.

## Audit correction during implementation inspection

On 2026-09-15, inspecting `rejected.json` together with the relation dispatch established that `bound_work_exact` and `bound_work_below` are already consumed: their relation tags are `work_exact` and `work_one_below`. The earlier audit incorrectly searched for row IDs as dispatch tags. The table above is corrected transparently. Exact overflow-operand replay, response-frame replay, and overbroad single-test documentation remain distinct correction work; no corpus bytes need changing.
