# Resume state, 2026-09-08 (architecture-tightening campaign closed)

The previous resume note (2026-09-05, S20-330 closure checkpoint) is history
in git at `560a5f16`. Since then: REWEAVE-1.0 was adopted (ADR-0049,
2026-09-06), RW-030 to RW-075 landed, RW-080 produced provisional C0
construction slices 1 to 7 under the 2026-09-07 operator override, and the
pre-freeze architecture-tightening campaign ran on 2026-09-08.

## Where the truth is

| Thing | Where |
|---|---|
| Repository | `/home/greyforge/sley2`, branch `main`; private `origin` on GitHub (browse it for the latest validated commits); public sanitized mirror is remote `mirror`, unrelated history, never force-pushed |
| Remote-head locator | `docs/status/SLEY2-REMOTE-HEAD.md` (derived from the machine summary's `locator` by `scripts/check_remote_head.py --render`) |
| Campaign record | `docs/audits/SLEY-2.0-ARCHITECTURE-TIGHTENING-AUDIT.md` (80 findings, gate table, closeout block), plus `-NATIVE-INVENTORY.md` and `-IDENTITY-MAP.md` |
| Independent reviews | `machineresearch/sley-2.0/reviews/arch-tighten-{nabu,ariadne}-2026-09-08.log` |
| REWEAVE lane records | `machineresearch/sley-2.0/reweave/`; SH2 work-item registry `evidence/reweave/sh2-work-items.json` |
| Council review queue and retained verdicts | `/home/greyforge/machineresearch/sley-2.0/council-queue/`, `machineresearch/sley-2.0/reviews/`, `reviews/verdicts.json` |

## What the campaign concluded

The architecture is sound: no ProgramRoot, the v2 execution package already
binds the closure, every native semantic operation is declared C0 seed with
its assigning REWEAVE clause, no forbidden SH2 shortcut exists, BLACKGLASS
wording is intact, Witness is not yet active, and primitive mutation costs
1 to 18 primitives per edit class. The defects were in records and gates,
and were repaired (derived review evidence, master-goal path binding, v1
profile digest literal, package identity wording, bootstrap manifest P,
SMP1 revision pins, fixture-family count, SH2 registry, domain-tag table,
locator). Closeout block: audit record section 7.

## Running the gates from any checkout

- `make quick` needs `SLEY2_MASTER_GOAL=/home/greyforge/machineresearch/Sley2.0mastergoal.md`
  (two checkers read the master goal outside the repository).
- In a fresh worktree or clone, four quick steps fail until
  `make release-candidate-smoke` has produced the gitignored candidate
  evidence: check_release_candidate_packaging, check_reproducibility_and_
  independent_conformance, check_standards_sbom_and_provenance (all need
  `evidence/runtime/s20-720-release-candidate/evidence.json`) and
  check_error_symbol_registration --check (five RW-075 `PACKAGE_*` symbols
  unregistered; lane-owned). Everything else is green.
- `make lint` is green. `make remote-consistency REMOTE_CONSISTENCY_ARGS=--allow-ahead`
  runs the two remote gates (consistency and locator).

## To resume

0. **CLOSED 2026-09-10: AT-MW-02.** Nabu re-reviewed the pushed lane
   commit `aedc443a` and returned `PASS` with zero findings at every
   level (`AT_MW_02_NABU_REREVIEW_JSON`, council log
   `atmw02-nabu-rereview.log`, verdict `atmw02-nabu-rereview`): methods
   306/307 satisfy master 8.2 with exact root/session binding and the
   stated work formula, bounded-context accounting is honest, frozen v1
   is unwidened. Lane work: I1/I2/I3/I3b/I4, phase-3 v2 offer merged
   lane-to-lane as `7f120a5`, I4b demonstrations VALID
   (`docs/audits/AT_MW_02_I4B_DEMONSTRATIONS.md`). Owner: S20-410
   (with S20-310 query profile).
1. **RW-080 lane:** rebase the next summary or manifest edit on merged
   `main`; register the five `PACKAGE_*` error symbols in ERROR_CODES_V1 and
   exercise `PACKAGE_SECTION_DIGEST_MISMATCH` (Tier 1 red until then); the
   envelope byte codec (AT-EC-07) is yours when the builder emits bytes.
2. **S20-410 protocol package:** see item 0; bridge and CLI re-pins follow
   the SMP1 revision (their checkers now assert the pins both ways).
3. **Release smoke:** `make release-candidate-smoke` has not been re-attested
   since the S20-330 closure; the S20-730 report is stale by design until it
   runs. Expect the usual drift repairs, committed as deterministic rebuilds
   with the T54 scan last.
4. **Owner-work frozen vectors** (not done here): branch record and ref
   digests (S20-500), capability token and summary (S20-380), attempt,
   context and phase digests (S20-360).
5. **Push discipline:** every validated checkpoint goes to `origin`
   (operator instruction 2026-09-08); refresh the public mirror only through
   its pipeline under a publication decision; never spell the public
   repository name in tracked files (clean-room sentinel).

## Gates

Unchanged from the campaign closeout: R2 exit NOT_READY (premium delta
re-review), self-host succession not started (SH0), finding register
FINDING_REGISTER_OPEN, five operator gates (narrowed schema-epoch decision,
succession trials, root license text, second-host attestation, release
decision), publication_authorized false.
