# Resume state, 2026-09-05 (night, second push)

The Council review round is **complete (69/69) plus two reconciliation
re-reviews (71 verdicts: 68 FAIL, 3 PASS)**. The repository is clean and
Tier 1 (`make quick`, `make lint`) plus Tier 2 (`core`, `conformance`,
`adversarial`, `fuzz-smoke`) plus the clean `release-candidate-smoke`
are green at `651bcc3`.

## Where the work is

| Thing | Where |
|---|---|
| Repository | `/home/greyforge/sley2`, branch `main` |
| Review candidate | `/home/greyforge/cache/worktrees/sley2-review-2026-09-04`, detached at `9dc78fe` (four commits behind `main` now; see below) |
| Council review queue | `/home/greyforge/machineresearch/sley-2.0/council-queue/` (durable, and gitignored there) |
| Retained verdicts | `machineresearch/sley-2.0/reviews/` plus `reviews/verdicts.json` |
| Checkpoint narrative | `machineresearch/sley-2.0/COUNCIL_REVIEW_CHECKPOINT_2026-09-04.md` |

## Reviews run against an immutable candidate now

The first dispatches read the live checkout while it was being edited. The
Greyforge Git Guard recorded `Dirty repo after openclaw` on `260-vulcan-surface`
and all three `300` logs, which is the review mode's "immutable candidate"
requirement being violated: a reviewer reading a moving tree can produce
findings that match no commit.

Reviews now run inside a detached worktree pinned at `9dc78fe`. Every request
names that path, and `patient_dispatcher.sh` changes into it. Work on `main`
no longer touches what a reviewer sees. **When the round finishes, repin the
worktree (or make a new one) before starting another round, or reviewers will
be reading an old candidate.**

The round is deliberately still reading `9dc78fe` even though three fixes have
landed on `main` since. That is the point of pinning: a reviewer's findings
stay reproducible from one commit. It also means a reply may raise something
already fixed, so check a landing finding against `main` before acting on it.

## Council reviews: round complete (69/69)

All 69 dispatched and all 69 answered (68 FAIL, 1 PASS).
`machineresearch/sley-2.0/reviews/` holds every retained log,
`verdicts.json` the counts, and `p0-worklist.json` the derived P0 list.

The round raised **110 P0 entries** across 23 packages. Three replies arrived
truncated mid-object and were requeued; **check every reply ends in `}` before
counting it**. The final `740-vulcan-surface` verdict (4 P0) landed after the
`4aa867b` retain commit and is now retained here.


## Findings: 84 of 110 P0 entries closed

Closed, each reproduced before fixing:

- **S20-260** (3 entries): checked left shift; ordered-map entry order
  (`83d571a`); cell value units (`70f4a2e`).
- **S20-300** (2 entries, found independently by two reviewers): an exchange
  import adopting an index cache it never wrote (`cfdd263`).
- **S20-620** (12 entries, 7 distinct defects, the round's largest cluster):
  the handle that was not a capability boundary (`28ec1fc`); `context_bytes`
  undercounting and the arm grading itself (`48b71e9`); the swallowable guard
  (`a0d8a6f`); the unfrozen affordance allowlist and protocol failures read as
  candidate verdicts (`20e8bf4`); the shared execution controls narrowed per
  arm (`1e5e77b`).
- **S20-400** (8 entries): SMP1 frame contract revision 9 (`19587b5`).
- **S20-320** (7 entries): context-capsule contract revision 3 (`9164ba3`).
- **S20-330** (6 entries): negotiated-session contract revision 2 (`ed7fe87`).
  Repair 2026-09-05 (`9cb1fc6`): both fixes landed with closeout
  mappings but the register-first step was skipped, so the summary
  still read open 7/6 against this list's claim. Each of the thirteen
  findings was verified against `main` (oracle PASS 24 vectors,
  negotiated arm required; both stage checkers PASS; sley-protocol
  30/30; the 320 revision cross-check verified live by drift
  injection) and the dispositions recorded; the 84 count below is now
  what the register actually reads.
- **S20-520** (7 entries): merge contract revision 4 (`ee7451d`).
- **S20-630** (7 entries): succession-accounting contract revision 3
  (`5580fe6`).
- **S20-310** (4 entries): confirmed closed on `main`, not partial. The two
  Ariadne P0s (charging schedule, applicability table) closed in `d047eaf`;
  the two Nabu P0s (input binding, charging schedule) closed in `95c90df`.
  Checker `PASS`, `p0_open_count` 0.
- **S20-740** (8 entries): finding-register contract revision 2 (`524188d`).
  First-token classification, same-lane directional supersession, negation
  stripping, per-package clearance claims, frozen obligation payload,
  per-status verdict assertion. The two restricted-query Nabu `REVISE`
  records this rule surfaced were re-reviewed by Nabu to `PASS` with no
  findings (logs retained, verdicts 71 total); the `REVISE` dispositions
  stand as history with `superseded_by` naming the new fields. Register now
  reads 204 obligations (87 `PASS`, 26 `HISTORICAL_ROUND`, 79 `PENDING`, 11
  `DEFERRED`, 1 `OTHER`), result `FINDING_REGISTER_OPEN`. Closeout
  `docs/audits/S20_740_FINDING_REGISTER_CLOSEOUT.md`.
- **S20-750** (5 entries): decision-dossier contract revision 5 (`5e4f1ae`).
  Unshadowed the license/test inventories (SBOM entry now reports the true
  19 `BLOCKED` against the source it cites), counted the property-test
  absence in the test inventory (zero with harness scan, no unit-count
  substitution), and made `derive_decision` read the entries per a section 3
  mapping written into the contract, with missing/gated inputs failing
  closed. No re-review needed: the round verdicts stand as `FAIL` history.
  Closeout `docs/audits/S20_750_DECISION_DOSSIER_CLOSEOUT.md`. The register
  picked up the closure (`decision_dossier.p0_open_count` 5 to 0) on
  rebuild.
- **S20-720** (5 entries): packaging contract revision 3 (`42e5b00` plus a
  call-site repair `935a5d5`). Remaps ordered most-general-first (the old
  order shadowed the tree rule; the shipped binary carried fourteen
  `/home-remapped` paths, confirmed with `strings`), scan needles for the
  residue and username, `--require-clean` default with `--allow-dirty`
  escape, self-describing manifest (GA/publication/blockers/cleanliness
  inside the digest), honest 20.12 verb enumeration (four covered, four
  residual; open question 1 answered yes as a `workspace.create`
  extension), and a demo-limits repair (the S20-330 `max_sessions` field
  broke every demo frame; fixed with a bridge-pinned limit set). Round
  verdicts stand as `FAIL` history. Closeout
  `docs/audits/S20_720_RELEASE_CANDIDATE_CLOSEOUT.md`.
- **S20-730** (5 entries): reproducibility contract revision 3
  (`b9bab83`). Hermetic digest/shape verification in the checker,
  attestation ancestry plus artifact-surface freshness plus toolchain
  matching, carry-forward rebuilds, and a semantic/codec-and-identity
  depth axis with the `COMPLETE` limits stated. Round verdicts stand as
  `FAIL` history. Closeout
  `docs/audits/S20_730_REPRODUCIBILITY_CLOSEOUT.md`.
- Evidence commit `8722a55`: clean smoke at `e20b9ad` (2,083,924-byte
  artifact, demo 12/12, byte-identical rebuild, 4 `/sley2` / 0
  `/home-remapped` / 0 username strings); the repro report attests the
  commit single-host; `make quick` fully green including the new gates.
- **S20-710** (5 entries): SBOM/provenance contract revision 3
  (`7de2d99` code, `ad53ee8` dispositions, `651bcc3` evidence). License
  normalization plus grammar validation (the verbatim `MIT/Apache-2.0`
  is now `MIT OR Apache-2.0`; anything unparseable is
  `SBOM_COMPONENT_INCOMPLETE`), SPDX namespace bound to the candidate
  artifact as well as the inventory, and fail-closed `--check`
  (`local_build_ahead` fires only on loaded-and-differing evidence;
  missing evidence exits 1 with 74000/74004, verified live). The old
  Tier 1 hermeticity claim is retracted: the drift gates require
  candidate evidence. Fourteen new hermetic tests (29 in the file); the
  checker pins namespace, expression validity, and counts (which caught
  a stale 119 vs 120 relationship count in the summary). Round verdicts
  stand as `FAIL` history. Closeout
  `docs/audits/S20_710_STANDARDS_SBOM_CLOSEOUT.md`. A mid-session smoke
  failed `PACKAGE_TREE_DIRTY` on pre-smoke refresh drift; corrected by
  committing dispositions, discarding derived drift, and re-running
  clean.

**26 entries remain open.** Five clusters tied at 4: 390extended, 420,
430, 510, 700fuzz; then 360full (3), 780 (3).

Two patterns account for most of what has been closed, and are worth carrying
into the rest:

1. **A contract asserted an invariant nothing executed.** The map order, the
   cell units, the handle boundary and the affordance list were all like this.
   When a reviewer cites a clause, check whether anything runs it.
2. **A measurement favoured the thing it measured.** `context_bytes`,
   `invalid_candidates` and the oracle override all read low for the arm under
   test. Ask which side an error falls on.


## To resume

1. **The dispatcher is done.** `patient_dispatcher.log` ends `ALL_DISPATCHED`
   (69 round reviews plus the 2 reconciliation re-reviews); do not restart
   it. If a new round starts, repin the worktree first (see above) so
   reviewers read the new candidate. The re-review worktree
   `/home/greyforge/cache/worktrees/sley2-review-2026-09-05` (at `9c7d4ba`)
   can be removed with `git worktree remove` once no session needs it.

2. **Work the next P0 cluster**: 390extended, 420, 430, 510, or
   700fuzz (4 each, tied largest).
   Triage each landing verdict into the S20-740 finding register by recording the disposition in
   `machineresearch/sley-2.0/machine-summary.json` and rebuilding, and keep
   `reviews/verdicts.json` current. Take reviewer counts from the emitted
   JSON, not the prose.

## Gates

Council model access is open. Five stand, all outside the integrator: the
narrowed schema-epoch decision, succession trials (model access plus spend
authorization), the root license text, second-host attestation, and the release
decision.

## Validation at this commit

`make quick`, `make lint`, Tier 2 (`core`, `conformance`, `adversarial`,
`fuzz-smoke`), and the clean `release-candidate-smoke` all pass at
`651bcc3`. The full `make v1` gate was skipped
because the 740 revision is a subsystem handoff, not a release boundary;
`make v2` and `make release-check` remain intentionally fail closed. One
combined Tier 2 invocation at an earlier checkpoint exited 2 once and did
not reproduce across later runs, individually or combined.

Two clippy errors exist in `fuzz/targets/root_query_engine.rs` and
`context_capsule_builder.rs` (`manual_is_multiple_of`). They are pre-existing:
`make lint` runs `cargo clippy --workspace`, which does not include the separate
`fuzz` crate, so that crate has never been linted under `-D warnings`.
