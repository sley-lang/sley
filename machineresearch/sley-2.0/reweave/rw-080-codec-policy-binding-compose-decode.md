# RW-080 §1.1 program slice 16: PolicyBinding program decode composition — provisional C0 construction record

Status: PROVISIONAL (2026-09-17, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice composes the
strict slice-15 PolicyBinding body decoder into the supported whole-program
decode entry. It is development evidence, not accepted runtime authority.
RW-080 remains BLOCKED and R2 remains NOT_READY; independent review and
acceptance debt are unchanged.

## 1. Scope and semantic arm

`decode_supported_program(17, stored, unit)` validates the complete stored
object and digest, decodes the outer entity record, and invokes the strict
PolicyBinding body decoder. The returned policy value contains the exact
entity id, subject id, and canonical concatenated requirement ids.

The four-kind semantic value is:

```text
Result<
  Result<
    EntryPoint(entity_id, function, exposure),
    TaggedEntitySet(kind, entity_id, subject_or_parent, requirements_or_members)
  >,
  DependencyBinding(entity_id, root, package, namespace)
>
```

The tagged entity-set arm uses kind 3 for Namespace and kind 17 for
PolicyBinding. The enclosing codec `Result<..., Bytes>` still separates every
valid value from the exact `SCB_*` or `SSMC_*` refusal. The tag avoids
duplicating two equal three-`Bytes` tuple schemas while preserving their kind
identity. The prior three-kind encoder round-trip test explicitly converts
kind 3 back to its established Namespace arm and refuses to reinterpret kind
17 as an older value.

## 2. Shared entity-set decoder

Namespace and PolicyBinding now share one admitted entity-set decoder graph.
The dispatcher supplies the already-classified kind as a typed function
parameter. The decoder requires the body union tag to equal that kind, then
routes field 1 to the Namespace option parser for kind 3 or the direct
fixed-32 subject path for kind 17. Field 2 retains the shared strict ordered
identity-set parser. The standalone per-kind graphs remain available to their
native parity tests, including PolicyBinding's explicitly unreachable
Namespace-only blocks.

This removes a duplicate copy of the large shared parser from the admitted
package. That is part of the execution resource contract: package image bytes
are charged at execution start. It also leaves false declarations fail closed;
the selected decoder still compares the actual body tag with the declared
kind.

DependencyBinding retains its fixed-shape whole-program path. Its fourteen
framing checks now use a bounded read pipeline that accumulates one Boolean,
and its four identity loops use the function-level Unit parameter instead of
carrying another loop value. All checks and copy ranges are unchanged.

## 3. Evidence and resources

One admitted package decodes all four supported kinds under the unchanged
limits of 100,000 instructions, 1,000,000 fuel, 1,000,000 value units, and
100,000 output units:

| Kind | Stored bytes | Fuel | Instructions | Peak value units |
|---|---:|---:|---:|---:|
| EntryPoint 16 | 153 | 27,815 | 3,488 | 599,495 |
| Namespace 3, parent present and no members | 155 | 29,130 | 3,658 | 617,836 |
| DependencyBinding 18 | 219 | 28,270 | 3,295 | 996,703 |
| PolicyBinding 17, one requirement | 186 | 41,669 | 4,685 | 873,796 |

The valid PolicyBinding assertion checks kind 17 plus the exact entity,
subject, and requirement bytes. The mismatch matrix covers declarations 3,
16, 17, and 18 against other supported stored forms, known unsupported and
unknown declarations, and a corrupt PolicyBinding digest. Envelope validation
still precedes declaration classification. The prior EntryPoint, Namespace,
and DependencyBinding semantic assertions remain in the same test.

Rejected construction studies were measured only under temporary diagnostic
ceilings that were immediately restored. A 128-call direct identity extractor
peaked at 1,399,273 value units; storing the source vector in a local cell was
worse because each `CellGet` clones the value. Neither construction remains.
The accepted graph stays below the frozen ceiling without a limit, charging,
adapter, or host-primitive change.

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

Local strict review checked the four-kind type arms, runtime mode selection,
body-tag equality, field-1 routing, native body parity, mismatch and digest
precedence, package closure, protected limits, and absence of host decoding.
Independent review was unavailable; no gate, acceptance verdict, or runtime
authority status is promoted.
