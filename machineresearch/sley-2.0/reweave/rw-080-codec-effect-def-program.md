# RW-080 §1.1 program slice 28: bounded EffectDef program codec — provisional C0 construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice constructs a
paired whole-program codec for entity kind 11. It is development evidence, not
accepted runtime authority. RW-080 remains BLOCKED and R2 remains NOT_READY;
independent review and acceptance debt are unchanged.

## 1. Bounded profile

The decoder accepts a complete canonical stored `EntityObject`: SCB1 envelope,
contract 200, epoch `[9; 32]`, object digest, two-field outer record, exact
32-byte entity identity and an EffectDef body. Its admitted body profile is:

- kind-11 union tag and six ordered field frames;
- `EffectKind::AdapterCall`;
- `TypeExpr::Unit` for scope, request, response and failure; and
- `Visibility::Private`.

The 25-byte body has no dynamic body range. The checker compares every union,
record, field, length, effect-kind, type and visibility byte. The successful
value is `(entity_id, canonical_body)`. The paired encoder validates the same
body witness and entity width, derives the object digest through RHW1 and
recreates the exact native stored bytes.

Valid bodies using `StdoutWrite`, a `Bool` request type or exported visibility
are outside this bounded profile and return `SSMC_RESERVED_FIELD_PRESENT`. A
31-byte encoder entity returns `SCB_LENGTH_OVERFLOW`.

This slice does not add kind 11 to the aggregate supported dispatcher. The
aggregate DependencyBinding decode remains close to the protected value-unit
ceiling, so a compact multi-profile router remains a prerequisite. No limit,
host semantic codec, unchecked body path or charging rule changed.

## 2. Evidence and resources

The native reference builds a 138-byte EffectDef. The Sley decoder returns its
exact entity and 25-byte body; the separately admitted Sley encoder recreates
all 138 bytes.

Under the unchanged protected limits:

| Path | Stored bytes | Fuel | Instructions | Peak value units |
|---|---:|---:|---:|---:|
| EffectDef decode, AdapterCall/Unit/Private | 138 | 20,575 | 2,735 | 420,639 |
| EffectDef encode, AdapterCall/Unit/Private | 138 | 15,109 | 1,741 | 425,445 |
| EffectDef decode, StdoutWrite refusal | 138 | 19,896 | 2,485 | 416,199 |

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

Local strict review checks the exact fixed body, valid out-of-profile values,
entity width, shared-harness behavior, adapter confinement and unchanged
limits. Independent review is unavailable, so no gate, ledger, acceptance
verdict or runtime-authority state is promoted.
