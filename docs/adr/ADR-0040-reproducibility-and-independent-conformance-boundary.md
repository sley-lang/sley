# ADR-0040: reproducibility attestations and independent conformance as derived evidence

Status: proposed; the S20-730 contract is a draft at revision 10 with
Council review pending; mechanics implemented (2026-09-03, revised
2026-09-05) with a single-host reproducibility report, a tracked
independent conformance report with coverage depths, and `release-check`
still fail-closed

Note (revision 7, 2026-09-15; reworded at revision 10): the tracked report carried a second-host attestation minted through the contract's section 5.1 runbook between `6a2eef7` and `22970457`; the re-mint at a7852028 is single-host again, so the current state is the tracked report, and decision 1's "names the gated second host" describes the single-host case.

Note (revision 8, 2026-09-15): host builds use `release-candidate-build`; merge receipts live in the records-only ledger `evidence/release/second-host-lane-records.json`, with retained attestation bytes and checked merge history. Verification follows the merge.

Note (revision 9, 2026-09-15): the provisional commit carries the merged report and retained attestation only; the mandatory evidence refresh follows receipt filing. This matches the exercised sequence and avoids a redundant pre-commit refresh.

Date: 2026-09-03

## Context

The master goal's GA candidate needs a reproducibility report and a final
independent PASS. S20-720 established byte-for-byte reproducibility on one
host and toolchain and left the second host to S20-730. The independent SCB1
oracle already checks seventeen fixture families under `make conformance`,
but nothing recorded which families it covers, which are exercised only
natively, and whether the oracle stayed free of Rust dependencies. The second
host (`greyforgelab`) is reachable but gated: the operator posture allows it
only for ZJX performance qualification, so a Sley build there is not
authority-safe today.

The Council lanes were still unavailable at this draft (see ADR-0026); the
design is the integrator's and is submitted to Ariadne, Nabu, and Vulcan as
soon as a lane returns.

## Decision

1. **Attestations, not assertions.** A host contributes a small attestation
   derived from its own S20-720 evidence record; the report merges
   attestations and computes the multi-host result. A single host yields
   `SINGLE_HOST_REPRODUCIBLE` and names the gated second host; nothing
   claims what was not attested.
2. **Conflicts fail closed.** Two attestations for one commit with different
   artifact digests are a conflict and no report is written.
3. **Coverage is declared.** Every fixture family must be mapped to an
   independent oracle command of the `make conformance` recipe or declared
   native-only; an unmapped family or a command missing from the recipe is a
   failure, so the report cannot silently lose coverage.
4. **Derived from the commit.** The independent conformance report is
   computed from tracked files only and checked for drift under `make quick`;
   the reproducibility report is rebuilt by the release smoke and records the
   attested commit.
5. **No identity leakage.** Neither report carries a host name, user name,
   path, or timestamp; host labels are operator-chosen.
6. **Codes.** S20-730 reserves 73000 through 73007 for the exact failures of
   the two scripts.
7. **Staging.** A staged checker in `make quick` tracks the contract from
   draft to complete; completion requires three Council `PASS` reviews.

## Consequences

- The GA dossier can cite one report per concern instead of transcript
  evidence; the second host stays an explicit blocker until the operator
  reopens that lane.
- The extended VM vectors and the release demo are checked at
  `codec_and_identity` depth (revision 2 closed their native-only state;
  revision 3 records the depth so the report no longer reads as a semantic
  judgment where none exists). No independent VM semantic oracle exists;
  commissioning one is a separate package.
- Adding a fixture family now requires declaring its coverage.
- A rebuild carries previously merged attestations forward, and every
  attested commit must be an ancestor of the filing HEAD with no
  artifact-surface file changed since (revision 3): the report cannot
  silently drop another host or present an old candidate as current.

Note (revision 10, 2026-09-18): a re-mint carries forward only attestations of the commit it re-attests; attestations of a superseded commit are listed as `superseded_attestations` in the report, never carried onto the new candidate and never dropped silently. The `exec-package-envelope` family's `v2` pin is named in section 3.
