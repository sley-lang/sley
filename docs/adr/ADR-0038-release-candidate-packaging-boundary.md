# ADR-0038: release candidate mechanics without a release

Status: proposed; the S20-720 contract is a draft at revision 6 with
Council review pending; mechanics implemented (2026-09-03, revised
2026-09-05) with a reproducible artifact, an unpacked demo, and
`release-check` still fail-closed

Note (2026-09-15): Revision 5 adds the owned artifact-content evidence surface, shared member enumeration, coded failures, and local verification prerequisites. The current contract and machine summary are authoritative for staging; the context below records the original boundary.

Date: 2026-09-03

## Context

The master goal ends at a clean, fully evidenced local Sley 2.0 candidate
and names the packaging criteria: a named artifact built from the final
commit, runnable with no source tree, free of secrets and local paths,
recorded with manifest, checksums, SBOM, licenses, and provenance, and
reproducible or precisely nondeterministic. The S20-430 endpoint now makes
the source-independence demo possible, while the root license, standards
SBOM, provenance, and succession thresholds remain gated.

The Council lanes were still unavailable at this draft (see ADR-0026); the
design is the integrator's and is submitted to Ariadne, Nabu, and Vulcan as
soon as a lane returns.

## Decision

1. **Mechanics now, gate later.** The candidate is built, packaged,
   unpacked, exercised, scanned, and rebuilt under a new smoke target;
   `release-check` and `v2` stay fail-closed until the GA gates exist.
2. **Deterministic archive.** Sorted members, zero timestamps and owners,
   normalized modes, and a mtime-free gzip, so reproducibility is a byte
   comparison.
3. **No local path.** The binary is built with the working tree remapped
   and every member is scanned for the tree path and any home path.
4. **The demo is the endpoint.** The canonical demo drives `bin/sley` over
   empty directories with a fixture emitted from the executable test
   genesis; it proves import, query, execute, report, branch, export, and
   clone-equivalent import without source, and names its candidate gap.
   (2026-09-05: the claim is narrowed to the verbs the demo covers —
   execute, branch, export, import of 20.12's eight — with create, modify,
   test, and merge residual; open question 1, covering create via
   `workspace.create` from a packaged trusted genesis, is answered yes.)
5. **Inventory reused.** The S20-710 pre-release inventory travels
   verbatim as the SBOM with declared licenses and the root-license
   blocker; nothing is invented.
6. **Codes.** Eight `PACKAGE_*` codes 72000 through 72007.
7. **Staging.** `scripts/check_release_candidate_packaging.py` binds the
   contract, ADR, work-package row, and summary section, keeps the GA
   gates fail-closed, and fails closed if the build script appears before
   the summary allows it.

## Consequences

- A release decision needs only the gated approvals; the artifact
  mechanics are already evidenced.
- The demo doubles as the source-independence proof for the master goal
  section 20.12 verbs it covers (execute, branch, export, import), with
  create, modify, test, and merge residual; open question 1 (cover create
  via `workspace.create` from a packaged trusted genesis) is answered yes
  and lands as a demo extension.
