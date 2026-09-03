# S20-520 Merge Closeout

Status: **implemented under the draft Merge v1 contract (revision 3); Council reviews pending, so the package is not complete; the Sley 2 goal remains incomplete**

Date: 2026-09-03

Validation tier: **Tier 1 plus repository-focused Tier 2 handoff**

## Claim under review

Two complete roots compose against their exact common ancestor through the
frozen S20-510 deltas: judgment rules J1 through J8, set-valued field
composition, and the non-ownership collateral rule decide disjointness or
deterministic composition; the merged root is re-judged by the S20-250 full
closure; the merge plan is an S20-350 candidate committed through the
frozen S20-390 path on ours with the S20-500 direct-parent advance; and
every unproven composition yields one canonical conflict object. The
contract is `docs/spec/MERGE_V1.md` with ADR-0028. It is a draft: every
Council lane was unavailable when it was written and when the
implementation landed, so the reviews that freeze it and complete the
package are pending and must pass before the status above changes.

The implementation provides:

- `sley-id`: the thirty-second domain `sley2.merge-conflict.v1` and
  `MergeConflictId` with its frozen vector;
- `sley-repo`: `merge.rs` with `find_common_ancestor` over two head-first
  S20-500 ancestries, `MergeSide` (from a verified revision or synthetic
  objects), `judge_merge` returning `MergeOutcome::Merged` or
  `MergeOutcome::Conflict`, field composition over the generated proposal
  bodies (`copy_field`, `replace_identity_set`), `build_merge_plan` with
  the S20-345 identity remap of created entities and a full local-reference
  rewriter over every body kind, `commit_merge` through `TransactionRepository::commit`
  and `BranchRepository::advance_branch` with the post-commit root check,
  `encode_merge_conflict` and `decode_merge_conflict`, the conflict epoch
  record (tag 520, digest domain 21), and the fourteen `MERGE_*` codes
  52000 through 52013 with wrapped `COMPARE_*`, `IMPACT_*`, `SCB_*`, and
  commit-path codes preserved.

The S20-390 commit, the S20-360 validator, the S20-500 refs, and the
S20-510 delta are consumed exactly as frozen; the merge writes no root,
object, receipt, or ref itself.

## Evidence

- Contract draft revision 1 at `ded0943`; revision 2 narrowed the
  collateral rule to non-ownership relations; revision 3 added the
  created-identity remap the frozen S20-345 identity rule requires.
  Implementation, corpus, oracle, and fuzz slice in the commit after the
  staging commit.
- Conformance corpus: `conformance/merge/v1/accepted.json` (thirteen
  three-way cases: identical, disjoint entities, composed members, disjoint
  fields, convergent, fast-forward, metadata overridden, and the six
  conflict reasons field-edit, add-add, delete-edit, kind-edit, collateral,
  metadata-edit) and `rejected.json` (four stored-byte mutations),
  drift-gated by `scripts/generate_merge_fixtures.py --check` in
  `make quick`; `scripts/check_merge_vector.py` re-derives every outcome,
  merged body, conflict entry, canonical conflict bytes, and
  `MergeConflictId` from the compact root descriptions under the frozen
  oracle environment, registered in `make conformance`.
- Native tests: eight `sley-repo` merge tests (deterministic and symmetric
  disjoint plus composed merges over 128 runs, every conflict reason with
  ownership relations proven non-collateral, metadata-only override and
  conflict, empty, fast-forward, and every precondition, the common
  ancestor rule, the conflict codec rejection matrix with every code, a
  two-repository merge committed through the frozen path whose new head
  equals the plan's merged root with the created entity re-identified and
  every reference rewritten, and a conflict that commits nothing);
  `sley-repo` 341 tests and `sley-id` 7 tests pass.
- Persistent fuzz: `fuzz/targets/merge_conflict_decoder.rs` (conflict
  decoder in two lanes plus the ancestor rule), smoke `PASS` over a
  311-seed corpus (`docs/audits/S20_700_MERGE_PERSISTENT_SLICE.md`); with
  it every Section 18.5 required surface has a landed target (fifteen
  targets, fourteen smoke gates, sixteen landed surfaces); the S20-700
  finding register and independent review remain deferred.
- Tier 1: `make quick` green at every commit.
- Tier 2: see the validation record below.

## Findings closed in flight

- The first draft's collateral rule spanned all relations, which made any
  edit of a namespace member conflict with any concurrent membership
  change; revision 2 excludes `Ownership` relations, which the set rules
  compose, and the tests prove ownership alone is never collateral.
- The frozen S20-345 rule derives every `CreateEntity` identity from the
  candidate nonce, so entities added on theirs cannot keep their identity
  in ours' repository; revision 3 adds the deterministic remap with a full
  reference rewriter, and the repository test proves the committed root
  equals the plan's root while the judged root stays symmetric.
- Trusted genesis over identical inputs yields one transaction identity, so
  two repositories built from the same objects share an exact ancestor.

## Explicitly open and deferred

- **Council reviews.** Ariadne, Nabu, and Vulcan reviews are queued behind
  the S20-250 and S20-510 reviews and land as contract revisions.
- The merge judgment is not fuzzed over synthetic roots; the decoder and
  ancestor surfaces are, and the judgment's inputs are the frozen S20-250
  and S20-510 surfaces with their own targets.
- The `Closure` conflict path is defensive: no corpus case reaches it,
  because set composition over complete roots preserved every closure rule
  in every constructed case.
- Restricted S20-360 bounds committable merges to executable-program-
  operation-free roots; the judgment and plan are exact regardless.
- Strict pedantic clippy debt in older `sley-repo` test modules is
  pre-existing; the new paths lint clean under `--no-deps`.

## Validation record

Tier 1 `make quick` passed at every commit of the slice. Tier 2 ran on
2026-09-03 at `4f01b12`: `make core` (934 tests), `make conformance`
(including the merge oracle line), `make adversarial`, `make fuzz-smoke`, and
`make merge-persistent-fuzz-smoke` all exited 0 in 36 seconds. The full
`make v1` gate was skipped because this is a subsystem handoff, not a release
boundary; `make v2` and `make release-check` remain intentionally fail closed.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.
