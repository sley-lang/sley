# RW-080 §1.1 program slice 13: DependencyBinding program encode composition — provisional C0 construction record

Status: PROVISIONAL (2026-09-17, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice adds kind 18 to
the supported whole-program Sley encoder. It is development evidence, not
accepted runtime authority. RW-080 remains BLOCKED and R2 remains NOT_READY;
independent review and acceptance debt are unchanged.

## 1. Scope and semantic arm

`encode_supported_program(18, value, unit)` accepts the additive outer `Err`
arm introduced by slice 12:

```text
(entity_id, dependency_root, external_package, local_namespace)
```

All four values are exact 32-byte `Bytes`. The established outer `Ok` arm
continues to contain the unchanged `Result<EntryPoint, Namespace>` value, so
the two earlier kind identities and their decode-to-encode round trips remain
stable. The enclosing codec `Result<Bytes, Bytes>` still distinguishes a valid
DependencyBinding value from a refusal.

Kind 18 requires the DependencyBinding arm. Supplying an EntryPoint or
Namespace value for kind 18, or a DependencyBinding value for kind 3 or 16,
returns `SSMC_RESERVED_FIELD_PRESENT`. Known unsupported and unknown declared
kinds retain the slice-9 classification rules.

## 2. Direct fixed-shape construction

The kind-18 program encoder validates dependency root, external package, local
namespace, then entity id. That order matches composition of the strict body
encoder followed by the outer encoder: a malformed body field wins over a
malformed entity id. Short values return `SCB_LENGTH_OVERFLOW`; long values
return `SCB_TRAILING_BYTES`.

After validation, a small Sley octet getter reads exact vector indices and
traps only on the unreachable missing-index case. The caller invokes it inside
one block for each dynamic octet, then uses `VectorNew` once for the exact
202-byte digest preimage and once for the exact 219-byte stored object. RHW1
hashes the Sley-constructed preimage. B2V1 and V2B1 perform only the frozen
representation conversions.

This shape is required by the protected value-unit budget. Repeated PSH1
accumulator growth charges every intermediate vector and measured 1,868,596
units under a temporary diagnostic ceiling. Carrying every extracted scalar
through separate blocks made the derived image itself exceed the ceiling. The
accepted construction allocates each large vector once and measures 416,321
units without changing the one-million-unit limit, charging rules, host ABI,
or hash semantics.

The EntryPoint and Namespace program wrappers now share two ordinary stages:
body-to-outer payload composition and payload-to-envelope composition. This
removes duplicated envelope construction while preserving their outputs,
errors, and function identities.

## 3. Evidence and resources

One admitted supported-encode package emits native canonical stored bytes for
all three supported kinds. A second admitted decode package feeds each decoded
semantic arm directly to the encoder and recovers the exact original stored
bytes. Rejection vectors cover arm mismatches in both directions, malformed
body fields, short and long entity ids, and body-before-entity precedence with
the exact fixed-width errors.

Under the unchanged limits (100,000 instructions, 1,000,000 fuel, 1,000,000
value units, 100,000 output units):

| Kind | Stored bytes | Fuel | Instructions | Peak value units |
|---|---:|---:|---:|---:|
| EntryPoint 16 | 153 | 21,949 | 2,743 | 945,714 |
| Namespace 3, no parent or members | 123 | 16,292 | 2,156 | 629,908 |
| DependencyBinding 18 | 219 | 3,757 | 1,031 | 416,321 |

Targeted validation:

- `cargo test -p sley-vm --test rw080_codec_program_outer` (42 tests)
- `cargo clippy -p sley-vm --test rw080_codec_program_outer -- -D warnings`
- `cargo test -p sley-vm --test rw080_codec_uvar`
- `cargo test -p sley-vm --test rw080_codec_envelope`
- `cargo test -p sley-vm --test rw080_codec_scaffold`
- `cargo test -p sley-scb1 --locked --lib`
- `cargo test -p sley-mutate --locked --lib object`
- `cargo fmt --all -- --check`
- `python3 scripts/build_anti_goal_conformance.py --check`
- `git diff --check`

Local strict review checked canonical byte order, the exact domain and envelope
preimage, digest placement, validation precedence, nested sum identities,
fail-closed dispatch, adapter confinement, and frozen resource limits.
Independent review was unavailable; no gate, acceptance verdict, or runtime
authority state is promoted.

The supported encode and decode entries now cover kinds 3, 16, and 18. The
next codec work is another bounded entity-body pair, while label/NFC,
fingerprint verification, remaining kinds, and the schema-bearing canonical
entry remain explicit dependencies.
