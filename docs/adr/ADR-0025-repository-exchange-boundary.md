# ADR-0025: Repository exchange composition and clone trust boundary

Status: accepted for the S20-540 contract draft; implementation pending
contract review

Date: 2026-09-03

## Context

S20-170 froze a root/object-only pack whose field schema requires empty `refs`
and `transaction_inventory` sets and whose conformance fixture, limits hash,
and decoder are bound by `scripts/check_repository_pack_spec.py`. S20-390 owns
receipts and the single fixed accepted head, and ADR-0021 forbids imported
receipt bytes from becoming commit authority. S20-500 owns immutable
branch-origin records and mutable refs, and ADR-0022 says an origin record
records exact ancestry facts inherited at creation that a replay cannot
reproduce. S20-530 froze the exclusive-recovery matrix over those layers.

S20-540 must reconstruct a clone-equivalent repository in a clean location
from one exchange artifact without weakening any of those boundaries. The
Nabu design consult (session
`forge-nabu-s20-540-design-20260903T013712-0fb98776`) recommended composition
over inline sections, a transaction-owner clone-install API, and byte-exact
branch installation, and raised concerns about nested byte ceilings,
`sley-txn` ownership in the work-package row, interrupted-clone retry, the
definition of an empty target, tombstone and surplus equality, and an
unreachable receipt ceiling.

## Decision

1. **Composition.** Repository Exchange v1 (`docs/spec/REPOSITORY_EXCHANGE_V1.md`,
   contract tag 540, domain `sley2.repository-exchange.v1`,
   `digest_domain_tag` 19) embeds one exact stored S20-170 pack and adds
   receipts, the accepted head, and visible branch pairs as new sections. The
   S20-170 decoder, limits hash, and fixture are untouched. An exchange never
   nests an exchange, and the embedded pack is capped at `33,554,432` bytes,
   strictly below the `67,108,864` outer ceiling, with one shared top-down
   allocation budget.
2. **Trust boundary.** Import targets only a fresh location or an incomplete
   clone of the same `RepositoryExchangeId`, proven by a stage marker.
   `EXCHANGE_TARGET_NOT_EMPTY` and `EXCHANGE_TARGET_INCOMPLETE_MISMATCH` fire
   before any write. The fixed accepted head is written last and is the sole
   completion witness; a target without it is not a repository to any reader.
   Imported bytes never become authority inside an existing repository.
3. **Transaction-owner API.** `sley-txn` gains a two-phase
   `initialize_trusted_clone` boundary (receipts, then head), the explicit
   root-of-trust analogue of `initialize_trusted_genesis`: exclusive
   maintenance, absent head, strict decoding and verification of every receipt
   against the durable store and its installed parents, S20-390 durability
   orders, no candidate or policy authority. The dependency direction stays
   `sley-repo -> sley-txn`. This is transaction-owner work inside the S20-540
   slice, exactly as S20-500 added the verified revision lookup; the
   work-package row names it.
4. **Byte-exact branch install.** `sley-repo` installs exported origin and ref
   records byte-exactly after full verification (fast-forward within the
   S20-500 ancestry ceiling, facts equal to the named receipts). Replay through
   `create_branch` and `advance_branch` is rejected because it cannot
   reproduce immutable origin records and would multiply invariants.
5. **Equivalence over the exported set.** Clone equivalence is head identity
   and bytes, every verified revision including tombstones, branch list and
   ancestry equality, absence of any surplus receipt, branch, object, marker,
   orphan origin, or temporary, and byte-identical re-export. Orphan origins
   and unreachable receipts are excluded from export, so equivalence is
   defined over the exported set.
6. **Interruption matrix.** Rows X-01 through X-07 freeze the retry behavior
   of an interrupted import; retrying identical bytes converges, retrying
   different bytes fails closed. S20-530 rows are unchanged because an
   incomplete clone is never accepted state.
7. **Limits.** `4,096` receipts and `4,096` branches are clone-profile
   limits consistent with the outer byte ceiling; deeper histories need a
   later streamed or compressed profile.

## Consequences

- S20-540 can be frozen, implemented, and reviewed without amending S20-170,
  S20-390, S20-500, or S20-530 contracts.
- `sley-txn` acquires one bounded write path that shares nothing with
  candidate commit and cannot be reached from it.
- A future streamed or compressed exchange profile is a new contract version
  with its own limits and decoder.

## Rejected alternatives

- Inline sections under one new decoder (would re-specify and re-fixture the
  S20-170 sections, duplicating a frozen contract).
- Replaying branch creation and advancement (cannot reproduce origin records;
  more invariants than byte-exact installation).
- Importing into an existing repository or merging exchanged history (an
  authority claim over accepted state; belongs to later merge and fetch
  packages).
- A single outer byte ceiling equal to the embedded pack's ceiling (a full
  S20-170 pack could never be exchanged with any receipt).
