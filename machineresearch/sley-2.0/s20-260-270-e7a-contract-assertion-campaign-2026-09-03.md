# S20-260/S20-270 slice E7a: contract assertion execution

Date: 2026-09-03. Integrator: Claude (front line). Council reviews pending.

## Why this slice exists

`EPOCH_MIGRATION_POLICY_V1.md` section 1 requires the profile-versus-epoch
question to be answered before an epoch is proposed. Answering it for the
epoch-2 agenda (revision 2 of that contract) showed that one of the five E7
opcodes needs nothing that epoch 1 lacks:

- `contract_assert` (144) is in the frozen epoch-1 opcode table;
- `CONTRACT_TEST_PROFILE_V1.md` section 2 accepts it statically under epoch 1
  and assigns predicate execution to S20-270 and report evidence to S20-290;
- the extended profile carries its own `lowering_profile` identity in the
  cache-key preimage, so no existing lowered byte or cache key can change.

The operation was therefore an owned, specified, unimplemented behaviour, not
a blocked one. This slice implements it.

## What landed

- **Judgment** (`crates/sley-vm/src/extended.rs`). The profile re-derives every
  rule rather than trusting the checker that already passed: the immediate
  resolves in the request's Contract inventory; the kind is one of the three
  epoch-1 supported kinds and carries no resource ceiling; the target is the
  enclosing function; the predicate is a different function with no effects and
  no contracts, zero type parameters, and result exactly `Bool`; the operands
  equal the predicate's parameters in order; the single result is exactly
  `Result<Unit, BuiltinFailure(ContractViolation)>`.
- **Execution** (`crates/sley-vm/src/execute.rs`). The predicate enters as an
  ordinary E6 frame under the same call-depth ceiling and the same shared fuel,
  instruction, value-unit, and cell budgets. The caller wraps the answer:
  `true` yields `Ok(Unit)`, `false` yields
  `Err(BuiltinFailure(ContractViolation, 1))`, the only code the S20-210
  checker admits for that family. A violated contract is a value, never a trap.
- **Callee closure** (`crates/sley-vm/src/lower.rs`). The transitive lowering
  closure now follows contract predicates as well as `call_direct` callees.
- **Inventories.** `LoweringInput` and `LoweringContext` carry the Contract
  inventory; the protocol server passes the complete root's contracts through.

## The owner boundary that did not move

`contract_assert` stays outside S20-360 phase 7 operation analysis. Its static
typing belongs to the S20-240 checker at phase 10, which reports the exact
`CONTRACT_ASSERT_TYPE` diagnosis. Admitting it to phase 7 would have replaced
that owner's diagnosis with a lowering code, so the candidate validator keeps
its five excluded opcodes and the VM judges the operation again when it lowers,
after validation has passed. The regression that revealed this
(`resource_and_unsupported_operation_analysis_fail_closed`) is why the boundary
is now written down in three places.

## Evidence

- Two crate tests: the verdict test (both arms, the exact five-fuel cost, and
  128 identical repetitions) and a seven-case rejection matrix.
- Two conformance vectors, `contract-assert-holds` and
  `contract-assert-violated`, emitted from the crate and drift-gated.
- The independent Python oracle now refuses any opcode outside the landed
  families, so a lowerer that quietly admitted an excluded E7 opcode would fail
  there; it re-derives both new vectors' containers and cache keys.
- One new `vm_canonical_inputs` fuzz lane (eight extended families, 769 seeds).
- Tier 1 `make quick` and Tier 2 `core`, `conformance`, `adversarial`,
  `fuzz-smoke`, plus the VM persistent smoke.

## What this slice does not claim

Not E7 beyond 144: `test_observe` needs a schema epoch, and effects, adapters,
and capability narrowing need S20-280 full and S20-380 full. Not report
persistence, not test execution, not GA. The three Council reviews are pending,
so the contract is not frozen and the package is not complete.
