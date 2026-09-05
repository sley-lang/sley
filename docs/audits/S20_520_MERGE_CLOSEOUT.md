# S20-520 Merge Closeout

Status: **implemented under the draft Merge v1 contract (revision 4); all
three Council reviews received (2026-09-04, all FAIL) and every P0 and P1
closed in revision 4; the package awaits re-review, so it is not complete;
the Sley 2 goal remains incomplete**

Date: 2026-09-03; revised 2026-09-05

Validation tier: **Tier 1 plus repository-focused Tier 2 handoff**

## Claim under review

Two complete roots compose against their verified exact common ancestor
through the frozen S20-510 deltas: judgment rules J1 through J8 (J2 total,
with convergent removal), set-valued field composition, and the
non-ownership collateral rule over surviving identities decide
disjointness or deterministic composition; composed objects carry
ours-side metadata with divergence reported; the merged root is re-judged
by the S20-250 full closure; the merge plan is an S20-350 candidate with
derived creation identities emitted in derivation order, committed through
the frozen S20-390 path on ours with the branch pre-check and the S20-500
direct-parent advance; and every unproven composition yields one canonical
conflict object with pinned identity-free entries. The contract is
`docs/spec/MERGE_V1.md` with ADR-0028.

The implementation provides:

- `sley-id`: the thirty-second domain `sley2.merge-conflict.v1` and
  `MergeConflictId` with its frozen vector;
- `sley-repo`: `merge.rs` with `find_common_ancestor` over two head-first
  S20-500 ancestries (bounded, work-charged), `transaction_ancestry`
  walking repository chains, `verify_merge_ancestor` proving the ancestor
  precondition, `judge_merge_verified` as the production entry point (bare
  `judge_merge` stays for synthetic inputs), `MergeSide` (from a verified
  revision or synthetic objects), field composition over the generated
  proposal bodies, `build_merge_plan` with the S20-345 identity remap in
  derivation-is-operation order, fail-closed reference rewriting, and the
  single-candidate entry-point bound, `commit_merge` with the branch
  pre-check, the empty-plan pairing check, and numeric-preserving
  commit-path failures, `encode_merge_conflict` and the strict
  `decode_merge_conflict`, the conflict epoch record (tag 520, digest
  domain 21), and the fourteen `MERGE_*` codes 52000 through 52013 with
  wrapped `COMPARE_*`, extraction, `IMPACT_*`, `SCB_*`, and commit-path
  codes (with owning numerics) preserved.
- `sley-protocol`: both merge handlers verify the ancestor over
  server-walked ancestries before any composition.

The S20-390 commit, the S20-360 validator, the S20-500 refs, and the
S20-510 delta are consumed exactly as frozen; the merge writes no root,
object, receipt, or ref itself.

## Evidence

- Contract draft revision 1 at `ded0943`; revision 2 narrowed the
  collateral rule to non-ownership relations; revision 3 added the
  created-identity remap the frozen S20-345 identity rule requires;
  revision 4 closes every P0 and P1 from the three Council reviews (see
  below). Implementation, corpus, oracle, and fuzz slices in the revision
  4 commit.
- Conformance corpus: `conformance/merge/v1/accepted.json` (nineteen
  three-way cases: the original thirteen plus the theirs-direction
  collateral, the swapped disjoint pair, the both-removed convergence,
  the conflict-without-collateral survivor case, and the root-anchor and
  policy-root conflicts; every side carries its record anchors),
  and `rejected.json` (four stored-byte mutations),
  drift-gated by `scripts/generate_merge_fixtures.py --check` in
  `make quick`; `scripts/check_merge_vector.py` re-derives every outcome,
  merged body, conflict entry (with anchor detail bits), canonical
  conflict bytes, and `MergeConflictId` from the compact root
  descriptions under the frozen oracle environment, registered in
  `make conformance`.
- Native tests: twenty-one `sley-repo` merge tests, including the revision
  4 evidence: theirs-added entry point plans `MERGE_PLAN_UNSUPPORTED`
  with a bind-on-ours-first recovery that commits; two created entities
  derive in operation order and commit; composed objects carry ours-side
  label and fingerprint with divergence reported and bodies symmetric
  under swap; conflicted identities gain no collateral; collateral keeps
  its sides under swap; both-sides removal converges; anchor conflicts pin
  the moved class and round-trip; bare judgment takes a caller-chosen
  `O = A` silently while the verified entry rejects it; oversized
  ancestries fail before search; the stale branch fails before anything is
  durable with the exact frozen code and numeric; the foreign empty-plan
  pairing fails; forged detail/kind fail strict decode.
- Persistent fuzz: `fuzz/targets/merge_conflict_decoder.rs` (conflict
  decoder in two lanes plus the ancestor rule) and the new judgment lane
  `fuzz/targets/merge_judgment.rs` (mutation scripts over a fixed valid
  base root asserting deterministic well-formed outcomes or frozen-coded
  failures), both smoke `PASS`
  (`docs/audits/S20_700_MERGE_PERSISTENT_SLICE.md`); the S20-700 finding
  register and independent review remain deferred.
- Tier 1: `make quick` green at every commit.
- Tier 2: see the validation record below.

## Findings closed in revision 4

Seven P0s (Ariadne 3, Nabu 3, Vulcan 1):

- **Kind-16 plan rule (Ariadne P0-1, Nabu P0-1/P0-4).** The draft's
  `AddEntryPoint`-creates-an-entity row was unexecutable, but the
  reviewers' suggested shape (derive, `CreateEntity`, then `AddEntryPoint`)
  is also inexpressible: the frozen S20-350 descriptor binds
  `AddEntryPoint` to `ExactEntityVersion`, and frozen S20-360 phase 3
  checks every such precondition against the base state, so no single
  candidate binds what it creates (proven by a failing commit test during
  revision 4). The plan reports `MERGE_PLAN_UNSUPPORTED` with a documented
  bind-first recovery, which the repository test proves commits.
- **Creation ordinals (Ariadne P0-2).** Creations now emit in derivation
  order, so the frozen validator's position-numbered ordinals agree; the
  two-entity repository test proves the commit.
- **Theirs-side collateral objects (Ariadne P0-3, Nabu P1-5, Vulcan
  P1-2).** `ours_object` comes from `A` and `theirs_object` from `B` in
  both directions; the mirror corpus vector plus the independent oracle
  (which always read the correct sides) lock the agreement.
- **Fingerprint claims (Nabu P0-2).** Composed objects carry `A`'s
  fingerprint claim, matching frozen `replace_body`; the test proves it.
- **Labels and symmetry (Nabu P0-3, Ariadne P2-9).** Composed objects
  carry `A`'s label with theirs-side divergence reported in
  `metadata_overridden`; the symmetry paragraph now states exactly what
  swap preserves (entity set, bodies, swapped conflicts) and what follows
  the ours side (reported metadata).
- **Common-ancestor enforcement (Vulcan P0-1, Nabu P1-4, Ariadne P1-5).**
  Both protocol handlers walk ancestries server-side and verify before
  judging; the `O = A` attack test proves bare judgment is silent and the
  verified entry fails `MERGE_ANCESTOR_MISMATCH`. `52001` now also names
  the plan-base mismatch the commit path has always enforced.

Every P1 is closed alongside: J2 totality, survivor-scoped collateral
with retyped dependents and documented `MetadataOnly` exclusion, pinned
anchor/`Closure` entry fields with a strict decoder, whole-tuple dedup,
fail-closed reference rewriting, the branch pre-check with the stated
durability boundary, the empty-plan pairing check, numeric-preserving
commit failures, the ancestry bound and full work accounting, the judgment
fuzz lane, the self-contained-plan paragraph corrected to the
`MergeCommitInput` split, the grantless-commit bound stated, and the
`MergeSide` trust boundary documented. The campaign's three open questions
are answered in the contract (J6 kept and reported; `MetadataOnly`
excluded from collateral; `A`'s label with report).

## Explicitly open and deferred

- **Re-review.** The three lanes verdicts above predate revision 4; the
  package completes when they (or their successors) pass.
- The `Closure` conflict path is defensive: no corpus case reaches it,
  because set composition over complete roots preserved every closure rule
  in every constructed case. The contract carves it out of the corpus bar
  explicitly; its coverage is the strict-decoder rejection matrix and the
  pinned nonzero-detail rule.
- Restricted S20-360 bounds committable merges to executable-program-
  operation-free roots; the judgment and plan are exact regardless.
- The merge judgment fuzz lane scripts stay within nine valid entities;
  resource-ceiling inputs are covered by native bound tests, not by fuzz.
- Strict pedantic clippy debt in older `sley-repo` test modules is
  pre-existing; the new paths lint clean under `--no-deps`.

## Validation record

Tier 1 `make quick` passed at every commit of the slice. Tier 2 runs at
the revision 4 commit: `make core`, `make conformance` (including the
merge oracle line over nineteen vectors), `make adversarial`,
`make fuzz-smoke`, and `make merge-persistent-fuzz-smoke` (decoder plus
judgment lanes, nightly builds with live runs). The full `make v1` gate
was skipped because this is a subsystem handoff, not a release boundary;
`make v2` and `make release-check` remain intentionally fail closed.

## Independent review

Ariadne contract review `FAIL_3_P0_5_P1_9_P2_6_P3`, Nabu architecture
review `FAIL_3_P0_5_P1_8_P2_4_P3`, Vulcan surface review
`FAIL_1_P0_5_P1_7_P2_3_P3` (all 2026-09-04, at `9dc78fe` against contract
revision 3). Every P0 and P1 above is closed in contract revision 4;
re-review is pending.
