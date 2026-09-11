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
the 20.12 verbs it covers (execute, branch, export, import — four of
eight; create, modify, test, and merge are residual and enumerated in
contract section 4): it imports the executable
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
  `4adaca8`; revision 2 and the mechanics at `cb51cb0`.
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

Tier 1 `make quick` passed at the commit. Tier 2 ran on 2026-09-03 at
`cb51cb0` (`make core` 985 tests, `make conformance` 19 oracles,
`make adversarial` 597 tests, `make fuzz-smoke`,
`make release-candidate-smoke` reproducible, and the S20-620 and S20-630
smokes, all exit 0 in 61 seconds of wall time) and is recorded in
`machineresearch/sley-2.0/s20-720-release-candidate-campaign-2026-09-03.md`.
The full `make v1` gate was skipped because this is a subsystem handoff,
not a release boundary; `make v2` and `make release-check` remain
intentionally fail closed.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.

## Revision 3 closeout (2026-09-05)

The Council round returned five P0s (Ariadne 2, Nabu 1, Vulcan 2), all
confirmed live and closed by fix; the round verdicts stand as `FAIL`
history. No re-review was needed: the fixes change mechanics and reported
facts without contradicting any recorded disposition.

- Remap order (Vulcan P0-1): `remap_flags()` emits home, registry, tree —
  most-general-first, because rustc applies the last matching rule. The
  prior order shadowed the tree and registry rules; the shipped binary
  carried fourteen `/home-remapped` paths (crate and registry sources) the
  old needles could never match, confirmed with `strings` on the staged
  artifact. The scan now also needles `/home-remapped` and the build
  username, and offline tests pin the order plus planted-leak detection.
  The positive control the review asked for lives in those tests rather
  than the gate: a `/sley2`-present assertion would fail a legitimately
  path-free binary, while the order test plus the residue needle close the
  exact hole that let the leak through.
- Cleanliness (Ariadne P0-1, Vulcan P0-2): `--require-clean` is now the
  default (opt out with `--allow-dirty`, which no tracked target uses),
  the smoke passes it explicitly, and the manifest carries
  `working_tree_clean` inside its digest and refuses a manifest without
  it. The reference dirty-tree artifact the reviews cited is superseded by
  the clean smoke below.
- Manifest self-description (Nabu P0): `ga_claimed: false`,
  `publication_authorized: false`, the blockers, and `working_tree_clean`
  are manifest fields inside the digest; the summary's candidate fields
  carry the built commit and its cleanliness beside the digest.
- Demo claim (Ariadne P0-2): section 4 enumerates the nine demo methods
  and the 20.12 verbs (four covered, four residual); the ADR and this
  closeout no longer claim every dispatched operation. Open question 1 is
  answered yes: covering create via `workspace.create` from a packaged
  trusted genesis lands as a demo extension, with create residual until
  then.
- Demo repair (blocking, new): the S20-330 `max_sessions` limit broke
  every demo frame at bridge shape validation (the demo's hardcoded limit
  set fossilized at seven), so the smoke could not produce evidence. The
  demo now zeroes the governed eight with a unit test pinning its names to
  the bridge's `LIMIT_FIELDS`.

Validation: Tier 1 (`make quick`, `make lint`) and Tier 2 (`make core`,
`make conformance`, `make adversarial`, `make fuzz-smoke`) are re-run after
the smoke below; the per-gate record lands with the evidence commit. The
full `make v1` gate was skipped as a subsystem handoff. P1 and P2 findings
from the round stay open and tracked; nothing here claims them.

Smoke record: `make release-candidate-smoke` passes clean on the
committed tree at `e20b9ad` (30.1 seconds): artifact
`sley-2.0.0-linux-x86_64.tar.gz`, 2,083,924 bytes, SHA-256
`ec20da7b…3a46`, fourteen members, manifest digest `114a940a…0ae8`
carrying `working_tree_clean: true` with the GA/publication flags and
blockers inside the digest; conformance subset PASS; demo PASS on all
twelve steps; forbidden content findings 0; reproducibility
`REPRODUCIBLE` with no differing member. `strings` on the staged binary:
4 `/sley2` references (remap engaged), 0 `/home-remapped`, 0 build
username. The S20-730 staleness gate on the fresh attestation is green,
and `make quick` is fully green with it.

## Superseded-by note (round 7, 2026-09-11)

The smoke record above describes the `e20b9ad` candidate and is retained
as history. It is superseded by the clean-worktree re-mints filed since:
`56bac4b`/`ac4ce59a`, `9115bd0`/`ce87e6cc`, `6a33523`/`912d2881`, and
`cd3864a`/`c1dec862` (current attestation; canonical detached linked
worktree procedure, `docs/spec/RELEASE_CANDIDATE_PACKAGING_V1.md`
section 7). The register's `mint_worktree`/`mint_method` fields point at
the mint procedure; this closeout's per-gate narrative is not re-run per
mint.
