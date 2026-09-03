# S20-300 Full Complete-Root Snapshot Closeout

Status: **implemented under the draft Complete-Root Index Snapshot Profile v1 contract (revision 1); Council reviews pending, so the package is not complete; the Sley 2 goal remains incomplete**

Date: 2026-09-03

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
  corpus, oracle, and fuzz slice in the commit after it.
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

## Explicitly open and deferred

- **Council reviews.** Ariadne, Nabu, and Vulcan reviews are queued behind
  the S20-250, S20-510, and S20-520 reviews and land as contract revisions.
- The residual cache risk named in the contract (a digest-valid forged
  record with the correct inventory and wrong edges served to a read-only
  query) is accepted as bounded, not eliminated; `verify_cached_snapshot`
  exists for audits and no non-query surface reads the cache.
- The repository builder is exercised over synthetic genesis repositories;
  no multi-transaction repository with a cache across heads exists yet, and
  no consumer of a cache hit exists until full S20-310 lands.
- Strict pedantic clippy debt in older `sley-repo` exchange and GC test
  modules is pre-existing; the new paths lint clean under `--no-deps`.

## Validation record

Tier 1 `make quick` passed at every commit of the slice. Tier 2 ran on
2026-09-03 after the implementation commit: `make core`, `make conformance`
(including the complete-root snapshot oracle line), `make adversarial`,
`make fuzz-smoke`, and `make complete-root-snapshot-persistent-fuzz-smoke`;
results are recorded in the campaign record
`machineresearch/sley-2.0/s20-300-full-complete-root-snapshot-campaign-2026-09-03.md`.
The full `make v1` gate was skipped because this is a subsystem handoff, not
a release boundary; `make v2` and `make release-check` remain intentionally
fail closed.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.
