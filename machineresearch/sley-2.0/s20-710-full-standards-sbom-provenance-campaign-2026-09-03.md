# S20-710 full standards SBOM and release provenance campaign (2026-09-03)

Package: the standards SBOM and release provenance half of S20-710 (full
pre-release audit), dependency S20-720 and S20-730, phase M6. Owner of record
Argus; executed by the integrator because every Council lane was unavailable
(ADR-0026). Nothing here signs, publishes, or completes the S20-710 audit:
`make release-check` and `make v2` stay fail-closed and the audit keeps its
root license blocker.

## Contract

- `docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md`, draft revision 1, with
  `docs/adr/ADR-0041-standards-sbom-and-unsigned-provenance.md`.
- Staged checker `scripts/check_standards_sbom_and_provenance.py` in
  `make quick`; summary section `standards_sbom_and_provenance`; work package
  row updated; error codes 74000 through 74007 reserved in
  `docs/spec/ERROR_CODES_V1.md`; the audit document records the draft
  documents and keeps its blocker.

## Mechanics

| Surface | File | What it does |
|---|---|---|
| SBOM | `scripts/build_standards_sbom.py` | derives CycloneDX 1.6 and SPDX 2.3 documents from the T52 inventory and the S20-720 candidate; deterministic serial number and namespace, fixed SPDX instant, purls, licenses, dependency graph |
| Provenance | `scripts/build_release_provenance.py` | derives an in-toto Statement v1 with a SLSA Provenance v1 predicate for the candidate, wrapped with a local `attestation` block recording `signed: false` and the open blockers |
| Tests | `bench/release/tests/test_standards_sbom.py` | 14 offline tests: format and version fields, derived serial, component completeness, multi-artifact handling, SPDX extracted proprietary reference, relationship counts, statement shape, subject agreement, resolved dependencies, unsigned attestation, no clock or host path, purity |

`make release-candidate-smoke` now rebuilds the reproducibility report, both
SBOM documents, and the provenance statement after a candidate build, so all
tracked release evidence names the commit it was built from.

## Result at this commit

- CycloneDX 1.6: 43 components, 44 dependency entries, serial number
  `urn:uuid:803a4af7-1e80-87b4-8018-fdc6cd93fba7` derived from the document
  digest, root component carrying the candidate artifact digest.
- SPDX 2.3: 44 packages (43 components plus the candidate root), 119
  relationships (one `DESCRIBES` plus 118 `DEPENDS_ON`), namespace
  `urn:sley2:spdx:<inventory digest>`, `created` fixed at the Unix epoch,
  `LicenseRef-Proprietary` defined as extracted licensing info naming the
  pending operator decision.
- Nineteen components carry a blocked license disposition (the fifteen
  workspace crates, the oracle package, and their siblings declaring
  `LicenseRef-Proprietary`), so the missing root license stays visible in both
  formats instead of being hidden by the format conversion.
- Provenance: subject `sley-2.0.0-linux-x86_64.tar.gz` with the candidate
  digest, build type `urn:sley2:buildtype:release-candidate/v1`, builder
  `urn:sley2:builder:local-primary`, resolved dependencies covering the git
  commit, both locks, the T52 inventory, and both SBOM documents, byproducts
  covering the S20-730 reports, `signed: false`, `transparency_log: null`,
  `publication_authorized: false`.
- No document carries a timestamp (beyond the fixed SPDX epoch instant), host
  name, user name, or absolute path.

## Open questions for the Council

1. Ariadne: is the fixed SPDX `created` instant the right determinism trade,
   or should the document carry the candidate commit's author date instead?
2. Nabu: should the provenance statement move to its own evidence contract
   version when signing becomes possible, or is the wrapper (`statement` plus
   local `attestation`) stable enough to sign in place?
3. Vulcan: does emitting a standards SBOM whose workspace components carry a
   blocked license disposition create a misuse risk if the file is read
   outside this repository, and should the documents carry a stronger
   in-band "not for distribution" marker?

## In-flight repair (2026-09-03)

The first `make release-candidate-smoke` after this package failed
(`check_release_candidate_packaging.py`, six test errors). Cause: the release
checkers share one `bench/release` test suite, and the recipe ran the S20-720
checker between the candidate build and the SBOM rebuild, so the provenance
tests derived a statement against a CycloneDX document that still named the
previous candidate (`PROVENANCE_SUBJECT_MISMATCH`). A second run then failed
in `build_reproducibility_report.py` with `REPRO_ATTESTATION_INVALID` ("the
attested tree must be clean"), correctly refusing to attest a build made from
a dirty tree.

Both were repaired at `abb1dae` rather than worked around:

1. the smoke now runs every evidence builder before every checker;
2. `--check` of both S20-710 builders reports
   `LOCAL_BUILD_AHEAD_OF_TRACKED_DOCUMENTS` with result `PASS` instead of
   failing, because `evidence/runtime/` is untracked and Tier 1 must stay
   hermetic over tracked files;
3. the shared test suite derives the provenance against the derived CycloneDX
   document, and a new test covers the subject-mismatch refusal directly;
4. contract revision 2 records both rules (section 5).

The clean rerun passed end to end (exit 0, seven `PASS` results, two builds
`REPRODUCIBLE`), and the evidence it rebuilt is committed at `3861b9f`.

## Validation

Landed at `6cd4380` with the repair at `abb1dae` and the rebuilt evidence at
`3861b9f`. Tier 1 `make quick` passed at each commit. Tier 2 ran on
2026-09-03:

| Gate | Result | Wall time | Evidence |
|---|---|---:|---|
| `make core` | exit 0 | 15 s | 995 tests passed, 0 failed |
| `make conformance` | exit 0 | 8 s | 19 oracle results PASS |
| `make adversarial` | exit 0 | 8 s | 597 tests passed, 0 failed |
| `make fuzz-smoke` | exit 0 | under 1 s | 5 bounded smoke tests passed |
| `make release-candidate-smoke` | exit 0 (after the repair above) | 25 s | two builds REPRODUCIBLE, artifact `c6b61983...`, all four evidence documents rebuilt |

`make v1` was not run: this is a subsystem handoff, not a release boundary.
