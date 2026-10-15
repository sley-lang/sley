# Operator decision record: FAMILIAR-FRONTEND-2026-10-02 (RECORDED)

## The decision (verbatim excerpts, 2026-10-02)

> Decision: proceed with the familiar frontend described in ADR-0054,
> limited to an optional, noncanonical proposal adapter. This authorizes the
> narrow architectural amendment needed for that adapter, not a return to
> source-authoritative Sley.

> The approved policy is:
>
> "Optional, noncanonical authoring frontends may accept textual proposals
> when justified by machine utility. They must lower deterministically
> through the ordinary typed candidate and validation path. Their syntax,
> source files, formatting, and parser must not become canonical program
> authority or a prerequisite for the graph-native lifecycle."
>
> Preserve SSMC1, SCB1, SMP1, kernel authority, capabilities, stale-state
> checks, admission, and atomic commit semantics.
>
> Keep the constitutional change separate from the implementation change,
> following the repository's existing constitutional-change procedure. This
> is authorization for this specific boundary adjustment, not permission to
> weaken unrelated policies.

> Do not push, tag, release, install, or change GA claims.

(The operator's message also carried implementation, verification and
evidence instructions that do not bear on the governance text and are not
reproduced here. No signature, credential or token is attached to this file:
the authority is the operator's issued message, quoted so nothing is
paraphrased into a broader grant. This file records; the operator decided.)

## Recorded as

- ADR-0055 (`docs/adr/ADR-0055-familiar-authoring-frontend.md`): the
  amendment, the exact clauses it changes, and those it leaves unchanged.
- `docs/ANTI_GOALS.md` row "Sley source syntax or parser outside the optional
  authoring frontend" (carrier C-02) and the ADR-0055 paragraph of
  `CONTRIBUTING.md` "Prohibited additions" (carrier C-01).
- The additive frontend confinement check in
  `scripts/build_anti_goal_conformance.py`.

## Limits

This is a scope decision for one boundary adjustment. It is not a test
result, an independent review, a release authorization or a GA claim.
