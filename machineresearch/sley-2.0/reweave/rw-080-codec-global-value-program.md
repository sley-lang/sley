# RW-080 §1.1 program slice 26: bounded GlobalValue program codec — provisional C0 construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice constructs a
paired whole-program codec for entity kind 10. It is development evidence, not
accepted runtime authority. RW-080 remains BLOCKED and R2 remains NOT_READY;
independent review and acceptance debt are unchanged.

## 1. Bounded profile

The decoder accepts a complete canonical stored `EntityObject`: SCB1 envelope,
contract 200, epoch `[9; 32]`, object digest, two-field outer record, exact
32-byte entity identity and a GlobalValue body. Its admitted body profile is:

- kind-10 union tag and three ordered field frames;
- `TypeExpr::Unit` as the exact value type;
- one arbitrary exact 32-byte initializer identity; and
- `Visibility::Private` as the exact visibility.

The successful value is `(entity_id, canonical_body)`. The paired encoder
requires that same 44-byte body witness, checks the entity width, reconstructs
the exact contract-200 object preimage, invokes the frozen RHW1 row for its
digest, and returns byte-identical stored bytes. The checker skips only the
initializer's 32 dynamic bytes while comparing every union, record, field,
length, type and visibility byte.

Other valid GlobalValue type or visibility choices are outside this bounded
profile and return `SSMC_RESERVED_FIELD_PRESENT`. Encoder entity widths retain
the strict fixed-width diagnostics: 31 bytes returns `SCB_LENGTH_OVERFLOW`.

## 2. Shared construction

The Workspace slice's whole-program fixed-body decode and encode roots are now
reused for any already validated body witness. The fixed-template comparator
is exposed inside the test construction module, and the fixed-body witness
composer accepts explicit payload and body lengths. This removes duplicated
envelope, outer-record, hash and rebuild graphs while retaining separately
admitted images and exact native bytes.

This slice does not add kind 10 to the aggregate supported dispatcher. The
aggregate DependencyBinding decode remains close to the protected value-unit
ceiling, so another shared-router or image-size optimization must precede that
addition. No temporary limit, host semantic codec, unchecked body path or
charging change was introduced.

## 3. Evidence and resources

The native reference builds a 157-byte GlobalValue with nonuniform entity and
initializer identities. The Sley decoder returns its exact entity and 44-byte
body; the separately admitted Sley encoder recreates all 157 bytes. Valid
native fixtures using `Bool` or `Exported` exercise the bounded-profile scope
refusal, and a short encoder entity exercises fixed-width precedence.

Under the unchanged protected limits:

| Path | Stored bytes | Fuel | Instructions | Peak value units |
|---|---:|---:|---:|---:|
| GlobalValue decode, Unit/Private | 157 | 23,729 | 2,886 | 529,397 |
| GlobalValue encode, Unit/Private | 157 | 16,681 | 1,767 | 544,994 |
| GlobalValue decode, Bool/Private refusal | 157 | 23,492 | 2,800 | 527,844 |

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

Local strict review checks exact native byte identity, the single dynamic
range, valid out-of-profile bodies, entity width, adapter confinement and
unchanged protected limits. Independent review is unavailable in this
session, so no gate, ledger, acceptance verdict or runtime-authority state is
promoted.
