Baseline verified: `git rev-parse HEAD` = `db1bc623d01e838d49c153feb0be05a7502b8794`. Read-only review except this verdict file; `machine-summary.json` untouched. Working tree clean apart from one other lane's untracked verdict draft; no tracked file is modified. Prior lane verdict `vulcan_surface_review-a4b6029.md` (root-backed-query REVISE, load-bearing P2: 2 rev-5 corpus rows harness-only) read first; Ariadne (PASS) and Nabu (PASS) finals stand and are not re-litigated. Scope of the post-verdict repair, per `git diff a4b6029..db1bc62 --stat`: `scripts/check_root_backed_query_vector.py` (+70), `scripts/generate_root_backed_query_fixtures.py` (+29), `conformance/root-backed-query/v1/rejected.json` (+6), `conformance/root-backed-query/v1/SHA256SUMS` (re-mint). Untouched by the repair: `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` (zero diff — S20-310 wording NOT touched, rev 5 byte-identical), `crates/sley-query/src/root_query.rs` (zero diff — engine grounds of the prior verdict stand as read), `scripts/check_root_backed_query_profile.py` (zero diff), `conformance/root-backed-query/v1/accepted.json` (SHA256 `6cb756ebd96cb6d501656c6ca0f70bce57d8a4671bddd4c887286413e752ac05` unchanged).

## Gate runs at `db1bc62`

| Check | Result |
|---|---|
| `python3 scripts/generate_root_backed_query_fixtures.py --check` | PASS, `{"drift": [], "vectors": 27, "rejections": 10}` — live engine re-emitted all 27 vectors and 10 rejections with zero drift against the frozen corpus (fixture regeneration re-derived from primaries, read-only mode, tree unmodified) |
| `sha256sum -c SHA256SUMS` (run inside `conformance/root-backed-query/v1/`) | OK, `accepted.json` + `rejected.json` (rejected re-minted to `a7e78d0b…6f5993`; accepted hash unchanged) |
| `uv run --project oracle/scb1 --frozen python scripts/check_root_backed_query_vector.py` | **PASS**, exit 0, `problems: []`, `vectors: 27, mutations: 10, query_classes: 19` — the prior `binding-substituted-fact:accepted`, `arm-1-snapshot-profile:accepted` failures are gone |
| `python3 scripts/check_root_backed_query_profile.py` | PASS, revision 5, `S20_310_FULL_IMPLEMENTED_REVIEW_PENDING`, zero problems |
| `cargo test -p sley-query` | ok, `102 passed; 0 failed; 4 ignored` (106-test binary) — matches the expected 102 |

## P2 fix is real, not theater (adversarial confirmation)

**Mechanism, per-row.** Both rev-5 mutation rows carry an honest class-1 query whose unmutated form the oracle ACCEPTS (re-derived live: `qid=ca3e7a29…`, `record_bytes=640` for both), so the tamper cannot be masking an already-failing request — a masked tamper is a gate problem (`tamper-masks`, checker `:594-598`). The recorded tampers are the exact engine-applied mutations, not descriptions: `binding-substituted-fact` records `tamper.substituted_root = "09"*32` (`rejected.json`), which is byte-identical to the engine emitter's `tampered.root = StateRoot::from_bytes([0x09; 32])` (`root_query.rs:3077`, asserted `31_008` at `:3092`); the oracle observes inequality against the honest context root `bcf36d94…` (`:605-610`) and applies spec §1 rule 5 → `QUERY_ROOT_MISMATCH`, matching expected code/numeric 31008. `arm-1-snapshot-profile` records `tamper.arm = 1`, matching the engine emitter's restricted-kind-subset arm-1 snapshot (`:3115-3135`, asserted `31_000` at `:3135`); the oracle applies spec §1 arm rule / §8 item 2 → `QUERY_PROFILE_UNSUPPORTED`, matching 31000. Oracle reads that observe the corruption: the honest-accept run (proves the mutation row is live) plus the substituted-vs-context-root inequality comparison (proves the corruption names a real substitution). The generator is the tamper source of truth (`generate_root_backed_query_fixtures.py:115-122`, keyed by mutation id to the engine-identical constant), so frozen bytes and oracle agree without trusting either alone.

**Negative control, actually executed.** Two vacuous-pass vectors were constructed from the real `binding-substituted-fact` row and run through the gate's tamper branch live: (1) `tamper.substituted_root == context.root_hex` (a corruption the oracle provably cannot observe — the "substitution" is the honest root) → gate emits `tamper-not-substituted` (checker `:607-609`), i.e. REJECTED; (2) `tamper = {"note": "does-nothing"}` (unknown key, no observable effect) → gate emits `tamper-empty` (`:611`), i.e. REJECTED. Both real rows in the same harness flip to their expected codes (`QUERY_ROOT_MISMATCH`, `QUERY_PROFILE_UNSUPPORTED`). A vacuous pass is therefore inexpressible: any tamper that fails to denote an observed substitution is a named problem, not a silent accept. No infeasibility caveat applies — the control ran against the shipped checker logic.

## Tag-3 / page-union properties

`page-roots-2` carries the tag-3 `Root` cursor (`after.root = 0909…09`, tag 3) and `page-entry-points-2` the tag-1 entity cursor; both `-1` pages are complete (`next_after: null`). The checker's rev-5 walk-union block (`:542-571`) re-derives both pages from primaries via fresh `compute()` + `page()` calls (not frozen bytes), enforcing cursor-tag (`:551-553`), terminal-chain (`:554-555`), exact totals on each page (`:568-569`), and disjoint complete union (`:570-571`). The pre-existing `page-namespaces` / `page-edges` after==prior-next_after chaining assertions (`:535-541`) are unchanged. The prior P4 (walks byte-pinned but chaining unasserted) is CLOSED.

## Carried P1s (engine files untouched — prior derivations stand as read)

Facts ordering (`verify()` strictly-increasing, `root_query.rs:267-271`), page-stitching predicate (`page()` `:1266-1494` + `validate_cursor` `:713-719`), class-10/11 continuation walks (8 walk vectors, zero drift), `accept_cached` inversion (`snapshot.rs:741` via `index_cache.rs:112`) — no engine byte changed since `a4b6029`, so all four remain CLOSED on the prior grounds without re-derivation. `seq_root_advanced` 33002 rationale and proving-test citation untouched.

## Out of scope (noted, ungraded)

S20-310 contract wording (byte-identical, excluded); ownership binding composition (held-gated); Rust-consumes-vectors harness direction (still not built); machine-summary lane dispositions of other lanes.

## Findings

One P4 (checker-linkage nit, no wrong-answer path). No P0/P1/P2/P3: the evidence gate passes 27/10, every corruption is load-bearing-proven, and no wrong answer reaches any caller.

```
VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_1_P4
SECTION: root_backed_query_profile
FIELD: vulcan_surface_review
SCOPE_SHA: db1bc623d01e838d49c153feb0be05a7502b8794
FINDINGS:
[P4] [review-pinned-not-gate-pinned] scripts/generate_root_backed_query_fixtures.py:115-122 - the recorded tamper constants ("09"*32, arm 1) are mapped by mutation id, not parsed from the engine's ROOT_QUERY_REJECT lines, so a future engine-constant drift (e.g. 0x09 -> 0x0A at root_query.rs:3077) would pass both --check (frozen and regenerated tampers agree by construction) and the oracle (any substituted != context root still flips); the tamper-to-engine link is pinned only by code reading, verified true at this SHA. Impact is descriptive accuracy only: the load-bearing proof depends solely on substituted != honest root plus spec section 1 rule 5, which a stale constant would still satisfy. Harden by emitting the substituted value in the REJECT line, or leave as review-pinned.
SUMMARY: At db1bc62 the prior REVISE P2 is closed for real: both rev-5 rows now carry the engine-identical tamper (0x09-root substitution, arm-1 snapshot), the oracle proves each tamper load-bearing by honest-accept-then-flip with an inequality the corruption observably alters, the vector gate passes 27/10 with zero problems, and executed negative controls prove a vacuous-pass vector is rejected (tamper-not-substituted / tamper-empty) rather than silently accepted. Tag-3 cursors and walk-union re-derivation are enforced for page-roots and page-entry-points (prior P4 closed); S20-310 wording, the engine, the profile checker, and accepted.json are byte-untouched. The package is PASS with one P4 linkage nit.
```
