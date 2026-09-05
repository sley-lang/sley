# S20-710 standards SBOM and provenance closeout

The three Council lanes landed `FAIL` on the S20-710 full-audit package
with five P0s (Ariadne 2, Nabu 2, Vulcan 1). The round verdicts stand as
`FAIL` history; the contract is revised to revision 3 and the five defects
are fixed and pinned below. No disposition was edited and no review was
re-run.

## The five P0s

- License expressions emitted verbatim (Ariadne). The T52 inventory
  declares `MIT/Apache-2.0` for `unicode-normalization@0.1.24`, and both
  builders carried it into the CycloneDX `expression` and the SPDX
  `licenseDeclared` unchanged. `/` is not SPDX expression syntax, so both
  tracked documents failed strict validation, and the contract had no
  normalization or validation rule. Cargo documents `/` as an
  OR-equivalent dual-license separator, so contract section 2 now
  normalizes a `/`-joined declaration to the `OR` chain with that exact
  meaning (`MIT OR Apache-2.0`) and every emitted expression, normalized
  or not, must parse under the checked SPDX subset (uppercase `AND`,
  `OR`, `WITH`, parentheses, license and `LicenseRef-` ids); anything
  else is `SBOM_COMPONENT_INCOMPLETE`. All 43 current inventory
  declarations normalize to valid expressions.
- SPDX `documentNamespace` from the inventory digest alone (Ariadne). Two
  candidates sharing a lock set received the same document identity, and
  SPDX requires a unique namespace per document version. Contract section
  3 now binds both: the namespace carries the inventory digest for
  traceability and the candidate artifact digest for uniqueness.
- `--check` fail-opens when candidate evidence is absent (Nabu). The SBOM
  `local_build_ahead()` returned true when the evidence failed to load,
  so on any checkout without a candidate build the builder exited 0 with
  `LOCAL_BUILD_AHEAD_OF_TRACKED_DOCUMENTS`, reproduced live, and code
  74003 was unreachable in check mode.
- Contract section 5 hermeticity claim false (Nabu). The section claimed
  Tier 1 stayed hermetic over tracked files, but all fifteen release
  tests derive from untracked candidate evidence, and the 710 checker
  failed on a clean tree with `release-tests:fail`, reproduced live.
- `--check` fail-open in both builders (Vulcan). The provenance
  `local_build_ahead()` had the same swallowing shape, so codes
  74000 through 74007 were unreachable in check mode and the drift gate
  was a permanent no-op without a candidate build, reproduced live with
  both builders exiting 0 `PASS` on missing evidence. The cited ADR-0041
  consequence (an unreviewed change fails `make quick`) is now true: the
  short-circuit fires only when the evidence loads and disagrees, and
  missing or unreadable evidence fails with the input code that names
  the producing command.

## What changed

- `scripts/build_standards_sbom.py`: `normalize_license()`,
  `valid_spdx_expression()`, normalization plus grammar validation in
  `component_facts()`; the namespace carries the artifact digest;
  `local_build_ahead()` returns false on load failure.
- `scripts/build_release_provenance.py`: `local_build_ahead()` returns
  false on load failure.
- `docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md`: revision 3 (sections
  2, 3, 5, 6, 9).
- `docs/spec/ERROR_CODES_V1.md`: the 74002 reservation covers unusable
  license expressions.
- `docs/adr/ADR-0041-standards-sbom-and-unsigned-provenance.md`: status
  carries revision 3 and the fail-closed check semantics.
- `scripts/check_standards_sbom_and_provenance.py`: source markers for
  the new functions; pins that the namespace binds the tracked
  inventory digest and the evidence artifact digest, that every emitted
  license expression parses, and that the document counts agree with
  the recorded component and relationship counts.
- `bench/release/tests/test_standards_sbom.py`: fourteen new hermetic
  tests (normalization, grammar accept/reject tables, all 43 inventory
  declarations usable, namespace binding and determinism, and the
  fail-closed ahead semantics of both builders by monkeypatched loader).
  The fifteen derivation tests still run against the real evidence.

## What was deliberately not changed

- The T52 inventory still records `MIT/Apache-2.0` verbatim: it states
  declared facts, and normalization belongs to the standards-format
  layer, which the contract now says.
- No new error code: the unusable-expression failure is
  `SBOM_COMPONENT_INCOMPLETE` inside the frozen 74000-74007 reservation.
- The round verdicts stand: the machine summary keeps the three `FAIL`
  strings and records `p0_open_count` 0 with five `p0_closed`.

## Validation

Fourteen new tests plus the fifteen existing ones pass (29 total).
Tier 1 (`make quick`, `make lint`) and Tier 2 (`make core`,
`make conformance`, `make adversarial`, `make fuzz-smoke`) pass, and the
clean `release-candidate-smoke` re-attestation rebuilds both SBOM
documents and the provenance statement in order after the candidate,
leaving the drift, namespace, license, count, subject, and staleness
gates green. The full `make v1` gate was skipped because this is a
subsystem handoff, not a release boundary. `make v2` and
`make release-check` remain intentionally fail-closed.

Smoke record: pending at this commit; the evidence commit records the
exact documents and gate states.
