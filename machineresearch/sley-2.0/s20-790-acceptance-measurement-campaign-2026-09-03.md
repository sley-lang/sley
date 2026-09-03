# Acceptance measurement campaign (2026-09-03)

Three master-goal control tables existed as prose with no measurement. Each is
now derived, drift-checked in `make quick`, and rebuilt by
`make evidence-refresh`. None of them decides anything: every one states
explicitly that a review-only or gated entry is not a pass.

## What was missing

| Control table | Master goal | State before |
|---|---|---|
| `docs/THREAT_REGISTER.md` (55 threats) | section 26.6 "all P0/P1 threats have passing tests" | a planned-control map; no measurement of the plan |
| `docs/ANTI_GOALS.md` (26 prohibitions) | section 28 non-goals | "must remain mechanically testable"; nothing tested them |
| Section 26 acceptance criteria (52) | GA gate | no mapping from a criterion to its evidence |

## What was built

- `scripts/build_threat_coverage_report.py` locates each threat's expected
  failure code and classifies it. Result: 24 exercised, 6 with their planned
  evidence directory, 25 unlocated, of which 19 have a realized code family
  (so the control most likely exists under a more specific name) and 6 have no
  family symbol at all. Those six (T07, T08, T35, T50, T53, T55) are the sharp
  end of the security review's work list.
- `scripts/build_anti_goal_conformance.py` evaluates every prohibition whose
  acceptance evidence is mechanical: 9 HOLDS, 16 REVIEW_ONLY, 0 violated. The
  nine include no parser or language-service crate in the lock, no networking
  crate (the whole lock is 40 crates), no Greyforge product dependency, no
  process invocation in any kernel source, `unsafe_code = "forbid"` with no
  kernel exception, no legacy crate, and no git tag with 377 unpushed commits.
- `scripts/build_ga_acceptance_report.py` maps all 52 section 26 criteria to
  their evidence: **34 evidenced, 16 awaiting review, 2 gated**. The two gated
  are the section 22 succession thresholds (no trial executed) and "artifact is
  built from the final candidate commit" (the final commit is fixed at the
  release decision). The dossier's release decision item now cites those
  numbers.

## Finding: a self-referential false positive

The threat coverage generator named `SCB_MALFORMED` in its own docstring as an
example, which made T01 count as a located and exercised control. An evidence
tool that reads the tree can satisfy its own search, so the generator now
excludes itself, and the same exclusion pattern guards the clean-room checker.
The corrected report moved T01 back to unlocated with its family candidates
listed.

## Validation

Landed at `848e0b0`, `e6ae074`, `0588854`, and `502e572`. Tier 1 `make quick`
passed at each commit. Tier 2 on 2026-09-03: `make core`, `make conformance`,
`make adversarial`, and `make fuzz-smoke` all exit 0.

## Open questions for the Council

1. Vulcan: are the six threats with no family symbol genuinely uncovered, or
   realized under names the family heuristic cannot see?
2. Ariadne: should the 16 REVIEW_ONLY anti-goals gain mechanical evidence, or
   are they irreducibly human judgments?
3. Nabu: the acceptance report derives states from other derived reports; is
   that layering acceptable, or should each criterion cite a primary artifact?
