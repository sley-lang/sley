# cli current-delta review — SLEY_CLI_V1 revision 8 (delta 1023a31 relative to 43f2f5b) — Vulcan surface lane

Section: `cli`. Field: `current_delta_review.vulcan`. Role: Vulcan (QA/security, surface lane).

## Scope verification

- `git rev-parse HEAD` at the start of the review = `1023a314b18834e36a7daa020fc9078807a3ac16` (matched SCOPE_SHA; branch main; working tree clean).
- During the review the branch history was rewritten in this checkout (reflog `HEAD@{0}: Remove AI attribution from history (operator approved)`); HEAD now reads `e0ec341eb42e79b7ba5dc29ecabc2ea23d41f776` with the same subject, author, and content. Tree identity proven: `git rev-parse 1023a31^{tree} HEAD^{tree}` = `bf293ad53aaa2d3dc7b8d580621d4f478bc88427` for both, `git diff --quiet 1023a31 HEAD` exit 0. Every file, command, and test in this transcript ran against that single tree, so the judgment applies byte-for-byte to both commit ids. The footer names the assigned SCOPE_SHA. The rewrite is metadata-only and outside this lane's scope; it is recorded so the dispatcher can reconcile the S20-730 records.
- Summary block (`machineresearch/sley-2.0/machine-summary.json`): `cli.contract_revision = 8`, `current_delta_review = {contract_revision: 8, ariadne: PENDING, nabu: PENDING, vulcan: PENDING}`, status `S20_430_IMPLEMENTED_REVIEW_PENDING` (from the checker output below).
- Previous round: `evidence/review/verdicts/cli/delta_review_vulcan-43f2f5b.md` (revision 7, PASS with 2 P3: test-pin currency in `scripts/test_current_contract_review.py`; no CLI test driving the element-ceiling stream end and section 8 naming only the text ceiling).

## Inputs read in full

- `git diff 43f2f5b..HEAD -- crates/sley-cli`: exactly one file, `crates/sley-cli/tests/cli.rs` (+60/-1): the `MAX_JSON_ELEMENTS` import and the new test `a_json_line_at_the_element_ceiling_is_refused_and_ends_the_input` (lines 494-548). `crates/sley-cli/src/lib.rs` and `src/main.rs` have zero diff.
- `git diff 43f2f5b..HEAD -- docs/spec/SLEY_CLI_V1.md` (52 lines): Status line rev 7 -> 8 with the revision-8 sentence; composition pin bridge 9 -> 10 (line 28); section 8 stream rule rewritten for every ceiling (lines 269-276); revision-7 history entry corrected (lines 330-338); new revision-8 entry (lines 340-356).
- `git diff 43f2f5b..HEAD -- scripts/check_cli_contract.py` (`SPEC_REVISION 8`, `BRIDGE_REVISION 10`, ADR/work-package markers for the revision-8 record), `scripts/test_cli_contract.py` (`passing_review(revision=CHECKER.SPEC_REVISION)`), `scripts/test_current_contract_review.py` (all five literal revision pins replaced by `*_CHECKER.SPEC_REVISION` / `CONTRACT_REVISION`), and `Makefile` (`quick:` now runs the seven `scripts/test_*.py` suites after `check_declared_limits.py --check`).
- `crates/sley-cli/src/lib.rs` lines 696-725 (`read_line`), 1015-1035 (handshake read; hello counts `frames_read` at line 1027), 1080-1125 (serve loop: `Next::Rejected` at 1102-1108 counts the line, writes the rejection, `break`s only on `BridgeError::Bridge(JsonBridgeErrorCode::ResourceLimit)`, else `continue`s; `Next::TooLarge` at 1096-1101 `break`s after queueing the prefix).
- `crates/sley-json-bridge/src/lib.rs` lines 25-48 (ceiling constants and const assertions) and 430-476 (`check_resources`, `parse`).
- `docs/spec/SLEY_CLI_V1.md` section 8 at HEAD (lines 262-282) and the revision 7/8 history entries.
- `scripts/check_cli_contract.py` lines 210-237 (own revision, SMP1 pin, bridge pin, both asserted against the composed documents' Status lines and the pin text in the CLI spec).

## Tool results (executed, exact; `SLEY2_MASTER_GOAL` set)

- `cargo test -p sley-cli --locked` -> `tests/cli.rs`: `test a_json_line_at_the_element_ceiling_is_refused_and_ends_the_input ... ok`; `test result: ok. 29 passed; 0 failed; 0 ignored` (28 -> 29 since the last round); lib/main/doc-tests 0; exit 0.
- `python3 scripts/check_cli_contract.py` -> `{"result": "PASS", "revision": 8, "problems": [], "status": "S20_430_IMPLEMENTED_REVIEW_PENDING", "new_stable_error_codes": 4, "implementation_present": ["crates/sley-cli"]}` exit 0.
- `python3 scripts/check_cli_rules.py` -> `{"result": "PASS", "problems": [], "encode_frame_calls": 1, "frame_literals": 1, "method_names_audited": 43, "method_tags_audited": 43, "dependencies": ["serde_json", "sley-json-bridge", "sley-protocol"]}` exit 0.
- `python3 scripts/test_cli_contract.py` -> `Ran 7 tests ... OK` exit 0.
- `python3 scripts/test_current_contract_review.py` -> `Ran 6 tests ... OK` exit 0 (was `FAILED (failures=2)` at the previous round).
- `git diff 43f2f5b..HEAD -- crates/sley-cli --stat` -> `crates/sley-cli/tests/cli.rs | 60 +++-` only.
- Arithmetic re-derivation (Python): the test's wide line is `[` + 1,048,576 zeros joined by 1,048,575 commas + `]` = 2,097,153 bytes, below the text ceiling 268,435,456 and below the `read_line` bound of 268,435,458, so the line is read in full and reaches `check_resources`; positions = 1 (`[`) + 1,048,575 (`,`) = 1,048,576 = `MAX_JSON_ELEMENTS`, refused by `positions >= MAX_JSON_ELEMENTS` (`lib.rs:457-463`), so the test also pins the inclusive boundary at the CLI level.

## Independently re-derived claims

1. **The new test proves what it says.** The input is three lines: the client hello, the wide line, and `direct.frames[0]` (a well-formed request rendered by `frame_to_json`). Assertions: status 0 with empty stderr; exactly two stdout lines, both parsed back through `frame_from_json`; the second is a response with `(session, request_id, method) = (None, 0, 0)` whose body decodes as `ProtocolFailure` with `(code, symbol) = (42_004, "JSON_BRIDGE_RESOURCE_LIMIT")`; report `frames_read == 2`, `failed_answers == 1`, `codes["42004"] == 1`. Because every non-`End` outcome of `source.next` increments `frames_read` (lines 1027, 1092, 1098, 1103) and no read happens after the `break` at 1106 (the `pending` batch queue is empty in non-batch mode), `frames_read == 2` is a proof that the third line was never taken from the input, not merely never answered. The report counts agree with section 8 (`frames_read` counts rejected lines; `failed_answers`/`codes` count the endpoint's own rejection).
2. **Section 8 rule versus code.** Section 8 (lines 272-276) now says a JSON line the bridge refuses with `JSON_BRIDGE_RESOURCE_LIMIT` for any ceiling (text bytes, depth, value positions) is answered with that code and ends the input. `lib.rs:1105-1107` compares the rejection to exactly `BridgeError::Bridge(JsonBridgeErrorCode::ResourceLimit)`, which is the single code all three `check_resources` branches return (`lib.rs:431-463`), and `break`s; every other rejection (`ShapeInvalid` from bad UTF-8 or shape, `NumberInvalid`, `HexInvalid`, `MethodUnknown`, `Protocol(_)`) `continue`s. The rule and the code agree for all three ceilings.
3. **No other CLI behavior changed.** The crate diff is test-only; the CLI codes (43000-43003), exit statuses, report fields, `read_line` bound (`MAX_JSON_TEXT_BYTES + 2`, numerically unchanged at 268,435,458), and the rule audit (one `encode_frame` call, one frame literal) are unchanged; 29/29 tests including the byte-mode `TooLarge` end, negotiation failure, batch, rejection stamping, report v2, and I/O fault injection pass.
4. **Pins fail closed.** `check_cli_contract.py` asserts `BRIDGE_REVISION = 10` against the bridge Status line (`^Status: S20-420 contract draft, revision (\d+)`) and the pin text `` `docs/spec/SMP1_JSON_BRIDGE_V1.md` revision 10 `` in the CLI spec; SMP1 revision 12 likewise. The bridge checker independently reports `"revision": 10` (json_bridge transcript).
5. **Previous P3s closed.** (a) `scripts/test_current_contract_review.py` and `scripts/test_cli_contract.py` derive the revision from the checker constants, and the Makefile `quick:` target now runs every `scripts/test_*.py` suite, so a revision move can no longer leave these tests red unobserved; both suites are green. (b) The element-ceiling stream end is now driven by a CLI test and stated in section 8; the revision-7 note's overstatement is corrected in the history entry (lines 330-338).
6. **Remaining precision defect (new, editorial).** Section 8 line 275 says "the line was read in full" for every ceiling. For the text-bytes ceiling that is not literally true: `read_line` reads at most `MAX_JSON_TEXT_BYTES + 2` bytes (`lib.rs:698-703`), so a line longer than that is truncated at the bound, refused as `ResourceLimit` (length > ceiling), and its remainder is never read. The observable behavior is identical (the input ends either way, and the boundary at exactly `MAX_JSON_TEXT_BYTES` bytes is correct: a ceiling-length line plus newline reads `MAX + 1` bytes, pops the newline, and passes), so this is wording only. Depth- and element-limited lines are read in full.
7. **Not a disguised behavior change.** The revision-8 note claims "no behavior change" for the bridge re-pin; bridge revision 10 is wording/declaration only (json_bridge transcript), the bridge crate's behavior and the CLI crate's code are unchanged, and the one CLI-observable stream rule the note documents was already the code's behavior at revision 7.

## Per-item analysis

- Adversarial adequacy: unchanged surface; the composed refusal path that was untested is now exercised end-to-end with an exact code and report proof. PASS.
- Fail-closed behavior: all three bridge ceilings end the input on one code, verified in code and (element ceiling) by test; UTF-8/shape rejections continue as before. PASS.
- Limits actually enforced: `read_line` bound and bridge ceilings unchanged and enforced pre-parse; the CLI test pins the inclusive element boundary. PASS.
- Tests reaching refusal paths: 29/29 native; gate tests green and now wired into `make quick`. PASS.
- No regression in frozen bytes: the CLI has no corpus; the bridge v1 corpus is byte-identical (json_bridge transcript). PASS.
- Gates green for the stated reason: `check_cli_contract.py` PASS because both composed pins match the composed Status lines; the previously red `test_current_contract_review.py` is green because its pins are derived, not because a literal was bumped.

## Findings

- [P4] [contract-precision] `docs/spec/SLEY_CLI_V1.md:275` - "the line was read in full" holds for the depth and element ceilings but not for the text-bytes ceiling: `crates/sley-cli/src/lib.rs:698-703` bounds the read at `MAX_JSON_TEXT_BYTES + 2` bytes, so an over-ceiling line is truncated at the bound and its remainder never read. Behavior is identical (input ends); say "read up to the text ceiling" or scope the clause to the depth and element ceilings.

```
VERDICT: PASS
SECTION: cli
FIELD: current_delta_review.vulcan
SCOPE_SHA: 1023a314b18834e36a7daa020fc9078807a3ac16
FINDINGS:
[P4] [contract-precision] docs/spec/SLEY_CLI_V1.md:275 - "the line was read in full" is not literally true for the text-bytes ceiling (read_line bounds the read at MAX_JSON_TEXT_BYTES + 2, lib.rs:698-703); behavior identical, wording only
SUMMARY: Revision 8 closes both revision-7 P3s with primary evidence: the new CLI test a_json_line_at_the_element_ceiling_is_refused_and_ends_the_input drives a fully read 2,097,153-byte line holding exactly 1,048,576 value positions through serve --json and proves the refusal carries 42004 JSON_BRIDGE_RESOURCE_LIMIT, that the report counts frames_read 2 / failed_answers 1 / codes["42004"] 1, and (because every read increments frames_read and nothing reads after the break) that the third well-formed line is never taken from the input; section 8's end-of-input rule for every ceiling matches lib.rs:1102-1108, where only a ResourceLimit rejection breaks the loop; the CLI crate diff is test-only, so no byte, exit-status, or report behavior moved; cargo test 29/29, check_cli_contract.py PASS at revision 8 with both composed pins asserted against the SMP1 and bridge Status lines, check_cli_rules.py PASS, and the gate tests (7/7, 6/6) are green because their revision pins now derive from the checker and are wired into make quick. HEAD was rewritten mid-review to e0ec341 with an identical tree (bf293ad5), which does not affect the judgment. One P4 wording note remains; no report-grade finding open.
```
