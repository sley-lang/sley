# S20-300 Full Complete-Root Snapshot Closeout

Status: **implemented under the draft Complete-Root Index Snapshot Profile v1 contract (revision 3); the three Council review rounds landed 2026-09-04 (all FAIL) and revision 3 closes every report-grade finding below; the package awaits re-review, so it is not complete; the Sley 2 goal remains incomplete**

Date: 2026-09-03; revised 2026-09-14 (revision 3)

Validation tier: **Tier 1 plus query-and-repository-focused Tier 2 handoff**

## Claim under review

The frozen `SLEYIDX1` record gains completeness arm `2` (`CompleteRoot`):
a snapshot bound to the exact `StateRoot` it indexes, carrying all eighteen
SSMC1 entity kinds, derived only through the S20-250 full complete-root
judgment (closure rules C1 through C11) over a request whose inventory is
the root's bindings. The restricted arm `1`, its vectors, its codes 30000
through 30007, and its rebuild-first admission are byte-identical. The
repository owns the single reuse path: a cache under `index/v1/` keyed by
root hex, written by temp-and-rename after a fresh build from a verified
revision, and accepted without object decoding only when the cached record
bounded-decodes with the exact arm and context, its trailer is its derived
identity, its inventory equals the revision's bindings, and its edges and
reverse groups are consistent. The contract is
`docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md` with ADR-0029. It is
a draft: every Council lane was unavailable when it was written and when the
implementation landed, so the reviews that freeze it and complete the
package are pending and must pass before the status above changes.

The implementation provides:

- `sley-query`: `IndexCompleteness::CompleteRoot` (tag 2),
  `build_complete_root_snapshot`, `admit_complete_root_snapshot`,
  `decode_complete_root_snapshot`, the arm-aware inspector that accepts
  kinds 1 through 18 only under arm `2` and requires a bound root there,
  `CacheDiscardReason::RootMismatch`, and the three codes 30008 through
  30010 (`INDEX_SNAPSHOT_ROOT_INCOMPLETE`, `INDEX_SNAPSHOT_ROOT_MISMATCH`,
  `INDEX_SNAPSHOT_IO`) with every wrapped `IMPACT_*` code preserved; the
  restricted query builder and the capsule fail closed with
  `QUERY_UNSUPPORTED` on an arm-`2` snapshot;
- `sley-repo`: `index_cache.rs` with `index_cache_path`,
  `complete_root_snapshot(repository, revision)` returning `Hit` or
  `Rebuilt { reason, .. }`, and `verify_cached_snapshot`; the `index`
  directory joins the frozen incomplete-clone layout allowlist in
  `exchange.rs`.

The S20-250 full judgment, the S20-390 verified revision loader, and the
restricted S20-300 record are consumed exactly as frozen; no validation,
comparison, merge, commit, exchange, GC, or recovery path reads the cache.

## Evidence

- Contract draft revision 1 and ADR-0029 at `9111c0e`; implementation,
  corpus, oracle, and fuzz slice at `094a7bf`.
- Fixed vector: the arm-`2` record over the frozen eighteen-kind S20-250
  fixture is 5,888 bytes with identity
  `8cd104d09967263e6422b759bd58bff6f881d48ccf5b212856fe832c5c64023d`,
  stable over 128 rebuilds, different from the arm-`1` record and from the
  same request under another root.
- Conformance corpus: `conformance/complete-root-index-snapshot/v1/accepted.json`
  (the record, identity, root, epoch, and a digest binding to the S20-250
  fixture request) and `rejected.json` (six discard candidates: trailer bit,
  format version, restricted arm tag, other root, rootless context,
  truncation), drift-gated by
  `scripts/generate_complete_root_index_snapshot_fixtures.py --check` in
  `make quick`; `scripts/check_complete_root_index_snapshot_vector.py`
  rebuilds the record bytes and `IndexSnapshotId` from the S20-250 fixture's
  entities and frozen edge set with its own encoder and BLAKE3 domain, then
  classifies every rejected candidate with its own bounded inspector,
  registered in `make conformance` under the frozen oracle environment.
- Native tests: three new `sley-query` snapshot tests (the fixed vector and
  arm inequality with the rootless and wrong-arm rejections and the
  restricted-consumer fail-closed proof; impact failures preserved as
  `INDEX_SNAPSHOT_ROOT_INCOMPLETE`; the full arm-`2` discard matrix
  including content mismatch), three `sley-repo` cache tests (miss then hit
  with the object store made unreadable, forged inventory discarded with
  `INDEX_SNAPSHOT_ROOT_MISMATCH`, corrupt bytes and wrong arm rewritten, a
  second root cached beside the first, `verify_cached_snapshot` equality),
  and one exchange test (an incomplete clone carrying `index/v1` still
  resumes while any other stray entry stays `EXCHANGE_TARGET_NOT_EMPTY`);
  `sley-query` 54 tests and `sley-repo` 344 tests pass.
- Persistent fuzz: `fuzz/targets/complete_root_snapshot_decoder.rs` (arm-2
  decoder in a direct lane and a trailer-rehash lane with the context read
  from the candidate header), smoke `PASS` over a 480-seed corpus
  (`docs/audits/S20_700_COMPLETE_ROOT_SNAPSHOT_PERSISTENT_SLICE.md`);
  sixteen scoped targets and fifteen smoke gates now stand; the S20-700
  finding register and independent review remain deferred.
- Tier 1: `make quick` green at every commit.
- Tier 2: see the validation record below.

## Findings closed in flight

- The restricted inspector mapped every kind tag through the twelve
  modeled kinds; the arm-aware inspector takes the completeness arm before
  the inventory so an arm-`2` candidate can never be read as arm `1` and
  a restricted candidate with kinds outside 4 through 15 still fails.
- The cache writer refuses to follow or overwrite a non-file at the cache
  path and never returns a cached record that failed any of the four
  acceptance rules, so a discarded file is always rewritten from the fresh
  build.

## Findings closed in revision 3 (2026-09-14)

- **Closed with code.** Unique exclusive temp files (`create_new` over
  `<name>.tmp.<pid>.<counter>`); guard-held cache access (callers pass
  shared maintenance over the same repository, mismatch refused; threaded
  through S20-310 queries, the server, and all tests); fail-open cache I/O
  (read/metadata trouble rebuilds, write-back is best-effort, tampering
  still fails closed); handle-pinned bounded cache reads; fresh-only
  exported capsules (`run_root_query_fresh` + fresh `run_context_capsule`,
  server capsule path included); `verify_cached_snapshot` distinguishes
  `Match`/`Missing`/`Mismatch`; shared `discard_reason` mapping (dup
  removed); decoder checks the digest before the inversion, matching the
  rule order; fuzz target asserts the decoder partition (30000-30007).
- **Closed with text.** Forged-kind residual named (kinds, like edges, are
  digest-bound but not cross-checked — same three bounds); context-check
  precedence stated (rootless arm-2 under a rooted context is
  `CONTEXT_MISMATCH`, matching code and fixture); Hit-export ban with the
  narrowed transient-read grant; determinism invariant; no-eviction rule;
  code-surfacing rule; strictly-ascending-unique inventory order; §9
  consumer state.
- **Verified absent (no change).** The fuzz `OPTION_NONE` arm IS exercised
  (a missing option tag yields a rootless expected context); no code
  needed, recorded here so it is not re-raised.
- **Checker.** Revision anchored and pinned (`SPEC_REVISION = 3`) with the
  summary cross-check; cache-caller allowlist (only the designated
  transient-read surface); fresh-only capsule pin; guard and exclusivity
  markers.

## Explicitly open and deferred

- **Council reviews.** Ariadne, Nabu, and Vulcan reviews landed 2026-09-04
  (all FAIL: 2 P0s with one shared root cause, 13 P1s); revision 3 below
  closes every report-grade finding, and re-review is queued.
- The residual cache risk named in the contract (a digest-valid forged
  record with the correct inventory and wrong edges — or wrong inventory
  kinds, which the alignment binds by identity only — served to a
  read-only query) is accepted as bounded, not eliminated;
  `verify_cached_snapshot` distinguishes missing from differing caches for
  audits, and no non-query surface reads the cache. Exported capsules
  never rest on bare hits (fresh-only capsule paths in `root_query` and
  the server).
- The repository builder is exercised over synthetic genesis repositories;
  no multi-transaction repository with a cache across heads exists yet.
  S20-310 root-backed queries consume cache hits for transient reads
  today; the full S20-320 capsule builds fresh.
- Strict pedantic clippy debt in older `sley-repo` exchange and GC test
  modules is pre-existing; the new paths lint clean under `--no-deps`.

## Validation record
Tier 1 `make quick` passed at every commit of the slice. Tier 2 ran on
2026-09-03 at `094a7bf`: `make core` (941 tests), `make conformance`
(including the complete-root snapshot oracle line), `make adversarial`,
`make fuzz-smoke`, and `make complete-root-snapshot-persistent-fuzz-smoke`
all exited 0 in 36 seconds; the per-gate record is in
`machineresearch/sley-2.0/s20-300-full-complete-root-snapshot-campaign-2026-09-03.md`.
The full `make v1` gate was skipped because this is a subsystem handoff, not
a release boundary; `make v2` and `make release-check` remain intentionally
fail closed.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.
