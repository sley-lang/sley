<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: nabu; field: nabu_architecture_review_revision_12; dispatched: 2026-09-23T13:45:33Z; duration_s: 527; process_exit_code: 0 -->
# Nabu Council review — json_bridge

Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

What I verified myself:
- **Scope.** `git rev-parse HEAD` returned `26d050e629b669ef4acbd54826ef4008c05a061d`, so the scope matches. The branch is work/succ-context-impl. The only uncommitted files are three untracked verdict transcripts from other lanes. I made no tree edits.
- **Git commands run:**
  - `git log --oneline 2b0f1c9f..HEAD`: 12 commits.
  - `git diff --stat 2b0f1c9f..HEAD`.
  - `git diff 2b0f1c9f..HEAD` on:
    - `docs/spec/SMP1_JSON_BRIDGE_V1.md`
    - `scripts/check_smp1_json_bridge_contract.py`, `scripts/test_smp1_json_bridge_contract.py` (new), `scripts/generate_smp1_json_bridge_table.py`, `scripts/check_smp1_json_bridge_vector.py`, `scripts/generate_smp1_json_bridge_fixtures.py`
    - `crates/sley-json-bridge`, `conformance/smp1-json-bridge`
    - ADR-0034, the S20-420 closeout, `docs/WORK_PACKAGES.md`
    - `docs/spec/SMP1.md`, `scripts/check_smp1_contract.py`, `scripts/test_smp1_contract.py`
  - `git diff --name-only 60be11c8..HEAD`: no bridge path changed after the fuzz-proof refresh.
  - `git log -S` on:
    - the SMP1 "bit 4 extended_execute" line: dde9a21e, 2026-09-05
    - the crate's `"native_tests"`: 0c655fc1, 2026-09-16
    - the NATIVE sentence "JSON bridge maps these exact typed records": 76cd3dc4, 2026-09-16
- **Checkers and tests run, with exit codes and result lines:**
  - `python3 scripts/check_smp1_json_bridge_contract.py`: exit 0, `"result": "PASS"`, `"revision": 12`, `"status": "S20_420_IMPLEMENTED_REVIEW_PENDING"`, `"problems": []`.
  - `python3 scripts/test_smp1_json_bridge_contract.py -v`: 4 tests, OK.
  - `python3 scripts/check_smp1_contract.py`: exit 0, `"result": "PASS"`, `"revision": 15`.
  - `python3 scripts/generate_smp1_json_bridge_table.py --check --protocol-version 1`, then `2`, then `3`: `{"result": "PASS"}` each.
  - `python3 scripts/test_smp1_json_bridge_table.py`: `{"cases": 17, "result": "PASS"}`.
  - `uv run --no-project --with blake3 python3 scripts/check_smp1_json_bridge_vector.py` (run through a Python subprocess): rc 0, `"result": "PASS"`, 41 methods, 5 vectors, 37 rejections.
  - `python3 scripts/check_cli_contract.py`: exit 0, PASS, revision 10. This is the consumer pin of bridge revision 12.
  - `python3 scripts/check_smp1_json_bridge_persistent_fuzz_slice.py`: `"result": "PASS"`, scope `SMP1_JSON_BRIDGE_TEXT_AND_FRAME_ROUND_TRIP_ONLY`.
  - `cargo test --locked --offline -p sley-json-bridge`: 12 passed, 0 failed, 1 ignored (the fixture emitter). This includes the new `the_version_3_hello_rendering_is_render_only`.
  - `cargo test --locked --offline -p sley-cli --test cli v3_`: 13 passed, 0 failed.
- **Probes (all in memory; the tree was not modified):**
  - **Emitter vs fixture.** I ran the print-only ignored emitter `emit_smp1_json_bridge_vectors_for_fixture_refresh -- --ignored --nocapture` (rc 0) and compared its 37 rejections with `conformance/smp1-json-bridge/v1/rejected.json` (`mutations`). They are identical in order and content. The new `hello-version-above-with-request-id` gives `PROTOCOL_VERSION_UNSUPPORTED`.
  - **Base oracle vs HEAD fixtures.** The base oracle (`git show 2b0f1c9f:scripts/check_smp1_json_bridge_vector.py`, run through uv and blake3 with its `__future__` import hoisted) returns rc 1 against the HEAD fixtures, with the problem `rejection:hello-version-above-with-request-id:expected=PROTOCOL_VERSION_UNSUPPORTED:observed=PROTOCOL_FRAME_INVALID`. So the new vector discriminates.
  - **Checker mutation probes** (patching the checker's `read`):

    | Mutation | rc | Result |
    |---|---|---|
    | Baseline | 0 | pass |
    | Drop the ADR "Revision 12 record (" | 1 | `adr-revision-record` |
    | Break the section 11 export marker | 1 | `spec-marker:…` |
    | Composition pin set to 14 | 1 | `smp1-pin-text`, `composition-sentence:smp1-revision-14` |
    | WORK_PACKAGES row set to "revision 3" | 0 | no problems |
    | Render-only sentence reworded | 0 | no marker (the behavior is pinned by the crate test) |

  - **Live v3 hello.** `cargo run -p sley-cli --bin sley -- hello --json --protocol-profile v3-capable` (run through Python) exits rc 0. It emits `"features":{"cancel":true,"checksum":false,"json_bridge":false,"native_tests":true,"stream":true}`.
- **Files read, with line ranges:**
  - `docs/spec/SMP1_JSON_BRIDGE_V1.md:1-363` (whole file)
  - `scripts/check_smp1_json_bridge_contract.py:1-364` (whole file)
  - `docs/adr/ADR-0034-json-bridge-boundary.md:1-70` (whole file)
  - `crates/sley-json-bridge/src/lib.rs`: 270-299, 523, 641-670, 735-1181
  - `crates/sley-json-bridge/src/tests.rs`: 880-920, plus the diff
  - `crates/sley-protocol/src/lib.rs`: 55-84, 1328-1342, 1423-1429, 1754-1768
  - `scripts/check_smp1_json_bridge_vector.py`: 290-450
  - `docs/spec/NATIVE_TEST_ADMISSION_V1.md`: 360-371, 530-586
  - `docs/spec/SMP1.md`: the rev 14→15 diff
  - `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md`: 860-963 (§13.2-13.5)
  - `docs/WORK_PACKAGES.md:48`
  - `crates/sley-cli/tests/cli.rs`: 1270-1297
  - `evidence/review/verdicts/json_bridge/delta_review_nabu-1023a31.md`: 68-76
  - My revision-11 transcript (whole)
  - The `json_bridge` section of `machineresearch/sley-2.0/machine-summary.json`, diffed key by key against 2b0f1c9f

## Evidence checked

**Status of my round-7 (revision 11) findings:**
- CLOSED — [P3] [undeclared-surface] docs/spec/SMP1_JSON_BRIDGE_V1.md:122-127,148-158,297-311 (version 3 surface undeclared)
- CLOSED — [P3] [ownership-record] docs/adr/ADR-0034-json-bridge-boundary.md:3-18 (ADR stuck at revision 10; revision-agnostic ADR markers; closeout at revision 9)
- CLOSED — [P4] [stale-comment] scripts/generate_smp1_json_bridge_table.py:27-31 (comment said SMP1 revision 13)
- CLOSED — [P4] [restated-authority] docs/spec/SMP1_JSON_BRIDGE_V1.md:31-33 (qualifier dropped from the `open_summary` paraphrase)

1. **The version 3 surface is now declared (P3 #1).**
   - **Table and owners.** Section 11 (`:329-362`) names the v3 table, its 46 rows and its union shape. That matches the table itself:
     - The v3 table has 46 rows, with reserved tags [305, 503].
     - The v2 table has reserved tags [305, 503, 601, 602].
     - The header carries `contract` = the bridge, `source` = SMP1, and `v3_source` = NATIVE.
     - It separates the rendering owner (this contract) from the row authority (NATIVE appendix D, `NATIVE_TEST_ADMISSION_V1.md:552-585`).
   - **Exports.** Section 11 lists every version-selected export, and the checker requires them (`check_smp1_json_bridge_contract.py:114-122`). Section 10's withdrawn "no capable symbol is required" is annotated (`:325-327`).
   - **The `native_tests` key.** It is declared with its mask. Mask 32 is `FEATURE_NATIVE_TESTS_V1` (`sley-protocol/src/lib.rs:74`), which is "bit32 native_tests_v1" in NATIVE:366. The key is declared render-only.
   - **Render-only behavior is tested.** `the_version_3_hello_rendering_is_render_only` (tests.rs:923-949) asserts:
     - five feature keys;
     - `JSON_BRIDGE_METHOD_UNKNOWN` for the v3 text;
     - `JSON_BRIDGE_SHAPE_INVALID` for a v1 hello with an added `native_tests` key;
     - an error for version 4.
   - **The reader is unchanged.** `hello_from_json` (lib.rs:1161-1181) still reads the four-key `FEATURE_FIELDS`.
   - **WORK_PACKAGES.** The row at `docs/WORK_PACKAGES.md:48` now names the declared v3 surface.
2. **ADR and closeout are current (P3 #2).**
   - ADR-0034:3 reads "draft at revision 12". It has Revision 11 and 12 records (`:17-24`) and a dated line (`:26`).
   - The closeout Status now says it records the revision 9 implementation and defers current state to the summary.
   - `pin_problems` (`check_smp1_json_bridge_contract.py:129-148`) anchors the ADR current line and record on `SPEC_REVISION`. It also reads the SMP1 pin from the single composition sentence, so a history line cannot satisfy it.
   - The four revert tests pass, and my probes confirm each anchor fails closed. `test_smp1_json_bridge_contract.py` is in `make quick` (Makefile:107).
3. **Generator comment (P4 #1).** `generate_smp1_json_bridge_table.py:27-32` no longer names an SMP1 revision.
4. **Qualifier restored (P4 #2).**
   - The Status line (`:31-35`) quotes SMP1's qualifier "under every later selection whose method table includes version 2's row 201". The ADR revision 11 record carries it too.
   - The current machine-summary `status_note` no longer paraphrases the owner. The older wording survives only inside "Previous note" history, which is correct preservation.
5. **The oracle order repair (Ariadne P3, checked for evidence binding).**
   - The oracle (`check_smp1_json_bridge_vector.py:338-352`) now judges in this order: bounds, then version split, then header. That matches the crate: the bridge bounds rule, then `validate_header` (lib.rs:894-903), with the codec judging version first.
   - The crate matrix, the fixture and the HEAD oracle all agree.
   - The base oracle fails on the new vector, so the vector pins the order rather than restating it.
   - `EXPECTED_REJECTION_COUNT` is 37, and the v1 SHA256SUMS were updated for `rejected.json` only.
6. **SMP1 revision 15 re-pin.**
   - The pin holds in both directions:
     - The bridge names SMP1 15 at `:56`, anchored.
     - SMP1 names bridge 12 at `SMP1.md:21-22`, checked at `check_smp1_contract.py:410`.
     - The CLI names both (`check_cli_contract.py` PASS).
   - Revision 15's per-selection filter touches no bridge statement:
     - The generated tables come from SMP1 appendix A and NATIVE appendix D, not from negotiation.
     - Section 10's "never admits entity methods on an explicit ordinary expected-1 frame" agrees with the v1 drop of 306/307.
     - Under v3 without the native bit, the bridge still *names* 601/602/605-607 (section 11: "Under version 3 a frame names methods from the version 3 table"). The server refuses them as not negotiated, which fits the bridge's shape-only rule.
     - The handshake identity stays opaque render data (`:155-159`).
7. **Machine summary.**
   - `contract_revision` is 12.
   - `current_delta_review` is `{12, PENDING×3}`.
   - The status is unchanged at `S20_420_IMPLEMENTED_REVIEW_PENDING`, and `implementation_complete` is false.
   - My revision 11 verdict is recorded verbatim (`PASS_0_P0_0_P1_0_P2_2_P3_2_P4`) with its transcript path. No terminal status was claimed.
8. **Fuzz lane.** No bridge path changed after the 60be11c8 proof refresh, and the slice checker passes.

## Findings

[P4] [stale-comment] scripts/check_smp1_json_bridge_contract.py:20-22,225-230,335-337 - Three checker comments contradict revision 12. (a) `:20-22` says the v3 table "is owned by the native draft family …, not by this frozen contract". Section 11 (`SMP1_JSON_BRIDGE_V1.md:340-341`) makes this contract the rendering's owner and NATIVE the row authority, and this contract is a draft, not frozen. (b) `:227-228` says v3 "overrides the frozen reserved 601/602/605/606/607 rows". 605-607 are in neither frozen table (v1/v2 reserved = 305, 503, 601, 602), and section 11 bullet 1 states this correctly. (c) `:337` says "no capable bridge symbol is required", the sentence revision 12 withdrew from section 10 (`:325-327`), while `CRATE_MARKERS` (`:114-122`) now require the version-selected exports. - Closure: align the three comments with section 11: rendering owner vs row authority, 605-607 added rather than overridden, and version-selected exports required.
[P4] [unanchored-record] scripts/check_smp1_json_bridge_contract.py:80,213-216 - `WORK_PACKAGE_MARKERS` are still revision-agnostic. My in-memory probe rewrote the S20-420 row (`docs/WORK_PACKAGES.md:48`) to "(revision 3, 2026-09-23" and the checker still returned rc 0 with no problems. `pin_problems` anchors only the ADR half of the revision-10 Nabu P3 (`delta_review_nabu-1023a31.md:72`: "ADR_MARKERS and WORK_PACKAGE_MARKERS … remain revision-agnostic"). The row is correct today. - Closure: anchor the row's `` `docs/spec/SMP1_JSON_BRIDGE_V1.md` (revision {SPEC_REVISION}, `` text, as `check_cli_contract.py:103` does for its row, and add a revert case to `test_smp1_json_bridge_contract.py`.
[P4] [declared-surface-precision] docs/spec/SMP1_JSON_BRIDGE_V1.md:344-352 - Section 11 declares the version-selected exports but is imprecise at three edges. (a) It states out-of-range behavior only for `hello_to_json_for_version`. `frame_value_for_version` and `frame_from_value_for_version` accept any version without refusal and resolve names asymmetrically: render (lib.rs:809-819) uses v1 for ≤1, v3 for 3 and v2 otherwise; read (lib.rs:279-299) uses v3, v2 and otherwise v1. At version 0 or ≥4, a rendered `entity.version` therefore does not read back (METHOD_UNKNOWN). The crate's own comments also disagree (lib.rs:741 "Any other version resolves version 1" vs :811-813 "anything else version 2"). This is latent: the text exports fail closed at `validate_for_version` (sley-protocol lib.rs:1328-1334), and no caller passes such a version (sley-cli imports neither value export). Derived statically, not executed. (b) "after the frozen four" (`:352`) is true only of declared order. Emission is lexicographic (section 1, BTreeMap `Map`), and the live v3 hello emits `cancel, checksum, json_bridge, native_tests, stream` (observed). (c) The v3 key set names bits 0-3 and 5 but not SMP1's bit 4 `extended_execute` (`SMP1.md:168`). A hello carrying bit 4 therefore fails `JSON_BRIDGE_SHAPE_INVALID` under every rendering (lib.rs:641-645), yet section 8 (`:275-277`) describes that refusal as applying to bits with "no frozen name". The behavior is fail-closed. The imprecision predates this revision, but section 11 now restates the key set. - Closure: section 11 states that the value-level exports admit only versions 1, 2 and 3 and refuse others (code plus test), or declares the fallback; says "declared after the frozen four, emitted in section 1 order"; and names bit 4 as deliberately unrendered (or renders it).
[P4] [cross-contract-claim] docs/spec/NATIVE_TEST_ADMISSION_V1.md:548-550 - NATIVE says "JSON bridge maps these exact typed records". The bridge carries every owner body, native request and response records included, as opaque hex: see the intro (`SMP1_JSON_BRIDGE_V1.md:59-60`), section 7 (`:250-251`, "JSON forms of owner bodies … stay hex"), and section 11 (`:333-334`, "exactly as follows, and nothing more"). Revision 12 names NATIVE appendix D as the v3 row authority but leaves this sentence unreconciled, so two owners disagree on whether the bridge maps native bodies. The sentence dates from 76cd3dc4 (2026-09-16). There is no behavior impact, because the crate treats bodies as opaque (lib.rs:846, 883). - Closure: the NATIVE owner states that native bodies cross the bridge as opaque hex (or scopes the sentence to a future N7 bridge revision), or bridge section 11 records that no native typed-record mapping exists.

## Assessment

Revision 12 does what §13.3 claims for this package, and I checked each claim against code, fixtures, tests and checker behavior rather than the record:

- **Version 3 surface.** It is now a declared, checker-gated part of this contract. Ownership is split cleanly: the bridge owns the rendering, NATIVE appendix D owns the rows, and SMP1 owns versions 1 and 2. The render-only `native_tests` key keeps the section 1 and 2 reader shapes intact, and a crate test pins that.
- **Oracle.** It now follows the crate's version-before-header order. The new vector is bound to the crate's emitted matrix, and it provably fails the base oracle.
- **Pins.** The ADR record and the SMP1 composition pin are anchored on the checker's revision constants, with revert tests in `make quick`. The pins fail closed in both directions, and the consumer (CLI) pins agree.
- **Unchanged material.** No encoding, generated table, crate source or v1/v2 rendering changed.
- **SMP1 revision 15.** Its per-selection filter leaves every bridge statement true, because the bridge is shape-only and names tables, not negotiated selections.

Dependency direction is clean: the bridge composes SMP1 and NATIVE and restates neither.

What remains is four P4 notes, none of which blocks acceptance:
- stale checker comments;
- the WORK_PACKAGES half of an older marker finding;
- three precision edges in section 11 (latent out-of-range fallbacks on the value exports, the emission-order phrase, and the unrendered bit 4);
- an unreconciled NATIVE sentence about typed-record mapping.

My lane accepts the package at contract revision 12. This review claims nothing about release readiness or GA.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_4_P4_PRIOR_P3_P4_CLOSED
SECTION: json_bridge
FIELD: nabu_architecture_review_revision_12
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS: [P4] [stale-comment] scripts/check_smp1_json_bridge_contract.py:20-22,225-230,335-337 - comments say the v3 table is owned by the native family "not by this frozen contract" (section 11 makes the bridge the rendering owner, NATIVE the row authority), that v3 "overrides the frozen reserved 601/602/605/606/607 rows" (605-607 are in no frozen table), and "no capable bridge symbol is required" (withdrawn from section 10; CRATE_MARKERS now require the versioned exports) - align the comments with section 11; [P4] [unanchored-record] scripts/check_smp1_json_bridge_contract.py:80,213-216 - WORK_PACKAGE_MARKERS are revision-agnostic (a probe setting the S20-420 row to revision 3 passes rc 0), the WORK_PACKAGES half of the revision-10 Nabu P3 left open by pin_problems - anchor the row's contract revision on SPEC_REVISION with a revert case; [P4] [declared-surface-precision] docs/spec/SMP1_JSON_BRIDGE_V1.md:344-352 - section 11 leaves the out-of-range behavior of frame_value_for_version/frame_from_value_for_version undeclared, and the crate falls back asymmetrically (render v2, read v1 for version 0 or >=4; lib.rs:279-299,809-819; latent, since the text exports fail closed at validate_for_version and there is no caller); "after the frozen four" holds only in declared order (emission is lexicographic, observed cancel,checksum,json_bridge,native_tests,stream); SMP1 bit 4 extended_execute (SMP1.md:168) has no key and fails SHAPE_INVALID, although section 8 limits that refusal to bits with "no frozen name" - declare the version domain of the value exports (refuse others, with a test), state emission order, and name bit 4 as unrendered; [P4] [cross-contract-claim] docs/spec/NATIVE_TEST_ADMISSION_V1.md:548-550 - NATIVE says "JSON bridge maps these exact typed records" while the bridge keeps all owner bodies, native ones included, as opaque hex (bridge :59-60, :250-251, :333-334); revision 12 names NATIVE as the v3 row authority without reconciling this; pre-existing since 76cd3dc4, no behavior impact - the NATIVE owner states native bodies stay opaque hex (or scopes the sentence to a future N7 revision), or bridge section 11 records that no typed mapping exists
SUMMARY: Revision 12 closes all four of my revision-11 findings. Section 11 declares the version 3 table, its NATIVE row authority, the versioned exports and the render-only native_tests key, and a crate test pins it; ADR-0034 and the SMP1 composition pin are anchored on the checker's revision constants with revert tests; the generator comment and the SMP1 qualifier are fixed. The oracle's hello version-before-header repair is bound to the crate's emitted rejection matrix, and the new vector provably fails the base oracle. All bridge, SMP1 and CLI checkers, the vector oracle, the table drift gates, the fuzz-slice checker, and the bridge and CLI v3 tests pass at 26d050e6. Four P4 notes remain (stale checker comments, an unanchored WORK_PACKAGES revision, three section 11 precision edges, and an unreconciled NATIVE typed-mapping sentence), and none blocks acceptance at contract revision 12.
