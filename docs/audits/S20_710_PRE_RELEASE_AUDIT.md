# S20-710 Pre-Release Supply-Chain Audit

Status: **BLOCKED - operator-approved root license text required**

This is a bounded, offline pre-release audit. It is not the S20-710 acceptance
record, a legal opinion, a standards SBOM, release provenance, or permission to
publish. The audit is frozen through Git commit
`51863f7b93271bd7a73f9b7b3b02eeca93447d9a`; a later release audit must use a
new anchor and cover all subsequent history. The anchor is 678 commits
behind the current scope, so superseded blobs from those commits (and the
~4445 file-versions between anchor and scope) are outside both scan scopes;
re-anchoring is a tracked precondition (`release_candidate_history_reanchored:
false`), not a hidden gap.

## Local results

- Cargo is locked and inspected offline: 18 workspace crates and 30 registry
  crates, with registry sources, lock checksums, and dependency relationships.
- The SCB1 Python oracle is locked: an offline `uv lock --check` proves the
  lock is fresh for the project metadata, and the inventory covers one local
  package and two PyPI packages with registry sources and artifact hashes.
- All 19 local packages declare `LicenseRef-Proprietary`. No root `LICENSE`,
  `COPYING`, or `NOTICE` file exists, so their license disposition is blocked.
- Cargo registry license expressions come from offline Cargo metadata. The two
  Python dependency expressions are curated local pre-release dispositions,
  not lockfile declarations. Neither category is a legal compatibility
  opinion.
- The bounded high-confidence scan found no matching secret pattern in the
  candidate file set or reachable Git blobs through the audit anchor. It does
  not scan ignored local files, reflogs, remotes, provider stores, or external
  secret managers, and it does not perform entropy or credential validation.
- Common secret-bearing files are ignored at the repository boundary.

Machine-readable evidence:

- `evidence/security/T52/pre-release-inventory.json`
- `evidence/security/T54/secret-scan.json`

`python3 scripts/check_supply_chain_audit.py` validates deterministic
regeneration, locked sources and hashes, package/relationship coverage, the
absence of emitted secret values and host paths, and the exact remaining
blocker. Its successful outcome is deliberately `DEFERRED`, not `PASS`.

## Required operator decision

Provide or approve the exact root proprietary license text, including the
copyright holder, year/range, grant or restriction terms, and any required
notice language. The implementation must not invent these legal facts.

After that decision, Argus must re-run license and secret review, approve the
release-standard SBOM/provenance artifacts, re-anchor history at the release
candidate, and disposition every finding before S20-710 can pass.

### What happens mechanically after the text lands

The decision is one file plus one command; nothing else needs hand editing.

1. Write the approved text to the repository root as `LICENSE` (and `NOTICE`
   if the terms require a separate notice). The packaged
   `LICENSE-PENDING.txt` is replaced by the real text on the next candidate
   build.
2. Run `python3 scripts/generate_supply_chain_evidence.py`. The T52 inventory
   re-reads the root license files, so the nineteen components whose
   disposition is `BLOCKED_MISSING_APPROVED_PROPRIETARY_LICENSE_TEXT` move to
   their approved disposition and `license_text_files` stops being empty.
3. Run `make evidence-refresh`. Both SBOM documents regenerate with the new
   dispositions, the provenance re-derives over them, and the register and
   dossier follow.
4. Flip `root_license_text_approved` to `true` in the machine summary's
   `s20_710_pre_release_audit` section and update the expectation in
   `scripts/check_supply_chain_audit.py`, which pins it.
5. Run `make quick`, then `make release-candidate-smoke` from a clean tree so
   the artifact carries the real license, and commit the regenerated evidence.

Steps 2 through 5 are mechanical and take about a minute; step 1 and the
approval in step 4 are the operator's.

## Draft standards documents (2026-09-03)

The mechanics of the standards formats now exist under
`docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md` (draft revision 3, ADR-0041):
`scripts/build_standards_sbom.py` derives a draft standards SBOM in both
CycloneDX 1.6 (`evidence/release/sbom/cyclonedx-1.6.json`) and SPDX 2.3
(`evidence/release/sbom/spdx-2.3.json`) from this inventory, and
`scripts/build_release_provenance.py` derives an unsigned in-toto statement
(`evidence/release/provenance.json`) for the S20-720 candidate. They are
deterministic, carry no host path or wall clock, assert no license conclusion,
and are signed by nobody. They do not lift this audit's blocker: the machine
summary keeps `standards_sbom` and `release_provenance` false until the
operator approves the root license text and Argus and Vulcan disposition the
result at the release candidate.
