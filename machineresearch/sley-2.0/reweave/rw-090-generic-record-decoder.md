# RW-090 canonical generic record decoder

Status: PROVISIONAL CODEC CONSTRUCTION (2026-09-18, operator development
override `rw075_correction.operator_override_2026_09_07`). This slice removes
the fixed-witness restriction from one foundational codec layer. It does not
complete the nested SSMC1 schema codec or establish official C1.

The retained Sley graph accepts arbitrary canonical SCB1 record bytes and
returns an ordered `UInt64 -> Bytes` map of the exact field payloads. It runs
the Sley uvar decoder for the field count, every field tag, and every field
length; enforces the 65,535-field and 64 MiB field bounds; detects duplicate
and descending tags in canonical precedence; copies every payload inside the
admitted byte bridges; and rejects trailing, truncated, non-minimal, and
over-limit encodings with the frozen error bytes.

The graph contains two functions, 583 parameters, 86 blocks, 166 operations,
and 43 constants. Its admitted image is 25,332 bytes and its package digest is
`ec734c391fdec9625919a9bc62445c4be2041eabd30f738b75d9dff08123f9da`.
Executable tests pin the graph surface, prove every entry block is reachable,
check every CFG edge arity, decode empty and arbitrary three-field records,
and cover duplicate, order, trailing, truncation, non-minimal-uvar, and field
limit failures.

This primitive deliberately returns raw canonical field payloads. The next
RW-090 slices must apply the frozen per-kind schemas to those payloads,
including recursive types, values, references, immediates, terminators, and
entity sets. Only then can the build driver derive checked lowering facts from
canonical state objects instead of native-prepared facts.

Validation:

- `cargo test -p sley-vm --test rw120_toolchain_integration generic_record_decoder -- --nocapture`
- `cargo test -p sley-vm --test rw120_toolchain_integration`
- `cargo clippy -p sley-vm --test rw120_toolchain_integration -- -D warnings`
- `cargo fmt --all -- --check`
