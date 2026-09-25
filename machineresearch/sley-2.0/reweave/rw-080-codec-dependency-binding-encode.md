# RW-080 §1.1 program slice 10: DependencyBinding body encode — provisional C0 construction record

Status: PROVISIONAL (2026-09-17, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice begins
entity kind 18 with a canonical Sley body encoder. It is development
evidence, not accepted runtime authority. RW-080 remains BLOCKED and
R2 remains NOT_READY; independent review and acceptance debt are
unchanged.

## 1. Scope and canonical bytes

`encode_dependency_binding(dependency_root: Bytes,
external_package: Bytes, local_namespace: Bytes, unit: Unit)` returns
`Result<Bytes, Bytes>` for the exact epoch-1 schema record:

```text
union(18, record([
  (1, dependency_root: FixedBytes32),
  (2, external_package: EntityId),
  (3, local_namespace: EntityId),
]))
```

All framing integers are single-byte canonical uvars for this fixed
shape: `12 67 03 01 20 <root32> 02 20 <package32> 03 20
<namespace32>`, 105 bytes total. Expected bytes come from native
`sley-mutate::build_entity_object` fixtures and are extracted through
SCB cursors; the test never uses a hand-built expected body.

This slice owns body encoding only. The paired strict body decoder landed in
slice 11; whole-program decode and encode composition later landed in slices
12 and 13 without changing this standalone diagnostic entry.

## 2. Construction and error behavior

- `crates/sley-vm/tests/rw080_codec_program/dependency_binding.rs`
  owns the builder, admitted image, native fixtures, and two tests.
- Each Bytes input is converted through frozen B2V1 and immediately
  checked before the next field. A length below 32 returns
  `SCB_LENGTH_OVERFLOW`; a length above 32 returns
  `SCB_TRAILING_BYTES`. This preserves field order even if a later
  input is independently invalid.
- The output uses frozen PSH1 and V2B1 only. Rust builder loops generate
  backedge-free Sley block chains; the admitted image performs 96
  explicit indexed reads and pushes. This avoids the recorded F8
  counted-loop defect.
- Indexed-read `None` is an internal invariant after each exact-length
  gate. Adapter capacity refusal returns `SCB_RESOURCE_LIMIT`.
- The implementation factors exact-length gates, constant-push chains,
  and fixed-32 copy chains instead of adding another hand-expanded
  multi-thousand-line encoder.

The native API accepts typed `StateRoot`/`EntityId` values, so malformed
lengths have no native encode counterpart. The Sley-side codes are the
existing fixed-width decode/outer-encode mirrors and are explicitly
tested rather than presented as native parity claims.

## 3. Evidence and resources

One admitted image encodes three distinct post-admission input triples.
Every result matches the native canonical body byte-for-byte. Eight
malformed inputs cover short and long values for each field plus two
multi-fault precedence cases; the first invalid field determines the
exact refusal.

Each valid 105-byte result uses 2,294 fuel, 324 instructions, and
190,992 peak value units under the unchanged codec limits. No
protected limit, value charging rule, host primitive, hash preimage, or
authority state changed.

Targeted validation:

- `cargo test -p sley-vm --test rw080_codec_program_outer` (40 tests)
- `cargo clippy -p sley-vm --test rw080_codec_program_outer -- -D warnings`
- `cargo test -p sley-vm --test rw080_codec_uvar`
- `cargo test -p sley-vm --test rw080_codec_envelope`
- `cargo test -p sley-vm --test rw080_codec_scaffold`
- `cargo test -p sley-scb1 --locked --lib`
- `cargo test -p sley-mutate --locked --lib object`
- `cargo fmt --all -- --check`
- `python3 scripts/build_anti_goal_conformance.py --check`
- `git diff --check`

Local strict review checked byte layout, field-order precedence,
backedge absence, exact native comparison, adapter confinement, and
frozen limits. Independent review was unavailable; no gate, ledger, or
runtime-authority promotion is claimed.

The strict kind-18 body decoder is recorded in
`rw-080-codec-dependency-binding-decode.md`, and the whole-program decode
composition is recorded in
`rw-080-codec-dependency-binding-compose-decode.md`. The matching
whole-program kind-18 encode composition and third supported encode arm are
recorded in `rw-080-codec-dependency-binding-compose-encode.md`.
