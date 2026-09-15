# cli current-delta review — Ariadne (contract lane) — CLI r7 (delta 9efe984) at scope 43f2f5b

Section: `cli`. Field judged: `current_delta_review.ariadne`. Role: Ariadne, contract lane.

## Scope verification

`git rev-parse HEAD` = `43f2f5ba738b587a9a0bb365a55f3096527e3e3d` (matches SCOPE_SHA; branch main).
Summary block (`machineresearch/sley-2.0/machine-summary.json` → `cli`): `contract_revision: 7`,
`status: S20_430_IMPLEMENTED_REVIEW_PENDING`, `current_delta_review: {contract_revision: 7, ariadne: PENDING, nabu: PENDING, vulcan: PENDING}`.
No prior verdict directory `evidence/review/verdicts/cli/` existed before this file.

Delta bounded: revision 6 → 7 is commit `9efe984` ("P-C finals fallout II: ... SMP1/CLI pin moves ...").
Its CLI-relevant hunks: `docs/spec/SLEY_CLI_V1.md` (+13/-3: Status line rev 6→7 and date, bridge pin
8→9 at line 26, new "### Revision 7 (2026-09-14)" block at lines 325-332) and
`scripts/check_cli_contract.py` (`SPEC_REVISION = 7`, `BRIDGE_REVISION = 9`). No crate file is touched
(`git diff 9efe984^..HEAD --stat -- crates/sley-cli` is empty). The last reviewed point for revision 6 is
repair lane `934eb3f` (WORK_PACKAGES row 49); `git diff 934eb3f..HEAD --stat -- crates/sley-cli` touches
only `crates/sley-cli/tests/cli.rs` (+58, commits `7804432` governance exercise wave and `3830022`), no
`src/` change.

## Inputs read in full

- `docs/spec/SLEY_CLI_V1.md` (382 lines, Status rev 7).
- `scripts/check_cli_contract.py` (242 lines, SPEC_REVISION 7, SMP1_REVISION 12, BRIDGE_REVISION 9).
- `scripts/test_cli_contract.py`, `scripts/test_current_contract_review.py` (192 lines), `scripts/check_cli_rules.py` output, `scripts/test_cli_rules.py` output.
- `crates/sley-cli/Cargo.toml`; `crates/sley-cli/src/lib.rs` lines 690-705 (`read_line`, `MAX_JSON_TEXT_BYTES + 2` take limit) and 1090-1125 (serve loop: `Next::Rejected` path, `ResourceLimit` ends input at line 1105).
- `docs/adr/ADR-0035-thin-cli-boundary.md` lines 1-18; `docs/WORK_PACKAGES.md` line 49; `docs/audits/S20_430_THIN_CLI_CLOSEOUT.md` lines 1-8; `docs/spec/ERROR_CODES_V1.md:492`.
- `docs/spec/SMP1.md:3` and `docs/spec/SMP1_JSON_BRIDGE_V1.md:3` status lines (the composed authorities).
- Precedents `evidence/review/verdicts/json_bridge/delta_review-a4b6029.md`, `evidence/review/verdicts/required_contract_index/delta_review-a4b6029.md`.

## Tool results (run with `SLEY2_MASTER_GOAL=/home/greyforge/machineresearch/Sley2.0mastergoal.md`)

- `python3 scripts/check_cli_contract.py` → `{"result": "PASS", "revision": 7, "problems": [], "status": "S20_430_IMPLEMENTED_REVIEW_PENDING", "implementation_present": ["crates/sley-cli"], "new_stable_error_codes": 4}` exit 0.
- `python3 scripts/check_cli_rules.py` → PASS (`method_names_audited: 43`, `method_tags_audited: 43`, `encode_frame_calls: 1`, `frame_literals: 1`, `problems: []`).
- `python3 scripts/test_cli_rules.py` → 10 tests OK.
- `python3 scripts/test_cli_contract.py` → **FAILED (failures=2 of 7)**: `ValidPinControl.test_current_passing_review_accepted` (line 80) and `ValidPinControl.test_pending_review_accepted_before_freeze` (line 85), both `Lists differ: ['review:current-delta-revision:6'] != []` — the positive-control fixture `passing_review()` hardcodes `contract_revision` 6 while the checker now expects 7.
- `python3 scripts/test_current_contract_review.py` → **FAILED (failures=2 of 6)**: `CliValidBaseline.test_valid_baseline_accepted` (line 101, `AssertionError: 7 != 6`) and `CliTerminalAcceptance.test_complete_all_pass_accepted` (line 179, `pass_current_review(section, 6)` assertion). The bridge (9), SMP1 (12), and index (3) cases in the same file pass; only the CLI cases still say 6 (`git log -S` shows the 6 was written in `d313df0`, the rev-6 implementation commit, and `23bd4cf` refreshed the other sections but predates the CLI pin move).
- `cargo test -p sley-cli -p sley-json-bridge --locked` → sley-cli `tests/cli.rs` 28 passed, 0 failed; sley-json-bridge 11 passed, 1 ignored (the ignored test is the `fixture refresh emitter`, not a behaviour test); exit 0.
- Neither `scripts/test_cli_contract.py` nor `scripts/test_current_contract_review.py` is a `make quick` recipe member (grep of `Makefile`); they are named only in `.forge/slices/phase-3-v2-endpoint-offer.json:38` as part of that slice's verification command. The `quick` recipe does run `check_cli_contract.py` and `check_cli_rules.py` (both green).

## Independently re-derived claims

1. **Pins asserted both ways.** `check_cli_contract.py:212-226` anchors its own Status line (`revision (\d+)` == 7), the SMP1 Status line (`docs/spec/SMP1.md:3` → revision 12 == `SMP1_REVISION`), the spec's pin sentence `"SMP1 revision 12 and bridge revision 9"` (present at `SLEY_CLI_V1.md:332`), the bridge Status line (`SMP1_JSON_BRIDGE_V1.md:3` → revision 9 == `BRIDGE_REVISION`), and the spec's composition pin `` `docs/spec/SMP1_JSON_BRIDGE_V1.md` revision 9 `` (present at line 26). A stale pin in either direction fails; the revision-7 sentence "asserts both pins against the composed status lines" is true.
2. **"No behavior change" (crate).** Delta commit touches no crate file; since the rev-6 review point only `tests/cli.rs` changed (additive governance-exercise tests); 28/28 CLI tests pass; rule audit PASS. The CLI's JSON read path still bounds a line at `MAX_JSON_TEXT_BYTES + 2` (lib.rs:698) and delegates every judgment to `frame_from_json`. True for the crate.
3. **"The bridge's revision-9 ceilings and precision notes change nothing the CLI renders or parses."** Partially overstated. Rendering: true (no encoding change in bridge r9). Parsing: the CLI parses through `frame_from_json`, whose r9 element ceiling now rejects a line with ≥ 1,048,576 structural value positions as `JSON_BRIDGE_RESOURCE_LIMIT`; `crates/sley-cli/src/lib.rs:1105` then **ends the input** on any `ResourceLimit`, where before r9 such a line (under 256 MiB and ≤ 32 deep) was parsed and answered (`SHAPE_INVALID`/codec code) with reading continuing. The CLI contract's own rule at §8 lines 270-271 names only "a JSON line above the text ceiling" as the case that ends input; the code ends input on every `ResourceLimit` (depth, pre-existing; element count, new via r9). The composition pin move therefore does carry a CLI-observable behaviour change on the JSON path that the revision-7 note denies and §8 does not describe.
4. **Revision-history text.** The "### Revision 7" block (lines 325-332) states the pins correctly (SMP1 12, bridge 9). The Status paragraph (lines 3-17) still ends "The revision 5 history is retained as history and does not review revision 6; its new-delta review is pending" — stale: the revision-6 delta review passed (WORK_PACKAGES:49 "new-delta review PASS at repair lane `934eb3f`") and it is revision 7 whose delta review is pending; the paragraph's revision list also stops at 6.
5. **Cross-references not moved by the delta.** `docs/adr/ADR-0035-thin-cli-boundary.md:3` "the S20-430 contract is a draft at revision 6" with revision-5/6 records only (no revision-7 record); `docs/WORK_PACKAGES.md:49` "(revision 6, 2026-09-09, ADR-0035, ...)". The checker's `ADR_MARKERS`/`WORK_PACKAGE_MARKERS` pin the revision-6 wording, so the stale text is actively asserted rather than caught. The bridge revision-8 precedent verified ADR/WP revision records as part of the delta; this delta left both at 6. (`docs/audits/S20_430_THIN_CLI_CLOSEOUT.md:3` still says "revision 3" — pre-existing before this delta, noted only.)
6. **Codes and counts.** `| 43000..43003 |` rows with exit 2-5 at lines 181-184; `ERROR_CODES_V1.md:492` range sentence; `new_stable_error_codes: 4`; summary `commands`, `forbidden`, `offered_hello` strings match the spec and checker expectations.
7. **Contract-cited test evidence.** §5 lines 218-219: "`scripts/test_cli_contract.py` pins the current-delta-review record gate with stale-revision, missing-record, and frozen-with-pending negatives." The negatives still pass, but the two positive controls are red at HEAD as a direct consequence of the delta (fixture pinned to 6). The contract cites, as evidence, a suite the delta broke.

## Per-item analysis

- Pin move: correct, symmetric, checker-enforced. No finding.
- Crate: unchanged; tests green. No finding.
- Cited tests: two suites red because the delta moved the revision without refreshing their positive controls; both suites run outside `make quick`, so the green `quick`/checker output does not cover them. Report-grade because the contract text names one of them as pinning evidence.
- Revision-7 note: "change nothing the CLI ... parses" overstates (item 3); §8 end-of-input rule under-describes the code. Report-grade (moderate).
- ADR-0035 / WORK_PACKAGES currency: stale at 6 (item 5). Moderate.
- Status paragraph: stale pending sentence (item 4). Editorial.

```
VERDICT: REVISE_0_P0_0_P1_1_P2_4_P3
SECTION: cli
FIELD: current_delta_review.ariadne
SCOPE_SHA: 43f2f5ba738b587a9a0bb365a55f3096527e3e3d
FINDINGS:
[P2] [evidence-currency] scripts/test_cli_contract.py:80 - the contract (SLEY_CLI_V1.md:218-219) cites this script as pinning the current-delta-review gate, but its positive controls hardcode contract_revision 6 (`passing_review()`), so 2 of 7 tests fail at HEAD with `review:current-delta-revision:6`; broken by the rev-7 pin move (9efe984) and not run by `make quick`, so the green checker does not cover it
[P3] [evidence-currency] scripts/test_current_contract_review.py:101 - CLI baseline and terminal-acceptance cases (lines 101, 179) still assert revision 6; 2 of 6 tests fail at HEAD for the same cause (bridge/SMP1/index cases in the file were refreshed by 23bd4cf, the CLI ones were not)
[P3] [overstatement] docs/spec/SLEY_CLI_V1.md:327-331 - "the bridge's revision-9 ceilings ... change nothing the CLI ... parses" is not exact: the CLI parses through `frame_from_json`, the r9 element ceiling now yields RESOURCE_LIMIT for a line with >= 1,048,576 value positions, and crates/sley-cli/src/lib.rs:1105 ends the input on every RESOURCE_LIMIT while section 8 (lines 270-271) describes only the text ceiling as ending input
[P3] [cross-reference] docs/adr/ADR-0035-thin-cli-boundary.md:3 - ADR still states "a draft at revision 6" with no revision-7 record; the checker's ADR_MARKERS pin the revision-6 record only
[P3] [cross-reference] docs/WORK_PACKAGES.md:49 - S20-430 row still reads "(revision 6, 2026-09-09, ADR-0035, ...)" after the revision-7 status move; the checker's WORK_PACKAGE_MARKERS assert that stale text
[P4] [editorial] docs/spec/SLEY_CLI_V1.md:14-16 - Status paragraph still says "does not review revision 6; its new-delta review is pending" although the rev-6 delta review passed (WORK_PACKAGES:49) and revision 7 is the pending delta; the paragraph's revision summary also stops at 6
SUMMARY: The revision-7 delta is a composition pin move (bridge 8 -> 9) and the pins are true and enforced in both directions by scripts/check_cli_contract.py against the SMP1 and bridge status lines; the crate is untouched, cargo tests (28) and the rule audit pass. It does not pass the contract lane as written: the delta moved the revision without refreshing the positive controls of the test the contract itself cites (test_cli_contract.py, 2 red) and its companion suite (test_current_contract_review.py, 2 red), the revision-7 note's "changes nothing the CLI parses" overstates because the bridge's new element ceiling ends CLI JSON input via lib.rs:1105 in a case section 8 does not describe, and ADR-0035 and WORK_PACKAGES still record revision 6. Re-pin the two test fixtures to 7, add the revision-7 ADR/WP records, and state the end-of-input rule for every RESOURCE_LIMIT; then the field can pass.
```
