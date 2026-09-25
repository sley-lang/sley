# json_bridge current-delta review — Ariadne (contract lane) — bridge r10 (delta 1023a31 over r9 at 43f2f5b) at scope 1023a31

Section: `json_bridge`. Field judged: `current_delta_review.ariadne`. Role: Ariadne, contract lane.

## Scope verification

`git rev-parse HEAD` = `1023a314b18834e36a7daa020fc9078807a3ac16` (matches SCOPE_SHA; branch main; working tree clean).
Summary block `json_bridge`: `contract_revision: 10`, `status: S20_420_IMPLEMENTED_REVIEW_PENDING`,
`current_delta_review: {contract_revision: 10, ariadne: PENDING, nabu: PENDING, vulcan: PENDING}`.
Prior round: revision 9 at scope `43f2f5b` (`delta_review_ariadne-43f2f5b.md`, REVISE_0_P0_0_P1_1_P2_2_P3; Nabu REVISE 1 P2 3 P3; Vulcan REVISE 1 P2 3 P3).

Delta bounded with `git diff 43f2f5b..1023a31`: `docs/spec/SMP1_JSON_BRIDGE_V1.md` (+38/-: Status rev 10 and revision-10 sentence at lines 25-32;
section 3 inclusive boundary at lines 170-173; section 8 hello-version split at lines 246-251; section 10 additive exports at lines 299-307),
`docs/adr/ADR-0034-json-bridge-boundary.md` (status "draft at revision 10", revision 9 and 10 records, date line),
`docs/WORK_PACKAGES.md:48` (revision 10 row), `docs/spec/SMP1.md:22` (composition pin bridge 10),
`scripts/check_smp1_json_bridge_contract.py` (`SPEC_REVISION = 10`; `CEILINGS` table and `FRAME_CEILING` coupling crate/contract/oracle literals),
`scripts/check_declared_limits.py` (`DERIVED` pattern, two-pass `evaluate(initializer, known)`), `evidence/build/declared-limits.json`
(declared 155 → 156, documented 2 → 4, weak 153 → 152; `MAX_JSON_TEXT_BYTES` leaves the weak list),
`crates/sley-json-bridge/src/tests.rs` (`hello_header_violations_carry_the_codec_code` pins `VersionUnsupported` for 99 and `Downgrade` for 0).
`git diff 43f2f5b..HEAD --stat -- crates/sley-json-bridge/src/lib.rs` is empty: no production code moved.

## Inputs read in full

- `docs/spec/SMP1_JSON_BRIDGE_V1.md` lines 1-35 (Status), 156-183 (section 3), 203-237 (precedence, sections 6-7), 238-262 (section 8), 277-307 (sections 9-10); section headings (no per-revision history section exists in this document; the Status paragraph is the history).
- `scripts/check_smp1_json_bridge_contract.py` (markers, `CEILINGS`, crate-marker loop); `scripts/check_smp1_json_bridge_vector.py` lines 28-36, 203-233, 288-357, 413-445, 454-500; `scripts/check_declared_limits.py` (whole file).
- `crates/sley-json-bridge/src/lib.rs` lines 20-51 (ceilings), 430-472 (`check_resources`), 812-851 (`frame_from_value_with`), 856-935 (`frame_to_json[_for_version]`, `frame_from_json[_for_version]`), 998-1015 (`hello_to_json[_versioned]`); `crates/sley-json-bridge/src/tests.rs` lines 499-547.
- `crates/sley-protocol/src/lib.rs` lines 1140-1200 (`validate_header` → `validate` → `validate_for_version`).
- `scripts/check_cli_contract.py:83-91` (`CRATE_MARKERS`); `crates/sley-cli/src/lib.rs:17,444` (`METHOD_TABLE_V2_JSON` use).
- `docs/adr/ADR-0034-json-bridge-boundary.md` lines 1-20, 53-55; `docs/WORK_PACKAGES.md:48`; `docs/audits/S20_420_JSON_BRIDGE_CLOSEOUT.md:1-6`; `docs/spec/SMP1.md:19-24,94-95,126`.
- `git log -S` for the four exports named in section 10.

## Tool results (run with `SLEY2_MASTER_GOAL=/home/dev/machineresearch/Sley2.0mastergoal.md`)

- `python3 scripts/check_smp1_json_bridge_contract.py` → `{"result": "PASS", "revision": 10, "problems": [], "status": "S20_420_IMPLEMENTED_REVIEW_PENDING", "new_stable_error_codes": 5}` exit 0 (the new `ceiling:*` checks all satisfied).
- `python3 scripts/generate_smp1_json_bridge_fixtures.py --check` → `{"drift": [], "mode": "check", "rejections": 36, "result": "PASS", "vectors": 5}` exit 0.
- `uv run --project oracle/scb1 --frozen python scripts/check_smp1_json_bridge_vector.py` → `{"result": "PASS", "methods": 41, "rejections": 36, "vectors": 5, "problems": []}` exit 0.
- `python3 scripts/check_declared_limits.py --check` → `{"result": "PASS", "declared_limits": 156, "undocumented_limits": 0, "weak_limits": 152}` exit 0. Re-running the module's own `evaluate`/`check_constant` in-process: `MAX_JSON_TEXT_BYTES` → 268435456 `documented`, `MAX_JSON_ELEMENTS` → 1048576 `documented`, `MAX_JSON_DEPTH` → 32 `weak`, `MAX_JSON_NUMBER` → `weak`; the tracked census matches.
- `python3 scripts/test_smp1_json_bridge_table.py` → 9 cases PASS. `python3 scripts/check_smp1_contract.py` → PASS (revision 12; composition pin bridge 10 accepted); `python3 scripts/test_smp1_contract.py` → 14 OK.
- `python3 scripts/test_current_contract_review.py` → 6 OK (bridge cases derive revision 10 from the checker).
- `cargo test -p sley-cli -p sley-json-bridge --locked` → sley-json-bridge 11 passed, 0 failed, 1 ignored (`fixture refresh emitter`); sley-cli 29 passed; exit 0.

## Independently re-derived claims (every number and rule the revision-10 text states)

- **Section 8 hello-version split (prior P2).** Lines 246-251: version below 1 → `PROTOCOL_DOWNGRADE`, above 1 → `PROTOCOL_VERSION_UNSUPPORTED`, other session/request id/method/flags → `PROTOCOL_FRAME_INVALID`. Bridge `frame_from_value_with` (lib.rs:840-848): bounds shape (`JSON_BRIDGE_SHAPE_INVALID`) first, then `validate_header()` → `validate_for_version(1)` (protocol lib.rs:1170-1200): `< 1` Downgrade (1175), `> 1` VersionUnsupported (1178), then flag mask / failed-flag / hello-header FrameInvalid (1186-1200). Codes 40004 / 40000 / 40001. The unit test now pins `Err(protocol(VersionUnsupported))` for 99 and `Err(protocol(Downgrade))` for 0 (tests.rs:526-537) and FrameInvalid for the four header fields (509-521). The Status paragraph (lines 25-28) states the same split. Contract, crate, and native test agree; the P2 is closed.
- **"The codec judges the protocol version first" (line 248).** True of the crate (version claim precedes the hello-header check in `validate_for_version`). Not true of the independent oracle for a hello violating both: `check_smp1_json_bridge_vector.py:338-345` (`frame_from_json`) raises `PROTOCOL_FRAME_INVALID` for a nonzero session/request id/method/flags on kind 4 before `encode_frame` (413-421) applies the downgrade/unsupported split. A hello text with `protocol_version: 99` and `request_id: 1` is `PROTOCOL_VERSION_UNSUPPORTED` in the crate and `PROTOCOL_FRAME_INVALID` in the oracle. No rejection vector carries a hello with a non-1 version at all (the two version mutations are request frames), so the vector checker cannot see the divergence; the native test sets one field at a time. Single-field violations, the only cases exercised, agree everywhere. The ordering claim is therefore crate-true but oracle-unverified and oracle-contradicted (moderate).
- **Section 3 inclusive boundary (prior P4).** Lines 170-173: "1,048,576 or more value positions is `JSON_BRIDGE_RESOURCE_LIMIT` ... `positions >= MAX_JSON_ELEMENTS` refuses, so at most 1,048,576 values materialize". Crate `positions >= MAX_JSON_ELEMENTS` (lib.rs:457,463), oracle `positions >= MAX_ELEMENTS` (223,227); accepted positions ≤ 1,048,575 and the same paragraph's "at most positions + 1 values" gives ≤ 1,048,576 materialized. Section 6 line 220 "positions at and over 1,048,576" and the crate doc (lib.rs:37-44) agree. The CLI's new test drives exactly 1,048,576 positions to a refusal. Exact; closed.
- **Text ceiling.** "larger than 268,435,456 bytes": `MAX_JSON_TEXT_BYTES = 4 * (MAX_FRAME_BYTES as usize)` = 4 × 67,108,864 = 268,435,456, `text.len() > MAX` (lib.rs:431), oracle `> MAX_TEXT_BYTES = 268_435_456` (204). The checker's `CEILINGS` table couples the crate declaration line, the contract literal, and the oracle literal, plus `FRAME_CEILING` `67_108_864` in `crates/sley-protocol`; it is a string-presence coupling (the 4× arithmetic is witnessed by the census, which now evaluates the derived initializer through `MAX_FRAME_BYTES` and grades `MAX_JSON_TEXT_BYTES` `documented`, and by the two `const _: () = assert!` lines). True.
- **Depth.** "nested deeper than 32 levels": `depth > MAX_JSON_DEPTH = 32` (453), oracle `> MAX_DEPTH = 32` (220). True. (Census grade `weak` because no spec text names `MAX_JSON_DEPTH` beside the value; the checker's `CEILINGS` row is the coupling.)
- **Section 10 additive exports.** `frame_to_json_for_version` (lib.rs:886), `frame_from_json_for_version` (932), `hello_to_json_versioned` (1012), `METHOD_TABLE_V2_JSON` (61) all `pub`; introduced by `c6ebb48` / `5e3bfa1` (both `phase-3` commits, 2026-09-09) — "additively since the phase-3 slice" true. "the unversioned entrypoints keep their frozen version 1 defaults": `frame_to_json` renders under `PROTOCOL_VERSION`, `frame_from_json` → `frame_from_value` (frozen 1), `hello_to_json` → `hello_value` (v1 names). True. "which the S20-430 capable CLI calls (`scripts/check_cli_contract.py` requires them)": the CLI calls all four (`lib.rs:17,444,719-720`), but `check_cli_contract.py` `CRATE_MARKERS` (83-91) require only `hello_to_json_versioned(`, `frame_from_json_for_version(`, `frame_to_json_for_version(`; `METHOD_TABLE_V2_JSON` is required by no checker (`check_cli_rules.py` neither). Over-inclusive by one symbol (editorial). "no capable symbol is required by the stage checker": bridge `CRATE_MARKERS` name only the unversioned entrypoints, codes, `validate_header`, `is_sign_negative`. True. "the vectors and oracle stay version 1-only": oracle `encode_frame` hard-codes the version-1 split; vectors 5/36 unchanged. True.
- **Status paragraph.** "no encoding or behavior changes": `src/lib.rs` untouched. "The revision 9 history is retained as history and does not review revision 10; its new-delta review is pending": current (prior P4 closed).
- **Cross-references (prior P3 x2).** ADR-0034:3 "draft at revision 10", revision 9 and 10 records (lines 9-15), date line; WORK_PACKAGES:48 "(revision 10, 2026-09-14, ADR-0034; revision-8 delta review PASS at `a4b6029`, revision-9 delta round 2026-09-14 REVISE closed by revision 10 ...)" — `delta_review-a4b6029.md` exists with PASS, all three revision-9 lanes are REVISE. Closed. The bridge checker's `ADR_MARKERS` do not pin the revision text (unlike the CLI checker), so the ADR currency rests on review, not the gate; noted, not report-grade. `docs/audits/S20_420_JSON_BRIDGE_CLOSEOUT.md:3,5` still says revision 9 (moved with the substantive r9 delta, not with r10; pinned by no checker).
- **Unchanged, re-asserted.** Method tables 41/43 rows (checker, table test 9/9), codes 42000-42004 at lines 195-201, `ERROR_CODES_V1.md` range, SMP1 pin revision 12 (`SMP1.md:3`, spec line 34, `smp1-pin-text`), hello lists 4,096, fuzz lanes 64 KiB, retryability `never` (section 9).

## Per-item analysis

- Hello-version rule: the false code is gone; the crate, contract, Status paragraph, and native test state one split with exact codes. The word "first" adds an ordering the oracle does not implement and no vector pins — a follow-up, not a reopening (the implementation is the authority the contract describes, and every exercised case agrees).
- Inclusive boundary: one unit, one comparison, matched on both readers and driven by a CLI test. Closed.
- Ceiling coupling and census: the three literals are now gated in three places and the derived value is evaluated; refiled census matches a fresh derivation. Closed (Nabu/Vulcan items from the r9 round).
- Section 10: the additive exports are declared truthfully with one over-inclusive parenthetical.
- ADR/WP: current. Closeout: one revision behind (editorial).

```
VERDICT: PASS
SECTION: json_bridge
FIELD: current_delta_review.ariadne
SCOPE_SHA: 1023a314b18834e36a7daa020fc9078807a3ac16
FINDINGS:
[P3] [evidence-currency] docs/spec/SMP1_JSON_BRIDGE_V1.md:248 - "The codec judges the protocol version first" is true of the crate (crates/sley-protocol/src/lib.rs:1174-1178 precede the hello-header FrameInvalid at :1194-1200) but the independent oracle orders it the other way: scripts/check_smp1_json_bridge_vector.py:344-345 raises PROTOCOL_FRAME_INVALID for a nonzero hello session/request id/method/flags inside frame_from_json before encode_frame (:418-421) applies the downgrade/unsupported split, so a hello with protocol_version 99 and request_id 1 is PROTOCOL_VERSION_UNSUPPORTED in the crate and PROTOCOL_FRAME_INVALID in the oracle; no rejection vector carries a non-1 hello version and the native test varies one field at a time, so the stated precedence is pinned by nothing independent — move the oracle's hello-header check after its version split (or state the rule without "first") and add a non-1 hello-version vector
[P4] [editorial] docs/spec/SMP1_JSON_BRIDGE_V1.md:302-304 - "`scripts/check_cli_contract.py` requires them" covers three of the four named exports: CRATE_MARKERS (scripts/check_cli_contract.py:83-91) require hello_to_json_versioned, frame_from_json_for_version, and frame_to_json_for_version, while METHOD_TABLE_V2_JSON (used at crates/sley-cli/src/lib.rs:444) is required by no checker
[P4] [cross-reference] docs/audits/S20_420_JSON_BRIDGE_CLOSEOUT.md:3 - closeout status still names "(revision 9)" of the contract now at revision 10; pinned by no checker
SUMMARY: Every claim the revision-10 text adds re-derives from code: the hello protocol-version rule now names the codec's actual codes (below 1 PROTOCOL_DOWNGRADE, above 1 PROTOCOL_VERSION_UNSUPPORTED, other header fields PROTOCOL_FRAME_INVALID) and the native test pins both exact codes; the element ceiling is stated in one unit and one comparison (`positions >= 1,048,576` refuses, at most 1,048,576 values materialize) matching crate and oracle; section 10 truthfully declares the four additive version-selected exports as phase-3 additions with frozen version-1 defaults; the checker's CEILINGS table couples 268,435,456 / 32 / 1,048,576 across crate, contract, and oracle and the limits census evaluates the derived text ceiling (both bridge ceilings graded documented, census refiled and matching); ADR-0034 and WORK_PACKAGES carry revision 9 and 10 records; no production code changed and the checker, fixtures, oracle (36 rejections, 5 vectors), table tests, census, and cargo tests are green. One moderate follow-up: the new word "first" states a codec ordering the oracle contradicts for a doubly-invalid hello and no vector exercises; plus two editorial points. Nothing report-grade is open.
```
