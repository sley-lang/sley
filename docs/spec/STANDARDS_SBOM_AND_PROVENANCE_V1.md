# Standards SBOM and Release Provenance v1

Status: S20-710 full-audit contract draft, revision 6 (2026-09-15); Council
review pending (Ariadne contract review, Nabu architecture review, Vulcan
surface review, plus Council review of the revision 5 records-closure
model before any standards review row closes on a closure HEAD). Revision 2 records the clarifications found while wiring the
release smoke (section 5). Revision 3 closes the five S20-710 P0s: license
normalization and validation (section 2), a candidate-bound SPDX namespace
(section 3), and fail-closed `--check` semantics over missing evidence
(section 5). Revision 4 binds the subject to the tracked reproducibility
attestation instead of the per-checkout candidate evidence (sections 3 and
4), records the candidate invocation instead of inferring it (section 4),
symmetrizes the ahead states of both builders (section 5), and specifies the
attestation cross-checks (section 7). Revision 5 adds the records-closure
model (section 5): the attested source candidate commit is distinguished
from a later records-closure HEAD, and a records-only advancement derives
the identical candidate-bound documents with nothing re-minted. Erratum
(2026-09-24, no contract-revision change): the non-normative user guides
listed in section 5 are records-closure eligible, so documentation can be
updated without re-minting a candidate. The mechanics are `scripts/build_standards_sbom.py` and
`scripts/build_release_provenance.py`; implementation state is tracked in the
machine summary.

## Boundary

This contract freezes how the Sley 2 candidate's dependency inventory becomes
standards-format SBOM documents and how the local build becomes an unsigned
provenance statement (master goal sections 16.7, 26.8, dossier 21; the
S20-710 full-audit requirement named in `docs/audits/S20_710_PRE_RELEASE_AUDIT.md`).
It composes the S20-710 T52 inventory and the S20-720 candidate evidence and
derives; it never queries a registry, contacts a network, signs anything, or
authorizes publication.

It does not complete the S20-710 audit. That still requires the
operator-approved root license text, the Argus and Vulcan dispositions, and a
history re-anchor at the release candidate, so the machine summary keeps
`s20_710_pre_release_audit.standards_sbom` and `release_provenance` false and
these documents are draft, local, and unapproved.

## 1. Inputs

- `evidence/security/T52/pre-release-inventory.json` (contract
  `s20-710-pre-release-inventory-v1`): packages with purl `bom_ref`, version,
  ecosystem, source, declared license expression and disposition, plus the
  dependency relationships. A missing file is `SBOM_INVENTORY_MISSING`; a file
  whose contract tag, package list, or relationship list is absent or
  malformed is `SBOM_INVENTORY_INVALID`.
- `evidence/runtime/s20-720-release-candidate/evidence.json`: the artifact
  name, digest, size, member count, manifest digest, commit, toolchain,
  recorded invocation, and working-tree cleanliness of the local
  candidate. A missing file is `PROVENANCE_EVIDENCE_MISSING`; a
  record that is not a reproducible `PASS` is `PROVENANCE_EVIDENCE_INVALID`.
- `Cargo.lock` and `oracle/scb1/uv.lock` digests, hashed from the tracked
  lockfiles by the provenance builder (the T52 inventory records the same
  digests, and the inventory itself is a resolved dependency).
- `evidence/release/reproducibility-report.json` (S20-730): the
  subject-authority input for the attestation binding (also recorded as
  a provenance byproduct).

Every component must carry a purl, a name, a version, an ecosystem, and a
license expression; anything else is `SBOM_COMPONENT_INCOMPLETE`, as is a
license expression that normalizes to nothing usable under the section 2
grammar.

## 2. CycloneDX 1.6

`evidence/release/sbom/cyclonedx-1.6.json` is a CycloneDX 1.6 JSON BOM:

- `$schema` `http://cyclonedx.org/schema/bom-1.6.schema.json`, `bomFormat`
  `CycloneDX`, `specVersion` `1.6`, `version` 1;
- `serialNumber` is `urn:uuid:` followed by a UUID derived from the SHA-256 of
  the canonical BOM without that field, with the version nibble set to 8 and
  the variant nibble to 8, so the document is deterministic and carries no
  random state;
- `metadata.component` is the candidate: `type` `application`, the artifact
  name, version `2.0.0`, the artifact SHA-256, and `licenses` as the
  single expression of the operator-approved root license;
- `metadata.tools.components` names `sley2-standards-sbom` version 1;
- `metadata.properties` records `sley2:commit`, `sley2:inventory-digest`,
  `sley2:license-disposition-blocked`, `sley2:manifest-digest`,
  `sley2:signed`, and `sley2:publication-authorized` (both always `false`;
  nothing is signed and no publication is authorized);
- `components` is one entry per inventory package, ascending by purl, with
  `bom-ref` the purl, `type` `library`, `name`, `version`, `purl`,
  `licenses` as a single `expression` (a declared expression is emitted
  verbatim after the normalization below), `properties` `sley2:ecosystem`,
  `sley2:license-disposition`, and `sley2:locked-source` (ascending by name),
  `externalReferences` for the locked source of a registry package only (a
  workspace package has none), and a
  `hashes` entry only when the lock records exactly one artifact digest;
- a declared license expression is normalized before emission: Cargo
  documents `/` as an OR-equivalent dual-license separator, but `/` is not
  SPDX expression syntax, so a `/`-joined declaration such as
  `MIT/Apache-2.0` becomes the `OR` chain with that exact meaning
  (`MIT OR Apache-2.0`); every other declaration passes through unchanged;
- every emitted expression, normalized or not, must parse under the SPDX
  license-expression subset the builder checks (uppercase `AND`, `OR`, and
  `WITH`, where `WITH` joins a license id to an exception id, parentheses,
  and license or `LicenseRef-` ids); an expression that does not parse is
  not a usable license fact and fails as `SBOM_COMPONENT_INCOMPLETE`;
- a component whose lock records several platform artifacts (Python wheels)
  carries no `hashes` and instead the property
  `sley2:locked-artifact-digests` with their count, because no single digest
  identifies the component; the digests stay in the T52 inventory, which the
  BOM references by digest;
- `dependencies` is one leading entry for the candidate root whose
  `dependsOn` lists the ascending workspace purls, then one entry per
  component, ascending, with `dependsOn` the ascending purls of its
  inventory relationships;
- no timestamp, host name, user name, or absolute path appears anywhere.

## 3. SPDX 2.3

`evidence/release/sbom/spdx-2.3.json` is an SPDX 2.3 JSON document:

- `spdxVersion` `SPDX-2.3`, `dataLicense` `CC0-1.0`, `SPDXID`
  `SPDXRef-DOCUMENT`;
- `documentNamespace` is
  `urn:sley2:spdx:<inventory digest>:<candidate artifact digest>`, an
  absolute URI that names no host; the inventory portion keeps the document
  traceable to the lock set it was derived from, and the artifact portion
  keeps it unique per candidate, because two candidates that share a
  dependency inventory are different documents and SPDX requires a unique
  namespace per document version;
- `creationInfo.created` is `1970-01-01T00:00:00Z`, so the document is
  deterministic; `creators` is `["Tool: sley2-standards-sbom-1"]` and the
  comment names the local, unapproved status;
- `packages` mirrors section 2, one per inventory package plus the candidate
  root, with `SPDXID` `SPDXRef-<ecosystem>-<name>-<version>` (every character
  outside `[A-Za-z0-9.-]` replaced by `-`), `versionInfo`,
  `downloadLocation` (the locked source, or `NOASSERTION` for a workspace
  package), `filesAnalyzed` false, `licenseDeclared` the declared expression,
  `licenseConcluded` `NOASSERTION` (no legal opinion), `copyrightText`
  `NOASSERTION`, `checksums` under the section 2 single-digest rule, and an
  `externalRefs` PACKAGE-MANAGER purl entry;
- no `hasExtractedLicensingInfos` section: SPDX 2.3 clause 10.1 reserves
  extracted licensing information for licenses absent from the SPDX license
  list, identified by `LicenseRef-` ids, and the declared workspace
  expression `Apache-2.0` is a listed identifier referenced by id alone;
- `relationships` carries `DESCRIBES` from the document to the candidate root
  and one `DEPENDS_ON` per inventory relationship, ascending.

## 4. Provenance

`evidence/release/provenance.json` wraps an in-toto statement so the
statement can later be signed verbatim without rewriting local facts:

```text
file = {
  "contract": "sley2.release-provenance.v1",
  "statement": in-toto Statement v1 (below),
  "attestation": { "signed": false, "signature_algorithm": null,
                   "transparency_log": null, "publication_authorized": false,
                   "blockers": [string, ...] },
  "statement_digest": SHA-256 of the canonical statement
}
```

The statement is:

- `_type` `https://in-toto.io/Statement/v1`;
- `subject`: one entry, the artifact name and its SHA-256 digest;
- `predicateType` `https://slsa.dev/provenance/v1`;
- `predicate.buildDefinition.buildType`
  `urn:sley2:buildtype:release-candidate/v1`;
- `predicate.buildDefinition.externalParameters`: the commit, the artifact
  name, the make target (derived from the recorded invocation:
  `release-candidate-build` exactly for the Makefile build
  rendering, `build_release_candidate.py direct` otherwise), the recorded candidate
  invocation copied verbatim from the candidate evidence (ADR-0041
  principle 1: derive, never restate; a candidate without a recorded
  invocation predates invocation recording and refuses with
  `PROVENANCE_EVIDENCE_INVALID`), and `working_tree_clean`;
- `predicate.buildDefinition.internalParameters`: the cargo and rustc
  versions, the `release` profile, `locked` true, the path remaps of the
  S20-720 build (which are themselves path-free strings), the artifact size
  in bytes, and the member count;
- `predicate.buildDefinition.resolvedDependencies`: the git commit (a `sha1`
  digest), `Cargo.lock`, `oracle/scb1/uv.lock`, the T52 inventory, and both
  SBOM documents, each with its SHA-256;
- `predicate.runDetails.builder.id` `urn:sley2:builder:local-primary`, naming
  a host label rather than a host;
- `predicate.runDetails.metadata.invocationId`: the candidate manifest digest;
- `predicate.runDetails.byproducts`: the reproducibility report and the
  independent conformance report with their SHA-256 digests (the manifest
  digest rides as `invocationId` above, not as a third byproduct);
- no timestamp anywhere: `startedOn` and `finishedOn` are omitted because a
  wall clock would break determinism and leak nothing useful locally.

The subject digest must equal the digest of a clean `REPRODUCIBLE`
tracked reproducibility attestation naming the same commit, and the CycloneDX
root component digest; any disagreement is `PROVENANCE_SUBJECT_MISMATCH`.
The tracked attestation is the subject authority, not the per-checkout
candidate evidence: `build_statement()` refuses a candidate no clean
`REPRODUCIBLE` attestation names, and refuses a candidate whose commit is not
the tree's `HEAD` with `PROVENANCE_EVIDENCE_INVALID` (the inputs are read
from the live tree, so a candidate from another commit would misbind the
statement), except for a provable records-closure HEAD under the
records-closure model below, whose attestation-bound inputs are unchanged
so the derived statement is byte-identical. The reproducibility report is therefore a subject-authority
input: a missing report is `PROVENANCE_EVIDENCE_MISSING`, an unreadable or
attestation-less report is `PROVENANCE_EVIDENCE_INVALID`. A statement minted in a
clean linked worktree therefore verifies on any checkout of the same commit.

## 5. Determinism and check semantics

Both builders write canonical JSON (sorted keys, two-space indentation,
trailing newline) and are pure functions of their inputs. `--check`
recomputes and fails with `SBOM_DOCUMENT_DRIFT` or
`PROVENANCE_DOCUMENT_DRIFT` when a tracked document differs. Repeated runs on
unchanged inputs rewrite byte-identical files.

The candidate evidence record lives under untracked `evidence/runtime/`, so a
local candidate build legitimately leaves the tracked documents describing the
previous candidate. `--check` detects exactly that state (the evidence
loads, and its commit and artifact digest disagree with the tracked
documents), first validates the tracked documents themselves, then reports
`CANDIDATE_EVIDENCE_MISMATCH_TRACKED_DOCUMENTS` with result
`MISMATCH_TRACKED_VALIDATED` (or `MISMATCH_TRACKED_INVALID` when the
tracked documents fail their own validation), and names
`make release-candidate-smoke` as the reconciling command. The state name
is direction-neutral: the evidence may be newer, older, or simply different.
Both builders validate the tracked pair in the mismatch state: document
shape and determinism pins plus the attestation binding of the SPDX
namespace and the provenance subject. "Names" binds the same 4-tuple on
both builders: commit, artifact digest, manifest digest, and size, with
the SBOM side additionally requiring a `PASS` record and the provenance
side additionally requiring `PASS` and `REPRODUCIBLE` at candidate load.
Write mode never tolerates the skew: both builders refuse a candidate
that is not `HEAD` (other than a provable records-closure HEAD, which
derives byte-identical documents with nothing re-minted) or that no clean
`REPRODUCIBLE` attestation names
(`SBOM_INVENTORY_INVALID` / `PROVENANCE_EVIDENCE_INVALID` /
`PROVENANCE_SUBJECT_MISMATCH`), so the documents always derive from the
attested candidate rather than merely from the current evidence.

Anything else fails closed. Missing or unreadable candidate evidence is
missing input, not a build running ahead: the SBOM `--check` fails with
`SBOM_INVENTORY_MISSING` or `SBOM_INVENTORY_INVALID`, the provenance
`--check` fails with `PROVENANCE_EVIDENCE_MISSING` or
`PROVENANCE_EVIDENCE_INVALID`, and each names the command that produces the
evidence. Tier 1 therefore requires candidate evidence and never passes the
710 drift gates without deriving from it; on a checkout where no candidate
has been built, the 710 check fails with the input code until
`make release-candidate-smoke` runs. A provenance derived against an
SBOM that still names another candidate is `PROVENANCE_SUBJECT_MISMATCH`
(the write-mode gate above makes this unreachable except by hand-editing
the tracked documents).

The shared `bench/release` test suite derives the provenance against the
*derived* CycloneDX document rather than the tracked one, for the same
reason.

### Records-closure model

The attested source candidate commit and a later records-closure HEAD are
different things, and the contract treats them differently. A HEAD past the
candidate commit admits derivation only when the source-to-HEAD diff is
provably records-only, decided by `scripts/records_closure.py`:

- every changed tracked path is under `evidence/` or `machineresearch/`,
  or is one of the non-normative user guides (`README.md`,
  `docs/README.md`, `docs/QUICKSTART.md`, `docs/CONCEPTS.md`, and
  `docs/examples/`), or a GitHub community-health file
  (`.github/FUNDING.yml`, `.github/ISSUE_TEMPLATE/`,
  `.github/PULL_REQUEST_TEMPLATE.md`, `CODE_OF_CONDUCT.md`, `SUPPORT.md`;
  none is an artifact input, a spec, or a contract, and
  `.github/workflows/` stays bound);
  any change to `crates/`, `scripts/`, specs/contracts (including every
  other file under `docs/`), lockfiles, build inputs, or any other
  attestation-bound path makes the HEAD ineligible;
- none of the bound inputs changed. The bound input is the T52 inventory
  the SPDX namespace binds. The emitted documents (the SBOM pair and the
  provenance statement) and the reproducibility report are not bound
  inputs: they are validated by the second layer, byte-identical
  re-derivation over the closure HEAD plus the attestation 4-tuple gate
  (commit, artifact digest, manifest digest, size) that the candidate must
  still satisfy with a clean `REPRODUCIBLE` attestation. A reader needs
  both layers: the first says what may not move, the second says how the
  documents that may be re-derived are checked.

The SBOM and provenance documents stay bound to the original attested
source candidate, the closure HEAD is recorded separately (the checker's
`records_closure` output block; never inside the documents or the machine
summary, whose candidate pins name only the attested commit), and no artifact is
rebuilt or re-minted solely for a permitted records-only advancement.
This is not a general commit-skew tolerance: any attestation-bound change,
any bound-artifact change, or any unverifiable diff refuses closed with
the same codes (`SBOM_INVENTORY_INVALID` / `PROVENANCE_EVIDENCE_INVALID`)
carrying a `records-closure-…` reason, and write mode still names
`make release-candidate-smoke` as the reconciling command. Closing any
standards review row on a closure HEAD additionally requires the fresh
Council review of this model named in the Status line.

## 6. Codes

S20-710 full reserves 74000 through 74007: `SBOM_INVENTORY_MISSING` (74000),
`SBOM_INVENTORY_INVALID` (74001), `SBOM_COMPONENT_INCOMPLETE` (74002),
`SBOM_DOCUMENT_DRIFT` (74003), `PROVENANCE_EVIDENCE_MISSING` (74004),
`PROVENANCE_EVIDENCE_INVALID` (74005), `PROVENANCE_SUBJECT_MISMATCH` (74006),
`PROVENANCE_DOCUMENT_DRIFT` (74007). Each script exits 1 and prints one JSON
object naming the code on failure.

## 7. Staging

`scripts/check_standards_sbom_and_provenance.py` runs under `make quick`.
Statuses: `S20_710_FULL_CONTRACT_DRAFT_REVIEW_PENDING`,
`S20_710_FULL_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS`,
`S20_710_FULL_SBOM_AND_PROVENANCE_IMPLEMENTED_REVIEW_PENDING`, and
`S20_710_FULL_COMPLETE`, the last requiring the three Council reviews to read
`PASS` and the S20-710 audit blockers to be closed. In every implementation
status the checker verifies the three documents exist with their contract
tags and versions, that they carry no timestamp or host path, that both
builders report no drift, that the SPDX namespace binds the inventory digest
and a tracked attested artifact digest (`spdx:namespace-not-candidate-bound`
otherwise, `spdx:namespace-unbound` when the inventory or the
reproducibility report is missing), that the provenance subject names a
clean `REPRODUCIBLE` attestation with the same commit
(`provenance:subject-attestation-mismatch` otherwise,
`provenance:attestation-unbound` when the report is missing) and a clean
tree (`provenance:dirty-candidate` otherwise), that the unit tests pass, and
that `release-check` and `v2` stay `NOT_IMPLEMENTED`.

`make release-candidate-smoke` rebuilds the reproducibility report, both SBOM
documents, and the provenance statement after a candidate build, so the
tracked evidence names the commit it was built from. Every builder of that
recipe runs before every checker, because the release checkers share one
`bench/release` test suite whose tests read the evidence a candidate build
has just replaced; a checker placed between builders sees a stale artifact
digest and fails with `PROVENANCE_SUBJECT_MISMATCH`.

## 8. Explicit exclusions

- No signature, key, keyless flow, transparency log, or attestation service.
- No registry, network, or vulnerability lookup; no VEX document.
- No legal opinion: `licenseConcluded` stays `NOASSERTION` and the
  approved workspace license rides as a declared `Apache-2.0` expression
  with the approved disposition, pending Argus and Vulcan disposition of
  the result at the release candidate.
- No completion of the S20-710 audit, no GA claim, no release decision, no
  publication; `release-check` and `v2` stay fail-closed.
- No second-host provenance: the builder id names the primary host label, and
  a multi-host claim needs S20-730 attestations.

## 9. Clarifications

Revision 1 carries none.

Revision 3 closes the five S20-710 P0s without touching the round verdicts,
which stand as `FAIL` history:

- license expressions (Ariadne): the section 2 normalization and grammar
  rule; `MIT/Apache-2.0` was the one inventory declaration the old verbatim
  emission carried into both documents unparseable.
- `documentNamespace` (Ariadne): the section 3 candidate binding; the old
  inventory-digest-only namespace gave two candidates sharing a lock set the
  same document identity.
- `--check` short-circuit on missing evidence (Nabu, Vulcan): the section 5
  rewrite; `local_build_ahead()` returned true when the evidence failed to
  load, so both builders passed open on any checkout without a candidate
  build and codes 74003 and 74007 were unreachable in check mode.
- Tier 1 hermeticity (Nabu): the section 5 admission that the 710 drift
  gates derive from untracked candidate evidence and fail with the input
  codes where none exists; the old claim that Tier 1 stayed hermetic over
  tracked files was false, since all fifteen release tests derive from that
  evidence.

## 10. Revision 6 (2026-09-15)

This revision records the normative SPDX clause 10.1 and frozen-shape edits
made at `40dbbd0` (previously carrying the revision-5 label), and the Council
repairs following `e050fe7`. Existing section 5 records-closure rules remain.

The section 4 make-target label means build-equivalent invocation, not proof
that the verify target ran. The separate `release-candidate-build` recipe
owns the 900-second, require-clean, no-keep rendering. All other renderings
remain `build_release_candidate.py direct`.

Provenance `attestation.blockers` retains the three held decisions:
`signing_key_and_transparency_log_unauthorized`,
`final_argus_and_vulcan_dispositions`, and `council_reviews`.
It appends `second_host_attestation_operator_lane` exactly when fewer than
two distinct admissible host labels bind the candidate's full four-field
identity. Historical hosts for another candidate do not discharge it.
The summary's `blockers` mirrors this emitted list; its
`license_disposition_blocked_components` mirrors CycloneDX metadata
`sley2:license-disposition-blocked`. Counter sync writes both and the
section checker rejects drift. These mirrors grant no signing or review
approval.

Hermetic namespace and provenance-subject checks bind the selected current
candidate, refusing a tie or absence. Mismatch-state validators may verify
historical documents only against S20-730's complete admissibility predicate;
they never substitute a weaker locally copied predicate.

S20-720 section 15 owns the artifact-content report, its shape, codes and
archive/member checks. The dossier is its consumer. Tier 1 `make quick`
and candidate verification require the matching local archive and build
record in addition to tracked release evidence. Those prerequisites are
not supplied by a repository-only T54 scan.
