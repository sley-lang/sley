# Full S20-300 complete-root index snapshot: campaign record (opened 2026-09-03)

Status: contract draft revision 1 committed; Council review pending.

## Why now

S20-520 merge landed the last Section 18.5 fuzz surface and the frontier's
next dependency-complete package is the full S20-300 complete-root snapshot,
which the full S20-250 profile unblocked (six bodies, strict root and object
extraction). The remaining gap the restricted profile named is useful safe
cache reuse.

## Council availability

Unchanged: every lane was still down when this draft was written; the retry
loop in the session scratchpad dispatches the queued S20-250, S20-510, and
S20-520 reviews first. This package's reviews follow.

## Design decisions taken by the integrator (pending review)

1. Same `SLEYIDX1` grammar and identity domain; new completeness arm 2
   (`CompleteRoot`) with a mandatory bound root and all eighteen kinds; each
   consumer accepts exactly one arm, so the restricted S20-310 and S20-320
   surfaces reject arm 2.
2. Provenance by build path (S20-250 full judgment over a verified
   revision), never by digest.
3. The one reuse path: the repository-owned `index/v1/<root>.idx.scb1`
   cache, accepted under four cheap rules (arm and context, digest,
   inventory equals the root's bindings, endpoint and inversion checks),
   consumed only by read-only derived query surfaces; validation,
   comparison, merge, commit, exchange, GC, and recovery never read it;
   `verify_cached_snapshot` audits by full rebuild.
4. Codes 30008 to 30010 appended.

## Open questions for the reviews

- Whether the four cache rules are enough, or a per-repository cache
  authentication (for example a receipt-bound snapshot manifest) is required
  before any surface other than S20-310 may consume a hit.
- Whether cache writes should require exclusive maintenance rather than
  shared maintenance with temp-and-rename.

## Commits

- contract draft revision 1: the commit that adds this file.

## Implementation under the draft (2026-09-03)

The Council lanes were still unavailable, so the integrator implemented the
full profile under contract draft revision 1 with the reviews queued:

- `crates/sley-query/src/snapshot.rs`: arm `2` (`CompleteRoot`) on the
  frozen `SLEYIDX1` record, `build_complete_root_snapshot`,
  `admit_complete_root_snapshot`, `decode_complete_root_snapshot`, the
  arm-aware inspector, `CacheDiscardReason::RootMismatch`, codes 30008
  through 30010; restricted query and capsule consumers fail closed on
  arm `2`.
- `crates/sley-repo/src/index_cache.rs`: `index/v1/<root hex>.idx.scb1`
  cache with `complete_root_snapshot` (hit without object access under the
  four acceptance rules, else rebuild and rewrite) and
  `verify_cached_snapshot`; `index` joins the incomplete-clone layout
  allowlist.
- Fixed vector over the frozen S20-250 fixture: 5,888 bytes, identity
  `8cd104d09967263e6422b759bd58bff6f881d48ccf5b212856fe832c5c64023d`;
  `conformance/complete-root-index-snapshot/v1` with six discard
  candidates; independent oracle
  `scripts/check_complete_root_index_snapshot_vector.py` PASS (record,
  identity, and all six codes reproduced from the S20-250 fixture edges).
- Persistent fuzz `fuzz/targets/complete_root_snapshot_decoder.rs`, smoke
  PASS over 480 seeds in 11 seconds.
- Native: `sley-query` 54 tests, `sley-repo` 344 tests; clippy clean on the
  new paths (pre-existing exchange and GC test debt unchanged).
- Closeout: `docs/audits/S20_300_FULL_COMPLETE_ROOT_SNAPSHOT_CLOSEOUT.md`;
  summary status `S20_300_FULL_IMPLEMENTED_REVIEW_PENDING`; frontier
  re-anchored to full root-backed S20-310.

Tier 2 results are appended below when the handoff gate runs.
