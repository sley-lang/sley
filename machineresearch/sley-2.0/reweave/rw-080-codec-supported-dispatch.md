# RW-080 §1.1 program slice 8: supported-kind Sley decode dispatch — provisional C0 construction record

Status: PROVISIONAL (2026-09-17, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice composes
the previously proven envelope, outer-record, EntryPoint, and
Namespace decoders behind one Sley entry. It is development evidence,
not accepted runtime authority. RW-080 remains BLOCKED and R2 remains
NOT_READY pending the recorded independent review and acceptance debt.

## 1. Bounded scope

`decode_supported_program(declared_kind: UInt64, stored: Bytes,
unit: Unit)` performs one ordered pipeline in one approved execution:

1. validate the complete SCB1 stored-object envelope and digest;
2. decode the SSMC1 outer entity record once;
3. classify the declared kind;
4. invoke the selected body decoder; and
5. join the entity id with the decoded body value.

The supported kinds are Namespace (3) and EntryPoint (16). The body
decoder checks its own union tag after dispatch, so a false declaration
cannot reinterpret bytes. Known but unsupported tags 1 through 18
return `SSMC_RESERVED_FIELD_PRESENT`; 0 and 19 or greater return
`SSMC_ENTITY_KIND_UNKNOWN`. Envelope and outer failures precede kind
classification, and selected-body failures follow it.

This is a supported-kind decode entry with an explicit declared-kind
input. It is not the final canonical `codec_main`: it does not derive
the kind from a complete schema-aware program request, implement the
other 16 entity bodies, or process label/NFC/fingerprint fields. The
matching bounded encode dispatcher landed later as slice 9
(`rw-080-codec-supported-encode-dispatch.md`).

## 2. Result representation

The entry returns `Result<SupportedValue, Bytes>`, where
`SupportedValue` is a closed two-arm sum represented by another
`Result`:

- inner `Ok`: EntryPoint
  `(entity_id: Bytes, function: Bytes, exposure: UInt64)`;
- inner `Err`: Namespace
  `(entity_id: Bytes, parent: Bytes, members: Bytes)`.

The outer `Err(Bytes)` is reserved for the exact existing `SCB_*` or
`SSMC_*` refusal code. This preserves the difference between a valid
Namespace value and a codec refusal without introducing a new named
layout before the schema-bearing main entry exists.

## 3. Implementation

- `crates/sley-vm/tests/rw080_codec_program_outer.rs` declares the
  child construction module.
- `crates/sley-vm/tests/rw080_codec_program/supported_dispatch.rs`
  owns the dispatcher graph and its tests. Keeping this graph in a
  child module avoids further growth of the retained 29k-line
  per-format construction record while still reusing its private
  assembler and proven decoder builders.
- The admitted image contains one shared raw decoder, envelope
  validator, outer decoder, EntryPoint decoder, Namespace decoder, and
  dispatch entry. Equal immutable constants from the independently
  authored graphs are coalesced before lowering, with every
  `ConstantRef` rewritten to the retained identity.
- Imports remain the frozen `HOST_ABI_V2` rows B2V1, PSH1, V2B1, and
  RHW1. The image uses the v2 admit/approve/execute boundary and the
  unchanged codec limits.

## 4. Fail-closed and precedence evidence

The two added tests prove:

- canonical EntryPoint kind 16 returns the inner EntryPoint arm;
- canonical parent-only Namespace kind 3 returns the inner Namespace
  arm with the exact entity, parent, and empty member bytes;
- declaring Namespace for an EntryPoint object reaches the Namespace
  decoder and refuses `SSMC_RESERVED_FIELD_PRESENT`;
- declared known-unsupported kind 1 refuses
  `SSMC_RESERVED_FIELD_PRESENT`;
- declared unknown kinds 0 and 19 refuse
  `SSMC_ENTITY_KIND_UNKNOWN`;
- a corrupt digest refuses `SCB_DIGEST_MISMATCH` for both supported
  kind 16 and unknown kind 19, proving envelope validation precedes
  declaration classification.

The same admitted package handles all vectors. The harness supplies
only the declared kind and stored bytes; it does not decode the
envelope, outer record, or body for Sley.

## 5. Resource evidence and discarded constructions

Under the unchanged limits (100,000 instructions, 1,000,000 fuel,
1,000,000 value units, 100,000 output units):

| Kind | Stored bytes | Fuel | Instructions | Peak value units |
|---|---:|---:|---:|---:|
| EntryPoint 16 | 153 | 28,937 | 3,483 | 586,544 |
| Namespace 3, parent present and no members | 155 | 30,264 | 3,652 | 604,759 |

Two rejected internal designs are retained as engineering evidence.
Dispatching to the two already composed whole-program wrappers added a
redundant caller frame and stopped at 999,975 units before producing a
value. The replacement validates the envelope and outer record once,
then calls the body decoder directly. A parent-plus-two-member
Namespace fixture measured 1,262,004 units under a temporary diagnostic
ceiling, consistent with the slice-7 F5 size envelope; the ceiling was
immediately restored and no protected limit changed. The accepted
parent-only vector is one of the slice-7 in-envelope cases.

## 6. Validation and review state

Targeted validation for this slice:

- `cargo test -p sley-vm --test rw080_codec_program_outer`
- `cargo clippy -p sley-vm --test rw080_codec_program_outer -- -D warnings`
- `cargo test -p sley-vm --test rw080_codec_uvar`
- `cargo test -p sley-vm --test rw080_codec_envelope`
- `cargo test -p sley-vm --test rw080_codec_scaffold`
- `cargo test -p sley-scb1 --locked --lib`
- `cargo test -p sley-mutate --locked --lib object`
- `cargo fmt --all -- --check`
- `python3 scripts/build_anti_goal_conformance.py --check`
- `git diff --check`

The implementation was reviewed locally for strict error ordering,
type-arm separation, accidental host decoding, protected-limit changes,
and growth of the already large parent test file. Independent review
was not available in this session, so the operator-authorized result
remains provisional. No gate, ledger, acceptance verdict, or runtime
authority status is promoted.

## 7. Remaining dependency

The next codec work after the matching slice-9 encode dispatcher is to
add the remaining body kinds, then label/NFC/fingerprint handling and
verification on the path to a canonical schema-bearing main entry. The
current explicit-kind entries remain bounded composition proofs for
kinds 3 and 16.
