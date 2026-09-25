# RW-080 §1.1 program slices 32–37: remaining entity program codecs — provisional C0 construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). These slices construct
paired whole-program codecs for entity kinds 4, 5, 7, 8, 13 and 14. They are
development evidence, not accepted runtime authority. RW-080 remains BLOCKED
and R2 remains NOT_READY; independent review and acceptance debt are
unchanged.

## 1. Bounded profiles

Every decoder accepts a complete canonical stored `EntityObject`: SCB1
envelope, contract 200, epoch `[9; 32]`, object digest, two-field outer record,
exact 32-byte entity identity and the stated canonical body. Every paired
encoder requires the same body witness and entity width, derives the object
digest through RHW1 and recreates the native object byte for byte.

The six profiles are:

| Kind | Admitted body profile | Arbitrary identity fields |
|---:|---|---|
| 4 TypeDef | no type parameters, empty record form, no invariants, Private | none |
| 5 Function | no parameters/effects/blocks/contracts, Unit result, Private | entry block |
| 7 Block | no parameters/operations, unreachable trap without payload, Required | owner function |
| 8 Operation | ordinal 0, ConstantRef opcode, no operands/results, no immediate | owner block |
| 13 Contract | Precondition, no bindings or resource limits | target and predicate |
| 14 TestCase | no inputs/replay/observations, Unit result, six zero limits | target |

The tests build each witness and each out-of-profile case through the native
`sley-mutate` object codec. Supported bodies return `(entity_id,
canonical_body)` and encode back to the exact original bytes. Valid alternate
forms, collection contents, enum variants and nonzero scalar values return
`SSMC_RESERVED_FIELD_PRESENT`. Each encoder rejects a 31-byte entity with
`SCB_LENGTH_OVERFLOW`.

These slices complete standalone paired-codec coverage for all 18 entity
kinds. They do not add kinds to the aggregate dispatcher. The aggregate
DependencyBinding decode remains close to the protected value-unit ceiling,
so a compact multi-profile router remains a prerequisite. No limit, host
semantic codec, unchecked body path or charging rule changed.

## 2. Evidence and resources

Under the unchanged protected limits:

| Path | Stored bytes | Fuel | Instructions | Peak value units |
|---|---:|---:|---:|---:|
| TypeDef decode | 130 | 18,707 | 2,487 | 378,623 |
| TypeDef encode | 130 | 13,981 | 1,557 | 376,805 |
| TypeDef one-invariant refusal | 163 | 24,437 | 2,831 | 565,811 |
| Function decode | 172 | 27,160 | 3,340 | 638,086 |
| Function encode | 172 | 18,796 | 2,112 | 656,324 |
| Function one-block refusal | 206 | 32,723 | 3,592 | 918,236 |
| Block decode | 171 | 27,011 | 3,322 | 630,697 |
| Block encode | 171 | 18,655 | 2,089 | 648,594 |
| Block one-operation refusal | 205 | 32,532 | 3,575 | 908,588 |
| Operation decode | 166 | 25,824 | 3,164 | 592,974 |
| Operation encode | 166 | 17,950 | 1,974 | 610,604 |
| Operation alternate-immediate refusal | 167 | 25,227 | 2,903 | 593,991 |
| Contract decode | 190 | 30,172 | 3,476 | 780,883 |
| Contract encode | 190 | 20,234 | 2,118 | 795,777 |
| Contract resource-limits refusal | 212 | 33,895 | 3,698 | 977,614 |
| TestCase decode | 195 | 32,433 | 4,038 | 833,633 |
| TestCase encode | 195 | 22,039 | 2,641 | 846,258 |
| TestCase one-input refusal | 206 | 32,723 | 3,592 | 918,236 |

The protected limits remain 100,000 instructions, 1,000,000 fuel, 1,000,000
value units and 100,000 output units.

## 3. Validation and review state

Validation for these slices:

- `cargo test -p sley-vm --test rw080_codec_program_outer`
- `cargo clippy -p sley-vm --tests -- -D warnings`
- `cargo test -p sley-vm --test rw080_codec_uvar`
- `cargo test -p sley-vm --test rw080_codec_envelope`
- `cargo test -p sley-vm --test rw080_codec_scaffold`
- `cargo test -p sley-vm --test rw080_checker_scaffold`
- `cargo test -p sley-vm --test rw080_lower_scaffold`
- `cargo test -p sley-scb1 --locked --lib`
- `cargo test -p sley-mutate --locked --lib`
- `cargo fmt --all -- --check`
- `python3 scripts/build_anti_goal_conformance.py --check`
- `python3 -m json.tool evidence/reweave/sh2-work-items.json`
- `git diff --check`

Local strict review checks the exact native bodies, every dynamic range,
representative valid out-of-profile forms, entity width, adapter confinement
and unchanged limits. Independent review is unavailable, so no gate, ledger,
acceptance verdict or runtime-authority state is promoted.
