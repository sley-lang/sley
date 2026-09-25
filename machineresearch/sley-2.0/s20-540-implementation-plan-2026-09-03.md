# S20-540 implementation plan (draft, written before the freeze)

Status: PLAN for the implementation slice after the contract freeze; no code
exists. One owner per file set; Tier 1 `make quick` after every step, Tier 2
`make core conformance adversarial` at the handoff.

## Primitives already present (reuse, do not duplicate)

| Need | Existing primitive | Where |
|---|---|---|
| receipt install, idempotent over exact bytes | `persist_receipt` (stage, verify, hard-link, sync, `verify_existing_receipt` on `AlreadyExists`) | `crates/sley-txn/src/repository.rs` |
| receipt verification against the store | `load_verified_revision` = `read_receipt_readonly` + `verify_transaction_relationship` + `load_objects` + `verify_manifest_lengths` + `validate_inventory` | same |
| head write | `cas_head(None, new)` (stage, verify, rename, sync, reread) | same |
| head read | `read_head` | same |
| origin install, no overwrite, exact-existing tolerated | `persist_no_overwrite(path, bytes, BRANCH_STAGE_PREFIX, code, verify)` | `crates/sley-repo/src/refs.rs` |
| ref install | `persist_expected_ref` / `replace_ref` | same |
| origin and ref facts vs receipts | `verify_origin_target`, `verify_ref_target`, `validate_origin_ref_binding` | same |
| pack export/import | `export_conformance_pack`, `import_conformance_pack` (frozen S20-170) | `crates/sley-repo/src/lib.rs` |
| envelope/payload/tree codec pattern | `stored_pack_bytes`, `encode_payload`, `decode_envelope`, `decode_payload`, `compute_leaves`, `content_leaf`, `merkle_root`, `check_id_order`, `decode_absent_signature` | same |
| exclusive maintenance for a composite caller | `acquire_exclusive_repository_maintenance`, `RepositoryMaintenanceGuard`, `recover_with_maintenance` pattern | `crates/sley-txn/src/maintenance.rs`, `repository.rs` |

## Step 1: `sley-id` (identifier owner)

- `Domain::RepositoryExchange` with bytes `sley2.repository-exchange.v1`,
  appended to `ALL` (30 entries) and the domain-bytes match.
- `digest_type!(RepositoryExchangeId, Domain::RepositoryExchange)`.
- `IDENTIFIERS_V1.md`: table row `| repository exchange | sley2.repository-exchange.v1 |`.
- Frozen vector test and machine-summary `identifiers.domains` count.
- Gate: `cargo test -p sley-id`, `make quick`.

## Step 2: `sley-txn` (transaction owner, inside the S20-540 slice)

- `TransactionErrorCode::IncompleteClone` = `TXN_INCOMPLETE_CLONE` = `39022`
  (append; keep contiguity; update `TRANSACTION_MODEL_V1.md` table and
  `scripts/check_transaction_contract.py` `ERROR_CODES`).
- `pub fn incomplete_clone_marker_present(root: &Path) -> Result<bool, CommitError>`:
  `symlink_metadata` on `exchange/v1/`; a symlink or non-directory is `TXN_IO`;
  any regular-file entry ending in `.stage` (read via `read_dir`, no follow)
  returns true.
- Guard calls (after locks, before any acceptance write, ref write, or
  deletion): `initialize_trusted_genesis_inner` (after `acquire_lock`),
  `commit_inner` (after the accepted lock), `recover_with_maintenance`
  (before cleanup).
- `initialize_trusted_clone_receipts_with_maintenance(&self, guard, expected_head, receipts: &[&[u8]]) -> Result<usize, CommitError>`:
  `validate_maintenance` (guard must be exclusive: add `RepositoryMaintenanceGuard::is_exclusive()`
  or a typed exclusive guard; check `maintenance.rs`), `ensure_layout_under_maintenance`,
  `acquire_lock` (exclusive accepted lock), head precondition (`read_head()`
  is `None` or `Some(expected_head)` with that receipt durable), decode every
  receipt (`import_transaction_receipt`), topological order by parent links
  (genesis first), for each: if durable exact, `verify_existing_receipt`;
  else verify against store exactly as `load_verified_revision` does but with
  the receipt bytes in hand (`verify_transaction_relationship` needs the
  parent receipt: parent must already be durable; `load_objects`,
  `verify_manifest_lengths`, `validate_inventory`), then `persist_receipt`.
  Return the durable count. Work ceilings from the exchange contract are
  enforced by the caller's preflight; the API applies the S20-390 receipt
  durability order only.
- `initialize_trusted_clone_head_with_maintenance(&self, guard, head) -> Result<AcceptedHead, CommitError>`:
  same locks; parent chain durable (walk `read_receipt_readonly` up to the
  genesis, bounded 4,096); `read_head()` `None` → `cas_head(None, head)`;
  `Some(head)` → `load_accepted(head)` reverify and return; other → `AlreadyInitialized`.
- Tests: guard on each path, two-phase install on an empty root, X-07 exact
  re-entry, foreign head rejection, parent-missing preserves `TXN_*`.

## Step 3: `sley-repo` (exchange owner)

- `crates/sley-repo/src/exchange.rs`, `lib.rs` gains `pub mod exchange;`
  (module graph now `exchange`, `gc`, `refs`; ADR-0024 covers the frozen
  S20-530 inventory at the accepted state only).
- Constants: tag 540, domain tag 19, kind 540, field schema hash
  `a843405b…`, decoder limits hash `808eaba9…`, leaf/node domains, sections
  1..4, limits (stored 67,108,864; pack 16,777,216; receipts 4,096; branches
  4,096; leaves 8,194; allocation 134,217,728; work ceilings).
- `exchange_epoch_record()` / `exchange_epoch_id()` / registry with a
  preserved `ExchangeEpoch1Decoder`, mirroring `pack_epoch_record`.
- Types: `ExchangeReceiptEntry`, `ExchangeHeadEntry`, `ExchangeBranchEntry`,
  `AcceptedRepositoryExchange`, `ExchangeImportReport`, `ExchangeError`
  (`EXCHANGE_*` codes 54000..54021 with numerics; upstream passthrough for
  `PACK_*`, `TXN_*`, `REF_*`, `BRANCH_*`, `STORE_*`, `SCB_*`).
- Export: `export_repository_exchange(root, verifier)`: shared maintenance;
  head via `accepted_head_with_maintenance`; `list_branches(MAX_BRANCHES)`;
  parent-link closure via `verified_revision_with_maintenance` bounded 4,096;
  roots → `export_conformance_pack`; entries sorted (receipts by id;
  branches by canonical element order: compare
  `uvar(len(encode_bytes(name)))||encode_bytes(name)`); tree; envelope.
- Import: `import_repository_exchange(target, bytes, verifier)`: preflight
  steps 1..7 (decode; canonical checks; pack preflight without writes: needs
  an S20-170 preflight-only entry point, add `preflight_conformance_pack`
  in `lib.rs` factoring `import_conformance_pack`'s first phase; receipt
  decode and closure rules; branch checks; receipt verification against the
  pack's objects in memory; target classification with symlink discipline);
  persistence 8.1..8.7 (marker temp+rename; layout; `initialize_repository_maintenance`;
  exclusive maintenance non-blocking; re-classify; pack import; clone
  receipts; branch install via `persist_no_overwrite` + `persist_expected_ref`
  under `acquire_refs_lock`; clone head; marker removal).
- Guard in `sley-repo`: `create_branch_with_maintenance_inner`,
  `advance_branch` inner, `recover_refs_with_maintenance`,
  `recover_gc_witness`, `acquire_exclusive_gc*` call
  `sley_txn::incomplete_clone_marker_present(root)` and return
  `BranchError::Transaction(CommitError::Transaction(IncompleteClone))`.
- Tests: envelope/payload/tree round trip; rejection matrix per code;
  closure rules; target rules; X-01..X-07 injected cuts (use the S20-530
  `#[cfg(test)]` cut pattern: a thread-local selected cut); clone-equivalent
  test (two branches, one advanced, one orphan origin, one unreachable
  receipt, one unreferenced object); re-export after X-07; canonical-order
  test with 127/128/253/254/255-byte names; guard tests; symlink tests;
  preflight-cost and maximal-decode tests.

## Step 4: conformance and oracle

- `conformance/repository-exchange/v1/accepted.json` + `SHA256SUMS` emitted
  by an ignored Rust test (same pattern as S20-390's emitter, with a
  frozen-grant fixture) and `scripts/generate_repository_exchange_fixtures.py --check`.
- `scripts/check_repository_exchange_vector.py` (independent Python: decode
  envelope, recompute leaves, tree root, `RepositoryExchangeId`), run under
  `uv run --project oracle/scb1`; Makefile `conformance` target line.
- `scripts/check_repository_exchange_spec.py`: add source markers and fixture
  digests; summary status → implementation complete only at closeout.

## Step 5: fuzz

- `fuzz/targets/repository_exchange_import.rs` persistent target over the
  preflight decoder; `scripts/check_repository_exchange_persistent_fuzz_slice.py`;
  Make target `repository-exchange-persistent-fuzz-smoke`.

## Step 6: closeout

- Frontier re-anchor (`s20_540_implementation_started` etc.), machine summary
  `repository_exchange` complete, `docs/audits/S20_540_REPOSITORY_EXCHANGE_CLOSEOUT.md`,
  Tier 2 record, Nabu/Ariadne/Vulcan implementation reviews.
