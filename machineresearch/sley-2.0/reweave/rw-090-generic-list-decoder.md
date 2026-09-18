# RW-090 canonical generic list decoder

Status: PROVISIONAL CODEC CONSTRUCTION (2026-09-18, operator development
override `rw075_correction.operator_override_2026_09_07`). This slice
completes the schema-independent record/union/list parser trio. It does not
complete the schema-specific SSMC1 codec or establish official C1.

The retained Sley graph accepts arbitrary canonical SCB1 list bytes and
returns an ordered `UInt64 -> Bytes` map keyed by zero-based element ordinal.
It decodes the runtime element count and every element length through the Sley
uvar implementation, enforces the 65,535-element and 64 MiB payload bounds,
copies each payload through admitted byte bridges, and requires exact input
consumption. Empty-input, trailing, truncated, non-minimal-count,
element-limit, and payload-limit cases return the frozen structural errors.

The graph contains two functions, 505 parameters, 78 blocks, 153 operations,
and 40 constants. Its admitted image is 22,882 bytes and its package digest is
`9685540044fec173ea4e93cc7e332440c9921853911e1ea04b1543a200dc13fe`.
Tests pin those facts and the full CFG surface, then decode a runtime
three-element list containing nonempty and empty payloads in exact order.

Record, union, and list framing can now be decoded without fixed fixture
digests. The next layer must compose these primitives into schema-specific
decoders and validate primitive leaves and exact field/tag sets before the
checker or lowerer consumes them.

Validation:

- `cargo test -p sley-vm --test rw120_toolchain_integration generic_list_decoder -- --nocapture`
- `cargo clippy -p sley-vm --test rw120_toolchain_integration -- -D warnings`
- `cargo fmt --all -- --check`
