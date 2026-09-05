# ADR-0041: standards SBOM and unsigned provenance derived from local evidence

Status: proposed; the S20-710 full-audit contract is a draft at revision 3
(2026-09-05) with Council review pending; mechanics implemented with
deterministic CycloneDX 1.6 and SPDX 2.3 documents, an unsigned in-toto
statement, and the S20-710 audit still blocked. Revision 3 normalizes
Cargo-style `/` license separators to `OR` chains, validates every emitted
expression, binds the SPDX namespace to the candidate artifact as well as the
inventory, and fails `--check` closed where candidate evidence is missing
instead of passing open.

Date: 2026-09-03

## Context

The packaging criteria require an SBOM, a license inventory, and provenance
alongside the artifact digest and the reproducibility result. S20-710 already
produces a locked, offline dependency inventory with purls, versions, source
URLs, digests, and declared license expressions, and S20-720 already produces
a reproducible candidate with a manifest digest and toolchain record. What was
missing was the standards-format expression of that evidence, which the audit
document names as a remaining blocker together with the root license decision.

Signing is not available: no key material, no operator authorization for a
transparency log, and no publication grant. Wall-clock timestamps would break
the determinism the rest of the evidence enjoys.

The Council lanes were still unavailable at this draft (see ADR-0026); the
design is the integrator's and is submitted to Ariadne, Nabu, and Vulcan as
soon as a lane returns.

## Decision

1. **Derive, never restate.** Both documents are pure functions of the T52
   inventory and the S20-720 evidence record; nothing is hand-maintained, and
   `--check` fails on drift.
2. **Two formats, one inventory.** CycloneDX 1.6 and SPDX 2.3 are emitted from
   the same components so a consumer of either sees the same graph; the
   dependency relationships become `dependencies` and `DEPENDS_ON`.
3. **Deterministic identity instead of random identity.** The CycloneDX
   `serialNumber` is a UUID derived from the document digest and the SPDX
   `documentNamespace` is a `urn:sley2:spdx:` URN; the SPDX `created` field is
   the Unix epoch. No random or clock state enters an artifact of record.
4. **No legal opinion.** `licenseConcluded` is `NOASSERTION` and
   `LicenseRef-Proprietary` is an extracted licensing info naming the pending
   operator decision, so the missing root license stays visible instead of
   being papered over by a format conversion.
5. **The statement is signable later.** The provenance file wraps a pure
   in-toto statement next to a local `attestation` block that records
   `signed: false` and the open blockers, so a future signing step consumes
   the statement verbatim.
6. **Honest audit state.** These documents do not complete S20-710: the
   summary keeps `standards_sbom` and `release_provenance` false, adds
   pointers to the draft documents, and the audit keeps its blocker.
7. **Codes and staging.** 74000 through 74007 name the exact failures; a
   staged checker in `make quick` carries the contract from draft to complete,
   and the release smoke rebuilds all three documents.

## Consequences

- The dossier can cite standards-format SBOMs and a provenance statement
  today, with their limits stated in the same files.
- Adding a dependency changes the SBOM documents, so the drift check makes an
  unreviewed dependency addition fail `make quick` until the evidence is
  regenerated.
- Signing, VEX, and vulnerability data stay out of scope and remain visible as
  exclusions rather than silent gaps.
