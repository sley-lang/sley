# RW-080 §1.1 program slice 22: Package supported-decode dispatch — provisional C0 construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice adds entity kind
2 to the supported whole-program semantic decode dispatcher. It is development
evidence, not accepted runtime authority. RW-080 remains BLOCKED and R2
remains NOT_READY; the independent-review and acceptance debt is unchanged.

## 1. Bounded profile and result

`decode_supported_program(declared_kind: UInt64, stored: Bytes, unit: Unit)`
now recognizes Package kind 2 in addition to Namespace 3, EntryPoint 16,
PolicyBinding 17 and DependencyBinding 18. Envelope validation and strict
outer-record decoding still run before kind selection. A false declaration
therefore cannot bypass the stored digest or reinterpret another body as a
Package.

This slice admits the exact canonical empty-set Package body:

- body length 77;
- Package union tag and four ordered field frames;
- exact 32-byte workspace and root-namespace widths; and
- canonical empty dependency and export sets.

The compact checker compares a fixed template byte by byte and skips only the
two 32-byte identity ranges. A counted loop avoids retaining the seven generic
Package body subgraphs in the composed image. The selected result uses the
existing four-field tagged arm:

```text
(kind = 2, entity_id, validated_body, empty_reserved)
```

The final slot is canonically empty. This preserves the established
`Result<Result<Result<EntryPoint, TaggedValue>, DependencyBinding>, Bytes>`
shape while avoiding a second live body copy on the matching encode path and
keeps the kind-18 path inside the frozen value budget. It does not
claim a general decoded Package value: nonempty dependency or export sets are
outside this bounded dispatcher profile. The strict standalone and composed
Package decoders from slices 19 and 20 remain the semantic authority for those
larger values.

## 2. Fail-closed behavior

The Package checker returns only a Boolean to the dispatcher. Success is
normalized into the existing tagged semantic arm; failure reaches the shared
`SSMC_RESERVED_FIELD_PRESENT` refusal. This keeps the checker result small and
lets the dispatcher own its established error type.

The tests pin three distinct boundaries:

- an empty canonical Package with nonuniform entity, workspace and root
  identities returns the kind-2 tagged arm and exact body witness;
- a same-length body with a noncanonical dependency count reaches the typed
  scope refusal after envelope and outer validation;
- declaring kind 2 for an EntryPoint object returns the typed scope refusal;
- a valid Package with one dependency or one export fails closed at
  `ResourceLimit(ValueUnits)` under the protected profile. The larger stored
  object reaches the capacity boundary before a typed scope value can be
  materialized, matching the earlier composed Package evidence.

Known unsupported and unknown-kind rules are unchanged, as are digest-first
precedence and the four older semantic arms.

## 3. Resource evidence

Under the unchanged limits (100,000 instructions, 1,000,000 fuel, 1,000,000
value units and 100,000 output units), the five successful fixtures measure:

| Kind | Stored bytes | Fuel | Instructions | Peak value units |
|---|---:|---:|---:|---:|
| EntryPoint 16 | 153 | 27,822 | 3,490 | 602,676 |
| Namespace 3 | 155 | 29,136 | 3,659 | 620,996 |
| DependencyBinding 18 | 219 | 28,277 | 3,296 | 999,972 |
| PolicyBinding 17 | 186 | 41,675 | 4,686 | 876,956 |
| Package 2, empty sets | 190 | 30,192 | 3,485 | 849,463 |

The kind-18 fixture remains the tight valid case with 28 value units of
headroom. No protected limit changed. One-dependency and one-export Package
objects are each 224 bytes and reach the explicit value-unit boundary at
26,027 fuel, 2,984 instructions and 999,883 recorded peak units.

Program slice 23 adds the paired supported encoder for this exact semantic
witness; see `rw-080-codec-package-supported-encode.md`.

The retained construction studies showed why the bounded checker is needed.
Adding the generic Package decoder family raised the kind-18 image above 1.08
million units. Enlarging the semantic sum also exceeded the ceiling. The
landed construction keeps the prior result type, returns the dependency tuple
from its compact callee before normalizing it, validates the empty Package with
one masked loop, and reuses the already validated declared kind in later
normalization and entity-set calls.

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

Local strict review checks the exact empty-body framing, dynamic identity
ranges, result-arm placement, declaration mismatch, capacity behavior,
digest-first ordering, adapter confinement and unchanged protected limits.
Independent review is unavailable in this session, so no gate, acceptance
verdict or runtime-authority state is promoted.
