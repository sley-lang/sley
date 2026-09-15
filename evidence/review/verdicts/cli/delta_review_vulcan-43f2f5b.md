# cli current-delta review — SLEY_CLI_V1 revision 7 (delta 9efe984) — Vulcan surface lane

Section: `cli`. Field: `current_delta_review.vulcan`. Role: Vulcan (QA/security, surface lane).

## Scope verification

- `git rev-parse HEAD` = `43f2f5ba738b587a9a0bb365a55f3096527e3e3d` (matches SCOPE_SHA; branch main).
- Summary block read directly from `machineresearch/sley-2.0/machine-summary.json`:
  `cli.contract_revision = 7`, `cli.current_delta_review = {contract_revision: 7, ariadne: PENDING, nabu: PENDING, vulcan: PENDING}`,
  status `S20_430_IMPLEMENTED_REVIEW_PENDING`.
- Previously reviewed revision: 6 (closed by `d0dea3f` "record Council repair-review PASS: close ... CLI rev-6", integrated at `411c675`). Delta under judgment: revision 6 -> 7, single commit `9efe984` (P-C finals fallout II: pin move, bridge 8 -> 9).
- No prior `evidence/review/verdicts/cli/` directory existed; created for this transcript only.

## Inputs read in full

- `git show 9efe984 --stat` and its diff for `docs/spec/SLEY_CLI_V1.md` (13 lines: Status line rev 6 -> 7, composition pin `SMP1_JSON_BRIDGE_V1.md` revision 8 -> 9 at line 26, new "Revision 7" history entry at lines 325-332) and `scripts/check_cli_contract.py` (4 lines: `SPEC_REVISION 6 -> 7`, `BRIDGE_REVISION 8 -> 9`).
- `git diff 411c675..HEAD -- crates/sley-cli crates/sley-protocol/src/server.rs docs/spec/SLEY_CLI_V1.md scripts/check_cli_contract.py scripts/check_cli_rules.py` (full): `crates/sley-cli/src/lib.rs` and `src/main.rs` are byte-identical since the rev-6 integration; `crates/sley-cli/tests/cli.rs` gained one test (`cli_failures_name_their_stable_symbols`, I/O fault injection via `FailWrite` proving `CLI_IO_FAILURE`/status 4) and split two multi-part tests into four; `server.rs` changes are the S20-300/S20-310 guard/capture changes outside the CLI surface.
- `scripts/check_cli_contract.py` lines 28-40 and 215-237 (pin assertions).
- `crates/sley-cli/src/lib.rs` lines 696-725 (`read_line`: `take(MAX_JSON_TEXT_BYTES + 2)`, bridge conversion) and 1083-1125 (serve loop: `Next::Rejected` writes the rejection and `break`s only on `JsonBridgeErrorCode::ResourceLimit`, otherwise `continue`s).
- `docs/spec/SLEY_CLI_V1.md` lines 262-275 (section 8 stream rules) and 325-332 (revision 7 entry).
- `scripts/test_current_contract_review.py` (CLI classes) and `git show 23bd4cf -- scripts/test_current_contract_review.py`.
- Bridge delta context (`git diff f1684a9..HEAD -- crates/sley-json-bridge/src/lib.rs`): `MAX_JSON_TEXT_BYTES = 4 * MAX_FRAME_BYTES` = 4 * 67,108,864 = 268,435,456, numerically identical to the revision-8 literal; new `MAX_JSON_ELEMENTS = 1_048_576` refusal returning `JSON_BRIDGE_RESOURCE_LIMIT`.
- `Makefile` `quick:` recipe (lines 37-38 run `check_cli_contract.py` and `check_cli_rules.py`; no `test_*.py` script is invoked by any Makefile target).

## Tool results (executed, exact)

- `cargo test -p sley-cli --locked` -> `tests/cli.rs`: `test result: ok. 28 passed; 0 failed; 0 ignored`; lib/main/doc: 0 tests; exit 0.
- `SLEY2_MASTER_GOAL=... python3 scripts/check_cli_contract.py` -> `{"result": "PASS", "revision": 7, "problems": [], "status": "S20_430_IMPLEMENTED_REVIEW_PENDING", "new_stable_error_codes": 4}` exit 0.
- `python3 scripts/check_cli_rules.py` -> `{"result": "PASS", "problems": [], "dependencies": ["serde_json","sley-json-bridge","sley-protocol"], "encode_frame_calls": 1, "frame_literals": 1, "method_names_audited": 43, "method_tags_audited": 43}` exit 0.
- `python3 scripts/test_current_contract_review.py` -> `..FF..` `Ran 6 tests`, `FAILED (failures=2)`:
  `CliValidBaseline.test_valid_baseline_accepted` line 101 `AssertionError: 7 != 6`; `CliTerminalAcceptance.test_complete_all_pass_accepted` line 179 -> `pass_current_review(section, 6)` asserts `review["contract_revision"] == 6`. The bridge (rev 9) and index (rev 3) classes in the same file pass.
- `grep -rn test_current_contract_review Makefile .forge` -> not in the Makefile; cited only in `.forge/slices/phase-3-v2-endpoint-offer.json:38` as an evidence command.
- `grep -n 'ResourceLimit\|RESOURCE_LIMIT\|MAX_JSON' crates/sley-cli/tests/cli.rs` -> no matches.

## Independently re-derived claims

1. **Pins assert against composed status lines.** `check_cli_contract.py:215-226` regex-reads `docs/spec/SMP1.md` `Status: ... revision (\d+)` and `docs/spec/SMP1_JSON_BRIDGE_V1.md` `Status: ... revision (\d+)`, compares each to `SMP1_REVISION = 12` / `BRIDGE_REVISION = 9`, and additionally requires the literal pin text `"SMP1 revision 12 and bridge revision 9"` and `` `docs/spec/SMP1_JSON_BRIDGE_V1.md` revision 9 `` inside the CLI spec. The bridge status line is revision 9 (verified by the bridge checker, `"revision": 9`). A stale pin in either direction fails closed. Claim in the revision-7 note holds.
2. **No CLI byte, exit-status, or report behavior changed in the CLI's own code.** `crates/sley-cli/src/lib.rs` and `src/main.rs` have zero diff since `411c675`; the 4 CLI codes and exit statuses (2/3/4/5) are unchanged in the checker; 28/28 tests pass including byte-mode ceiling, negotiation, batch, rejection-stamping, report contract v2, and the new I/O fault test. The rule audit still finds exactly one `encode_frame` call and one frame literal.
3. **Composition-only revision.** The spec diff is Status line, one pin sentence, and a history entry. No section 1-9 command, report field, code, or stream rule text moved.
4. **Composed behavior through the bridge.** The text ceiling value is numerically unchanged (268,435,456), so `read_line`'s `take(MAX_JSON_TEXT_BYTES + 2)` bound is unchanged. The bridge's new element ceiling is a new `JSON_BRIDGE_RESOURCE_LIMIT` refusal; through `lib.rs:1103-1106` a `ResourceLimit` rejection ends the input, so a fully-read JSON line holding >= 1,048,576 value positions (but under the byte ceiling) is now answered `JSON_BRIDGE_RESOURCE_LIMIT` and ends the stream, where revision 6 would have parsed it, answered a bridge shape code or a codec `PROTOCOL_*` code, and continued. This is fail-closed and bridge-owned, but it is an observable CLI stream-behavior change for that (contrived, unbounded-allocation) input class, and section 8 line 270-271 names only "a JSON line above the text ceiling" as stream-ending; depth- and element-limited lines also end input. No CLI-level test drives the JSON resource-limit stream end at all (the byte-mode `TooLarge` path is tested at `cli.rs:494`).
5. **Not a disguised behavior change.** The revision-7 text claims nothing beyond the pin move; the one composed effect in claim 4 is the bridge's declared refusal reaching the CLI through its pre-existing, spec-stated resource-limit rule, not a CLI rule change. Recorded as a precision/test-gap finding, not as a hidden behavior change.
6. **Delta left a repository test red.** `9efe984` moved the CLI spec and checker to revision 7 but did not move the CLI pins in `scripts/test_current_contract_review.py` (lines 101 and 179 still expect 6), although the earlier commit `23bd4cf` had moved that file's bridge and index pins for their revisions. The CLI terminal-acceptance path (`COMPLETE` + three PASS accepted by the checker) is therefore not currently proven by that test. The Makefile does not run the file, so `make quick` is unaffected.

## Per-item analysis

- Adversarial adequacy of the declared surface: unchanged surface, checker pins fail-closed on both composed documents; PASS.
- Fail-closed behavior: unchanged; new I/O fault test strengthens it; PASS.
- Limits and ceilings enforced: CLI byte ceiling and JSON line bound unchanged and tested (byte mode); JSON resource-limit stream end untested at CLI level (pre-existing gap widened by the element ceiling) — P3.
- Tests reaching refusal paths: 28/28 native; `test_current_contract_review.py` CLI classes broken by the delta — P3.
- No regression in frozen bytes: CLI has no corpus; bridge v1 corpus verified byte-identical in the json_bridge transcript.

## Findings

- [P3] [test-currency] `scripts/test_current_contract_review.py:101` (and `:179`) - delta `9efe984` moved `SLEY_CLI_V1.md`/`check_cli_contract.py` to revision 7 but left this file's CLI pins at 6, so it fails 2/6 (`7 != 6`); the CLI terminal-acceptance test no longer runs green. Not invoked by the Makefile; cited as evidence in `.forge/slices/phase-3-v2-endpoint-offer.json:38`.
- [P3] [contract-precision/test-gap] `docs/spec/SLEY_CLI_V1.md:327-331` (with `:270-271` and `crates/sley-cli/src/lib.rs:1103-1106`) - "changes nothing the CLI renders or parses" holds for the CLI's own code, but the bridge's revision-9 element ceiling now turns fully-read JSON lines with >= 1,048,576 value positions into a stream-ending `JSON_BRIDGE_RESOURCE_LIMIT` answer (revision 6: shape/codec code, stream continues). Section 8 names only the text ceiling as stream-ending, and no CLI test drives the JSON resource-limit stream end. Fail-closed direction; document the composed effect and add one CLI-level test.

```
VERDICT: PASS
SECTION: cli
FIELD: current_delta_review.vulcan
SCOPE_SHA: 43f2f5ba738b587a9a0bb365a55f3096527e3e3d
FINDINGS:
[P3] [test-currency] scripts/test_current_contract_review.py:101 - CLI revision pin left at 6 by delta 9efe984 (spec/checker at 7); 2 of 6 tests fail (`7 != 6`, `pass_current_review(section, 6)` at :179); not run by the Makefile
[P3] [contract-precision] docs/spec/SLEY_CLI_V1.md:327 - revision-7 "no behavior change" omits that the bridge's new element ceiling reaches the CLI as a stream-ending JSON_BRIDGE_RESOURCE_LIMIT (lib.rs:1103-1106) for fully-read lines with >= 1,048,576 value positions; section 8 :270-271 names only the text ceiling, and no CLI test drives the JSON resource-limit stream end
SUMMARY: Delta 9efe984 is a composition-only pin move (bridge 8 to 9): the CLI crate sources are byte-identical since the rev-6 integration, cargo test -p sley-cli --locked passes 28/28, check_cli_contract.py PASS at revision 7 with both composed pins asserted against the SMP1 and bridge status lines, and check_cli_rules.py PASS; the text ceiling value the CLI imports is numerically unchanged. Two P3 follow-ups: the delta left scripts/test_current_contract_review.py's CLI pins at revision 6 (2 failing tests, outside the Makefile), and the bridge's new element ceiling is an observable stream-ending refusal through the CLI that the revision-7 note and section 8 do not mention and no CLI test exercises. No report-grade P0/P1/P2 open.
```
