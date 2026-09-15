# json_bridge current-delta review — Ariadne (contract lane) — bridge r9 (deltas 7718391, d384f0f; const pin 43f2f5b) at scope 43f2f5b

Section: `json_bridge`. Field judged: `current_delta_review.ariadne`. Role: Ariadne, contract lane.

## Scope verification

`git rev-parse HEAD` = `43f2f5ba738b587a9a0bb365a55f3096527e3e3d` (matches SCOPE_SHA; branch main).
Summary block `json_bridge`: `contract_revision: 9`, `status: S20_420_IMPLEMENTED_REVIEW_PENDING`,
`current_delta_review: {contract_revision: 9, ariadne: PENDING, nabu: PENDING, vulcan: PENDING}`.
Previously judged: revision 8 at scope `a4b6029` (`delta_review-a4b6029.md`, PASS all lanes); final reviews
at `d384f0f` (`ariadne-final-review-d384f0f.md`). This review covers revision 8 → 9 only.

Delta bounded with `git diff a4b6029..HEAD -- docs/spec/SMP1_JSON_BRIDGE_V1.md scripts/check_smp1_json_bridge_contract.py scripts/check_smp1_json_bridge_vector.py crates/sley-json-bridge`:
- `7718391` (rev 9): spec Status paragraph rev-9 sentence; §1 bytes/integer precision (frozen-name fields are not byte strings; number-vs-string split rationale; "above 2^53-1 only the string form reads"); §1 duplicate-key rule; §3 element ceiling + 4x derivation + open-list ownership; §6 evidence bullets (element-ceiling native tests, fuzz slice 64 KiB lanes); §8 hello-rendering wrapper rule; §9 retryability `never` for RESOURCE_LIMIT rationale. Checker `SPEC_REVISION = 9`. Oracle `MAX_ELEMENTS = 1_048_576` + positions count in `check_resources`. Crate: `MAX_JSON_TEXT_BYTES = 4 * (MAX_FRAME_BYTES as usize)`, `MAX_JSON_ELEMENTS = 1_048_576`, `const _: () = assert!(MAX_JSON_ELEMENTS <= MAX_JSON_TEXT_BYTES)`, positions count in `check_resources`; tests for wide_ok/wide_over, structural bytes in strings, duplicate keys.
- `d384f0f`: spec §8 hello line "all flags false" and "The protocol version, session, request id, method, and flags are the codec's hello header rule ... any other value there is `PROTOCOL_FRAME_INVALID`, never a bridge code"; tests.rs comment + new `protocol_version: 99` assertion (`matches!(..., Err(BridgeError::Protocol(_)))`).
- `43f2f5b`: `#[allow(clippy::cast_possible_truncation)]` + `const _: () = assert!(MAX_FRAME_BYTES <= (u32::MAX as u64) / 4);` (no value change); `crates/sley-protocol/src/server.rs` rustfmt-only hunk.

## Inputs read in full

- `docs/spec/SMP1_JSON_BRIDGE_V1.md` (291 lines, Status rev 9, SMP1 pin rev 12 at line 34).
- `scripts/check_smp1_json_bridge_contract.py` (243 lines); `scripts/check_smp1_json_bridge_vector.py` lines 28-36, 203-230, 296-350, 405-430.
- `crates/sley-json-bridge/src/lib.rs` lines 23-52 (ceilings), 417-470 (`check_resources`), 815-876 (`frame_from_value_with`, `frame_to_json` hello wrapper), 945-1041 (hello render/parse); `crates/sley-json-bridge/src/tests.rs` lines 495-535, 600-640, 881.
- `crates/sley-protocol/src/lib.rs:39` (`MAX_FRAME_BYTES = 67_108_864`), `:76` (`MAX_HELLO_LIST = 4_096`), `:737-746` (hello list caps), `:1147-1190` (`validate_header` → `validate` → `validate_for_version(PROTOCOL_VERSION)`).
- `conformance/smp1-json-bridge/v1/rejected.json` (36 cases; the hello-header cases and the two version cases inspected).
- `fuzz/targets/smp1_json_bridge.rs:27` (`MAX_FUZZ_INPUT_BYTES = 65_536`); `scripts/check_smp1_json_bridge_persistent_fuzz_slice.py:34,56`.
- `docs/adr/ADR-0034-json-bridge-boundary.md` lines 1-12, 37; `docs/WORK_PACKAGES.md:48`; `docs/audits/S20_420_JSON_BRIDGE_CLOSEOUT.md` lines 3-5; `docs/spec/SMP1.md` lines 91, 194-236, 568-583.

## Tool results (run with `SLEY2_MASTER_GOAL` set)

- `python3 scripts/check_smp1_json_bridge_contract.py` → `{"result": "PASS", "revision": 9, "problems": [], "status": "S20_420_IMPLEMENTED_REVIEW_PENDING"}` exit 0.
- `uv run --project oracle/scb1 --frozen python scripts/check_smp1_json_bridge_vector.py` → `{"result": "PASS", "methods": 41, "rejections": 36, "vectors": 5, "problems": []}` exit 0.
- `python3 scripts/test_smp1_json_bridge_table.py` → 9 cases PASS. `python3 scripts/check_smp1_json_bridge_persistent_fuzz_slice.py` → PASS (`scope: SMP1_JSON_BRIDGE_TEXT_AND_FRAME_ROUND_TRIP_ONLY`).
- `python3 scripts/test_current_contract_review.py` → bridge cases (`BridgeValidBaseline`, `BridgeTerminalAcceptance`, revision 9) pass; the 2 failures in that run are CLI cases (revision 6 fixtures) and belong to the `cli` section.
- `cargo test -p sley-cli -p sley-json-bridge --locked` → sley-json-bridge 11 passed, 0 failed, 1 ignored (`#[ignore = "fixture refresh emitter"]` at tests.rs:881 — not a behaviour test; the element-ceiling, string-structural, and duplicate-key assertions live inside `the_rejection_matrix_reports_the_contract_codes_in_precedence`, which ran and passed); exit 0.

## Independently re-derived claims (every number and rule in the revision-9 text)

- Text ceiling: `MAX_JSON_TEXT_BYTES = 4 * (67_108_864 as usize)` = 268,435,456 — matches §3 "268,435,456 bytes" and "four times the absolute frame ceiling"; oracle `MAX_TEXT_BYTES = 268_435_456` with the `4 * 67_108_864` comment. "Derived in code so the relationship is compiler-checked": the constant is an expression over `MAX_FRAME_BYTES`, and `43f2f5b` adds `const _: () = assert!(MAX_FRAME_BYTES <= (u32::MAX as u64) / 4)` pinning the cast. True.
- Depth: `MAX_JSON_DEPTH = 32`, oracle `MAX_DEPTH = 32`, §3 "32 levels". True.
- Element ceiling: `MAX_JSON_ELEMENTS = 1_048_576`, oracle `MAX_ELEMENTS = 1_048_576`, §3 "1,048,576". Counting rule "every `{`, `[`, `,`, and `:` outside strings" matches `check_resources` (lib.rs:453-465: `{`/`[` add a position after the depth check; `,`/`:` add a position; strings skipped with escape tracking) and the oracle (lines 216-228). "At most positions + 1 values materialize": verified by hand on `{"a":1,"b":2}` (4 positions, 5 materialized values incl. keys), `[1,2]` (2, 3), scalar (0, 1). True. `const _: () = assert!(MAX_JSON_ELEMENTS <= MAX_JSON_TEXT_BYTES)` present.
- Boundary wording: §3 says a text "holding **more than** 1,048,576 value positions" is rejected, but both readers fire at `positions >= MAX` (lib.rs:458,463; oracle 222,227), and the native test rejects `wide_over` (exactly 1,048,576 positions) while accepting `wide_ok` (1,048,575). Under the crate-doc reading (lib.rs:35-36: the ceiling is the number of materialized values, which is positions + 1) the sentence is exact; under §3's own definition (the ceiling "counts every `{`, `[`, `,`, and `:`") it is off by one. The two readings coexist in the same paragraph. Editorial precision, not a divergence (crate and oracle agree; §6 says "positions at and over 1,048,576").
- Open-list ownership: "hello lists capped at 4,096, judged with `PROTOCOL_*`" — `MAX_HELLO_LIST = 4_096` enforced in `Hello::validate` (protocol lib.rs:737-746). True.
- Duplicate keys: `serde_json 1.0.145` `Map` (no `preserve_order` feature in `crates/sley-json-bridge/Cargo.toml`) inserts last-wins; Python `json.loads` dict last-wins; native test injects a duplicate `protocol_versions` and parses back to the fixture. True.
- Hello rendering wrapper (§8 new bullet): `frame_to_json` (lib.rs:860-876) builds `protocol_version: PROTOCOL_VERSION, session: None, request_id: 0, kind: Hello, method: 0, flags: 0, bounds: none(), body: hello.encode()` — exactly the stated wrapper. True.
- Fuzz slice: `MAX_FUZZ_INPUT_BYTES = 65_536` in `fuzz/targets/smp1_json_bridge.rs:27` and pinned by the slice checker — "64 KiB lanes" true; every ceiling (256 MiB, depth 32 is reachable but the wide/element ceilings are not at 64 KiB) is covered by native tests as §6 claims.
- Retryability `never` for `RESOURCE_LIMIT`: `BridgeError::envelope` unchanged; consistent with SMP1 §9 (pre-existing).
- Method tables: v1 41/37, v2 43/39 (checker re-asserts counts and union; table test 9/9). True, unchanged from r8.
- SMP1 pin: `docs/spec/SMP1.md:3` revision 12; spec line 34 "(revision 12)"; checker `smp1-revision-pin` and `smp1-pin-text` both enforced. True.
- Codes 42000-42004 rows at lines 190-194; `ERROR_CODES_V1.md` range sentence (checker PASS). True.
- **Hello-version rule (d384f0f) vs the server/codec.** §8 lines 236-241 now say the protocol version of a hello Frame is judged by the codec's hello header rule and "any other value there is `PROTOCOL_FRAME_INVALID`, never a bridge code". The bridge reader calls `frame.validate_header()` (lib.rs:848) → `validate()` → `validate_for_version(PROTOCOL_VERSION)` (protocol lib.rs:1170-1185), which returns `PROTOCOL_DOWNGRADE` for `protocol_version < 1` and `PROTOCOL_VERSION_UNSUPPORTED` for `> 1` **before** the hello-kind `FrameInvalid` check at line 1183 (which is unreachable on the frozen path because the version must already equal 1 to get there). So a hello text with `protocol_version: 99` yields `PROTOCOL_VERSION_UNSUPPORTED`, and `0` yields `PROTOCOL_DOWNGRADE`, never `PROTOCOL_FRAME_INVALID`. The oracle agrees with the crate (`encode_frame` lines 418-421 apply the same split; its hello check at 341-345 covers session/request id/method/flags only). The delta's own unit test (tests.rs:522-528) asserts only `Err(BridgeError::Protocol(_))` — "any PROTOCOL_* code" — and no rejection vector carries a hello with a non-1 version (the two version vectors are `request` frames). The contract sentence therefore states a stable code the implementation does not produce, and it contradicts the document's own Status paragraph (lines 11-14: below the selection `PROTOCOL_DOWNGRADE`, above it `PROTOCOL_VERSION_UNSUPPORTED`). An independent reader implementing §8 literally emits a different failure code than the crate and the oracle for the same text.
- Cross-references: `docs/adr/ADR-0034-json-bridge-boundary.md:3` "the S20-420 contract is a draft at revision 8" with only a revision-8 record; `docs/WORK_PACKAGES.md:48` "(revision 8, 2026-09-08, ADR-0034, ...)". The closeout was moved to revision 9 (`S20_420_JSON_BRIDGE_CLOSEOUT.md:3,5`), the ADR and work-package row were not. The r8 precedent verified the ADR/WP records as part of the delta.
- Status paragraph lines 25-26: "The revision 7 history is retained as history and does not review revision 8; its new-delta review is pending" — stale: the r8 delta review passed at `a4b6029`; it is revision 9 that is pending.

## Per-item analysis

- 7718391 (element ceiling, 4x derivation, precision): numbers, rules, tests, and oracle all match; only the "more than"/"at" unit ambiguity remains (editorial).
- d384f0f (hello-version rule, flags currency): "all flags false" is correct (three flags). The protocol-version sentence names the wrong code against `crates/sley-protocol` and the oracle; the test was written not to pin it. Report-grade (major): a declared stable code in a contract rule is false.
- 43f2f5b (const assertion): value unchanged; claim "compiler-checked" now literally true. No finding.
- ADR-0034 / WORK_PACKAGES not moved to 9: moderate cross-reference currency.

```
VERDICT: REVISE_0_P0_0_P1_1_P2_2_P3
SECTION: json_bridge
FIELD: current_delta_review.ariadne
SCOPE_SHA: 43f2f5ba738b587a9a0bb365a55f3096527e3e3d
FINDINGS:
[P2] [contract-falsity] docs/spec/SMP1_JSON_BRIDGE_V1.md:236-241 - the revision-9 hello rule says a hello Frame's protocol version, like its session/request id/method/flags, fails as `PROTOCOL_FRAME_INVALID`; the reader's `validate_header` (crates/sley-json-bridge/src/lib.rs:848) runs `validate_for_version(1)` (crates/sley-protocol/src/lib.rs:1174-1179), which returns `PROTOCOL_DOWNGRADE` (< 1) or `PROTOCOL_VERSION_UNSUPPORTED` (> 1) before the hello-kind FrameInvalid check at :1183 can fire; the oracle (scripts/check_smp1_json_bridge_vector.py:418-421) agrees with the crate, the unit test (tests.rs:522-528) asserts only the PROTOCOL_* family, no vector covers a non-1 hello version, and the sentence contradicts the Status paragraph's own split (lines 11-14)
[P3] [cross-reference] docs/adr/ADR-0034-json-bridge-boundary.md:3 - ADR still says "a draft at revision 8" with a revision-8 record only; no revision-9 record although the closeout was moved to 9
[P3] [cross-reference] docs/WORK_PACKAGES.md:48 - S20-420 row still reads "(revision 8, 2026-09-08, ADR-0034, ...)" after the revision-9 status move
[P4] [editorial] docs/spec/SMP1_JSON_BRIDGE_V1.md:163-172 - "more than 1,048,576 value positions" is rejected, yet the same paragraph defines the count as structural characters and both readers reject at exactly 1,048,576 positions (`>=`, lib.rs:458/463, oracle :222/227; native test `wide_over`); state the boundary in one unit (values materialized = positions + 1, ceiling inclusive) so an independent reader cannot accept a text the crate rejects
[P4] [editorial] docs/spec/SMP1_JSON_BRIDGE_V1.md:25-26 - "does not review revision 8; its new-delta review is pending" is stale (the r8 delta review passed at a4b6029; revision 9 is the pending delta)
SUMMARY: Every number the revision-9 text states re-derives from code: MAX_JSON_TEXT_BYTES = 4 * MAX_FRAME_BYTES = 268,435,456 with the cast pinned by a const assertion, depth 32, element ceiling 1,048,576 counted over `{ [ , :` outside strings with the positions + 1 materialization bound, hello lists 4,096 owned by the codec, last-wins duplicate keys on both readers, the synthesized hello wrapper in frame_to_json, and 64 KiB fuzz lanes; the checker, oracle (36 rejections, 5 vectors), table tests, fuzz-slice checker, and cargo tests are green. The delta fails the contract lane on one false rule it introduced: section 8 now promises `PROTOCOL_FRAME_INVALID` for a hello's protocol version while the codec path the bridge calls answers `PROTOCOL_DOWNGRADE`/`PROTOCOL_VERSION_UNSUPPORTED` (the oracle agrees with the code, and the delta's test deliberately pins only the family). Correct that sentence to the split the Status paragraph already states (and add a hello-version vector), move ADR-0034 and WORK_PACKAGES to revision 9, and tidy the two editorial points; then the field can pass.
```
