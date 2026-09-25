# ADR-0030: Root-backed query classes and continuation boundary

Status: proposed; the S20-310 full contract is a draft at revision 7 with
Council review pending; implemented under the draft
(`docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md`)

Note (2026-09-15): Revision 7 records the section 9 evidence boundary and the unit-walk query. The closeout and current contract record subsequent revisions; the context below preserves the original decision rationale.

Date: 2026-09-03

## Context

The restricted S20-310 profile froze four typed queries over an arm-1
snapshot with hard failure whenever an applied limit would omit a fact,
because without lawful continuation a partial answer is indistinguishable
from a complete one (ADR-0012). The master goal requires nineteen
root-backed query classes, but no authority in the repository enumerates
them: the count survives in the work-package row, the error-code registry,
and the machine summary (`full_query_classes_required: 19`), and the legacy
Sley exposed six structural query kinds. The full S20-300 profile now binds
an arm-2 snapshot to an exact verified root and owns the repository cache
whose hits may serve read-only derived query surfaces only.

The Council lanes were still unavailable at this draft (see ADR-0026); the
class enumeration is the integrator's and is submitted to Ariadne, Nabu,
and Vulcan as soon as a lane returns.

## Decision

1. **Nineteen classes, one input.** The contract enumerates the classes
   over the verified root's facts (record, bindings, bodies, field-4
   fingerprints) and the arm-2 snapshot's edges, with a class-kind
   applicability table; nothing is answered from labels, paths, or text.
2. **Exact then paged.** Every list class is computed completely under the
   work ceiling and then paged by canonical key; `total_count` is exact on
   every page, `after` and `next_after` are typed cursors, and paging
   happens only when the caller sets `allow_continuation`. Without it the
   restricted rule holds unchanged. Closure depth and single-key classes
   never page.
3. **Binding before answering.** The snapshot, bodies, bindings, and root
   facts must agree exactly (`QUERY_ROOT_MISMATCH`), so a cache hit can
   supply edges but never an inventory the record disagrees with.
4. **One repository surface.** `sley-repo` is the only producer of the
   input from persistent state, over an S20-390 verified revision and the
   S20-300 cache; it is the first consumer of a cache hit and stays
   read-only derived evidence.
5. **Identity and codes.** The thirty-third domain `sley2.root-query.v1`
   names the request; `SLEYRQQ1`/`SLEYRQR1` records are new and the
   restricted `SLEYQRY1`/`SLEYQRS1` records are untouched; codes 31008
   through 31010 are appended to the frozen 31000 range.
6. **Staging.** `scripts/check_root_backed_query_profile.py` binds the
   contract, ADR, work-package row, and summary section, and fails closed if
   the root-query module appears before the summary allows implementation.
7. **Entity-read composition.** Section 11 of the root-backed query
   contract composes this profile with `ENTITY_READ_PROFILE_V2.md`.
   Entity payload projection and its independent corpus checker remain
   owned by the entity-read profile; root-backed queries retain their
   nineteen-class enumeration, binding and continuation authority.
   Shared verified-root input does not make either profile's acceptance
   evidence a substitute for the other's.

## Consequences

- Full S20-320 capsules, S20-330 handles, and the S20-400 SMP1 transport
  gain a frozen query surface to wrap, bind, and carry; the restricted
  S20-320 capsule keeps wrapping only the restricted responses.
- Continuation is lawful because it is explicit, keyed, and total-counted;
  the T14 gate (truncation hides required facts) is preserved by
  construction rather than by refusing to page.
