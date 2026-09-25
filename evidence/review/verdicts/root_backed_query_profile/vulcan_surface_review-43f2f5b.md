# root_backed_query_profile / vulcan_surface_review — Vulcan (QA and security), revision-2 round

Scope verified: `git rev-parse HEAD` = `43f2f5ba738b587a9a0bb365a55f3096527e3e3d` (branch `main`), matching SCOPE_SHA. Read-only review; the only file written is this transcript. `git status --short conformance/` was empty after every gate run (the `--check` generator and the emitter test wrote nothing tracked). Scratch probes (`oracle-probe/probe.py`, `oracle-probe/recompute_root.py`, mutated copies of `rejected.json`) live only in the session scratchpad.

Prior lane round `vulcan_surface_review-a4b6029.md` (2026-09-13, `REVISE_0_P0_0_P1_1_P2_0_P3_1_P4`) read in full. Its two open items — the P2 (rev-5 rejection rows harness-only; oracle exited 1 with both rows `:accepted`) and the P4 (page-roots / page-entry-points chaining not asserted) — are re-derived below. The four carried P1s it verified closed are re-scanned for regression because `crates/sley-query/src/root_query.rs`, `crates/sley-repo/src/root_query.rs`, and `crates/sley-repo/src/index_cache.rs` changed in `209c661` / `c0f4ff6` and were reformatted in `43f2f5b`.

## Inputs read in full

- `scripts/check_root_backed_query_vector.py` (638 lines, HEAD) and its diff `a4b6029..HEAD`
- `scripts/generate_root_backed_query_fixtures.py` (186 lines) and its diff `a4b6029..HEAD`
- `scripts/check_root_backed_query_profile.py` (234 lines)
- `conformance/root-backed-query/v1/rejected.json` (all 10 rows), `SHA256SUMS`, and the `accepted.json` context + every `page-*` / `class-10` / `class-11` vector header decoded from `record_hex`
- `crates/sley-query/src/root_query.rs`: `RootQueryInput` + `verify()` (:150-300), `key_tag` (:402-414), `build_root_query_request` / `validate_cursor` / `execute_root_query` (:683-930), `canonical` / `forward_closure` / `edge_key` / `page` / `page_items` (:1221-1500), class 10/11 compute (:1112-1131), the continuation union test (:2052-2200), the emitter harness incl. both tamper rows (:2858-3149)
- `crates/sley-repo/src/index_cache.rs` (:1-330 at HEAD) and `crates/sley-repo/src/root_query.rs` (all 377 lines)
- `crates/sley-query/src/snapshot.rs` decoder tail (:736-756); `crates/sley-state-root/src/lib.rs` `recompute_root` (:377-381)
- `git diff -w a4b6029..c0f4ff6` on the four engine files (semantic P-C delta, 32 KB) and `git diff -w c0f4ff6..43f2f5b` (formatting-only confirmation)
- `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` sections 1, 3, 8, 9
- `scripts/check_state_root_vector.py` (encoder + `root_of`), used for the independent STATE_ROOT_V1 recompute
- `machineresearch/sley-2.0/machine-summary.json` `root_backed_query_profile` section (read only)

## Gate runs at `43f2f5b` (all with `SLEY2_MASTER_GOAL` set)

| Check | Result |
|---|---|
| `python3 scripts/check_root_backed_query_profile.py` | PASS, exit 0, revision 5, `S20_310_FULL_IMPLEMENTED_REVIEW_PENDING`, `problems: []` |
| `python3 scripts/generate_root_backed_query_fixtures.py --check` | PASS, exit 0, `{"drift": [], "rejections": 10, "vectors": 27}` — shells to `cargo test -p sley-query emit_root_query_vectors_for_fixture_refresh -- --ignored --nocapture` with `check=True` (generator :64), so the emitter's `assert_eq!(…, 31_008)` / `assert_eq!(…, 31_000)` are load-bearing for this gate |
| `sha256sum -c SHA256SUMS` in `conformance/root-backed-query/v1/` | `accepted.json: OK`, `rejected.json: OK`, exit 0 |
| `uv run --project oracle/scb1 --frozen python scripts/check_root_backed_query_vector.py` | PASS, exit 0, `{"mutations": 10, "problems": [], "query_classes": 19, "vectors": 27}` |
| `cargo test -p sley-query root_query --locked` | ok: 7 passed, 0 failed, 1 ignored (the emitter), 99 filtered |
| `cargo test -p sley-repo --locked index_cache` (regression scan) | ok: 8 passed (7 `index_cache::tests` + `exchange::…_without_adopting_the_index_cache`) |
| `cargo test -p sley-repo --locked root_query` (regression scan) | ok: 2 passed (`cache_hit_and_rebuild_answer_byte_identical_records_without_object_access`, `capsules_build_fresh_and_never_rest_on_a_cache_hit`) |

## Prior P2 — WHY the oracle now exits 0, re-derived

### What changed since `a4b6029`

Three coordinated edits (diff `a4b6029..HEAD`): (1) `rejected.json` rows `binding-substituted-fact` and `arm-1-snapshot-profile` gained a `tamper` object (`{"substituted_root": "09"*32}` and `{"arm": 1}`); (2) the generator stamps those objects by row id (`generate_root_backed_query_fixtures.py:119-121`), not by parsing anything from the `ROOT_QUERY_REJECT` line; (3) the oracle grew a tamper branch (`check_root_backed_query_vector.py:580-619`).

### The oracle's tamper branch, read literally

For a row with `tamper` (:580-599): it first runs the honest request through `answer()` and requires acceptance; any `Failure` is reported as `<id>:tamper-masks:<code>` (:597), so a tamper can never hide behind an already-failing request. Then (:602-619): `arm == 1` raises `QUERY_PROFILE_UNSUPPORTED`; `substituted_root` present raises `QUERY_ROOT_MISMATCH` after asserting it differs from `context["root_hex"]` (`tamper-not-substituted` otherwise, :608); any other descriptor is `tamper-empty` (:611). The raised code is then compared to `expected_code` and `expected_numeric` through `CODES` (the same comparison path as the eight pre-rev-5 rows). Rows without `tamper` keep the original rule (:581-593), so the pre-rev-5 mutations are unchanged.

### Fail-closed probe (scratchpad copy of the oracle with `REJECTED` repointed at mutated copies)

| Mutation of `rejected.json` | Oracle result |
|---|---|
| unmodified | exit 0, `[]` |
| `tamper` removed from binding row | exit 1, `binding-substituted-fact:accepted` (prior-round behaviour reproduced) |
| `tamper` removed from arm row | exit 1, `arm-1-snapshot-profile:accepted` |
| `substituted_root` set to the real root | exit 1, `…:tamper-not-substituted` |
| `arm` set to 2 / set to 3 | exit 1, `…:tamper-empty` (both) |
| binding row given `max_work: 0` (honest request already fails) | exit 1, `…:tamper-masks:QUERY_RESOURCE_LIMIT` |
| binding row `expected_code` swapped to 31000 | exit 1, `…:code:QUERY_ROOT_MISMATCH` |
| arm row `expected_code` swapped to 31008 | exit 1, `…:code:QUERY_PROFILE_UNSUPPORTED` |
| binding row `expected_numeric` 31009 | exit 1, `…:numeric` |
| both tampers on one row | exit 1, `…:code:QUERY_PROFILE_UNSUPPORTED` (arm wins, matching section 8 item 2 before item 5) |
| unknown tamper key | exit 1, `…:tamper-empty` |

Every route to a green gate other than the frozen rows fails loudly. The gate is not cosmetically green.

### Does the oracle re-compute the checks, or trust a field?

**`binding-substituted-fact` (31008) — adequately data-derived by the oracle; rule-5 recompute closed independently here.** The engine harness tampers `input.root = StateRoot::from_bytes([0x09; 32])` (`root_query.rs:3086`) and both build and execute refuse (`:3097`). In the engine, `verify()` fires on its first predicate — `snapshot.context().claimed_root_context != Some(self.root)` (`:225-229`) — before the STATE_ROOT_V1 recompute (`:272-289`). The oracle re-derives exactly that first predicate from fixture bytes: `context.root_hex` is pinned to the S20-300 snapshot fixture's `root_hex` (`context:snapshot-drift`, :510; that fixture's `record_hex` embeds `bcf36d94…` at the claimed-root-context position, and its own oracle `check_complete_root_index_snapshot_vector.py:141,173,189` decodes arm and root from bytes), and the substituted value must differ (:606-609). This is a predicate evaluated on data, not a copied code field.

What the oracle does *not* model is section 1 rule 5 (the nine-field digest recompute). I closed that independently: using the shared-nothing encoder from `scripts/check_state_root_vector.py` (`uvar`/`sized`/`record`/`mapping`/`sequence`/`root_of`) over the frozen context — `workspace_id`, `schema_epoch_hex` (as field 2 and preimage epoch), the 19 `bindings` as a fixed32→fixed32 map, `facts.entry_points = [0x0a…]`, `facts.dependency_roots = [0x09…]`, `contract_root`/`test_root`/`policy_root`, empty flags — the recompute is `bcf36d9415edb1c3a8e4826ce73937119d2832b6eed980c4e9cbf1e4a65262e8`, byte-equal to `context.root_hex` (MATCH). `0x09*32` is not that digest. Three single-fact tampers mirroring the engine matrix (`root_query.rs:2441-2453`: entry point → `0x06`, dependency root → `0x0a`, `bindings[0].object` → `0xff`) each produce a different root. So the frozen context genuinely binds every answer-bearing fact under an encoder that shares no code with `sley-state-root`, and a substituted root or fact cannot recompute. Rule 5 is real, not a shape check.

**`arm-1-snapshot-profile` (31000) — engine-pinned; the oracle's adjudication is a descriptor lookup.** The harness builds an arm-1 snapshot from the restricted-kind subset with the *same* claimed root (`:3110-3122`) and swaps it in; both stages refuse at `input.snapshot.completeness() != CompleteRoot` (`:691-693`, `:896-898`), which precedes `verify()`. That ordering is what makes the answer 31000 rather than 31008 (the arm-1 inventory disagrees with the 19 bindings, so binding would also fail), and the harness asserts it (`:3139`) inside a test the generator runs with `check=True` — a precedence inversion would break `--check` loudly. The oracle, however, has no completeness model (its preimage hard-codes `u32(2)`, :169) and its rule is literally `if tamper.get("arm") == 1: raise Failure("QUERY_PROFILE_UNSUPPORTED")` (:603-604): not `arm != 2`, not derived from any byte, and applied to a descriptor the generator stamps by row id. For this row the "independent oracle" proves only that the honest request accepts and that the row is well-formed and code/numeric-consistent; the refusal itself rests on the Rust assert. Both `tamper` descriptors share the stamping hazard (generator :119-121 vs harness label), though the binding descriptor is at least validated against data.

**Conclusion on the prior P2.** Closed. The gate is green for a real reason — the oracle now adjudicates both rows with fail-closed tamper semantics, the binding row's refusal is re-derived from fixture bytes (oracle) and from a full STATE_ROOT_V1 recompute (this review), the arm row's refusal and its precedence are engine-asserted inside the fixture gate, and the corpus regenerates byte-identically. The residual is evidence-quality on the arm row and descriptor provenance: a P3 follow-up, below.

## Prior P4 — page-roots / page-entry-points chaining

The oracle now has a dedicated block (:546-571): cursor tag check (3 for roots, 1 for entry points), terminal `next_after is None` on page 2, an independent re-derivation of both pages via `compute()` + `page()`, `total_count == len(complete)` on each page, and `sel_1 + sel_2 == complete`.

`after == prior next_after` is still not asserted — and cannot be, because these are not truncated chains. The frozen fixture carries exactly one entry point (`0x0a…`) and one dependency root (`0x09…`) (`complete-entity-impact/v1/accepted.json` facts), the walk limit is `max_returned_entities: 2`, so page 1 is untruncated (`record_hex` header: total 1, returned 1, truncated=false, next=None) and page 2 uses a harness-chosen cursor `Some(Cursor::Root(only_root))` / `Some(Cursor::Entity(only_entry))` (`root_query.rs:2959-2991`) and returns zero items (header: total 1, returned 0, truncated=false). The harness comment (:2955-2958) says as much. These vectors pin tag-3 cursor decoding, the strict `>` predicate past the end, and exact `total_count`; they do not pin the truncated-emission path for `Roots` (`:1430-1443`, `items.last().copied().map(Cursor::Root)`) or `EntryRows` (`:1404-1417`). Nor does any unit test: `continuation_pages_union_to_the_complete_result_with_exact_counts` walks only class 4 (entity cursor) and class 12 (edge cursor), and the `Owned` fixture's single-element fact sets cannot truncate at any admissible limit (`validate_limits` rejects 0). By inspection the two arms are structurally identical to the pinned `Entities` arm (`:1318-1340`), so no defect is asserted; but contract section 9 bullet 2 ("a continuation walk for every paged class whose pages union to the complete result") is met only degenerately for classes 10/11. The prior P4 is superseded by a sharper P3 on corpus coverage.

One latent checker note: the union assertion (:570-571) assumes disjoint pages. In the current degenerate shape it holds because page 2 is empty; a future two-root fixture that keeps page 1 complete and page 2 after a mid cursor would produce a duplicate and a *false* `:union` failure. Fail-loud, so P4.

## Regression scan on the four carried P1s (P-C wave `209c661`, `c0f4ff6`; reformat `43f2f5b`)

`git diff -w c0f4ff6..43f2f5b` on the four engine files touches only `index_cache.rs` and `sley-repo/root_query.rs` and is import reordering plus line wraps (no token-level change); `sley-query/root_query.rs` and `snapshot.rs` have no diff in `43f2f5b`. The semantic P-C delta (`git diff -w a4b6029..c0f4ff6`) is:

- `sley-query/src/root_query.rs`: one new accessor `subject_entity()` (:441-449) and a harness reformat of the two tamper `println!`s. `verify()`, `validate_cursor`, `key_tag`, `page`, `page_items`, `canonical`, the closures, and the class computations are byte-unchanged in this range.
- `sley-repo/src/index_cache.rs`: guard-held entry (`guard.repository_root() != repository` → `RootIo`, :208-212), `read_record` (:234-248: `symlink_metadata` is_file, open, re-check `file.metadata().is_file()` on the pinned inode, `take(MAX_SNAPSHOT_RECORD_BYTES + 1)`), unique `create_new` temp names (:150-172), best-effort write-back (`let _ = write_record`, :232) with the non-file fail-closed check kept before it (:224-230), `CacheVerify` tri-state, and four new tests.
- `sley-repo/src/root_query.rs`: `run_root_query` takes the guard; new `run_root_query_fresh` builds the same `RootQueryInput` from `fresh_snapshot` and never consults the cache; `run_context_capsule` now routes through the fresh path.

1. **Facts ordering rule — still CLOSED.** `verify()` still requires `strictly_increasing(facts.entry_points)` and `strictly_increasing(facts.dependency_roots)` (`:267-271`) before the recompute; `unsorted_root_committed_fact_sets_are_root_mismatch` and the seven-fact matrix `caller_declared_facts_are_bound_to_the_committed_root` pass at HEAD. Classes 10/11 still emit verbatim (`:1112-1131`), so the guarantee rests on `verify()` exactly as before.
2. **Page-stitching predicate — still CLOSED.** `page()` (`:1275-1483`) and `page_items` (`:1485-1500`) unchanged: `total` is the pre-cursor complete count, the filter is strict `> key` on the class key, `truncated = remaining > limit`, `RequiredFactOmitted` when continuation is refused, `next_after` = last returned key iff truncated. `validate_cursor` (`:722-728`) still gates on `key_tag()` (`:402-414`, table unchanged: single-key 1/2/3/9 → `None`, 11 → `CURSOR_ROOT`, 12/13 → `CURSOR_EDGE`, else `CURSOR_ENTITY`) in both entry points before any paging, so `page()`'s `_ => None` fallbacks for mistyped cursors remain unreachable.
3. **Class-10/11 walk — still present and engine-emitted.** All 8 walk vectors are in the 27-vector corpus; `--check` reports zero drift; the tag-3 `Root` cursor round-trips (`page-roots-2.after = {tag: 3, root: 0x09…}`). Coverage depth is the P3 above, not a regression.
4. **`accept_cached` inversion residual — still CLOSED.** `accept_cached` (`index_cache.rs:108-126`) is unchanged: the only route to a `Hit` is `decode_complete_root_snapshot` (`:112`) followed by binding alignment; the decoder still refuses `decoded_reverse != invert_edges(&direct)` after the digest check (`snapshot.rs:742-748`). The new `read_record` feeds the same decoder (an oversized or truncated read fails digest/format and rebuilds — fail-closed); the guard check, symlink refusal, and best-effort write-back cannot serve an undecoded record. `run_root_query_fresh` builds from `build_complete_root_snapshot` directly and never reads the cache, so exported capsules are strictly stronger than before. 8 + 2 `sley-repo` tests pass, including `forged_inventory_corrupt_bytes_and_wrong_arm_are_discarded_and_rewritten` and `symlink_at_the_cache_path_fails_closed`.

No P1 regressed. Cursor-before-drift ordering (`execute_root_query :899-910`) is unchanged and still fail-closed either way.

## Out of scope (noted, ungraded)

S20-310 contract wording decision packet (`contract_text_review`, operator-held) — not judged. Ownership-binding composition — not judged. The trust-on-cache-dir boundary for a fully self-consistent forged record is unchanged from the prior round's ungraded note and is by design.

## Findings

No P0/P1/P2. The prior P2 is closed with an independent re-derivation; the prior P4 is superseded by one P3 (corpus coverage) and one P4 (checker note); one new P3 records the arm-row oracle gap and descriptor provenance.

```
VERDICT: PASS
SECTION: root_backed_query_profile
FIELD: vulcan_surface_review
SCOPE_SHA: 43f2f5ba738b587a9a0bb365a55f3096527e3e3d
FINDINGS:
[P3] [evidence] scripts/check_root_backed_query_vector.py:603-604 and scripts/generate_root_backed_query_fixtures.py:119-121 - the arm-1 rejection is adjudicated by the descriptor lookup `arm == 1 -> QUERY_PROFILE_UNSUPPORTED` with no data derivation (no `arm != 2` generality, no completeness model, no precedence-over-binding model), and both `tamper` descriptors are stamped by row id in the generator rather than emitted on the ROOT_QUERY_REJECT line; the refusal and its 31000-before-31008 precedence are engine-asserted (root_query.rs:3139) inside the `--check` gate, and the binding row is data-derived (substituted root != snapshot-fixture-pinned root; full STATE_ROOT_V1 recompute confirmed in this review), so this is an independence-claim gap, not a correctness gap; repair by emitting the tamper on the reject line and modelling arm as a preimage/context field
[P3] [evidence] conformance/root-backed-query/v1/accepted.json page-roots-2 / page-entry-points-2 vs crates/sley-query/src/root_query.rs:2955-2991,2052-2200 - the class-10/11 walks are degenerate: the fixture carries one entry point and one dependency root, page 1 is untruncated and page 2 is a harness-chosen past-the-end cursor returning zero items, so `after == prior next_after` is unassertable and the truncated-emission arms for Roots (:1430-1443) and EntryRows (:1404-1417) are exercised by no vector and no unit test; section 9 bullet 2 is met only degenerately for these classes; repair with a two-root/two-entry-point fixture or unit walk at limit 1
[P4] [evidence] scripts/check_root_backed_query_vector.py:570-571 - the class-10/11 union assertion assumes disjoint pages; it holds today only because page 2 is empty and would false-fail (loudly) on a non-degenerate untruncated-page-1 plus mid-cursor-page-2 shape
SUMMARY: At 43f2f5b every gate in the lane is green and re-derived: profile checker PASS (rev 5), fixture --check zero drift with the emitter asserts load-bearing under check=True, SHA256SUMS OK, the independent oracle PASS with problems [], and 7+8+2 Rust tests pass locked. The prior P2 is closed for a real reason: the oracle's tamper branch fails closed under eleven mutations of the frozen rows, the binding row's refusal is re-derived from fixture bytes by the oracle and by a shared-nothing STATE_ROOT_V1 recompute in this review (context root recomputes exactly from the 19 bindings and the facts; 0x09*32 and every single-fact tamper do not), and the arm row's refusal plus its precedence are engine-asserted inside the fixture gate. The four carried P1s show no regression across the P-C wave: verify() ordering, the page()/page_items predicate with pre-page cursor typing, the class-10/11 vectors, and the decode-only cache-hit path with the reverse-inversion check are all unchanged or strengthened (fresh-only capsules, guard-held cache, inode-pinned reads). Two P3 follow-ups remain on evidence depth (descriptor-driven arm adjudication and generator-stamped tamper provenance; degenerate class-10/11 continuation walks leaving the Roots/EntryRows truncated arms unpinned) and one P4 checker note; none reaches a caller.
```
