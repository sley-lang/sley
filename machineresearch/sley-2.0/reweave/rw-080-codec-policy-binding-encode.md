# RW-080 §1.1 program slice 14: PolicyBinding body encode — provisional C0 construction record

Status: PROVISIONAL (2026-09-17, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice begins entity
kind 17 with a canonical Sley body encoder. It is development evidence, not
accepted runtime authority. RW-080 remains BLOCKED and R2 remains NOT_READY;
independent review and acceptance debt are unchanged.

## 1. Scope and canonical value

`encode_policy_binding(subject: Bytes, requirements: Bytes, unit: Unit)`
returns `Result<Bytes, Bytes>` for the epoch-1 schema value:

```text
union(17, record([
  (1, subject: EntityId),
  (2, requirements: Set<EntityId>),
]))
```

`subject` is exactly 32 bytes. `requirements` is the canonical ascending,
duplicate-free concatenation of zero or more 32-byte entity identities. The
encoder writes canonical uvars for the set count, set payload length, record
length, and union length. Native `sley-mutate::build_entity_object` fixtures
provide every expected body; tests extract the body with SCB cursors instead
of using hand-built expected bytes.

This slice owns standalone body encoding only. Strict body decode and
whole-program kind-17 dispatch remain subsequent bounded slices.

## 2. Construction and refusal behavior

The Namespace and PolicyBinding encoders share one generator-level
entity-id-set pipeline. The PolicyBinding mode requires a present 32-byte
subject, emits field 1 directly instead of wrapping it in `Option`, and uses
union tag 17. Namespace mode retains its prior optional-parent framing and
union tag 3; all existing Namespace byte and refusal tests remain unchanged.

Frozen B2V1 converts both inputs. The subject width is checked before the
requirements pipeline: short returns `SCB_LENGTH_OVERFLOW`, long returns
`SCB_TRAILING_BYTES`. The set walk requires a whole number of 32-byte
identities and returns `SCB_LENGTH_OVERFLOW` for a partial item. Adjacent
identities are compared bytewise after complete reads; descent returns
`SCB_MAP_ORDER` and equality returns `SCB_MAP_DUPLICATE`. Canonical `encode_uvar`
is called for all variable framing integers. PSH1 and V2B1 remain the only
output bridges. No host primitive, adapter, limit, charging rule, or authority
state changed.

Malformed encode inputs have no typed native encode counterpart. Their exact
codes reuse the already tested Namespace fixed-width and set-canonicality
rules and are asserted as Sley-side canonical admission behavior.

## 3. Evidence and resources

One admitted image encodes native canonical fixtures with zero, one, and three
requirements. Each result is deterministic and byte-identical to the native
body. The three-requirement fixture uses a nonuniform subject, which detects
index or position mistakes that repeated-byte fixtures can hide.

| Requirements | Body bytes | Fuel | Instructions | Peak value units |
|---:|---:|---:|---:|---:|
| 0 | 40 | 4,362 | 683 | 111,157 |
| 1 | 73 | 12,263 | 1,559 | 223,596 |
| 3 | 140 | 26,288 | 3,027 | 663,095 |

Seven refusal vectors cover short and long subjects, two partial requirement
lengths, descending order, duplicates, and subject-before-requirements
multi-fault precedence. The complete program-outer suite passes 44 tests,
including all prior Namespace, EntryPoint, and DependencyBinding cases.

Validation:

- `cargo test -p sley-vm --test rw080_codec_program_outer` (44 tests)
- `cargo clippy -p sley-vm --test rw080_codec_program_outer -- -D warnings`
- `cargo test -p sley-vm --test rw080_codec_uvar`
- `cargo test -p sley-vm --test rw080_codec_envelope`
- `cargo test -p sley-vm --test rw080_codec_scaffold`
- `cargo test -p sley-scb1 --locked --lib`
- `cargo test -p sley-mutate --locked --lib object`
- `cargo fmt --all -- --check`
- `python3 scripts/build_anti_goal_conformance.py --check`
- `git diff --check`

Local strict review checked schema tag and field framing, native byte parity,
subject-before-set precedence, set ordering, deterministic output, adapter
confinement, unchanged Namespace behavior, and frozen limits. Independent
review was unavailable; no gate, acceptance verdict, or runtime authority
state is promoted.
