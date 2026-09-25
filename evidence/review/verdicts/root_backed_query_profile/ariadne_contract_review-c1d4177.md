# Ariadne contract review: section `root_backed_query_profile`, field `ariadne_contract_review` (revision 7 of the S20-310 contract, delta round)

Role: Ariadne (contract conformance). Delta round at c1d4177 (2026-09-15)
over my a809906 verdict (`ariadne_contract_review-a809906.md`,
`REVISE_0_P0_0_P1_1_P2_4_P3_2_P4`). Read-only session: the only file
written is this transcript.

## Scope verification

`git rev-parse HEAD` = `c1d41778ea35d04b54b2b27c7594715b1d905b0e`. Match.
`git status --short` at start = empty. `git diff c1d4177..HEAD --stat` =
empty (HEAD had not advanced). At the end of the session the tree carried
one untracked file, `nabu_architecture_review-c1d4177.md`, written
concurrently by the Nabu lane; no tracked file changed during this session.

Commits since a809906: cebc47c (records), 40dbbd0 (S20-710, unrelated),
f02f8de (revision 7, the delta under review), 52c3389 (merge), c1d4177
(machine-summary `contract_revision` 6 -> 7 and the `contract_text_review_note`
rewording; nothing else). `git show f02f8de --stat`: six files,
`crates/sley-query/src/root_query.rs`, `docs/WORK_PACKAGES.md`,
`docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md`,
`docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md`,
`evidence/review/decision-packets/round-7-root-query-contract.md`,
`scripts/check_root_backed_query_profile.py`. No change under
`crates/sley-repo`, `conformance/root-backed-query`, or the oracle.

## Inputs read in full

- `/tmp/claude-sley2/review-brief.md`, `/tmp/claude-sley2/delta-s20-310-c1d4177.md`.
- My a809906 transcript; `git show f02f8de` (full diff); `git show c1d4177`.
- `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` at c1d4177 (542 lines, revision 7).
- `crates/sley-repo/src/index_cache.rs` 100-320 (`accept_cached` 108-126,
  `fresh_snapshot` 132-146, `complete_root_snapshot` 203-232,
  `verify_cached_snapshot` 278-306, test call sites 345-548).
- `crates/sley-query/src/snapshot.rs` 450-520 (`decode_complete_root_snapshot`
  457-478, `admit_index_snapshot` 486-510, `invert_edges` 512) and 605-760
  (`inspect_candidate_for_arm` 612-750).
- `crates/sley-repo/src/root_query.rs` 60-200 (`run_root_query` 104-137,
  `run_root_query_fresh` 152-187, `run_context_capsule` 203).
- `crates/sley-query/src/root_query.rs` 211-300 (`verify()` and its new doc
  comment), 690-710 and 900-925 (both entry points), 990-1025 (class 1/2
  arms), 1284-1460 (`page()` arms), 1900-2160 (the extended unit walk).
- `scripts/check_root_backed_query_profile.py` (the diff plus lines 1-42,
  170-215, 277-290 at HEAD).
- `docs/WORK_PACKAGES.md` line 36; `docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md`
  lines 1-130; `evidence/review/decision-packets/round-7-root-query-contract.md`
  Disposition; `machineresearch/sley-2.0/machine-summary.json` section
  `root_backed_query_profile`; `evidence/review/finding-register.json`
  obligations for the section and `open_reviews`.
- `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md` 30-40, 90-98, 144-153
  (S20-300 hit-authority rule and `verify_cached_snapshot` sentence).
- `conformance/root-backed-query/v1/accepted.json` (vector ids, queries,
  cursors; class-07 and class-08 records decoded for `total_count`),
  `rejected.json` (mutation count).
- `docs/spec/FINDING_REGISTER_V1.md` (state/disposition grammar) and the
  Council rules grep for "actionable"/"authorization" (no rule on
  out-of-grant items beyond the brief's own).

## Tool results (exact)

`SLEY2_MASTER_GOAL=/home/dev/machineresearch/Sley2.0mastergoal.md`
was set for every run.

1. `python3 scripts/check_root_backed_query_profile.py` (exit 0):
   `{"contract": "s20-310-full-root-backed-query-profile-v1", "expected_revision": 7, "implementation_present": [crates/sley-query/src/root_query.rs, crates/sley-repo/src/root_query.rs, sley-id:RootQuery, conformance/root-backed-query], "machine_summary_revision": 7, "new_stable_error_codes": 3, "problems": [], "query_classes": 19, "result": "PASS", "revision": 7, "status": "S20_310_FULL_IMPLEMENTED_REVIEW_PENDING"}`
2. `python3 scripts/generate_root_backed_query_fixtures.py --check` (exit 0):
   `{"drift": [], "mode": "check", "rejections": 10, "result": "PASS", "vectors": 27}`
3. `uv run --project oracle/scb1 --frozen python scripts/check_root_backed_query_vector.py` (exit 0):
   `{"contract": "s20-310-full-root-backed-query-oracle-v1", "mutations": 10, "problems": [], "query_classes": 19, "result": "PASS", "vectors": 27}`
4. `sha256sum -c SHA256SUMS` in `conformance/root-backed-query/v1`:
   `accepted.json: OK`, `rejected.json: OK`.
5. `cargo test -p sley-query --locked`:
   `test result: ok. 104 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 0.20s`
   including `root_query::tests::single_item_classes_walk_two_items_at_limit_one ... ok`,
   `unsorted_root_committed_fact_sets_are_root_mismatch ... ok`,
   `class_kind_applicability_table_is_executed ... ok`,
   `every_class_answers_exact_facts_over_the_fixture ... ok`. Doc-tests 0.
6. `grep -rn 'ListImpactClosure\|ListReverseImpactClosure'` over `*.md *.rs
   *.py *.json` excluding `target/` and `evidence/review/verdicts`: no hits.
7. `grep -rn verify_cached_snapshot --include=*.rs crates/` excluding
   `crates/sley-repo/src/index_cache.rs`: no hits. Inside that file every
   call site (345, 442, 481, 486, 495, 521, 548) is below `mod tests` (307).
8. `sed -n 36p docs/WORK_PACKAGES.md | grep -o 'contract draft revision [0-9] ([^)]*)'`
   = `contract draft revision 7 (2026-09-15; revision 6 of the same day closed
   the round-7 wording packet under the 2026-09-15 operator authorization, and
   revision 7 repairs the a809906 council round's section 7 attribution, class
   names, and class-1 count, the section 3 duplicate, and the section 9
   evidence rule)`.
9. Machine summary at c1d4177: `contract_revision` = 7,
   `status` = `S20_310_FULL_IMPLEMENTED_REVIEW_PENDING`,
   `ariadne_contract_review` = `PASS_WITH_P3_P4_FOLLOWUPS_NO_P0_P1_P2` (the
   a4b6029 base value, held pending this delta), `contract_text_review` =
   `PENDING_S20_310_WORDING_DECISION_PACKET_ROUND7_P1_4_IMPLEMENTED`.
   Register: the section's `contract_text_review` row is the only
   `open_reviews` entry; the lane rows carry the base values.
10. Corpus (parsed): 27 accepted = `class-01`..`class-19` +
    `page-namespaces-1/2` (class 4, kind 3, entity cursor tag 1),
    `page-edges-1/2` (class 12, edge cursor tag 2), `page-roots-1/2` (class
    11, root cursor tag 3), `page-entry-points-1/2` (class 10, entity
    cursor). No vector walks class 8. Record decode: `class-07`
    `total_count = 1, returned = 1, truncated = false`; `class-08`
    `total_count = 9, returned = 9, truncated = false`. 10 mutations.
11. Checker negative test (scratchpad shadow tree: symlinks to the
    repository plus copied spec, machine summary, and checker; the
    repository was not modified, `git status` confirmed):
    A. unmodified copies: `PASS []`;
    B. spec status line reverted to "revision 6": `FAIL
    ['spec-revision:6!=7', 'machine-summary:contract_revision:7!=spec:6']`;
    C. section 7 class names reverted to the revision-6 names: `FAIL
    ['spec-revision-marker:and those facts answer exactly the edge classes 12 through 1']`;
    D. machine-summary `contract_revision` set to 6: `FAIL
    ['machine-summary:contract_revision:6!=spec:7']`;
    E. section 9 unit-walk sentence reflowed (whitespace only): `PASS []`.

## Independently re-derived claims: section 7 (revision 7, lines 394-428) versus the code

Every sentence of the rewritten paragraph was checked against the four
files the task names.

- "A hit supplies the derived edge facts, `direct_edges` and their inverse,
  and those facts answer exactly the edge classes 12 through 15
  (`ListDirectDependencies`, `ListDirectDependents`, `ReverseImpactClosure`,
  `ForwardDependencyClosure`) and the direct-edge count that
  `GetRootSummary` (class 1) reports; no other answer fact comes from a
  hit." The names are section 2's (table rows 14 and 15, lines 141-142;
  the old names exist nowhere outside verdict transcripts, tool result 6).
  Class 1 reads `input.snapshot.direct_edges().len()` (root_query.rs:1004).
  Classes 12-15 read the snapshot's direct edges and reverse groups. No
  other class arm reads snapshot edges. Agree. (Closes a809906 P3 x2.)
- "the inventory a hit carries is read for identities and kinds, but the
  section 1 binding, re-checked on every request, forces each of them equal
  to the verified objects', so those answers are record-determined."
  `verify()` (root_query.rs:232-292) compares every inventory entry's
  identity to `bindings[index].0`, `facts.bound_entities[index]`, and
  `entities[index].entity_id()`, and its kind to `entities[index].kind()`
  (249-252). `verify()` is called from both entry points,
  `build_root_query_request` (705) and `execute_root_query` (923), after
  the arm gate (701, 906). Class 2 reads
  `input.snapshot.inventory()[index].kind` (1013), which the binding has
  forced equal to the body's kind. Agree.
- "The hit-path admission audit is the decode-time audit inside
  `decode_complete_root_snapshot`: the hit path (`complete_root_snapshot`
  -> `accept_cached` -> decode, then alignment of the cached inventory
  identities with the record's `entity_bindings`, in
  `crates/sley-repo/src/index_cache.rs`) checks the format, context, and
  arm, authenticates the record's self-digest trailer, rebuilds the inverse
  edge groups from the direct edges, and refuses a snapshot whose groups
  disagree." `complete_root_snapshot` (index_cache.rs:203-232) reads the
  cache file and calls `accept_cached` (108-126), which calls
  `decode_complete_root_snapshot(context_for(revision), record)` (112) and
  then aligns `snapshot.inventory()` with
  `revision.state_root().record.entity_bindings` by length and identity
  (113-119, `RootMismatch` otherwise). `decode_complete_root_snapshot`
  (snapshot.rs:457-478) calls `inspect_candidate_for_arm` (612-750), which
  checks magic/format/profile version (629-634), schema epoch, field schema
  hash, limits profile, and claimed root context (636-651), the arm
  (652-662), then structural decoding, then the trailer
  (`IndexSnapshotId::derive(preimage).as_bytes() != trailer`, 743-745,
  `DigestMismatch`) and the inversion (`decoded_reverse !=
  invert_edges(&direct)`, 746-748, `FormatInvalid`). Agree, exactly.
- "That audit proves the internal consistency of the cached edge set, not
  its agreement with the objects, and this profile claims no more: under
  the S20-300 hit-authority rule a self-consistent cached edge set is
  authoritative on a hit, accepted only under the repository authority that
  wrote it and only for this read-only derived query surface." Nothing on
  the hit path reads an object or re-derives an edge (`accept_cached` doc
  comment: "Reads no object and extracts no edge"). The S20-300 text
  (COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:33-38) says "a cache hit is
  accepted only under the repository authority that wrote it, only for
  read-only derived query surfaces, and never as input to validation,
  comparison, merge, commit, or recovery", and 146-150 restricts hit
  consumption to "transient in-process reads on the read-only derived
  query surfaces (S20-310 root-backed queries)". Agree. The a809906 P2
  claim ("a forged edge set cannot reach the engine") is gone; the
  replacement states the reviewed semantics. (Closes a809906 P2.)
- "The byte-rebuild audit `verify_cached_snapshot` (S20-300) rebuilds the
  snapshot from the revision's objects and compares the cached record byte
  for byte; it is a separate off-path audit for Tier 2 evidence that the
  hit path never calls." `verify_cached_snapshot` (index_cache.rs:278-306)
  calls `fresh_snapshot` and compares `record == fresh.record()`; the
  S20-300 text (152-153) names it for "audits and Tier 2 evidence". It has
  no caller outside `index_cache.rs`'s test module (tool result 7) and
  `complete_root_snapshot` does not call it. Agree.
- "Exported evidence never rests on a bare hit: `run_root_query_fresh`
  (`crates/sley-repo/src/root_query.rs`) answers from a fresh rebuild and
  is the exported-evidence path." `run_root_query_fresh` (152-187) calls
  `fresh_snapshot` and never `complete_root_snapshot`; `run_context_capsule`
  (203) builds on it; the `run_root_query` doc (94-98) sends exported
  evidence there. Agree.
- The closing authority sentence is unchanged from revision 6 and agrees
  with S20-300 and ADR-0030's boundary.

Precision note, not a finding: "Two audits apply to a cached record" is
scoped to this profile's repository surface. S20-300 also keeps the
rebuild-first admission API (`admit_index_snapshot`, snapshot.rs:486-510),
which compares a candidate to a fresh build; it is conformance and
security evidence for S20-300, not a path the S20-310 surface calls, and
section 7 does not need to name it.

## Independently re-derived claims: the rest of the delta

- Section 3 (lines 254-259): the page-union sentence appears once, with the
  same predicate as before ("because `total_count` is exact on every page
  and the key order is canonical, keys strictly increase across the walk,
  the union of the pages is the complete result, and no page can hide a
  fact"). The page-set acceptance rule (261-272) is unchanged from
  revision 6 and still matches `page()`. (Closes a809906 P4 duplicate.)
- Section 9 (lines 474-489): "one dependency root for `ListDependencyRoots`,
  one entry point for `ListEntryPoints`, one package dependency for
  `ListPackageDependencies`" — class-07 `total_count = 1` (tool result 10);
  the class-10 and class-11 single items were verified at a809906. "a
  fixture walk is degenerate at best" — classes 10 and 11 have degenerate
  walks, class 7 has none; "at best" covers both. The unit walk
  `single_item_classes_walk_two_items_at_limit_one` (root_query.rs:1920-2160)
  builds both requests of every walk through `build_root_query_request`
  over the real fixture with `max_returned_entities: 1` (1924-1927) and
  pages a synthetic two-item `Complete` through `page()` for `Roots`,
  `EntryRows`, `DependencyRows` (class 7 over package `id(0x03)`, key
  `row.binding`, `Cursor::Entity`; `page()` arm 1361-1380), and
  `InventoryEntries` (class 8 over namespace `id(0x04)`, key
  `entry.entity`; arm 1387-1406), asserting `(2, 1, true, Some(first key))`
  then `(2, 1, false, None)` and the ordered union. The test passes (tool
  result 5). "the last for `ListNamespaceMembers`, whose fixture result no
  vector walks" — `page-namespaces` is class 4 (`ListEntitiesByKind`, kind
  3), so no vector walks class 8 (tool result 10). With the fixture walks
  covering the `Entities` (class 4) and `Edges` (class 12) arms, every
  paged result arm of `page()` now has truncated-emission evidence. Agree.
- Section 1 / `verify()` doc comment (root_query.rs:211-231): the new
  carve-out paragraph states that both entry points route arm disagreement
  through `QUERY_PROFILE_UNSUPPORTED` before `verify()` runs and that the
  completeness comparison in `verify()` is a defensive residue. That is
  what the code does (701/705, 906/923) and what section 1 lines 82-87 say.
  Agree.
- Checker: `CONTRACT_REVISION = 7`; the status-line regex is anchored
  (`^Status: S20-310 full contract draft, revision (\d+)`, `re.M`) instead of
  the old first-`revision (\d+)` match; the spec revision is asserted equal
  to 7 and the machine summary's `contract_revision` asserted equal to the
  spec's; `SPEC_REVISION_MARKERS` bind the section 3/7/9/10 paragraphs
  against the whitespace-flattened spec; `ENGINE_MARKERS` gains the unit
  walk's function name and `REPOSITORY_MARKERS` gains `pub fn
  run_root_query_fresh`. The negative test (tool result 11) shows the gate
  is real: a revision revert, a class-name revert, and a summary drift each
  fail, and a whitespace reflow passes. (Closes a809906 P3 checker.)
- Records: WORK_PACKAGES row 36 names revision 7 (2026-09-15) with the
  dated history (tool result 8; closes a809906 P3 record). Machine summary
  `contract_revision` = 7 at c1d4177 (the checker would have failed at
  f02f8de with the 6 != 7 drift the commit message predicted, and the
  summary pin closes it). The packet Disposition records the operator
  authorization and the revision-7 repair, and states that section 1 rule
  1 and ADR-0030 were outside the grant. The closeout Status line and first
  Evidence bullet name revision 7 with the history.

## Per-item analysis: the a809906 list

| a809906 item | Revision 7 | Status |
|---|---|---|
| P2 section 7 audit attribution and forged-edge-set claim | lines 406-424 separate the decode-time audit (hit path, internal consistency) from `verify_cached_snapshot` (off-path, Tier 2); the forgery claim is replaced by the S20-300 hit-authority rule; `run_root_query_fresh` named | closed, verified against index_cache.rs, snapshot.rs, both root_query.rs |
| P3 class-1 direct-edge count omitted | lines 398-400 | closed (root_query.rs:1004) |
| P3 classes 14/15 misnamed | lines 397-398 | closed (section 2 names; no stray names) |
| P3 WORK_PACKAGES row 36 at revision 5 | revision 7 with history | closed |
| P3 checker never asserts summary revision | `CONTRACT_REVISION` and the two assertions; negative-tested | closed |
| P4 section 3 duplicated sentence | one sentence at 254-259 | closed |
| P4 section 1 rule 1 arm residue | untouched by design (outside the grant) | open, see below |

### The carried rule-1 residue (moved to prose)

Section 1 lines 82-87 state the carve-out ("A snapshot whose completeness
is not `CompleteRoot(2)` is not a binding failure ... fails
`QUERY_PROFILE_UNSUPPORTED` (precedence item 2), never
`QUERY_ROOT_MISMATCH`"), while rule 1 (line 89) still lists
`snapshot.completeness = CompleteRoot(2)` inside the numbered list that
the sentence "Every other rule below fails `QUERY_ROOT_MISMATCH`"
introduces. Revision 7 adds the matching carve-out to the `verify()` doc
comment, so the prose, the code comment, and the code (arm gate before
`verify()` at both entry points) all state the same rule and the same
outcome; only the numbered rule's wording is awkward. Section 1 was outside
the 2026-09-15 grant, the packet Disposition and the machine-summary
`ariadne_contract_review_revision_2_note` both record the residue as
parked, and the owner cannot amend the sentence without a wording
authorization. Because it is outcome-consistent, unreachable via the gated
entry points, documented in code, and gated on an authorization the owner
does not hold, it is not an item the owner must act on in this round; it
stays a candidate for the next section 1 wording authorization (suggested
form: drop the completeness conjunct from rule 1 or restate rule 1 as the
gate's precondition) and should be picked up before freeze.

## New observation from the delta: the closeout's corpus and test bullets

The delta edited the closeout's Status line and first Evidence bullet to
revision 7 and left the neighbouring evidence bullets at their 2026-09-03/04
text. Lines 61-66 describe `accepted.json` as "twenty-three vectors ...
plus two-page continuation walks over entities and edges" and
`rejected.json` as "eight failures" listing eight names; the corpus at
c1d4177 has 27 vectors (four walks, including the root-cursor and
entry-point walks revision 5 added) and 10 mutations, the two additions
being exactly the `binding-substituted-fact` (31008) and
`arm-1-snapshot-profile` (31000) rejections that revision-7 section 9
names as required evidence. Lines 75-87 ("continuation walks over
entities and edges", "`sley-query` 65 tests") are likewise pre-revision-5
(104 tests pass at HEAD; the unit walk revision 7 names in its own first
bullet is not in the native-tests bullet). The closeout is the package's
named audit record (WORK_PACKAGES row 36 points at it); as written its
evidence bullets describe a corpus that would not satisfy the contract
they are cited for. This is record drift, not a code or contract defect:
the checkers, the fixture `--check`, and the oracle all read the real
corpus. It is owner-editable (the closeout is not the contract and needs
no wording authorization). Graded P3 [record], the same class and grade as
the WORK_PACKAGES revision drift at a809906.

## Findings

[P3] [record] docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md:61-66 - the Evidence corpus bullet states "twenty-three vectors ... two-page continuation walks over entities and edges" and "`rejected.json` (eight failures: ...)"; at c1d4177 `accepted.json` carries 27 vectors (four walks: entity, edge, root, entry-point) and `rejected.json` 10 mutations, the two additions being the 31008 binding and 31000 arm-1 rejections section 9 names as required evidence (generator `--check` and oracle: vectors 27, rejections/mutations 10); lines 75-87 (native tests: "walks over entities and edges", "`sley-query` 65 tests"; 104 pass, the revision-7 unit walk is not named) are stale in the same way. Update the two bullets to the revision-7 corpus and test set.

```
VERDICT: REVISE_0_P0_0_P1_0_P2_1_P3
SECTION: root_backed_query_profile
FIELD: ariadne_contract_review
SCOPE_SHA: c1d41778ea35d04b54b2b27c7594715b1d905b0e
FINDINGS:
[P3] [record] docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md:61-66 - Evidence corpus bullet still says twenty-three vectors with entity and edge walks and eight rejections; the corpus at c1d4177 is 27 vectors (four walks incl. root and entry-point cursors) and 10 mutations incl. the 31008 binding and 31000 arm-1 rejections that section 9 requires; lines 75-87 (native-tests bullet: entity/edge walks only, "sley-query 65 tests" vs 104 passing, revision-7 unit walk unnamed) are stale the same way
SUMMARY: Revision 7 at c1d4177 closes six of the seven items of my a809906 verdict and I verified each against the primary sources: section 7 now states the hit path exactly as the code runs it (complete_root_snapshot -> accept_cached -> decode_complete_root_snapshot with format/context/arm, self-digest trailer, and inverse-group rebuild, then inventory alignment against entity_bindings), claims only internal consistency for that audit under the S20-300 hit-authority rule, places verify_cached_snapshot off-path as the Tier 2 byte-rebuild audit (no non-test caller anywhere in crates/), names run_root_query_fresh as the exported-evidence path, names classes 14/15 by section 2's identifiers, and lists the class-1 direct-edge count as cache-supplied; the section 3 duplicate is collapsed; section 9's evidence rule matches the extended unit walk, which pages Roots, EntryRows, DependencyRows, and InventoryEntries at limit 1 through build_root_query_request over the real fixture and passes; the class-7 fixture count is 1 and no vector walks class 8, as the text says; WORK_PACKAGES row 36 and the machine summary are at revision 7; and the checker's new revision assertion and section markers were negative-tested in a scratchpad shadow tree (revision revert, class-name revert, and summary drift each FAIL, a reflow passes). Checker PASS, fixture --check drift [], oracle PASS 27/10, digests OK, cargo test 104 passed. The carried section 1 rule-1 residue is outcome-consistent, now documented in the verify() doc comment, recorded as parked pending its own wording authorization, and is therefore prose, not a listed item. One record item remains for the owner: the closeout audit's corpus and native-tests bullets, adjacent to the lines revision 7 edited, still describe the pre-revision-5 corpus (23 vectors, two walks, eight rejections, 65 tests) and so misdescribe the very evidence section 9 requires; a text-only update returns this lane to PASS.
```
