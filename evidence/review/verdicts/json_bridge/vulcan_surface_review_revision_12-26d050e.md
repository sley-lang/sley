<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: vulcan; field: vulcan_surface_review_revision_12; dispatched: 2026-09-23T13:46:04Z; duration_s: 497; process_exit_code: 0 -->
# Vulcan Council review — json_bridge

Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

This is an independent Vulcan delta review of `docs/spec/SMP1_JSON_BRIDGE_V1.md` revision 12, covering range `2b0f1c9f..26d050e6`.
- I did not open any other lane's revision-12 transcript.
- A shared memory note from a concurrent Ariadne session on the same scope was visible to me. Every item reported below I checked myself against code or checker output.

What I ran and read:
- **Scope.** `git rev-parse HEAD` returned `26d050e629b669ef4acbd54826ef4008c05a061d`, which matches. Branch is `work/succ-context-impl`.
- **Git reads:**
  - `git diff --stat 2b0f1c9f..HEAD`.
  - `git diff 2b0f1c9f..HEAD` on the package's paths: the bridge spec, ADR-0034, the S20-420 closeout, `check_smp1_json_bridge_contract.py`, `test_smp1_json_bridge_contract.py` (new), `check_smp1_json_bridge_vector.py`, `generate_smp1_json_bridge_{table,fixtures}.py`, `conformance/smp1-json-bridge/v1/{rejected.json,SHA256SUMS}`, `crates/sley-json-bridge/src/tests.rs`, the Makefile, and `docs/spec/SMP1.md`.
  - The S20-420 row of `docs/WORK_PACKAGES.md`.
  - `git diff --stat` over `crates/sley-json-bridge`, `sley-protocol/src/lib.rs`, `sley-scb1`, `sley-id`, the v1 `methods.json` and `roundtrip.json`, v2/v3, and the fuzz-slice checker. The only change is `tests.rs` (+40), so the bridge `lib.rs` and the codec are byte-for-byte unchanged.
  - In-process diff of the `json_bridge` machine-summary section against `2b0f1c9f`.
- **Checkers and tests (exit codes):**
  - `python3 scripts/check_smp1_json_bridge_contract.py`: exit 0, `"result": "PASS"`, `"revision": 12`, `"status": "S20_420_IMPLEMENTED_REVIEW_PENDING"`, `"problems": []`.
  - `check_smp1_json_bridge_vector.py`, run through `uv run --offline --frozen --project oracle/scb1`: exit 0, PASS (methods 41, vectors 5, rejections 37).
  - `check_smp1_vector.py`, same way: exit 0, PASS (frames 3, mutations 4).
  - `generate_smp1_json_bridge_table.py --check --protocol-version 1/2/3`: PASS each.
  - `test_smp1_json_bridge_table.py`: 19 cases, all PASS.
  - `test_smp1_json_bridge_contract.py -v`: 4 tests, OK.
  - `check_smp1_json_bridge_persistent_fuzz_slice.py`: PASS.
  - `check_smp1_contract.py`: PASS, revision 15.
  - In-process SHA256SUMS check of v1/v2/v3: 5 files OK.
  - `cargo test --locked --offline -p sley-json-bridge --lib`: 12 passed, 0 failed, 1 ignored. This includes the new `the_version_3_hello_rendering_is_render_only`.
  - The ignored emitter `emit_smp1_json_bridge_vectors_for_fixture_refresh -- --ignored --nocapture`, parsed in Python: the crate emits 37 rejections, identical to `rejected.json` in order, text and code. The 5 round-trip vectors are identical to `roundtrip.json`.
- **In-memory oracle probes** (uv subprocess; base oracle loaded from `git show 2b0f1c9f:` and exec'd):

  | Probe | Head oracle | Base oracle |
  |---|---|---|
  | new vector (v99, request_id 1) | `VERSION_UNSUPPORTED` | `FRAME_INVALID` |
  | v0 + request_id 1 | `DOWNGRADE` | `FRAME_INVALID` |
  | v0 + session | `DOWNGRADE` | `FRAME_INVALID` |
  | v2 + cancel | `VERSION_UNSUPPORTED` | `FRAME_INVALID` |
  | v99 + nonzero bounds | `SHAPE_INVALID` (both) | |
  | v99 + unknown method | `METHOD_UNKNOWN` (both) | |
  | v99 + bad hex | `HEX_INVALID` (both) | |
  | v99 + undecodable body | `VERSION_UNSUPPORTED` (both) | |

  - Only `hello-version-above-with-request-id` differs between head and base across the 37 vectors.
  - Mutation test: removing only the head's `< 1` downgrade branch still passes all 37 vectors, but misclassifies v0 + request_id 1 as `FRAME_INVALID`.
- **In-memory checker negative probes** (patching `chk.read`):

  | Probe | Result |
  |---|---|
  | baseline | PASS |
  | second composition sentence in history (r15) with the normative one at r14 | FAIL `composition-sentence:['14','15']` |
  | ADR status set to r11 (r12 record kept) | FAIL `adr-current-revision` |
  | ADR r12 record dropped | FAIL `adr-revision-record` |
  | §11 heading removed | FAIL `spec-marker` |
  | render-only sentence removed | FAIL `spec-marker` |
  | vector-pin sentence removed | FAIL `spec-marker` |
  | `frame_value_for_version` dropped from §11 | FAIL `spec-marker` |
  | crate `"native_tests"` renamed | FAIL `crate-marker` |
  | crate `pub fn frame_value_for_version` made private | FAIL `crate-marker` |

- **Static comparisons:**
  - `conformance/smp1-json-bridge/v3/methods.json` equals `Method::V3_ALL`: 46 names in order, and tags equal, including the 7 constant-backed tags 306/307/601/602/605–607.
  - `FEATURE_NATIVE_TESTS_V1 = 32` (sley-protocol `lib.rs:74`).
  - Table row keys are exactly `family`, `name`, `owner`, `reserved`, `tag`.
  - Spec lines 1–60 and 312–363 are at most 76 characters.
- **Files read:**
  - `docs/spec/SMP1_JSON_BRIDGE_V1.md:1-363` (all)
  - `docs/adr/ADR-0034-json-bridge-boundary.md:1-70` (all)
  - `scripts/check_smp1_json_bridge_contract.py:150-364` plus the diff hunks
  - `scripts/check_smp1_json_bridge_vector.py:1-58, 300-515`
  - `scripts/generate_smp1_json_bridge_table.py:95-124`
  - `crates/sley-json-bridge/src/lib.rs:255-300, 653-667, 729-770, 780-1219`
  - `crates/sley-json-bridge/src/tests.rs:357-383, 512-557, 602-627, 903-949`
  - `crates/sley-protocol/src/lib.rs:1304-1364, 560-599, 3378-3437`
  - `scripts/check_cli_contract.py:100-129`
  - `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:880-959` (§13.3–13.5)
  - `evidence/review/verdicts/json_bridge/vulcan_surface_review_revision_11-2b0f1c9.md` (my lane's prior transcript)

## Evidence checked

- **Prior findings (round 7, `vulcan_surface_review_revision_11-2b0f1c9.md`):**
  - CLOSED — [P3] [repin-completeness] ADR-0034 stuck at revision 10, generator comment at SMP1 13, revision-agnostic ADR markers.
    - ADR-0034:3 now reads "draft at revision 12", and :17-24 carry the revision 11 and 12 records.
    - The generator comment (:27-32) is revision-neutral.
    - `pin_problems` (checker :127-148) ties the ADR status revision to `SPEC_REVISION` and requires `Revision N record (`. Revert tests are in `test_smp1_json_bridge_contract.py`, which is added to `make quick`.
    - My probes confirm both anchors fail closed.
  - CLOSED — [P3] [undeclared-surface] bridge contract declared no version 3 surface.
    - §11 (:329-362) names the v3 table and its NATIVE appendix D authority, `METHOD_TABLE_V3_JSON`, all six version-selected exports, and the `native_tests` key (mask 32, which matches `FEATURE_NATIVE_TESTS_V1`).
    - It also states the render-only rule. The crate enforces it: `hello_from_json` reads `FEATURE_FIELDS` only (lib.rs:1175), and `bits_from_value` → `object(…, &[])` (:653-654). The new test checks both refusals.
    - §10 (:325-327) now says the stage checker requires these exports, and `CRATE_MARKERS` (:114-123) requires each one (probed).
    - The v1/v2 renderings are unchanged because `lib.rs` is unchanged in range.
  - CLOSED — [P4] [checker-anchoring] SMP1 pin was an unanchored substring.
    - `COMPOSITION_ANCHOR` requires exactly one normative composition sentence at `SMP1_REVISION`.
    - A history line carrying the current pin fails (revert test), and a duplicated composition sentence also fails (my probe).
  - CLOSED — [P4] [precision] "no body columns" sentence.
    - :31-41 now say the tables derive the `reserved` flag from the request/response cells under the `--check` drift gate. That matches the generator :107-113.
    - The sentence quotes SMP1's qualifier "every later selection whose method table includes version 2's row 201", which matches SMP1.md at the `open_summary` paragraph, and it is wrapped.
- **Oracle fix.**
  - The head oracle's hello order is: bounds (bridge) → version split → header → body. That is exactly the codec's `validate_for_version` order (sley-protocol lib.rs:1335-1362) behind the crate's all-zero-bounds check (bridge lib.rs:899-902).
  - The new vector distinguishes head from base.
  - The crate list and the fixture are identical (emitter comparison), and `EXPECTED_REJECTION_COUNT` is 37.
- **SMP1 r15 re-pin.**
  - SMP1 r15 changes only negotiation-filter prose, the compatibility statement, and an S20-300 pin. No bridge-owned record changes (Frame, Hello, SelectedProfile, LimitProfile, BoundedContext, Failure, StreamChunk).
  - The bridge never derives selection. `selected_to_json` renders v1 names and the four-key features, so it refuses any v2/v3 selection that carries non-v1 names or bit 32. That is fail-closed and consistent with §11's "nothing more".
  - Consumer pins agree: SMP1.md:21-23 names bridge r12, SLEY_CLI_V1 :25/:38/:442 and ADR-0035:28 name bridge r12, and `check_smp1_contract.py` PASSes.
- **Machine summary.**
  - Exactly these keys changed: `contract_revision` 12; `current_delta_review` rebound to r12 with all lanes PENDING; `status_note`; `current_delta_review_note`, which keeps the r10 round as history; and the six new `*_revision_11` fields.
  - My r11 verdict is recorded verbatim as `PASS_0_P0_0_P1_0_P2_2_P3_2_P4`.
  - Status is `S20_420_IMPLEMENTED_REVIEW_PENDING`, not COMPLETE.
- **Non-counted observation (pre-existing, outside the delta, carried from r11):** `SMP1_JSON_BRIDGE_V1.md:146` still reads `}Failure {`.

## Findings

[P4] [version-domain] crates/sley-json-bridge/src/lib.rs:279-300,804-819,866-880; docs/spec/SMP1_JSON_BRIDGE_V1.md:344-349 - §11 now declares the value-level exports `frame_value_for_version` and `frame_from_value_for_version`, but gives an out-of-range rule only for `hello_to_json_for_version`. In code:
- Rendering (:809-819) falls through to v1 names for version 0 and to v2 names for any version ≥ 4.
- Reading (:279-300) resolves v1 names for every version other than 2 or 3.
- For non-hello frames, neither function reaches the codec's version gate.

So at version 4 a frame with tag 306 renders as `entity.version`, and that text is refused `JSON_BRIDGE_METHOD_UNKNOWN` by the reader at the same version. The value-level exports accept an undefined selection instead of refusing it. It is latent: they have no caller outside the crate, the CLI uses only the text entrypoints with a negotiated 1/2/3, and no canonical bytes are affected. - Closure evidence: refuse versions outside {1,2,3} in both value-level exports with the same code as `hello_to_json_for_version`, with a test. Alternatively, declare in §11 that they are ungated helpers defined only for 1/2/3.

[P4] [test-coverage] scripts/check_smp1_json_bridge_vector.py:347-350; conformance/smp1-json-bridge/v1/rejected.json; docs/spec/SMP1_JSON_BRIDGE_V1.md:268-270 - The spec says the new vector "pins" the version-before-header order, but it only pins the above-1 half. Mutation probe: deleting the oracle's `protocol_version < 1` branch still passes all 37 vectors, and v0 with request_id 1 then classifies as `PROTOCOL_FRAME_INVALID` instead of `PROTOCOL_DOWNGRADE`. The crate test at tests.rs:544-549 checks below-1 only without a header violation. - Closure evidence: add a crate rejection and a fixture vector for protocol_version 0 plus a nonzero request_id (expecting `PROTOCOL_DOWNGRADE`), or narrow the sentence to the above-1 side.

[P4] [test-coverage] crates/sley-json-bridge/src/tests.rs:357-383 - Only the v1 embedded table is tested against the frozen `Method::ALL`. The name resolvers for versions 2 and 3 use `Method::V2_ALL` and `Method::V3_ALL` (lib.rs:284, 292, 747), not the embedded `METHOD_TABLE_V2_JSON` / `METHOD_TABLE_V3_JSON` that §11 publishes as "the version 3 table" a frame names from. No test asserts that the two agree. My static comparison shows they agree today (46 names in order, tags equal), but the generator drift gate ties the v3 table to NATIVE appendix D prose, not to the resolver. - Closure evidence: a crate test asserting that each v2/v3 embedded row's name and tag equals `V2_ALL`/`V3_ALL` in order, and that `method_tag_for_version` / `method_name_for_version` round-trip every row.

[P4] [precision] docs/spec/SMP1_JSON_BRIDGE_V1.md:320-327,347-358; scripts/check_smp1_json_bridge_contract.py:337; crates/sley-json-bridge/src/tests.rs:948 - Several statements in the revised text are inaccurate or imprecise. All of them fail closed:
- (a) §10 still says `scripts/check_cli_contract.py` requires `METHOD_TABLE_V2_JSON`. Its `CRATE_MARKERS` (:107-129) require `METHOD_TABLE_V3_JSON` but not V2, and r12 edited this very sentence.
- (b) "refused (at its first name outside the version 1 table, `JSON_BRIDGE_METHOD_UNKNOWN`)" holds only when the v3 hello lists such a name. A v3 hello whose methods are all v1-table names (601/602 resolve there) is refused `JSON_BRIDGE_SHAPE_INVALID` at `features` (lib.rs:1175, 653-654).
- (c) "any other version is `JSON_BRIDGE_SHAPE_INVALID`" holds only after `hello.validate()` (lib.rs:1143), since an invalid hello yields the codec's code first. The test asserts only `is_err()`.
- (d) "after the frozen four" is true of the declared field order (`FEATURE_V3_FIELDS`, :1013-1019). The emitted text is lexicographic (§1), so `native_tests` is emitted between `json_bridge` and `stream`.
- (e) The checker comment at :337, "no capable bridge symbol is required", contradicts `CRATE_MARKERS` :114-123 and §10's r12 parenthetical.

Closure evidence: at the next bridge revision, correct (a); word (b) as "METHOD_UNKNOWN at the first non-v1 name, otherwise SHAPE_INVALID at `features`"; state the validate-first precedence in (c) and pin the SHAPE_INVALID code in the test; say "declared order" in (d); update the comment in (e).

## Assessment

Revision 12 closes all four of my lane's round-7 findings, and the closures are enforced, not merely asserted:
- The ADR-0034 revision and the SMP1 composition pin are now anchored in the checker, with revert tests that I reproduced. A duplicated composition sentence also fails.
- The version 3 surface is declared in §11, and every named export is required by the checker (probed).
- The render-only `native_tests` rule is enforced in code and tested.

The oracle now judges a hello's version before its header, exactly as the codec does. The new vector discriminates the fixed oracle from the old one, and the crate's rejection list and the committed fixture are identical.

Canonical bytes are untouched: the bridge `lib.rs`, the codec and all three method tables are unchanged in range. Every gate I ran passes: the contract checker, both vector oracles, the drift checks, SHA256SUMS, the fuzz slice and the crate tests.

The residuals are four P4s:
1. The newly declared value-level exports handle versions outside 1–3 asymmetrically and without a gate. They have no external caller.
2. The downgrade half of the oracle fix is not pinned by any vector.
3. The v2/v3 embedded tables are never tested against the resolvers' `V2_ALL`/`V3_ALL`, though they agree today.
4. Several wording and comment inaccuracies.

None admits hostile input past a refusal, and none affects canonical bytes. My lane accepts this package at contract revision 12.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_4_P4_PRIOR_P3_P4_CLOSED
SECTION: json_bridge
FIELD: vulcan_surface_review_revision_12
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS: [P4] [version-domain] crates/sley-json-bridge/src/lib.rs:279-300,804-819,866-880; docs/spec/SMP1_JSON_BRIDGE_V1.md:344-349 - §11-declared value-level exports frame_value_for_version/frame_from_value_for_version have no out-of-range rule: render falls to v1 names for v0 and v2 names for v≥4, read resolves v1 names for any v∉{2,3}, and neither reaches the codec version gate for non-hello frames (v4 renders 306 as entity.version, which the v4 reader refuses METHOD_UNKNOWN); latent, no external caller - refuse v∉{1,2,3} in both with a test, or declare them ungated helpers for 1/2/3 only | [P4] [test-coverage] scripts/check_smp1_json_bridge_vector.py:347-350; conformance/smp1-json-bridge/v1/rejected.json; docs/spec/SMP1_JSON_BRIDGE_V1.md:268-270 - "the vector pins it" covers only the above-1 half; deleting the oracle's <1 branch still passes all 37 vectors while v0+request_id 1 misclassifies as FRAME_INVALID - add a v0+nonzero-request-id DOWNGRADE crate rejection and fixture vector, or narrow the sentence | [P4] [test-coverage] crates/sley-json-bridge/src/tests.rs:357-383 - only the v1 embedded table is tested against Method::ALL; METHOD_TABLE_V2_JSON/V3_JSON are never tested against the V2_ALL/V3_ALL the resolvers use (lib.rs:284,292,747); a static comparison agrees today - crate test asserting v2/v3 rows equal V2_ALL/V3_ALL (name, tag, order) and round-trip through the versioned resolvers | [P4] [precision] docs/spec/SMP1_JSON_BRIDGE_V1.md:320-327,347-358; scripts/check_smp1_json_bridge_contract.py:337; crates/sley-json-bridge/src/tests.rs:948 - §10 says check_cli_contract.py requires METHOD_TABLE_V2_JSON (it does not); "refused at its first non-v1 name, METHOD_UNKNOWN" misses the SHAPE_INVALID-at-features case; "any other version is SHAPE_INVALID" omits validate-first precedence and the test pins only is_err(); "after the frozen four" holds only in declared, not emitted, order; checker comment :337 "no capable bridge symbol is required" is stale; all fail closed - reword at the next revision, pin the SHAPE_INVALID code, update the comment
SUMMARY: Bridge revision 12 closes all four Vulcan round-7 findings: the ADR-0034 revision and SMP1 composition pin are anchored in the checker with revert tests, and §11 declares the v3 table, the version-selected exports and the render-only native_tests key, all checker-required and tested. The oracle now judges a hello's version before its header, and the new vector matches the crate's own emission byte for byte. The bridge lib.rs, the codec and the method tables are unchanged, and every checker, oracle, drift gate and crate test passes. Four P4 residuals remain, none of which lets input past a refusal or affects canonical bytes: undeclared out-of-range behavior on the value-level exports, the unpinned downgrade half of the oracle fix, untested v2/v3 table-to-resolver equality, and several wording inaccuracies.
