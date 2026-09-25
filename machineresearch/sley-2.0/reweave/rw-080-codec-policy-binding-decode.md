# RW-080 §1.1 program slice 15: PolicyBinding body decode — provisional C0 construction record

Status: PROVISIONAL (2026-09-17, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice adds the strict
entity-kind-17 body decoder paired with slice 14. It is development evidence,
not accepted runtime authority. RW-080 remains BLOCKED and R2 remains
NOT_READY; independent review and acceptance debt are unchanged.

## 1. Scope and semantic result

`decode_policy_binding(body: Bytes, unit: Unit)` accepts exactly the epoch-1
schema value:

```text
union(17, record([
  (1, subject: EntityId),
  (2, requirements: Set<EntityId>),
]))
```

It returns `Result<(subject: Bytes, requirements: Bytes, count: UInt64),
Bytes>`. `subject` is the exact 32-byte identity. `requirements` is the
canonical ascending concatenation of all decoded 32-byte identities, and
`count` is the decoded set count. Other declared entity tags remain explicit
`SSMC_RESERVED_FIELD_PRESENT` scope exclusions; tag 0 and tags above 18 return
`SCB_UNION_INVALID`.

This slice owns standalone body decoding. Whole-program kind-17 dispatch
landed provisionally in slice 16 and is recorded in
`rw-080-codec-policy-binding-compose-decode.md`.

## 2. Shared strict parser

The Namespace and PolicyBinding decoders share one generator-level record and
entity-id-set parser. PolicyBinding selects union tag 17 and routes field 1
directly into the existing exact-32-byte copy path. Namespace retains union tag
3 and its nested `Option<EntityId>` parser. In the PolicyBinding image, the
Namespace-only option blocks are declared `ExplicitlyUnreachable`, preserving
the admitted CFG reachability contract without creating a second set parser.

Record field decisions retain native order: count, field-1 tag and length,
field-1 bounds and exact width, field-2 tag and length, set count, each sized
identity, strict adjacent ordering, then record/union/body trailing checks.
Subject lengths below 32 return `SCB_LENGTH_OVERFLOW`; lengths above 32 return
`SCB_TRAILING_BYTES`. Equal requirements return `SCB_MAP_DUPLICATE`; descending
requirements return `SCB_MAP_ORDER`. Subject faults precede requirement faults.

The image uses only the frozen B2V1, PSH1, and V2B1 bridges plus the shared Sley
uvar decoder. No host primitive, adapter, limit, charging rule, or authority
state changed.

## 3. Evidence and resources

Three native canonical bodies with zero, one, and three requirements decode to
the exact typed semantics. The largest case uses a nonuniform subject.

| Requirements | Body bytes | Fuel | Instructions | Peak value units |
|---:|---:|---:|---:|---:|
| 0 | 40 | 5,469 | 814 | 82,889 |
| 1 | 73 | 12,882 | 1,454 | 115,442 |
| 3 | 140 | 25,575 | 2,400 | 247,652 |

Six malformed bodies compare the Sley refusal directly with the native import
code: short and long subjects, duplicate and descending requirements,
subject-before-requirements multi-fault precedence, and body trailing bytes.
A transplanted canonical Namespace body proves the explicit scope refusal.
The complete program-outer suite passes 46 tests, including all prior
Namespace behavior.

Validation:

- `cargo test -p sley-vm --test rw080_codec_program_outer` (46 tests)
- `cargo clippy -p sley-vm --test rw080_codec_program_outer -- -D warnings`
- `cargo test -p sley-vm --test rw080_codec_uvar`
- `cargo test -p sley-vm --test rw080_codec_envelope`
- `cargo test -p sley-vm --test rw080_codec_scaffold`
- `cargo test -p sley-scb1 --locked --lib`
- `cargo test -p sley-mutate --locked --lib object`
- `cargo fmt --all -- --check`
- `python3 scripts/build_anti_goal_conformance.py --check`
- `git diff --check`

Local strict review checked schema selection, field and trailing order, native
semantic parity, native refusal parity, explicit unreachable declarations,
shared Namespace behavior, adapter confinement, and frozen limits.
Independent review was unavailable; no gate, acceptance verdict, or runtime
authority state is promoted.
