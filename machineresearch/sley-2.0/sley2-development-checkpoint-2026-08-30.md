# Sley 2.0 development checkpoint

Status: PAUSED BY OPERATOR

Owner: Codex orchestrator

Checkpoint date: 2026-08-30, America/New_York

## Repository state

- Repository: `/home/greyforge/sley2`
- Branch: `main`
- Last landed commit before this checkpoint:
  `c605395aa2f6beef6eb9f55b0226f8c992314856`
- Commit subject: `docs: propose S20-530 v5 semantic amendment`
- Nothing was pushed, deployed, published, or executed against an external
  provider.
- The worktree was clean before this checkpoint file was added.

## Current S20-530 state

The immutable v4 grouped corruption authority contains 235 leaves. The live
Rust sources contain 126 exact grouped tests:

- COR-06: 2 of 74;
- COR-07: 124 of 161.

Every remaining v4 leaf is covered by one of six recorded defects. There is no
known honest v4-compatible grouped leaf left to implement without changing the
frozen contract or one of its exact test authorities.

The proposed v5 amendment is documented in
`machineresearch/sley-2.0/s20-530-v5-semantic-amendment-design.md`. Its derived
inventory is:

- 71 normalized visible-revision cases;
- 73 COR-06 leaves;
- 159 COR-07 leaves;
- 232 corruption fixture plans;
- 419 unique mapped Rust tests.

The proposal makes no production behavior change. It repairs frozen Rust call
and assertion shapes, aligns eight visible SCB cases with production carriers,
removes one recovery-unreachable label case, and records decoder-before-
semantic precedence for two candidate cases.

## Frozen v4 identities

- Checker raw SHA-256:
  `44b7d77bd010ae0d82bc5c32de3870133373776913d92418b5af7590cf9e377d`
- Contract set SHA-256:
  `4c6b0e12836f6601df4cfc67e36fdf754b7bd1a60c7df2ca21c2b6e037ca93fa`
- Evidence payload SHA-256:
  `cf8491ff4928679554b138f2b2391f3bb4f67699c8aec366e6843e05e9306467`
- Checker self SHA-256:
  `4941f73a60f1e80807d517cd49ffe53109edc845e24c6933ebdd58ad9e010752`

Do not edit `scripts/check_s20_530_crash_recovery.py`, the frozen specification,
ADR-0023, the runner, freeze evidence, or machine-summary bindings without
explicit operator authorization for the v5 refreeze.

## Last validation evidence

Validation tier: Tier 1 targeted documentation and contract-design checks,
plus the bounded supply-chain audit.

- Proposed count derivation: PASS for 71, 73, 159, 232, and 419.
- Checker raw identity: PASS, unchanged.
- `git diff --check`: PASS.
- Greyforge no-em-dash check: PASS.
- `python3 scripts/generate_supply_chain_evidence.py`: PASS with two outputs.
- `python3 scripts/generate_supply_chain_evidence.py --check`: PASS.
- T52 local lock inventory: PASS.
- T54 high-confidence secret scan: PASS.
- Supply-chain audit: DEFERRED only for operator-approved root license text and
  the release SBOM.
- Full `make v1` gate: not run because this was not a release boundary.

## Exact resume point

1. Read this checkpoint and
   `machineresearch/sley-2.0/s20-530-v5-semantic-amendment-design.md`.
2. Confirm the checker raw SHA-256 before any work.
3. Keep the v4 freeze immutable unless the operator explicitly authorizes the
   S20-530 v5 refreeze.
4. After authorization, first prototype the three typed-candidate SCB carriers
   and five nested state-root carriers through the production recovery import
   path. Require the exact proposed error codes before changing contract
   authority.
5. Apply Amendments A through F only after those prototypes pass. Recompute all
   grouped, fixture, multifault, source, body, contract-set, and evidence
   digests rather than copying proposed values.
6. Settle all bytes before requesting fresh Nabu, Ariadne, and Vulcan reviews.
   Prior v4 reviews cannot close v5.
7. Follow the tiered validation sequence in the v5 design and commit the
   refreeze as one coherent change set.

The completion estimate has already been supplied in this thread. Do not issue
another estimate automatically when work resumes unless the operator asks.

## Authority and routing

The last active turn used:

- `/home/greyforge/machineresearch/reference/pointer.md`
- `/home/greyforge/machineresearch/reference/operations-index.md`
- `/home/greyforge/obsidian-forge/Home.md`
- `/home/greyforge/machineresearch/reference/greyforge-operations-rules.md`
- `/home/greyforge/machineresearch/reference/gold-standard-prompt.md`
- `/home/greyforge/machineresearch/reference/tiered-validation-policy.md`
- `/home/greyforge/sley2/README.md`
- `/home/greyforge/sley2/docs/WORK_PACKAGES.md`
- `/home/greyforge/machineresearch/sley/s20-530-checkpoint-2026-08-30/README.md`

The allowed posture was write-local only. No Council call or specialist
handoff was used because provider-backed review requires explicit operator
authorization. `greyforgelab` remained disabled.
