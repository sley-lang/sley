# S20-750 decision dossier campaign (2026-09-03)

Package: the dossier mechanics of S20-750 (complete dossier, machine summary,
decision record), dependencies S20-640 and S20-740, phase M6. Owner of record
Codex; executed by the integrator because every Council lane was unavailable
(ADR-0026). The dossier reports; it makes no release decision, claims no GA,
and `make release-check` and `make v2` stay fail-closed.

## Contract

- `docs/spec/DECISION_DOSSIER_V1.md`, draft revision 1, with
  `docs/adr/ADR-0043-decision-dossier-derived-not-decided.md`. Section 1.1
  lists the thirty-four master-goal section 30 items verbatim, so the checker
  verifies coverage without reading a document outside the repository.
- Staged checker `scripts/check_decision_dossier.py` in `make quick`; summary
  section `decision_dossier`; work package row updated; error codes 76000
  through 76003 reserved in `docs/spec/ERROR_CODES_V1.md`.

## Mechanics

| Surface | File | What it does |
|---|---|---|
| Dossier | `scripts/build_decision_dossier.py` | resolves every section 30 item from the machine summary and the tracked S20-710 full, S20-720, S20-730, and S20-740 reports, marks the unevidenced ones `GATED` with the missing authority named, and derives the section 27 decision state with its exact reasons |
| Tests | `bench/review/tests/test_decision_dossier.py` | 11 offline tests: contract coverage and order, count agreement, gated items carry no value, evidenced items name existing tracked paths, no publication claim, no host path, purity, the current `BLOCKED` derivation, each blocking condition, the precedence to `FAIL`, `ALPHA_COMPLETE`, `CONDITIONAL_PASS`, and `PASS`, and the missing-source refusal |

`make release-candidate-smoke` now also rebuilds the finding register and the
dossier after a candidate build, so the whole evidence chain names the commit
it was built from.

## Result at this commit

- 34 entries: 23 `EVIDENCED`, 11 `GATED`.
- Gated items: final branch; property-test counts; security review result; the
  six per-arm succession metrics (strict correctness, ACT, context bytes and
  tokens, repair loops, invalid committed states, stale candidates); the
  independent review result; and the release decision state itself.
- Derived decision state `BLOCKED`, with six reasons: sixty-three open review
  obligations, twelve deferred review lanes, an unapproved root license text,
  no executed succession trial, fail-closed `release-check` and `v2` gates, and
  a single attesting host.
- `decision_authority` reads `OPERATOR_DECISION_NOT_DELEGATED`, and every
  publication flag is false.

## Open questions for the Council

1. Ariadne: should a partially evidenced item (fuzz coverage recorded but no
   timed campaign) be `EVIDENCED` with a null sub-field, as it is today, or a
   third state such as `PARTIAL`?
2. Nabu: the dossier reads the machine summary and five derived reports; should
   it instead read only the derived reports, with the summary reduced to a
   pointer index?
3. Vulcan: is `BLOCKED` the right state while implementation continues, or
   should an incomplete goal derive `FAIL` until every gate exists?

## Validation

Landed at `59d3acd`. Tier 1 `make quick` passed at the commit. Tier 2 ran on
2026-09-03 at that commit:

| Gate | Result | Wall time | Evidence |
|---|---|---:|---|
| `make core` | exit 0 | 13 s | 995 tests passed, 0 failed |
| `make conformance` | exit 0 | 8 s | 19 oracle results PASS |
| `make adversarial` | exit 0 | 10 s | 597 tests passed, 0 failed |
| `make fuzz-smoke` | exit 0 | under 1 s | 5 bounded smoke tests passed |
| `make release-candidate-smoke` | exit 0 | 32 s | ten `PASS` results, two builds REPRODUCIBLE, the whole evidence chain rebuilt and the dossier still `BLOCKED` |

`make v1` was not run: this is a subsystem handoff, not a release boundary.
