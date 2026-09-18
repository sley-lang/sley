# RW-080 §1.1 program slice 19: Package body decode — provisional C0 construction record

Status: PROVISIONAL (2026-09-17, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice adds strict
decoding for entity kind 2. It is development evidence, not accepted runtime
authority. RW-080 remains BLOCKED and R2 remains NOT_READY; independent
review and acceptance debt are unchanged.

## 1. Scope and semantic result

`decode_package(body: Bytes, unit: Unit)` returns:

```text
Result<
  Tuple<
    workspace: Bytes,
    root_namespace: Bytes,
    dependencies: Bytes,
    dependency_count: UInt64,
    exports: Bytes,
    export_count: UInt64
  >,
  Bytes
>
```

The input must be the canonical epoch-1 `EntityBody` union tag 2 with a
four-field `PackageBody` record. Fixed identities return their exact 32 bytes.
Each set returns the concatenated 32-byte identities plus its decoded count.
Known body tags other than 2 return `SSMC_RESERVED_FIELD_PRESENT`; tag 0 and
tags above the closed 18-kind union return `SCB_UNION_INVALID`.

At slice-19 landing this slice owned standalone body decoding. Program
envelope/outer composition remained a later bounded slice. Program slice 20
subsequently composed Package decode; addition to the supported semantic sum
still remains later, so the whole-program dispatcher is unchanged.

## 2. Reused cursor and set semantics

The strict DependencyBinding cursor builders now accept typed semantic output
lists and explicit expected union tags, record counts and known-field tags.
DependencyBinding still instantiates the same kind-18, three-field,
three-`Bytes` shape. Package instantiates kind 2, count 4, fields 1 through 4,
and threads set counts as `UInt64` values at their actual field positions.
The existing DependencyBinding semantic and rejection suites pass unchanged.

Fields 1 and 2 use the shared exact-32 validator and bounded copy loop. Fields
3 and 4 use a new variable-field bounds stage followed by the same copy loop.
Immediately after field 3 is copied, Sley validates and decodes the dependency
set before reading field 4. This preserves native field-order precedence: a
malformed dependency set wins over any later export-field fault.

Set payload validation reuses the strict PolicyBinding decoder. A Sley helper
wraps the bounded list payload and its already validated subject identity in a
temporary canonical kind-17 body using the established uvar encoder and
byte-part concatenator, then calls the PolicyBinding decoder. The helper runs
again for exports. No host semantic decoder or unchecked byte slice is added;
imports remain B2V1, PSH1 and V2B1.

Record trailing is checked only after the export set succeeds. Outer union
trailing is checked after record completion. Uvar, bounds, missing, unknown,
duplicate, order, fixed-width and map-canonicality refusals therefore retain
the native nesting order.

## 3. Landing evidence and resource envelope

One admitted image returns native semantic values for empty sets, one member
in each set, one dependency plus two exports, and a nonuniform fixed-identity
fixture.

| Dependencies | Exports | Body bytes | Fuel | Instructions | Peak value units |
|---:|---:|---:|---:|---:|---:|
| 0 | 0 | 77 | 26,527 | 3,428 | 317,228 |
| 1 | 1 | 144 | 53,172 | 5,772 | 607,483 |
| 1 | 2 | 177 | 65,227 | 6,721 | 844,848 |

Two dependencies plus two exports reach `ResourceLimit(ValueUnits)` first
under the unchanged one-million value-unit profile (62,818 fuel, 6,371
instructions, 999,993 recorded peak before refusal). The smaller fixtures
prove canonical semantics; this slice does not claim an unbounded set size.

Twenty-nine malformed bodies compare code-for-code with native import. They
cover missing and extra record fields, duplicate/order/unknown field tags,
short and long fixed identities, partial/duplicate/descending members in both
sets, dependency-before-field-4 precedence, nonminimal framing at three outer
nesting levels, a bounded unterminated nested count, union overrun/underrun
and outer trailing. Five additional vectors pin fail-closed body-kind scope
and closed-union behavior. The full
program-outer suite passes 53 tests.

Validation:

- `cargo test -p sley-vm --test rw080_codec_program_outer` (53 tests)
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

These measurements are the slice-19 landing baseline. Program slice 20 later
reduced transient set-wrapper allocations while preserving every semantic and
malformed-code check. Its current measurements and new F5 boundary are in
`rw-080-codec-package-compose-decode.md`.

Local strict review checks typed cursor generalization, unchanged kind-18
behavior, schema tag and field order, native semantic values, malformed-code
parity, dependency-before-export precedence, the measured F5 boundary,
adapter confinement and unchanged protected limits. Independent review is
unavailable; no gate, acceptance verdict or runtime authority state is
promoted.
