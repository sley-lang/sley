# Council qualification review — e050fe7

Three independent Council lanes completed on 2026-09-15 through Claude Code
(observed model `claude-fable-5-1`). Each reviewed four sections at
`e050fe75a86c1b2f0bf779561ac2bc70bce34289` against the preserved candidate source
`9b4f0648c9e226b0852d7a83b10ac6f550ecf45b`.

## Results

One PASS and eleven REVISE verdicts; no P0, P1, or P2 findings.
The 7 P3 and 30 P4 mentions include overlapping findings across lanes.

| Section | Ariadne | Nabu | Vulcan |
|---|---|---|---|
| root_backed_query_profile | [REVISE](../verdicts/root_backed_query_profile/ariadne-e050fe7.md) | [PASS](../verdicts/root_backed_query_profile/nabu-e050fe7.md) | [REVISE](../verdicts/root_backed_query_profile/vulcan-e050fe7.md) |
| reproducibility_and_independent_conformance | [REVISE](../verdicts/reproducibility_and_independent_conformance/ariadne-e050fe7.md) | [REVISE](../verdicts/reproducibility_and_independent_conformance/nabu-e050fe7.md) | [REVISE](../verdicts/reproducibility_and_independent_conformance/vulcan-e050fe7.md) |
| standards_sbom_and_provenance | [REVISE](../verdicts/standards_sbom_and_provenance/ariadne-e050fe7.md) | [REVISE](../verdicts/standards_sbom_and_provenance/nabu-e050fe7.md) | [REVISE](../verdicts/standards_sbom_and_provenance/vulcan-e050fe7.md) |
| decision_dossier | [REVISE](../verdicts/decision_dossier/ariadne-e050fe7.md) | [REVISE](../verdicts/decision_dossier/nabu-e050fe7.md) | [REVISE](../verdicts/decision_dossier/vulcan-e050fe7.md) |

## Main findings

- Missing historical review registration and a missing audit row for the current second-host mint.
- Build provenance and runbook language still name the pre-split smoke target.
- The artifact-content report needs explicit owner contract coverage and failure semantics.
- Several tests assert current qualification records, which can obstruct legitimate records-only updates.
- Additional P4 findings cover stale summary fields, validation precision, and refusal-path coverage.

## Validation and limits

The reviewers inspected source, contracts, tests, prior transcripts, and available
evidence. Their harness denied Python/checker/test execution. They did not rerun
the successful recovery gates recorded separately in
`evidence/validation/qualification-recovery-2026-09-15.json`.

The exact verdicts are filed in the machine summary and the full reports are
preserved. No package status or release approval is promoted. Findings remain
open; historical missing-round registration remains an explicit repair item.

The machine-readable index is [qualification-e050fe7.json](qualification-e050fe7.json).

Registration checks passed after filing the reports: `make evidence-refresh`,
`make release-candidate-verify`, the root-query checker, and all twelve transcript
hashes. These orchestrator checks do not close the reviewers' findings.
