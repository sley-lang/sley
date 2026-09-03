# Repository Exchange v1

Status: S20-540 contract frozen at revision 6 (2026-09-03); revision 7 corrects the
branch-order framing sentence to the realized single framing (see ADR-0025
decision 10) and changes no preimage. Nabu design
consult applied; Ariadne contract review `PASS_CONTRACT_DRAFT` on revision 6
(session `forge-ariadne-s20-540-pass5-20260903T023812-51133a3e`) after five
passes; Vulcan import-surface review `PASS_CONTRACT_DRAFT` on revision 4
(session `forge-vulcan-s20-540-rereview-20260903T022707-896ec8f9`) with its
text notes applied in revision 5. Implementation state is tracked separately
in the machine summary; no implementation exists at the freeze.

## Notation

This document uses the SCB1 notation: `||` is byte concatenation, `uvar(x)`
is the unsigned varint of `x`, `len(x)` is `uvar(byte_length(x))`,
`encode_bytes(x)` is the SCB1 `Bytes` encoding `len(x) || x`, and
`BLAKE3-256` is the 32-byte BLAKE3 digest. All byte comparisons are unsigned
lexicographic comparisons.

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

The exchange schema epoch is a standalone epoch-1 record with exactly one
contract descriptor: `contract_tag = 540`, `digest_domain_tag = 19`,
`kind_tag = 540`, `required_fields = {1, 2, 3, 4, 5, 6, 7, 8}`,
`optional_fields = {}`, `variant_tags = {}` (the `Option` union tags of field
8 are SCB1 type tags, not contract variants, exactly as the frozen S20-170
descriptor treats its own `signature_metadata` field), and the two frozen
hashes below. The descriptor freezes raw BLAKE3-256 hashes of these exact
ASCII texts:

```text
field schema preimage = sley2.repository-exchange.v1.schema:required(1:exchange_version u32,2:object_pack bytes,3:receipts set receipt_entry,4:accepted_head head_entry,5:branches set branch_entry,6:compression_profile u32,7:digest_tree digest_tree,8:signature_metadata option_bytes);receipt_entry=record(1:transaction_id fixed32,2:receipt_id fixed32,3:stored_bytes bytes);head_entry=record(1:transaction_id fixed32,2:receipt_id fixed32);branch_entry=record(1:branch_name bytes,2:stored_origin bytes,3:stored_ref bytes);digest_tree=record(1:algorithm_tag u32,2:leaf_count u64,3:leaves list fixed32,4:root_digest fixed32);profile=0;signature=none;epoch=1
field_schema_hash = a843405be5e34d979bb4889b0e98c152dd01f86d9c614c1afda4cf88dd884e2c

decoder limits preimage = sley2.repository-exchange.v1.decoder-limits:stored=67108864,expanded=reserved,pack=16777216,receipts=4096,branches=4096,leaves=8194,allocation=134217728,compression=none
decoder_limits_hash = 808eaba936f09b2a938306c538e0dff636a1a6d2617ed0b4298d929891db9d09
```

## Payload

The payload is a closed SCB1 Record with all fields required:

| Tag | Field | S20-540 type and rule |
|---:|---|---|
| 1 | `exchange_version` | `UInt<32>`; exactly `1` |
| 2 | `object_pack` | `Bytes`; one exact stored S20-170 pack (tag 170, version 1) of at most `16,777,216` bytes; an exchange never nests an exchange, and a nested exchange is `EXCHANGE_PACK_INVALID` |
| 3 | `receipts` | `CanonicalSet` of receipt entries; because a receipt entry begins with `transaction_id fixed32`, canonical-set order equals raw `TransactionId` order |
| 4 | `accepted_head` | head entry naming one receipt of tag 3 |
| 5 | `branches` | `CanonicalSet` of branch entries in strictly increasing SCB1 canonical-set order over the complete `branch_entry` encoding excluding the element length prefix (the exclusion SCB1 states for map keys and the frozen S20-170 `object_entry` set order applies to set elements); see the ordering rule below |
| 6 | `compression_profile` | `UInt<32>`; exactly `0` (`none`) |
| 7 | `digest_tree` | digest-tree record below |
| 8 | `signature_metadata` | `Option<Bytes>`; MUST be present as `Option` union tag `0` (absent value), never omitted and never tag `1` |

A `Bytes`-typed record field is framed exactly once: the frozen SCB1
encoder (`encode_record` with `encode_sized`), decoder, and the independent
S20-170 Python oracle all realize SCB1 section 4 so that the field's length
prefix is the `Bytes` length, `uvar(tag) || len(bytes) || bytes`, as the
frozen S20-170 fixture proves for its `stored_bytes` fields. `branch_entry`
field 1 therefore encodes as `uvar(1) || len(branch_name) || branch_name`,
and the branch order is by the varint bytes of `byte_length(branch_name)`,
then by the raw name bytes. For every legal name length (1 through 255) this
is length-then-bytes order: one-byte varints (1 through 127) precede
two-byte varints (128 through 255), whose first byte exceeds `0x7f`. The
names `b` and `aa` sort `b` first. This is deliberately not the raw-name
order of `list_branches` (S20-500 section 8.3); exporters MUST re-sort into
this order and MUST NOT sort by raw name bytes alone.

A receipt entry carries the declared `TransactionId`, the declared `ReceiptId`,
and the exact stored S20-390 receipt bytes including the trailer. The stored
bytes MUST decode through the frozen receipt codec, the embedded transaction's
identity MUST equal field 1, and the trailer MUST equal field 2.

A head entry carries the exporter's fixed accepted head as a `TransactionId`
and its `ReceiptId`; both MUST match one receipt entry.

A branch entry carries the exact canonical branch name, the exact stored
S20-500 branch-origin record, and the exact stored S20-500 branch-ref record
for one visible branch. The name MUST satisfy the S20-500 grammar; the origin
and ref MUST decode through the frozen S20-500 codecs; the `branch_name`
field inside both embedded records MUST equal field 1; the ref's
`branch_record_digest` MUST equal the origin's digest. Orphan origin records
(an origin without a visible ref) are never exported and never imported.

Duplicate or noncanonical entries fail `EXCHANGE_DUPLICATE_ENTRY` or
`EXCHANGE_CANONICAL_ORDER`; a decoder never sorts input.

## Closure rules

Every rule is decided at preflight from the exchange bytes alone, without
writes.

1. **Ancestry closure and shape.** Every `parent_transaction_ids` element of
   every receipt's transaction MUST name a receipt in the set, and the parent
   graph MUST be acyclic. Exactly one receipt's transaction has
   `transaction_kind = 1` and no parents; every other receipt's transaction
   has `transaction_kind = 2` with exactly one parent and one aligned parent
   root, as `TRANSACTION_MODEL_V1.md` requires. Zero or multiple genesis
   transactions, or any other kind or parent shape, is
   `EXCHANGE_ANCESTRY_OPEN`; a cycle is `EXCHANGE_ANCESTRY_CYCLE`.
2. **No surplus.** Every receipt MUST be the accepted head, a visible branch's
   current head or origin, or a transitive parent of one of those. Any other
   receipt is `EXCHANGE_ANCESTRY_SURPLUS`.
3. **Root closure.** The embedded pack's root set MUST equal exactly the
   `committed_root` values of all receipts together with every root those
   roots depend on under S20-170 dependency closure, which is the smallest
   superset closed under the `dependency_roots` field of each S20-160 state
   root record. Policy roots travel only inside receipt bytes (receipt field
   7) and are neither pack roots nor pack objects. Any inequality is
   `EXCHANGE_ROOT_CLOSURE`. The pack's own S20-170 rules already require
   dependency and exact object-inventory closure within it.
4. **Branch closure.** For every branch entry, `origin_transaction_id` and
   `head_transaction_id` MUST name receipts in the set; the current head MUST
   be reachable from the origin by following parent links only (fast-forward);
   because the receipt set is bounded by `4,096` and acyclic, every walk
   terminates within `4,096` nodes; an unreachable head is
   `EXCHANGE_BRANCH_NOT_FAST_FORWARD`; and every
   origin and current fact (workspace, roots, epoch, policy, dependency
   roots) MUST equal the facts derived from the named receipts exactly as
   S20-500 sections 4 and 5 require, else `EXCHANGE_BRANCH_INVALID`.
5. **Head closure.** The accepted head MUST be a receipt in the set, else
   `EXCHANGE_HEAD_INVALID`. It is not required to be a descendant of every
   branch; branches may point anywhere in the exported ancestry.
6. **Workspace uniformity.** Every transaction, origin record, and ref in the
   exchange MUST carry the same `WorkspaceId`, else
   `EXCHANGE_WORKSPACE_MISMATCH`. Version 1 exchanges exactly one workspace.

## Digest tree

The tree algorithm tag is `1`, BLAKE3-256. One leaf exists for the embedded
pack, for every receipt entry, for every branch entry, and for the head, in
this exact order: the pack, all receipts in the canonical-set order of field
3, all branches in the canonical-set order of field 5, then the head. Section
tags are `1`, `2`, `3`, and `4` respectively.

```text
leaf_preimage = "sley2.repository-exchange-leaf.v1" ||
                uvar(section_tag) || identifier[32] ||
                len(stored_bytes) || stored_bytes
leaf = BLAKE3-256(leaf_preimage)

node = BLAKE3-256("sley2.repository-exchange-node.v1" || left[32] || right[32])
```

The leaf and node construction is byte-identical to the S20-170 construction
proven by its frozen fixture, under the exchange leaf and node domains. The
leaf identifier and stored bytes per section are:

| Section | Identifier | Stored bytes |
|---:|---|---|
| 1 | `RepositoryPackId` of the embedded pack | the exact stored pack |
| 2 | `TransactionId` | the exact stored receipt |
| 3 | the S20-500 `name_key` recomputed from the entry's `branch_name` (section 3 of that contract), never read from the payload | `encode_bytes(stored_origin) \|\| encode_bytes(stored_ref)` |
| 4 | `TransactionId` of the accepted head | `stored_head \|\| receipt_id[32]`, where `stored_head` is the S20-390 fixed-head value computed by the importer from `accepted_head.transaction_id` (`TRANSACTION_MODEL_V1.md`, fixed accepted-head bytes) and `receipt_id` is `accepted_head.receipt_id` |

`leaf_count` MUST equal `1 + |receipts| + |branches| + 1`, so a tree has at
least three leaves. The payload stores the exact ordered leaf list, its
count, and the tree root. At each level adjacent digests are paired; an
unpaired final digest is promoted unchanged; a single digest is its own root.
Any leaf-list or root disagreement, including a section-4 leaf that disagrees
with the head computed from field 4, is `EXCHANGE_DIGEST_TREE_MISMATCH`. The
outer `RepositoryExchangeId` additionally binds the header, the compression
profile, the tree, and signature absence.

## Resource limits

All SCB1 epoch-1 canonical limits still apply: the stored exchange is one
SCB1 standalone value bounded by `67,108,864` bytes, and every `Bytes` field
(the embedded pack, each stored receipt, origin, and ref) is bounded by the
epoch-1 `Bytes` payload ceiling of `16,777,216` bytes. This contract adds
closed exchange limits:

- stored exchange bytes: `67,108,864`;
- embedded pack bytes: `16,777,216`, the epoch-1 `Bytes` payload ceiling,
  which is also the exchange-profile cap; every inner S20-170 limit still
  applies unchanged, and the pack's own `4,096` root and `65,536` object
  ceilings bind independently;
- receipts: `4,096`;
- branches: `4,096` visible `(origin, ref)` pairs;
- digest leaves: `8,194` (the exact sum `1 + 4,096 + 4,096 + 1`);
- decoder allocation budget: `134,217,728` bytes, one top-down budget shared
  by the exchange decoder and the embedded pack decoder;
- expanded bytes: reserved; under profile `0` no limit beyond the stored and
  `Bytes` ceilings can bind, and a future compressed profile MUST define it;
- preflight work ceilings, single counters over the whole preflight (not
  per branch), mirroring the frozen S20-530 recovery work limits: object
  verifications `2,097,152` (each object is verified once and the
  result memoized across receipts), binding visits `4,194,304`, verified
  object bytes `1,073,741,824`, and verified receipt bytes `1,073,741,824`;
  exhausting any of them is `EXCHANGE_RESOURCE_LIMIT`.

Inner admissibility is a named invariant: the set of S20-170 packs the
embedded decoder accepts inside an exchange equals the set it accepts
standalone, because a maximal legal exchange consumes at most `83,886,080`
bytes (exactly `80` MiB) of the shared budget and leaves the pack decoder
more than its own `16,777,216`-byte worst case. Implementation proves it with a maximal-legal-exchange decode test.

The allocation budget counts decoder heap allocations for decoded values,
excluding the caller-owned stored input buffer; the embedded pack decoder is
charged against this same budget rather than receiving a fresh S20-170
budget. A maximal legal exchange is always decodable: the decoded payload
holds at most one copy of every embedded byte (at most `67,108,864`), the
embedded pack's decoded entries hold at most one further copy of its at most
`16,777,216` bytes, and fixed-size metadata is bounded by the entry ceilings,
so the sum stays below `134,217,728`.

The receipt ceiling is a clone-profile limit, not an ancestry limit. Export
evaluates every exchange limit and the pack's own root and object ceilings
before assembling bytes and fails `EXCHANGE_RESOURCE_LIMIT`; an S20-170 pack
larger than `16,777,216` stored bytes, a reachable history beyond `4,096`
receipts, or more than `4,096` visible branches is therefore not exchangeable
by v1. Import preserves the exact `PACK_RESOURCE_LIMIT` from the embedded
pack and returns `EXCHANGE_RESOURCE_LIMIT` for every exchange-level bound,
each checked before allocation. Deep histories require a later streamed or
compressed profile, which is explicitly outside v1.

Profile `0` performs no decompression. Any other profile returns
`EXCHANGE_COMPRESSION_UNSUPPORTED` before interpreting blocks.

## Export

Export holds shared repository maintenance ownership and reads only durable
verified facts:

1. resolve the fixed accepted head through `sley-txn`;
2. enumerate visible branches through `sley-repo` (`list_branches` up to the
   S20-500 ceiling); orphan origins are excluded;
3. collect the receipt set as the parent-link closure of the accepted head and
   of every branch's current head, loading each revision through the verified
   revision lookup; exceeding `4,096` receipts is `EXCHANGE_RESOURCE_LIMIT`;
4. export the S20-170 pack over the committed roots of that set, failing
   `EXCHANGE_RESOURCE_LIMIT` before assembly if the pack ceilings would be
   exceeded;
5. assemble the canonical entries, the tree, and the envelope.

Export order has no byte effect. Two exports of the same head, branch set,
and durable bytes yield identical exchange bytes and the same
`RepositoryExchangeId`.

## Import target

The import target is a path. It MUST be one of:

- a **fresh target**: the path does not exist; or it is a directory that
  contains nothing; or it is a directory whose only entry is an empty
  `exchange/` directory, or an `exchange/v1/` directory containing nothing
  except at most one `sley-repo`-owned marker temporary, which is exactly
  one regular file named `<hex>.stage.tmp` (removed on retry); any other
  entry under `exchange/v1/` is `EXCHANGE_TARGET_NOT_EMPTY`;
- an **incomplete clone** of this same exchange: the directory contains the
  stage marker for this `RepositoryExchangeId`; the fixed accepted head is
  absent, or present, resolving, and equal to `accepted_head.transaction_id`;
  every receipt already installed in the target is byte-exactly one of the
  exchange's receipt entries; every visible branch already installed is
  byte-exactly one of the exchange's branch entries; and every branch-origin
  record already installed, including one without a visible ref, is
  byte-exactly the origin of one of the exchange's branch entries. Any
  installed receipt, origin, branch, or head outside the exchange is
  `EXCHANGE_TARGET_INCOMPLETE_MISMATCH`.

Target inspection and every write use symlink discipline: the importer
resolves and pins the target directory once, requires the target, `exchange/`,
`exchange/v1/`, and every layout component it creates or writes through to be
a real directory that it created or verified non-symlink, opens the marker
and its temporary without following symlinks, and treats any symlink or
non-regular entry on those paths as `EXCHANGE_IO` before any write; a symlink
at any path component or at the marker or its temporary is `EXCHANGE_IO` and
takes precedence over the marker-shape rules above.

The stage marker is exactly one regular file
`exchange/v1/<hex>.stage`, where `<hex>` is the lowercase 64-character hex
`RepositoryExchangeId` (the S20-500 key convention), whose exact contents are
that 32-byte identifier. More than one marker file, a non-regular marker, a
marker whose contents disagree with its filename, a marker naming a different
`RepositoryExchangeId`, or a present head that does not resolve or names any
other transaction is `EXCHANGE_TARGET_INCOMPLETE_MISMATCH` before any write.
A path that exists as a non-directory, or a directory that cannot be read, is
`EXCHANGE_IO`. Any other directory, including every repository with an
accepted head and no marker, is `EXCHANGE_TARGET_NOT_EMPTY` before any write.
Imported bytes therefore never become authority inside an existing
repository.

## Import phases

Import is split into preflight and persistence:

1. bound stored bytes, decode the closed envelope and payload, verify the
   exchange trailer;
2. verify canonical order, counts, closed profiles, every declared identity,
   and the complete digest tree;
3. run the complete S20-170 preflight (its steps 1 through 5) over the
   embedded pack without store writes;
4. decode every receipt through the frozen codec and prove closure rules 1,
   2, 3, 5, and 6;
5. decode every branch record and ref and prove closure rule 4;
6. verify every receipt against the embedded pack's objects exactly as the
   verified revision lookup would (roots, policy, manifest lengths, object
   closure, tombstones) without writes;
7. classify the target as fresh or incomplete under the import-target rules;
8. only after all preflight checks pass, persist in this exact order, where
   every step is re-entrant (an already-complete step reverifies exact bytes
   and returns success without rewriting):
   1. create the target directory if absent, then create `exchange/v1/`,
      write the marker to a same-directory `sley-repo`-owned temporary file,
      fsync it, atomically rename it over `exchange/v1/<hex>.stage`, and
      fsync `exchange/v1/` and its parent, so the marker is the first regular
      file durably visible in the target; a marker is visible only after its
      rename, so no partially written marker is observable;
   2. create the remaining repository layout, initialize repository
      maintenance, and acquire exclusive repository maintenance ownership
      without waiting; a held lock is `EXCHANGE_IO`. The importer holds that
      ownership through step 8.7. Then re-run the complete step-7
      classification against the now-owned target and abort with the
      classification's code unless the owned result is an incomplete clone
      of this same exchange (which is what step 8.1 necessarily makes of a
      fresh target); the pre-ownership classification is advisory and only
      the owned classification authorizes writes;
   3. import the embedded pack (S20-170 step 6; idempotent for present
      objects);
   4. install every receipt through
      `initialize_trusted_clone_receipts_with_maintenance` (the receipt
      phase of the transaction-owner API below); no head is written;
   5. install every branch origin and ref byte-exactly under
      `locks/refs.lock` (origin, fsync, ref, fsync; an exact existing record
      is reverified and resynced, any other existing record is `EXCHANGE_IO`)
      and release the refs lock;
   6. write the fixed accepted head through
      `initialize_trusted_clone_head_with_maintenance`, last;
   7. remove the stage marker, fsync its directory, and release maintenance
      ownership;
9. return the reconstructed accepted head, the receipt count, the branch
   count, and the promoted and present object counts.

The lock order is the frozen `maintenance -> refs -> accepted`: the importer
holds exclusive `locks/maintenance.lock` for the whole persistence phase (the
composite-caller mode that recovery already uses), the branch step takes and
releases `locks/refs.lock` before the head step, and each clone-API phase
takes the exclusive `accepted.lock` inside the importer's maintenance
ownership. Concurrent importers of the same bytes into one target serialize on
the maintenance lock; the loser fails `EXCHANGE_IO` and may retry. Because
the marker is written before maintenance ownership exists, two importers of
different bytes can leave two markers and permanently jam a target as
`EXCHANGE_TARGET_INCOMPLETE_MISMATCH`; concurrent imports into one target are
otherwise outside v1. `sley-repo` owns `exchange/`; it lies outside the
S20-500 ref fan-out and the GC `objects/scb1` inventory, so no existing
unknown-entry rule fires on an X-07 clone.

The head is the completion witness and the marker is the write guard. On a
marked root (an `exchange/v1/` directory containing any entry whose name
ends in `.stage`, read without following symlinks) no reader resolves an
accepted head, and every frozen acceptance-establishing,
ref-mutating, or deleting path fails closed with `TXN_INCOMPLETE_CLONE`:
`sley-txn` `initialize_trusted_genesis`, `commit`, and `recover`, and
`sley-repo` `create_branch`, `advance_branch`, `recover_refs`,
`recover_gc_witness`, and exclusive GC acquisition all check the marker after
taking their locks and before any acceptance-establishing write, ref write,
or deletion (creating the maintenance layout or lock files is none of
those). Read paths (`verified_revision`, `resolve_branch`, `list_branches`)
stay available on a marked root and establish no acceptance. Object-store
puts remain content-addressed and establish nothing. Only the importer,
holding exclusive maintenance ownership, establishes acceptance or mutates
receipts, refs, or the head in a marked root, and every step before the head
is idempotent over exact bytes, so retry converges.

## Interruption and retry

An interrupted import leaves a fresh target or a marked incomplete clone (the
same `.stage`-entry predicate); on an incomplete clone no reader resolves an
accepted head and every frozen
write path fails closed with `TXN_INCOMPLETE_CLONE` until the importer
completes it. Retrying the import of the identical exchange bytes converges to the
complete clone; retrying with different exchange bytes fails
`EXCHANGE_TARGET_INCOMPLETE_MISMATCH`. The frozen interruption rows are:

| Row | Interruption point | Retry result |
|---|---|---|
| X-01 | before the stage-marker rename is durable (at most the empty `exchange/v1/` directory and one owned temporary exist) | target is fresh; the owned temporary is removed; import restarts from step 8.1 |
| X-02 | after the marker, before any object (layout may be partial) | incomplete clone; layout completed, objects, receipts, branches, head installed |
| X-03 | during object promotion | S20-170 idempotent re-import; staged object temporaries are removed by the importer's own re-import or, after the clone completes, by S20-530 owner recovery |
| X-04 | during receipt installation | no reader resolves a head, writes fail closed; exact existing receipts reverified; missing receipts installed |
| X-05 | during branch installation | no reader resolves a head, writes fail closed; exact existing origins and refs reverified; missing ones installed |
| X-06 | before the head rename is durable | no reader resolves a head, writes fail closed; branches complete; head installed on retry |
| X-07 | after the head, before marker removal | complete clone with marker; every step reverifies exact bytes, then the marker is removed |

`sley-txn` and `sley-repo` recovery on a marked incomplete clone fail closed
with `TXN_INCOMPLETE_CLONE` before removing anything, because their owned
cleanup could otherwise delete the importer's in-progress files; they never
complete an import. Any interruption row not listed above is an
implementation defect. No S20-530
matrix row is added or changed: an incomplete clone is never accepted state.

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
   `RepositoryExchangeId`, also after an X-07 retry; export order has no byte
   effect;
6. `T` contains no stage marker, no orphan origin, and no staged temporary.

Equivalence is defined over the exported set: a source that carries orphan
origins, unreachable receipts, or present-but-unreferenced objects exports
the same bytes as one that does not.

## Stable failures

Numeric codes `54000` through `54021` are exact:

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
| 54020 | `EXCHANGE_ROOT_CLOSURE` |
| 54021 | `EXCHANGE_WORKSPACE_MISMATCH` |

Strict lower-layer failures retain their exact owning code and numeric value:
`SCB_*`, `PACK_*`, `STORE_*`, `TXN_*`, `REF_*`, and `BRANCH_*` are preserved,
never remapped. `EXCHANGE_PROFILE_UNSUPPORTED` rejects signature presence
(field 8 tag `1`) and any compression or profile field other than the frozen
values; `EXCHANGE_PACK_INVALID` wraps no code and is returned exactly when
the embedded bytes are not a tag-170 version-1 pack, which includes a nested
exchange (tag 540) and every other contract tag; `EXCHANGE_RECEIPT_INVALID`
covers a receipt entry whose declared identities disagree with its bytes.
A marked incomplete clone rejects every frozen write path with the S20-390
code `TXN_INCOMPLETE_CLONE` (`39022`), named in the ownership section.

## Ownership and the transaction-owner API

`sley-repo` owns `exchange.rs`: export, preflight, the stage marker, the
maintenance ownership of the persistence phase, byte-exact branch
installation, and the clone-equivalent test. It may depend on `sley-txn`;
`sley-txn` remains independent of `sley-repo`.

S20-540 implementation must add one bounded two-phase API to `sley-txn`,
exactly as S20-500 added the verified revision lookup:

```text
initialize_trusted_clone_receipts_with_maintenance(
    &self, maintenance: &RepositoryMaintenanceGuard,
    expected_head: TransactionId, receipts: &[&[u8]],
) -> Result<usize, CommitError>

initialize_trusted_clone_head_with_maintenance(
    &self, maintenance: &RepositoryMaintenanceGuard,
    head: TransactionId,
) -> Result<AcceptedHead, CommitError>
```

Both require the caller's exclusive `RepositoryMaintenanceGuard` for this
exact repository, exactly as `recover_with_maintenance` does, and take the
exclusive `accepted.lock` for their duration. The receipt phase requires the fixed head
to be absent, or present and equal to `expected_head` with that receipt
already durable in this target; it never advances or replaces a present head.
It strictly decodes every stored receipt, installs in a deterministic
parent-before-child order derived from the decoded parent links (never the
slice order), verifies each receipt against the durable object store (state
root and policy root imported through their registries, manifest lengths,
complete object closure, tombstones) and against its already-durable parent,
installs it under its `TransactionId` with the S20-390 receipt durability
order (an exact existing receipt is reverified and resynced), and returns the
number of receipts durable after the call; a parent that is neither durable
nor present in the slice preserves the exact `TXN_*` failure. The head phase
requires every parent-chain receipt of `head` to be durable and requires the
fixed head to be absent, in which case it writes it with the S20-390 head
durability order, or already exactly equal to `head`, in which case it
reverifies and returns success without a rename.

S20-540 implementation also adds the incomplete-clone write guard as
transaction-owner and ref-owner work inside the slice: `sley-txn` exposes the
marked-root predicate (an `exchange/v1/` directory containing any entry
whose name ends in `.stage`, read without following symlinks) and returns the new
S20-390 code `TXN_INCOMPLETE_CLONE` (`39022`, the next contiguous numeric
after the frozen `TXN_RESOURCE_LIMIT` `39021`, appended to the frozen S20-390
code table in `ERROR_CODES_V1.md`, whose frozen range then extends to
`39022`, and to the `TransactionErrorCode` enum, its `symbol()` and numeric
mappings, and the S20-390 contract checker's symbol list by the same commit) from `initialize_trusted_genesis`, `commit`, and `recover`;
`sley-repo` returns the same code through its existing upstream transaction
error from `create_branch`, `advance_branch`, `recover_refs`,
`recover_gc_witness`, and exclusive GC acquisition. The clone API phases
themselves run only on a marked root and are the sole acceptance-establishing
writers there.

Relative to `initialize_trusted_genesis`, the clone path reproduces from
receipt bytes and imported objects every check that genesis derives from its
inputs: registry-authorized state root and policy root, exact object
inventory and manifest lengths, and the tombstone set. It replaces the
caller's explicit root-of-trust input with the exchange trailer, the fresh or
same-id incomplete target precondition, and the importer's exclusive
maintenance ownership. Neither phase accepts a candidate, a policy
transition, or any authority claim beyond the receipt bytes and one
`TransactionId`. The API is not reachable through ordinary candidate commit.

## Required evidence

Implementation acceptance requires at least:

- the frozen conformance fixture under `conformance/repository-exchange/v1/`
  with an independent Python reproduction of `RepositoryExchangeId` and the
  tree root, and a checker that recomputes both frozen hashes from the
  preimage texts in this document;
- exact envelope, payload, and digest-tree round trips;
- field-perturbation and section-perturbation rejection matrices for every
  code in the table, including a canonical-set order test covering branch
  names of 127, 128, 253, 254, and 255 bytes together with the `b`-versus-`aa`
  case;
- ancestry closure, surplus, cycle, zero and multiple genesis, parent-shape,
  head-membership, root-closure, workspace, and branch fast-forward rejection
  tests;
- nested-exchange, signature-presence, nonzero-profile, oversize-pack, and
  each resource-limit rejection before any write;
- fresh-target, non-empty-target, non-directory target, incomplete-clone
  retry, mismatched-marker, malformed-marker, and foreign-head tests;
- the complete X-01 through X-07 interruption matrix with injected cuts, and
  re-export determinism after an X-07 retry;
- the clone-equivalent test over a source with at least two branches, one
  advanced branch, one orphan origin, one unreachable receipt, and one
  present-but-unreferenced object;
- a test that the clone API is unreachable from ordinary candidate commit;
- genesis-into-incomplete-clone, commit-into-incomplete-clone,
  recovery-into-incomplete-clone (transaction, refs, and GC witness),
  branch-mutation-into-incomplete-clone, and GC-into-incomplete-clone tests
  proving `TXN_INCOMPLETE_CLONE`, and a read-path test proving no accepted
  head resolves on a marked root;
- an owned re-classification test (a target changed between the advisory and
  the owned classification aborts), a pre-seeded-symlink rejection test, an
  extra-installed-branch incomplete-clone mismatch test, a nested-exchange
  test expecting `EXCHANGE_PACK_INVALID`, a preflight-cost test at the
  maximal legal exchange, and the maximal-legal-exchange decode test;
- a structural dependency check proving no `sley-txn -> sley-repo` edge;
- an exchange-import persistent libFuzzer target (S20-700 adjacent surface);
- Tier 1 plus repository-focused Tier 2 validation;
- Ariadne contract review, Nabu design review, and Vulcan import-surface
  review with every report-grade finding closed.

## Explicit exclusions

This contract does not claim:

- compression, streaming, signatures, packs above `16,777,216` bytes, or
  histories beyond `4,096` receipts;
- import into an existing repository, merge, fetch, or partial exchange;
- more than one workspace per exchange;
- branch deletion, rename, force movement, tags, or symbolic refs;
- semantic comparison, conflict objects, or any S20-510/S20-520 semantics;
- SMP1, JSON bridge, CLI, runtime, benchmark, packaging, release, or GA.
