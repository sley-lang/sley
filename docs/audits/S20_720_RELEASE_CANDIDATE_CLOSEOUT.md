# S20-720 Release Candidate Closeout

Status: **mechanics implemented under the draft Release Candidate Packaging v1 contract (revision 2); Council reviews pending, so the package is not complete; no release, no GA, `release-check` fail-closed; the Sley 2 goal remains incomplete**

Date: 2026-09-03

Validation tier: **Tier 1 plus release-focused Tier 2 handoff**

## Claim under review

The clean-room release candidate is mechanical evidence, not a release.
`scripts/build_release_candidate.py` builds `sley-cli` twice in fresh
target directories with the working tree, the cargo registry sources, and
the home directory remapped, stages the fourteen members the contract
names (the endpoint binary, canonical manifest, the S20-710 inventory as
SBOM, declared licenses with the root-license blocker, the SMP1, bridge,
and demo fixtures, and the self-contained demo), packages a deterministic
gzip tar, unpacks it outside the tree, runs the conformance subset and the
canonical demo from inside the unpacked artifact with an environment of
`PATH` and `LANG` only, scans every member for the tree path, any home
path, and bounded secret patterns, compares the two artifacts byte for
byte, and writes the evidence with the blockers that keep `release-check`
fail-closed. The demo is the master goal's source-independence proof for
every operation the protocol dispatches today: it imports the executable
genesis's exchange, answers the bound summary query byte-identically to
the fixture, executes the Function and reads the stored report back with
the fixture's identity, creates a branch and exports, imports the export
into a second directory and answers the same query and execution, and dry
runs GC on both with no deletion candidate. The contract is
`docs/spec/RELEASE_CANDIDATE_PACKAGING_V1.md` with ADR-0038; it is a draft
written and implemented while every Council lane was unavailable, so the
Ariadne, Nabu, and Vulcan reviews that freeze it and complete the package
are pending and must pass before the status above changes.

## Evidence

- Contract draft revision 1, ADR-0038, and the stage checker at
  `4adaca8`; revision 2 and the mechanics in the commit recorded in the
  campaign record.
- Offline tests (four, all pass): the tar is byte-identical across
  changed timestamps with sorted members, zero times and owners, normalized
  modes, and a zero gzip mtime; the manifest digest is canonical, stable,
  and detects a changed member or commit; the scan finds a planted home
  path and secret pattern; the comparison distinguishes a content
  difference (naming the member) from an archive-only one.
- `make release-candidate-smoke` (`evidence/runtime/s20-720-release-candidate/evidence.json`,
  ignored runtime path): result PASS; artifact
  `sley-2.0.0-linux-x86_64.tar.gz`, 1,990,615 bytes, SHA-256
  `72d2e936a6d122c2015c166ceb0b79860d50a4fde9988b8a282b1047c7a2edd4`,
  fourteen members, manifest digest
  `3a6ce05ab26adb7083e564cb58496e88fbd49c268fa2c2e4a0178c97a9530fca`;
  conformance subset PASS (methods table, version, frame decode against
  the bridge fixture, frame encode reproducing the frames); demo PASS on
  all twelve steps with a 9,430-byte export; forbidden content findings 0;
  reproducibility `REPRODUCIBLE` with no differing member; 24.6 seconds;
  working tree dirty (recorded, development run).
- The first run refused the artifact with `PACKAGE_CONTENT_FORBIDDEN`
  because ten cargo registry source paths under the home directory
  survived in the binary; the registry and home remaps closed it and the
  scan stayed in force.
- `make quick` green at the commit; `release-check` and `v2` still answer
  `NOT_IMPLEMENTED` with exit status 2. Tier 2: see the validation record
  below.

## Findings closed in flight

- The working-tree remap alone was insufficient (registry paths); revision
  2 names all three remaps.
- Revision 2 records tree cleanliness as a recorded fact enforced only on
  demand, the clone-side execution and GC steps of the demo, the member
  count, and the cleanup rule.

## Explicitly open and deferred

- **Council reviews.** Ariadne, Nabu, and Vulcan reviews land as contract
  revisions; the campaign record lists the open questions.
- **Gated blockers.** Root license text (operator), standards SBOM and
  provenance (S20-710 full), succession thresholds (S20-640), and the GA
  decision (S20-750) remain closed; `machine-summary.json` `artifact` stays
  null and no push, tag, upload, or publication exists.
- The demo cannot construct, commit, test, or merge candidates until a
  public candidate builder exists (S20-350 is proposal-only); it says so in
  its output.
- Reproducibility is established on this host and toolchain only; a
  second host is S20-730.

## Validation record

Tier 1 `make quick` passed at the commit. Tier 2 is recorded in
`machineresearch/sley-2.0/s20-720-release-candidate-campaign-2026-09-03.md`.
The full `make v1` gate was skipped because this is a subsystem handoff,
not a release boundary; `make v2` and `make release-check` remain
intentionally fail closed.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.
