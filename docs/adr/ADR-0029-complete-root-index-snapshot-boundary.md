# ADR-0029: Complete-root index snapshot and cache reuse boundary

Status: proposed; the S20-300 full contract is a draft at revision 1 with
Council review pending; implementation pending

Date: 2026-09-03

## Context

The restricted S20-300 profile froze a rebuild-first `SLEYIDX1` record over
modeled kinds 4 through 15 with an unverified root claim and no useful cache
reuse, naming three gaps: the six missing bodies, strict root and object
extraction, and useful safe cache reuse. The full S20-250 profile now
supplies the eighteen-kind complete-root judgment over verified objects, so
the first two gaps close by composition. The work-package row names "cache
becomes authority" as the primary risk.

The Council lanes were still unavailable at this draft (see ADR-0026); the
design is the integrator's and is submitted to Ariadne, Nabu, and Vulcan as
soon as a lane returns.

## Decision

1. **One record, two arms.** The frozen `SLEYIDX1` grammar and identity
   domain are unchanged; arm `2` (`CompleteRoot`) binds the exact root and
   carries all eighteen kinds. Arm `1` keeps every restricted vector and
   consumer; each consumer accepts exactly one arm.
2. **Provenance by build path.** An arm-`2` record is built only from a
   complete-root request that passed closure rule C1 against the root's
   bindings, and in the repository only from a verified revision. The
   digest still authenticates bytes only.
3. **One reuse path.** The repository-owned cache under `index/v1/` is the
   only place a snapshot is accepted without a rebuild, under four cheap
   rules (arm and context, digest, inventory equals the root's bindings,
   edge endpoints and inversion), and only for read-only derived query
   surfaces. Validation, comparison, merge, commit, exchange, GC, and
   recovery never read it; `verify_cached_snapshot` exists for audits.
4. **Derived and disposable.** Cache files sit outside the object store and
   every retention root, are never packed or exchanged, and may be deleted
   at any time; `index` joins the incomplete-clone layout allowlist.
5. **Codes.** Three codes 30008 through 30010 are appended to the frozen
   30000 range; every wrapped `IMPACT_*`, `STORE_*`, `TXN_*`, and `SCB_*`
   code is preserved.
6. **Staging.** `scripts/check_complete_root_index_snapshot_profile.py`
   binds the contract, ADR, work-package row, and summary section, and
   fails closed if the complete arm or the cache module appears before the
   summary allows implementation.

## Consequences

- Root-backed S20-310 queries become dependency-complete once this profile
  and its reviews land; the restricted S20-310 and S20-320 surfaces are
  unchanged and reject arm `2`.
- The residual cache risk is stated, bounded, and auditable rather than
  hidden behind a rebuild that would make the cache useless.
