# S20-530 Crash Recovery Closeout

Status: **S20-530 crash recovery complete under the v13 contract; the Sley 2 goal remains incomplete**

Date: 2026-09-03

Validation tier: **Tier 2 captured closeout plus the full contract checker**

## Closed claim

S20-530 closes the exclusive-recovery boundary across `sley-store`,
`sley-txn`, and `sley-repo` on the one-way dependency
`sley-repo -> sley-txn -> sley-store`. Under the frozen v13 contract
(`docs/spec/CRASH_RECOVERY_MATRIX_V1.md`, ADR-0023, contract set
`0257eddda95d24dba657b993eee981443e6c215e93f7cb04387a8d444a9c7caa`):

- every durability interruption in the exact 100-row crash matrix yields the
  old state or the complete new state, never a partial accepted state;
- owner preflight is read-only, cleanup never precedes corruption detection,
  delete-before-sync retry gaps are closed, ancestry work is bounded, and the
  lock order is fixed;
- exact guards, limits, work accounting, witness states, and component reports
  are closed;
- all 100 rows and every evidence-family subcase map to passing production
  tests (419 mapped tests) in one captured Tier 2 closeout.

## Evidence

- Contract freeze: `evidence/validation/s20-530-crash-recovery-contract-freeze-v13.json`
  in its single addition commit `250557833364ae1f85dc67290ff6c7c5f0c7fdba`,
  with `PASS_CONTRACT_FREEZE` receipts from Nabu, Ariadne, and Vulcan
  (sessions `…20260902T173505-4e2f40e0`, `…-ee0cd3fa`, `…-d7eb9f55`),
  verified `PASS_TRUSTED_LOCAL_REVIEW_RECEIPTS`.
- Test plan: `evidence/validation/s20-530-crash-recovery-test-plan-v1.json`
  (rows=100, tests=419).
- Captured closeout: `evidence/validation/s20-530-crash-recovery-closeout-v1.json`
  PASS at validated commit `8f7c7630c3ba786478aa0066ba35c271feff2036`, source
  set `468490e1b67360344fd8384a71be3861b650eb6004587256c6fa70c859cc2ab5`,
  with 28 retained logs under
  `evidence/validation/s20-530-crash-recovery-logs-v1/`.
- Implementation receipts: `PASS_IMPLEMENTATION` from Nabu, Ariadne, and
  Vulcan (sessions `…20260902T215406-23c1f91b`, `…-3ae4faa7`, `…-33df6034`),
  verified `PASS_TRUSTED_LOCAL_REVIEW_RECEIPTS`.
- Acceptance: commit `034cc75abb59d743ea30b2f2a410205a018cb25f` records
  `implementation_complete: true` in `machineresearch/sley-2.0/machine-summary.json`.
- Final checker confirmation: `python3 scripts/check_s20_530_crash_recovery.py`
  at commit `cc0f92f0b3ff41f3f8ca5db86117619255ddac5c` printed
  `S20-530 crash-recovery contract check: PASS (100 exact matrix rows;
  implementation_complete=True)` (2026-09-02T22:58:01Z to
  2026-09-03T01:06:49Z, exit 0); the log is
  `machineresearch/sley-2.0/s20-530-final-checker-v13-confirmation-2026-09-02.log`.

The campaign record, thirteen contract versions, and the amendment designs are
under `machineresearch/sley-2.0/`.

## Independent review

Nabu, Ariadne, and Vulcan each returned `PASS_CONTRACT_FREEZE` on the v13
contract and `PASS_IMPLEMENTATION` on the captured closeout, with no blocking
finding. Vulcan additionally confirmed all 28 retained-log digests and that the
nine `rustfmt::skip` occurrences hide nothing beyond the three frozen
owner-module chains. The implementation receipts were obtained on
`claude-cli/claude-opus-5` after the operator directed the switch away from
the exhausted OpenAI account; the Council is model-neutral and the receipts are
trusted local session transcripts exactly as the contract requires.

## Post-acceptance aging

ADR-0024 governs how this accepted package ages. The full checker is
authoritative at the accepted state and is rerun there by
`make s20-530-verify`; `make quick` runs the read-only
`scripts/check_s20_530_acceptance_anchor.py`, which proves the accepted
evidence is intact as immutable history. Later changes to the owner sources
are governed by the ordinary gates, the mapped tests, and the changing
package's own contract.

## Aging rule review

Bounded reviews on `claude-cli/claude-opus-5` of commits `3036c15` and
`18d4296` (range `cc0f92f..18d4296`), advisory because ADR-0024 is not an
S20-530 contract phase:

- Nabu, session `forge-nabu-s20-530-aging-20260903T012356-62f886b2`
  (2026-09-03T01:28:33Z): `PASS_AGING_RULE`, no blocking finding. Advisory
  P2: the ADR-0024 re-validation wording was not executable because the
  frozen checker reads the v13 evidence at frozen paths and rejects the added
  lint-allow attribute chain by construction; P3: the anchor did not bind its
  own Makefile registration, the closeout audit's fact header, or the
  test-plan builder and grouped-M2 renderer; the verify script copied the live
  `.git/config`; ownership reversion after acceptance was unstated; the first
  `make s20-530-verify` run had not completed.
- Vulcan, session `forge-vulcan-s20-530-aging-20260903T012356-3f3125a1`
  (2026-09-03T01:33:30Z): `PASS_AGING_RULE`, no blocking finding. Confirmed
  the range touches no production code, conformance vector, or S20-530 bound
  digest, that the T54 delta is counters-only with zero findings, and that
  the verify script cannot pass falsely. Advisory P2: gate removal and the
  anchor's own trust base were unbound; P3: tree modes were not compared, the
  clone ran without a clean Git environment, and the `dead_code` allow could
  hide a dropped `#[test]`; P4: some machine-summary fields and the nested
  receipt digests were not compared.

Disposition (commit after `18d4296`): ADR-0024 sections 1, 3, and 4 restate
the rule executably (re-validation is a new contract version at versioned
evidence paths; live ownership returns to the current package owner; the
frozen checker cannot pass after the confirmation commit by construction);
the anchor now binds its `make quick` line and the `s20-530-verify` target,
this audit's fact header, the test-plan builder and renderer, tree modes,
every machine-summary S20-530 scalar, the six receipt rows byte-for-byte
against the frozen evidence, the presence of all 419 mapped tests with
`#[test]` in their owner crates, and the digests of ADR-0024 and the verify
script; the verify script writes the frozen `.git/config` and
`.git/info/exclude` bytes and runs Git with a clean environment.

First `make s20-530-verify` run (2026-09-03T01:23:36Z to 03:36:05Z, isolated
clone at `cc0f92f`): FAIL with `Git local authority differs from validated
execution`. Root cause: the verify script forced `.git/config` and
`.git/info/exclude` to mode `0644`, while the validated Git-authority record
captured `0664` under the workstation's umask; the frozen checker compares
those modes exactly. The script now applies the modes recorded in the
frozen closeout evidence and fails early on an owner mismatch.

Second run (03:38:05Z to 05:42:12Z): the Git-authority check passed and
the checker failed one step later with `current non-output bytes/modes
differ from validated commit`: the clone's working-tree files were `0664`
under the workstation umask while the validated blobs are `0644`. The script
now clones under umask `022` and restores the recorded `0775` Git-authority
directory modes afterwards.

Third run (05:43:30Z to 07:30:20Z, 107 minutes): **PASS**. The checker
reported `S20-530 crash-recovery contract check: PASS (100 exact matrix
rows; implementation_complete=True)` at `cc0f92f` in the isolated clone,
and the script removed the clone. This is the first completed ADR-0024
verification run; both failed runs are non-authoritative evidence of the
script, not of the accepted state, whose in-place confirmation at
`cc0f92f` stands and is now reproduced in isolation.

## Validation record

The captured Tier 2 closeout at `8f7c763` ran three Cargo test lists and 25
commands (owner crate tests, workspace tests, SCB1 and oracle conformance, the
adversarial slices, the exception-ledger reconciler, and `cargo fmt --check`),
all PASS with 28 retained logs. The full contract checker passed at `cc0f92f`.

`make quick` as a whole was not green at the accepted state, for reasons that
predate S20-530 acceptance and sit ahead of its line in the target: the S20-710
T54 secret-scan manifest hashes every candidate file and had not been
regenerated since `7c622a3` (2026-08-30), and the S20-390 fixture-refresh check
drifted from `8343beb` (2026-08-30) when the recovery tests widened the shared
test grant with `DeleteEntityBinding`, which changed the emitter's genesis. Both
were repaired at the first ADR-0024 commits on 2026-09-03: the T54 evidence was
regenerated (counters only, zero findings) and the ignored emitter now builds
its fixture with the frozen S20-390 grant, so the reviewed conformance vectors
and every bound digest are unchanged. The 1,244 rustc lint warnings raised by
the three frozen mapped-test modules (`sley-store/src/lib.rs`,
`sley-txn/src/repository.rs`, `sley-repo/src/refs.rs`) are expected contract
forms and are allowed at those modules with an explanatory comment; lint
hygiene for the modules is a deferred slice. After the repairs `make quick`
(Tier 1, now running the acceptance anchor) passes in about 18 seconds with no
warnings, and `cargo test --workspace --locked` passes.

## Explicitly open

This closeout does not complete:

- S20-540 clone-equivalent pack exchange (next dependency-complete package);
- S20-510 semantic comparison (blocked by full S20-250) or S20-520 merge;
- full S20-250 entity bodies or complete-root impact semantics;
- protocol, bridge, CLI, runtime deployment, real benchmark trials, release
  evidence, packaging, publication, or GA.

The local-write gate remained in force: no push, runtime deployment,
publication, spend beyond Council model calls, trading action, or external
system mutation occurred.
