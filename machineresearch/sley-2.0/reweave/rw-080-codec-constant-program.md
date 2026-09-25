# RW-080 §1.1 program slice 29: bounded Constant program codec — provisional C0 construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice constructs a
paired whole-program codec for entity kind 9. It is development evidence, not
accepted runtime authority. RW-080 remains BLOCKED and R2 remains NOT_READY;
independent review and acceptance debt are unchanged.

## 1. Bounded profile

The decoder accepts a complete canonical stored `EntityObject`: SCB1 envelope,
contract 200, epoch `[9; 32]`, object digest, two-field outer record, exact
32-byte entity identity and a Constant body. Its admitted body is the exact
kind-9 record containing a `ConstValue` whose declared type and data are both
unit.

The 14-byte body has no dynamic body range. The checker compares every entity
union, record, field, nested `ConstValue`, type and data byte. The successful
value is `(entity_id, canonical_body)`. The paired encoder validates the same
body witness and entity width, derives the object digest through RHW1 and
recreates the exact 127-byte native object.

Valid boolean constants are outside this bounded profile and return
`SSMC_RESERVED_FIELD_PRESENT`. A 31-byte encoder entity returns
`SCB_LENGTH_OVERFLOW`.

This slice does not add kind 9 to the aggregate supported dispatcher. The
aggregate DependencyBinding decode remains close to the protected value-unit
ceiling, so a compact multi-profile router remains a prerequisite. No limit,
host semantic codec, unchecked body path or charging rule changed.

## 2. Evidence and resources

Under the unchanged protected limits:

| Path | Stored bytes | Fuel | Instructions | Peak value units |
|---|---:|---:|---:|---:|
| Constant decode, unit | 127 | 18,052 | 2,401 | 364,138 |
| Constant encode, unit | 127 | 13,558 | 1,488 | 359,291 |
| Constant decode, boolean refusal | 128 | 17,726 | 2,232 | 365,087 |

The protected limits remain 100,000 instructions, 1,000,000 fuel, 1,000,000
value units and 100,000 output units.

## 3. Validation and review state

Validation for this slice:

- `cargo test -p sley-vm --test rw080_codec_program_outer`
- `cargo clippy -p sley-vm --test rw080_codec_program_outer -- -D warnings`
- `cargo test -p sley-vm --test rw080_codec_uvar`
- `cargo test -p sley-vm --test rw080_codec_envelope`
- `cargo test -p sley-vm --test rw080_codec_scaffold`
- `cargo test -p sley-vm --test rw080_checker_scaffold`
- `cargo test -p sley-vm --test rw080_lower_scaffold`
- `cargo test -p sley-scb1 --locked --lib`
- `cargo test -p sley-mutate --locked --lib object`
- `cargo fmt --all -- --check`
- `python3 scripts/build_anti_goal_conformance.py --check`
- `python3 -m json.tool evidence/reweave/sh2-work-items.json`
- `git diff --check`

Local strict review checks the exact nested value bytes, valid out-of-profile
constants, entity width, shared-harness behavior, adapter confinement and
unchanged limits. Independent review is unavailable, so no gate, ledger,
acceptance verdict or runtime-authority state is promoted.
