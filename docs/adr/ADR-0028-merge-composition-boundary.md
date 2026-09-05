# ADR-0028: Merge composition and conflict boundary

Status: accepted with revision 4 of the S20-520 contract (2026-09-05);
written against the draft at revision 3 while Council lanes were
unavailable, then revised against all three Council reviews (closeout
`docs/audits/S20_520_MERGE_CLOSEOUT.md`)

Date: 2026-09-03; revised 2026-09-05

## Context

The Repository Model requires three-way merge over an exact common
ancestor, automatic composition only under proven disjointness or
deterministic composition, and canonical conflict objects. S20-510 now
supplies a typed delta with entity classes, per-field identity sets, and a
collateral set; S20-500 supplies linear verified ancestry; S20-390 accepts
only candidates whose expected parent is the fixed accepted head; and the
frozen S20-350 candidate profile carries whole-body `CreateEntity`,
`ReplaceEntityVersion`, and `DeleteEntityBinding` operations with exact
version preconditions. The work-package row names a silent choice or a lost
change as the primary risk.

The Council lanes were still unavailable at this draft (see ADR-0026); the
design is the integrator's and is submitted to Ariadne, Nabu, and Vulcan as
soon as a lane returns.

## Decision

1. **Merge theirs into ours through the frozen commit.** A merge composes
   `A` and `B` against their exact common ancestor `O` and commits the
   result as an ordinary S20-350 candidate on `A`'s repository through
   S20-390 and S20-500. No root, object, receipt, or ref is written by the
   merge itself, and no multi-parent transaction exists.
2. **Judgment over S20-510 deltas only.** Entity rules J1 through J8,
   set-valued field composition, and the collateral rule of
   `docs/spec/MERGE_V1.md` are the whole judgment. Anything not proven
   disjoint or deterministically composable is a conflict; a metadata-only
   change overridden by a semantic change is reported, never silent. The
   composed object carries ours-side metadata (label and fingerprint
   claim), the only bytes the frozen commit reproduces, and a theirs-side
   label change joins the same report. Collateral runs only on identities
   that survived J1 through J8 without conflict.
3. **Merged root is re-judged.** The merged request must pass the S20-250
   full complete-root judgment before any plan exists; failure is a
   `Closure` conflict, and the merged `StateRoot` is derived by the frozen
   S20-160 builder from `O`'s anchors.
4. **Canonical conflict object.** Conflicts are one SCB1 record
   (`sley2.merge-conflict.v1`, tag 520, digest domain 21), the thirty-second
   `sley-id` domain, binding the three roots and both delta identities.
5. **Result mismatch fails closed.** After the commit the new head's root
   must equal the plan's merged root; a mismatch is `MERGE_RESULT_MISMATCH`
   and is never repaired.
6. **Staging.** `scripts/check_merge_spec.py` binds the contract, ADR,
   work-package row, summary section, and both frozen hashes, and fails
   closed if `crates/sley-repo/src/merge.rs` or the corpus appears before
   the summary allows implementation; the frontier checkers' fail-closed
   marker on that file is replaced by the staged rule at implementation.
7. **Created identities are re-derived.** Because S20-345 derives every
   `CreateEntity` target from the candidate nonce, kind, and creation
   ordinal, entities the other side added are re-identified in the plan
   with every local reference rewritten; derivation order is operation
   order, so the frozen ordinal check passes. A theirs-added entry point
   is the exclusion the frozen profile forces: no single candidate can
   bind what it creates, so the plan reports `MERGE_PLAN_UNSUPPORTED`
   (bind on ours first, then merge) instead of emitting an unexecutable
   shape or a foreign identity.
8. **The ancestor is proven, not asserted.** Both protocol handlers walk
   the ancestries server-side and verify the supplied `O` before any
   composition; bare `judge_merge` stays available for synthetic inputs
   only.

## Consequences

- The merge engine is the eleventh Section 18.5 persistent-fuzz surface:
  the conflict-decoder lane and the judgment lane both run under
  `make merge-persistent-fuzz-smoke`.
- Restricted S20-360 bounds which merged roots can be committed today; the
  judgment and plan are exact for every root regardless.
- Rename detection and block-level merging stay outside the frozen boundary.
