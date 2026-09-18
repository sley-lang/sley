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

The frozen root (re-minted 2026-09-18 with the arbitrary RW-090 codec,
source commit `e3a50ca7`) is
`b1992814cfc2332215d65e1ede6a619fc2ebb8122b8105e5fc527ca8b10548c3`.
Its 18,857 entity objects occupy 4,747,089 stored bytes and have ordered-bundle
SHA-256
`7e10c8a2474fd6d1aca295f025d06562197a061880343ec47802b196bcf48249`.
The 1,244,993-byte stored root has SHA-256
`5fbcdf19be9761daa5dd3ccc617cdf00fdfb68d0ec6d43ad7ccf6cd59ed7bcef`.

The closure contains 189 functions, 8,748 parameters, 3,077 blocks, 6,146
operations, 676 constants, and four adapter imports. The bounded-codec
generation it replaced — root
`4cbcd1eeea202d482895e70ec91f1e0d154c1b7599cc19d60cc6751e61ecfe47`, 11,876
objects, 101 functions — is recorded as `superseded_bounded_s` in
`canonical-s-manifest.json` and retained behind the `bounded_*` tests. The exact machine-readable
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
