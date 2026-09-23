# Complete-Root Index Snapshot Profile v1

Status: S20-300 full contract draft, revision 4 (2026-09-23); implemented
under this draft with Council review pending (Ariadne contract review, Nabu
architecture review, Vulcan surface review), so the contract is not frozen
and the package is not complete. Revision 3 closes the remaining
report-grade findings with guard-held cache access, exclusive temp files,
fail-open cache I/O, fresh-only exported capsules, and the residual
precision notes. Revision 4 admits one second hit reader, the read-only
identity probe `cached_complete_root_snapshot_id` (section 5), whose only
consumer is the SMP1 revision 13 `workspace.open` snapshot identity
(REQ-10); nothing else changes. Implementation state is tracked in the
machine summary.

This profile completes S20-300. It adds the complete-root completeness arm
to the frozen `SLEYIDX1` record, binds every complete-root snapshot to the
exact `StateRoot` it indexes, derives the index from the S20-250 full
complete-root judgment over verified objects, and defines the one place a
snapshot may be reused without a rebuild: the repository-owned index cache
that serves read-only derived query evidence. It composes, and never alters:

- the restricted profile `docs/spec/INDEX_SNAPSHOT_PROFILE_V1.md`: its
  record grammar, identity domain `sley2.index-snapshot.v1`, restricted arm
  `1`, fixed vectors, codes 30000 through 30007, and rebuild-first admission
  stay normative and byte-identical;
- the S20-250 full profile `docs/spec/COMPLETE_ENTITY_IMPACT_PROFILE_V1.md`:
  the complete-root request, closure rules C1 through C11, and the exact
  eighteen-kind index;
- the S20-390 verified revision, whose objects are loaded and
  inventory-checked before any extraction.

The central security rule of the restricted profile is unchanged:

> A snapshot digest authenticates bytes, never semantic provenance.

Provenance for a complete-root snapshot comes from the build path alone:
the record is derived from a verified revision's objects through the
S20-250 full judgment, and a cache hit is accepted only under the repository
authority that wrote it, only for read-only derived query surfaces, and
never as input to validation, comparison, merge, commit, or recovery.

## 1. Completeness arm and context

```text
IndexCompleteness = RestrictedModeledKinds4To15Only(1) | CompleteRoot(2)

SnapshotContext {
  schema_epoch: SchemaEpochId,
  claimed_root_context: Option<StateRoot>   // MUST be Some for arm 2
}
```

For arm `2` the context root is not a claim: the builder MUST have derived
the inventory from exactly that root's `entity_bindings` (closure rule C1),
so the field is the bound root. A record with arm `2` and `claimed_root_context = None`
is `INDEX_SNAPSHOT_FORMAT_INVALID`. The context check runs first: under a
rooted expected context a rootless arm-`2` record fails
`INDEX_SNAPSHOT_CONTEXT_MISMATCH` (the rejected rootless-context vector
pins this precedence); the `FORMAT_INVALID` arm covers decoding with a
rootless expected context. A record with arm `1` keeps the
restricted semantics unchanged. Unknown arms fail
`INDEX_SNAPSHOT_COMPLETENESS_UNSUPPORTED`, and each consumer states which arm
it accepts: the restricted S20-310 queries and the S20-320 capsule accept
arm `1` only and fail closed on arm `2`; the complete-root consumers of this
profile accept arm `2` only.

## 2. Record

The record grammar is the restricted profile's section 4 with two changes
for arm `2`: `u32be(completeness=2)` and inventory kinds over SSMC1 tags
`1` through `18`. Inventory is the complete root's entity inventory in strictly-ascending
unique raw `EntityId` order; direct edges are the complete-root index's canonical edge
set; reverse groups are its exact inversion. The identity is derived by the
unchanged domain over the unchanged preimage grammar, so arm `1` and arm
`2` records over the same entities always differ.

## 3. Build

```text
build_complete_root_snapshot(schema_epoch, root, entities, facts)
  = judge_complete_root(entities, facts)   // S20-250 full, C1 through C11
    then encode(arm 2, Some(root))
```

`entities` and `facts` are the S20-250 full complete-root request; `root` is
the `StateRoot` whose record produced `facts`. The pure builder lives in
`sley-query` and performs no I/O. Every `IMPACT_*` failure is preserved
inside `INDEX_SNAPSHOT_ROOT_INCOMPLETE`. The repository-backed builder in
`sley-repo` extracts the request from a verified revision through the
S20-250 full adapter, so the root, facts, and objects are chain-verified.

## 4. Admission

`admit_complete_root_snapshot(schema_epoch, root, entities, facts, candidate)`
is the restricted admission with the complete-root builder: rebuild first,
bounded-inspect the candidate against the expected context and arm `2`,
discard on any format, version, context, completeness, digest, or resource
failure, compare byte for byte, and return `Hit` only on exact equality.
Discard reasons are the restricted profile's. This API is conformance and
security evidence; it never skips the rebuild.

## 5. Repository index cache

`sley-repo` owns the only reuse path. The cache lives under
`<repository>/index/v1/` as one file per root named by the lowercase hex of
the `StateRoot`, suffix `.idx.scb1`, written by temp-and-rename (a unique
`<name>.tmp.<pid>.<counter>` created exclusively, then rename, directory
synced) after a fresh build from a verified revision under shared
repository maintenance. Every cache-touching call takes the caller's
maintenance guard over the same repository and refuses a guard for another
root; unguarded access is a contract violation the code does not admit.
`index` joins the frozen repository layout entries that an incomplete S20-540 clone may carry,
and an S20-540 import **removes** that directory before it promotes anything.
Every other entry such a clone holds is proved to belong to the exchange; a
cache record cannot be, because it is keyed by a state root the import has not
yet accepted and its bytes were written by whoever created the target. Import
removes it rather than refusing the target, because the cache is derived and
disposable and the next request rebuilds it. A symlink or a non-directory at
the cache path is refused.

```text
complete_root_snapshot(repository, revision) =
  Hit(snapshot)                      // cached record accepted
  | Rebuilt { reason, snapshot }     // fresh build written back
```

The cached record is accepted, without decoding objects or extracting
edges, exactly when all of the following hold:

1. it bounded-decodes with arm `2` and context
   `(revision.schema_epoch, Some(revision.root))`;
2. its trailer equals the identity derived from its preimage;
3. its inventory identities equal the revision's record `entity_bindings`
   keys, in order, and its inventory length equals the binding count;
4. every edge endpoint is an inventory identity and the reverse groups are
   the exact inversion (the restricted inspector's rules).

Any other outcome discards the file, rebuilds from the revision, writes the
fresh record, and returns `Rebuilt` with the reason; a cache write or read
failure is `INDEX_SNAPSHOT_IO` only when the fresh build itself cannot be
returned. A cached inventory that differs from the record's bindings is
`INDEX_SNAPSHOT_ROOT_MISMATCH` as the discard reason's exact code.

Because rules 1 through 4 do not re-derive edges, a filesystem writer that
can forge a digest-valid record with the right inventory could serve wrong
edges — or wrong inventory kinds, which the alignment binds by identity
only — to a query. That is the residual the restricted profile's rule names,
and it is bounded three ways: the cache is under the same local filesystem
authority as objects, receipts, and refs, which is why an exchange import
removes an inherited one rather than adopting it; only transient
in-process reads on the read-only derived query surfaces (S20-310
root-backed queries) and the identity probe below may consume a hit, and
exported evidence MUST NOT
rest on a bare hit — exported capsules build from a fresh snapshot, never
from the cache; validation, comparison, merge, commit, exchange, GC, and recovery never read
the cache. `verify_cached_snapshot(repository, revision)` rebuilds and
compares the cached record byte for byte for audits and Tier 2 evidence,
reporting `Match`, `Missing`, or `Mismatch`: a missing cache is benign,
a differing one (including a present-but-unreadable file) demands
investigation. Concurrent readers and writers observe atomic renames over
deterministic fresh builds, so last-writer-wins is harmless: every fresh
build of one revision is byte-identical.

**Identity probe (revision 4).**
`cached_complete_root_snapshot_id(repository, revision, guard)` answers
the `IndexSnapshotId` of the cached record for `revision.root` when, and
only when, that record exists as a regular file and is accepted under rules
1 through 4 above; it answers absence otherwise. It is the cheap half of
`complete_root_snapshot` alone: it reads and bounded-decodes at most one
cache file (up to `MAX_SNAPSHOT_RECORD_BYTES`), reads no object, extracts no
edge, never builds, never writes back, and never deletes. A missing,
unreadable, non-file, or discarded record is absence, not an error and not a
rebuild (unlike the discard-rebuild-write rule of `complete_root_snapshot`);
the only error is a guard naming another repository (`INDEX_SNAPSHOT_IO`).
The caller holds shared repository maintenance over the same repository,
as for every cache-touching call. Its one sanctioned consumer is the
SMP1 revision 13 `workspace.open` field 9 under a version 2 selection
(`docs/spec/SMP1.md` appendix A `open_summary`), which takes the guard
without waiting and without initializing the boundary and treats any
failure as absence; the identity it discloses is a pointer, not evidence:
queries that bind it still load or rebuild the snapshot through
`complete_root_snapshot`, and exported evidence still builds fresh. A new
probe caller fails the stage checker until it is deliberately listed.

Cache files are derived and disposable: they are outside the object store,
outside every retention root, never packed or exchanged, and safe to delete
at any time. Their names carry no branch, ref, transaction, path, or time
fact. There is no eviction policy and no size cap beyond one file per root:
growth is bounded by the revision count, GC ignores `index/`, and operators
may delete the directory whenever they like; the next request rebuilds it.

## 6. Limits

The restricted profile's limits apply unchanged; inventory entries are
bounded by the S20-250 request ceiling of `65,535`, and a complete root
whose index exceeds the `400,000` edge ceiling cannot be snapshotted and
fails `INDEX_SNAPSHOT_RESOURCE_LIMIT` with no partial record.

## 7. Stable failures

Codes 30000 through 30007 are unchanged. This profile appends:

| Numeric | Symbolic code |
|---:|---|
| 30008 | `INDEX_SNAPSHOT_ROOT_INCOMPLETE` |
| 30009 | `INDEX_SNAPSHOT_ROOT_MISMATCH` |
| 30010 | `INDEX_SNAPSHOT_IO` |

`IMPACT_*` codes are preserved inside `INDEX_SNAPSHOT_ROOT_INCOMPLETE`;
`STORE_*`, `TXN_*`, and `SCB_*` codes from revision loading are preserved by
the S20-390 loader and never remapped. Cache errors surface the outer
`INDEX_SNAPSHOT_*` symbol with the wrapped code traveling inside the error
value (`IndexCacheError::code()` names the outer symbol; the inner
`IMPACT_*` stays inspectable, never stringified away).

## 8. Required evidence

Implementation acceptance requires at least:

- fixed vectors freezing an arm-`2` record and identity over the frozen
  S20-250 complete-entity fixture, with an independent Python reproduction
  of the record bytes and identity from the fixture's edges;
- proof that arm `1` and arm `2` over the same entities differ, that arm
  `2` without a root is rejected, and that the restricted query and capsule
  surfaces fail closed on an arm-`2` snapshot;
- admission tests for every discard reason under arm `2`;
- repository cache tests: miss then hit without object decoding (proven by
  a counter or by removing the objects' readability), stale or forged
  inventory discarded with `INDEX_SNAPSHOT_ROOT_MISMATCH`, corrupt file
  discarded and rewritten, a second root cached beside the first,
  `verify_cached_snapshot` equality, and an incomplete S20-540 clone that
  carries an `index` directory still classifying as incomplete;
- identity probe tests (revision 4): cold absence with nothing written, the
  identity of an accepted record, a hit without object access, and a
  discarded record reported as absence without a rewrite;
- an S20-700 persistent libFuzzer target over the arm-`2` decoder;
- Tier 1 plus semantics-focused Tier 2 validation;
- Ariadne contract review, Nabu architecture review, and Vulcan surface
  review with every report-grade finding closed.

## 9. Explicit exclusions

This contract does not claim:

- the full S20-320 capsule, which is a later package building on the
  fresh-only rule above; S20-310 root-backed queries consume arm-`2`
  snapshots today (built directly or through the repository cache, whose
  hit path has no readers beyond those queries and the revision 4 identity
  probe), and cross-repository,
  signed, or exchanged snapshots;
- fingerprint catalogs inside the record;
- SMP1, sessions, JSON bridge, CLI, runtime, benchmark, packaging, release,
  or GA.
