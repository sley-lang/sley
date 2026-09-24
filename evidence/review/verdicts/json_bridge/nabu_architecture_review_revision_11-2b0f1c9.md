<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2b0f1c9f4940c020565137891a49a3769cd02061; role: nabu; field: nabu_architecture_review_revision_11; dispatched: 2026-09-23T12:09:18Z; duration_s: 397; process_exit_code: 0 -->
# Nabu Council review — json_bridge

Harness: claude-code
Reviewed checkpoint: 2b0f1c9f4940c020565137891a49a3769cd02061

What I verified myself:
- **Scope.** `git rev-parse HEAD` returned `2b0f1c9f4940c020565137891a49a3769cd02061`, so the scope matches. Branch is work/succ-context-impl. The only uncommitted files are six untracked verdict transcripts from other lanes, and none of them is a tree edit.
- **Git commands run:**
  - `git diff ab42a3a9..HEAD -- docs/spec/SMP1_JSON_BRIDGE_V1.md`: 8 lines added, 4 removed, from commits a8b4cddb and 8357c243.
  - `git diff ab42a3a9..HEAD` on the bridge checker and generator.
  - `git show 8357c243` / `git show a8b4cddb`.
  - `git show ab42a3a9:docs/spec/SMP1.md`: row 201 at the base, and no mention of version 3 at SMP1 revision 12.
  - `git diff --quiet ab42a3a9 HEAD -- conformance/smp1-json-bridge conformance/smp1 crates/sley-json-bridge`: UNCHANGED_SINCE_BASE.
  - `git log` on `conformance/smp1-json-bridge/v3`: added 2026-09-16 in commits 0c655fc1 through e0ff1371.
- **Checkers run, with exit codes and result lines:**
  - `python3 scripts/check_smp1_json_bridge_contract.py`: exit 0, `"result": "PASS"`, `"revision": 11`, `"status": "S20_420_IMPLEMENTED_REVIEW_PENDING"`, `"problems": []`.
  - `python3 scripts/generate_smp1_json_bridge_table.py --check --protocol-version 1`, then `2`, then `3`: exit 0 each, `{"result": "PASS"}` for the v1, v2 and v3 tables.
  - `python3 scripts/check_smp1_contract.py`: exit 0, `"result": "PASS"`, `"revision": 14`.
  - `uv run --no-project --with blake3 python3 scripts/check_smp1_json_bridge_vector.py`: exit 0, `"result": "PASS"`, 41 methods, 5 vectors, 36 rejections. The plain `python3` run exited 1 with `ModuleNotFoundError: blake3`, which is an environment gap, not a finding.
  - `python3 scripts/test_smp1_json_bridge_table.py -v`: exit 0, `{"cases": 17, "result": "PASS"}`.
  - `python3 -m unittest scripts/test_smp1_contract.py`: exit 0, 16 tests OK.
  - `cargo test -p sley-json-bridge --offline`: 11 passed, 0 failed, 1 ignored (the fixture-refresh emitter). Cargo found the build already fresh for this tree and did not recompile.
  - The SHA256SUMS files for v1, v2 and v3 all verify (checked in Python).
- **In-memory mutation probe of the bridge checker.** I patched `Path.read_text` in memory; nothing in the tree was changed. Baseline passes. A stale SMP1 pin gives `smp1-pin-text`. A stale own revision gives `spec-revision`. Moving the SMP1 status line gives `smp1-revision-pin`. Changing the revision in ADR-0034 to any number still gives rc 0 with no problems.
- **Files read, with line ranges:**
  - `docs/spec/SMP1_JSON_BRIDGE_V1.md:1-311` (whole file)
  - `docs/spec/SMP1.md`: 1-120, 210-240, 320-350, 676-770, 885-905
  - `docs/spec/NATIVE_TEST_ADMISSION_V1.md`: 1-12, 352-364, 536-600
  - `scripts/check_smp1_json_bridge_contract.py:1-325` (whole file)
  - `scripts/generate_smp1_json_bridge_table.py:1-330` (whole file)
  - `crates/sley-json-bridge/src/lib.rs`: 1-80, 200-300, 523-546, 641-670, 700-1218
  - `crates/sley-protocol/src/server.rs:2904-2933`
  - `docs/adr/ADR-0034-json-bridge-boundary.md:1-30`
  - `docs/WORK_PACKAGES.md:48`
  - `docs/audits/S20_420_JSON_BRIDGE_CLOSEOUT.md:1-20`
  - The `json_bridge` section of `machineresearch/sley-2.0/machine-summary.json`, diffed field by field against ab42a3a9.
  - The bridge fixtures `v1/roundtrip.json` and `v1/rejected.json`, and all three `methods.json` headers.
- **Disclosure.** A repository-wide grep for "revision-agnostic" also printed two lines of the untracked Ariadne revision-11 transcript. My P4 on the stale generator comment was already derived from `git diff ab42a3a9..HEAD -- scripts/generate_smp1_json_bridge_table.py` before that grep ran. No other finding draws on that transcript.

## Evidence checked

1. **The re-pin delta is complete for its stated subject.**
   - The spec Status line (`SMP1_JSON_BRIDGE_V1.md:3`) and the composition sentence (`:44`) now read revision 11 and SMP1 revision 14.
   - SMP1's own composition line (`SMP1.md:21-22`) names bridge revision 11.
   - Both checkers enforce the pin in both directions: `check_smp1_json_bridge_contract.py:301-309` and `check_smp1_contract.py:404`. The mutation probe shows each pin fails closed.
   - The machine summary `json_bridge` section changed only in the expected fields: `contract_revision` 11; `current_delta_review` `{contract_revision: 11, ariadne/nabu/vulcan: PENDING}`; `status` `S20_420_IMPLEMENTED_REVIEW_PENDING`; `implementation_complete` false; and a new `status_note`. This matches the checker's rule `implementation_complete == (status == COMPLETE)`.
   - The WORK_PACKAGES row (`docs/WORK_PACKAGES.md:48`) records revision 11.
2. **SMP1 revision 14 does not make the bridge's workspace.open or method-table statements false.**
   - The SMP1 row 201 cells changed from `repository path digest | accepted head` to `none | accepted head summary (appendix A)`. The bridge generator reads the body columns only to decide `reserved` (`generate_smp1_json_bridge_table.py:101-113`), and row 201 stays `reserved: false`.
   - The generated row is still `{family: repository, name: workspace.open, owner: S20-390, reserved: false, tag: 201}` in v1, v2 and v3. The table bytes are unchanged since ab42a3a9, `--check` passes, and the checksums verify.
   - SMP1 keeps the owner column at the base owner on purpose because it feeds `methods.json` (`SMP1.md:334-337`). So the bridge's claim that "the method table has no body columns" holds for the tables it generates.
   - Counts are unchanged: v1 has 41 rows (37 dispatched) and v2 has 43 (39 dispatched).
3. **Bytes the bridge produces and consumes are unaffected.**
   - The crate treats every body as opaque hex: the `body` field at `lib.rs:846` and `:883`. No bridge code, oracle, fixture or fuzz seed mentions 201, `revision_summary`, `open_summary` or `IndexSnapshotId`.
   - The non-empty-201 refusal is server dispatch (`server.rs:2905-2907`: `PayloadInvalid`). It happens after the codec, which `frame_from_json` reuses (`lib.rs:968-975`). A JSON request for 201 with a body still bridges on shape alone and the server refuses it, which is consistent with the bridge's rule of no validation beyond shape (`SMP1_JSON_BRIDGE_V1.md:42-43`).
   - Field 9 is optional by omission and lives inside the opaque response body, so it cannot surface as a bridge shape.
   - The round-trip vectors cover only the hello and `query.root` frames from `conformance/smp1/v1/accepted.json`, which did not change.
4. **Protocol-version wording is consistent with SMP1 revision 14.** Bridge section 8 (`:252-255`: a hello claim below 1 is DOWNGRADE, above 1 is VERSION_UNSUPPORTED) agrees with SMP1 revision 14, under which every hello frame travels as frame version 1 (`SMP1.md:230-231`). The section 10 rule never admitting entity methods on an expected-version-1 frame agrees with SMP1's filtering of tags 306 and 307 under version 1 (`SMP1.md:228-229`).
5. **What revision 14 newly brings into the composition.** SMP1 revision 12 never named version 3. Revision 14 does, as a selectable version (`SMP1.md:220-227`) and as a selection that answers `open_summary` for row 201 (`:62-69`, `:682`, `:722-725`). Bridge revision 11 never mentions version 3 anywhere, yet the crate and the v3-capable CLI carry a live v3 bridge surface. This is Finding 1.

## Findings

[P3] [undeclared-surface] docs/spec/SMP1_JSON_BRIDGE_V1.md:122-127,148-158,297-311 - Bridge revision 11 re-pins SMP1 revision 14, the first SMP1 revision that names protocol version 3 (SMP1.md:62-69, 220-227, 722-725), but it never declares the version 3 surface the crate carries under this contract's name. (a) `conformance/smp1-json-bridge/v3/methods.json` has the header `"contract": "docs/spec/SMP1_JSON_BRIDGE_V1.md"`, while the checker comment (check_smp1_json_bridge_contract.py:20-22) assigns ownership to the native family. (b) The exports `METHOD_TABLE_V3_JSON`, `hello_to_json_for_version`, `frame_value_for_version` and `frame_from_value_for_version` (lib.rs:63-72, 804, 866, 1142) are missing from section 10's list of exports "the crate does carry". (c) The v3 Hello rendering (lib.rs:1010-1069) emits a fifth `features` key, `native_tests`. That key is declared in no contract: NATIVE_TEST_ADMISSION:359 names the bit `native_tests_v1`, not the JSON field. The emission contradicts section 1's exact-shape rule and section 2's four-field `features` record. The crate's own `hello_from_json` (lib.rs:1161-1181, FEATURE_FIELDS at :523) refuses that text with SHAPE_INVALID, and the live CLI emits it (sley-cli/src/lib.rs:526). This predates revision 11 (landed 2026-09-16) and no bridge revision has reviewed it. The re-pin makes the gap part of the composed authority. - Closure: a bridge revision that declares the v3 table and names its row owner (NATIVE_TEST_ADMISSION appendix D) and its generator; lists the v3 exports; declares the `native_tests` feature field and whether the v3 Hello text is render-only or has a reader. Alternatively, an explicit delegation clause that the checker pins with a marker. The WORK_PACKAGES S20-420 row should also stop describing only the v1 and v2 tables.
[P3] [ownership-record] docs/adr/ADR-0034-json-bridge-boundary.md:3-18 - ADR-0034 still says "the S20-420 contract is a draft at revision 10" and has no revision-11 record. The closeout docs/audits/S20_420_JSON_BRIDGE_CLOSEOUT.md:3 still says revision 9. The re-pin updated WORK_PACKAGES.md:48 but not the package's own decision record. The checker's ADR_MARKERS (check_smp1_json_bridge_contract.py:66-74) are revision-agnostic: my in-memory probe that set the ADR to "revision 3" still returned rc 0. This is the third consecutive round with this issue (Nabu revision-9 and revision-10 P3s in evidence/review/verdicts/json_bridge/delta_review_nabu-43f2f5b.md:83 and delta_review_nabu-1023a31.md:72). - Closure: add a revision-11 record to ADR-0034, and either refresh the closeout or explicitly mark its revision as historical. Add an ADR marker derived from SPEC_REVISION, with a revert test showing that a stale ADR revision fails the checker.
[P4] [stale-comment] scripts/generate_smp1_json_bridge_table.py:27-31 - The generator comment says "SMP1.md stays revision 13". SMP1 is at revision 14, and NATIVE appendix D pins revision 14. Commit a8b4cddb updated this comment for revision 13, but the revision-14 re-pin (8357c243) missed it. - Closure: set the comment to 14 or make it revision-agnostic.
[P4] [restated-authority] docs/spec/SMP1_JSON_BRIDGE_V1.md:31-33 - The Status line paraphrases SMP1 revision 14 as defining `open_summary` "under version 2 and every later selection". It drops the owner's qualifier "whose method table includes version 2's row 201" (SMP1.md:62-64, 682, 722-725), and the machine-summary `status_note` repeats the broader wording. Nothing is false today, because version 3 is the only later selection and it carries row 201. But a consumer restating an owner's version scope is exactly the drift the SMP1 revision-13 REVISE addressed. - Closure: cite SMP1 appendix A row 201 instead of paraphrasing it, or quote the qualifier verbatim.

## Assessment

The revision 11 delta does what it claims.
- It re-pins SMP1 revision 14 under its own dated revision.
- The pin is enforced in both directions and fails closed, as the mutation probe shows.
- The machine summary is consistent and moves the package off COMPLETE with PENDING current-delta lanes.
- No bridge clause, encoding, generated table, fixture or crate byte changed.

Nothing the package states about `workspace.open`, the version 1 and 2 method tables, or the bytes it produces or consumes is made false by SMP1 revision 14:
- Row 201's column edit leaves the generated row identical.
- Bodies, including field 9, stay opaque.
- The non-empty-body refusal is a server dispatch judgment downstream of the codec the bridge reuses.

Ownership and dependency direction are clean for the delta: the bridge composes SMP1 and does not restate it. The exception is the P4 paraphrase.

Two P3 issues remain, and both concern authority and evidence binding rather than the delta's correctness:
- The version 3 bridge surface is live under this contract's name but undeclared by it.
- ADR-0034 and the closeout are stale again, as three consecutive rounds now show.

Neither blocks acceptance of the pin. My lane accepts revision 11 with these findings open. This review claims nothing about release readiness or GA.

VERDICT: PASS_0_P0_0_P1_0_P2_2_P3_2_P4
SECTION: json_bridge
FIELD: nabu_architecture_review_revision_11
SCOPE_SHA: 2b0f1c9f4940c020565137891a49a3769cd02061
FINDINGS: [P3] [undeclared-surface] docs/spec/SMP1_JSON_BRIDGE_V1.md:122-127,148-158,297-311 - revision 11 re-pins SMP1 revision 14, the first SMP1 revision naming protocol version 3, but never declares the v3 surface the crate carries under this contract's name: the v3 methods.json header names this contract as its contract; section 10 omits METHOD_TABLE_V3_JSON, hello_to_json_for_version and the frame/value for_version exports; the v3 Hello emits an undeclared fifth features key native_tests, contrary to the section 1/2 exact shape, which the crate's own hello_from_json refuses and the live CLI emits (lib.rs:63-72,1010-1069,1142-1146,1161-1181; sley-cli/src/lib.rs:526); pre-existing since 2026-09-16 - a bridge revision declaring the v3 table and its owner, the v3 exports and the native_tests field (reader or render-only), or a checker-pinned delegation clause, plus a v3 mention in the WORK_PACKAGES row; [P3] [ownership-record] docs/adr/ADR-0034-json-bridge-boundary.md:3-18 - ADR-0034 still says draft at revision 10 with no revision-11 record, the closeout says revision 9, and ADR_MARKERS (check_smp1_json_bridge_contract.py:66-74) are revision-agnostic (a probe setting the ADR to any revision still passes); third consecutive round - a revision-11 ADR record, the closeout refreshed or marked historical, and an ADR marker derived from SPEC_REVISION with a revert test; [P4] [stale-comment] scripts/generate_smp1_json_bridge_table.py:27-31 - the comment says SMP1.md stays revision 13 while SMP1 is revision 14 - set to 14 or make revision-agnostic; [P4] [restated-authority] docs/spec/SMP1_JSON_BRIDGE_V1.md:31-33 - the Status line paraphrase "under version 2 and every later selection" drops SMP1's qualifier "whose method table includes version 2's row 201" (SMP1.md:62-64,722-725), and the machine-summary status_note repeats it - cite SMP1 appendix A row 201 or quote the qualifier
SUMMARY: The revision 11 delta correctly re-pins SMP1 revision 14: both checkers enforce the pin in both directions and fail closed on stale values, the machine summary is consistent, and no bridge table, fixture or crate byte changed. The row 201 cell edit leaves the generated method row identical, and 201 bodies, including field 9, stay opaque bytes whose non-empty-body refusal is server-side, so nothing the bridge states about workspace.open, the v1/v2 tables or its bytes becomes false. Two P3s remain: the version 3 bridge surface, now inside the composed authority, is undeclared by this contract (including an undeclared native_tests Hello field the crate's own reader refuses), and ADR-0034 and the closeout are stale again under revision-agnostic checker markers. Two P4s cover a stale generator comment and a paraphrase that drops SMP1's version-scope qualifier.
