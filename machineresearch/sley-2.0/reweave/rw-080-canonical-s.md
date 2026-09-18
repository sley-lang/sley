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
again the same day after the 178873d7 Council repairs — strict kind-18
route, native nesting-depth charging, native-parity refusal tests — and a
third time after the 92fa6646 re-review round, which moved the constant
projector's `value_type` charge to the node depth and pinned the residual
deviations RW090-DEV-01..03; source commit recorded in
`canonical-s-manifest.json`) is
`48f8af2422b3e56b2924f56b11ca07f141b87541cdba6584cf1c6e87602577dd`.
Its 20,383 entity objects occupy 5,131,404 stored bytes and have ordered-bundle
SHA-256
`d7d2fcb57d19676c269559bc2a938c04ada682a46b4c34f197b5126901ded25a`.
The 1,345,709-byte stored root has SHA-256
`76235656984ec59963b8b39a425cb6af721069a25cafca0b6988dfea3ffb2e8c`.

The closure contains 202 functions, 9,763 parameters, 3,236 blocks, 6,492
operations, 669 constants, and four adapter imports. The three generations it
replaced under RW080-ID-02 replacement semantics — the bounded-codec root
`4cbcd1eeea202d482895e70ec91f1e0d154c1b7599cc19d60cc6751e61ecfe47` (11,876
objects, 101 functions, cf210232), the pre-review arbitrary-codec root
`b1992814cfc2332215d65e1ede6a619fc2ebb8122b8105e5fc527ca8b10548c3` (18,857
objects, 189 functions, e3a50ca7), and the first-repair root
`1d64fcd1298bbab91e8c42a1b673fba683e1a0e2075f560ecfc6aaf849e16c58` (20,382
objects, 203 functions, 7426bc0b) — are recorded as `superseded_bounded_s`,
`superseded_pre_review_arbitrary_s`, and `superseded_first_repair_s` in
`canonical-s-manifest.json`; the bounded generation stays retained behind the
`bounded_*` tests. The exact machine-readable
record is `canonical-s-manifest.json`; `check_reweave_canonical_s.py` pins its
facts, constructor source digest, and binding into `bootstrap-manifest.json`.
The construction test independently rebuilds every object and root byte,
round-trips them through the frozen decoders, and executes the root-bound
handoff driver.

Validation:

- `python scripts/check_reweave_canonical_s.py`
- `cargo test -p sley-vm --test rw120_toolchain_integration integrated_driver_hands_lowered_bytes_to_the_package_builder -- --nocapture`

The integrated codec judges arbitrary canonical bodies on both program legs
(re-mint of 2026-09-18), but the clean build request still receives
native-prepared lowering facts, the integrated checker and lowerer remain
bounded (RW-100, RW-110), and the RW-090 re-review of the second repair is
open. Those
gaps prevent C1, C2/C3, fixed-point, or self-hosting claims. Freezing `S`
makes them testable against one exact immutable input; it does not erase
them.
