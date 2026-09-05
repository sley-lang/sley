# S20-520 merge: campaign record (opened 2026-09-03)

Status: contract draft revision 4 committed; all three Council reviews
received (2026-09-04, all FAIL) with every P0 and P1 closed in revision 4;
re-review pending.

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

## Answers recorded in contract revision 4 (2026-09-05)

- J6 stays: overriding a metadata-only change with a semantic one,
  reported, is composition with a receipt. The defect was elsewhere: J7
  used to drop a theirs-side label change unreported.
- No: `MetadataOnly` never triggers collateral on either side (a removed
  dependent likewise resolves by removal). The dependent side counts
  `Added`, `Changed`, and `Retyped`.
- `A`'s label and `A`'s fingerprint claim, because the frozen
  `replace_body` preserves exactly those and anything else makes the
  precomputed root uncommittable; a theirs-side label change joins the
  `metadata_overridden` report. Neither "A's label, dropped silently" nor
  "none" survives the frozen commit path.

## Commits

- `ded0943` contract draft revision 1, ADR-0028, checker, this record;
  registration and the merge-conflict domain in the staging commit.
- implementation, revisions 2 (non-ownership collateral) and 3 (identity
  remap), corpus, oracle, fuzz slice, closeout, and the S20-700 surface
  closure: the commit after the staging commit.
- revision 4: all seven P0s and every P1 from the three 2026-09-04 Council
  reviews closed (verified ancestor enforcement, ours-side composed
  metadata with divergence report, survivor-scoped and side-correct
  collateral, derivation-ordered creations, single-candidate entry-point
  bound with recovery, strict decoder, commit guards with preserved
  numerics, 19-vector corpus with anchors, judgment fuzz lane); the
  revision 4 commit on `main` (see `git log --oneline`).
