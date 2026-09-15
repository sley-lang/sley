# cli — current-delta review, field `current_delta_review.nabu` (Nabu, architecture lane) — CLI contract revision 8 at scope 1023a31

## Scope verification

- `git rev-parse HEAD` = `1023a314b18834e36a7daa020fc9078807a3ac16` (matches SCOPE_SHA; branch `main`; `git status --short` empty at the start of the round). No tracked file was modified by this review; this transcript is the only file written.
- Section `cli` in `machineresearch/sley-2.0/machine-summary.json`: `status: S20_430_IMPLEMENTED_REVIEW_PENDING`, `contract_revision: 8`, `current_delta_review: {contract_revision: 8, ariadne: PENDING, nabu: PENDING, vulcan: PENDING}`, `semantic_authority: SERVER_ONLY`, `forbidden: [kernel_dependency, reverse_dependency, private_validation_rules, hand_built_failures, method_match_arms, prose_output]`, `implementation_complete: false`.
- Delta under review: revision 7 → 8, commit `1023a31` ("Qualification wave: council-lane verdicts at 43f2f5b, CLI revision 8, bridge revision 10, gate-test wiring") relative to `43f2f5b`. `git diff 43f2f5b 1023a31 --stat` lists 48 files; the CLI-relevant subset is `Makefile` (+7), `crates/sley-cli/tests/cli.rs` (+60/−1), `docs/spec/SLEY_CLI_V1.md` (+52/−?), `docs/adr/ADR-0035-thin-cli-boundary.md` (+9), `docs/WORK_PACKAGES.md` (rows 48–49), `docs/spec/SMP1.md` (composition paragraph), `scripts/check_cli_contract.py` (+7/−?), `scripts/test_cli_contract.py` (+2/−2), `scripts/test_current_contract_review.py` (+10/−10), `scripts/test_smp1_contract.py` (+28/−?). **`crates/sley-cli/src` is not in the diff** (`git diff 43f2f5b 1023a31 --stat -- crates/sley-cli/src` printed nothing): the CLI binary's code, commands, flags, and dependencies are unchanged in this delta.
- Previous round: `evidence/review/verdicts/cli/delta_review_nabu-43f2f5b.md` (revision 7, REVISE_0_P0_0_P1_1_P2_1_P3): P2 gate tests hard-coded revision 6 and nothing ran `scripts/test_*.py`; P3 ADR-0035/WORK_PACKAGES not moved to revision 7 and the checker pinned only the revision-6 records.

## Inputs read in full

- `docs/spec/SLEY_CLI_V1.md` at HEAD (Status paragraph, sections 1–9, revision 7 and revision 8 history entries) and its full diff against `43f2f5b`.
- `crates/sley-cli/tests/cli.rs` diff (the new `a_json_line_at_the_element_ceiling_is_refused_and_ends_the_input` test, lines 494–548), `crates/sley-cli/Cargo.toml` dependencies, `crates/sley-json-bridge/Cargo.toml` dependencies.
- `scripts/check_cli_contract.py` (pins at lines 31–33, `ADR_MARKERS` through line 76, `WORK_PACKAGE_MARKERS` lines 77–82, versioned-export crate markers lines 89–91, status-line pin checks lines 218–226), `scripts/check_cli_rules.py` output, `scripts/test_cli_contract.py`, `scripts/test_current_contract_review.py`, `scripts/test_smp1_contract.py` diffs.
- `Makefile` `quick` recipe (lines 3–~130; the new `scripts/test_*.py` block at lines 89–95).
- `docs/adr/ADR-0035-thin-cli-boundary.md` lines 1–22; `docs/WORK_PACKAGES.md` rows 48–49; `docs/spec/SMP1.md` composition paragraph (lines 21–23).
- `crates/sley-json-bridge/src/lib.rs` lines 33–48 (ceilings) and 431–464 (text/depth/element refusal); the element ceiling is what the new CLI test exercises.
- Master goal §14.2, §14.3, §22.6 (as in the previous round).

## Tool results (run with `SLEY2_MASTER_GOAL=/home/greyforge/machineresearch/Sley2.0mastergoal.md`)

| Command | Result |
|---|---|
| `python3 scripts/check_cli_contract.py` | `{"contract": "s20-430-thin-cli-v1", "implementation_present": ["crates/sley-cli"], "new_stable_error_codes": 4, "problems": [], "result": "PASS", "revision": 8, "status": "S20_430_IMPLEMENTED_REVIEW_PENDING"}`, exit 0 |
| `python3 scripts/check_cli_rules.py` | `result: PASS`, `dependencies: [serde_json, sley-json-bridge, sley-protocol]`, `encode_frame_calls: 1`, `frame_literals: 1`, `method_names_audited: 43`, `method_tags_audited: 43`, `problems: []`, exit 0 |
| `python3 scripts/test_cli_rules.py` | `Ran 10 tests ... OK`, exit 0 |
| `python3 scripts/test_cli_contract.py` | `Ran 7 tests ... OK`, exit 0 (was FAILED failures=2 at 43f2f5b) |
| `python3 scripts/test_current_contract_review.py` | `Ran 6 tests ... OK`, exit 0 (was FAILED at 43f2f5b) |
| `python3 scripts/check_clean_room_boundary.py` | `result: PASS`, `register_entries: 7`, `status: S20_780_REGISTER_ACCEPTED`, exit 0 |
| `python3 scripts/check_smp1_json_bridge_contract.py` | `result: PASS`, `revision: 10`, `problems: []`, exit 0 (composed authority the CLI pins) |
| `python3 scripts/check_required_contract_index.py` | `result: PASS`, `problems: []`, exit 0 |
| `cargo test -p sley-cli` | integration suite `tests/cli.rs`: `29 passed; 0 failed; 0 ignored`, including `test a_json_line_at_the_element_ceiling_is_refused_and_ends_the_input ... ok` |
| `cargo test -p sley-json-bridge` | unit suite: `11 passed; 0 failed; 1 ignored` (the ignored case is the fixture generator, unchanged) |
| `grep -rln --include=Cargo.toml -E 'sley-cli\|sley-json-bridge' crates/ fuzz/ Cargo.toml` | `crates/sley-json-bridge/Cargo.toml`, `crates/sley-cli/Cargo.toml`, `fuzz/Cargo.toml`, `Cargo.toml` (root workspace members) — no kernel crate |
| `grep -rn 'sley_cli\|sley_json_bridge' crates/ fuzz/ --include=*.rs` outside the two crates | only `fuzz/targets/smp1_json_bridge.rs` (the S20-700 fuzz slice) |
| `ls scripts/test_*.py` vs `grep -n 'scripts/test_' Makefile` | exactly seven suites on disk; exactly the same seven at `Makefile:89-95` inside `quick` |

Staging note on evidence: after the results above were captured, the session's disk quota filled (`EDQUOT`) and the Bash tool stopped returning output. I therefore did **not** observe at HEAD in this round: `scripts/test_required_contract_index.py`, `scripts/test_session_handle_profile.py`, `scripts/test_smp1_contract.py`, `scripts/test_smp1_json_bridge_table.py`, and `scripts/check_smp1_contract.py` (the reverse composition pin). The first of these was green at `43f2f5b` in my previous round and the last was PASS there; the `test_smp1_contract.py` diff derives its pins from the two status lines, which at HEAD read revision 10 / revision 8 (verified by reading), so nothing in the delta gives me a reason to expect red — but I am recording them as unobserved rather than passed.

## Independently re-derived claims

1. **The CLI is still a leaf wrapper.** `crates/sley-cli/Cargo.toml` depends on `serde_json`, `sley-json-bridge`, `sley-protocol` only (rule audit `dependencies` agrees). No `Cargo.toml` under `crates/` other than the two named crates references `sley-cli` or `sley-json-bridge`; no `.rs` outside them imports `sley_cli`; the only external `sley_json_bridge` import is the fuzz target. Master goal §14.3 holds. `crates/sley-cli/src` is byte-identical across the delta.
2. **Gate tests derive their revision.** `scripts/test_cli_contract.py:41` now reads `def passing_review(revision: int = CHECKER.SPEC_REVISION)`; `scripts/test_current_contract_review.py` uses `BRIDGE_CHECKER.SPEC_REVISION`, `CLI_CHECKER.SPEC_REVISION`, and `SMP1_CHECKER.CONTRACT_REVISION` in place of the literals 9/6/12 (lines 92, 101, 116, 158, 179). The fourth, unpinned copy of the revision that caused the revision-7 failure no longer exists; both suites are green at `SPEC_REVISION = 8`.
3. **The suites run under a gate.** `Makefile:89-95` adds all seven `scripts/test_*.py` to `quick`, immediately after `check_declared_limits.py --check`. The set on disk and the set in the recipe are identical (seven and seven), so a future suite added without wiring would be a new omission, not a regression of this fix.
4. **Ownership records are current and pinned.** ADR-0035 Status reads "draft at revision 8" and carries a revision 7 record (composition pin move to bridge revision 9) and a revision 8 record (pin move to bridge 10, section 8 end-of-input rule, derived gate-test revision, `make quick` wiring); the Date line lists both. `docs/WORK_PACKAGES.md:49` reads "(revision 8, 2026-09-14, ADR-0035; revision-6 new-delta review PASS at repair lane `934eb3f` ...; revision-7 delta round 2026-09-14 REVISE closed by revision 8; Council reviews pending)". `check_cli_contract.py` pins `"Revision 8 record (2026-09-14)"` in `ADR_MARKERS` and the revision-8 row prefix `"(revision 8, 2026-09-14, ADR-0035; revision-6 new-delta review PASS"` in `WORK_PACKAGE_MARKERS`, so the next pin move fails the checker unless both records move with it — the revision-7 mode of failure is closed.
5. **Composition pins hold in both directions (forward verified).** `check_cli_contract.py:218-226` asserts SMP1 revision 12 and bridge revision 10 against the composed documents' own status lines and against the CLI spec's pin sentences; `docs/spec/SMP1.md:21-23` reads "bridge ... revision 10 and the S20-430 CLI contract ... revision 8" (the reverse pin text is correct by reading; its checker was not observed this round, see the staging note).
6. **The section 8 change is descriptive, not a new rule.** Revision 8 rewords the end-of-input bullet from "a JSON line above the text ceiling" to "a JSON line the bridge refuses with `JSON_BRIDGE_RESOURCE_LIMIT` for any of its ceilings (text bytes, nesting depth, or value positions; bridge contract section 3)". The CLI code did not change; the behaviour was already "any `frame_from_json` refusal ends the input". The rule's owner remains the bridge (`crates/sley-json-bridge/src/lib.rs:431-464` refuses at `text.len() > MAX_JSON_TEXT_BYTES`, `depth > MAX_JSON_DEPTH`, `positions >= MAX_JSON_ELEMENTS`); the CLI contract only states what it does after a refusal. No semantics moved into the CLI.
7. **The new test introduces no private validation.** `crates/sley-cli/tests/cli.rs:494-548` is an integration test in `tests/`, not crate source; it imports `MAX_JSON_ELEMENTS` from `sley_json_bridge` (the owner) rather than restating the number, builds the over-ceiling line from that constant, and asserts the codec-owned failure record (`42_004`, `JSON_BRIDGE_RESOURCE_LIMIT`) decoded through `ProtocolFailure::decode`. The counting report assertions (`frames_read: 2`, `failed_answers: 1`, `codes["42004"]: 1`) match section 8's counting rule. `frame_literals: 1` and `encode_frame_calls: 1` in the rule audit are unchanged, confirming no new hand-built frame in the crate.
8. **History wording.** The revision 7 history entry was edited in place (its former "changes nothing the CLI ... parses" sentence is replaced by a paragraph that records the overstatement and says revision 8 corrects it). The correction is candid and the pin facts are unchanged, but the revision-7 entry as approved at `43f2f5b` no longer exists verbatim; see P4 below.

## Per-item analysis against the revision-7 asks

| Ask (43f2f5b) | Status at 1023a31 | Evidence |
|---|---|---|
| P2 derive gate-test revision from `CHECKER.SPEC_REVISION`; run `scripts/test_*.py` under a gate | **Closed.** | claims 2–3; both suites OK; `Makefile:89-95` |
| P3 add revision records to ADR-0035 and WORK_PACKAGES; pin the current record | **Closed.** | claim 4; `check_cli_contract.py` markers; checker PASS |

Architecture lane checks: one authority per judgment (server/codec/bridge own every judgment; CLI restates none) — holds. No duplicated semantics — holds; the one literal-copy the CLI keeps (its own four codes 43000–43003) is unchanged. Dependency direction — holds (claim 1). Bounded surface — no new flag, command, or shape. Ownership records — current and pinned. Staging honesty — the spec says "no behavior change" for the bridge-10 re-pin and says the revision-7 note was an overstatement; both statements are true by code reading. Gates that actually run — the previously-unrun suites are now in `quick` and green.

## Findings

- **P4 [editorial / history]** `docs/spec/SLEY_CLI_V1.md:330-339` — the "Revision 7 (2026-09-14)" history entry was rewritten in place rather than left as filed and corrected by the revision 8 entry (which already says "revision 8 corrects it"). The content is honest; the practice of mutating an earlier revision's record is the only wrinkle. Prefer appending corrections under the revision that makes them and leaving prior entries verbatim.

No P0/P1/P2/P3. No architectural drift in the delta; the CLI remains a transport endpoint with no private validation and no reverse dependency.

```
VERDICT: PASS
SECTION: cli
FIELD: current_delta_review.nabu
SCOPE_SHA: 1023a314b18834e36a7daa020fc9078807a3ac16
FINDINGS:
[P4] [editorial] docs/spec/SLEY_CLI_V1.md:330 - the revision-7 history entry was rewritten in place (candidly, recording its own overstatement) instead of being left verbatim and corrected under the revision-8 entry; keep prior revision records immutable and append corrections
SUMMARY: The revision 7 -> 8 delta (1023a31) closes both revision-7 asks: scripts/test_cli_contract.py and scripts/test_current_contract_review.py now derive the record revision from the checkers' SPEC_REVISION constants (7 and 6 tests OK at revision 8, where both were red at 43f2f5b), all seven scripts/test_*.py suites are wired into make quick (Makefile:89-95, matching the seven on disk), and ADR-0035 plus WORK_PACKAGES row 49 carry revision-7 and revision-8 records that check_cli_contract.py now pins by text. The CLI stays a leaf wrapper: crates/sley-cli/src is unchanged, its dependencies are serde_json, sley-json-bridge, and sley-protocol only, no kernel crate imports either wrapper crate, the rule audit passes with one documented failure-frame literal and 43 names/tags audited, and the new element-ceiling integration test takes MAX_JSON_ELEMENTS from the bridge and asserts the codec-owned failure record rather than restating any rule. Section 8's end-of-input wording now covers every bridge ceiling without moving any judgment into the CLI. check_cli_contract.py, check_cli_rules.py, check_clean_room_boundary.py, check_smp1_json_bridge_contract.py, check_required_contract_index.py, and cargo test -p sley-cli (29 passed) all PASS at HEAD; four other scripts/test_*.py suites and check_smp1_contract.py were not observed this round because the session disk filled after the results above were captured. One P4 on rewriting a prior history entry in place.
```
