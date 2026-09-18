# RW-080 §1.1 program slice 30: bounded AdapterImport program codec — provisional C0 construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice constructs a
paired whole-program codec for entity kind 15. It is development evidence, not
accepted runtime authority. RW-080 remains BLOCKED and R2 remains NOT_READY;
independent review and acceptance debt are unchanged.

## 1. Bounded profile

The decoder accepts a complete canonical stored `EntityObject`: SCB1 envelope,
contract 200, epoch `[9; 32]`, object digest, two-field outer record, exact
32-byte entity identity and an AdapterImport body. Its admitted body profile
is:

- kind-15 union tag and six ordered field frames;
- one arbitrary exact 32-byte adapter identity;
- ABI version 1;
- `TypeExpr::Unit` for request, response and failure; and
- an empty effect set.

The successful value is `(entity_id, canonical_body)`. The paired encoder
requires the same 55-byte body witness and entity width, derives the object
digest through RHW1 and recreates the exact 168-byte native object. The body
checker skips only the adapter identity's 32 bytes and compares all other
framing and semantic bytes.

Valid ABI version 2, boolean request type and one-effect bodies are outside
this bounded profile and return `SSMC_RESERVED_FIELD_PRESENT`. A 31-byte
encoder entity returns `SCB_LENGTH_OVERFLOW`.

This slice does not add kind 15 to the aggregate supported dispatcher. The
aggregate DependencyBinding decode remains close to the protected value-unit
ceiling, so a compact multi-profile router remains a prerequisite. No limit,
host semantic codec, unchecked body path or charging rule changed.

## 2. Evidence and resources

Under the unchanged protected limits:

| Path | Stored bytes | Fuel | Instructions | Peak value units |
|---|---:|---:|---:|---:|
| AdapterImport decode, ABI1/Unit/empty | 168 | 26,304 | 3,228 | 607,886 |
| AdapterImport encode, ABI1/Unit/empty | 168 | 18,232 | 2,020 | 625,668 |
| AdapterImport decode, ABI2 refusal | 168 | 25,765 | 3,030 | 604,332 |
| AdapterImport decode, one-effect refusal | 201 | 31,721 | 3,481 | 875,273 |

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

Local strict review checks exact adapter bytes, the single dynamic range,
valid out-of-profile ABI/type/effect bodies, entity width, adapter confinement
and unchanged limits. Independent review is unavailable, so no gate, ledger,
acceptance verdict or runtime-authority state is promoted.
