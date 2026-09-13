## Nabu architecture review (final): `root_backed_query_profile` / `nabu_architecture_review`

**Baseline**: `git rev-parse HEAD` = `a4b60294e4dd75133fa08f92bf16022fb05bb807`. Match.
Read-only except this verdict file. Working tree carries only untracked
verdict files from other lanes; no tracked file is modified.

**Prior**: `evidence/review/verdicts/root_backed_query_profile/nabu_architecture_review-0bcc9c6.md`
(REVISE: P2 register-section absence, ownership-record binding incl.
S20-310 surfaces + AT-MW-02 staleness, P3/P4 notes). Read first; this
verdict supersedes it on the `nabu_architecture_review` field only, and is
itself root-query scoped: it does not touch the entity-read `*_entity_read_review`
fields (all PASS, retained as history) and does not alter any S20-310
normative wording.

**Method**: read `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` (rev 5) in full,
re-derived the rev-5 claims against `crates/sley-query/src/root_query.rs`
(build/execute order, `key_tag`, `verify()`), the fixture generator, and the
pinned corpora; ran `scripts/check_root_backed_query_profile.py` (PASS,
revision 5, 19 classes, zero problems); recomputed corpus pins with
`sha256sum`. No engine behavior was changed by this lane and none is
required: HEAD's `root_query.rs` delta is tests-only (page walks, binding
and arm rejection pins).

### Closure ledger (prior REVISE items)

| Prior item | Evidence at a4b6029 | Result |
|---|---|---|
| P2 register-section absence | Register now carries distinct scoped fields: `nabu_entity_read_review` = `PASS_WITH_P3_P4_FOLLOWUPS_NO_P0_P1_P2` (REQ-05 scope, 9ae09a1), plus dated `nabu_architecture_review_revision_1` REVISE scoped to entity-read on 0bcc9c6, explicitly non-superseding. This final is root-query scoped, so no laundering path remains. | CLOSED |
| P2 ownership binding (composition) | Spec section 11 (`ROOT_BACKED_QUERY_PROFILE_V1.md:453-468`) composes owner, adapter, corpus + `SHA256SUMS`, and vector line; `machine-summary.json` `implementation` lists all six surfaces; `check_root_backed_query_profile.py:85-97,195-206` enforces the entity-read markers; `ERROR_CODES_V1.md:230-242` names the 31004/31007/31008/31010 reuse with owner-numeric identity; `WORK_PACKAGES.md:36` S20-310 row composes the entity-read surface. | CLOSED |
| P2 ownership binding (AT-MW-02) | `machine-summary.json:120` carries the STALE annotation (2026-09-11: methods 306/307 exist under S20-310 with contract, implementation, corpus; S20-410 ownership superseded). | CLOSED |
| Rev-5 arm split | Spec section 1 (`:69-74`) carves arm-1 out of binding into `QUERY_PROFILE_UNSUPPORTED` (precedence item 2); engine gates arm first in `build_root_query_request` (`root_query.rs:682-684`) and `execute_root_query` (`:887-889`) before shape/cursor/binding; tests pin 31000 at both stages; corpus mutation `arm-1-snapshot-profile` added and pinned. | CLOSED |
| Rev-5 cursor/drift split | Spec section 8 items 3-4 (`:393-399`) state shape-before-cursor, drift-after-cursor; engine order is exactly limits, arm, shape (`validate_shape`), cursor (`validate_cursor`), then drift/identity recompute (`:892-904`), then `verify()` (`:905`) in execute (build omits drift, same prefix order). | CLOSED |
| Rev-5 paging keys + union rule | Spec section 3 (`:238-247`) names per-class keys (member identity, entry-point identity, edge triple, `StateRoot`) matching `key_tag` (`root_query.rs:402-415`); union predicate (identical requests except `after`, strictly increasing keys, exact `total_count` every page) is valid; `verify()` now enforces `strictly_increasing` entry points / dependency roots (`:267-271`), closing the silent-truncation hole. Corpus walks cover every paged class (new `page-roots-1/2`, `page-entry-points-1/2`). | CLOSED |
| Rev-5 exclusions | Spec section 10 (`:446-449`) names the four non-enumerating lookup classes with `ListEntitiesByKind` as the enumerating class; consistent with `key_tag` (single-key classes 1-3 plus unpaged chain class 9 return `None`). | CLOSED |
| Corpus pins | `conformance/root-backed-query/v1`: 27 vectors (19 classes + 8 page walks) + 10 mutations (incl. `binding-substituted-fact`, `arm-1-snapshot-profile`); `sha256sum` recompute matches `SHA256SUMS`; generator `EXPECTED_VECTORS`/`EXPECTED_REJECTIONS` name exactly these. | CLOSED |
| `seq_root_advanced` 33002 | `conformance/entity-read/v2/inputs.json:561` and `rejected.json:1253` expect `SESSION_ROOT_ADVANCED` 33002 with the admission-precedes-decode note. | CLOSED |
| Architecture axes (owner/adapter/selection/capture/oracle) | Unchanged from the prior round; all PASS and re-confirmed by reading (dependency direction, thin trusted-input adapter, revision-borrowed selection, capture-consumes-views, pure-Python oracle). | PASS |

S20-310 normative wording untouched by this wave: `git show HEAD --stat`
touches no S20-310 normative file (`ENTITY_READ_PROFILE_V2.md`,
`ERROR_CODES_V1.md`, closeout, work-package rows unchanged).

### Findings

[P2] [held-gated] docs/spec/ENTITY_READ_PROFILE_V2.md:4,239 + S20-310 wording surfaces - ownership is still declared single-direction on the S20-310 normative side; the residual S20-310-side acknowledgment stays open by boundary (S20-310 normative wording frozen; section record contract_text_review = PENDING_S20_310_WORDING_DECISION_PACKET_ROUND7_P1_4_IMPLEMENTED). HELD-GATED per task boundary: not required for PASS, gated on the wording packet, no engine or layering content.
[P3] [record] machineresearch/sley-2.0/machine-summary.json (root_backed_query_profile.contract_revision = 4, fixture_vectors = 23), docs/WORK_PACKAGES.md:36 ("revision 4") - section record trails spec revision 5 and the 27-vector corpus; checker passes (it reports revision without asserting it). Integrator to bump at close; no architecture content.
[P3] [record] scripts/check_root_backed_query_profile.py:218-228 - revision extracted and reported but never asserted against the section record (residue of prior-round P1-7); the live 4-vs-5 divergence proves the gap. Downgraded: scripts/records_closure.py is now the revision authority, so this is a gate-hardening note, not a layering defect; recommend asserting at COMPLETE status.
[P4] [implementation-doc] crates/sley-query/src/root_query.rs:211-222 - verify() doc comment still frames snapshot disagreement as QUERY_ROOT_MISMATCH without the section 1 arm carve-out; unreachable via build/execute (arm gated first, 31000 pinned at both stages) but the doc overstates verify()'s domain.
[P4] [record] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:243-247 - page-union sentence duplicated verbatim (rev-5 predicate insert plus the original closing sentence); harmless redundancy, no semantic conflict.
[P4] [record] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:76-77 (rule 1) vs :69-74 - the arm condition is retained inside the numbered binding-rule list while the new prose assigns it to the arm gate; outcome-consistent (engine order decides) but the list placement preserves the old P1-1 shape textually.

VERDICT: PASS_0_P0_0_P1_1_P2-HELD-GATED_2_P3_3_P4 / SECTION: root_backed_query_profile / FIELD: nabu_architecture_review / SCOPE_SHA: a4b60294e4dd75133fa08f92bf16022fb05bb807 / FINDINGS:
/ [P2] docs/spec/ENTITY_READ_PROFILE_V2.md:4,239 + S20-310 wording surfaces - single-direction ownership declaration remains; S20-310 normative wording frozen by boundary (contract_text_review PENDING round-7 packet); HELD-GATED, not lane-blocking
/ [P3] machineresearch/sley-2.0/machine-summary.json (contract_revision = 4, fixture_vectors = 23), docs/WORK_PACKAGES.md:36 - section record trails spec revision 5 and 27-vector corpus; integrator bump at close
/ [P3] scripts/check_root_backed_query_profile.py:218-228 - revision reported but never asserted against the section record; recommend asserting at COMPLETE status
/ [P4] crates/sley-query/src/root_query.rs:211-222 - verify() doc comment lacks the section 1 arm carve-out; unreachable via gated entry points
/ [P4] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:243-247 - page-union sentence duplicated verbatim; harmless
/ [P4] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:76-77 vs :69-74 - arm condition retained in the numbered rule list though assigned to the arm gate; outcome-consistent
/ SUMMARY: On a4b6029 the root-backed-query surface is architecturally sound on every lane-owned axis and every enumerated revision-5 repair is genuinely closed by re-derivation: the arm split (spec text, engine arm-first order at both stages, 31000 pins, corpus mutation), the cursor-before-drift order (spec items 3-4 match build/execute exactly), the paging keys plus a valid page-union predicate with the strictly-increasing guard and full-class walk coverage, the exclusion list, the section 11 composition with owner-numeric code reuse, the 27+10 pinned corpus, the 33002 seq_root_advanced expectation, the AT-MW-02 STALE annotation, and the dated non-superseding lane record with distinct PASS entity-read fields. The checker reports PASS with zero problems. The sole P2 is the boundary-held S20-310-surface wording item, explicitly not required for PASS; the two P3s and three P4s are record/doc follow-ups with no layering content. This verdict is scoped to the root-query field and neither supersedes nor disturbs the entity-read PASS fields.
