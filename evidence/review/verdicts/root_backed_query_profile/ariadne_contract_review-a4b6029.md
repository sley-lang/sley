## Ariadne contract review (final): `root_backed_query_profile` / `ariadne_contract_review`

**Baseline**: `git rev-parse HEAD` = `a4b60294e4dd75133fa08f92bf16022fb05bb807`. Match.
Read-only except this verdict file. No tracked file modified; working tree
carries only untracked verdict files from other lanes.

**Prior**: `evidence/review/verdicts/root_backed_query_profile/ariadne_contract_review-0bcc9c6.md`
(REVISE on the nineteen-class contract: P1-1 arm rule, P1-2 cursor-before-drift,
P1-4/5 paging keys + page-union rule, P1-6 walk-per-class, P1-7 exclusions,
P2-7 31008/31000 vectors, plus P2/P3/P4 notes). Read first; this verdict
supersedes it on the `ariadne_contract_review` field only. It does not touch
the entity-read `*_entity_read_review` fields (all PASS, retained as history)
and does not alter any S20-310 normative wording.

**Method**: read `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` (rev 5) in full,
re-derived each rev-5 claim against `crates/sley-query/src/root_query.rs`
(build/execute order, `key_tag`, `verify()`), the fixture generator, and the
pinned corpora; ran `scripts/check_root_backed_query_profile.py` (PASS,
revision 5, 19 classes, zero problems) and
`scripts/generate_root_backed_query_fixtures.py --check` (PASS, 27 vectors,
10 rejections, drift []); recomputed corpus pins with `sha256sum -c`
(both OK). HEAD's `root_query.rs` delta is tests-only (page walks, binding
and arm rejection pins); no engine behavior changed and none required.

### Closure ledger (enumerated revision-5 text + corpus repairs)

| Repair | Evidence at a4b6029 | Result |
|---|---|---|
| S1 arm split (arm-1 -> item 2, binding -> item 5) | Spec `ROOT_BACKED_QUERY_PROFILE_V1.md:69-74` carves arm-1 out of binding into `QUERY_PROFILE_UNSUPPORTED` (item 2); every other rule fails `QUERY_ROOT_MISMATCH` (item 5). Both entry points gate arm first: `build_root_query_request` (`root_query.rs:682-684`) and `execute_root_query` (`:887-889`) run before shape/cursor/binding. Corpus mutation `arm-1-snapshot-profile` pins 31000. | CLOSED |
| S8 items 3-4 split (shape before cursor, drift after cursor) | Spec `:393-399` states shape-before-cursor with drift checked after the cursor. Engine order is exactly limits, arm, shape (`validate_shape`, `:890`), cursor (`validate_cursor`, `:891`), drift/identity recompute (`:892-904`), `verify()` (`:905`); build uses the same prefix order (`:681-687`, drift omitted by construction). | CLOSED |
| S3 named paging keys + page-union rule | Spec `:238-247` names per-class keys (class 8 member identity, class 10 entry-point identity, edge `(dependent, dependency, kind)` triples, `StateRoot` roots) matching `key_tag` (`root_query.rs:402-413`); union predicate (identical requests except `after`, strictly increasing keys, exact `total_count` every page) is valid, and `verify()` enforces `strictly_increasing` entry points / dependency roots (`:267-271`), closing the silent-truncation hole. | CLOSED |
| S10 non-enumerating lookup classes | Spec `:446-449` names `GetEntity`, `GetSemanticFingerprint`, `ListOwningNamespaces`, `ListDeclaredEffects` as named-subject-only with `ListEntitiesByKind` as the enumerating class; consistent with `key_tag` single-key set (classes 1, 2, 3, 9, `:404-407`). | CLOSED |
| Corpus 27 vectors (page-roots-1/2 tag-3 Root cursor, page-entry-points-1/2) | `accepted.json` carries all 19 `class-NN` vectors plus 4 walk pairs; `page-roots-2` `after` is `tag 3` Root, `page-entry-points-2` `after` is entity. Byte-arithmetic re-derivation confirms paging: page-roots 320 + 36 (Root cursor) - 32 (one root payload) = 324; page-entry-points 356 + 36 (entity cursor) - 68 (one entry row) = 324. Emitter added in `root_query.rs` tests (`git show a4b6029`); generator `EXPECTED_VECTORS` names exactly these 27. | CLOSED |
| Corpus 10 rejections (31008 + 31000) | `rejected.json` mutations include `binding-substituted-fact` (`QUERY_ROOT_MISMATCH`, 31008) and `arm-1-snapshot-profile` (`QUERY_PROFILE_UNSUPPORTED`, 31000); both pinned at build and execute stages in tests. Generator `EXPECTED_REJECTIONS` names exactly these 10. | CLOSED |
| SHA256SUMS current | `sha256sum -c` in `conformance/root-backed-query/v1`: both files OK; `git show a4b6029` re-pinned both digests alongside the corpus delta. | CLOSED |
| Mechanized Rust-vs-vector comparison (root side) | Generator `--check` runs the Rust emitter via cargo and diffs against the corpus: `drift: []`. The prior P2 gap was entity-read-specific and remains entity-read-scoped (see still-open notes). | CLOSED |
| seq_root_advanced 33002 | `conformance/entity-read/v2/inputs.json:561` expects `SESSION_ROOT_ADVANCED` 33002 with the admission-precedes-decode note; `rejected.json:1243` carries the matching row. | CLOSED (already-correct) |
| AT-MW-02 stale-marked | `machine-summary.json:120` carries the STALE annotation (2026-09-11: methods 306/307 exist under S20-310; S20-410 ownership superseded). | CLOSED (already-correct) |

### Findings (lane scope; none report-grade above P3)

[P3] [record] machineresearch/sley-2.0/machine-summary.json (root_backed_query_profile.contract_revision = 4, fixture_vectors = 23) - section record trails spec revision 5 and the 27-vector corpus; checker passes (it reports revision without asserting it). Integrator-owned; not edited per task boundary; bump at close.
[P3] [record] docs/WORK_PACKAGES.md:36 - S20-310 row states "contract draft revision 4 (2026-09-11)"; trails rev 5. Same integrator record class as above.
[P3] [record] scripts/check_root_backed_query_profile.py:218-228 - spec revision extracted and reported but never asserted against the section record (gates only at COMPLETE status); the live 4-vs-5 divergence demonstrates the gap. Gate-hardening note, no contract content.
[P4] [contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:243-247 - page-union sentence duplicated verbatim (rev-5 predicate insert plus the original closing sentence); harmless redundancy, no semantic conflict.
[P4] [contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:76-77 vs :69-74 - arm condition retained inside the numbered binding-rule list while the new prose assigns arm-1 to the item-2 gate; outcome-consistent (engine order decides), residual old P1-1 shape only.
[P4] [implementation-doc] crates/sley-query/src/root_query.rs:211-222 - verify() doc comment frames snapshot disagreement as QUERY_ROOT_MISMATCH without the section 1 arm carve-out; unreachable via gated entry points (arm refused first, 31000 pinned at both stages).

### Still open, out of scope (noted live per task boundary; not lane-blocking, not counted)

- Entity-read Rust-vs-vector harness (prior-round P2, entity-read corpus): unchanged this wave, entity-read-field scope; root side is mechanized via generator `--check` (drift []).
- S20-310 normative wording surfaces: untouched by this wave and must stay untouched (`git show a4b6029 --stat` touches no S20-310 normative file).
- Engine-side items (cursor/drift reorder already matches contract; facts-ordering / page-stitching / accept_cached engine P1s): engine behavior unchanged, other lanes' fields.
- Ownership record binding: integrator-owned records listed above as P3 notes only.

VERDICT: PASS_0_P0_0_P1_0_P2_3_P3_3_P4 / SECTION: root_backed_query_profile / FIELD: ariadne_contract_review / SCOPE_SHA: a4b60294e4dd75133fa08f92bf16022fb05bb807 / FINDINGS:
/ [P3] machineresearch/sley-2.0/machine-summary.json (root_backed_query_profile.contract_revision = 4, fixture_vectors = 23) - section record trails spec revision 5 and 27-vector corpus; integrator bump at close, not edited per boundary
/ [P3] docs/WORK_PACKAGES.md:36 - S20-310 row states revision 4 (2026-09-11); trails rev 5
/ [P3] scripts/check_root_backed_query_profile.py:218-228 - revision reported but never asserted against the section record; recommend asserting at COMPLETE status
/ [P4] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:243-247 - page-union sentence duplicated verbatim; harmless
/ [P4] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:76-77 vs :69-74 - arm condition retained in numbered rule list though assigned to the arm gate; outcome-consistent
/ [P4] crates/sley-query/src/root_query.rs:211-222 - verify() doc comment lacks the section 1 arm carve-out; unreachable via gated entry points
/ SUMMARY: On a4b6029 every enumerated revision-5 text+corpus repair is genuinely closed by re-derivation: the section 1 arm split (prose, arm-first order at both engine stages, 31000 pin + corpus mutation), the section 8 items 3-4 split (text matches build/execute order exactly), the section 3 named paging keys plus a valid page-union predicate backed by the strictly-increasing guard and byte-verified walk pages including the tag-3 Root cursor, the section 10 exclusion list, the 27+10 pinned corpus with current SHA256SUMS and zero-drift mechanized check, and the already-correct 33002 expectation and AT-MW-02 STALE annotation. Checker PASS with zero problems. No P0/P1/P2 remains in lane scope; the three P3s and three P4s are integrator-record and editorial follow-ups with no contract content. Out-of-scope items (entity-read harness, S20-310 wording, engine-side P1s, ownership records) are noted as still-open where live and are not required for PASS. This verdict supersedes ariadne_contract_review-0bcc9c6 on this field only and neither supersedes nor disturbs the entity-read PASS fields.
