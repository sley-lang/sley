## Summary

The recovered changes improve review-state derivation, artifact input coverage, and root-query contract checks, but two reproducible release-report defects remain. GA/dossier candidate selection bypasses the new shared selector, and the GA artifact-content criterion reads a repository credential scan that cannot establish the claimed artifact checks. This is one development review, not Council approval; known candidate-attestation staleness and derived-record drift are excluded.

## Issues

### Issue 1 -- Severity: bug

- File: scripts/build_ga_acceptance_report.py:412
- Description: GA still filters attestations only by dictionary shape and selects `attestations[0]`; `scripts/build_decision_dossier.py:155` also selects the first entry. This contradicts the newly revised reproducibility contract, which explicitly requires both consumers to use the shared admissibility/selection helpers. Reproduced entirely in memory using `build_reproducibility_report.build_report`: an `archive` host attests commit `d` repeated 40 times, while `primary` and `secondary` attest commit `b` repeated 40 times. The shared selector correctly chooses `b`, but with `release_decision={state: RELEASE_APPROVED, final_commit: b}`, GA reports the final-candidate criterion `GATED` and cites `d`. Dossier candidate facts likewise come from the archived host; malformed or tied selections also bypass the intended fail-closed rule.
- Suggestion: Import and use `admissible_attestations` and `select_attestation` in both builders. Derive candidate-dependent facts from the selected candidate and gate them when no unique admissible candidate exists. Add cross-consumer tests for a carried older host, tied commits, and inadmissible attestations.
- Status: open

### Issue 2 -- Severity: bug

- File: scripts/build_ga_acceptance_report.py:428
- Description: `scan_clean` uses only `evidence/security/T54/secret-scan.json` to evidence “artifact contains no secrets, local paths, caches, or debug files,” and the rendered evidence incorrectly calls it the S20-720 forbidden-content scan. The T54 record actually describes a repository/history high-confidence credential scan; it explicitly says compressed content is opaque and ignored local files are excluded. Artifact member/path checks belong to `build_release_candidate.py` and its S20-720 evidence (`forbidden_content_findings`). Reproduced in memory by clearing the reproducibility attestations and replacing the packaging summary with `{}` while retaining T54: this artifact criterion still reads `EVIDENCED` even though no candidate packaging evidence remains.
- Suggestion: Require tracked evidence of the S20-720 artifact-content/member checks bound to the selected candidate identity, and gate the criterion when that evidence is absent. Keep the T54 source-scan result separately attributed; do not describe it as the artifact scan. Cover absent and failed artifact evidence independently of a passing T54 scan.
- Status: open

### Issue 3 -- Severity: suggestion

- File: scripts/build_release_candidate.py:43
- Description: Several added comments narrate review history and implementation details rather than concisely explaining the invariant. Examples include the seven-line environment comment citing “Vulcan P4 at a809906,” the embed comment at line 56, and the derived-bound-input comment at `scripts/records_closure.py:52`. The expanded test documentation at `crates/sley-query/src/root_query.rs:1903` similarly describes each step already visible in the test. These comments increase maintenance surface and preserve obsolete review context in executable code.
- Suggestion: Reduce each comment to the enduring reason (for example, ambient compiler/profile overrides must not change the pinned release build). Keep reviewer names, revision history, and detailed evidence narratives in the existing audit/spec documents.
- Status: open
