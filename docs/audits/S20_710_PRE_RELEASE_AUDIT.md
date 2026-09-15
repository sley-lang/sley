# S20-710 Pre-Release Supply-Chain Audit

Status: **BLOCKED - operator-approved root license text required**

This is a bounded, offline pre-release audit. It is not the S20-710 acceptance
record, a legal opinion, a standards SBOM, release provenance, or permission to
publish. The audit is frozen through Git commit
`db1bc623d01e838d49c153feb0be05a7502b8794` (the pushed head at re-anchor);
a later release audit must use a new anchor and cover all subsequent
history. The anchor is the pushed head, so no committed history stands
outside either scan scope; the candidate scan additionally covers the
committed tree at generation time. Re-anchoring at the release candidate
remains a tracked precondition (`release_candidate_history_reanchored`),
not a hidden gap.

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
`docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md` (draft revision 5, ADR-0041):
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

## License decision executed (2026-09-14)

The operator approved: Apache License, Version 2.0 (SPDX `Apache-2.0`),
copyright notice `Copyright 2026 Greyforge Labs`. This section records the
execution; the pre-decision record above is preserved unchanged.

- `LICENSE` at the repository root is the official text fetched from
  `https://www.apache.org/licenses/LICENSE-2.0.txt`, installed byte-identical
  (11358 bytes, LF-only), sha256
  `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30`.
- `NOTICE` at the repository root identifies Sley 2.0, carries the approved
  ownership line exactly, points at `LICENSE`, and states that third-party
  dependency licenses are declared in the inventory rather than relicensed;
  sha256 `e7151ea0ee545a9edec91ecf963acefec4d6c2cfd92aa6080b1afe517d5a5dfa`.
- All 18 Cargo workspace crates resolve `Apache-2.0` through workspace
  inheritance (`cargo metadata --offline --no-deps`: 18 packages, one license
  value); `oracle/scb1/pyproject.toml` declares `Apache-2.0`. Together the
  19 first-party packages carry the approved declaration; registry
  dependencies keep their own declared expressions and are not relicensed.
- Mechanics, mapped to the five steps above: (1) `LICENSE` (+ `NOTICE`)
  landed; (2) `scripts/generate_supply_chain_evidence.py` re-reads the root
  files, the nineteen components carry `APPROVED_OPERATOR_APACHE_2_0_ROOT_LICENSE`,
  and `license_text_files` is `["LICENSE", "NOTICE"]`; (3) `make
  evidence-refresh` regenerates both SBOM documents with the approved
  dispositions and re-derives provenance, register, and dossier;
  (4) `root_license_text_approved` is `true` in the machine summary's
  `s20_710_pre_release_audit` section and in the `check_supply_chain_audit.py`
  and `check_local_completion_frontier.py` expectations, pinned to the exact
  digests above rather than an unconditional true; (5) `make quick` then
  `make release-candidate-smoke` from a clean tree, so the artifact carries
  the real license members and no pending-license staging.
- The installed text is enforced, not just recorded: the T52 inventory
  carries `root_license_sha256`/`notice_sha256`, the generator returns
  workspace dispositions to `BLOCKED` on any digest mismatch, and the checker
  pins the exact approved bytes.
- Still required after this execution: Argus license-and-secret re-review on
  the new anchor, SBOM/provenance approval, history re-anchor at the release
  candidate, and disposition of every finding before S20-710 can pass. No
  legal compatibility opinion is offered here.

## Attribution rewrite anchor mapping — 2026-09-15

The pre-rewrite anchor `724a899` maps to reachable commit `7804f66`.
Both have tree `b29d1681ba4508ae0b9dd600a131cd58a467c93d`. The mapping
repairs fresh-clone regeneration after the attribution rewrite; it does not
advance the audit boundary to the current release candidate. The history
scan is regenerated over the rewritten ancestry and its counts are recorded
in the machine summary. The old review artifacts remain historical evidence.
