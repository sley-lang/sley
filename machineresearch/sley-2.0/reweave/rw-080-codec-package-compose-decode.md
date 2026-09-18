# RW-080 §1.1 program slice 20: composed Package decode — provisional C0 construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice composes strict
stored-program decoding for entity kind 2. It is development evidence, not
accepted runtime authority. RW-080 remains BLOCKED and R2 remains NOT_READY;
independent review and acceptance debt are unchanged.

## 1. Composed semantic result

`decode_program_package(stored: Bytes, unit: Unit)` executes, in order:

1. program envelope validation, including the stored digest;
2. strict `EntityObject` outer-record decoding; and
3. strict Package body decoding.

It returns:

```text
Result<
  Tuple<
    entity_id: Bytes,
    body: Tuple<
      workspace: Bytes,
      root_namespace: Bytes,
      dependencies: Bytes,
      dependency_count: UInt64,
      exports: Bytes,
      export_count: UInt64
    >
  >,
  Bytes
>
```

The nested body tuple preserves the body decoder's existing typed result and
avoids rebuilding six values in the caller. Exact nonuniform entity,
workspace and root-namespace identities are covered. Callee errors pass
through unchanged, so envelope errors precede outer errors and outer errors
precede body errors. A valid Namespace stored object reaches the Package body
scope refusal `SSMC_RESERVED_FIELD_PRESENT` after both earlier layers pass.

The supported whole-program semantic dispatcher subsequently added the exact
canonical empty-set kind-2 profile in program slice 22. Its deliberately
bounded representation and capacity evidence are recorded in
`rw-080-codec-package-supported-decode.md`; this slice remains the strict
general composed Package decoder.

## 2. Allocation refinement

Composition initially exposed excessive transient allocation in the reusable
set-payload wrapper. The repaired Sley construction computes the temporary
PolicyBinding record length before emission and concatenates the body once.
For the exact canonical empty-list payload `00`, it returns the already
validated subject, empty member bytes and count zero directly. Package fields
1 and 2 have already passed exact-32 validation before either set helper is
called, so this fast path preserves the native semantic preconditions.

All nonempty and malformed payloads continue through the strict
PolicyBinding-derived list parser. Thirty native-parity malformed Package
vectors remain green, including an empty-list trailing-byte discriminator,
nonminimal framing, partial identities, duplicate and descending members,
and dependency-before-export precedence.
No host semantic decoder, unchecked slice or larger execution limit was
introduced. Imports remain B2V1, PSH1, V2B1 and the existing envelope RHW1;
the protected execution profile remains unchanged.

## 3. Evidence and resource envelope

Two canonical empty-set Package objects, including nonuniform fixed
identities, decode successfully from 190 stored bytes:

| Dependencies | Exports | Stored bytes | Fuel | Instructions | Peak value units |
|---:|---:|---:|---:|---:|---:|
| 0 | 0 | 190 | 35,150 | 3,956 | 907,847 |

One dependency or one export is the composed F5 boundary. Each 224-byte
stored object reaches `ResourceLimit(ValueUnits)` first at 23,473 fuel, 2,623
instructions and 998,543 recorded peak units before refusal. This is an
explicit capacity boundary, not a semantic mismatch.

The refined standalone body path now measures:

| Dependencies | Exports | Body bytes | Fuel | Instructions | Peak value units |
|---:|---:|---:|---:|---:|---:|
| 0 | 0 | 77 | 5,639 | 670 | 151,092 |
| 1 | 1 | 144 | 45,538 | 5,042 | 474,841 |
| 1 | 2 | 177 | 55,844 | 5,826 | 643,335 |

Three dependencies plus three exports reach the standalone F5 boundary first
at 59,595 fuel, 5,740 instructions and 998,853 recorded peak units. Four
composed malformed vectors pin envelope, outer, body and digest behavior;
the namespace transplant separately pins body-kind scope. The full
program-outer suite passes 56 tests.

Validation:

- `cargo test -p sley-vm --test rw080_codec_program_outer` (56 tests)
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
- `git diff --check`

Local strict review checks exact layer order, the nested result type, empty
fast-path preconditions, unchanged malformed parity, both measured F5
boundaries, adapter confinement and unchanged protected limits. Independent
review is unavailable; no gate, acceptance verdict or runtime authority state
is promoted.
