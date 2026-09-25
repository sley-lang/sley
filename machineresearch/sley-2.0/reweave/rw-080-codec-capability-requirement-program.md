# RW-080 §1.1 program slice 31: bounded CapabilityRequirement program codec — provisional C0 construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice constructs a
paired whole-program codec for entity kind 12. It is development evidence, not
accepted runtime authority. RW-080 remains BLOCKED and R2 remains NOT_READY;
independent review and acceptance debt are unchanged.

## 1. Bounded profile

The decoder accepts a complete canonical stored `EntityObject`: SCB1 envelope,
contract 200, epoch `[9; 32]`, object digest, two-field outer record, exact
32-byte entity identity and a CapabilityRequirement body. Its admitted body
profile is:

- kind-12 union tag and three ordered field frames;
- one arbitrary exact 32-byte effect identity;
- an empty allowed-scope vector; and
- an empty constraint-contract set.

The successful value is `(entity_id, canonical_body)`. The paired encoder
requires the same 43-byte body witness and entity width, derives the object
digest through RHW1 and recreates the exact 156-byte native object. The body
checker skips only the effect identity's 32 bytes and compares all other
framing and semantic bytes.

Valid one-scope and one-contract bodies are outside this bounded profile and
return `SSMC_RESERVED_FIELD_PRESENT`. A 31-byte encoder entity returns
`SCB_LENGTH_OVERFLOW`.

This slice does not add kind 12 to the aggregate supported dispatcher. The
aggregate DependencyBinding decode remains close to the protected value-unit
ceiling, so a compact multi-profile router remains a prerequisite. No limit,
host semantic codec, unchecked body path or charging rule changed.

## 2. Evidence and resources

Under the unchanged protected limits:

| Path | Stored bytes | Fuel | Instructions | Peak value units |
|---|---:|---:|---:|---:|
| CapabilityRequirement decode, empty collections | 156 | 23,502 | 2,856 | 522,686 |
| CapabilityRequirement encode, empty collections | 156 | 16,540 | 1,744 | 537,924 |
| CapabilityRequirement decode, one scope refusal | 166 | 25,010 | 2,882 | 586,769 |
| CapabilityRequirement decode, one contract refusal | 189 | 29,377 | 3,269 | 767,077 |

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

Local strict review checks the exact effect bytes, the dynamic range, valid
nonempty collection bodies, entity width, adapter confinement and unchanged
limits. Independent review is unavailable, so no gate, ledger, acceptance
verdict or runtime-authority state is promoted.
