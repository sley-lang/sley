# S20-520 merge: campaign record (opened 2026-09-03)

Status: contract draft revision 1 committed; Council review pending.

## Why now

S20-510 is implemented (reviews pending), so S20-520 merge is the next
dependency-complete package: the last M4 blocker and the one absent
Section 18.5 persistent-fuzz surface.

## Frozen facts that shaped the design

- S20-390 `commit` requires `expected_parent` to equal the fixed accepted
  head, so a merge commits "theirs into ours" as an ordinary candidate on
  `A`'s repository; `B` may come from another repository (for example an
  S20-540 clone).
- The S20-350 candidate profile carries whole-body `CreateEntity`,
  `ReplaceEntityVersion`, and `DeleteEntityBinding` with
  `ExpectedIdentityAbsent` and `ExactEntityVersion` preconditions, plus
  `AddEntryPoint`/`RemoveEntryPoint`; entry points must be removed before
  their entity is deleted.
- Restricted S20-360 rejects dependency-root changes and validates only
  executable-program-operation-free candidates, which bounds committable
  merges, not the judgment.
- S20-500 ancestry is linear and head first, so the common ancestor is exact.

## Council availability

Unchanged: every lane was still down when this draft was written; the
retry loop in the session scratchpad dispatches the queued S20-250 reviews
first. The S20-510 and S20-520 review requests follow when a lane returns.

## Open questions for the reviews

- Whether J6 (semantic change overrides a metadata-only change, reported in
  `metadata_overridden`) is acceptable or must be a conflict.
- Whether the collateral rule should also treat `MetadataOnly` changes as
  touching an entity.
- Whether composed objects should carry `A`'s label or none.

## Commits

- contract draft revision 1: the commit that adds this file.
