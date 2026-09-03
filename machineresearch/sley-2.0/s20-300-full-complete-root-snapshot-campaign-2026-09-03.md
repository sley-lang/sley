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
