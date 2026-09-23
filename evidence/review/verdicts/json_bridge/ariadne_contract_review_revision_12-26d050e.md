<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: ariadne; field: ariadne_contract_review_revision_12; dispatched: 2026-09-23T13:45:21Z; duration_s: 475; process_exit_code: 0 -->
# Ariadne Council review — json_bridge

Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

I ran these checks myself:

- **Scope.** `git rev-parse HEAD` returned `26d050e629b669ef4acbd54826ef4008c05a061d`, which matches. The working tree has two untracked files, both other lanes' verdicts, and nothing is modified.
- **Git read commands:**
  - `git diff --stat 2b0f1c9f..HEAD`.
  - `git diff 2b0f1c9f..HEAD` for the package-owned paths:
    - `docs/spec/SMP1_JSON_BRIDGE_V1.md`, `docs/adr/ADR-0034-json-bridge-boundary.md` and `docs/audits/S20_420_JSON_BRIDGE_CLOSEOUT.md`.
    - `scripts/check_smp1_json_bridge_contract.py` and the new `scripts/test_smp1_json_bridge_contract.py`.
    - `scripts/check_smp1_json_bridge_vector.py`, `scripts/generate_smp1_json_bridge_fixtures.py` and `scripts/generate_smp1_json_bridge_table.py`.
    - `conformance/smp1-json-bridge/` and `crates/sley-json-bridge/`.
  - `git diff 2b0f1c9f..HEAD` for the composed and consuming records: `docs/spec/SMP1.md`, `Makefile` and `docs/WORK_PACKAGES.md`.
  - A key-by-key diff of the machine-summary `json_bridge` section at 2b0f1c9f and at HEAD.
  - `git show 2b0f1c9f:scripts/check_smp1_json_bridge_vector.py`, used for a base-vs-head oracle probe.
- **Checkers and tests, with exit codes:**

| Command | Exit | Result |
|---|---:|---|
| `python3 scripts/check_smp1_json_bridge_contract.py` | 0 | `"result": "PASS"`, `"revision": 12`, `"status": "S20_420_IMPLEMENTED_REVIEW_PENDING"`, `"problems": []` |
| `python3 scripts/test_smp1_json_bridge_contract.py -v` | 0 | 4 tests OK (current texts pass; stale composition pin masked by a history line refused; stale ADR status refused; missing ADR record refused) |
| `python3 scripts/generate_smp1_json_bridge_fixtures.py --check` | 0 | `{"drift": [], "rejections": 37, "result": "PASS", "vectors": 5}` |
| `python3 scripts/generate_smp1_json_bridge_table.py --check --protocol-version 1` | 0 | PASS (v1 table) |
| same, `--protocol-version 2` | 0 | PASS (v2 table) |
| same, `--protocol-version 3` | 0 | PASS (v3 table) |
| `uv run --offline --frozen --project oracle/scb1 python scripts/check_smp1_json_bridge_vector.py` (subprocess) | 0 | PASS: methods 41, vectors 5, rejections 37, problems [] |
| `python3 scripts/test_smp1_json_bridge_table.py` | 0 | 17 cases PASS |
| `python3 scripts/check_smp1_json_bridge_persistent_fuzz_slice.py` | 0 | PASS |
| `python3 scripts/check_smp1_contract.py` | 0 | PASS, revision 15 |
| `python3 scripts/test_smp1_contract.py` | 0 | 16 tests OK |
| `python3 scripts/check_cli_contract.py` | 0 | PASS, revision 10 |
| `python3 scripts/test_cli_contract.py` | 0 | 10 tests OK |
| `python3 scripts/check_required_contract_index.py` | 0 | PASS |
| `cargo test --locked --offline -p sley-json-bridge` | 0 | 12 passed, 0 failed, 1 ignored (the fixture-refresh emitter); includes the new `the_version_3_hello_rendering_is_render_only` and the rejection matrix with `hello-version-above-with-request-id` |

- **Base vs head oracle probe.** I ran `frame_from_json` from the HEAD oracle and from the base (2b0f1c9f) oracle, both exec'd under uv, on the new vector with the protocol version and request id varied:

| Case | HEAD oracle | Base oracle |
|---|---|---|
| pv=99, rid=1 | `PROTOCOL_VERSION_UNSUPPORTED` | `PROTOCOL_FRAME_INVALID` |
| pv=0, rid=1 | `PROTOCOL_DOWNGRADE` | `PROTOCOL_FRAME_INVALID` |
| pv=1, rid=1 | `PROTOCOL_FRAME_INVALID` | `PROTOCOL_FRAME_INVALID` |

  The vector discriminates the old ordering from the new.
- **Hashes and tables.** I recomputed SHA256SUMS in Python for `v1` (roundtrip, rejected and methods), `v2` and `v3`: all match. From the table headers:

| Table | Rows | Reserved | Reserved tags | Other |
|---|---:|---:|---|---|
| v1 | 41 | 4 | 305, 503, 601, 602 | |
| v2 | 43 | 4 | 305, 503, 601, 602 | |
| v3 | 46 | 2 | 305, 503 | v3 minus v2 = {605, 606, 607}; `contract` = the bridge spec; `v3_source` = `docs/spec/NATIVE_TEST_ADMISSION_V1.md` |

- **Files read:**
  - Bridge package: `docs/spec/SMP1_JSON_BRIDGE_V1.md:1-363` (whole), `docs/adr/ADR-0034-json-bridge-boundary.md:1-70` (whole), `scripts/check_smp1_json_bridge_contract.py:150-364` and `scripts/check_smp1_json_bridge_vector.py:290-440`.
  - Crate: `crates/sley-json-bridge/src/lib.rs:50-89,255-300,435-437,515-545,641-670,700-1230` and `crates/sley-json-bridge/src/tests.rs:357-383,920-949`.
  - Protocol: `crates/sley-protocol/src/lib.rs:806-824,1304-1364,1405-1449,1740-1799` (plus grep hits for `FEATURE_NATIVE_TESTS_V1` = 32 at :74 and the method names at :709-717).
  - Composed specs: `docs/spec/SMP1.md:600-616` (section 8) and `docs/spec/NATIVE_TEST_ADMISSION_V1.md:360-374,548-577`.
  - CLI: `scripts/check_cli_contract.py:100-124,286-288` and `crates/sley-cli/src/lib.rs:16-19,507-535` (grep).
  - Records: `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:860-963` (§13.2 to §13.5), `docs/WORK_PACKAGES.md:48`, `Cargo.lock:247-257`, and my prior transcript `evidence/review/verdicts/json_bridge/ariadne_contract_review_revision_11-2b0f1c9.md:1-133`.

## Evidence checked

1. **The delta matches gate record §13.3 rows 923-929.**
   - Contract, `SMP1_JSON_BRIDGE_V1.md`:
     - Status revision 11 → 12 (`:3`).
     - History rewritten, and the SMP1 qualifier "whose method table includes version 2's row 201" restored (`:31-47`).
     - Composition pin revision 14 → 15 (`:56`).
     - Authority block marked as a paraphrase (`:62-63`).
     - Oracle-order sentence added to section 8 (`:268-270`).
     - Section 10 amended (`:324-327`), and new section 11 (`:329-362`).
   - ADR-0034 gains a current status and records for revisions 11 and 12 (`:3,17-26`).
   - The closeout defers the current revision to the machine summary (`:3`).
   - Checker, oracle and fixtures: `pin_problems` in the checker; the oracle version split; one new rejected vector (36 → 37); a crate test; the generator comment.
   - Machine summary: `contract_revision` 11→12, `current_delta_review` {12, PENDING×3}, the three `_revision_11` lane fields (mine recorded exactly as `PASS_0_P0_0_P1_0_P2_3_P3_3_P4`) and the notes. No historical field was overwritten.
   - The crate `lib.rs` is unchanged in this delta.
2. **SMP1 revision 15 makes nothing in the bridge false.**
   - The revision 15 delta (`SMP1.md:69-96,240-251`) states the per-selection intersection filter of explicit negotiation. The code has applied it since 2026-09-16.
   - The bridge owns no negotiation. Section 2 (`:155-159`) says it defines nothing about the handshake.
   - `selected_to_json` renders with the v1 names only (`lib.rs:1208,1212`), unchanged.
   - The table drifts at v1, v2 and v3 are all PASS, and the sums match.
   - The back-pins agree: SMP1 `:21-23` names bridge 12; the CLI pins bridge 12 at `SLEY_CLI_V1.md:25,38,442`, checked by `check_cli_contract.py:33,306,313`; `WORK_PACKAGES.md:48` names revision 12 and SMP1 15.
3. **The section 11 claims hold against code and tables:**
   - v3 table: 46 rows, with 601 and 602 live and 605-607 added; checked by table data and checker `:252-280`.
   - The authority is NATIVE appendix D (`NATIVE:552-577`).
   - The three embedded tables are at `lib.rs:54,61,71`.
   - Version-3 name resolution is at `lib.rs:283-289,746-749,809-814`.
   - `hello_to_json_for_version` accepts {1,2,3} and otherwise returns `ShapeInvalid` (`lib.rs:1142-1154`).
   - The v3 features are the frozen four plus `native_tests` = `FEATURE_NATIVE_TESTS_V1` = 32 (`lib.rs:1013-1026`; `sley-protocol/lib.rs:74`; NATIVE appendix C `:366` "bit32 native_tests_v1").
   - `hello_from_json` reads only `FEATURE_FIELDS` (`lib.rs:1175`), and the new test pins both refusals (`tests.rs:937-947`).
   - The checker requires every listed export plus `METHOD_TABLE_V2_JSON` and `"native_tests"` (`check:114-123`).
4. **Oracle order.** The oracle judges the bounds (bridge shape), then version <1 / >1, then the header (`check_smp1_json_bridge_vector.py:345-352`). The crate does the same: bounds, then `validate_header` → `validate_for_version(1)`, which splits the version before the header rule (`lib.rs:899-902`; `sley-protocol/lib.rs:1328-1362`). The executed probe above confirms the new order and that the base oracle disagreed.
5. **The pin anchors are wired and tested.** `pin_problems` is called from `main` (`check:347`). It reads the SMP1 pin only from the single "It composes, and never alters" sentence, and the ADR revision from its status line plus a `Revision N record (` entry. The four revert tests pass, and the Makefile quick target runs the suite (diff `+python3 scripts/test_smp1_json_bridge_contract.py`).
6. **Residual observations** are enumerated as findings below. The first two are the narrower remainder of my prior declared-surface P3; the last two concern the new section 11 text.

Prior-finding status (revision-11 transcript):
- CLOSED — [P3] [record-currency] ADR-0034 stuck at revision 10 and revision-agnostic `ADR_MARKERS`. ADR-0034 `:3` names revision 12. It has records for 11 and 12 (`:17-24`) and the date line (`:26`). `pin_problems` anchors it (`check:128-148`), with negative tests `test_stale_adr_status_is_refused` and `test_missing_adr_revision_record_is_refused`.
- CLOSED — [P3] [declared-surface] undeclared version 3 surface; `METHOD_TABLE_V2_JSON` required by no checker. Section 11 names the v3 table, its NATIVE appendix D authority, all six version-selected exports, the render-only `native_tests` key, and the checker coverage. `CRATE_MARKERS` now require `METHOD_TABLE_V2_JSON` and the `pub fn` exports (`check:114-123`). The narrower residuals are filed as new P4s 1 and 2.
- CLOSED — [P3] [evidence-currency] oracle judges the hello header before the version. The oracle order is fixed (`vector.py:347-352`), and `hello-version-above-with-request-id` is in `rejected.json`, the crate matrix (`tests.rs:301-312`) and the passing oracle run. The executed probe shows the base oracle returned `PROTOCOL_FRAME_INVALID` for it.
- CLOSED — [P4] [stale-comment] generator comment "SMP1.md stays revision 13". It now reads "SMP1.md (whatever revision its Status line names) owns no version 3 row" (`generate_smp1_json_bridge_table.py:27-32`). That is accurate: SMP1 `:236-240` defers version 3 to NATIVE.
- CLOSED — [P4] [citation] authority rule quoted as SMP1 section 8. It is now marked "this contract's paraphrase of SMP1 section 8 (not a quotation of it)" (`:62-63`), and its substance agrees with `SMP1.md:604-615`.
- CLOSED — [P4] [cross-reference] closeout names revision 9. The closeout (`:3`) now says it records the revision 9 implementation and defers the current revision to the machine summary, with revisions 10 to 12 in the contract and ADR-0034.

## Findings

[P4] [declared-surface] docs/spec/SMP1_JSON_BRIDGE_V1.md:344-349 - Section 11 declares a version domain only for `hello_to_json_for_version` ({1,2,3}, otherwise `JSON_BRIDGE_SHAPE_INVALID`). The four frame exports have none, and outside {1,2,3} their behavior is undeclared and asymmetric. `frame_value_for_version` names methods with the v1 table at version ≤1 and with the v2 table at 2 and at every version ≥4 (crates/sley-json-bridge/src/lib.rs:809-819). `frame_from_value_for_version` resolves every version other than 2 and 3 with the v1 table (lib.rs:279-299). Neither reaches the codec's version gate. So `frame_value_for_version(f, 4)` renders a 306 frame as "entity.version", which `frame_from_value_for_version(_, 4)` refuses as `JSON_BRIDGE_METHOD_UNKNOWN`. The text exports instead fail the codec's `PROTOCOL_VERSION_UNSUPPORTED` for non-hello frames. The new test checks only `is_err()` for version 4 (crates/sley-json-bridge/src/tests.rs:948), not the declared code. In-tree callers pass only negotiated versions (crates/sley-cli/src/lib.rs:597,799,1023), so this is not reachable today. - Closure evidence needed: a section 11 sentence giving the frame exports' version domain and their out-of-domain result (or making the fallback symmetric or refused), and a test that pins the declared code for `hello_to_json_for_version(_, 4)`.

[P4] [citation] docs/spec/SMP1_JSON_BRIDGE_V1.md:320-324 - Section 10 still says the four exports including `METHOD_TABLE_V2_JSON` are what "`scripts/check_cli_contract.py` requires". The CLI checker's `CRATE_MARKERS` require `hello_to_json_versioned(`, `hello_to_json_for_version(`, `METHOD_TABLE_V3_JSON`, `frame_from_json_for_version(` and `frame_to_json_for_version(`, but not `METHOD_TABLE_V2_JSON` (scripts/check_cli_contract.py:107-118). Only the bridge checker now requires it (check_smp1_json_bridge_contract.py:114). The revision 12 parenthetical at :325-327 adds the stage checker but leaves the inaccurate attribution. - Closure evidence needed: the sentence attributes `METHOD_TABLE_V2_JSON` to the bridge checker (or the CLI checker gains the marker).

[P4] [wording] docs/spec/SMP1_JSON_BRIDGE_V1.md:350-352 - Section 11 says the version 3 hello carries `native_tests` "after the frozen four". Emission is lexicographic by contract (section 1, :97-99), and in code: `render` is `Value::to_string` (lib.rs:435-436) over serde_json's sorted map, since Cargo.lock:247-257 has no `indexmap`/`preserve_order`. So the emitted order is cancel, checksum, json_bridge, native_tests, stream: the new key sits before `stream`, not after the four. "After" holds only for the declared list `FEATURE_V3_FIELDS` (lib.rs:1013-1019), and declared order has no precedence role for a render-only object. An independent renderer that read this literally would emit different text. - Closure evidence needed: reword to "a fifth declared `features` field, emitted in the section 1 lexicographic order", or drop the positional phrase.

[P4] [test-coverage] docs/spec/SMP1_JSON_BRIDGE_V1.md:342-343 - Section 11 declares `METHOD_TABLE_V3_JSON`, the table the CLI emits verbatim for its v3 profile (crates/sley-cli/src/lib.rs:507-508). No test compares the embedded v2 or v3 table to `Method::V2_ALL`/`Method::V3_ALL`, which the bridge's name resolver actually uses (lib.rs:284-297). `the_embedded_method_table_equals_the_frozen_method_table` covers only the v1 table (tests.rs:357-383). The drift gate ties the v3 table to NATIVE appendix D, not to the crate resolver, so the published table and the rendered frame names come from two unlinked sources. Their names agree today (sley-protocol/src/lib.rs:709-717 vs v3 `methods.json`), which I checked by inspection only. - Closure evidence needed: a crate test that the v2 and v3 embedded tables equal `V2_ALL`/`V3_ALL` by tag, name, family and reserved flag, or a section 6/11 statement that the resolver, not the table, is authoritative for frame naming.

## Assessment

I accept bridge revision 12 for this lane.

- **All six revision-11 findings are closed, each with executed or code-level evidence:**
  - ADR-0034 is current, and the checker now anchors it, with negative tests.
  - The version 3 surface is declared in section 11. Each claim checks out against the crate, the tables, NATIVE appendices C/D and the checker.
  - The oracle judges the hello version before the header. The discriminating vector passes on HEAD and would have failed on the base oracle.
  - The generator comment, the authority paraphrase and the closeout reference are fixed.
- **The SMP1 revision 15 re-pin is consistent.** It is in the composition sentence, the checker anchors, the SMP1 and CLI back-pins, WORK_PACKAGES and the machine summary. SMP1 revision 15's negotiation-filter statement touches nothing the bridge owns.
- **Gates.** Every package gate and the bounded crate tests are green.
- **Four P4s remain, none a correctness defect:**
  - The frame exports' version domain is undeclared and asymmetric outside {1,2,3}.
  - Section 10 still says the CLI checker requires `METHOD_TABLE_V2_JSON`.
  - The positional wording for `native_tests` does not match the lexicographic emission order.
  - The v2/v3 embedded tables are never tested against the resolver.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_4_P4_PRIOR_P3_P4_CLOSED
SECTION: json_bridge
FIELD: ariadne_contract_review_revision_12
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS:
[P4] [declared-surface] docs/spec/SMP1_JSON_BRIDGE_V1.md:344-349 - version domain declared only for hello_to_json_for_version; frame_value_for_version renders version >=4 with v2 names (lib.rs:809-819) while frame_from_value_for_version reads every version but 2/3 with v1 names (lib.rs:279-299), neither reaching the codec gate; test checks only is_err() for version 4 (tests.rs:948); unreachable from in-tree callers - declare the frame exports' domain and out-of-domain result (or make it symmetric/refused) and pin the hello code in a test
[P4] [citation] docs/spec/SMP1_JSON_BRIDGE_V1.md:320-324 - section 10 still says scripts/check_cli_contract.py requires METHOD_TABLE_V2_JSON; its CRATE_MARKERS (check_cli_contract.py:107-118) do not, only the bridge checker does (check_smp1_json_bridge_contract.py:114) - attribute the marker to the bridge checker or add it to the CLI checker
[P4] [wording] docs/spec/SMP1_JSON_BRIDGE_V1.md:350-352 - "after the frozen four" contradicts the lexicographic emission (section 1 :97-99; render = Value::to_string over the sorted map, lib.rs:435-436, no preserve_order per Cargo.lock:247-257): native_tests is emitted before stream - reword to a declared fifth field emitted in lexicographic order
[P4] [test-coverage] docs/spec/SMP1_JSON_BRIDGE_V1.md:342-343 - METHOD_TABLE_V2_JSON/V3_JSON are never tested against Method::V2_ALL/V3_ALL, which the resolver uses (lib.rs:284-297); only v1 is (tests.rs:357-383); names agree today by inspection only - add a table-equals-resolver test for v2/v3 or state which source is authoritative
SUMMARY: Bridge revision 12 closes all six revision-11 findings: ADR-0034 is current with an anchored checker and revert tests, section 11 declares the version 3 table, exports and render-only native_tests key, and the oracle judges the hello version first (the new vector passes at HEAD, and the executed probe shows the base oracle gave PROTOCOL_FRAME_INVALID). The generator comment, authority paraphrase and closeout reference are fixed too. The SMP1 revision 15 re-pin is consistent across the contract, checker anchors, SMP1/CLI back-pins, WORK_PACKAGES and the machine summary, and it changes nothing the bridge owns. The bridge checker, its 4 revert tests, the table drifts at v1/v2/v3, fixtures (37 rejections), the uv-run oracle, and the crate tests (12 passed, 1 ignored) are all green; four P4s remain on version-domain, citation, wording and table-test coverage.
