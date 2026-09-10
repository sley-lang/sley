# AT-MW-02 I4b response-derived edit demonstrations

Status: REVIEW_ACCEPTED_CLOSED, 2026-09-10. Nabu re-reviewed pushed lane
commit `aedc443a` and returned `PASS` with zero findings
(`AT_MW_02_NABU_REREVIEW_JSON` in
`machineresearch/sley-2.0/council-queue/atmw02-nabu-rereview.log`;
verdict `atmw02-nabu-rereview`). RESUME.md item 0 is closed on this
verdict. Implementation owner: Muse Spark (OpenCode).

## Prerequisite

Phase-3 v2 endpoint offer lane (`feat/phase-3-v2-endpoint-offer`, P0-P4
done, P4 unblock note in `docs/audits/PHASE3_V2_OFFER_DESIGN.md`) was
merged lane-to-lane into the AT-MW-02 lane as `7f120a5` under explicit
operator approval (2026-09-10, lane-to-lane only, not to main). The
merge is conflict-free; `check_remote_head`, SMP1 rev-12, CLI rev-6,
and entity-read vector gates all re-passed after it.

## What was built

Two demonstrations over the real runner/stdio pipeline
(`sley serve --protocol-profile v2-capable`, selection 2), each run
against two fixture variants (a/b):

* **expression**: `entity.version` (306) reads the exact current
  Operation object; the agent swaps the Boolean binary opcode
  (And 103 <-> Or 104) and preserves every other field byte for byte;
  the candidate (`ReplaceEntityVersion` + `ExactEntityVersion`
  precondition bound to the response ObjectId) creates and validates.
* **signature**: `entity.signature` (307) reads the Function plus its
  ordered Parameters; the agent retargets the first operand to the
  other declared Bool parameter selected by declaration order and exact
  type; the candidate creates and validates.

Fixture repositories are emitted by the Rust owner
(`emit_i4b_demo_exchange_for_fixture_refresh`, `crates/sley-repo`,
ignored test): genesis-only repos, one Namespace plus one complete
Boolean function unit (Function, two Bool Parameters, Block, one
Boolean Operation under Return), policy granting the demo principal
`ReplaceEntityVersion`. Variants differ in workspace (hence every
derived identity), base opcode (a: And, b: Or), and operand order.
Committed fixtures: `bench/sley2/fixtures/i4b_demo_v1.json`
(sha256 `28a65024...25cbfb`), refresh/verify with
`scripts/generate_i4b_demo_fixtures.py [--check]`.

Agent-input discipline: the harness owns the exchange bytes (import +
session open). The agent (`bench/sley2/entity_read_edit_demo.py`)
receives only identities plus the transacting callable and learns
every body through live reads (`refs.list/resolve`, `revision.read`,
306/307). The recorded method transcript proves confinement to
`ARM_AFFORDANCES`; no denied method moves.

## Evidence

* Receipt: `/home/greyforge/.local/state/sley2/at-mw-02/i4b-demonstrations-receipt.json`
  (sha256 `4b22a930...912ff2` at authoring; per-run nonces/expiry
  change digests across runs). Freezes per case: method sequence,
  before/after field values, response ObjectId, candidate/result ids,
  14-phase VALID decision, cross-replay outcomes.
* Gate: `uv run --project oracle/scb1 --frozen python3
  scripts/check_i4b_edit_demos.py --sley target/debug/sley`
  → `PASS` (4/4 cases decision 1, failed_phase null, zero problems).

Results at authoring:

| case | base | edit | decision |
|---|---|---|---|
| a/expression | And(p0,p1) | Or(p0,p1) | VALID, 14 phases |
| a/signature | And(p0,p1) | And(p1,p1) | VALID, 14 phases |
| b/expression | Or(p1,p0) | And(p1,p0) | VALID, 14 phases |
| b/signature | Or(p1,p0) | Or(p0,p0) | VALID, 14 phases |

Independent judgment (separate logic in the checker, not the demo
derivation): fresh live re-read reproduces the frozen before-body byte
for byte; after differs in exactly the intended field; the recorded
stored candidate re-validates on a fresh session; each variant's
candidate replayed against the other variant fails (non-transferable,
no hardcoded substitute).

## Process note (greyarch environment)

A bare scoped-looking `cargo fmt -- <file>` invocation normalized
unrelated files workspace-wide under this machine's rustfmt, producing
semantics-free reflow churn in five files; the churn was identified via
`git diff` review and fully reverted before commit. The lane diff
contains only I4b files. Lesson: verify `git status` after any fmt
invocation here, and never commit reflows outside the slice.

## Not claimed

* Nabu/Bounded-context re-review of AT-MW-02 (the remaining RESUME.md
  item-0 obligation) — requested as the next step, not done here.
* Any main-branch integration or public-mirror action.
* The historical EC1a mapping (unchanged; see
  AT_MW_02_CONSUMER_ACCEPTANCE_CLARIFICATION.md).
* Full-GA or M3 completion of any kind.
