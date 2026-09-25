# S20-720 Release Candidate Packaging Campaign (2026-09-03)

Status: contract draft revision 1 written by the integrator with every
Council lane unavailable; Ariadne contract review, Nabu architecture
review, and Vulcan surface review queued.

## Frontier at start

- The S20-430 `sley` endpoint dispatches every non-reserved SMP1 method
  (revision 7), so a source-independent demo can import, query, execute,
  report, branch, export, and clone-import through one binary.
- `make release-check` and `make v2` are fail-closed `NOT_IMPLEMENTED`
  gates; the root license, standards SBOM, provenance, and succession
  thresholds remain gated on the operator, S20-710 full, and S20-640.
- The local completion frontier names S20-720 mechanics as the next
  authority-safe work and its guard blocks the build script until the
  summary allows it.

## Design brief

Contract: `docs/spec/RELEASE_CANDIDATE_PACKAGING_V1.md`, ADR-0038, stage
checker `scripts/check_release_candidate_packaging.py`.

- Two clean release builds with the working tree remapped, a
  deterministic tar, a canonical manifest, the S20-710 inventory as SBOM,
  and a content scan for local paths and secrets.
- The artifact is unpacked outside the tree and exercised there: the
  conformance subset and the canonical demo over a fixture emitted from the
  executable test genesis.
- Reproducibility is a byte comparison; a difference is recorded with its
  members, never rounded.
- Codes 72000 through 72007; `release-check` stays fail-closed.

## Open questions for the reviews

- Whether the demo should also exercise `workspace.create` with a packaged
  trusted genesis input instead of creating the root by import.
- Whether the artifact should carry the full conformance corpus or only the
  protocol subset.
- Whether a `NOT_REPRODUCIBLE` archive-only difference should block the
  smoke or only be recorded.

## Records

| Stage | Commit | Tier 1 | Notes |
|---|---|---|---|
| Contract draft revision 1 | `4adaca8` | green | ADR-0038, stage checker |
| Mechanics, revision 2 | `cb51cb0` | green | build script, demo, fixture, 4 offline tests; smoke PASS: 1,990,615 bytes, 14 members, REPRODUCIBLE, demo 12 steps; first run refused ten registry paths (scan working); closeout `docs/audits/S20_720_RELEASE_CANDIDATE_CLOSEOUT.md` |

## Tier 2 handoff record (2026-09-03, at `cb51cb0`)

| Gate | Result | Wall time | Evidence |
|---|---|---:|---|
| `make core` | exit 0 | 15 s | 985 tests passed, 0 failed across 39 test binaries |
| `make conformance` | exit 0 | 12 s | 19 oracle results PASS |
| `make adversarial` | exit 0 | 9 s | 597 tests passed, 0 failed |
| `make fuzz-smoke` | exit 0 | under 1 s | 5 bounded smoke tests passed |
| `make release-candidate-smoke` | exit 0 | 24 s | two clean builds, artifact REPRODUCIBLE, conformance subset and demo PASS, stage checker PASS |
| `make sley2-runner-smoke` | exit 0 | 1 s | evidence PASS |
| `make accounting-smoke` | exit 0 | under 1 s | evidence PASS |

Logs were captured under the session scratchpad; `make v1` was skipped because this is a subsystem handoff, not a release boundary.
