# RW-080 §1.1 program slice 18: Package body encode — provisional C0 construction record

Status: PROVISIONAL (2026-09-17, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice begins entity
kind 2 with a canonical Sley body encoder. It is development evidence, not
accepted runtime authority. RW-080 remains BLOCKED and R2 remains NOT_READY;
independent review and acceptance debt are unchanged.

## 1. Scope and canonical value

`encode_package(workspace: Bytes, root_namespace: Bytes, dependencies: Bytes,
exports: Bytes, unit: Unit)` returns `Result<Bytes, Bytes>` for the epoch-1
schema value:

```text
union(2, record([
  (1, workspace: EntityId),
  (2, root_namespace: EntityId),
  (3, dependencies: Set<EntityId>),
  (4, exports: Set<EntityId>),
]))
```

Both identities are exactly 32 bytes. Each set input is an ascending,
duplicate-free concatenation of zero or more 32-byte entity identities. The
encoder emits canonical count uvars inside both sets, canonical field lengths,
the four-field record and union tag 2. Native
`sley-mutate::build_entity_object` fixtures supply every expected body; the
tests extract the body from native stored objects rather than relying on
hand-built expected bytes.

This slice owns standalone Package body encoding. The paired strict body
decoder subsequently landed in program slice 19, followed by whole-program
decode and encode composition in slices 20 and 21. See
`rw-080-codec-package-decode.md`,
`rw-080-codec-package-compose-decode.md` and
`rw-080-codec-package-compose-encode.md`. The supported whole-program
dispatcher continues to advertise kinds 3, 16, 17 and 18 until a separate
dispatch slice adds kind 2.

## 2. Construction and refusal behavior

The Package entry first runs a small exact-identity validator over `workspace`
and `root_namespace` in field order. The prior Namespace/PolicyBinding
entity-set generator now also has a construction-time canonical-set output.
It exits immediately after the existing exact-width, item-boundary, order and
duplicate checks, before body framing. Package then invokes that graph for
`dependencies` and `exports`. This preserves field-order refusal precedence
without copying the set validator.

A reusable Sley `concat_bytes(parts: Vector<Bytes>, unit: Unit)` graph flattens
ordered byte parts through B2V1, PSH1 and V2B1. The Package composer obtains
both set payload lengths from the canonical set bytes, calls the established
uvar encoder, builds the record, measures that completed record, encodes its
union length and adds tag 2. No host semantic encoder, unchecked byte splice,
new adapter, charging rule or protected limit is introduced.

Short fixed identities return `SCB_LENGTH_OVERFLOW`; long identities return
`SCB_TRAILING_BYTES`. Partial set items return `SCB_LENGTH_OVERFLOW`.
Descending and duplicate adjacent identities return `SCB_MAP_ORDER` and
`SCB_MAP_DUPLICATE`. Workspace is checked before root-namespace, followed by
dependencies and exports, matching the encoder's field order.

## 3. Evidence and resource envelope

At this slice's landing, one admitted image emitted byte-identical native
Package bodies for empty sets,
one member in each set, two dependencies, and one dependency plus two
exports. A second one-plus-one fixture uses nonuniform workspace and
root-namespace identities to detect position or copying mistakes hidden by
repeated-byte fixtures.

| Dependencies | Exports | Body bytes | Fuel | Instructions | Peak value units |
|---:|---:|---:|---:|---:|---:|
| 0 | 0 | 77 | 14,082 | 1,780 | 276,965 |
| 1 | 1 | 144 | 32,011 | 3,562 | 667,775 |
| 2 | 0 | 144 | 31,063 | 3,407 | 689,478 |
| 1 | 2 | 177 | 39,954 | 4,283 | 955,630 |

The F5 capacity boundary is explicit. Two dependencies plus two exports reach
`ResourceLimit(ValueUnits)` under the unchanged one-million value-unit
profile (43,933 fuel, 4,623 instructions, 997,470 recorded peak before the
refused allocation). The smaller fixtures prove canonical semantics; this
slice does not claim an unbounded set size.

The table and F5 figures above preserve the slice-18 landing evidence.
Program slice 21 subsequently added an exact canonical-empty-set fast path to
the shared set encoder. Current standalone measurements and the composed
program envelope are recorded in
`rw-080-codec-package-compose-encode.md`.

Fourteen refusal vectors cover short and long workspace and root-namespace
identities, partial, duplicate and descending members in each set, and
multi-fault field-order precedence. The complete program-outer suite passes
49 tests, including every prior
EntryPoint, Namespace, PolicyBinding, DependencyBinding and supported-dispatch
case.

Validation:

- `cargo test -p sley-vm --test rw080_codec_program_outer` (49 tests)
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

Local strict review checks schema tags and fields, exact native bytes,
nonuniform fixed-identity copies, both set positions, malformed precedence,
the measured F5 boundary, adapter confinement and unchanged protected limits.
Independent review is unavailable; no gate, acceptance verdict or runtime
authority state is promoted.
