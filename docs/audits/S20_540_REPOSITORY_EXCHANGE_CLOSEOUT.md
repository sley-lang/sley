# S20-540 Repository Exchange Closeout

Status: **S20-540 clone-equivalent repository exchange implemented under the frozen v1 contract; implementation reviews recorded below; the Sley 2 goal remains incomplete**

Date: 2026-09-03

Validation tier: **Tier 1 plus repository-focused Tier 2 handoff**

## Closed claim

This package closes the S20-540 pack-exchange boundary: one exact exchange
artifact reconstructs a clone-equivalent repository in a fresh location from
the frozen S20-170 pack, the S20-390 receipts of the exported ancestry, the
fixed accepted head, and every visible S20-500 branch as its exact origin and
ref pair. The frozen contract is `docs/spec/REPOSITORY_EXCHANGE_V1.md`
(revision 7, contract tag 540, domain `sley2.repository-exchange.v1`, digest
domain tag 19, field schema hash `a843405b…`, decoder limits hash
`808eaba9…`) with ADR-0025.

The implementation provides:

- `sley-id`: the thirtieth frozen domain and `RepositoryExchangeId`;
- `sley-txn`: `TXN_INCOMPLETE_CLONE` (`39022`, appended to the frozen
  S20-390 table under ADR-0025), the marked-root predicate that fails
  genesis, commit, and recovery closed on an incomplete clone, the two-phase
  `initialize_trusted_clone_receipts_with_maintenance` and
  `initialize_trusted_clone_head_with_maintenance` API under the caller's
  exclusive maintenance guard and the exclusive accepted lock, the pure
  `verify_receipt_against_objects` preflight, and a non-blocking exclusive
  maintenance acquisition;
- `sley-repo`: `exchange.rs` with export over the accepted head and visible
  branches, preflight enforcing ancestry closure and shape, no surplus, root
  closure, branch closure with fast-forward reachability, head closure, and
  workspace uniformity, receipt verification against the embedded pack's
  objects under the closed work ceilings, target classification with symlink
  discipline and the receipt, origin, ref, and head subset proofs, and
  persistence 8.1 to 8.7 with the stage marker written first by
  temp-and-rename, exclusive maintenance ownership, owned re-classification,
  pack promotion, the receipt phase, byte-exact branch installation under the
  refs lock, the head phase last, and marker removal; the incomplete-clone
  guard on branch create, advance, ref recovery, GC-witness recovery, and
  exclusive GC acquisition; and a preflight-only S20-170 entry point.

The dependency direction stays `sley-repo -> sley-txn -> sley-store`.

## Evidence

- Contract freeze: revision 6 at `58dc18c` after five Ariadne passes
  (final pass `forge-ariadne-s20-540-pass5-20260903T023812-51133a3e`,
  `PASS_CONTRACT_DRAFT`) and two Vulcan import-surface passes (final
  `forge-vulcan-s20-540-rereview-20260903T022707-896ec8f9`,
  `PASS_CONTRACT_DRAFT`); revision 7 corrected the branch-order framing
  paragraph without any preimage change and was confirmed by
  `forge-ariadne-s20-540-pass6-20260903T030956-34fa856b`.
- Implementation commits: `5076791` (sley-id), `43421d0` (sley-txn),
  `4d20839` (exchange module), `2dd8173` (conformance fixture and oracle),
  `f40f6cc` (interruption rows and decode limits), `08dc0dc` and `233398e`
  (persistent fuzz slice).
- Conformance: `conformance/repository-exchange/v1/accepted.json` (one
  clone-equivalent source exchange: two receipts, two visible branches, 7,754
  stored bytes, six leaves) and `rejected.json` (five rejection inputs with
  the codes the importer returns before any write), drift-gated by
  `scripts/generate_repository_exchange_fixtures.py --check` in `make quick`;
  `scripts/check_repository_exchange_vector.py` reproduces the trailer, the
  embedded pack identity, every leaf, the tree root, and both canonical
  orders independently in Python under the frozen oracle environment.
- Native tests: ten `sley-repo` exchange tests (clone-equivalent round trip
  with byte-identical re-export, X-02 and X-07 retries, the full X-01 to
  X-07 injected-cut matrix with retry convergence, the target matrix, the
  write-guard matrix, the rejection matrix, decode limits with a maximal
  shape, fixed-head bytes, code contiguity, canonical order over 1, 2, 127,
  128, 253, 254, and 255 byte names) plus five `sley-txn` clone and guard
  tests; `sley-repo` 313 tests, `sley-txn` 200 tests, `sley-store` 48 tests
  pass.
- Persistent fuzz: `fuzz/targets/repository_exchange_importer.rs`, smoke
  `PASS` over a 518-seed corpus (`docs/audits/S20_700_EXCHANGE_IMPORT_PERSISTENT_SLICE.md`).
- Tier 1: `make quick` green at every commit of the slice.
- Tier 2: see the validation record below.

## Findings closed in flight

- S20-170's exact object-inventory rule requires the state roots'
  contract-root and test-root objects to exist in the store; a repository
  built with placeholder anchors cannot be exported (`STORE_OBJECT_NOT_FOUND`).
  The fixture uses real objects; production repositories must too.
- The revision-3 branch-order wording assumed a double length prefix on a
  `Bytes` record field. The frozen SCB1 encoder, the S20-170 codec, and the
  independent oracle frame it once, so the order is plain length-then-bytes
  for every legal name length; the contract was corrected (revision 7).
- Deterministic fixtures export identical bytes; tests that need a second,
  different exchange use a different commit nonce.

## Explicitly open and deferred

- The preflight-cost test at a semantically maximal exchange (4,096 real
  receipts) is not built; the decode-level maximal shape and every
  count and byte ceiling are tested, and the work ceilings are enforced as
  single preflight counters.
- Strict pedantic clippy carries pre-existing debt in S20-390 and S20-530
  production functions (`too_many_lines`, `must_use_candidate`); the new
  S20-540 code paths lint clean under `--no-deps`; a hygiene slice is
  deferred.
- The three frozen S20-530 mapped-test modules keep their rustc and clippy
  allow attributes (ADR-0024).
- Merge, comparison, protocol, compression, streaming, signatures, histories
  beyond 4,096 receipts, more than one workspace, and import into an existing
  repository remain outside v1.

## Validation record

Tier 1 `make quick` passed at every commit of the slice (about 18 to 40
seconds each, zero warnings). Tier 2 ran on 2026-09-03T03:19Z at the
persistent-fuzz commit: `make core`, `make conformance` (including the new
exchange oracle line), `make adversarial`, `make fuzz-smoke`, and
`make exchange-persistent-fuzz-smoke` all exited 0 in 27 seconds. The full
`make v1` gate was skipped because S20-540 is a subsystem handoff, not a
release boundary; `make v2` and `make release-check` remain intentionally
fail closed.

## Independent review

Recorded below as the bounded Nabu, Ariadne, and Vulcan implementation
reviews complete.
