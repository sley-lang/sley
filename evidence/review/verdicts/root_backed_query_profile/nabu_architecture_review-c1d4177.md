## Nabu architecture review (revision 7 delta round): `root_backed_query_profile` / `nabu_architecture_review`

**Scope verification**: `git rev-parse HEAD` = `c1d41778ea35d04b54b2b27c7594715b1d905b0e`
(branch `main`, `/home/dev/Work/workspaces/sley2`). Match with the delta
SCOPE_SHA. `git status --porcelain` empty at start; `git diff c1d4177..HEAD --stat`
empty (HEAD is the scope). `git merge-base --is-ancestor f02f8de c1d4177`
confirms revision 7 is merged (52c3389) with the summary pin at c1d4177.
Read-only except this transcript.

**Prior lane round read in full**: `nabu_architecture_review-a809906.md`
(REVISE_0_P0_0_P1_1_P2_4_P3_5_P4 on revision 6). This round judges whether
f02f8de + c1d4177 close each of those ten items and introduce nothing new.

### Inputs read in full

- `/tmp/claude-sley2/review-brief.md`, `/tmp/claude-sley2/delta-s20-310-c1d4177.md`
- `git show f02f8de` (six files, +319/-52) and `git show c1d4177` (machine-summary pin, 2 lines)
- `git diff a809906..c1d4177 --stat` and `git log --oneline a809906..c1d4177`
- `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` at HEAD: status block (:1-25),
  section 1 preamble and rule 1 (:76-90), section 3 paging paragraph and
  acceptance rule (:250-275), section 7 in full (:382-428), section 9 walk
  bullet (:470-490)
- `crates/sley-query/src/root_query.rs`: `verify()` doc comment and body
  (:211-275), `build_root_query_request` (:692), `execute_root_query` (:900),
  `Complete` (:961), class-1 `direct_edges` count (:1004), private `page()`
  (:1284) with the `DependencyRows` (:1361-1386) and `InventoryEntries`
  (:1387-1410) arms, `pub(crate) mod tests` (:1742), the extended
  `single_item_classes_walk_two_items_at_limit_one` (:1903-2162)
- `crates/sley-query/src/snapshot.rs`: `decode_complete_root_snapshot` (:457-480),
  `inspect_candidate_for_arm` format/context/arm checks (:610-662), decoder
  tail digest-then-inversion (:740-753)
- `crates/sley-repo/src/index_cache.rs`: `accept_cached` (:106-126),
  `fresh_snapshot` (:128-135), `complete_root_snapshot` (:203-233),
  `verify_cached_snapshot` (:270-305); `grep -n verify_cached_snapshot` over
  both crates (definition + its own tests only)
- `crates/sley-repo/src/root_query.rs`: module doc, `run_root_query` (:94-140),
  `run_root_query_fresh` (:142-190), `run_context_capsule` (:192-205)
- `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md` :30-37 (hit accepted
  only under the writing repository authority) and :140-157 (residual, Tier 2
  `verify_cached_snapshot`)
- `scripts/check_root_backed_query_profile.py` at HEAD (`CONTRACT_REVISION`,
  `SPEC_REVISION_MARKERS`, `ENGINE_MARKERS`, `REPOSITORY_MARKERS`,
  `WORK_PACKAGE_MARKERS`, the revision assertions at :205-214)
- `docs/WORK_PACKAGES.md:36`; `docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md`
  (:3, :46-58); `evidence/review/decision-packets/round-7-root-query-contract.md`
  (Disposition, :54-69); `machineresearch/sley-2.0/machine-summary.json`
  section `root_backed_query_profile`; `docs/adr/ADR-0030-root-backed-query-boundary.md`
  (grep for section 11 / entity-read: none); `crates/sley-query/Cargo.toml`
  and `crates/sley-repo/Cargo.toml` dependency lines

### Tool results (exact)

| Command | Result |
|---|---|
| `SLEY2_MASTER_GOAL=/home/dev/machineresearch/Sley2.0mastergoal.md python3 scripts/check_root_backed_query_profile.py` | exit 0; `result: PASS`, `revision: 7`, `expected_revision: 7`, `machine_summary_revision: 7`, `status: S20_310_FULL_IMPLEMENTED_REVIEW_PENDING`, `problems: []`, `implementation_present` = `crates/sley-query/src/root_query.rs`, `crates/sley-repo/src/root_query.rs`, `sley-id:RootQuery`, `conformance/root-backed-query` |
| `cargo test -p sley-query --locked` | exit 0; `104 passed; 0 failed; 4 ignored`; doc-tests 0 (same count as a809906: the walk is one test extended, not a new test) |
| `cargo clippy -p sley-query --locked -- -D warnings` | exit 0; `Finished dev profile`, no warnings |
| marker simulation (scratchpad only): import the checker module, flatten `git show f02f8de^:docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` (revision 6) and the HEAD spec, test `SPEC_REVISION_MARKERS` and the status regex | revision 6 text: 8 of 12 revision markers missing and status revision 6 != 7 (checker would FAIL on a revert); revision 7 text: 0 missing; a whole-document reflow of revision 7 (every single newline joined): 0 missing |
| `git diff a809906..c1d4177 --stat` | 21 files: the six f02f8de files, the machine-summary pin, twelve a809906 council transcripts, and the S20-710 standards/SBOM repair (`docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md`, `scripts/build_standards_sbom.py`, `bench/release/tests/test_standards_sbom.py`); nothing else under `crates/sley-repo`, `conformance/root-backed-query`, or `scripts/check_root_backed_query_vector.py` moved |
| `grep -rn 'ListImpactClosure\|ListReverseImpactClosure'` over `*.md *.rs *.py *.json` excluding `evidence/review/verdicts` | no hits |
| `grep -n verify_cached_snapshot` over `crates/sley-repo/src crates/sley-query/src` | `index_cache.rs:278` (definition) and seven call sites, all inside `index_cache.rs`'s `#[cfg(test)] mod tests`; none in `root_query.rs` or the hit path |

### Independently re-derived claims

**Section 7 now states the S20-300 authority split as the code has it.**
Revision 7 (`:405-423`) separates three things and each matches a primary
source:

1. *Hit-path admission audit* = the decode-time audit inside
   `decode_complete_root_snapshot`, reached by `complete_root_snapshot ->
   accept_cached -> decode` then inventory-identity alignment. Code:
   `complete_root_snapshot` (`index_cache.rs:203-233`) reads the record and
   calls `accept_cached` (`:108-126`), which calls
   `decode_complete_root_snapshot(context_for(revision), record)` and then
   aligns `snapshot.inventory()[i].entity` with
   `record.entity_bindings[i].0` (identities only, as the text says). The
   decoder checks format (`snapshot.rs:617-644`: length bounds, magic,
   version, limits profile, option tag), context (`:646-651`
   `ContextMismatch` on epoch / field-schema hash / claimed root), arm
   (`:652-661` `CompletenessUnsupported`), then the self-digest trailer
   (`:742-744`, digest before inversion) and the inverse-group rebuild
   (`:746-748`). "Reads no object" is the function's own doc comment.
   The text's "proves the internal consistency of the cached edge set, not
   its agreement with the objects" is exactly the residual S20-300 states
   (`COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:141-144`), and "accepted only
   under the repository authority that wrote it and only for this read-only
   derived query surface" paraphrases S20-300 `:33-37` faithfully. S20-300
   has no heading titled "hit-authority rule"; the phrase is a label for the
   sentence at `:33-37`, which exists. Non-actionable editorial.
2. *Off-path byte-rebuild audit* = `verify_cached_snapshot` (`index_cache.rs:278-305`):
   `fresh_snapshot(revision)` then byte comparison, returning
   `Match|Missing|Mismatch`. The text says "a separate off-path audit for
   Tier 2 evidence that the hit path never calls"; the grep confirms no
   non-test caller, and S20-300 `:152-153` uses the same "audits and Tier 2
   evidence" wording.
3. *Fresh path* = `run_root_query_fresh` (`sley-repo/src/root_query.rs:152-190`):
   `fresh_snapshot` instead of `complete_root_snapshot`, and
   `run_context_capsule` (`:203`) builds every exported capsule through it.
   The text's "Exported evidence never rests on a bare hit" matches the
   `run_root_query` doc comment (`:96-98`) and S20-300 `:149-151`.

The revision-6 fusion ("admitted only through decode ... (`verify_cached_snapshot`,
the byte-rebuild audit)") and the "forged edge set cannot reach the engine"
overclaim are gone. The class names are the section 2 / engine identifiers
(`ReverseImpactClosure`, `ForwardDependencyClosure`), and the class-1
direct-edge count (`root_query.rs:1004`) is now listed as cache-supplied
with "no other answer fact comes from a hit" replacing "and nothing else".
The parenthetical about inventory identities and kinds being
record-determined is correct: `verify()` (`:249-252`) forces
`entry.entity` and `entry.kind` equal to the verified object's id and SSMC1
kind for every inventory position, on every request (both entry points call
it). Section 7 is now accurate at every layer boundary it names.

**The extended unit walk stays inside the engine's layer.** The test lives in
`pub(crate) mod tests` inside `root_query.rs` (`:1742`); `page()` is a
private `fn` (`:1284`) and `Complete` a private struct (`:961`), both
reachable only because the test is in the same module; no visibility was
widened and no non-test engine line changed in f02f8de (the diff outside
tests is the `verify()` doc comment only). The two new walks build both
requests through the public `build_root_query_request` over the real frozen
fixture (`Borrowed::new(&owned).input()`), so limits (`max_returned_entities = 1`),
the arm gate, cursor typing (`Cursor::Entity` for both classes), and
`verify()` run for real; only the `Complete` fed to `page()` is synthetic:
two `DependencyRow`s keyed `binding` `0x40 < 0x41` for class 7 and two
`IndexInventoryEntry`s keyed `entity` `0x50 < 0x51` for class 8, both in
strictly increasing key order. The keys match the `page()` arms
(`row.binding` at `:1368/:1373`, `entry.entity` at `:1394/:1399`). Assertions
are the section 3 page-set rule: `(2,1,true)` with `next_after = Some(first
key)`, then `(2,1,false)` with `next_after = None`, disjoint slices
`[..1]`/`[1..]` unioning to the complete result in key order, same
`total_count`. Dependency direction is unchanged: `crates/sley-query/Cargo.toml`
names no `sley-repo`; `crates/sley-repo/Cargo.toml:17` depends on
`sley-query`. Section 9 (`:478-489`) describes exactly this test, names the
three degenerate fixture classes (7, 10, 11) and the fourth arm
(`InventoryEntries` for `ListNamespaceMembers`, which no vector walks), and
keeps the corpus and oracle untouched (no change under
`conformance/root-backed-query` or `check_root_backed_query_vector.py`
between the rounds).

**The checker binds without duplicating authority.** `CONTRACT_REVISION = 7`
is asserted equal to the spec status line (`:207-210`) and the machine-summary
`contract_revision` is asserted equal to the spec (`:211-213`); the simulation
above shows the revision-6 text fails both the status assertion and 8 of 12
`SPEC_REVISION_MARKERS`, while a full reflow of revision 7 passes (markers
are matched against the whitespace-flattened spec). The markers are verbatim
quotations of the section 3/7/9/10 paragraphs; the checker asserts no paging
or cache semantics of its own, so the spec remains the only rule source and
the engine the only implementer. `ENGINE_MARKERS` binds the walk's function
name and `REPOSITORY_MARKERS` binds `pub fn run_root_query_fresh`, both
presence pins on the code the text names. The revision constant is a
governance pin (three places must agree: checker, spec, summary), not a
second authority over the contract.

**Records agree.** `docs/WORK_PACKAGES.md:36` says "implemented under
contract draft revision 7 (2026-09-15; ...)" with the revision-6 and
revision-7 history; the closeout status line (`:3`) says revision 7 with the
dated history and the Evidence bullet (`:46-58`) lists revisions 3 through 7
and names the extended walk; the packet carries a Disposition (`:54-69`)
naming the 2026-09-15 authorization, its primary record path, revision 6 as
f7df74f, the a809906 round, and revision 7; the machine summary pins
`contract_revision: 7` (c1d4177). The checker enforces spec == summary; the
WORK_PACKAGES row and closeout are human records that agree today.

### Per-item disposition of the a809906 verdict

| a809906 item | Status at c1d4177 |
|---|---|
| P2 section 7 hit-path audit misattributed to `verify_cached_snapshot` | Closed (`:405-423`; re-derived above against `index_cache.rs`, `snapshot.rs`, `root_query.rs`, S20-300 `:33-37, :141-153`) |
| P3 nonexistent class names | Closed (`:397-398`; grep clean) |
| P3 "and nothing else" vs class-1 edge count | Closed (`:398-400`) |
| P3 WORK_PACKAGES row / closeout revision drift | Closed (row 36 and closeout `:3, :46-58` at revision 7; summary 7) |
| P3 checker revision unasserted, no revision-paragraph markers | Closed (`CONTRACT_REVISION`, status and summary assertions, 12 markers; revert-fails / reflow-passes shown) |
| P4 packet disposition missing | Closed (Disposition section, `:54-69`) |
| P4 `verify()` doc comment framing | Closed (`root_query.rs:212-222` carve-out states the arm gate runs first and the completeness compare is defensive residue; consistent with the arm-first gates at both entry points) |
| P4 section 3 duplicate sentence | Closed (`:255-262`, one sentence) |
| P4 ADR-0030 does not acknowledge section 11 | Held open by design: outside the 2026-09-15 grant, recorded as such in the packet Disposition and the f02f8de message; needs its own authorization, so not actionable by the owner in this round |
| P4 rule 1 (`:88-89`) retains the arm condition the preamble (`:82-86`) assigns to the arm gate | Held open by design (same grant boundary); outcome-consistent, and the `verify()` doc comment now mirrors the preamble, so code and text agree on precedence |

Non-actionable observations (prose only): the collapsed section 3 sentence
(`:257-260`) says "because `total_count` is exact ... keys strictly increase";
strict key increase follows from canonical order plus the cursor predicate
rather than from count exactness, but the sentence's conclusion (union is
complete, no page can hide a fact) is outcome-correct and the acceptance rule
that follows is precise. `WORK_PACKAGE_MARKERS` (`check_root_backed_query_profile.py:118`)
still pins only the spec path and `ADR-0030`, so a future revision bump that
forgets row 36 would again surface only in council review; the two sources
the package checkers consume (spec, summary) are now bound, and the row is a
dated human record, so I do not hold the package on it and leave it to the
packet's item 9 governance residue. The machine-summary lane field
`nabu_architecture_review` currently reads `PASS_WITH_P3_P4_FOLLOWUPS_NO_P0_P1_P2`,
a dispatcher-owned value to be replaced from this verdict, not an owner
finding. The "S20-300 hit-authority rule" label has no heading by that name
in S20-300 but denotes an existing sentence (`:33-37`).

### Verdict

Revision 7 closes every actionable item of my a809906 verdict. Section 7 now
states the S20-300 authority split as the code implements it (decode-time
hit-path audit; off-path `verify_cached_snapshot` byte-rebuild for Tier 2;
`run_root_query_fresh` for exported evidence) and claims no more than S20-300
grants a hit. The extended walk is inside the engine's test module, touches
only the private `page()` with synthetic `Complete` values, drives requests
through the public constructor over the real fixture, and pins the class 7
and class 8 truncated arms; no engine code or dependency edge moved. The
checker binds the revision and the revision-6/7 paragraphs as quotation pins
with no semantic duplication, and revert-fails / reflow-passes was
demonstrated. Records agree at revision 7. The two items left open are
outside the operator grant and recorded as such. Checker PASS, 104 tests
pass locked, clippy clean. Nothing remains that the owner must act on in
this section.

```
VERDICT: PASS
SECTION: root_backed_query_profile
FIELD: nabu_architecture_review
SCOPE_SHA: c1d41778ea35d04b54b2b27c7594715b1d905b0e
FINDINGS:
SUMMARY: At c1d4177 revision 7 (f02f8de, summary pin c1d4177) closes every actionable item of the a809906 Nabu verdict: section 7 separates the hit-path decode-time audit (complete_root_snapshot -> accept_cached -> decode_complete_root_snapshot with format/context/arm checks, self-digest trailer, inverse-group rebuild, then inventory-identity alignment; index_cache.rs:108-126,203-233; snapshot.rs:617-661,742-748) from the off-path verify_cached_snapshot byte-rebuild audit for Tier 2 evidence (index_cache.rs:278-305, no non-test caller) and names run_root_query_fresh as the exported-evidence path (sley-repo root_query.rs:152-190, used by run_context_capsule), paraphrasing S20-300's hit-acceptance sentence and residual faithfully; the class names match section 2 and the engine, and the class-1 direct-edge count is listed as cache-supplied. The extended unit walk stays inside the engine's pub(crate) test module, reaches the private page() and Complete without widening visibility, builds both requests through the public constructor over the real fixture, and pins the DependencyRows (key binding) and InventoryEntries (key entity) truncated arms to the section 3 page-set rule; no non-test engine line changed and the sley-query -> sley-repo dependency direction is unchanged. The checker pins CONTRACT_REVISION = 7, asserts it against the status line and the machine-summary contract_revision, and binds twelve verbatim revision-6/7 paragraph markers plus the walk and run_root_query_fresh presence pins without restating any semantics; a simulation shows the revision-6 text fails eight markers and the revision assertion while a full reflow of revision 7 passes. WORK_PACKAGES row 36, the closeout, the packet Disposition, and the machine summary all agree at revision 7. Checker PASS, cargo test 104 passed locked, clippy clean. The section 1 rule-1 residue and ADR-0030 section 11 remain open by design outside the operator grant and are recorded as such; nothing actionable remains for the owner in this section.
```
