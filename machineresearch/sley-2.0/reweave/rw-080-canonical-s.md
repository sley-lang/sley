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
again after the 178873d7 Council repairs, a third time after the 92fa6646
round, and a fourth time after the c04539b9 round, whose per-arm boundary
sites found the Map constant arm charging one nesting level too many;
source commit recorded in `canonical-s-manifest.json`) is
`5332d4d758c17c928039cd321ba593aedc5e87f8a57e43ab8e43bc5d877308b3`.
Its 20,383 entity objects occupy 5,131,404 stored bytes and have ordered-bundle
SHA-256
`bee7da94a659c31366ca81fe634cd84d5aa9e5b2108bd56d5d1112cda55e3a5e`.
The 1,345,709-byte stored root has SHA-256
`35256f888669cb2ce6cc446b3bc955b75e58ea6376202b3b5a20d3a1152af098`.

The closure contains 202 functions, 9,763 parameters, 3,236 blocks, 6,492
operations, 669 constants, and four adapter imports. The four generations it
replaced under RW080-ID-02 replacement semantics — the bounded-codec root
`4cbcd1eeea202d482895e70ec91f1e0d154c1b7599cc19d60cc6751e61ecfe47` (11,876
objects, 101 functions, cf210232), the pre-review arbitrary-codec root
`b1992814cfc2332215d65e1ede6a619fc2ebb8122b8105e5fc527ca8b10548c3` (18,857
objects, 189 functions, e3a50ca7), the first-repair root
`1d64fcd1298bbab91e8c42a1b673fba683e1a0e2075f560ecfc6aaf849e16c58` (20,382
objects, 203 functions, 7426bc0b), and the third-repair root
`48f8af2422b3e56b2924f56b11ca07f141b87541cdba6584cf1c6e87602577dd` (20,383
objects, 202 functions, 0b7a1482) — are recorded as `superseded_bounded_s`,
`superseded_pre_review_arbitrary_s`, `superseded_first_repair_s`, and
`superseded_third_repair_s` in `canonical-s-manifest.json`; the bounded
generation stays retained behind the `bounded_*` tests. The exact machine-readable
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
bounded (RW-100, RW-110), and the RW-090 re-review of this generation is
open. Those
gaps prevent C1, C2/C3, fixed-point, or self-hosting claims. Freezing `S`
makes them testable against one exact immutable input; it does not erase
them.
