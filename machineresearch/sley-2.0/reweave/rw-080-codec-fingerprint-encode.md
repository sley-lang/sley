# RW-080 §1.1 program slice 39: semantic-fingerprint outer encoding — provisional C0 construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice implements
canonical field-4 emission for an SSMC1 `EntityObject` in an admitted Sley
program. It is development evidence, not accepted runtime authority. RW-080
remains BLOCKED and R2 remains NOT_READY; independent review and acceptance
debt are unchanged.

## Construction and behavior

`fingerprint_outer_encode(entity, body, fingerprint, unit)` composes the
existing two-field outer encoder and extends its result without a semantic
host codec. It:

1. calls the existing Sley outer encoder, preserving entity/body failures
   ahead of later metadata validation;
2. converts the runtime fingerprint to `Vector<UInt8>` and requires exactly
   32 bytes;
3. constructs a new record by emitting count `3`, copying the two encoded
   fields after the old count byte, emitting canonical tag/length bytes
   `04 20`, and copying all 32 fingerprint bytes; and
4. converts only the finished Sley-built vector back to `Bytes`.

Short fingerprints return `SCB_LENGTH_OVERFLOW`; long fingerprints return
`SCB_TRAILING_BYTES`. Existing entity-width failures pass through unchanged
and win in cross-field fault cases.
Bridge capacity failures return `SCB_RESOURCE_LIMIT`. Checked loop-index
overflow also takes that resource path. All failures are typed values.

The image uses only the three-profile subset it needs: B2V1, PSH1 and V2B1.
The construction reuses the exact-width gate, constant-push chain, and block
parameter helpers already owned by the DependencyBinding encoder; those
helpers are now visible to sibling codec modules without changing their
behavior. Runtime-sized body and fingerprint copies execute in Sley loops.

Optional labels remain excluded. Supporting them requires UTF-8 validation,
the pinned Unicode 16.0 NFC tables, the 1,024-byte entity-label ceiling and
the native first-failure order. This slice does not weaken or approximate
that requirement.

## Native parity and measured limits

The positive corpus compares exact output bytes with
`sley_scb1::encode_record` for empty, 127-byte and 140-byte runtime bodies and
three distinct fingerprints. The largest passing case emits a 212-byte outer
record and uses 17,592 fuel, 2,291 instructions and 927,488 peak value units.
The other measured cases are:

| Body bytes | Record bytes | Fuel | Instructions | Peak value units |
|---:|---:|---:|---:|---:|
| 0 | 71 | 5,120 | 695 | 112,428 |
| 127 | 198 | 16,309 | 2,120 | 807,338 |
| 140 | 212 | 17,592 | 2,291 | 927,488 |

A 300-byte exploratory body reached `ResourceLimit(ValueUnits)` under the
unchanged 1,000,000-unit execution ceiling. It is retained here as a capacity
finding and is not counted as a successful format case. Later composition
must either reduce simultaneous live values or retain a narrower admitted
body bound; it may not raise the protected limit silently.

The negative corpus covers 31- and 33-byte fingerprints plus 31- and 33-byte
entity identities. Two cross-field cases pin outer entity validation ahead of
fingerprint validation. Tests repeat the same admitted construction path used
by the other RW-080 codec slices.

## Validation and remainder

Source and tests:

- `crates/sley-vm/tests/rw080_codec_program/fingerprint_outer.rs`;
- `fingerprint_outer_encode_matches_native_record_bytes`;
- `fingerprint_outer_encode_preserves_fixed_width_and_outer_errors`.

Validation includes the complete `rw080_codec_program_outer` target, focused
Clippy with warnings denied, workspace formatting, the RW-080 companion
suites, the anti-goal registry check, JSON validation and `git diff --check`.

Fingerprint decode, full program-level composition, optional label/NFC
handling and schema codec legs remain open. No ledger, acceptance verdict,
authority state or release state is promoted by this slice.
