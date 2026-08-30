# Sley 2.0 development checkpoint

Status: RESUMED, V5 CONTRACT FROZEN, IMPLEMENTATION PENDING

Owner: Codex orchestrator

Checkpoint date: 2026-08-30, America/New_York

## Repository state

- Repository: `/home/greyforge/sley2`
- Branch: `main`
- Immutable v5 freeze commit:
  `d152598425c3dff4ff2e893195f77936bf0b2327`
- Contract-preparation commits:
  `7937a50`, `68c1982`, and `792a86b`
- Nothing was pushed, deployed, published, or executed against an external
  runtime.
- Provider-backed Council calls were limited to the explicitly authorized
  read-only Nabu, Ariadne, and Vulcan reviews.
- `greyforgelab` remained disabled.

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
Remaining implementation work includes the mapped grouped-test frontier,
helper-body manifests, dual-site proof manifests, limit-event control ancestry
and public-root call paths, and complete shared-state authority evidence.

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

Validation tier: Tier 2 contract refreeze, with one monolithic runtime debt.

- `cargo test -p sley-txn`: PASS, 123 active and 1 ignored.
- `cargo test -p sley-repo`: PASS, 244 active.
- Candidate and state-root carrier recovery prototypes: PASS, 8 exact errors.
- Frozen test-authority inventory: PASS, 50 files.
- Grouped and fixture metadata: PASS, 73 COR-06, 159 COR-07, 232 fixtures.
- Typed empty source-chain positive and hostile controls: PASS.
- Transaction and ref exact source-helper bodies: PASS.
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
2. Begin the remaining S20-530 mapped grouped-test implementation from the v5
   checker authority, not the historical v4 inventories.
3. Use targeted crate tests and checker-owned render/binding controls as the
   inner loop.
4. Keep the monolithic checker runtime debt separate from semantic failures;
   do not claim the full entrypoint passed unless it terminates successfully.
5. Complete the helper-body, dual-site, limit ancestry, public-root call-path,
   and shared-state evidence obligations before setting
   `implementation_complete` to true.
6. Run `make v1` only at the release boundary or under explicit operator
   direction.

The completion estimate has already been supplied in this thread. Do not issue
another estimate automatically unless the operator asks.

## Authority and routing

The active authority set included:

- `/home/greyforge/machineresearch/reference/pointer.md`
- `/home/greyforge/machineresearch/reference/operations-index.md`
- `/home/greyforge/obsidian-forge/Home.md`
- `/home/greyforge/machineresearch/reference/greyforge-operations-rules.md`
- `/home/greyforge/sley2/machineresearch/sley-2.0/s20-530-v5-semantic-amendment-design.md`

The posture remains write-local only. Push, deployment, publication, spend,
trading, and external runtime mutation remain unauthorized.
