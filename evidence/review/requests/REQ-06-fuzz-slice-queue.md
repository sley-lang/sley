# Review queue REQ-06 — S20-700 persistent fuzz slice vulcan reviews (staged)

Baseline: current `main` HEAD at dispatch time (verify `git rev-parse HEAD`
before starting; these slices are commit-independentbehavioral reviews).

Each item is a first review (PENDING) or a restored deferred review
(DEFERRED_FORGE_OAUTH_401, lane now restored) of one persistent fuzz
slice's vulcan surface: harness shape (libFuzzer target exists and runs
the bounded smoke), corpus discipline (seeds minimize, crashes reduce to
regression vectors), oracle/slice checker agreement, and adversarial
adequacy for the slice's stated threat. Read-only; verdicts in the REQ
format against the named field; prior verdicts preserved.

PENDING (never reviewed):
1. `s20_700_complete_root_persistent_fuzz_slice.vulcan_review`
2. `s20_700_complete_root_snapshot_persistent_fuzz_slice.vulcan_review`
3. `s20_700_context_capsule_persistent_fuzz_slice.vulcan_review`
4. `s20_700_merge_persistent_fuzz_slice.vulcan_review`
5. `s20_700_root_query_persistent_fuzz_slice.vulcan_review`
6. `s20_700_semantic_delta_persistent_fuzz_slice.vulcan_review`
7. `s20_700_smp1_json_bridge_persistent_fuzz_slice.vulcan_review`
8. `s20_700_smp1_persistent_fuzz_slice.vulcan_review`

DEFERRED (dispatch now):
9. `s20_350_mutation_candidate_persistent_fuzz.vulcan_review`
10. `s20_600_frozen_legacy_adapter.vulcan_review`
11. `s20_700_adapter_responses_persistent_fuzz_slice.vulcan_review`
12. `s20_700_pack_persistent_fuzz_slice.vulcan_review`
13. `s20_700_query_persistent_fuzz_slice.vulcan_review`
14. `s20_700_schema_persistent_fuzz_slice.vulcan_review`
15. `s20_700_semantic_checkers_persistent_fuzz_slice.vulcan_review`

Note: `s20_700_vm_persistent_fuzz_slice.vulcan_review` reads
`PENDING_S20_700FUZZ_FIX_LANDED_REREVIEW` (fix landed, re-review
pending) and belongs in this queue when its slice is dispatched.

## Reclassification (append-only, qualification repair wave)

Item 10 (`s20_600_frozen_legacy_adapter.vulcan_review`) is MOVED, not
closed: it is a pinned-artifact adapter governed by
`LEGACY_ARTIFACT_ADAPTER_V1.md` (checker `scripts/check_legacy_runner.py`,
smoke `make legacy-runner-smoke`), not a persistent fuzz slice. Record:
`evidence/review/reclassification/s20-600-item10.md`. The original entry
above is preserved verbatim; the obligation is refiled under the
`legacy_artifact_adapter` lane as `legacy_adapter_contract_review`.

## Records correction (append-only, target-closure wave, baseline `ba10c41`)

Operator-adjudicated correction of the superseded mid-wave aggregate
(`5 PASS / 9 REVISE / 1 reclassified`). Authoritative detailed state at
`ba10c41`, by newest filed transcript per slice (history preserved; no
transcript edited, moved, or deleted):

| # | Slice | Newest transcript | Verdict | Counts as |
|---|---|---|---|---|
| 1 | `s20_700_complete_root_persistent_fuzz_slice` | `vulcan_review-7cf25acd25d2.md` | PASS | PASS |
| 2 | `s20_700_complete_root_snapshot_persistent_fuzz_slice` | `vulcan_review-38fc94a0dc5a.md` | PASS | PASS |
| 3 | `s20_700_context_capsule_persistent_fuzz_slice` | `vulcan_review-38fc94a0dc5a.md` | PASS | PASS |
| 4 | `s20_700_merge_persistent_fuzz_slice` | `vulcan_review-7cf25acd25d2.md` | PASS | PASS |
| 5 | `s20_700_root_query_persistent_fuzz_slice` | `vulcan_review-7cf25acd25d2.md` | PASS | PASS |
| 6 | `s20_700_semantic_delta_persistent_fuzz_slice` | `vulcan_review-7cf25acd25d2.md` | PASS | PASS |
| 7 | `s20_700_smp1_json_bridge_persistent_fuzz_slice` | `vulcan_review-38fc94a0dc5a.md` | PASS | PASS |
| 8 | `s20_700_smp1_persistent_fuzz_slice` | `vulcan_review-b64ab9e.md` | PASS (tag-count filing, scope `ad46a43`) | **REVISE (operator hold)** |
| 9 | `s20_350_mutation_candidate_persistent_fuzz` | `vulcan_review-e72de4912fc0.md` | PASS | PASS |
| 11 | `s20_700_adapter_responses_persistent_fuzz_slice` | `vulcan_review-e72de4912fc0.md` | PASS | PASS |
| 12 | `s20_700_pack_persistent_fuzz_slice` | `vulcan_review-7cf25acd25d2.md` | PASS | PASS |
| 13 | `s20_700_query_persistent_fuzz_slice` | `vulcan_review-6b12d67.md` | PASS | PASS |
| 14 | `s20_700_schema_persistent_fuzz_slice` | `vulcan_review-6b12d67.md` | PASS | PASS |
| 15 | `s20_700_semantic_checkers_persistent_fuzz_slice` | `vulcan_review-38fc94a0dc5a.md` | PASS | PASS |
| — | `s20_700_scb1_persistent_fuzz_slice` (adjacent slice, second review) | `vulcan_review-7cf25acd25d2.md` | PASS | PASS |
| 10 | `s20_600_frozen_legacy_adapter` | `vulcan_review-e464ed4.md` | REVISE (provenance only) | **reclassified, not a fuzz verdict** |

Corrected aggregate: **14 fuzz slices PASS, 1 fuzz slice REVISE (SMP1),
0 FAIL, plus 1 non-fuzz item reclassified to `legacy_artifact_adapter`.**

Standing notes (mechanical, not verdicts):

- Item 8 (SMP1): the `b64ab9e` PASS transcript is preserved untouched, but
  its review scope is `ad46a43` and the filing at `9df9f9e` was a revision
  disposition by tag count. Commit `9df9f9e` then added the
  `fixture_hellos_negotiate_ok` self-check test
  (`crates/sley-protocol/src/lib.rs`), which no review has covered. Per
  operator direction the obligation returns to REVISE until a fresh
  explicit qualifying verdict at the new baseline verifies the narrowed
  oracle arm, the server-side gate, the live regression seed,
  `S20-700-SMP1-001`, corpus execution, instrumentation reach, and crash
  absence. Crash history is retained (`fuzz/regressions/S20_700_SMP1_001.json`
  plus artifact records).
- Items 13/14 (query/schema): the round-1 `vulcan_review` REVISE rows stay
  PENDING under the register builder rules until the reviewing lane files a
  `final`-round PASS; the `vulcan_review_revision_2` PASS rows stand. The
  operator tally counts both slices PASS per their qualifying second-round
  transcripts; no PENDING row was closed by this correction.
- `s20_700_vm_persistent_fuzz_slice` remains
  `PENDING_S20_700FUZZ_FIX_LANDED_REREVIEW` (fix landed, re-review pending)
  and is outside this aggregate, still open.
- Reclassification counts as no technical closure: the item-10 PENDING
  obligations are preserved and extended, never reduced.
