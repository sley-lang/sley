<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: nabu; field: nabu_architecture_review_revision_10; dispatched: 2026-09-23T13:38:32Z; duration_s: 466; process_exit_code: 0 -->
# Nabu Council review — cli

Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

What I verified myself:
- **Scope.** `git rev-parse HEAD` returned `26d050e629b669ef4acbd54826ef4008c05a061d`, which matches the scope. The branch is `work/succ-context-impl` and the worktree is clean, both before and after the review.
- **Git commands.**
  - `git log --oneline 2b0f1c9f..HEAD`: 12 commits.
  - `git diff --stat 2b0f1c9f..HEAD`
  - `git diff 2b0f1c9f..HEAD` on `docs/spec/SLEY_CLI_V1.md`, `crates/sley-cli/src/lib.rs`, `crates/sley-test-runner/src/worker.rs`, `crates/sley-cli/tests/cli.rs`, `scripts/check_cli_rules.py`, `scripts/test_cli_rules.py`, `scripts/check_cli_contract.py` and `docs/adr/ADR-0035-thin-cli-boundary.md`.
  - `git show HEAD:docs/adr/ADR-0035-thin-cli-boundary.md`
  - `git show ea286692` (the error-symbol registration count went from 534 to 535).
  - `git log -- docs/spec/NATIVE_TEST_RESERVATIONS_V1.md` and `git log -- crates/sley-test-runner/src/unit.rs`
- **Checkers.** Exit codes were confirmed through a `subprocess` wrapper.
  - `python3 scripts/check_cli_contract.py`: exit 0, `"result": "PASS"`, `"revision": 10`, `"status": "S20_430_IMPLEMENTED_REVIEW_PENDING"`, `"problems": []`.
  - `python3 scripts/check_cli_rules.py`: exit 0, `"result": "PASS"`. Output: `dependencies` [serde_json, sley-json-bridge, sley-protocol, sley-test-runner], `worker_calls` 1, `encode_frame_calls` 1, `frame_literals` 1, `method_tags_audited` 46, `method_names_audited` 46.
  - `python3 scripts/test_cli_rules.py`: exit 0, 16 tests OK. That is 10 earlier tests plus 6 `WorkerExceptionCases`: 1 control and 5 mutations.
  - `python3 scripts/test_cli_contract.py`: exit 0, 10 tests OK. The suite is unchanged this round.
  - **In-memory mutation probe of `check_cli_contract.revision_record_problems`** (no file writes):
    - baseline: `[]`
    - ADR status set back to "revision 9": `['adr-current-revision']`
    - `### Revision 10 (` removed: `['spec-revision-record']`
    - profile sentence reverted to v2-only: flat-marker failure
    - worker status-7 row removed: flat-marker failure
- **Timed-out check.** `python3 scripts/check_error_symbol_registration.py --check` exceeded the 120 s foreground limit. The harness moved it to the background and I stopped it immediately (TaskStop). I use no result from it. `git status` stayed clean. I read the rule it applies at `scripts/check_error_symbol_registration.py:82-99` instead.
- **Cargo tests.**
  - `cargo test -p sley-cli --test cli --offline`: 43 passed, 0 failed. This includes `native_test_worker_entry_runs_the_unit_argv_against_the_real_binary` and all 13 `v3_*` tests.
  - `cargo test -p sley-test-runner --offline --lib worker`: 6 passed, 0 failed. This includes `exit_statuses_are_disjoint_from_the_cli_statuses` and `input_path_entry_reads_the_binding_and_refuses_an_absent_one`.
- **Files read.**
  - `docs/spec/SLEY_CLI_V1.md`: the full diff, plus 205-300 and 440-536 in the HEAD text.
  - `crates/sley-cli/src/lib.rs`: 35-48, 101-108, 266-345 and 380-480.
  - `crates/sley-cli/src/main.rs` (whole) and `crates/sley-cli/Cargo.toml` (whole).
  - `crates/sley-cli/tests/cli.rs`: 1323-1330, 1931-1967 and 2420-2508.
  - `crates/sley-test-runner/src/worker.rs`: 1-35, 315-375 and 440-487.
  - `crates/sley-test-runner/src/unit.rs`: 60-120.
  - `crates/sley-protocol/src/server.rs` 641-660 and `crates/sley-protocol/src/lib.rs` 733-768.
  - `docs/spec/SMP1.md`: 1-80 and 228-252.
  - `docs/spec/SMP1_JSON_BRIDGE_V1.md`: 1-12, plus a grep for section 11 and `native_tests`.
  - `docs/spec/NATIVE_TEST_RESERVATIONS_V1.md`: 1-12 and 120-170.
  - `docs/spec/ERROR_CODES_V1.md`: 58-70.
  - `docs/adr/ADR-0035-thin-cli-boundary.md` (whole).
  - The `cli` section of `machineresearch/sley-2.0/machine-summary.json`.
  - `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md`: 844-880 and 893-900.
  - My round-7 transcript, `evidence/review/verdicts/cli/nabu_architecture_review_revision_9-2b0f1c9.md`.

## Evidence checked

**Closure status of my round-7 findings** (from `nabu_architecture_review_revision_9-2b0f1c9.md`):

- **CLOSED: [P2] [undeclared-surface] v3-capable profile not in the contract.**
  - The v3 surface now appears everywhere the finding named:
    - Section 1 lists `v2-capable|v3-capable` on all six commands and `--expected-version 1|2|3`, with 3 under `v2-capable` refused with cause `--expected-version` (SLEY_CLI_V1.md:45-75).
    - Section 2 states the `Server::offered_hello_v3` offer and that 3 is never forced, and puts stamping at "(1, 2, or 3)".
    - Section 3 defines `sley2-cli-report-v3` with `1 | 2 | 3 | null`.
    - Section 9 declares the v3 profile and defers its JSON renderings to bridge revision 12 section 11, which exists (SMP1_JSON_BRIDGE_V1.md:329).
    - Section 6 lists v3 evidence.
  - The code matches:
    - `lib.rs:38,42,48,520` and `parse_frame_flags` (`lib.rs:309-318`).
    - `offered_hello_v3` offers V3_ALL minus reserved non-native rows, so only 305 and 503 stay unoffered (`server.rs:648-652`, `lib.rs:733-768`). That matches "still-reserved rows unoffered".
    - The metadata string is asserted verbatim at `cli.rs:1323-1330`.
  - SMP1 revision 15 names the version 3 owner (NATIVE_TEST_ADMISSION appendices C and D, `SMP1.md:233-238`) and composes CLI revision 10 (`SMP1.md:21-22`). The CLI text attributes rather than redefines.
  - `FLAT_SPEC_MARKERS` (`check_cli_contract.py:64-83`) anchor the surface, and my probe shows the anchors fail closed.
  - The machine summary's `offered_hello` names all three offers and is checker-asserted.
- **CLOSED: [P2] [dependency-direction] runner edge and worker command undeclared.**
  - Section 5 admits `sley-test-runner` "for the section 10 worker entry only", with the transitive statement naming sley-vm, sley-scb1, sley-tests and sley-id.
  - Section 1 notes the private entry is not a user command, and section 10 bounds it.
  - ADR-0035 decision 8 records the exception and the alternative of a separate binary.
  - The summary adds `bounded_exceptions`, and `commands` stays the six user commands.
  - The audit now fails closed on any `sley_test_runner` mention other than the one `run_input_path(` call, on 0 or 2+ calls, and on 0 or 2+ `"__native-test-worker"` words (`check_cli_rules.py:142-158`). Six unit tests pin this.
  - Dependency direction is preserved: no crate depends on `sley-cli`.
  - The argv word is written in two places (`unit.rs:109`, `lib.rs:409`), but the real-binary test binds them: it renders the unit and runs its argv tail through `CARGO_BIN_EXE_sley` (`cli.rs:2465-2476`).
- **CLOSED: [P3] [record-currency] ADR-0035 stuck at revision 8.**
  - ADR-0035's status says revision 10 and it carries the revision 9 and revision 10 records (ADR-0035:3, 21-28).
  - `ADR_MARKERS` plus the new `adr-current-revision` and `### Revision N (` gates catch staleness, as my mutation probe confirmed.
- **CLOSED: [P4] [history] withdrawn in-place SMP1 revision 13 pin unrecorded.**
  - The status paragraph (SLEY_CLI_V1.md:17-20) and the section 8 revision 9 record (:422-425) both name the withdrawn in-place revision 13 pin and the ruling.

**New in this delta:**
- **The argv and status repair holds in code.**
  - `parse` accepts exactly one absolute operand (`lib.rs:409-413`); any other shape falls to `usage` and exits 2.
  - `run_input_path` opens the binding and refuses with tag 3 / status 7 when it is absent (`worker.rs:322-327`).
  - The CLI worker statuses no longer collide with the CLI's own statuses 2 to 5.
  - The gate record's §13.2 claims match the code and tests.

**Residual defects are listed below.** None concerns the v3 surface; all five concern the new section 10 exception.

## Findings

[P3] [ownership] docs/spec/SLEY_CLI_V1.md:506-536 - Section 10 is the only specification of the native worker's refusal protocol: tags 1, 2 and 3, the detail codes, and exit statuses 1/6/7/8. The code for that protocol lives in `sley-test-runner` (worker.rs:28-35, 359-375), and the `NATIVE_*` namespace belongs to the NATIVE_TEST_EXECUTION, NATIVE_TEST_ADMISSION and NATIVE_TEST_RESERVATIONS contracts (ERROR_CODES_V1.md:60-64). The new symbol `NATIVE_WORKER_INPUT_UNREADABLE` appears in docs/spec only at SLEY_CLI_V1.md:528. Its siblings are listed in the owner's "deliberately numberless" ledger (NATIVE_TEST_RESERVATIONS_V1.md:131-147), but the new symbol is not. The registration record at ea286692 ("none unregistered") therefore passes only because the textual rule counts any backticked mention anywhere in docs/spec (check_error_symbol_registration.py:82-99). That rule's own caveat is that a prose mention can launder an unassigned symbol. A contract that "owns no semantics" has become the registering authority for another package's symbol and exit protocol. - closure: add `NATIVE_WORKER_INPUT_UNREADABLE` to the native owner's numberless list, and either state the worker refusal/status table in the native owner or name it as owner, with section 10 citing that owner instead of being the sole definition.
[P3] [evidence-binding] crates/sley-test-runner/src/worker.rs:28-35,455-465; crates/sley-cli/tests/cli.rs:2482-2493 - The contract's numeric statuses are not bound to the code: the section 10 table (1, 6, 7, 8) and section 6 ("status 6", "status 1", "status 7", "status 2"). The real-binary test compares against the constants `EXIT_NOT_WIRED`, `EXIT_MALFORMED` and `EXIT_INPUT_UNREADABLE`, never the literals. The worker test only asserts that each constant is outside {0,2,3,4,5}, not its value and not that the four are pairwise distinct. `check_cli_contract.py` anchors only the spec text. Changing `EXIT_NOT_WIRED` to 9, or setting it equal to `EXIT_INPUT_UNREADABLE`, would keep every gate green while the contract says 6 and 7. - closure: a test asserting the literal values (1, 6, 7, 8) and pairwise distinctness, or literal status assertions in the real-binary test.
[P4] [contract-precision] docs/spec/SLEY_CLI_V1.md:224-227 - Section 4 says the worker entry's statuses are (1, 6, 7, 8) and that "it writes nothing to standard error". But section 10 (:532-533) and lib.rs:456-462 make a post-run flush failure `CLI_IO_FAILURE`, with status 4 and the JSON object on stderr. Also, the newline-free refusal words of about 40 bytes go into the binary's line-buffered locked stdout (main.rs:7-15), so an unwritable stdout surfaces at the flush as status 4. Status 8 is therefore effectively unreachable through `sley` itself. The machine summary's `bounded_exceptions` repeats "worker statuses 1/6/7/8". - closure: section 4 names the status 4 flush path and its stderr object, and section 10 marks status 8 as a library-level status (or the entry writes unbuffered).
[P4] [doc-drift] crates/sley-test-runner/src/worker.rs:4-10 - The module doc, rewritten this round, still says the worker "writes exactly one length-delimited [`WorkerReply`] frame on stdout". That contradicts `run_stdio`'s own doc (:331-334), CLI section 10 (:518-522, raw refusal words) and the success branch (:354), which returns 0 and writes nothing. No `WorkerReply` is ever written today. - closure: mark the `WorkerReply` output as the N5 target and state the current raw-refusal output.
[P4] [fail-closed] crates/sley-test-runner/src/worker.rs:335-356 - `run_stdio` reads the 12-byte header and the declared body and never checks for end of input. A binding holding a valid `SLEYWRK1` envelope followed by trailing bytes is therefore answered exactly like a clean one (status 6) instead of being refused as malformed. Section 10 says the worker "reads exactly one length-delimited `SLEYWRK1` request envelope", and the input is now a whole file rather than a stream. - closure: refuse trailing bytes as malformed with a test, or state in section 10 that trailing bytes are ignored.

## Assessment

Revision 10 closes all four of my round-7 findings, and I checked each closure against code, tests and checker behavior, not against the gate record.

**The version 3 surface.** The v3-capable profile is now declared exactly as implemented. It attributes the version 3 table to its owner (NATIVE_TEST_ADMISSION appendices C and D, via SMP1 revision 15) and its JSON renderings to bridge revision 12, and it is anchored by fail-closed flat markers.

**The runner edge.** The `sley-test-runner` edge and the private worker entry are admitted as a named, audited exception. ADR-0035 decision 8 records it. The rule audit bounds it to one call and one command word, with mutation tests. The single argv contract is proven by rendering the supervisor's unit and running its tail against the real binary. The status repair removes the old 2 and 3 collisions with `CLI_USAGE_INVALID` and `CLI_INPUT_INVALID`. Dependency direction is intact.

**What remains** is boundary hygiene around the new exception:
- The CLI contract is the only document that defines, and through the textual registration rule the only one that registers, a `NATIVE_*` worker symbol and the worker's status protocol, which belong to the native-test owner (P3).
- The contract's numeric statuses are pinned by no test or checker (P3).
- Three precision notes (P4): section 4 omits the status 4 flush path; the worker module doc is stale; trailing input bytes are ignored.

None of these makes the CLI's own surface false or opens a semantic path through the CLI. My lane accepts the package at contract revision 10 with these carried.

VERDICT: PASS_0_P0_0_P1_0_P2_2_P3_3_P4_PRIOR_P3_P4_CLOSED
SECTION: cli
FIELD: nabu_architecture_review_revision_10
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS:
[P3] [ownership] docs/spec/SLEY_CLI_V1.md:506-536 - Section 10 is the only specification of the native worker's refusal protocol: tags 1, 2 and 3, the detail codes, and exit statuses 1/6/7/8. The code for that protocol lives in `sley-test-runner` (worker.rs:28-35, 359-375), and the `NATIVE_*` namespace belongs to the NATIVE_TEST_EXECUTION, NATIVE_TEST_ADMISSION and NATIVE_TEST_RESERVATIONS contracts (ERROR_CODES_V1.md:60-64). The new symbol `NATIVE_WORKER_INPUT_UNREADABLE` appears in docs/spec only at SLEY_CLI_V1.md:528. Its siblings are listed in the owner's "deliberately numberless" ledger (NATIVE_TEST_RESERVATIONS_V1.md:131-147), but the new symbol is not. The registration record at ea286692 ("none unregistered") therefore passes only because the textual rule counts any backticked mention anywhere in docs/spec (check_error_symbol_registration.py:82-99). That rule's own caveat is that a prose mention can launder an unassigned symbol. A contract that "owns no semantics" has become the registering authority for another package's symbol and exit protocol. - closure: add `NATIVE_WORKER_INPUT_UNREADABLE` to the native owner's numberless list, and either state the worker refusal/status table in the native owner or name it as owner, with section 10 citing that owner instead of being the sole definition.
[P3] [evidence-binding] crates/sley-test-runner/src/worker.rs:28-35,455-465; crates/sley-cli/tests/cli.rs:2482-2493 - The contract's numeric statuses are not bound to the code: the section 10 table (1, 6, 7, 8) and section 6 ("status 6", "status 1", "status 7", "status 2"). The real-binary test compares against the constants `EXIT_NOT_WIRED`, `EXIT_MALFORMED` and `EXIT_INPUT_UNREADABLE`, never the literals. The worker test only asserts that each constant is outside {0,2,3,4,5}, not its value and not that the four are pairwise distinct. `check_cli_contract.py` anchors only the spec text. Changing `EXIT_NOT_WIRED` to 9, or setting it equal to `EXIT_INPUT_UNREADABLE`, would keep every gate green while the contract says 6 and 7. - closure: a test asserting the literal values (1, 6, 7, 8) and pairwise distinctness, or literal status assertions in the real-binary test.
[P4] [contract-precision] docs/spec/SLEY_CLI_V1.md:224-227 - Section 4 says the worker entry's statuses are (1, 6, 7, 8) and that "it writes nothing to standard error". But section 10 (:532-533) and lib.rs:456-462 make a post-run flush failure `CLI_IO_FAILURE`, with status 4 and the JSON object on stderr. Also, the newline-free refusal words of about 40 bytes go into the binary's line-buffered locked stdout (main.rs:7-15), so an unwritable stdout surfaces at the flush as status 4. Status 8 is therefore effectively unreachable through `sley` itself. The machine summary's `bounded_exceptions` repeats "worker statuses 1/6/7/8". - closure: section 4 names the status 4 flush path and its stderr object, and section 10 marks status 8 as a library-level status (or the entry writes unbuffered).
[P4] [doc-drift] crates/sley-test-runner/src/worker.rs:4-10 - The module doc, rewritten this round, still says the worker "writes exactly one length-delimited [`WorkerReply`] frame on stdout". That contradicts `run_stdio`'s own doc (:331-334), CLI section 10 (:518-522, raw refusal words) and the success branch (:354), which returns 0 and writes nothing. No `WorkerReply` is ever written today. - closure: mark the `WorkerReply` output as the N5 target and state the current raw-refusal output.
[P4] [fail-closed] crates/sley-test-runner/src/worker.rs:335-356 - `run_stdio` reads the 12-byte header and the declared body and never checks for end of input. A binding holding a valid `SLEYWRK1` envelope followed by trailing bytes is therefore answered exactly like a clean one (status 6) instead of being refused as malformed. Section 10 says the worker "reads exactly one length-delimited `SLEYWRK1` request envelope", and the input is now a whole file rather than a stream. - closure: refuse trailing bytes as malformed with a test, or state in section 10 that trailing bytes are ignored.
SUMMARY: Revision 10 closes all four round-7 Nabu findings (P2 undeclared v3 surface, P2 undeclared runner edge and worker entry, P3 ADR-0035 currency, P4 withdrawn revision 13 pin), verified against code, the real-binary argv test, checker mutation probes, and passing gates: check_cli_contract.py and check_cli_rules.py exit 0 PASS, 16+10 gate tests OK, 43 sley-cli and 6 worker cargo tests pass. The v3-capable profile is declared exactly as shipped with correct owner attribution, and the worker exception is named, ADR-recorded and mechanically bounded to one call and one command word, with dependency direction intact. What remains concerns the new exception only: the CLI contract is the sole definer and textual registrar of a NATIVE_* worker symbol and status protocol owned by the native-test package (P3), the contract's numeric worker statuses are unpinned by any test or checker (P3), and three precision notes (P4). My lane accepts the package at contract revision 10 with these carried.
