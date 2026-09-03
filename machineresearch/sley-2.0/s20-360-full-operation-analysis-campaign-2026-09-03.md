# S20-360 full operation analysis campaign (2026-09-03)

Package: the full operation analysis of S20-360 (candidate validation),
dependency S20-260/S20-270 full, phase M3. Owner of record Merlin; executed by
the integrator because every Council lane was unavailable (ADR-0026). No
commit, execution, capability-budget, or runtime authority is added: the
validator still only judges.

## Contract

- `docs/adr/ADR-0044-candidate-operation-analysis-through-the-vm-owner.md`,
  with the VM judgment surface in `docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md`
  revision 8 (section 3.1) and the revised subset in
  `docs/spec/CANDIDATE_RESULT_V1.md` section 9.
- Closeout addendum in `docs/audits/S20_360_CANDIDATE_VALIDATION_CLOSEOUT.md`;
  summary section `s20_360_candidate_validation`; work package row updated;
  frontier checker pins the new status, subset, judgment owner, and excluded
  opcodes.

## Mechanics

| Surface | Change |
|---|---|
| `crates/sley-vm/src/lower.rs` | `judge_function_operations` returns `OperationJudgment { operations, work }`: judgment only, no bytecode, no cache key, no callee lowering, no execution, and no refusal of type parameters, effects, or contracts (other owners' concerns) |
| `crates/sley-policy/src/candidate_program.rs` | `operation_analysis_supported` now refuses only the five E7 opcodes (144, 145, 160, 161, 162); `operation_count` feeds the work total |
| `crates/sley-policy/src/candidate_validation.rs` | phase 7 judges every unit after its S20-220 report, charges the judgment work, and records the judged-operation count and work in the phase evidence; `operation_failure` maps `VM_LOWER_*` onto the phase's decision; phase 12 adds the operation count to the graph work |
| Tests | `analyzable_operations_validate_and_signature_mismatches_fail_at_phase_seven` (a `constant_ref` program reaches `VALID`; a declared `Bool` result over a `Unit` constant fails phase 7 with `VM_LOWER_SIGNATURE_MISMATCH`), and the E7 matrix in `resource_and_unsupported_operation_analysis_fail_closed` |

## Result

- A candidate whose functions contain E1 through E6 operations can now reach
  `VALID`; before this package any semantic `Operation` entity refused at
  phase 12.
- The five E7 opcodes are each refused by the owner of their own phase, with
  exact symbols: 144 phase 10 `CONTRACT_ASSERT_TYPE`; 145 phase 11
  `TEST_PLAN_OBSERVATION_UNSUPPORTED`; 160 phase 8 `EFFECT_REQUEST_TYPE`; 161
  phase 8 `ADAPTER_INVOKE_TYPE`; 162 phase 8 `CAPABILITY_REQUIREMENT_TYPE`.
  The phase 12 guard is retained as defense in depth and is unreachable while
  those owners refuse first, which the matrix test records.
- Derived identity changes, all expected and regenerated: the valid fixture's
  candidate-result identity (phase 7 evidence gained two values), the
  transaction-receipt vectors, and the repository-exchange identities that
  embed a receipt. The pinned exchange identities in
  `scripts/check_repository_exchange_spec.py` were updated with them.
- `sley-policy` now depends on `sley-vm`; the supply-chain inventory records
  119 dependency relationships and the refreshed `Cargo.lock` digest.

## In-flight repair

Regenerating one derived document at a time cascaded through the evidence
chain (fixtures, conformance report, SBOM, provenance, register, dossier, and
the T54 scan that covers them). The repair is a `make evidence-refresh` target
that runs every builder in dependency order and reruns the supply-chain
generator last, because its T54 scan covers the documents the earlier steps
rewrite.

## Open questions for the Council

1. Ariadne: is phase 7 the right home for the opcode judgment, or should the
   result contract gain a decision that distinguishes an operation signature
   failure from a control-flow failure?
2. Nabu: `sley-policy` now depends on `sley-vm`; should the judgment instead
   be re-exported through `sley-check` so the policy crate keeps a single
   semantic dependency?
3. Vulcan: the phase 12 guard is now unreachable through every E7 opcode. Keep
   it as defense in depth, or is unreachable code a liability here?

## Validation

Landed at `9fb9ecc`. Tier 1 `make quick` passed at the commit. Tier 2 ran on
2026-09-03 at that commit:

| Gate | Result | Wall time | Evidence |
|---|---|---:|---|
| `make core` | exit 0 | 18 s | 996 tests passed, 0 failed |
| `make conformance` | exit 0 | 18 s | 19 oracle results PASS |
| `make adversarial` | exit 0 | 15 s | 597 tests passed, 0 failed |
| `make fuzz-smoke` | exit 0 | 1 s | 5 bounded smoke tests passed |
| `make candidate-result-persistent-fuzz-smoke` | exit 0 | 14 s | PASS |
| `make transaction-receipt-persistent-fuzz-smoke` | exit 0 | 12 s | 512 runs, PASS |
| `make vm-persistent-fuzz-smoke` | exit 0 | 10 s | 626 runs, PASS |

`make v1` was not run: this is a subsystem handoff, not a release boundary.
