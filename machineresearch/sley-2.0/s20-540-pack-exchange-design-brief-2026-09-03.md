# S20-540 pack exchange: design brief for the contract freeze (2026-09-03)

Status: DESIGN BRIEF for the Nabu architecture consult; no contract, ADR,
checker, or code exists yet for S20-540

Owner: Merlin (implementation); Ariadne (contract); Vulcan (import surface);
Claude orchestrator (integration)

## Package facts (from `docs/WORK_PACKAGES.md`)

- Dependencies: S20-170 repository pack (complete, uncompressed
  root/object-only profile) and S20-500 native refs and branches (complete).
- Owned paths: `sley-repo`, `sley-conformance`.
- Contract: pack exchange. Primary risk: clone divergence.
- Acceptance: a clean import reconstructs root, refs, and ancestry per
  profile. Focused gate: the clone-equivalent test. Release implication: M4
  exit.
- S20-170 says: "S20-540 later owns a new profile or contract for
  transactions, refs, branch heads, ancestry, and clone-equivalent repository
  reconstruction." S20-500 says: "S20-540 owns transaction/ref pack sections
  and clone-equivalent exchange. S20-170 packs continue to reject refs and
  transactions."

## What a repository is, for clone purposes

A Sley 2 repository directory today holds, on the one-way dependency
`sley-repo -> sley-txn -> sley-store`:

| Layer | Durable facts | Owner |
|---|---|---|
| object store | immutable `objects/scb1/xx/yy/<id>.scb1` objects | sley-store |
| transactions | receipts (nine-field records embedding the transaction, candidate, result, state root, policy root, and object manifest), the fixed `heads/accepted` slot, `locks/maintenance.lock` | sley-txn |
| refs | immutable branch-origin records and mutable branch-ref records under `branches/v1`, name-keyed fan-out | sley-repo |
| GC | `locks/gc.lock` witness, retention snapshots | sley-repo |

Not part of a clone: locks, GC witness, staged temporaries, recovery
reports, orphan origin records, maintenance state, host paths, Git facts.

## Proposed contract shape: Repository Exchange v1 (contract tag 540)

Compose, do not extend. The S20-170 pack contract is frozen with empty
`refs` and `transaction_inventory` sets in its field-schema hash; a new
contract embeds an exact S20-170 pack and adds the missing sections.

```text
format_version    = 1
contract_tag      = 540
contract_domain   = "sley2.repository-exchange.v1"
digest_domain_tag = (next free sley-id domain tag)
kind_tag          = 540

exchange_preimage = "SLEYSCB1" || uvar(1) || uvar(540) ||
                    ExchangeSchemaEpochId[32] || len(payload) || payload
RepositoryExchangeId = BLAKE3-256("sley2.repository-exchange.v1" || exchange_preimage)
stored_exchange = exchange_preimage || RepositoryExchangeId[32]
```

Payload, a closed SCB1 record with all fields required:

| Tag | Field | Rule |
|---:|---|---|
| 1 | `exchange_version` | exactly `1` |
| 2 | `object_pack` | exact stored S20-170 pack bytes; its roots MUST equal the closure of every committed root, parent root, and dependency root of the exported receipts |
| 3 | `receipts` | canonical set of `{receipt_id fixed32, stored_receipt bytes}` sorted by `ReceiptId`; every receipt's transaction ancestry MUST be closed inside the set |
| 4 | `accepted_head` | `{transaction_id fixed32, receipt_id fixed32}`; MUST name a receipt in tag 3 |
| 5 | `branches` | canonical set of `{branch_name bytes, stored_branch_record bytes, stored_branch_ref bytes}` sorted by raw name; origin and current targets MUST be receipts in tag 3 and current MUST be a fast-forward descendant of origin within the ancestry limit |
| 6 | `compression_profile` | exactly `0` |
| 7 | `digest_tree` | one leaf per receipt, branch, and the head, in that order, with new section tags; same leaf/node construction as S20-170 under a new leaf domain |
| 8 | `signature_metadata` | MUST be absent |

Open question for the consult: whether tag 2 should embed the S20-170 pack
bytes (composition, exact reuse of the frozen importer) or inline the three
S20-170 sections under the new contract (one decoder, no nested envelope).
The brief recommends composition: the S20-170 importer, limits, and
conformance fixture stay authoritative and untouched.

## Clone equivalence (the focused gate)

Export from source `S`, import into an empty target `T`. Then:

1. `T.accepted_head()` equals `S.accepted_head()` by receipt identity and
   exact stored bytes.
2. For every exported receipt, `T.verified_revision(id)` returns the same
   exact transaction and receipt bytes, root, policy, objects, and
   tombstones as `S`.
3. `T.list_branches()` equals `S.list_branches()` in names, origin-record
   digests, ref-record digests, and current targets; `branch_ancestry` is
   equal for every branch.
4. Re-exporting from `T` yields byte-identical exchange bytes and the same
   `RepositoryExchangeId` (round trip); export order has no byte effect.
5. Every object reachable from the exported roots is present and verified in
   `T`'s store; nothing else was written.

## Trust and durability model

- Import requires an empty target: no accepted head, no receipts, no
  branches, no objects beyond what the embedded pack promotes. A target with
  an accepted head fails `EXCHANGE_TARGET_NOT_EMPTY` before any write. This
  keeps the S20-500 rule that imported record bytes never become authority
  inside an existing repository.
- Preflight verifies everything without writes: envelope and trailer, the
  embedded pack through the frozen S20-170 preflight, every receipt through
  the S20-390 receipt codec and closure rules against the pack's objects,
  ancestry closure and acyclicity, the head membership, every branch record
  and ref (digests, workspace, origin and current verification,
  fast-forward), and the digest tree.
- Persistence order: objects (S20-170 import), then receipts, then branch
  records and refs, then the accepted head last. The head is the only fact
  that makes the target a repository; an interruption before it leaves a
  directory that every reader rejects and that the operator discards and
  re-imports. No new S20-530 matrix rows are needed because there is no
  partially accepted state: the clone either has its head or is not a
  repository.
- The accepted head and receipts are installed through one new
  `sley-txn`-owned API (working name `initialize_trusted_clone`) that takes
  verified receipts and the head under exclusive maintenance; `sley-repo`
  installs its own verified branch records byte-exactly (it owns those
  records) rather than replaying `create_branch` and `advance_branch`.
  Dependency direction stays `sley-repo -> sley-txn`.

## Limits (proposal)

| Limit | Value | Source |
|---|---:|---|
| stored exchange bytes | 67,108,864 | matches S20-170 and SCB1 ceilings |
| receipts | 65,536 | S20-500 ancestry ceiling |
| branches | 4,096 | S20-500 visible-branch ceiling |
| embedded pack | S20-170 limits apply unchanged | S20-170 |
| digest leaves | 65,536 + 4,096 + 1 | exact sum |

## Failure range (proposal)

Numeric codes 54000 through 54020: `EXCHANGE_VERSION_UNSUPPORTED`,
`EXCHANGE_DIGEST_MISMATCH`, `EXCHANGE_DIGEST_TREE_MISMATCH`,
`EXCHANGE_CANONICAL_ORDER`, `EXCHANGE_DUPLICATE_ENTRY`,
`EXCHANGE_PACK_INVALID`, `EXCHANGE_RECEIPT_INVALID`,
`EXCHANGE_ANCESTRY_OPEN`, `EXCHANGE_ANCESTRY_CYCLE`,
`EXCHANGE_HEAD_MISSING`, `EXCHANGE_BRANCH_INVALID`,
`EXCHANGE_BRANCH_NOT_FAST_FORWARD`, `EXCHANGE_TARGET_NOT_EMPTY`,
`EXCHANGE_RESOURCE_LIMIT`, `EXCHANGE_COMPRESSION_UNSUPPORTED`,
`EXCHANGE_PROFILE_UNSUPPORTED`, `EXCHANGE_IO`,
`EXCHANGE_INTERNAL_INVARIANT`. Lower-layer failures keep their exact codes.

## Touchpoints

- `docs/spec/REPOSITORY_EXCHANGE_V1.md` (new contract), ADR-0025 (exchange
  ownership and trust boundary), `scripts/check_repository_exchange_spec.py`
  in `make quick`.
- `crates/sley-id`: new domain row and vectors (`IDENTIFIERS_V1.md`
  table, machine-summary domain count).
- `crates/sley-txn`: `initialize_trusted_clone` (new write path, receipts
  then head, under exclusive maintenance).
- `crates/sley-repo`: `exchange.rs` (export, preflight, import), branch
  install, `lib.rs` module graph gains `exchange`.
- `crates/sley-conformance` or `conformance/repository-exchange/v1/`: fixed
  exchange fixture, independent Python oracle reproduction of
  `RepositoryExchangeId` and the tree root, clone-equivalent test.
- `fuzz/`: an exchange-import persistent target (S20-700 adjacent surface).

## Questions for the Nabu consult

1. Composition (embedded exact S20-170 pack) versus inline sections.
2. Whether `initialize_trusted_clone` belongs in `sley-txn` as proposed, and
   whether the empty-target rule is the right trust boundary.
3. Byte-exact branch install versus replaying create/advance.
4. Whether the equivalence definition above is complete for "clone
   equivalence per profile", and what the M4 exit needs beyond it.
5. Limits and whether the exchange needs its own larger byte ceiling.
