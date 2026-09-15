## Nabu architecture review (revision 6 round): `root_backed_query_profile` / `nabu_architecture_review`

**Scope verification**: `git rev-parse HEAD` = `a809906f78f1bfdb9cde8da4c108c4d692dad297`
(branch `main`, `/home/gfarch/Work/workspaces/sley2`). Match. `git status --porcelain`
was empty at start. Read-only except this transcript. The tree is the
records-closure-INELIGIBLE pre-re-mint HEAD the round brief describes; that
posture is not judged here.

**Prior lane rounds read in full**: `nabu_architecture_review-0bcc9c6.md`
(REVISE, entity-read scope) and `nabu_architecture_review-a4b6029.md` (PASS on
revision 5, one held-gated P2, two P3, three P4). Also read: the round-7 decision
packet `evidence/review/decision-packets/round-7-root-query-contract.md`, the
latest Vulcan transcript `vulcan_surface_review-43f2f5b.md` (for the degenerate
class-10/11 walk finding that revision 6 answers), the handoff
`docs/status/HANDOFF-2026-09-15-QUALIFICATION.md`, and the external handoff
record `~/.config/greyforge/agents/handoffs/2026-09-15-sley2-qualification-reconciliation.json`
(field `operator_authorization_2026_09_15`).

### Inputs read in full

- `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` revision 6 (513 lines) and
  `git show f7df74f -- docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md crates/sley-query/src/root_query.rs`
- `git show 70283ce -- crates/sley-query/src/root_query.rs` (5 insertions, 11 deletions, test module only)
- `scripts/check_root_backed_query_profile.py` (234 lines)
- `crates/sley-query/src/root_query.rs`: `Cursor` and `RootQueryInput::verify()`
  (:160-300), `key_tag`/`named_entities`/`validate_shape` (:395-470),
  `build_root_query_request`/`validate_cursor`/`encode_preimage` (:683-760),
  `execute_root_query`/`Complete`/`Paged` (:885-965), `compute` class 1
  (:966-1000), `page` (:1275-1420), the new test
  `single_item_classes_walk_two_items_at_limit_one` (:1905-2013)
- `crates/sley-query/src/query.rs`: `QueryLimits` (:162-189), `validate_limits` (:513-)
- `crates/sley-query/src/snapshot.rs`: `decode_complete_root_snapshot` (:457-),
  decoder tail with the digest-then-inversion audit (:725-760)
- `crates/sley-repo/src/index_cache.rs`: `accept_cached` (:108-126),
  `complete_root_snapshot` (:203-233), `read_record`, `CacheVerify`,
  `verify_cached_snapshot` (:278-305)
- `crates/sley-repo/src/root_query.rs` (:1-140, `run_root_query`)
- `crates/sley-query/src/context_capsule.rs` continuation-consumer lines (:364, :394-399, :424-435)
- `scripts/check_root_backed_query_vector.py` paging/walk assertions (:405-425, :533-571)
- `machineresearch/sley-2.0/machine-summary.json` section `root_backed_query_profile`;
  `evidence/review/finding-register.json` rows for the section (:3271-3430, :5655-5672);
  `docs/WORK_PACKAGES.md:36`; `docs/adr/ADR-0030-root-backed-query-boundary.md`;
  `docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md` (revision references)
- `conformance/root-backed-query/v1/{accepted,rejected}.json` vector ids and `SHA256SUMS`

### Tool results (exact)

| Command | Result |
|---|---|
| `SLEY2_MASTER_GOAL=... python3 scripts/check_root_backed_query_profile.py` | exit 0; `result: PASS`, `revision: 6`, `status: S20_310_FULL_IMPLEMENTED_REVIEW_PENDING`, `problems: []`, `implementation_present` = engine, repo surface, `sley-id:RootQuery`, `conformance/root-backed-query` |
| `cargo test -p sley-query --locked` | exit 0; `104 passed; 0 failed; 4 ignored`; `single_item_classes_walk_two_items_at_limit_one ... ok`, `continuation_pages_union_to_the_complete_result_with_exact_counts ... ok`, `unsorted_root_committed_fact_sets_are_root_mismatch ... ok`; doc-tests 0 |
| `cargo clippy -p sley-query --locked -- -D warnings` | exit 0; `Finished dev profile` with no warnings |
| `sha256sum -c SHA256SUMS` in `conformance/root-backed-query/v1` | `accepted.json: OK`, `rejected.json: OK`, exit 0; 19 class vectors + 8 page vectors (`page-namespaces`, `page-edges`, `page-roots`, `page-entry-points`, two pages each) |
| `git diff --stat 7a94a4a..HEAD -- fuzz/ scripts/check_root_backed_query_vector.py scripts/generate_root_backed_query_fixtures.py scripts/check_root_backed_query_profile.py conformance/root-backed-query conformance/entity-read crates/sley-repo/src/root_query.rs crates/sley-repo/src/index_cache.rs crates/sley-query/src/snapshot.rs crates/sley-query/src/query.rs crates/sley-query/src/context_capsule.rs docs/adr/ADR-0030-root-backed-query-boundary.md docs/spec/ENTITY_READ_PROFILE_V2.md docs/spec/ERROR_CODES_V1.md docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md` | empty: since the attested candidate the section's surface changed only in the two files the task names (`docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` +63/-6 at f7df74f; `crates/sley-query/src/root_query.rs` +127 test lines at f7df74f, reflowed at 70283ce) |
| `git show 7a94a4a:crates/sley-query/src/query.rs`, `git show f7df74f^:...`, `git log -S` on the derive | `#[derive(Clone, Copy, Debug, Eq, PartialEq)] pub struct QueryLimits` present at the candidate, at f7df74f^, and introduced at `3603dc6` (2026-08-27, restricted profile) |
| `grep -rn 'ListImpactClosure\|ListReverseImpactClosure'` over `*.md *.rs *.py *.json` | one hit: `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:393` |

### Independently re-derived claims

**`QueryLimits: Copy` changes no semantics, and 70283ce did not introduce it.**
The derive `Clone, Copy, Debug, Eq, PartialEq` has been on `QueryLimits`
(`query.rs:163`) since `3603dc6`; it is present at the attested candidate and at
`f7df74f^`. Commit 70283ce touches only the new test: it deletes three
`one.clone()` calls (clippy `clone_on_copy` under `-D warnings`) and reflows
the call sites. `QueryLimits` is five plain integers; every engine signature
already took it by value (`build_root_query_request(.., limits: QueryLimits, ..)`
:686, `encode_limits_request(&mut out, limits)`, `RootQueryRequest.limits`
:478). No request preimage byte, response byte, ordering, or precedence depends
on whether the caller copies or clones the value. The commit title "QueryLimits
is Copy" states a pre-existing property; a log reader could infer a derive
change, but git history is immutable and the handoff wording is accurate
enough ("clippy clean"). Prose note only; nothing actionable.

**The new unit walk sits at the right layer and does what section 9 now says.**
`single_item_classes_walk_two_items_at_limit_one` (`root_query.rs:1905-2013`)
builds both requests through the public `build_root_query_request` over the
real frozen fixture (`Borrowed::new(&owned).input()`), so `validate_limits`
(`max_returned_entities = 1` is admissible; zero is refused at `query.rs:514`),
the arm gate, `validate_cursor` (the second request's `after` is
`Cursor::Root(roots[0])` against `key_tag() = Some(CURSOR_ROOT)` and
`Cursor::Entity(rows[0].entry_point)` against `Some(CURSOR_ENTITY)`), and
`verify()` all run for real. Only the `Complete` fed to the private `page()` is
synthetic (two roots `0x10..`/`0xFE..`, two entry rows `0x30`/`0x31`, both in
strictly increasing key order as `verify()` would require of the record). The
test asserts on each class `(total_count, returned, truncated) = (2, 1, true)`
then `(2, 1, false)`, `next_after = Some(first key)` then `None`, and the
disjoint union equal to the complete result: exactly the section 3 page-set
rule (identical requests except `after`, first `after = None`, chained
`after == prev.next_after`, all-but-last truncated, last untruncated with no
`next_after`, same `total_count`, `sum(returned) == total_count`). The test
module is inside `root_query.rs`, so reaching `page()` needs no visibility
widening; no engine code changed. The `Roots` and `EntryRows` truncated-emission
arms that the Vulcan 43f2f5b P3 found unpinned are now pinned. Section 9's
new sentence says the fixture walk for these classes is degenerate and the
unit walk is "the evidence rule rather than a fixture with invented items";
the corpus (`page-roots-1/2`, `page-entry-points-1/2`, pinned by `SHA256SUMS`)
and the oracle block (`check_root_backed_query_vector.py:546-571`) are
unchanged and consistent with that statement.

**No authority is duplicated between spec, checker, oracle, and engine.**
- The engine is the only page producer (`page()`/`page_items`, one home) and
  the only order guard (`verify()` `strictly_increasing` on both root-committed
  fact sets, `:262-271`).
- The section 3 consumer rule has one in-tree implementer of its standalone
  arm: `context_capsule.rs:364` and `:433` derive `Complete` exactly when
  `!truncated && after.is_none()`, and `:424` refuses a response whose
  `truncated` disagrees with `next_after.is_some()`. No in-tree code stitches
  page sets; the capsule records `after`/`next_after` and calls a page
  not-complete. So the page-set arm is a stated consumer obligation with no
  second authority, as intended.
- The oracle re-derives the fixture walks (`:533-571`) as evidence, not as a
  rule source; the checker (`check_root_backed_query_profile.py`) binds
  structural markers, status gating, and the code presence list, and asserts
  no paging semantics.
- Section 7's division of labour (cache supplies edges; record supplies the
  rest; hit admitted by decode, then binding re-checked) matches
  `run_root_query` (`sley-repo/src/root_query.rs:113-133`): extraction from the
  verified revision, `complete_root_snapshot` for the snapshot, then
  `build_root_query_request` (which runs `verify()`) and `execute_root_query`
  (which runs it again). One producer, one binder.

**Packet items against revision 6.** The dispatcher's "four P1 items" are the
packet's wording items 1 (section 3 stitching predicate), 2 (section 9 cursor
walks), 7 (section 7 cache wording), and 8 (section 10 enumeration); items 3-6
were closed by revision 5 and re-derived in my a4b6029 round; item 9
(checker revision/vector-binding assertions) is governance and remains open
(see P3 below).
- Item 1: implemented, strengthened (adds first-page `after = None`,
  every-non-last-page `truncated = true`, and the no-continuation-authority
  sentence). Consistent with `page()`: `truncated = remaining > limit`,
  `next_after` set iff truncated, `total_count` the pre-cursor complete count.
- Item 2: the packet's second alternative ("weaken §9 to one-walk-per-cursor-
  key-type") plus the unit-walk evidence rule. Authorized alternative; evidence
  present and executed above.
- Item 8: implemented exactly (chosen not derived; class 20 additive; tag 3 and
  class 18 not reallocated; empty 16/17/19 lawful). Engine agrees:
  `CURSOR_ROOT = 3` (`:45`), `ListDeclaredEffects` is tag 18, and an empty
  `Entities` result pages to `total_count = 0, truncated = false`.
- Item 7: implemented in substance (edges cache-derived, rest record-derived,
  decode audit named, binding re-check named) but not accurately, in three
  places that are all in my lane because they describe which layer holds which
  authority. Detailed below.

**Ownership and layering boundaries from the prior rounds.** Dependency
direction (`sley-query` depends on check/id/scb1/ssmc/state-root only; `sley-repo`
depends on `sley-query`), the thin trusted-input adapter, the one producer of
`RootQueryInput` from persistent state, the arm-first gate at both entry points
(`:691-693`, `:896-898`), cursor-before-drift order in execute (`:899-910`), the
edges-only cache hit, and the pure-Python oracle are unchanged since a4b6029 and
re-confirmed by reading; the `git diff --stat` above shows no code outside the
new test moved. Revision 6 alters no engine behaviour and no normative rule
that the engine already implements; it adds consumer and evidence text.

### Per-item analysis of the section 7 sentence (revision 6, `:390-403`)

1. **Hit-path audit misattributed (`:397-401`).** The text: "A hit is admitted
   only through `decode_complete_root_snapshot`, whose decode-time audit
   rebuilds the inverse edge groups from the direct edges and refuses a
   snapshot whose groups differ (`verify_cached_snapshot`, the byte-rebuild
   audit of S20-300)". In code these are two different mechanisms at two
   different layers. The decode-time audit is inside the `sley-query` decoder
   (`snapshot.rs:742-748`: digest check, then
   `decoded_reverse != invert_edges(&direct)` fails `FormatInvalid`), reached
   from the hit path `complete_root_snapshot -> read_record -> accept_cached ->
   decode_complete_root_snapshot` (`index_cache.rs:108-126, 203-233`), and it
   reads no object ("Reads no object and extracts no edge"). `verify_cached_snapshot`
   (`index_cache.rs:278-305`) is a separate `sley-repo` audit that rebuilds the
   whole snapshot from the revision (object access via `fresh_snapshot`) and
   compares bytes, returning `CacheVerify::{Match,Missing,Mismatch}`; nothing on
   the hit path calls it. The packet (item 7) kept them distinct: "Decode layer
   already enforces inversion; body comparison in hot `accept_cached` is
   declined without a cache-authority change", and asked that section 7 point
   at `verify_cached_snapshot` as the byte-rebuild audit. Revision 6 fused them
   into one appositive, so the normative text now says a hit is admitted under
   the byte-rebuild audit. It is not, by design (the repository doc comment even
   says exported evidence "must use `run_root_query_fresh` instead, never a bare
   hit", which only makes sense because hits are decode-audited, not
   rebuilt). Even the charitable reading (parenthetical as an additional
   mechanism) still places `verify_cached_snapshot` inside "a hit is admitted
   only through", which is false. This misstates the S20-300 authority split
   the profile claims to compose without altering. Code is correct; text is
   wrong at the one boundary this lane exists to guard. P2, one-sentence repair:
   name the decoder's inversion audit as the hit-path audit and name
   `verify_cached_snapshot` separately as the off-path byte-rebuild audit that
   Tier 2 evidence runs.

2. **Nonexistent class names (`:392-393`).** Section 7 names classes 12-15 as
   `ListDirectDependencies`, `ListDirectDependents`, `ListImpactClosure`,
   `ListReverseImpactClosure`. Section 2 (`:134-136`) and the engine enum name
   14 `ReverseImpactClosure` and 15 `ForwardDependencyClosure`; the two section 7
   names occur nowhere else in the repository (grep above). Revision 6 put
   identifiers that do not exist into the sentence that maps cache authority to
   classes. P3.

3. **"and nothing else" is false for class 1 (`:390-393` vs `root_query.rs:995`).**
   `GetRootSummary` reports `direct_edges: to_u64(input.snapshot.direct_edges().len())`
   and section 2 (`:145-147`) says the summary includes "the snapshot's
   direct-edge count". That count is cache-derived on a hit and has no record
   counterpart to be re-checked against (the decode audit bounds it, the binding
   does not). So a hit answers one fact outside classes 12-15. Kinds are also
   read from `snapshot.inventory()` (`:302`, `:1004`), but `verify()` (`:239-247`)
   proves every inventory kind equals the entity's SSMC1 kind, so "kinds ...
   never from the cache" is defensible as a statement about authority; the edge
   count is not. P3: add the class 1 edge count to the cache-derived list, or
   drop "and nothing else".

### Other items

- **Record pointers trail revision 6.** `docs/WORK_PACKAGES.md:36` still says
  "implemented under contract draft revision 5 (2026-09-13)"; the machine
  summary was bumped to 6 but the row was not (the same shape as my a4b6029 P3,
  which was 4-vs-5 then). `docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md:3,46`
  reads "revision 3" and is named by the summary as the section closeout. P3
  record.
- **Checker still reports the revision without asserting it**
  (`check_root_backed_query_profile.py:218-228`); the live 5-vs-6 divergence in
  WORK_PACKAGES proves the gap again, and no marker binds any revision-6
  paragraph, so a revert of sections 3/7/9/10 text would still PASS. Packet
  item 9 names this as governance work; unchanged at a809906. Carried P3.
- **Operator authorization record.** The revision-6 status line, the handoff,
  and `machine-summary.json:872` cite an operator authorization of 2026-09-15;
  the in-repo packet still ends with "Operator decision needed" and no
  disposition, and the packet's own header says contract changes are "NOT
  authorized under the current order". The primary record is outside the
  repository (`~/.config/greyforge/agents/handoffs/2026-09-15-sley2-qualification-reconciliation.json`,
  `operator_authorization_2026_09_15`: "... S20-310 wording closure consistent
  with reviewed semantics ..."). I read it; it covers the four sections. The
  gap is that the packet, which is the in-repo primary source, does not carry
  the answer. P4 record: append the disposition line to the packet.
- **Held-gated P2 from a4b6029 (single-direction ownership acknowledgment).**
  Section 11 is the normative composition and is present; the closeout now
  references the entity-read surface (four mentions); only ADR-0030 still does
  not acknowledge section 11. ADR amendment was outside the authorization
  scope. Downgraded to P4 record, no longer gated.
- **Carried P4s, unchanged by revision 6**: `verify()` doc comment
  (`root_query.rs:212-222`) still frames arm disagreement as
  `QUERY_ROOT_MISMATCH` (unreachable via the gated entry points); the page-union
  sentence is still duplicated verbatim (`:248-254`), and revision 6 inserted
  the acceptance rule immediately after the duplicate rather than replacing it;
  rule 1 (`:83`) still carries the arm condition that `:76-80` assigns to the
  arm gate.

Non-actionable observations: the 70283ce commit title describes a pre-existing
derive; the Vulcan 43f2f5b P3 on degenerate class-10/11 walks is answered by the
unit walk plus the section 9 evidence rule; the Vulcan 43f2f5b P4 (oracle union
assertion assumes disjoint pages) is unaffected because the corpus did not
change.

### Verdict

No P0 or P1. Revision 6 changes no engine behaviour, duplicates no authority,
and keeps every layering boundary the prior rounds established; the unit walk
is correctly placed and executes the page-set rule end to end. What it gets
wrong is the description of the cache-hit authority in section 7, the one
sentence this lane cares most about, plus two smaller inaccuracies in the same
sentence and the recurring record/checker revision drift. All are text or
record repairs; none requires a code change.

```
VERDICT: REVISE_0_P0_0_P1_1_P2_4_P3_5_P4
SECTION: root_backed_query_profile
FIELD: nabu_architecture_review
SCOPE_SHA: a809906f78f1bfdb9cde8da4c108c4d692dad297
FINDINGS:
[P2] [contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:397-401 - revision 6 states a hit is admitted through the decode-time inverse-group audit "(verify_cached_snapshot, the byte-rebuild audit of S20-300)"; in code the hit path is complete_root_snapshot -> accept_cached -> decode_complete_root_snapshot (index_cache.rs:108-126,203-233; audit at snapshot.rs:742-748, no object access) and verify_cached_snapshot (index_cache.rs:278-305) is a separate off-path byte-rebuild audit never called on a hit; the packet (item 7) kept them distinct and declined hot-path body comparison; the text misattributes the S20-300 authority split it claims to compose unchanged; repair by naming the decoder inversion audit as the hit-path audit and verify_cached_snapshot separately as the Tier 2 byte-rebuild audit
[P3] [contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:392-393 - classes 14 and 15 are named ListImpactClosure and ListReverseImpactClosure; section 2 (:134-136), the engine enum, oracle, and corpus name them ReverseImpactClosure and ForwardDependencyClosure, and the two names occur nowhere else in the repository
[P3] [contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:390-393 vs crates/sley-query/src/root_query.rs:995 - "a hit supplies ... classes 12 through 15 ... and nothing else" is false: GetRootSummary reports the snapshot's direct-edge count (section 2 :145-147 says so), which is cache-derived on a hit and has no record counterpart to re-check; add it to the cache-derived list or drop "and nothing else"
[P3] [record] docs/WORK_PACKAGES.md:36, docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md:3,46 - the S20-310 row says "revision 5 (2026-09-13)" and the closeout says "revision 3" while the contract and machine-summary contract_revision say 6; same drift shape as the a4b6029 P3 (then 4-vs-5)
[P3] [checker] scripts/check_root_backed_query_profile.py:218-228 - the revision is extracted and reported but never asserted against machine-summary contract_revision or the WORK_PACKAGES row, and no marker binds any revision-6 paragraph (a revert of the sections 3/7/9/10 text still PASSes); packet item 9 governance work, unchanged at a809906; carried from a4b6029
[P4] [record] evidence/review/decision-packets/round-7-root-query-contract.md - the packet still ends "Operator decision needed" with no disposition while the spec status line, the handoff, and machine-summary.json:872 cite the 2026-09-15 authorization; the primary record lives only in ~/.config/greyforge/agents/handoffs/2026-09-15-sley2-qualification-reconciliation.json; append the disposition to the packet
[P4] [record] docs/adr/ADR-0030-root-backed-query-boundary.md - does not acknowledge the section 11 entity-read composition (closeout now does); residue of the a4b6029 held-gated P2, downgraded because section 11 is the normative composition and ADR amendment was outside the authorization
[P4] [implementation-doc] crates/sley-query/src/root_query.rs:212-222 - verify() doc comment still frames arm disagreement as QUERY_ROOT_MISMATCH without the section 1 carve-out; unreachable via the arm-gated entry points; carried
[P4] [contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:248-254 - page-union sentence still duplicated verbatim; revision 6 inserted the acceptance rule after the duplicate instead of replacing it; carried
[P4] [contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:83 vs :76-80 - rule 1 retains the arm condition that the preceding prose assigns to the arm gate; outcome-consistent; carried
SUMMARY: At a809906 the section's code surface is unchanged since the attested candidate except for the new paging-layer unit walk, which is correctly placed inside the root_query test module, drives both requests through the public constructor over the real fixture (limits, arm gate, cursor typing, and verify() all run), feeds only the Complete synthetically, and asserts the full section 3 page-set rule for the Roots and EntryRows truncated arms that were previously unpinned; QueryLimits has derived Copy since 3603dc6 and 70283ce merely removed clone_on_copy calls, so no semantics moved. Checker PASS at revision 6, 104 sley-query tests pass locked, clippy clean, corpus pins verified. No authority is duplicated: the engine is the only page producer and order guard, the capsule is the only in-tree consumer of the standalone completeness arm, the oracle re-derives fixture walks as evidence, and the checker binds only structure. Packet items 1 and 8 are implemented exactly and item 2 by its authorized alternative plus the unit-walk evidence rule; item 7 is implemented in substance but the revision-6 section 7 sentence misattributes the hit-path audit to verify_cached_snapshot (an off-path byte-rebuild the hit path never calls), names two classes that do not exist, and says a hit answers "nothing else" when class 1 reports the snapshot's edge count. Those three text defects sit on the cache-authority boundary this lane owns and are actionable (one P2, two P3); with the recurring revision-pointer drift in WORK_PACKAGES and the closeout, the still-unasserted checker revision, and five P4 record/editorial notes, the verdict is REVISE with no code change required.
```
