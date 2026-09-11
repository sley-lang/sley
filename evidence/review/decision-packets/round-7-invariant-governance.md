# Decision packet — invariant-gate governance lanes (round 7)

Baseline `5a3caf1`. Checker-scope detection repairs are landed (no
normative change); the dispositions below require owner/contract action
and are NOT authorized under the current order.

## Landed detection (qualification lane)

- `check_candidate_result_contract.py`: `validator_source_symbols` now
  unions direct `Failure::new` literals with `stale_root_failure("SYM")`
  and `resource_failure(N, "SYM")` literals; forwarding sites
  (`error.code().as_str()`, `error.source_symbol()`) stay excluded as
  preserved owner symbols. Retryability maps helper bodies.
- `check_error_symbol_registration.py`: multi-segment wildcard families
  (namespaces 34 → 49), longest-prefix ownership (emitted 330 → 399),
  helper-originated symbols, `Some(N)` / `Some("SYM")` / multi-variant
  pairing (codes 303 → 315, zero ambiguous), test-only exercise corpus
  with qualified-variant rule (unexercised 1 → 27, discriminating).
- Tracked report regenerated; summary pins refreshed (no hand edits).

## Newly visible sets (all FAIL-closed, dispositions elevate)

1. **11 validator-originated symbols undocumented in §8.1**
   (`CANDIDATE_BASE_*` ×4, `CANDIDATE_WORKSPACE_MISMATCH`,
   `CANDIDATE_SCHEMA_EPOCH_MISMATCH`, `CANDIDATE_POLICY_ROOT_MISMATCH`,
   `CANDIDATE_GRAPH_WORK_LIMIT`, `CANDIDATE_TEST_RESOURCE_LIMIT`,
   `CFG_RESOURCE_LIMIT`, `VM_LOWER_RESOURCE_LIMIT`,
   `CONTRACT_TEST_PLAN_RESOURCE_LIMIT`, `SCB_RESOURCE_LIMIT`):
   either amend `CANDIDATE_RESULT_V1.md` §8.1 (26 → 37 rows, S20-360
   contract change) or replace the emitter literals with
   already-documented symbols. Checker cannot decide.
2. **5 frozen `PACKAGE_*` unregistered** (`OVERSIZED`, `MALFORMED`,
   `SECTION_DIGEST_MISMATCH`, `BINDING_MISMATCH`, `HYDRATION_REFUSED`):
   registering them amends frozen `EXEC_PACKAGE_V1/V2.md` (RW-075,
   2026-09-06) against the §80-85 reservation for the RW-080
   strict-decoder handoff. Needs the RW-075 owner lane + freeze
   amendment, possibly deferred to RW-080.
3. **`CANDIDATE_VALIDATION_VALID`**: the success codec symbol is emitted
   but unregistered. Either register it or scope this checker to
   failure arms explicitly (checker-contract decision).
4. **Namespace declarations**: `EXCHANGE_*`, `IMAGE_*`, `AUTHORITY_*`,
   `ENTITY_READ_*`, `RAW_HASH_*`, `CANDIDATE_*` families need
   `ERROR_CODES_V1.md` declarations + owning-package code tables
   (S20-310/entity-read, admission authority, RW-075/RW-080, exchange,
   image/host-ABI, raw-hash owners).
5. **27 unexercised refusal paths**: owner backlog (candidate-apply
   arms, helper-originated symbols, CLI, exchange, pack, state-root,
   impact, VM-lower). No new ambiguous codes.

## Pre-existing (unchanged, still owned elsewhere)

- `check_error_symbol_registration --check` FAILs at baseline (5
  unregistered + 1 unexercised); now FAILs with fuller visibility.
  `make quick` Tier-1 state on this lane is unchanged in kind.
- Stale summary pin (329/302 vs 330/303) refreshed by regeneration.

## Operator decision needed

Route items 1-4 to their contract owners (S20-360, RW-075/RW-080,
S20-310, error-codes editor) as contract amendments, or accept the
FAILs as standing qualification debt with the gates left fail-closed.
