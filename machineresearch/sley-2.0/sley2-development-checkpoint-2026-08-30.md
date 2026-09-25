# Sley 2.0 development checkpoint

Status: RESUMED, V5 CONTRACT FROZEN, MAPPED CORRUPTION COMPLETE,
BROADER IMPLEMENTATION IN PROGRESS

Owner: Codex orchestrator

Checkpoint date: 2026-08-30, America/New_York

## Repository state

- Repository: `/home/dev/sley2`
- Branch: `main`
- Immutable v5 freeze commit:
  `d152598425c3dff4ff2e893195f77936bf0b2327`
- Contract-preparation commits:
  `7937a50`, `68c1982`, and `792a86b`
- Freeze-checkpoint commit: `e757b2a`
- Mapped-test implementation commits:
  `4f485ce`, `1f71818`, `68428f3`, `ab04689`, `65abb3b`, `4cb55e5`,
  `6cd997e`, `a46c447`, `2fe3252`, `4463960`, `dd8843f`, `506d150`,
  `312ad22`, `baaaa66`, `91d00f6`, and `7c622a3`
- Nothing was pushed, deployed, published, or executed against an external
  runtime.
- Provider-backed Council calls were limited to the explicitly authorized
  read-only Nabu, Ariadne, and Vulcan reviews.
- `secondary-host` remained disabled.

## Current S20-530 state

The immutable v5 contract contains:

- 71 normalized visible-revision cases;
- 73 COR-06 leaves;
- 159 COR-07 leaves;
- 232 corruption fixture plans;
- 419 unique mapped Rust tests.

The v5 amendment makes no production behavior change. It repairs frozen Rust
call and assertion shapes, aligns eight visible SCB cases with production
carriers, removes one recovery-unreachable label case, and records
decoder-before-semantic precedence for two candidate cases.

Candidate and state-root carrier prototypes reached all eight exact nested
errors through direct transaction-receipt import and repository recovery.
The mapped COR-06/COR-07 Rust implementation is now complete at 232 of 232
exact tests: 73 of 73 COR-06 leaves and 159 of 159 COR-07 leaves. This closes
the prior 75-leaf frontier, including all 40 nested COR-06 receipt-digest leaves
and all 35 remaining COR-07 visible-revision and ref-owner leaves.

Remaining implementation work also includes helper-body manifests, dual-site
proof manifests, limit-event control ancestry and public-root call paths, and
complete shared-state authority evidence.

## Frozen v5 identities

- Contract set SHA-256:
  `de921bbe2efda26d77c2d7476a8d30556b4a6ec5e077e83541990f189255c08d`
- Evidence payload SHA-256:
  `58476201512efd1b310bcfbc51d408b87d5d1eb22681ef1e80041eca0a421f56`
- Checker raw SHA-256:
  `7f4abef51e602f252bdd00847b5bcf229d12c15af3d8bc6244b08a1a970e4ee6`
- Checker self-contract SHA-256:
  `622d1b683d318b76ae77a1eb6cc70ba8aa77d07e368089ed754ba1ea23ad4022`
- Freeze evidence:
  `evidence/validation/s20-530-crash-recovery-contract-freeze-v5.json`
- Freeze commit:
  `d152598425c3dff4ff2e893195f77936bf0b2327`

The v1 through v4 evidence files remain immutable historical evidence.

## Review evidence

Fresh Nabu, Ariadne, and Vulcan reviews each returned the exact object shape
required by the freeze gate:

- result: `PASS_CONTRACT_FREEZE`;
- contract-set binding:
  `de921bbe2efda26d77c2d7476a8d30556b4a6ec5e077e83541990f189255c08d`;
- evidence-payload binding:
  `58476201512efd1b310bcfbc51d408b87d5d1eb22681ef1e80041eca0a421f56`.

No v4 review was reused.

## Validation evidence

Validation tier: Tier 2 mapped-corruption closeout, with one monolithic runtime
debt.

- `cargo test -p sley-txn`: PASS, 194 active and 1 ignored.
- `cargo test -p sley-repo`: PASS, 279 active.
- Candidate and state-root carrier recovery prototypes: PASS, 8 exact errors.
- Frozen test-authority inventory: PASS, 50 files.
- Grouped and fixture metadata: PASS, 73 COR-06, 159 COR-07, 232 fixtures.
- Typed empty source-chain positive and hostile controls: PASS.
- Transaction and ref exact source-helper bodies: PASS.
- Checker-rendered fixture, error, source-chain, no-mutation, canary, and
  control-flow bindings for all 232 mapped corruption leaves: PASS through
  bounded inventory and group runs.
- COR-06 mapped implementation: PASS, 73 of 73 exact tests.
- COR-07 mapped implementation: PASS, 159 of 159 exact tests.
- COR-06 root group: PASS, 9 of 9 exact mapped tests.
- COR-06 policy group: PASS, 8 of 8 exact mapped tests.
- COR-06 receipt-envelope slice: PASS, 7 of 7 exact mapped tests.
- T52 local lock and dependency inventory: PASS; release SBOM and
  operator-approved root license remain deferred to the release boundary.
- T54 high-confidence secret scan: PASS, 352 files, 8,854,504 candidate bytes,
  and no findings.
- ANC-04 exact mapped-body bindings: PASS, 121, 89, and 124 statements.
- COR-07 ref-digest rendered body: PASS, 122 statements.
- Checker and runner Python syntax, Ruff format, and Ruff lint: PASS.
- Isolated freeze-evidence binding: PASS with the exact contract set and all
  three review objects.
- `git diff --check`: PASS.
- Full `make v1`: skipped because this is not a release boundary.

The monolithic checker entrypoint was interrupted after 2 hours 49 minutes at
99.9% CPU. Its captured stack was inside the CROSS-05 hostile negative control
and `rust_code_mask`; it emitted no contract failure but did not terminate.
This run is not recorded as PASS. The full entrypoint remains deferred as
validation-runtime debt under the validation economy rule.

## Exact resume point

1. Treat the v5 evidence and contract-set bytes as immutable.
2. Treat mapped corruption implementation as closed at 232 of 232. Do not
   reopen those test bodies unless a frozen contract input changes.
3. Continue with the remaining helper-body manifests, dual-site proof
   manifests, limit-event control ancestry, public-root call paths, and
   shared-state authority evidence.
4. Use targeted crate tests and checker-owned render/binding controls as the
   inner loop. Do not use the monolithic checker as a debugging command.
5. Keep the monolithic checker runtime debt separate from semantic failures;
   do not claim the full entrypoint passed unless it terminates successfully.
6. Complete the remaining proof obligations before setting
   `implementation_complete` to true.
7. Run `make v1` only at the release boundary or under explicit operator
   direction.

The completion estimate has already been supplied in this thread. Do not issue
another estimate automatically unless the operator asks.

## Authority and routing

The active authority set included:

- `/home/dev/machineresearch/reference/pointer.md`
- `/home/dev/machineresearch/reference/operations-index.md`
- `/home/dev/obsidian-forge/Home.md`
- `/home/dev/machineresearch/reference/greyforge-operations-rules.md`
- `/home/dev/sley2/machineresearch/sley-2.0/s20-530-v5-semantic-amendment-design.md`

The posture remains write-local only. Push, deployment, publication, spend,
trading, and external runtime mutation remain unauthorized.
