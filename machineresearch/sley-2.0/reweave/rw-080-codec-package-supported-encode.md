# RW-080 §1.1 program slice 23: Package supported-encode dispatch — provisional C0 construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice adds entity kind
2 to the supported whole-program semantic encode dispatcher. It is development
evidence, not accepted runtime authority. RW-080 remains BLOCKED and R2
remains NOT_READY; independent review and acceptance debt are unchanged.

## 1. Paired bounded value

`encode_supported_program(declared_kind: UInt64, value: SupportedValue,
unit: Unit)` now accepts the same five-arm sum emitted by the slice-22 decoder.
The Package arm is the existing four-field tagged value:

```text
(kind = 2, entity_id, canonical_body, empty_reserved)
```

The body must be the exact 77-byte canonical empty-set Package profile, and
the reserved slot must be empty. This compact witness keeps the established
sum type unchanged and avoids carrying a second body copy. The dispatcher
still checks that the declared kind equals the tuple kind before any Package
work begins.

The encoder rejects a nonempty reserved slot or a noncanonical body with
`SSMC_RESERVED_FIELD_PRESENT`. Entity identity retains the exact fixed-width
codes: a short identity returns `SCB_LENGTH_OVERFLOW` and a long identity
returns `SCB_TRAILING_BYTES`. Declared-kind/arm mismatch and known/unknown kind
classification remain unchanged.

## 2. Construction

The admitted image reuses the slice-22 masked body checker. After the witness
passes, a Package-specific composer:

1. validates the entity identity as exactly 32 bytes;
2. assembles `sley2.object.v1`, the fixed program-envelope prefix, entity,
   fixed outer tail and validated body for hashing;
3. invokes RHW1 for the object digest; and
4. assembles the exact stored object with that digest.

The specialized composer operates on the already validated body witness. It
avoids the transient allocations of calling the general outer and envelope
graphs with a 77-byte opaque body inside the larger supported image. The first
general composition exceeded the protected value-unit ceiling; the landed
composer emits the same bytes with bounded live state. Namespace and
PolicyBinding still use the shared entity-set encoder, EntryPoint retains its
existing program encoder, and DependencyBinding retains its compact fixed
encoder.

No host semantic encoder, unchecked body path, charging change or larger
execution limit was introduced. Imports remain the frozen HOST_ABI_V2 B2V1,
PSH1, V2B1 and RHW1 rows.

## 3. Evidence and resources

An independently constructed kind-2 semantic value emits the exact native
190-byte stored object. A separately admitted supported decoder feeds its
kind-2 value directly to the encoder and recovers byte-identical storage. The
rejection matrix covers Package/Namespace declaration swaps, a nonempty
reserved slot, a same-length noncanonical dependency count and a short entity
identity.

Under the unchanged protected limits:

| Kind | Fuel | Instructions | Peak value units |
|---|---:|---:|---:|
| EntryPoint 16 | 22,507 | 2,848 | 928,496 |
| Namespace 3 | 26,242 | 3,386 | 994,486 |
| PolicyBinding 17, empty requirements | 25,714 | 3,320 | 966,144 |
| DependencyBinding 18 | 3,757 | 1,031 | 397,203 |
| Package 2, empty sets | 20,283 | 2,139 | 936,965 |

The existing one-requirement PolicyBinding capacity fixture still reaches
`ResourceLimit(ValueUnits)` first at 30,117 fuel, 3,751 instructions and
997,719 recorded peak units. Namespace is the tight successful encode case
with 5,514 units of headroom. No protected limit changed.

## 4. Validation and review state

Validation for this slice:

- `cargo test -p sley-vm --test rw080_codec_program_outer` (61 tests)
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

Local strict review checks exact native bytes, paired decode-to-encode value
identity, reserved-slot and body validation, entity-width diagnostics,
kind/arm mismatch, capacity behavior, adapter confinement and unchanged
protected limits. Independent review is unavailable in this session, so no
gate, acceptance verdict or runtime-authority state is promoted.
