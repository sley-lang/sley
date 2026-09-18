# RW-080 §1.1 program slice 17: PolicyBinding program encode composition — provisional C0 construction record

Status: PROVISIONAL (2026-09-17, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice adds kind 17 to
the supported whole-program encode entry. It is development evidence, not
accepted runtime authority. RW-080 remains BLOCKED and R2 remains NOT_READY;
independent review and acceptance debt are unchanged.

## 1. Scope and semantic arm

`encode_supported_program(17, value, unit)` now accepts the same compact
four-kind semantic sum returned by the supported decoder:

```text
Result<
  Result<
    EntryPoint(entity_id, function, exposure),
    TaggedEntitySet(kind, entity_id, subject_or_parent, requirements_or_members)
  >,
  DependencyBinding(entity_id, root, package, namespace)
>
```

The entity-set tag must equal the declared kind. Kind 3 routes the last two
byte fields to Namespace parent and members; kind 17 routes them to
PolicyBinding subject and requirements. A tag/declaration mismatch or any
other semantic-arm mismatch returns `SSMC_RESERVED_FIELD_PRESENT` before a
body encoder is called.

The accepted kind-17 path validates the subject as exactly 32 bytes, validates
requirements as a whole, ascending, duplicate-free sequence of 32-byte entity
ids, emits the canonical PolicyBinding body, wraps it in the canonical entity
record and envelope, and emits the digest entirely through the Sley graph.

## 2. Shared entity-set encoder

Namespace and PolicyBinding use one admitted runtime-mode body encoder and one
program wrapper. The dispatcher passes the already checked kind into that
graph. A zero-length field 1 is accepted only for Namespace. A present
Namespace parent gains the canonical option prefix; a PolicyBinding subject
is copied directly. The completed field-1 payload length selects the canonical
record length byte and, at final assembly, the union tag. Field 2 retains the
shared ordered entity-set validation and emission pipeline.

The dynamic field-1 copy terminates on the first missing vector element after
the exact 32- or 34-byte payload. This avoids a repeated late length read and
preserves the same bytes as the standalone encoders. The supported image uses
the current counted parent and digest-copy mechanisms, whose dedicated F7/F8
regression tests independently prove byte parity with the unrolled forms. The
historical anomaly records remain evidence of the earlier mechanisms; this
slice does not rewrite them.

The admitted image still contains DependencyBinding's specialized fixed-shape
program encoder. Imports remain the frozen B2V1, PSH1, V2B1 and RHW1 rows. No
limit, charging rule, host primitive, schema rule or authority state changed.

## 3. Evidence, refusals and resource envelope

One admitted encode package emits native canonical bytes for all four kinds.
A separately admitted decode package feeds each decoded arm directly to the
encoder and recovers the exact original bytes. The Namespace fixture has a
present parent, exercising the Namespace branch of the shared encoder; the
PolicyBinding fixture has an empty requirements set, exercising kind 17 under
the protected value-unit limit.

| Kind | Stored bytes | Fuel | Instructions | Peak value units |
|---|---:|---:|---:|---:|
| EntryPoint 16 | 153 | 22,504 | 2,845 | 919,100 |
| Namespace 3, parent present, no members | 155 | 26,233 | 3,380 | 984,804 |
| PolicyBinding 17, no requirements | 153 | 25,705 | 3,314 | 956,464 |
| DependencyBinding 18 | 219 | 3,757 | 1,031 | 387,836 |

The established F5 size envelope remains explicit. In this combined generic
package, adding one PolicyBinding requirement produces a canonical 186-byte
object shape but reaches `ResourceLimit(ValueUnits)` first (30,235 fuel, 3,761
instructions, 998,042 recorded peak before the refused allocation). The
standalone PolicyBinding encoder still proves zero-, one- and three-requirement
body parity. This slice does not claim that the generic whole-program path
widens the supported size envelope.

The mismatch matrix covers kinds 3, 16, 17 and 18 in the wrong semantic arms,
including kind-3/kind-17 tagged-arm swaps. Policy-specific refusals cover
short and long subjects, a partial requirement, duplicate requirements and
descending requirements with the exact `SCB_*` codes. Prior DependencyBinding
field precedence, known-unsupported kinds and unknown kinds remain covered.

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

Local strict review checks the four-kind sum identity, declaration/tag
equality, both runtime entity-set modes, exact native bytes, malformed semantic
precedence, deterministic size refusal, adapter confinement and unchanged
protected limits. Independent review is unavailable; no gate, acceptance
verdict or runtime authority state is promoted.
