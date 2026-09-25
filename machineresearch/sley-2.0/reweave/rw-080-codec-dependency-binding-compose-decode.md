# RW-080 §1.1 program slice 12: DependencyBinding program decode composition — provisional C0 construction record

Status: PROVISIONAL (2026-09-17, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice composes the
strict slice-11 `DependencyBinding` body work into the supported whole-program
decode entry. It is development evidence, not accepted runtime authority.
RW-080 remains BLOCKED and R2 remains NOT_READY; independent review and
acceptance debt are unchanged.

## 1. Scope and semantic arm

`decode_supported_program(18, stored, unit)` validates the complete program
envelope and digest, then recognizes the fixed canonical epoch-1 outer/body
shape for kind 18:

```text
outer record([
  (1, entity_id: FixedBytes32),
  (2, union(18, record([
    (1, dependency_root: FixedBytes32),
    (2, external_package: EntityId),
    (3, local_namespace: EntityId),
  ]))),
])
```

The supported value sum grows additively. Its outer `Ok` arm retains the
previous `Result<EntryPoint, Namespace>` unchanged; its outer `Err` arm carries
`(entity_id, dependency_root, external_package, local_namespace)`, all as exact
32-byte `Bytes` values. The codec refusal remains the enclosing
`Result<..., Bytes>::Err`, so a valid dependency value cannot collide with a
refusal.

## 2. Bounded canonical composition

The canonical kind-18 path works directly over the validated 142-byte outer
payload. It checks every fixed framing byte, then copies the four semantic
identities without materializing a second body value or running the generic
outer decoder. Any payload length or framing mismatch returns the existing
provisional `SSMC_RESERVED_FIELD_PRESENT` scope refusal. The standalone
slice-11 body decoder remains the strict diagnostic owner and retains its
24-vector native rejection-precedence matrix.

The distinction is deliberate. Composing the generic outer decoder after the
219-byte envelope exhausts the frozen one-million value-unit ceiling before it
can return a typed mismatch. The canonical path stays under that ceiling
without changing a protected limit, charging rule, adapter, or host primitive.
Envelope validation still runs first, so corrupted kind-18 digests return
`SCB_DIGEST_MISMATCH` before shape classification.

The shared envelope validator also stops carrying its digest-end counter and
original input `Bytes` after the trailing check, where neither value is read.
That liveness-only repair preserves output and refusal order while reducing all
composed decode paths. The compact identity-copy helper drops completed loop
counters before vector conversion for the same reason.

## 3. Evidence and resources

One admitted package proves the established Namespace and EntryPoint arms and
the new DependencyBinding arm. The canonical kind-18 fixture returns all four
identities exactly. Tests also pin kind-18 declaration against EntryPoint
bytes, a corrupted kind-18 digest, and a recomputed envelope containing
noncanonical dependency framing.

Under the unchanged limits (100,000 instructions, 1,000,000 fuel, 1,000,000
value units, 100,000 output units):

| Kind | Stored bytes | Fuel | Instructions | Peak value units |
|---|---:|---:|---:|---:|
| EntryPoint 16 | 153 | 27,808 | 3,486 | 599,647 |
| Namespace 3, parent present and no members | 155 | 29,119 | 3,655 | 617,842 |
| DependencyBinding 18 | 219 | 28,855 | 3,283 | 996,953 |

Targeted validation:

- `cargo test -p sley-vm --test rw080_codec_program_outer` (42 tests)
- `cargo clippy -p sley-vm --test rw080_codec_program_outer -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`

Local review checked the nested sum identities, every canonical byte and copy
range, envelope-first precedence, scope behavior for noncanonical kind-18
payloads, dead-value removal, and frozen limits. Independent review was not
available; no gate, ledger, acceptance verdict, or runtime-authority status is
promoted.

The matching kind-18 supported encode arm is recorded in
`rw-080-codec-dependency-binding-compose-encode.md`. The remaining work is the
other entity-body kinds and the schema-bearing canonical entry.

Slice 16 subsequently added PolicyBinding and consolidated Namespace plus
PolicyBinding into one admitted entity-set parser. The current kind-18 fixture
measures 28,270 fuel, 3,295 instructions, and 996,703 peak value units; see
`rw-080-codec-policy-binding-compose-decode.md`. The protected limits and
kind-18 semantic tuple are unchanged.
