# cli current-delta review — Ariadne (contract lane) — CLI r8 (delta 1023a31 over r7 at 43f2f5b) at scope 1023a31

Section: `cli`. Field judged: `current_delta_review.ariadne`. Role: Ariadne, contract lane.

## Scope verification

`git rev-parse HEAD` = `1023a314b18834e36a7daa020fc9078807a3ac16` (matches SCOPE_SHA; branch main; working tree clean).
Summary block (`machineresearch/sley-2.0/machine-summary.json` → `cli`): `contract_revision: 8`,
`status: S20_430_IMPLEMENTED_REVIEW_PENDING`, `current_delta_review: {contract_revision: 8, ariadne: PENDING, nabu: PENDING, vulcan: PENDING}`.
Prior round: revision 7 at scope `43f2f5b` (`delta_review_ariadne-43f2f5b.md`, REVISE_0_P0_0_P1_1_P2_4_P3; Nabu REVISE 1 P2; Vulcan PASS).

Delta bounded with `git diff 43f2f5b..1023a31`: `docs/spec/SLEY_CLI_V1.md` (+52/-, Status rev 8, bridge pin 10 at line 28,
section 8 end-of-input rule at lines 272-276, corrected "### Revision 7" block and new "### Revision 8" block at lines 330-356),
`docs/adr/ADR-0035-thin-cli-boundary.md` (status "draft at revision 8", revision-7 and revision-8 records, date line),
`docs/WORK_PACKAGES.md:49` (revision 8 row), `docs/spec/SMP1.md:22-23` (composition pins bridge 10 / CLI 8),
`scripts/check_cli_contract.py` (`SPEC_REVISION = 8`, `BRIDGE_REVISION = 10`, `ADR_MARKERS` + "Revision 8 record (2026-09-14)",
`WORK_PACKAGE_MARKERS` → "(revision 8, 2026-09-14, ADR-0035; revision-6 new-delta review PASS"),
`scripts/test_cli_contract.py:41` (`passing_review(revision=CHECKER.SPEC_REVISION)`),
`scripts/test_current_contract_review.py` (CLI/bridge/SMP1 cases derive from the checkers' `SPEC_REVISION`/`CONTRACT_REVISION`),
`scripts/test_smp1_contract.py` (reverse pins derived from the two status lines), `Makefile` quick recipe (+7 `scripts/test_*.py` lines),
`crates/sley-cli/tests/cli.rs` (+60: `MAX_JSON_ELEMENTS` import and the new test `a_json_line_at_the_element_ceiling_is_refused_and_ends_the_input`).
`git diff 43f2f5b..HEAD --stat -- crates/sley-cli/src crates/sley-json-bridge/src/lib.rs` is empty: no production code moved.

## Inputs read in full

- `docs/spec/SLEY_CLI_V1.md` lines 1-30 (Status, pins), 215-245 (section 5 evidence citations, section 6), 255-290 (sections 7-8), 330-356 (revision 7/8 notes).
- `scripts/check_cli_contract.py` (pins, ADR/WP/CRATE markers); `scripts/test_cli_contract.py` (whole file); `scripts/test_current_contract_review.py` (revision derivation lines 82-179); `Makefile` quick recipe; `ls scripts/test_*.py`.
- `crates/sley-cli/src/lib.rs` lines 696-726 (`read_line`), 1085-1125 (serve loop), the `MAX_JSON_TEXT_BYTES`/`METHOD_TABLE_V2_JSON` uses (lines 17, 444, 698, 1105).
- `crates/sley-cli/tests/cli.rs` new test (diff hunk, lines 494-549).
- `crates/sley-json-bridge/src/lib.rs` lines 26-51 (ceilings), 430-472 (`check_resources`).
- `docs/adr/ADR-0035-thin-cli-boundary.md` lines 1-20; `docs/WORK_PACKAGES.md:49`; `docs/audits/S20_430_THIN_CLI_CLOSEOUT.md:1-8`; `docs/spec/SMP1.md:19-24`; `docs/spec/SMP1_JSON_BRIDGE_V1.md:3` and 167-183 (section 3 ceilings the CLI rule cites).
- Prior verdict footers for the revision-7 round (`delta_review_{ariadne,nabu,vulcan}-43f2f5b.md`).

## Tool results (run with `SLEY2_MASTER_GOAL=/home/greyforge/machineresearch/Sley2.0mastergoal.md`)

- `python3 scripts/check_cli_contract.py` → `{"result": "PASS", "revision": 8, "problems": [], "status": "S20_430_IMPLEMENTED_REVIEW_PENDING", "implementation_present": ["crates/sley-cli"], "new_stable_error_codes": 4}` exit 0.
- `python3 scripts/test_cli_contract.py` → 7 tests OK (both `ValidPinControl` cases, red at revision 7, now green), exit 0.
- `python3 scripts/test_current_contract_review.py` → 6 tests OK (`CliValidBaseline`, `CliTerminalAcceptance` green), exit 0.
- `python3 scripts/check_cli_rules.py` → PASS (`method_names_audited: 43`, `method_tags_audited: 43`, `encode_frame_calls: 1`, `frame_literals: 1`).
- `python3 scripts/check_smp1_contract.py` → PASS, revision 12 (composition pins bridge 10 / CLI 8 accepted); `python3 scripts/test_smp1_contract.py` → 14 tests OK (reverse-pin cases now derive the pins from the status lines).
- `python3 scripts/check_smp1_json_bridge_contract.py` → PASS, revision 10 (the composed authority's own status line).
- `cargo test -p sley-cli -p sley-json-bridge --locked` → `tests/cli.rs` 29 passed, 0 failed (28 at revision 7 plus the new element-ceiling test); sley-json-bridge 11 passed, 1 ignored; exit 0.
- Makefile `quick` recipe now lists `test_cli_contract.py`, `test_cli_rules.py`, `test_current_contract_review.py`, `test_required_contract_index.py`, `test_session_handle_profile.py`, `test_smp1_contract.py`, `test_smp1_json_bridge_table.py` — exactly the seven `scripts/test_*.py` files on disk. (`make quick` itself was not run; it ends in `cargo test --workspace`.)

## Independently re-derived claims

1. **Pins.** Status line revision 8 == `SPEC_REVISION`; line 28 pins bridge revision 10 == `BRIDGE_REVISION` == `SMP1_JSON_BRIDGE_V1.md:3`; line 26 SMP1 revision 12 == `SMP1_REVISION` == `SMP1.md:3`; revision-8 note "SMP1 revision 12 and bridge revision 10" (line 356); `SMP1.md:22-23` pins back bridge 10 / CLI 8 (checked by `check_smp1_contract.py`). All symmetric and checker-enforced.
2. **Prior P2 (gate tests hard-coded to 6, unwired).** `test_cli_contract.py:41` now takes `CHECKER.SPEC_REVISION`; the stale-revision negative uses literal 5 (any revision below the current one is stale, so the negative stays valid across moves); `test_current_contract_review.py:101,179` use `CLI_CHECKER.SPEC_REVISION`. Both suites are `make quick` members and green. Closed.
3. **Prior P3 (revision-7 note "changes nothing the CLI parses").** Lines 330-338 now state the true effect: a fully read JSON line with 1,048,576 or more value positions is answered `JSON_BRIDGE_RESOURCE_LIMIT` and ends the input, "exactly as the text ceiling already did"; the old wording is named as an overstatement. Re-derived: `lib.rs:719-724` routes every line through `frame_from_json[_for_version]`, `check_resources` (bridge lib.rs:457,463) refuses at `positions >= 1_048_576`, and `lib.rs:1105` breaks the loop on `ResourceLimit`. True.
4. **Section 8 end-of-input rule (lines 272-276).** "A JSON line the bridge refuses with `JSON_BRIDGE_RESOURCE_LIMIT` for any of its ceilings (text bytes, nesting depth, or value positions; bridge contract section 3) is answered with that code and likewise ends the input." Matches `lib.rs:1102-1108` (write the rejection, break on `ResourceLimit`, continue on any other bridge error) and the three ceilings of bridge section 3 (lines 169-172). The clause "the line was read in full" is true for the depth and element ceilings (the newline was found inside the `take(MAX_JSON_TEXT_BYTES + 2)` window, lib.rs:698-702) but not for a text-ceiling line longer than `MAX_JSON_TEXT_BYTES + 2` bytes: `read_until` stops at the take limit with no newline, the unread remainder stays in the pipe, and the truncated text (268,435,458 bytes > 268,435,456) is what the bridge refuses. Answer code, report counts, and end-of-input are identical either way, so the observable contract is right; the parenthetical is inexact for one of the three ceilings (editorial).
5. **New test.** `wide = "[" + "0,"*... + "]"` with `MAX_JSON_ELEMENTS` zeros: 1 `[` + 1,048,575 `,` = exactly 1,048,576 positions, so it drives the inclusive boundary the bridge contract now states; depth 1; ~2 MiB text, well under the text ceiling. Asserts two output lines, the refusal's `(None, 0, 0)` header and `42_004 / JSON_BRIDGE_RESOURCE_LIMIT`, `frames_read == 2`, `failed_answers == 1`, `codes["42004"] == 1`, and that the trailing well-formed frame is never answered — the section 8 rule and the section 8 counting rule ("rejected lines included") both exercised. Passes.
6. **Revision-8 note claims.** "Re-pins bridge revision 10 (no behavior change ...)": bridge `src/lib.rs` unchanged since 43f2f5b, only `tests.rs`. True. "every `scripts/test_*.py` suite runs under `make quick`": 7 on disk, 7 in the recipe. True. "current-delta review round on revision 7, Ariadne P2 and Nabu P2": both footers carry `1_P2`. True. "ADR-0035 and `docs/WORK_PACKAGES.md` carry the revision-7 and revision-8 records": ADR-0035:3 "draft at revision 8", lines 16-20 revision 7 and 8 records, date line updated; WORK_PACKAGES:49 "(revision 8, 2026-09-14, ADR-0035; revision-6 new-delta review PASS at repair lane `934eb3f` ...; revision-7 delta round 2026-09-14 REVISE closed by revision 8 ...)". Both are now asserted by `ADR_MARKERS`/`WORK_PACKAGE_MARKERS`. True; prior P3 x2 closed.
7. **"derive the record revision from the checker instead of a literal" (lines 348-350).** True for every CLI, bridge, and SMP1 case in the two suites. `test_current_contract_review.py:138` still asserts the required-contract-index revision as the literal `3`; that case belongs to another section, but the sentence is written about the suite as a whole (editorial).
8. **Status paragraph (lines 13-18).** Revision list now runs through 8; "The revision 7 history is retained as history and does not review revision 8; its new-delta review is pending" is the current state. Prior P4 closed.
9. **Codes and counts.** `| 43000..43003 |` rows with exits 2-5, `new_stable_error_codes: 4`, `commands`/`forbidden`/`offered_hello` summary strings: unchanged and checker-asserted.
10. **Cross-references not moved.** `docs/audits/S20_430_THIN_CLI_CLOSEOUT.md:3` still says "(revision 3)"; pre-existing, outside the delta, pinned by no checker (`grep -n CLOSEOUT scripts/check_cli_contract.py` empty). Editorial.

## Per-item analysis

- Pin move 9 → 10 and the SMP1 reverse pins: exact, enforced both ways. No finding.
- Gate-test wiring: fixtures derive from the checker, suites in `make quick`, both green. P2 closed.
- Section 8 rule + revision-7 correction + new test: the contract now says what `lib.rs:1105` does and a test drives the element ceiling end-to-end. P3 closed; one inexact parenthetical remains (P4).
- ADR-0035 / WORK_PACKAGES: revision-8 records present and checker-pinned. P3 x2 closed.
- Status paragraph: current. P4 closed.
- Residual editorial points: "read in full" for the text ceiling, the suite-wide derivation sentence versus the index literal, and the closeout's revision 3.

```
VERDICT: PASS
SECTION: cli
FIELD: current_delta_review.ariadne
SCOPE_SHA: 1023a314b18834e36a7daa020fc9078807a3ac16
FINDINGS:
[P4] [editorial] docs/spec/SLEY_CLI_V1.md:275 - "the line was read in full" holds for the depth and element ceilings but not for a text-ceiling line longer than MAX_JSON_TEXT_BYTES + 2 bytes: crates/sley-cli/src/lib.rs:698-702 reads at most that many bytes (`take(limit).read_until`), the remainder is never consumed, and the truncated text is what the bridge refuses; answer, counts, and end-of-input are unchanged, so state "read up to the text ceiling" or drop the clause
[P4] [editorial] docs/spec/SLEY_CLI_V1.md:348-350 - the suite-wide sentence "derive the record revision from the checker instead of a literal" is true for every CLI, bridge, and SMP1 case, but scripts/test_current_contract_review.py:138 still asserts the required-contract-index revision as the literal 3 (another section's case; scope the sentence or derive that one too)
[P4] [cross-reference] docs/audits/S20_430_THIN_CLI_CLOSEOUT.md:3 - closeout status still names "(revision 3)" of the contract now at revision 8; pre-existing, pinned by no checker
SUMMARY: The revision-8 delta closes every item of the revision-7 round: the cited gate tests derive their revision from `check_cli_contract.py` (7/7 and 6/6 green) and now run under `make quick` together with every other `scripts/test_*.py` suite; section 8 states the end-of-input rule for all three bridge ceilings and matches `lib.rs:1105`; the revision-7 note is corrected to the true effect of the element ceiling and a new CLI test drives a line with exactly 1,048,576 value positions through JSON mode (refused as 42004, input ended, third frame never answered); ADR-0035 and WORK_PACKAGES carry revision-8 records asserted by the checker; the pins (SMP1 12, bridge 10) are true in both directions and the SMP1 reverse pins pass. No production code changed and the crate tests pass (29). Three editorial points remain (the "read in full" clause for the text ceiling, the suite-wide derivation sentence versus the index literal, and the stale closeout revision); nothing report-grade is open.
```
