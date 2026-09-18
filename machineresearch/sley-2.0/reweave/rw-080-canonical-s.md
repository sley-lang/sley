# RW-080 canonical Sley toolchain input `S`

Status: CANONICAL INPUT FROZEN (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This record establishes the
RW-080 graph-root, construction-manifest, and no-source-DSL gate. It does not
promote later codec, checker, lowerer, fixed-point, succession, or release
claims.

The deterministic C0 seed assembler constructs the retained codec, checker,
lowerer, package builder, and handoff driver directly as typed SSMC1 entities.
There is no Sley text parser or source DSL. The state root binds the complete
dependency closure, five local entry points, the frozen schema epoch, contract
and test anchors, and the four allowlisted bridge identities. It contains no
prebuilt C2 or C3 image.

The frozen root is
`4cbcd1eeea202d482895e70ec91f1e0d154c1b7599cc19d60cc6751e61ecfe47`.
Its 11,876 entity objects occupy 2,971,077 stored bytes and have ordered-bundle
SHA-256
`c40eab81641a843f4983c603cd3483bfcca7a3488658b053576e883deb0d336d`.
The 784,246-byte stored root has SHA-256
`85f94269365956705d3d7206ca2aa9a65099da1d55141fe06e3b60f0ec93bbde`.

The closure contains 101 functions, 5,148 parameters, 1,849 blocks, 4,058
operations, 699 constants, and four adapter imports. The exact machine-readable
record is `canonical-s-manifest.json`; `check_reweave_canonical_s.py` pins its
facts, constructor source digest, and binding into `bootstrap-manifest.json`.
The construction test independently rebuilds every object and root byte,
round-trips them through the frozen decoders, and executes the root-bound
handoff driver.

Validation:

- `python scripts/check_reweave_canonical_s.py`
- `cargo test -p sley-vm --test rw120_toolchain_integration integrated_driver_hands_lowered_bytes_to_the_package_builder -- --nocapture`

The integrated codec still uses bounded fixed profiles for graph-bearing body
kinds, and the clean build request still receives native-prepared lowering
facts. Those are RW-090 through RW-120 implementation gaps and prevent C1,
C2/C3, fixed-point, or self-hosting claims. Freezing `S` makes those gaps
testable against one exact immutable input; it does not erase them.
