# Ariadne contract review: section `root_backed_query_profile`, field `ariadne_contract_review` (revision 6 of the S20-310 contract)

Role: Ariadne (contract conformance). Council round at a809906 (2026-09-15).
Read-only session: the only file written is this transcript.

## Scope verification

`git rev-parse HEAD` = `a809906f78f1bfdb9cde8da4c108c4d692dad297`. Match.
`git status --short` = empty (clean tree before this transcript was written).
Commits since the attested candidate 7a94a4a (`git log --oneline 7a94a4a..HEAD`):
f7df74f, 70283ce, 4168332, 161a2f3, 75b5491, 08f577d, e11103e, a809906.
The section-relevant deltas are f7df74f (spec revision 6, new unit walk) and
70283ce (`QueryLimits` is `Copy`, test-only clippy cleanup). Nothing after
70283ce touches `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md`, `crates/sley-query`,
`crates/sley-repo/src/root_query.rs`, `conformance/root-backed-query`, or the
S20-310 scripts (verified with `git show --stat` on each commit).

## Inputs read in full

- `/tmp/claude-sley2/review-brief.md`, `/tmp/claude-sley2/round-a809906-sections.md`
- `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` (revision 6, 514 lines) and
  `git show f7df74f -- docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md crates/sley-query/src/root_query.rs`
- `evidence/review/decision-packets/round-7-root-query-contract.md` (the packet)
- `docs/status/HANDOFF-2026-09-15-QUALIFICATION.md` lines 12-40 (the only
  record of the "2026-09-15 authorization" the revision cites: "closes the
  round-7 wording packet under the 2026-09-15 authorization (sections 3, 7,
  9, 10)"). No separate authorization artifact exists in `docs/` or
  `evidence/`; the authorization's content is therefore the packet's
  "Operator decision needed" paragraph: approve the section 3/7/9/10 wording
  amendments as owner drafts, consistent with the reviewed semantics.
- Prior Ariadne transcripts `ariadne_contract_review-0bcc9c6.md` and
  `ariadne_contract_review-a4b6029.md`; Vulcan `vulcan_surface_review-43f2f5b.md`
  and `-db1bc62.md` for the recorded `accept_cached` disposition.
- `crates/sley-query/src/root_query.rs` (`verify()` 223-292, class arms
  970-1225, `page()` 1275-1480, the new test 1894-2020), `crates/sley-query/src/snapshot.rs`
  (`decode_complete_root_snapshot` 457-478, `inspect_candidate_for_arm` 612-750,
  `invert_edges` 512), `crates/sley-repo/src/index_cache.rs` (`accept_cached`
  108-126, `complete_root_snapshot` 203-232, `verify_cached_snapshot` 278-306),
  `crates/sley-repo/src/root_query.rs` (`run_root_query` 104-147, `run_root_query_fresh` doc).
- `scripts/check_root_backed_query_profile.py`, `docs/WORK_PACKAGES.md` row S20-310
  (line 36), `machineresearch/sley-2.0/machine-summary.json` section
  `root_backed_query_profile`, `evidence/review/finding-register.json` rows
  222-232 of `obligations` and `open_reviews[0]`.
- `conformance/root-backed-query/v1/{accepted,rejected}.json` (vector names,
  paging fields, and mutation ids parsed with Python; hex payloads not re-decoded
  by hand this round because the generator `--check` and the oracle mechanize
  that comparison, see below).
- `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md` lines 33-38, 93-96,
  146-150 (S20-300 hit-authority rule).

## Tool results (exact)

`SLEY2_MASTER_GOAL=/home/greyforge/machineresearch/Sley2.0mastergoal.md` was set for every run.

1. `cargo test -p sley-query --locked`:
   `test result: ok. 104 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 0.21s`
   including `root_query::tests::single_item_classes_walk_two_items_at_limit_one ... ok`,
   `root_query::tests::unsorted_root_committed_fact_sets_are_root_mismatch ... ok`,
   `root_query::tests::class_kind_applicability_table_is_executed ... ok`,
   `root_query::tests::every_class_answers_exact_facts_over_the_fixture ... ok`.
   Doc-tests: 0.
2. `python3 scripts/check_root_backed_query_profile.py` (exit 0):
   `{"contract": "s20-310-full-root-backed-query-profile-v1", "implementation_present": [engine, repository, sley-id:RootQuery, conformance/root-backed-query], "new_stable_error_codes": 3, "problems": [], "query_classes": 19, "result": "PASS", "revision": 6, "status": "S20_310_FULL_IMPLEMENTED_REVIEW_PENDING"}`
3. `python3 scripts/generate_root_backed_query_fixtures.py --check` (exit 0):
   `{"drift": [], "mode": "check", "rejections": 10, "result": "PASS", "vectors": 27}`
4. `uv run --project oracle/scb1 --frozen python scripts/check_root_backed_query_vector.py` (exit 0):
   `{"contract": "s20-310-full-root-backed-query-oracle-v1", "mutations": 10, "problems": [], "query_classes": 19, "result": "PASS", "vectors": 27}`
5. `sha256sum -c SHA256SUMS` in `conformance/root-backed-query/v1`: `accepted.json: OK`, `rejected.json: OK`.
6. Corpus shape (parsed): 27 accepted vectors = `class-01`..`class-19` plus
   walks `page-namespaces-1/2` (entity cursor tag 1, page 1 truncated with
   `next_after`), `page-edges-1/2` (edge cursor tag 2, page 1 truncated),
   `page-roots-1/2` (root cursor tag 3; page 1 `next_after = None`, page 2
   `after` = the single root), `page-entry-points-1/2` (entity cursor; page 1
   `next_after = None`, page 2 `after` = the single entry point). 10 rejections:
   `truncated-without-continuation` 31006, `cursor-wrong-type` 31009,
   `cursor-on-single-key` 31009, `class-not-applicable` 31010,
   `unresolved-entity` 31004, `filter-not-canonical` 31001, `depth-cut` 31006,
   `work-exhausted` 31005, `binding-substituted-fact` 31008,
   `arm-1-snapshot-profile` 31000.
7. `grep -rn 'ListImpactClosure\|ListReverseImpactClosure'` over `*.md *.rs *.py *.json`
   (excluding `target/`): exactly one hit, `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:393`.
8. `sed -n 36p docs/WORK_PACKAGES.md | grep -o 'contract draft revision [0-9] ([0-9-]*)'`
   = `contract draft revision 5 (2026-09-13)`. Machine summary
   `root_backed_query_profile.contract_revision` = 6; spec status line = revision 6.

## Independently re-derived claims

### The packet's four P1 wording items versus revision 6

**Item 1 (A-P1-5 / N-P1-2 / V-P1-2), section 3 page-set acceptance rule — implemented as authorized.**
Spec lines 256-267 state the two acceptance rules: standalone complete iff
`after = None` and `truncated = false`; page set complete iff identical
requests except `after`, first `after = None`, chained `after == prev.next_after`,
every non-last page `truncated = true`, last page `truncated = false` and
`next_after = None`, same `total_count` on every page, `sum(returned) == total_count`.
This is exactly the packet's predicate. Engine agreement: `page()` sets
`total_count` to the exact complete count on every page (`page_items` returns
`total` independent of the cursor), `truncated = returned < items after the
cursor`, `next_after = key of the last returned item` only when truncated and
`None` otherwise (root_query.rs:1327-1339, 1428-1447, 1450-1475), and keys are
unique and strictly increasing per key type (entity ids, `(dependent,
dependency, kind)` triples, `StateRoot` values; `verify()` 267-271 forces the
record-supplied sets of classes 10/11 to be strictly increasing). Under those
facts the page-set rule is sound: the union of a chained walk is the complete
result and the sum of `returned` equals the total. The corpus walks
`page-namespaces` and `page-edges` are valid page sets under the rule; the
degenerate `page-roots` and `page-entry-points` walks are not page sets (page
one is already a complete standalone response), which section 9 states
honestly ("page one complete, page two empty past the end").

**Item 2 (A-P1-6 / V-P1-3), section 9 one-walk-per-cursor-key-type — implemented as authorized (the "weaken" option, with the unit-walk supplement).**
Spec lines 452-461. The fixture carries one walk per key type: entity
(`page-namespaces`, truncated arm exercised; `page-entry-points`, degenerate),
edge-triple (`page-edges`, truncated arm exercised), root (`page-roots`,
degenerate). The truncated arms of `Roots` and `EntryRows` are pinned by
`single_item_classes_walk_two_items_at_limit_one` (root_query.rs:1894-2020),
which drives the paging layer `page()` directly over a two-item complete result
at `max_returned_entities = 1`: page one `(total 2, returned 1, truncated true,
next_after = Some(first key))`, page two `(2, 1, false, None)`, union equals
the input in order. The test passes. The test's own doc comment explains why
the fixture cannot carry a second dependency root (`RootDependencyRootsMismatch`
at the S20-250 judgment), which is the justification section 9 gives. The
rejection ids `truncated-without-continuation`, `cursor-wrong-type`, and
`depth-cut` are present, as the bullet requires. Minor wording note (not a
finding): section 9 says "over a two-item input"; the test pages a two-item
*complete result* against a request built over the fixture input. The
meaning is recoverable from the file it names.

**Item 4 (A-P1-7 / N-P1-5), section 10 chosen enumeration — implemented as authorized.**
Spec lines 490-496: nineteen classes are a chosen closed enumeration; class
20 additive only; no class number, cursor tag, or key type reallocated (tag 3
root cursor, class 18 unchanged); empty results for 16, 17, 19 lawful with
`total_count = 0`. Engine agreement: classes 16/17/19 (root_query.rs:1166-1215)
build a possibly empty `Vec` and return `RootQueryResult::Entities`, which
`page()` pages to `total_count = 0, returned = 0, truncated = false` with no
failure arm; `key_tag()` (402-413) keeps the cursor tag mapping.

**Item 3 (V-P1-4 / N-P1-3 / A-P2-1), section 7 cache-derived versus record-derived facts — NOT exactly as authorized; see findings 1-3.**
What the packet authorized: name `direct_edges` plus classes 12-15 as
cache-derived, the rest record-derived, "pointing at `verify_cached_snapshot`
byte-rebuild audit", with the recorded facts that the decode layer already
enforces inversion and that body comparison in the hot `accept_cached` path is
declined. What revision 6 says (spec lines 389-406) and what the code does:

- "A hit supplies exactly the derived edge facts: `direct_edges` and their
  inverse, which answer the edge classes 12 through 15 (...) and nothing else."
  Code: `GetRootSummary` reads `direct_edges: input.snapshot.direct_edges().len()`
  (root_query.rs:995). The class-1 direct-edge count is a cache-supplied answer
  fact on a hit. "Nothing else" is false; section 2 line 147 itself lists "the
  snapshot's direct-edge count" as a class-1 answer. (Finding 2.)
- The four names in parentheses: section 2 names class 14 `ReverseImpactClosure`
  and class 15 `ForwardDependencyClosure`; `ListImpactClosure` and
  `ListReverseImpactClosure` exist nowhere else in the repository (tool result 7).
  (Finding 3.)
- "A hit is admitted only through `decode_complete_root_snapshot`, whose
  decode-time audit rebuilds the inverse edge groups from the direct edges and
  refuses a snapshot whose groups differ (`verify_cached_snapshot`, the
  byte-rebuild audit of S20-300), and the section 1 binding is re-checked on
  every hit, so a cached inventory can never disagree with the record and a
  forged edge set cannot reach the engine."
  Code: the hit path is `complete_root_snapshot` (index_cache.rs:203-216) ->
  `accept_cached` (108-126) -> `decode_complete_root_snapshot` (snapshot.rs:457)
  -> `inspect_candidate_for_arm` (612-750), which checks format, context, arm,
  the self-digest trailer (`IndexSnapshotId::derive(preimage) == trailer`,
  743-745) and then `decoded_reverse != invert_edges(&direct)` (746-748); then
  `accept_cached` aligns the inventory identities with the record's
  `entity_bindings`. The decode-time inversion audit is real and correctly
  described. But `verify_cached_snapshot` (index_cache.rs:278-306) is a
  separate function that rebuilds from the revision and compares bytes; it is
  called only from tests (all call sites 345-548 are inside `mod tests`) and is
  not on the hit path. The parenthetical attributes the decode-time audit to
  it, which is wrong, and the packet's instruction was to point at it as the
  byte-rebuild audit, distinct from the decode layer's inversion check.
  Consequently "a forged edge set cannot reach the engine" is stronger than the
  mechanism: the decode audit refuses an *inconsistent* edge set (inverse
  groups disagreeing with direct edges, or a digest that does not match the
  bytes), but a self-consistent forged record whose trailer was recomputed
  passes decode and inventory alignment and reaches the engine on a hit. That
  is the reviewed and recorded design: S20-300 section text "a cache hit is
  accepted only under the repository authority that wrote it, only for
  read-only derived query surfaces" and "exported evidence MUST NOT rest on a
  bare hit" (COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:35-37, 148-150);
  `run_root_query` doc "a cache hit is accepted without re-deriving edges, so
  only a fresh build ... may underwrite anything that leaves the process"
  (sley-repo/src/root_query.rs); Vulcan 43f2f5b:96 "the trust-on-cache-dir
  boundary for a fully self-consistent forged record is unchanged ... and is by
  design"; and the packet itself: "body comparison in hot `accept_cached` is
  declined without a cache-authority change." Revision 6 therefore states a
  fail-closed property the hit path does not provide, contrary to the
  authorization's condition "consistent with the reviewed semantics".
  (Finding 1.) The profile also never names `run_root_query_fresh`, the path
  S20-300 requires for exported evidence; the rewrite should.
- "kinds ... come from the verified objects ... never from the cache": the
  engine reads the class-2 kind from `snapshot.inventory()[index].kind`
  (root_query.rs:1004), but `verify()` forces `entity.kind() == entry.kind`
  for every inventory entry (243-247), so the answered value is
  record-determined. Not a finding; noted for precision.

### Text, code, vectors, oracle, checker agreement (beyond the four items)

- Section 1 rules 1-5 versus `verify()` (223-292): rule 2 inventory/bindings/
  bound_entities identity and kind equality (236-247), rule 3 entities in raw
  order (244-246), rule 4 fingerprints subset with TypeDef/Function kinds
  (249-258), rule 5 nine-field `StateRootRecord` recompute (276-291) plus the
  strictly-increasing guard on entry points and dependency roots (267-271).
  Agree. Arm-first gating at both entry points is unchanged since a4b6029.
- Section 3 paging versus `page()`: entity limit applies to entities,
  dependency rows, inventory entries, entry rows, and roots; edge limit to
  edges; single-key `Entities` (class 9) never pages and fails
  `RequiredFactOmitted` over the limit (1310-1323). Agree.
- Section 9 evidence list versus corpus: 19 class vectors, four walks, ten
  rejections including 31008 and 31000; generator `--check` drift `[]`; oracle
  PASS 27/10. Agree.
- Checker: markers, codes, class table 1..19, summary keys, engine/repository
  markers all PASS; it reports `revision: 6` but still does not compare the
  spec revision to `machine-summary.contract_revision` (script lines 218-222,
  unchanged since the a4b6029 P3). The two agree today by inspection, not by gate.
- Records: `machine-summary.contract_revision` = 6 (the a4b6029 P3 on the
  summary is closed). `docs/WORK_PACKAGES.md:36` still reads "revision 5
  (2026-09-13)" (the same class of record drift the a4b6029 P3 named for
  revision 4 versus 5; f7df74f reconciled the completed packages' rows and
  left this open package's row stale). Register row 225 `contract_text_review`
  is PENDING with disposition
  `PENDING_S20_310_WORDING_DECISION_PACKET_ROUND7_P1_4_IMPLEMENTED`; rows
  222/226/230 carry the a4b6029 lane PASS values; consistent with the round
  brief.

### Carried editorial items from a4b6029 (re-checked at HEAD)

- Section 3 lines 248-254: the page-union sentence remains duplicated verbatim
  inside the section revision 6 amended. Harmless; still present.
- Section 1 rule 1 (lines 83-84) still lists `CompleteRoot(2)` inside the
  numbered `QUERY_ROOT_MISMATCH` list while the prose (76-81) assigns arm-1 to
  precedence item 2; `verify()` doc comment (211-222) and its first check
  (224-229) mirror the residue. Unreachable via the gated entry points; outcome
  unchanged. Section 1 was not among the authorized sections, so this needs its
  own wording authorization; listed so it is not lost before freeze.

## Per-item analysis summary

| Packet item | Revision 6 | Verdict |
|---|---|---|
| 1. Section 3 page-set acceptance rule | lines 256-267; engine `page()` agrees | as authorized |
| 2. Section 9 one-walk-per-cursor-key-type + unit walk | lines 452-461; corpus 4 walks; test passes | as authorized |
| 3. Section 7 cache-derived vs record-derived, byte-rebuild audit | lines 389-406 | not as authorized: wrong audit attribution and an overstated forgery claim (P2); class-1 omission (P3); wrong class names (P3) |
| 4. Section 10 chosen enumeration | lines 490-496; classes 16/17/19 empties lawful in code | as authorized |

Three of the four items are implemented exactly as authorized and agree with
code, vectors, oracle, and checker. Item 3 is the one that must be repaired
before this lane can pass; the repair is text-only (no code change is implied,
which is what the packet recorded).

## Findings

[P2] [contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:397-403 - section 7 attributes the decode-time inverse-group audit to `verify_cached_snapshot` and concludes "a forged edge set cannot reach the engine"; the hit path (`complete_root_snapshot` -> `accept_cached`, crates/sley-repo/src/index_cache.rs:203-216, 108-126) runs only `decode_complete_root_snapshot` (self-digest trailer + inversion consistency, crates/sley-query/src/snapshot.rs:743-748) and inventory alignment, `verify_cached_snapshot` (index_cache.rs:278-306) is an off-path audit called only from tests, and a self-consistent forged record reaches the engine on a hit by recorded design (S20-300 hit-authority rule; Vulcan 43f2f5b:96; the packet's own "body comparison in hot accept_cached is declined"). Rewrite: the decode-time audit refuses an inconsistent edge set; the byte-rebuild audit `verify_cached_snapshot` (off-path, Tier 2 evidence) and the fresh path `run_root_query_fresh` are the defenses against a self-consistent forgery; a hit is trusted only under the repository authority that wrote it and exported evidence never rests on a bare hit.
[P3] [contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:390-393 - "and nothing else" omits the class-1 `GetRootSummary` direct-edge count, which is read from the snapshot (crates/sley-query/src/root_query.rs:995) and which section 2 line 147 lists as a class-1 answer; name it as the third cache-derived answer fact.
[P3] [contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:392-393 - classes 14 and 15 are named `ListImpactClosure` and `ListReverseImpactClosure`, names that appear nowhere else in the repository; section 2 names them `ReverseImpactClosure` (14) and `ForwardDependencyClosure` (15).
[P3] [record] docs/WORK_PACKAGES.md:36 - S20-310 row states "implemented under contract draft revision 5 (2026-09-13)"; the spec and `machine-summary.contract_revision` are at revision 6 (2026-09-15).
[P3] [checker] scripts/check_root_backed_query_profile.py:218-222 - the spec revision is extracted and reported but never asserted equal to `machine-summary.root_backed_query_profile.contract_revision`; the summary agrees today only by hand (carried from a4b6029; the packet's item 9 defers stage-checker binding to governance work, but the gap remains open and the WORK_PACKAGES drift above shows the pattern).
[P4] [contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:248-254 - the page-union sentence is still duplicated verbatim inside the section revision 6 amended; remove one copy when section 7 is repaired.
[P4] [contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:83-84 (with crates/sley-query/src/root_query.rs:211-229) - section 1 rule 1 keeps the arm condition inside the `QUERY_ROOT_MISMATCH` list while lines 76-81 assign arm-1 to precedence item 2; outcome-consistent and unreachable via the gated entry points; needs its own wording authorization (section 1 was outside the round-7 grant).

```
VERDICT: REVISE_0_P0_0_P1_1_P2_4_P3_2_P4
SECTION: root_backed_query_profile
FIELD: ariadne_contract_review
SCOPE_SHA: a809906f78f1bfdb9cde8da4c108c4d692dad297
FINDINGS:
[P2] [contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:397-403 - section 7 attributes the decode-time inverse-group audit to `verify_cached_snapshot` and claims "a forged edge set cannot reach the engine"; the hit path runs only `decode_complete_root_snapshot` (self-digest + inversion consistency) plus inventory alignment, `verify_cached_snapshot` is an off-path audit called only from tests, and a self-consistent forged record reaches the engine on a hit by recorded design (S20-300 hit-authority rule, Vulcan 43f2f5b, the packet's declined accept_cached comparison); rewrite to separate the decode-time audit from the byte-rebuild audit and name `run_root_query_fresh` as the exported-evidence path
[P3] [contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:390-393 - "and nothing else" omits the class-1 direct-edge count, a cache-supplied answer fact (root_query.rs:995; section 2 line 147)
[P3] [contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:392-393 - classes 14/15 named `ListImpactClosure`/`ListReverseImpactClosure`, which section 2 does not define; use `ReverseImpactClosure` (14) and `ForwardDependencyClosure` (15)
[P3] [record] docs/WORK_PACKAGES.md:36 - S20-310 row states revision 5 (2026-09-13); spec and machine summary are at revision 6
[P3] [checker] scripts/check_root_backed_query_profile.py:218-222 - spec revision reported but never asserted against machine-summary contract_revision (carried from a4b6029)
[P4] [contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:248-254 - page-union sentence still duplicated verbatim in the amended section 3
[P4] [contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:83-84 - section 1 rule 1 keeps the arm condition in the QUERY_ROOT_MISMATCH list while lines 76-81 assign arm-1 to item 2 (with the root_query.rs:211-229 doc residue); outcome-consistent; needs its own wording authorization
SUMMARY: At a809906 the S20-310 revision-6 text implements three of the packet's four authorized wording items exactly and consistently with the engine, corpus, oracle, and checker: the section 3 page-set acceptance rule matches page()'s exact-total/strictly-increasing-key behavior, the section 9 one-walk-per-cursor-key-type rule is met by the four fixture walks plus the passing paging-layer unit walk that pins the truncated arms of classes 10 and 11, and the section 10 chosen-enumeration statement matches the code's lawful empty results for classes 16, 17, and 19; cargo test (104 passed), the profile checker (PASS, revision 6), the generator --check (27 vectors, 10 rejections, drift []), the oracle (PASS), and the corpus digests all agree. Section 7 is not as authorized: it misattributes the decode-time inverse-group audit to the off-path verify_cached_snapshot and states that a forged edge set cannot reach the engine, which the hit path does not guarantee and which contradicts the S20-300 hit-authority rule, the sley-repo doc, the prior Vulcan record, and the packet's own declined accept_cached comparison; the same sentence omits the cache-derived class-1 direct-edge count and names classes 14/15 by names section 2 does not define. Those are text repairs with no code change implied; the WORK_PACKAGES row and the checker's revision binding are the remaining record/gate follow-ups, and two carried P4 editorial items stay open. The lane returns to PASS once section 7 is rewritten to the reviewed semantics and the records are aligned.
```
