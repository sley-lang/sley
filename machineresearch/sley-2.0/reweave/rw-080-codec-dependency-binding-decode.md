# RW-080 §1.1 program slice 11: DependencyBinding body decode — provisional C0 construction record

Status: PROVISIONAL (2026-09-17, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice adds the
strict kind-18 body decoder paired with the slice-10 encoder. It is
development evidence, not accepted runtime authority. RW-080 remains
BLOCKED and R2 remains NOT_READY; independent review and acceptance
debt are unchanged.

## 1. Scope and semantic result

`decode_dependency_binding(body: Bytes, unit: Unit)` parses the exact
epoch-1 schema shape:

```text
union(18, record([
  (1, dependency_root: FixedBytes32),
  (2, external_package: EntityId),
  (3, local_namespace: EntityId),
]))
```

It returns `Result<Tuple<Bytes, Bytes, Bytes>, Bytes>` in that field
order. Every successful component is exactly 32 bytes. The three
post-admission valid fixtures come from native
`sley-mutate::build_entity_object` values, and body bytes are extracted
with SCB cursors rather than fixed stored-object offsets.

This slice owns body decoding only. Full envelope composition and the
kind-18 arm in both supported dispatchers remain later work.

## 2. Strict parsing and precedence

- `crates/sley-vm/tests/rw080_codec_program/dependency_binding_decode.rs`
  owns the builder, admitted image, native-parity fixtures, and tests.
- Union tag 18 is accepted. Tags 1 through 17 return the explicit
  provisional scope refusal `SSMC_RESERVED_FIELD_PRESENT`; tag 0 and
  tags above the closed 1-through-18 set return `SCB_UNION_INVALID`.
- Union and field integers call the existing Sley `decode_uvar` graph,
  retaining canonical overflow and nonminimal encodings. Declared
  lengths and record counts retain the frozen 64 MiB and 65,535 limits.
- The declared union payload is copied into a bounded `Bytes` value
  before its record is parsed. A continuation byte cannot escape into
  outer trailing data; an unterminated count at the exact union boundary
  therefore returns native `SCB_LENGTH_OVERFLOW`.
- Record count, duplicate, order, known-tag, and unknown-tag decisions
  follow `sley-mutate::codec::decode_record_fields` in its actual order.
  Immediate repetition is duplicate; a lower previous tag or another
  known schema tag in the wrong position is order; an out-of-schema tag
  is unknown.
- Each sized payload checks the resource limit, declared end, and exact
  32-byte width before copying. Short payloads return
  `SCB_LENGTH_OVERFLOW`; a fully present overlong nested payload returns
  `SCB_TRAILING_BYTES`. A declaration crossing the bounded union remains
  length overflow, matching native precedence.
- B2V1, PSH1, and V2B1 are the only adapters. Indexed-read failure and
  checked-index overflow after established bounds are internal
  invariants; adapter capacity refusal is `SCB_RESOURCE_LIMIT`.

## 3. Evidence and resources

One admitted image decodes three distinct input triples and returns all
96 identity bytes exactly. Twenty-four malformed kind-18 bodies compare
the Sley refusal byte-for-byte with native import: record count, every
tag position, all six short/long fixed-field cases, union overrun and
cutoff, inner and outer trailing bytes, a boundary-truncated uvar, a
nonminimal uvar, and all three declared resource-limit gates. Three
union-scope vectors separately pin known
unimplemented and closed-union behavior.

Each valid 105-byte body uses 14,059 fuel, 1,692 instructions, and
251,717 peak value units under the unchanged codec limits. No protected
limit, value charging rule, host primitive, hash preimage, or authority
state changed.

Targeted validation:

- `cargo test -p sley-vm --test rw080_codec_program_outer` (42 tests)
- `cargo clippy -p sley-vm --test rw080_codec_program_outer -- -D warnings`
- `cargo test -p sley-vm --test rw080_codec_uvar`
- `cargo test -p sley-vm --test rw080_codec_envelope`
- `cargo test -p sley-vm --test rw080_codec_scaffold`
- `cargo test -p sley-scb1 --locked --lib`
- `cargo test -p sley-mutate --locked --lib object`
- `cargo fmt --all -- --check`
- `python3 scripts/build_anti_goal_conformance.py --check`
- `git diff --check`

Local strict review checked bounded nested parsing, native decision
order, all field-copy indices, exact semantic tuple order, adapter
confinement, and frozen limits. Independent review was unavailable; no
gate, ledger, or runtime-authority promotion is claimed.

The next dependency is composition with the shared envelope and outer
decoder, followed by a third arm in both supported dispatchers. The
semantic sum must grow without changing the established EntryPoint and
Namespace arm identities.
