# Repository Exchange v1

Status: S20-540 contract draft. Nabu design consult applied; Ariadne contract
review and Vulcan import-surface review pending; no implementation exists.

## Scope

Repository Exchange v1 is the host-independent, clone-equivalent exchange
profile for one complete local Sley 2 repository. It composes, and never
alters, three frozen contracts:

- the S20-170 Repository Pack v1 for schema epochs, roots, and objects;
- the S20-390 transaction receipt for every exported revision and the fixed
  `accepted` head;
- the S20-500 immutable branch-origin and mutable branch-ref records for every
  visible branch.

It contains no filesystem paths, Git facts, locks, GC witness state, staged
temporaries, orphan origin records, recovery reports, runtime metadata,
compressed blocks, or signatures. Merge, comparison, protocol, and streaming
remain outside this contract.

`sley-repo` owns the exchange. The dependency direction stays
`sley-repo -> sley-txn -> sley-store`. The ownership section names the one
bounded transaction-owner API that S20-540 implementation must add inside
`sley-txn`.

## Envelope and identity

```text
format_version    = 1
contract_tag      = 540
contract_domain   = "sley2.repository-exchange.v1"
digest_domain_tag = 19
kind_tag          = 540
```

The envelope is:

```text
exchange_preimage = "SLEYSCB1" || uvar(1) || uvar(540) ||
                    ExchangeSchemaEpochId[32] || len(payload) || payload
RepositoryExchangeId = BLAKE3-256("sley2.repository-exchange.v1" ||
                                  exchange_preimage)
stored_exchange = exchange_preimage || RepositoryExchangeId[32]
```

The digest trailer is outside its own preimage. The envelope epoch selects an
immutable registry row containing the exact tag-540 descriptor and preserved
decoder. The embedded pack, receipts, and branch records select their own
frozen decoders; they never select the exchange decoder.

`sley2.repository-exchange.v1` is a new `sley-id` domain. Adding its row to
`IDENTIFIERS_V1.md`, the `Domain` enumeration, and the frozen vectors is
identifier-owner work inside the S20-540 slice.

The conformance descriptor freezes raw BLAKE3-256 hashes of these exact ASCII
texts:

```text
field schema preimage = sley2.repository-exchange.v1.schema:required(1:exchange_version u32,2:object_pack bytes,3:receipts set receipt_entry,4:accepted_head head_entry,5:branches set branch_entry,6:compression_profile u32,7:digest_tree digest_tree,8:signature_metadata option_bytes);receipt_entry=record(1:transaction_id fixed32,2:receipt_id fixed32,3:stored_bytes bytes);head_entry=record(1:transaction_id fixed32,2:receipt_id fixed32);branch_entry=record(1:branch_name bytes,2:stored_origin bytes,3:stored_ref bytes);digest_tree=record(1:algorithm_tag u32,2:leaf_count u64,3:leaves list fixed32,4:root_digest fixed32);profile=0;signature=none;epoch=1
field_schema_hash = a843405be5e34d979bb4889b0e98c152dd01f86d9c614c1afda4cf88dd884e2c

decoder limits preimage = sley2.repository-exchange.v1.decoder-limits:stored=67108864,expanded=67108864,pack=33554432,receipts=4096,branches=4096,leaves=8194,allocation=134217728,compression=none
decoder_limits_hash = 570c8c8ab522778ad1bdf00e09845b743de391e42e556221cebfe656a7ab2255
```

## Payload

The payload is a closed SCB1 Record with all fields required:

| Tag | Field | S20-540 type and rule |
|---:|---|---|
| 1 | `exchange_version` | `UInt<32>`; exactly `1` |
| 2 | `object_pack` | `Bytes`; one exact stored S20-170 pack (tag 170, version 1); an exchange never nests an exchange |
| 3 | `receipts` | canonical set of receipt entries, sorted by raw `TransactionId` |
| 4 | `accepted_head` | head entry naming one receipt of tag 3 |
| 5 | `branches` | canonical set of branch entries, sorted by raw branch-name bytes |
| 6 | `compression_profile` | `UInt<32>`; exactly `0` (`none`) |
| 7 | `digest_tree` | digest-tree record below |
| 8 | `signature_metadata` | option bytes; MUST be absent |

A receipt entry carries the declared `TransactionId`, the declared `ReceiptId`,
and the exact stored S20-390 receipt bytes including the trailer. The stored
bytes MUST decode through the frozen receipt codec, the embedded transaction's
identity MUST equal field 1, and the trailer MUST equal field 2.

A head entry carries the exporter's fixed accepted head as a `TransactionId`
and its `ReceiptId`; both MUST match one receipt entry.

A branch entry carries the exact canonical branch name, the exact stored
S20-500 branch-origin record, and the exact stored S20-500 branch-ref record
for one visible branch. The name MUST satisfy the S20-500 grammar; the origin
and ref MUST decode through the frozen S20-500 codecs; both embedded names
MUST equal field 1; the ref's `branch_record_digest` MUST equal the origin's
digest. Orphan origin records (an origin without a visible ref) are never
exported and never imported.

Duplicate or noncanonical entries fail; a decoder never sorts input.

## Closure rules

1. **Ancestry closure.** Every `parent_transaction_ids` element of every
   receipt's transaction MUST name a receipt in the set. The parent graph MUST
   be acyclic and MUST contain exactly one trusted-genesis transaction
   (`transaction_kind = 1`, no parents).
2. **No surplus.** Every receipt MUST be the accepted head, a visible branch's
   current head or origin, or a transitive parent of one of those. Any other
   receipt is `EXCHANGE_ANCESTRY_SURPLUS`.
3. **Root closure.** The embedded pack's root set MUST equal exactly the
   dependency closure of the committed roots of all receipts. The pack's own
   S20-170 rules already require dependency and object closure within it.
4. **Branch closure.** For every branch entry, `origin_transaction_id` and
   `head_transaction_id` MUST name receipts in the set, the current head MUST
   be reachable from the origin by following parent links only (fast-forward)
   within 65,536 nodes, and every origin and current fact (workspace, roots,
   epoch, policy, dependency roots) MUST equal the facts derived from the
   named receipts exactly as S20-500 sections 4 and 5 require.
5. **Head closure.** The accepted head MUST be a receipt in the set. It is not
   required to be a descendant of every branch; branches may point anywhere in
   the exported ancestry.

## Digest tree

The tree algorithm tag is `1`, BLAKE3-256. One leaf exists for the embedded
pack, for every receipt entry, for every branch entry, and for the head, in
this exact order: pack, all receipts by `TransactionId`, all branches by name,
then the head. Section tags are `1`, `2`, `3`, and `4` respectively.

```text
leaf_preimage = "sley2.repository-exchange-leaf.v1" ||
                uvar(section_tag) || identifier[32] ||
                uvar(len(stored_bytes)) || stored_bytes
leaf = BLAKE3-256(leaf_preimage)

node = BLAKE3-256("sley2.repository-exchange-node.v1" || left[32] || right[32])
```

The leaf identifier and stored bytes per section are:

| Section | Identifier | Stored bytes |
|---:|---|---|
| 1 | `RepositoryPackId` of the embedded pack | the exact stored pack |
| 2 | `TransactionId` | the exact stored receipt |
| 3 | S20-500 `name_key` of the branch name | `encode_bytes(stored_origin) \|\| encode_bytes(stored_ref)` |
| 4 | `TransactionId` of the accepted head | the exact 73-byte S20-390 `stored_head` value for that head |

The payload stores the exact ordered leaf list, its count, and the tree root.
At each level adjacent digests are paired; an unpaired final digest is promoted
unchanged. Any leaf-list or root disagreement is
`EXCHANGE_DIGEST_TREE_MISMATCH`. The outer `RepositoryExchangeId` additionally
binds the header, the compression profile, the tree, and signature absence.

## Resource limits

All SCB1 epoch-1 canonical limits still apply, so the stored exchange is one
SCB1 record bounded by the epoch-1 ceiling. This contract adds closed exchange
limits:

- stored exchange bytes: `67,108,864`;
- expanded bytes across the embedded pack, receipts, origins, and refs:
  `67,108,864`;
- embedded pack bytes: `33,554,432`, an exchange-profile cap strictly below
  the outer ceiling; every inner S20-170 limit still applies unchanged;
- receipts: `4,096`;
- branches: `4,096` visible `(origin, ref)` pairs;
- digest leaves: `8,194` (the exact sum `1 + 4,096 + 4,096 + 1`);
- fast-forward walk per branch: `65,536` nodes (the S20-500 ancestry ceiling);
- decoder allocation budget: `134,217,728` bytes, one top-down budget shared
  by the exchange decoder and the embedded pack decoder.

The receipt ceiling is a clone-profile limit, not an ancestry limit: a
repository whose reachable history exceeds `4,096` receipts is not exportable
by this profile and fails `EXCHANGE_RESOURCE_LIMIT` at export. Deep histories
require a later streamed or compressed profile, which is explicitly outside
v1.

Profile `0` performs no decompression. Any other profile returns
`EXCHANGE_COMPRESSION_UNSUPPORTED` before interpreting blocks.

## Export

Export runs under shared repository maintenance and reads only durable
verified facts:

1. resolve the fixed accepted head through `sley-txn`;
2. enumerate visible branches through `sley-repo` (`list_branches` up to the
   S20-500 ceiling); orphan origins are excluded;
3. collect the receipt set as the parent-link closure of the accepted head and
   of every branch's current head, loading each revision through the verified
   revision lookup; exceeding `4,096` receipts is `EXCHANGE_RESOURCE_LIMIT`;
4. export the S20-170 pack over the committed roots of that set;
5. assemble sorted entries, the tree, and the envelope.

Export order has no byte effect. Two exports of the same head, branch set,
and durable bytes yield identical exchange bytes and the same
`RepositoryExchangeId`.

## Import target and phases

The import target is a path. It MUST satisfy exactly one of:

- it does not exist, or is an empty directory (a **fresh target**);
- it is an **incomplete clone** of this same `RepositoryExchangeId`: the
  stage marker `exchange/v1/<RepositoryExchangeId hex>.stage` exists and the
  fixed accepted head is absent or present without the marker removed.

Any other target, including any repository with an accepted head and no
marker, any directory with other entries, and an incomplete clone whose marker
names a different `RepositoryExchangeId`, fails before any write:
`EXCHANGE_TARGET_NOT_EMPTY` for the former cases and
`EXCHANGE_TARGET_INCOMPLETE_MISMATCH` for a marker mismatch. Imported bytes
therefore never become authority inside an existing repository.

Import is split into preflight and persistence:

1. bound stored bytes, decode the closed envelope and payload, verify the
   exchange trailer;
2. verify canonical order, counts, closed profiles, every declared identity,
   and the complete digest tree;
3. run the complete S20-170 preflight (its steps 1 through 5) over the
   embedded pack without store writes;
4. decode every receipt through the frozen codec and prove ancestry closure,
   acyclicity, single genesis, head membership, and no surplus;
5. decode every branch record and ref and prove branch closure;
6. verify every receipt against the embedded pack's objects exactly as the
   verified revision lookup would (roots, policy, manifest lengths, object
   closure, tombstones) without writes;
7. only after all preflight checks pass, persist in this exact order:
   1. create the repository layout, then write and fsync the stage marker;
   2. import the embedded pack (S20-170 step 6; idempotent for present
      objects);
   3. install every receipt through `initialize_trusted_clone_receipts`
      (the receipt phase of the transaction-owner API in the ownership
      section); no head is written;
   4. install every branch origin and ref byte-exactly through `sley-repo`
      (origin, fsync, ref, fsync; an exact existing record is reverified and
      resynced, any other existing record is `EXCHANGE_IO`);
   5. write the fixed accepted head through `initialize_trusted_clone_head`,
      last;
   6. remove the stage marker and fsync its directory;
8. return the reconstructed accepted head, the receipt count, the branch
   count, and the promoted and present object counts.

The head is the completion witness: a target without a head is not a
repository to any reader, and only the importer may write into it. Every
step before the head is idempotent over exact bytes, so retry converges.

## Interruption and retry

An interrupted import leaves an incomplete clone. Retrying the import of the
identical exchange bytes converges to the complete clone: every persisted
artifact is content-addressed or exact, so each step reverifies existing bytes
and continues. Retrying with different exchange bytes fails
`EXCHANGE_TARGET_INCOMPLETE_MISMATCH`. The frozen interruption rows are:

| Row | Interruption point | Retry result |
|---|---|---|
| X-01 | before the stage marker is durable | target is fresh; import restarts |
| X-02 | after the marker, before any object | objects, receipts, branches, head absent; import continues |
| X-03 | during object promotion | S20-170 idempotent re-import; unreachable staged objects are S20-530 owner cleanup |
| X-04 | during receipt installation | exact existing receipts reverified; missing receipts installed |
| X-05 | during branch installation | exact existing origins and refs reverified; missing ones installed |
| X-06 | before the head rename is durable | head absent; branches complete; head installed on retry |
| X-07 | after the head, before marker removal | complete clone with marker; retry reverifies everything and removes the marker |

`sley-txn` and `sley-repo` recovery on an incomplete clone fail closed exactly
as they do today for an absent head; they never complete an import. Any
interruption row not listed above is an implementation defect.

## Clone equivalence

Export from a source `S`; import into a fresh target `T`. The clone-equivalent
test proves all of:

1. `T`'s fixed accepted head equals `S`'s by `TransactionId`, `ReceiptId`, and
   exact stored receipt bytes;
2. for every exported receipt, `T`'s verified revision equals `S`'s in exact
   transaction and receipt bytes, state root, policy root, object set, and
   tombstone set;
3. `T`'s visible branch list equals `S`'s exported branch list in names,
   origin-record digests, ref-record digests, and current targets, and
   `branch_ancestry` is equal for every branch;
4. `T`'s receipt set, branch set, and object store contain nothing beyond the
   exported set, the embedded pack's objects, and the repository layout;
5. re-exporting from `T` yields byte-identical exchange bytes and the same
   `RepositoryExchangeId`; export order has no byte effect;
6. `T` contains no stage marker, no orphan origin, and no staged temporary.

Equivalence is defined over the exported set: a source that carries orphan
origins or unreachable receipts exports the same bytes as one that does not.

## Stable failures

Numeric codes `54000` through `54019` are exact:

| Numeric | Symbolic |
|---:|---|
| 54000 | `EXCHANGE_VERSION_UNSUPPORTED` |
| 54001 | `EXCHANGE_DIGEST_MISMATCH` |
| 54002 | `EXCHANGE_DIGEST_TREE_MISMATCH` |
| 54003 | `EXCHANGE_CANONICAL_ORDER` |
| 54004 | `EXCHANGE_DUPLICATE_ENTRY` |
| 54005 | `EXCHANGE_PACK_INVALID` |
| 54006 | `EXCHANGE_RECEIPT_INVALID` |
| 54007 | `EXCHANGE_ANCESTRY_OPEN` |
| 54008 | `EXCHANGE_ANCESTRY_CYCLE` |
| 54009 | `EXCHANGE_ANCESTRY_SURPLUS` |
| 54010 | `EXCHANGE_HEAD_INVALID` |
| 54011 | `EXCHANGE_BRANCH_INVALID` |
| 54012 | `EXCHANGE_BRANCH_NOT_FAST_FORWARD` |
| 54013 | `EXCHANGE_TARGET_NOT_EMPTY` |
| 54014 | `EXCHANGE_TARGET_INCOMPLETE_MISMATCH` |
| 54015 | `EXCHANGE_RESOURCE_LIMIT` |
| 54016 | `EXCHANGE_COMPRESSION_UNSUPPORTED` |
| 54017 | `EXCHANGE_PROFILE_UNSUPPORTED` |
| 54018 | `EXCHANGE_IO` |
| 54019 | `EXCHANGE_INTERNAL_INVARIANT` |

Strict lower-layer failures retain their exact owning code and numeric value:
`SCB_*`, `PACK_*`, `STORE_*`, `TXN_*`, `REF_*`, and `BRANCH_*` are preserved,
never remapped. `EXCHANGE_PROFILE_UNSUPPORTED` rejects signature presence and
any nested exchange; `EXCHANGE_PACK_INVALID` wraps no code and is returned
only when the embedded bytes are not a tag-170 version-1 pack at all.

## Ownership and the transaction-owner API

`sley-repo` owns `exchange.rs`: export, preflight, the stage marker, byte-exact
branch installation, and the clone-equivalent test. It may depend on
`sley-txn`; `sley-txn` remains independent of `sley-repo`.

S20-540 implementation must add one bounded API to `sley-txn`, exactly as
S20-500 added the verified revision lookup:

```text
initialize_trusted_clone_receipts(&self, receipts: &[&[u8]]) -> Result<usize, CommitError>
initialize_trusted_clone_head(&self, head: TransactionId) -> Result<AcceptedHead, CommitError>
```

Both run under exclusive repository maintenance and require an absent fixed
head. The receipt phase strictly decodes every stored receipt, verifies it
against the durable object store (roots, policy, manifest lengths, object
closure, tombstones) and against its already-installed parents, and installs
it under its `TransactionId` with the S20-390 receipt durability order; an
exact existing receipt is reverified and resynced. The head phase requires
every parent-chain receipt of `head` to be durable and writes the fixed head
with the S20-390 head durability order. Neither phase accepts a candidate, a
policy transition, or any authority claim beyond the receipt bytes and one
`TransactionId`. The API is not callable through ordinary candidate commit.

## Required evidence

Implementation acceptance requires at least:

- exact envelope, payload, and digest-tree round trips with an independent
  Python reproduction of `RepositoryExchangeId` and the tree root over a fixed
  conformance exchange;
- field-perturbation and section-perturbation rejection matrices for every
  code in the table;
- ancestry closure, surplus, cycle, multi-genesis, head-membership, and
  branch fast-forward rejection tests;
- nested-exchange, signature-presence, nonzero-profile, oversize-pack, and
  each resource-limit rejection before any write;
- fresh-target, non-empty-target, incomplete-clone retry, and mismatched
  marker tests;
- the complete X-01 through X-07 interruption matrix with injected cuts;
- the clone-equivalent test over a source with at least two branches, one
  advanced branch, one orphan origin, and one unreachable receipt;
- a structural dependency check proving no `sley-txn -> sley-repo` edge;
- an exchange-import persistent libFuzzer target (S20-700 adjacent surface);
- Tier 1 plus repository-focused Tier 2 validation;
- Ariadne contract review, Nabu design review, and Vulcan import-surface
  review with every report-grade finding closed.

## Explicit exclusions

This contract does not claim:

- compression, streaming, signatures, or histories beyond `4,096` receipts;
- import into an existing repository, merge, fetch, or partial exchange;
- branch deletion, rename, force movement, tags, or symbolic refs;
- semantic comparison, conflict objects, or any S20-510/S20-520 semantics;
- SMP1, JSON bridge, CLI, runtime, benchmark, packaging, release, or GA.
