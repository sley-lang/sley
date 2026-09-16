# Resume state, 2026-09-14 (qualification wave after P-C)

Latest recovery: `docs/status/RECOVERY-2026-09-15.md`. It records integration
repairs, the reachable audit-anchor mapping, and validation caveats. Read the
current machine summary for candidate identity and outstanding reviews.

The previous resume note (2026-09-08, architecture-tightening closeout) is
history in git at `7c83630` (the 2026-09-14 operator-approved attribution
strip rewrote every commit id; trees are unchanged, so records that name
pre-rewrite ids map by tree: `c0f4ff6` -> `7c83630`, `43f2f5b` -> `7c7da9c`,
`1023a31` -> `e0ec341`, `d384f0f` -> `bf5b7e7`, `baf9a4d` -> `eae95d9`). Since then: the licensed candidate (Apache-2.0
root license, L1 `724a899`), the dual-host attestations for `74bb0ba` and
`baf9a4d`, the P-A symbol-gate closure, the P-C package repair wave with
harness finals (2026-09-13/14), and this wave's qualification repairs,
council-lane reviews, and re-mint.

## Where the truth is

| Thing | Where |
|---|---|
| Repository | `/home/gfarch/Work/workspaces/sley2`, branch `main`; private `origin` (`sley2.git`); public sanitized mirror is remote `mirror`, unrelated history, never force-pushed. `/home/greyforge/sley2` is a stale earlier checkout (94+ commits behind); do not resume there. |
| Acceptance map and closure order | `evidence/review/S20_710_ACCEPTANCE_MAP.md` (sections 5 and 9-10: P-A done, P-C done, this wave's records) |
| Review verdicts | `evidence/review/verdicts/<section>/` (the register derives from `machineresearch/sley-2.0/machine-summary.json`; a transcript is evidence, the summary field is the record) |
| Register / dossier / GA report | `evidence/review/finding-register.json`, `evidence/release/decision-dossier.json`, `evidence/release/ga-acceptance-report.json` (all derived; rebuild with `make evidence-refresh`) |
| Candidate | `release_candidate_packaging.candidate_*` in the machine summary is the live pointer; the reproducibility report names the attesting hosts |
| REWEAVE lane records | `machineresearch/sley-2.0/reweave/`; SH2 registry `evidence/reweave/sh2-work-items.json` |

## Running the gates from any checkout

- `make quick` needs `SLEY2_MASTER_GOAL=/home/greyforge/machineresearch/Sley2.0mastergoal.md`.
- `make lint`, `make core`, `make conformance`, `make adversarial`, `make fuzz-smoke`
  are green at the candidate commit named in the machine summary.
- In a fresh clone the release checkers need the gitignored candidate
  evidence: run `make release-candidate-build` (clean tree required; use a
  detached worktree when the checkout carries untracked material, contract
  `RELEASE_CANDIDATE_PACKAGING_V1.md` section 7) and merge the second-host
  attestation per `REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md` 5.1.
- Second host: `greyforgelab` over `forge-lab-connect` (`ssh greyforgelab`),
  toolchain pinned by `rust-toolchain.toml` (1.93.0 on both hosts). Copy a
  `git bundle` of `main`, clone detached at the candidate commit, run the
  build target, emit the attestation with `--host-label secondary`, copy the JSON
  back, merge with `--attest`, commit the merged records locally, file the
  records-eligible lane receipt naming that commit, refresh evidence, and run
  `make release-candidate-verify` after the merge.

## ZJX transport readiness amendment (2026-09-15)

The operator's S20-ZJX-READINESS amendment (tracked copy
`docs/audits/S20_ZJX_TRANSPORT_READINESS_AMENDMENT.md`) closed as
`READY_EXISTING`, documentation-and-test-only: the byte-oriented import seam
already exists (`import_conformance_pack`, `preflight_repository_exchange`,
`import_repository_exchange`), proven by the integration witness
`crates/sley-repo/tests/zjx_readiness_witness.rs` and recorded in
`docs/audits/S20_ZJX_TRANSPORT_READINESS_CLOSEOUT.md` plus
`evidence/validation/zjx-transport-readiness-v1.json`. Machine-summary
section `zjx_transport_readiness`; three council lanes review it under
`evidence/review/verdicts/zjx_transport_readiness/`. No ZJX version, backend,
or profile was selected, nothing ZJX ships, and the release disposition is
unchanged. The witness file is attestation-bound, so it rides the already
queued re-mint.

## Open items (machine-doable)

1. **Delta reviews at the new revisions.** CLI revision 8 and bridge
   revision 10 close the revision-7/9 delta REVISEs (transcripts under
   `evidence/review/verdicts/{cli,json_bridge}/delta_review_*-43f2f5b.md`);
   their own three-lane delta reviews are the next council round. When all
   three lanes PASS, the owner may move `cli` to `S20_430_COMPLETE` and
   `json_bridge` to `S20_420_COMPLETE` (the checkers require all-PASS
   current-delta records for a terminal status).
2. **Package terminal statuses.** Every S20 package whose three lanes PASS
   (finals plus current-delta where the checker binds one) can move from
   `*_IMPLEMENTED_REVIEW_PENDING` to its `COMPLETE` status through its
   checker; each move flips GA section 26 criteria from AWAITS_REVIEW to
   EVIDENCED. Do it package by package, running the package checker and
   `make quick` after each.
3. **Recorded P3/P4 follow-ups** from the 2026-09-14 round (all in the
   transcripts): S20-700 proof-record validation in every slice checker
   plus finding IDs and tracked seeds for the three retained crashers;
   root-query non-degenerate class-10/11 walk fixture and engine-emitted
   tamper descriptors; a hostile-label fault seed for T48; the S20-770
   drift-gate ownership fork (N-P1-4).
4. **Re-attest after any attestation-bound change** (anything outside
   `evidence/` and `machineresearch/`): mint in a detached worktree, attest
   on the lab, merge, commit the report and retained attestation provisionally,
   file its lane receipt, refresh evidence, verify and run `make quick`,
   commit the receipt, push.

## Held decisions (operator or council authority, not machine-doable)

- **S20-310 wording decision packet** (`root_backed_query_profile.contract_text_review`,
  `evidence/review/decision-packets/round-7-root-query-contract.md`): operator.
- **Succession trials** (S20-640): model access and spend authorization;
  the harness is ready (`make sley2-runner-smoke`, `make legacy-runner-smoke`,
  `make accounting-smoke` pass).
- **Signing and transparency** for the SBOM/provenance pair (P-D) and the
  history re-anchor at the release candidate (P-E): operator authorization.
- **Release decision, publication, GA claim**: operator; the dossier derives
  BLOCKED until the trials run and the register reads CLEAR.
- **REWEAVE (2.1.0) lane**: RW-075 premium delta round 2 and RW-080 C0
  slices stay outside the 2.0.0 qualification; see
  `evidence/reweave/sh2-work-items.json`.

## Push discipline

Every validated checkpoint goes to `origin` (operator instruction
2026-09-08); refresh the public mirror only through its pipeline under a
publication decision; never spell the public repository name in tracked
files (clean-room sentinel).
