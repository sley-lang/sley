<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2b0f1c9f4940c020565137891a49a3769cd02061; role: ariadne; field: ariadne_contract_review_revision_11; dispatched: 2026-09-23T12:00:31Z; duration_s: 463; process_exit_code: 0 -->
# Ariadne Council review — json_bridge

Harness: claude-code
Reviewed checkpoint: 2b0f1c9f4940c020565137891a49a3769cd02061

I ran these checks myself:

- **Scope.** `git rev-parse HEAD` returned `2b0f1c9f4940c020565137891a49a3769cd02061`, which matches. The working tree has one untracked file (another lane's verdict), and nothing is modified.
- **Git read commands:**
  - `git diff ab42a3a9..HEAD -- docs/spec/SMP1_JSON_BRIDGE_V1.md`: 8 lines added, 4 removed.
  - `git diff ab42a3a9..HEAD -- docs/spec/SMP1.md`: 101 added, 19 removed.
  - `git diff ab42a3a9..HEAD -- crates/sley-protocol/src/server.rs`.
  - `git diff ab42a3a9..HEAD --stat -- crates/ conformance/ docs/ scripts/`.
  - `git diff --quiet ab42a3a9..HEAD -- conformance/smp1-json-bridge crates/sley-json-bridge crates/sley-protocol/src/lib.rs`: unchanged.
  - Diffs of `scripts/check_smp1_json_bridge_contract.py` and `scripts/generate_smp1_json_bridge_table.py`.
  - A key-by-key diff of the machine-summary `json_bridge` section at ab42a3a9 and HEAD.
  - `git log -S` for `METHOD_TABLE_V3_JSON`: introduced by 0c655fc1 on 2026-09-16.
  - `git log -S` for the quoted authority sentence: last in SMP1 at 3a38db08 on 2026-09-03.
  - `git log` dates for d384f0f and 0c655fc1.
- **Checkers and tests, with exit codes:**

| Command | Exit | Result |
|---|---:|---|
| `python3 scripts/check_smp1_json_bridge_contract.py` | 0 | `"result": "PASS"`, `"revision": 11`, `"status": "S20_420_IMPLEMENTED_REVIEW_PENDING"`, `"problems": []` |
| `python3 scripts/generate_smp1_json_bridge_table.py --check --protocol-version 1` | 0 | PASS, v1 table |
| same, `--protocol-version 2` | 0 | PASS, v2 table |
| same, `--protocol-version 3` | 0 | PASS, v3 table |
| `python3 scripts/generate_smp1_json_bridge_fixtures.py --check` | 0 | `{"drift": [], "rejections": 36, "result": "PASS", "vectors": 5}` |
| `python3 scripts/check_smp1_json_bridge_vector.py` | 1 | Environment only: `ModuleNotFoundError: No module named 'blake3'` |
| `uv run --with blake3 python3 scripts/check_smp1_json_bridge_vector.py` | 0 | PASS: methods 41, vectors 5, rejections 36 |
| `python3 scripts/test_smp1_json_bridge_table.py` | 0 | 17 cases PASS |
| `python3 -m unittest scripts/test_smp1_json_bridge_table.py` | 5 | No tests: the file is a direct-run harness |
| `python3 scripts/check_smp1_json_bridge_persistent_fuzz_slice.py` | 0 | PASS |
| `python3 scripts/check_smp1_contract.py` | 0 | PASS, revision 14 |
| `python3 scripts/check_cli_contract.py` | 0 | PASS, revision 9 |
| `python3 scripts/test_smp1_contract.py` | 0 | 16 tests OK |
| `python3 scripts/test_current_contract_review.py` | 0 | 6 tests OK |
| `python3 scripts/test_cli_contract.py` | 0 | 10 tests OK |
| `python3 scripts/check_required_contract_index.py` | 0 | PASS |
| `cargo test -p sley-json-bridge --offline` | 0 | 11 passed, 0 failed, 1 ignored (the fixture-refresh emitter) |

- **SHA256SUMS.** I recomputed the sums in Python for `conformance/smp1-json-bridge/v1` (roundtrip, rejected, methods), `v2` and `v3`. All match.
- **Files read:**
  - Whole files: `docs/spec/SMP1_JSON_BRIDGE_V1.md:1-311`, `scripts/check_smp1_json_bridge_contract.py:1-325` and `scripts/generate_smp1_json_bridge_table.py:1-326`.
  - `crates/sley-json-bridge/src/lib.rs:1-90,270-300,725-993` plus its public-API listing; `crates/sley-json-bridge/src/tests.rs` (function index, 590-625).
  - `docs/spec/SMP1.md:1-80,581-596` and every rev-13/14 hunk (133, 219-229, 254-257, 326-348, 662-760, 890-896).
  - `docs/adr/ADR-0034-json-bridge-boundary.md:1-30`, `docs/audits/S20_420_JSON_BRIDGE_CLOSEOUT.md:1-40`, `docs/spec/NATIVE_TEST_ADMISSION_V1.md:530-575`, `docs/spec/SLEY_CLI_V1.md:14-34` and `docs/WORK_PACKAGES.md:48`.
  - The ADR-0033 diff, `scripts/check_smp1_json_bridge_vector.py:336-348,412-424`, and `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:560-570,676`.
  - The prior revision-10 transcripts `evidence/review/verdicts/json_bridge/delta_review_{ariadne,nabu,vulcan}-1023a31.md` (findings and closure lines).

## Evidence checked

1. **The delta is exactly the re-pin.**
   - Status line (`SMP1_JSON_BRIDGE_V1.md:3`): revision 10 (2026-09-14) became revision 11 (2026-09-23).
   - The history sentence was added at `:31-36`.
   - The composition pin at `:44` moved from `(revision 12)` to `(revision 14)`.
   - No clause in sections 1-10 changed.

2. **The revision-11 summary of SMP1 revision 14 is accurate.**
   - The bridge says SMP1 revision 14 "defines the `workspace.open` (201) response under version 2 and every later selection as `open_summary` (optional field 9) and refuses a non-empty 201 body under every version".
   - This matches `SMP1.md:59-76`, row 201 at `:348`, appendix A row 201 and `open_summary` at `:682` and `:716-760`.
   - The bridge drops SMP1's qualifier "whose method table includes version 2's row 201". That is harmless: the only later selection SMP1 admits is 3 (`SMP1.md:222-226,257`), and version 3 carries row 201 (`conformance/smp1-json-bridge/v3/methods.json`: tag 201, reserved false; NATIVE appendix D `:548-550`). The server gate is `>= PROTOCOL_VERSION_V2` (`server.rs` `workspace_open`), which is literally "every later selection". Not a finding.

3. **201 bodies are opaque in the bridge.**
   - The crate renders bodies as `hex(&frame.body)` (`lib.rs:846`) and reads them with `hex_field` (`:883`) for every method.
   - There is no 201 or `workspace` logic in `crates/sley-json-bridge`, in the oracle, or in the bridge scripts (grep returned nothing).
   - The codec the bridge calls (`crates/sley-protocol/src/lib.rs`) is unchanged since ab42a3a9.
   - The non-empty-201 refusal lives only in server dispatch: `workspace_open(body)` returns `PayloadInvalid` in `server.rs`. So `frame_from_json` still encodes any 201 frame, and the server judges it, as bridge section 9 (`:283-287`) states.
   - The bridge corpus holds no 201 frame. Its 5 vectors are request, response, failure, hello-client and hello-server. `rejected.json` has no `workspace.open`.
   - The bridge's "no bridge clause, encoding, or behavior changes" holds for 201.

4. **The method table has no body columns.**
   - `methods.json` rows carry `{family, name, owner, reserved, tag}`.
   - The generator reads the request and response cells only for the reserved predicate (`generate_smp1_json_bridge_table.py:106-111`). Row 201's new cells, "none" and "accepted head summary (appendix A)", are non-reserved, the same as before.
   - Drift PASS at v1, v2 and v3, plus matching SHA256SUMS, confirm the tables are byte-identical.
   - The counts in section 2 still hold: v1 is 41 rows with 4 reserved (37 dispatched); v2 is 43 with 4 reserved (39 dispatched).
   - The owner cell is still S20-390, which is what SMP1 `:334-338` says feeds `methods.json`.

5. **The pins are consistent.**
   - The checker's `SPEC_REVISION = 11` and `SMP1_REVISION = 14` (`check:26-27`) are anchored to both Status lines and to the `:44` pin text (`check:301-309`).
   - SMP1 `:21-23` pins bridge revision 11.
   - The CLI pins bridge revision 11 at `SLEY_CLI_V1.md:18,31`, and `check_cli_contract.py:33,258-262` checks it against the bridge Status line.
   - The reverse pin `check_smp1_contract.py:404` passes.
   - `WORK_PACKAGES.md:48` names revision 11, the SMP1 revision 14 re-pin and the new status.
   - In the machine summary only five keys changed: `contract_revision` 10→11; `current_delta_review` {11, PENDING×3}; `status` `S20_420_COMPLETE`→`S20_420_IMPLEMENTED_REVIEW_PENDING`; `implementation_complete` true→false; `status_note`. Historical PASS fields are preserved.

6. **Hello and version rules are unaffected.** SMP1 revision 14 keeps Hello at 1 (`SMP1.md:133`), so bridge section 8 (`:247-259`) and its tests (`hello_header_violations_carry_the_codec_code`, passing) still hold.

7. **Records and surrounding declarations were not all carried.** See the findings below: ADR-0034, the version 3 surface, the generator comment, the authority quote, and two prior items that are still open.

## Findings

[P3] [record-currency] docs/adr/ADR-0034-json-bridge-boundary.md:3-18 - The ADR status still reads "the S20-420 contract is a draft at revision 10". Its records stop at revision 10, and the date line has no 2026-09-23 entry. Revision 11 and its SMP1 revision 14 re-pin appear nowhere in the package ADR, although the same round added the re-pin record to ADR-0033 (session handle revision 5). This is the failure the revision-10 Nabu P3 predicted: `ADR_MARKERS` (scripts/check_smp1_json_bridge_contract.py:66-74) do not pin a revision, so the stale ADR passes the gate. - Closure evidence needed: a revision-11 record and a current status line in ADR-0034, plus a checker marker that pins the current-revision ADR record (as scripts/check_cli_contract.py:80 does for its row), with a negative test.

[P3] [declared-surface] docs/spec/SMP1_JSON_BRIDGE_V1.md:148-158,297-311 - The contract names only the version 1 and version 2 tables. Section 10 lists four versioned exports as what "the crate does carry". The crate also exports `METHOD_TABLE_V3_JSON` (crates/sley-json-bridge/src/lib.rs:71-72), `hello_to_json_for_version` (:1142), `frame_value_for_version` and `frame_from_value_for_version` (:804,:866), and resolves version 3 names (:279-290,:742-750,:809-814). This package's own checker requires `METHOD_TABLE_V3_JSON` and `hello_to_json_for_version` (check_smp1_json_bridge_contract.py:109-110) and gates the v3 table's counts, union and source (:186-245). The machine summary names `method_table_v3`, and the CLI checker requires the same v3 exports (check_cli_contract.py:92-93). Revision 11 composes SMP1 revision 14, the first SMP1 revision to name version 3 as a wire selection (SMP1.md:70,133,222-226,257), yet says no bridge clause changes. This surface landed in 0c655fc1 (2026-09-16), after the COMPLETE status and after every prior bridge review (d384f0f/1023a31, 2026-09-14), so no bridge lane has reviewed it. The prior revision-10 P4 is still open and is folded in here: `METHOD_TABLE_V2_JSON` is required by no checker, although section 10 says check_cli_contract.py "requires them". Nothing is made false by SMP1 revision 14; the gap is that the composed SMP1 now names a selection the bridge contract never declares. - Closure evidence needed: either a bridge clause naming the version 3 table and its authority (NATIVE_TEST_ADMISSION_V1 appendix D, `--protocol-version 3`) and the complete versioned-export list with its version domain {1,2,3} and checker coverage, or an explicit section 7 or 10 statement that the version 3 surface is owned elsewhere and that the section 10 list is not exhaustive.

[P3] [evidence-currency] docs/spec/SMP1_JSON_BRIDGE_V1.md:252-255 - Carried from the revision-10 Ariadne round, still open. The contract says "The codec judges the protocol version first". The crate does, but the independent oracle raises `PROTOCOL_FRAME_INVALID` for a non-zero hello header inside frame parsing (scripts/check_smp1_json_bridge_vector.py:344-345) before `encode_frame` applies the downgrade/unsupported split (:418-421). A hello with protocol_version 99 and request_id 1 therefore gets different codes from the crate and the oracle, and no vector pins this. The oracle has not changed since 2026-09-14. - Closure evidence needed: the oracle's version split moved ahead of its header check (or "first" dropped from the contract), plus a non-1 hello-version rejection vector.

[P4] [stale-comment] scripts/generate_smp1_json_bridge_table.py:27-31 - The comment says "SMP1.md stays revision 13". SMP1 is at revision 14, and NATIVE appendix D (docs/spec/NATIVE_TEST_ADMISSION_V1.md:548) pins 14. The comment was bumped at revision 13 but missed by this package's revision-14 re-pin. - Closure evidence needed: the comment made revision-agnostic or set to 14.

[P4] [citation] docs/spec/SMP1_JSON_BRIDGE_V1.md:50-54 - The block quoted as "The authority rule (SMP1 section 8)" is not in the pinned SMP1 revision 14; section 8 is docs/spec/SMP1.md:581-594. The sentence left SMP1 with the 3a38db08 S20-400 draft (2026-09-03), so the pinned authority does not contain the quoted text, although its substance agrees. This predates the delta. - Closure evidence needed: quote SMP1.md:583-594 verbatim, or mark the block as a paraphrase.

[P4] [cross-reference] docs/audits/S20_420_JSON_BRIDGE_CLOSEOUT.md:3-5 - Carried from the revision-10 Ariadne round, still open. The closeout status still names contract "(revision 9)", now two revisions behind. No checker pins it. - Closure evidence needed: the closeout names revision 11, or it says it records the revision-9 implementation and points to the machine summary for the current revision.

## Assessment

The revision-11 delta does what it says, and I accept it for this lane.

- **Nothing in the bridge is made false by SMP1 revision 14.** SMP1 revision 14 changes the version 2 and later body of `workspace.open`, row 201's request and response cells, and the server's refusal of non-empty 201 bodies. None of these reach the bridge:
  - The crate has no 201-specific path, and the codec it calls is unchanged.
  - The refusal is server dispatch, not frame validation.
  - The generated tables carry no body columns, and the one field derived from body cells (`reserved`) is unchanged for row 201. Drift gates and sums prove all three tables are byte-identical.
  - The corpus contains no 201 frame.
- **The pin and status move is complete and consistent** across the contract, the checker's anchored cross-checks, the SMP1 and CLI back-pins, WORK_PACKAGES, and the machine summary. Every relevant gate and the crate tests are green.
- **Three P3s remain, none of them correctness defects:**
  - ADR-0034 was not carried to revision 11, and its unpinned checker let that pass.
  - The contract has never declared the version 3 table and exports that its crate and checker carry. This is now more visible because the pinned SMP1 names version 3.
  - The revision-10 oracle-ordering P3 is still open.
- **Three P4s:** the generator comment, the authority quote, and the carried closeout reference.

VERDICT: PASS_0_P0_0_P1_0_P2_3_P3_3_P4
SECTION: json_bridge
FIELD: ariadne_contract_review_revision_11
SCOPE_SHA: 2b0f1c9f4940c020565137891a49a3769cd02061
FINDINGS:
[P3] [record-currency] docs/adr/ADR-0034-json-bridge-boundary.md:3-18 - ADR status still reads "draft at revision 10" with no revision-11 record or 2026-09-23 date, although the same round recorded ADR-0033's re-pin; ADR_MARKERS (scripts/check_smp1_json_bridge_contract.py:66-74) do not pin a revision, so the gate passes it - revision-11 record plus a checker marker pinning the current ADR record, with a negative test
[P3] [declared-surface] docs/spec/SMP1_JSON_BRIDGE_V1.md:148-158,297-311 - contract declares only the v1/v2 tables and four versioned exports, while the crate exports METHOD_TABLE_V3_JSON, hello_to_json_for_version and frame_*_value_for_version (lib.rs:71-72,804,866,1142) and resolves v3 names (lib.rs:279-290,742-750,809-814); the package checker requires them and gates the v3 table (check:109-110,186-245); pinned SMP1 revision 14 now names version 3 (SMP1.md:70,133,222-226,257); surface landed 0c655fc1 (2026-09-16) and no bridge lane has reviewed it; folds in the prior P4 that METHOD_TABLE_V2_JSON is required by no checker - clause naming the v3 table authority (NATIVE appendix D) and the complete export list with version domain, or an explicit out-of-scope statement with section 10 marked non-exhaustive
[P3] [evidence-currency] docs/spec/SMP1_JSON_BRIDGE_V1.md:252-255 - carried from revision 10: "codec judges the protocol version first" holds in the crate, but the oracle raises PROTOCOL_FRAME_INVALID for a hello header (check_smp1_json_bridge_vector.py:344-345) before its version split (:418-421), and no non-1 hello-version vector exists - oracle order fixed or "first" dropped, plus the vector
[P4] [stale-comment] scripts/generate_smp1_json_bridge_table.py:27-31 - comment says "SMP1.md stays revision 13"; SMP1 is revision 14 - comment made revision-agnostic or set to 14
[P4] [citation] docs/spec/SMP1_JSON_BRIDGE_V1.md:50-54 - block quoted as the "authority rule (SMP1 section 8)" is absent from pinned SMP1 revision 14 section 8 (SMP1.md:581-594); removed at 3a38db08 - quote the actual section 8 text or mark it as a paraphrase
[P4] [cross-reference] docs/audits/S20_420_JSON_BRIDGE_CLOSEOUT.md:3-5 - carried from revision 10: closeout status still names contract revision 9, pinned by no checker - closeout names revision 11 or defers the current revision to the machine summary
SUMMARY: Bridge revision 11 re-pins SMP1 revision 14 correctly. 201 bodies stay opaque (lib.rs:846,883), the codec, crate, conformance tables and vectors are unchanged since ab42a3a9, the 201 refusal is server-only, and the generated v1/v2/v3 tables are byte-identical (drift PASS, SHA256SUMS match), so no bridge statement is made false. The pin and status move is consistent across the contract, the checker (exit 0, PASS, revision 11), SMP1 and CLI back-pins, WORK_PACKAGES and the machine summary, and the crate tests pass (11/0/1 ignored). Open follow-ups are a stale ADR-0034 status, a version 3 bridge surface the contract never declares, one carried oracle-ordering P3, and three P4s.
