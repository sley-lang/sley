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
round, a fourth after the c04539b9 round, and a fifth after the db53894e
round, whose lanes found the recursive driver charging listed child i one
nesting level per preceding sibling — repaired at the driver; source commit
recorded in `canonical-s-manifest.json`) is
`3d2ed8f8bdbc38b4332c11a0cc51d3693e4ba3cdf6f2b15be623ad0a01be6148`.
Its 20,389 entity objects occupy 5,132,790 stored bytes and have ordered-bundle
SHA-256
`588543210a97caef7bac00679a24a0bfd2f1c4bede04ce84d2a4c47f5e2bf979`.
The 1,346,105-byte stored root has SHA-256
`1a31c41e1d2b846a054cac00e96716de3197de82cd148614868f9759487e9ec3`.

The closure contains 202 functions, 9,769 parameters, 3,236 blocks, 6,492
operations, 669 constants, and four adapter imports. The five generations it
replaced under RW080-ID-02 replacement semantics — the bounded-codec root
`4cbcd1eeea202d482895e70ec91f1e0d154c1b7599cc19d60cc6751e61ecfe47` (11,876
objects, 101 functions, cf210232), the pre-review arbitrary-codec root
`b1992814cfc2332215d65e1ede6a619fc2ebb8122b8105e5fc527ca8b10548c3` (18,857
objects, 189 functions, e3a50ca7), the first-repair root
`1d64fcd1298bbab91e8c42a1b673fba683e1a0e2075f560ecfc6aaf849e16c58` (20,382
objects, 203 functions, 7426bc0b), the third-repair root
`48f8af2422b3e56b2924f56b11ca07f141b87541cdba6584cf1c6e87602577dd` (20,383
objects, 202 functions, 0b7a1482), and the fourth-repair root
`5332d4d758c17c928039cd321ba593aedc5e87f8a57e43ab8e43bc5d877308b3` (20,383
objects, 202 functions, 867009de) — are recorded as `superseded_bounded_s`,
`superseded_pre_review_arbitrary_s`, `superseded_first_repair_s`,
`superseded_third_repair_s`, and `superseded_fourth_repair_s` in
`canonical-s-manifest.json`; the bounded generation stays retained behind
the `bounded_*` tests. The exact machine-readable
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
