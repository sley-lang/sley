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
