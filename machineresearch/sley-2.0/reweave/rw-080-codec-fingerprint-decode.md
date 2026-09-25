# RW-080 §1.1 program slice 40: semantic-fingerprint outer decoding — provisional C0 construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice admits one Sley
program for the three-field, no-label SSMC1 `EntityObject` profile. It is
development evidence, not accepted runtime authority. RW-080 remains BLOCKED
and R2 remains NOT_READY; independent review and acceptance debt are unchanged.

## Construction and behavior

`fingerprint_outer_decode(input, unit)` parses canonical fields 1, 2 and 4 in
Sley and returns `(entity_id, body, semantic_fingerprint)`. The construction:

1. converts the input to `Vector<UInt8>` for bounded byte access;
2. calls the existing admitted uvar decoder for the field count, every tag and
   every sized-payload length;
3. enforces collection/resource bounds, checked payload ends, canonical field
   order, duplicate detection and exact 32-byte entity/fingerprint widths;
4. reconstructs the canonical two-field prefix by emitting count `2` and
   copying the original fields 1 and 2 through a runtime Sley loop;
5. copies the fingerprint payload through a second runtime Sley loop; and
6. delegates the reconstructed prefix to the established outer decoder, then
   combines its entity/body result with the fingerprint bytes.

No semantic host codec is used. Host adapters only bridge `Bytes` and
`Vector<UInt8>` or append one byte. Uvar meaning, field parsing, bounds,
ordering, copying and result construction remain inside admitted Sley code.
The new reusable graph helpers cover typed uvar continuations, bounded
sized-payload ends, exact-width payload checks and runtime byte-range copies.

This profile requires count `3` and tag sequence 1, 2, 4. Count `2` returns
`SCB_FIELD_MISSING`. Count `4`, or tag 3 in the third slot, returns
`SSMC_RESERVED_FIELD_PRESENT` because optional labels still require the pinned
Unicode 16.0 NFC implementation. This is an explicit construction boundary,
not a claim that a valid four-field object is malformed.

## Native bytes, refusals and resources

The positive corpus starts from `sley_scb1::encode_record` and verifies the
exact returned entity, body and fingerprint for empty and 127-byte bodies.

| Body bytes | Record bytes | Fuel | Instructions | Peak value units |
|---:|---:|---:|---:|---:|
| 0 | 71 | 10,165 | 1,472 | 138,174 |
| 127 | 198 | 27,674 | 2,925 | 672,141 |

The negative corpus covers missing field 4, 31- and 33-byte fingerprints,
31- and 33-byte entity identities, truncated input, nonminimal count and
fingerprint-length uvars, count overflow, duplicate and descending tags in
fields 2 and 3, the reserved label profile, unknown field 3, payload resource
overflow and trailing bytes. Typed uvar failures are forwarded unchanged.
One hundred deterministic mutations confined to the field-4 suffix agree
code-for-code with native object import after rebuilding a valid envelope and
digest.

## Validation and remainder

Source and tests:

- `crates/sley-vm/tests/rw080_codec_program/fingerprint_outer.rs`;
- `fingerprint_outer_decode_returns_native_fields`;
- `fingerprint_outer_decode_rejects_missing_and_malformed_fingerprint`;
- `fingerprint_outer_decode_preserves_structural_errors`;
- `fingerprint_outer_decode_mutations_agree_with_native_outer_layer`.

Validation includes the complete `rw080_codec_program_outer` target, focused
Clippy with warnings denied, workspace formatting, the RW-080 companion
suites, anti-goal and derived-evidence checks, JSON validation and
`git diff --check`.

Optional label/NFC handling, combined optional-label plus fingerprint records,
full codec-driver composition and schema codec legs remain open. No ledger,
acceptance verdict, authority state or release state is promoted by this
slice.
