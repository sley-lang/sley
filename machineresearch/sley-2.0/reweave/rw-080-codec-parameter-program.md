# RW-080 §1.1 program slice 27: bounded Parameter program codec — provisional C0 construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice constructs a
paired whole-program codec for entity kind 6. It is development evidence, not
accepted runtime authority. RW-080 remains BLOCKED and R2 remains NOT_READY;
independent review and acceptance debt are unchanged.

## 1. Bounded profile

The decoder accepts a complete canonical stored `EntityObject`: SCB1 envelope,
contract 200, epoch `[9; 32]`, object digest, two-field outer record, exact
32-byte entity identity and a Parameter body. Its admitted body profile is:

- kind-6 union tag and four ordered field frames;
- one arbitrary exact 32-byte owner identity;
- `ParameterRole::Function`;
- ordinal zero; and
- `TypeExpr::Unit`.

The successful value is `(entity_id, canonical_body)`. The paired encoder
requires that same 47-byte body witness, checks the entity width, reconstructs
the exact contract-200 object preimage, invokes RHW1 for the digest, and
returns byte-identical stored bytes. The checker skips only the owner's 32
dynamic bytes while comparing every union, record, field, length, role,
ordinal and type byte.

Valid bodies using block role, ordinal one or `Bool` type are outside this
bounded profile and return `SSMC_RESERVED_FIELD_PRESENT`. A 31-byte encoder
entity returns `SCB_LENGTH_OVERFLOW`.

## 2. Shared construction

The fixed-body test harness now assembles the common decode and encode images
from a profile-specific body checker and exact payload/body lengths. Workspace
uses the same generic harness and retains its prior measurements and behavior.
The old Workspace-only witness wrapper was removed; all single fixed profiles
now use one explicit-length witness constructor.

This slice does not add kind 6 to the aggregate supported dispatcher. The
aggregate DependencyBinding decode remains close to the protected value-unit
ceiling, so a compact multi-profile router remains a prerequisite. No limit,
host semantic codec, unchecked body path or charging rule changed.

## 3. Evidence and resources

The native reference builds a 160-byte Parameter with nonuniform entity and
owner identities. The Sley decoder returns its exact entity and 47-byte body;
the separately admitted Sley encoder recreates all 160 bytes.

Under the unchanged protected limits:

| Path | Stored bytes | Fuel | Instructions | Peak value units |
|---|---:|---:|---:|---:|
| Parameter decode, Function/0/Unit | 160 | 24,462 | 2,984 | 550,134 |
| Parameter encode, Function/0/Unit | 160 | 17,104 | 1,836 | 566,468 |
| Parameter decode, Block refusal | 160 | 24,203 | 2,890 | 548,420 |

The protected limits remain 100,000 instructions, 1,000,000 fuel, 1,000,000
value units and 100,000 output units.

## 4. Validation and review state

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

Local strict review checks exact native bytes, the owner dynamic range,
out-of-profile role/ordinal/type values, entity width, shared-harness behavior,
adapter confinement and unchanged limits. Independent review is unavailable,
so no gate, ledger, acceptance verdict or runtime-authority state is promoted.
