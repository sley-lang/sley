# Council repair review — 400895e

All three independent claude-code sessions completed. Four PASS and eleven REVISE verdicts, with fourteen overlapping P4 mentions and no P0–P3 findings. All original e050fe7 findings were confirmed closed. Reviewers performed static inspection; their execution limitations are stated in the transcripts.

| Section | Ariadne | Nabu | Vulcan |
|---|---|---|---|
| root_backed_query_profile | [PASS](../verdicts/root_backed_query_profile/ariadne-400895e.md) | [PASS](../verdicts/root_backed_query_profile/nabu-400895e.md) | [REVISE](../verdicts/root_backed_query_profile/vulcan-400895e.md) |
| reproducibility_and_independent_conformance | [REVISE](../verdicts/reproducibility_and_independent_conformance/ariadne-400895e.md) | [REVISE](../verdicts/reproducibility_and_independent_conformance/nabu-400895e.md) | [REVISE](../verdicts/reproducibility_and_independent_conformance/vulcan-400895e.md) |
| standards_sbom_and_provenance | [REVISE](../verdicts/standards_sbom_and_provenance/ariadne-400895e.md) | [REVISE](../verdicts/standards_sbom_and_provenance/nabu-400895e.md) | [REVISE](../verdicts/standards_sbom_and_provenance/vulcan-400895e.md) |
| decision_dossier | [PASS](../verdicts/decision_dossier/ariadne-400895e.md) | [REVISE](../verdicts/decision_dossier/nabu-400895e.md) | [PASS](../verdicts/decision_dossier/vulcan-400895e.md) |
| release_candidate_packaging | [REVISE](../verdicts/release_candidate_packaging/ariadne-400895e.md) | [REVISE](../verdicts/release_candidate_packaging/nabu-400895e.md) | [REVISE](../verdicts/release_candidate_packaging/vulcan-400895e.md) |

## Follow-up implementation

The exact review dispositions remain filed until the follow-up is independently accepted. Seven distinct items are repaired:

- Root wording packet uses sections 3, 7, 9, 10 and identifies packet items 1, 2, 7, 8, without a false item range.
- Reproducibility revision 9 states the actual provisional report/attestation commit, receipt, refresh, verify sequence.
- The reproducibility revision note now describes revisions 7, 8 and 9 accurately.
- The S20-710 work-package row and remaining-gates record state that root licensing is approved; the full candidate-history re-anchor decision remains distinct from the reachable-anchor mapping.
- Packaging work-package/status records and closeout name revision 5, review-pending status, fifteen members and the owned section-15 surface; its stale license blocker is removed.
- Real staging tests bind Git fixture enumeration to the shared owner and exercise extra/missing member refusals with code 72007.
- GA CLI structural-source failures now emit the owner’s code 76001 in write and check modes, covered by a regression test that failed before the repair. Dossier contract/checker/ADR/summary are revision 9.

Focused validation: 139 release tests and 90 review tests passed; three focused new tests passed. The staging tests are characterization/regression coverage, and the GA CLI test was observed red before the fix. Full new-candidate validation follows because contracts and tests changed.

Index: [qualification-400895e.json](qualification-400895e.json).
