# Decision packet — root-query profile contract-text lanes (round 7)

Baseline `5a3caf1`. Owner-lane investigation authorized; contract or
governance changes are NOT authorized under the current order.

## Implemented (minimum-scope, fail-closed, clear normative intent)

- **V-P1-1 facts ordering** (`crates/sley-query/src/root_query.rs`,
  `verify()`): `entry_points` and `dependency_roots` must be strictly
  increasing, else `QUERY_ROOT_MISMATCH` (31_008, no new code). The
  recompute binds order only implicitly; an unsorted-but-self-consistent
  pair previously passed while classes 10/11 emit verbatim pages the
  cursor predicate can silently truncate. Regression test
  `unsorted_root_committed_fact_sets_are_root_mismatch` added. All 23
  accepted + 8 rejected vectors and `check_root_backed_query_profile`
  still PASS: no honest-path regression.

## Requires contract text (no code change on either decision)

1. **A-P1-5 / N-P1-2 / V-P1-2 page-stitching predicate**: §3 needs the
   acceptance rule (complete standalone iff `after=None && !truncated`;
   page-set rule with shared snapshot/root/epoch/workspace/limits,
   chained `after==prev.next_after`, last `!truncated`,
   `sum(returned)==total`). Engine behavior agreed correct.
2. **A-P1-6 / V-P1-3 cursor walks**: add class-11 multi-page accepted
   vectors (sorted + unsorted-rejection once the ordering rule above
   lands in the contract) or weaken §9 to one-walk-per-cursor-key-type.
3. **A-P1-1 / N-P1-1 arm-1 codes**: split §1 rule 1 (arm → `31000`
   prec.2, epoch/root → `31008` prec.5). Implementation already splits.
4. **A-P1-2 precedence-3**: split into noncanonical / cursor-key-type /
   identity-drift steps matching engine order shape → cursor → drift.
5. **A-P1-3 class 9**: carve class 9 out of "every list reordered by raw
   key" (chain order preserved by principle).
6. **A-P1-4 paging keys**: name per-class keys + strict-increase/unique
   invariant in §2 (implementation half landed above).
7. **V-P1-4 / N-P1-3 / A-P2-1 cache wording**: §7 must name
   `direct_edges` + classes 12-15 cache-derived, rest record-derived,
   pointing at `verify_cached_snapshot` byte-rebuild audit. Decode layer
   already enforces inversion; body comparison in hot `accept_cached`
   is declined without a cache-authority change.
8. **A-P1-7 / N-P1-5 enumeration**: state in §10 that 19 is chosen not
   derived, class 20 is additive, empties for 16/17/19 are lawful. Tag 3
   and class 18 are not reallocated.
9. **N-P1-6/7 governance**: freeze sequencing + stage-checker gaps
   (`check_root_backed_query_profile` contract-revision/vector-binding
   assertions) are scripts/docs/governance work.

## Operator decision needed

Approve the §3/§7/§9/§10 wording amendments (S20-310 owner drafts), or
decline with the consequence that the S20-320 T14 gate stays
uncheckable and the listed FAILs stand.
