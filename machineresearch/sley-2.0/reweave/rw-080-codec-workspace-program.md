# RW-080 §1.1 program slice 24: bounded Workspace program codec — provisional C0 construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice constructs a
paired whole-program codec for entity kind 1. It is development evidence, not
accepted runtime authority. RW-080 remains BLOCKED and R2 remains NOT_READY;
independent review and acceptance debt are unchanged.

## 1. Bounded profile

The decoder accepts a complete canonical stored `EntityObject`: SCB1 envelope,
contract 200, epoch `[9; 32]`, object digest, two-field outer record, exact
32-byte entity identity and a Workspace body. Its admitted body profile is:

- kind-1 union tag and five ordered field frames;
- canonical empty `packages`, `capability_requirements`, `contracts` and
  `tests` sets; and
- one arbitrary exact 32-byte `root_namespace` identity.

The successful value is `(entity_id, canonical_body)`. The paired encoder
requires that same 49-byte body witness, checks the entity width, reconstructs
the exact contract-200 object preimage, invokes the frozen RHW1 row for its
digest, and returns byte-identical stored bytes. The checker skips only the
root namespace's 32 dynamic bytes while comparing every union, record, field,
length and empty-set byte.

Nonempty Workspace sets are outside this bounded profile and return
`SSMC_RESERVED_FIELD_PRESENT`. A same-length body with a changed union tag
returns the same typed scope refusal after a valid envelope and digest.
Encoder entity widths retain the strict fixed-width diagnostics: 31 bytes
returns `SCB_LENGTH_OVERFLOW`.

## 2. Aggregate capacity finding

The existing five-kind supported decoder was measured with Workspace added to
the same image. Its tight DependencyBinding path rose from 999,972 units to
1,001,387 units under an otherwise unchanged execution, exceeding the frozen
one-million-unit limit. A shared Workspace/Package comparator reduced the
increase but did not eliminate it. The study was removed from the delivered
image; no temporary limit remains.

This slice therefore lands the complete Workspace program codec as separately
admitted decode and encode images. The five-kind aggregate dispatcher remains
unchanged. Its dependency callee now constructs the final supported sum
directly, removing one intermediate result layer and preserving its valid path
at 999,848 units. Aggregate kind-1 dispatch remains the next image-size and
representation optimization; this record does not overclaim it.

No host semantic codec, unchecked body path, charging change or larger
execution limit was introduced. Imports remain the frozen HOST_ABI_V2 B2V1,
PSH1, V2B1 and RHW1 rows.

## 3. Evidence and resources

The native reference builds a 162-byte empty-set Workspace with nonuniform
entity and root identities. The Sley decoder returns its exact entity and
49-byte body; the separately admitted Sley encoder recreates all 162 bytes.
Four 195-byte fixtures independently populate packages, capability
requirements, contracts and tests and each reaches the typed scope refusal.
The suite also covers a valid-digest, same-length malformed body and a short
encoder entity identity.

Under the unchanged protected limits:

| Path | Stored bytes | Fuel | Instructions | Peak value units |
|---|---:|---:|---:|---:|
| Workspace decode, empty sets | 162 | 24,890 | 3,040 | 564,046 |
| Workspace encode, empty sets | 162 | 17,386 | 1,882 | 581,004 |
| Workspace decode, one populated set | 195 | 30,575 | 3,379 | 820,091 |
| Aggregate DependencyBinding regression guard | 219 | 28,270 | 3,295 | 999,848 |

The protected limits remain 100,000 instructions, 1,000,000 fuel, 1,000,000
value units and 100,000 output units.

## 4. Validation and review state

Validation for this slice:

- `cargo test -p sley-vm --test rw080_codec_program_outer` (63 tests)
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

Local strict review checks native byte identity, envelope/digest precedence,
the exact dynamic range, every empty-set boundary, same-length malformed body,
entity width, adapter confinement, capacity behavior and unchanged protected
limits. Independent review is unavailable in this session, so no gate,
acceptance verdict or runtime-authority state is promoted.
