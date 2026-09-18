# RW-080 §1.1 program slice 21: composed Package encode — provisional C0 construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice composes
canonical stored-program encoding for entity kind 2. It is development
evidence, not accepted runtime authority. RW-080 remains BLOCKED and R2
remains NOT_READY; independent review and acceptance debt are unchanged.

## 1. Composed semantic input and result

`encode_program_package(entity_id: Bytes, workspace: Bytes,
root_namespace: Bytes, dependencies: Bytes, exports: Bytes, unit: Unit)`
returns `Result<Bytes, Bytes>`. It constructs, in semantic order:

1. the Package body for workspace, root namespace, dependencies and exports;
2. the `EntityObject` outer record for entity kind 2; and
3. the tag-200, epoch-`09*32` program envelope and object digest.

Successful output is byte-identical to native
`sley-mutate::build_entity_object` storage. Body validation precedes the
outer entity identity check: workspace, root namespace, dependencies and
exports retain their field-order refusal precedence before `entity_id`.

The supported whole-program semantic decoder subsequently added an exact
canonical empty-set kind-2 profile in program slice 22. The supported semantic
encoder still advertises kinds 3, 16, 17 and 18; adding its matching kind-2
arm remains a separate bounded slice.

## 2. Bounded construction

The general path composes the existing Package body, outer-record and program
envelope graphs. The protected one-million value-unit profile exposes F5 for
the smallest nonempty dependency or export set. The exact empty-set case has
a bounded Sley specialization:

- dispatch selects it only when both raw set inputs equal empty bytes;
- workspace and root namespace are validated before the outer entity
  identity, preserving body-before-outer precedence;
- the fixed Package, outer and envelope framing is assembled around the three
  validated 32-byte identities;
- RHW1 hashes `sley2.object.v1` followed by the stored prefix; and
- the digest is included in the same final byte assembly, avoiding a second
  materialized prefix.

The shared canonical-set encoder also returns the fixed `00` body directly
when its already width-validated member input is exactly empty. Nonempty and
malformed inputs continue through the prior order, duplicate and item-width
checks. No host semantic encoder, unchecked slice, charging change or larger
execution limit was introduced. The image imports only the existing B2V1,
PSH1, V2B1 and RHW1 adapters.

Four multi-fault vectors pin body-before-outer behavior: a short workspace
beats a short entity, a long root namespace beats a short entity, duplicate
dependencies beat a short entity, and a valid empty body then exposes the
short entity. Exact native refusal codes are preserved.

## 3. Evidence and resource envelope

Two canonical empty-set Package values, including distinct identities, emit
the exact native 190-byte stored form:

| Dependencies | Exports | Stored bytes | Fuel | Instructions | Peak value units |
|---:|---:|---:|---:|---:|---:|
| 0 | 0 | 190 | 20,040 | 1,995 | 882,632 |

One dependency reaches `ResourceLimit(ValueUnits)` first at 34,167 fuel,
3,995 instructions and 997,663 recorded peak units. One export reaches the
same explicit boundary at 34,176 fuel, 3,997 instructions and 997,670 peak
units. These are capacity boundaries under the unchanged protected profile,
not semantic mismatches.

The refined standalone Package body path measures:

| Dependencies | Exports | Body bytes | Fuel | Instructions | Peak value units |
|---:|---:|---:|---:|---:|---:|
| 0 | 0 | 77 | 9,614 | 1,068 | 210,435 |
| 1 | 1 | 144 | 32,023 | 3,566 | 668,139 |
| 2 | 0 | 144 | 28,835 | 3,053 | 656,395 |
| 1 | 2 | 177 | 39,966 | 4,287 | 955,994 |

Two dependencies plus two exports retain the standalone F5 boundary at
43,945 fuel, 4,627 instructions and 997,834 recorded peak units. The full
program-outer suite passes 59 tests.

Validation:

- `cargo test -p sley-vm --test rw080_codec_program_outer` (59 tests)
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

Local strict review checks exact native bytes, object-domain hash framing,
body-before-outer precedence, empty-path preconditions, nonempty malformed
parity, measured F5 boundaries, adapter confinement and unchanged protected
limits. Independent review is unavailable; no gate, acceptance verdict or
runtime authority state is promoted.
