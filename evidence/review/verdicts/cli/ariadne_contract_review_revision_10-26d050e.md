<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: ariadne; field: ariadne_contract_review_revision_10; dispatched: 2026-09-23T13:38:32Z; duration_s: 409; process_exit_code: 0 -->
# Ariadne Council review — cli

Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

This was a read-only review. I did not edit, create, delete, format, or commit anything in the tree. Two runtime probes needed a scratch directory. Each time I created a `tempfile.mkdtemp` directory under `~/Work/checkpoints/`, outside the worktree, and removed it in the same step.

**Commands run, with exit codes**
- `git rev-parse HEAD` printed `26d050e629b669ef4acbd54826ef4008c05a061d`, which matches the scope SHA.
- `git log --oneline 2b0f1c9f..HEAD` lists 12 commits, 531805ae through 26d050e6. The repair commits are `004cf5be` (code) and `60be11c8` (contracts).
- `git diff --stat 2b0f1c9f..HEAD` over this package's paths: 10 files, +459/−81. The files are:
  - `SLEY_CLI_V1.md`, `ADR-0035`
  - `sley-cli/src/lib.rs`, `sley-cli/tests/cli.rs`
  - `sley-test-runner/src/worker.rs`
  - `check_cli_contract.py`, `check_cli_rules.py`, `test_cli_rules.py`
  - `WORK_PACKAGES.md`, `Makefile`
- `git diff 2b0f1c9f..HEAD` was read in full for each of those files.
- The `cli` section of `machineresearch/sley-2.0/machine-summary.json` was diffed field by field against 2b0f1c9f.
- `python3 scripts/check_cli_contract.py`: exit 0.
  - `"result": "PASS"`, `"revision": 10`, `"status": "S20_430_IMPLEMENTED_REVIEW_PENDING"`, `"problems": []`.
- `python3 scripts/check_cli_rules.py`: exit 0, PASS.
  - `method_tags_audited` 46 and `method_names_audited` 46.
  - `worker_calls` 1, `encode_frame_calls` 1, `frame_literals` 1.
  - Dependencies: `serde_json`, `sley-json-bridge`, `sley-protocol`, `sley-test-runner`.
- `python3 -m unittest scripts.test_cli_contract -v`: exit 0, 10 tests OK.
- `python3 -m unittest scripts.test_cli_rules -v`: exit 0, 16 tests OK. Six of them are `WorkerExceptionCases`: 5 mutations plus the real-source control.
- `cargo test --offline -p sley-cli --test cli`: `test result: ok. 43 passed; 0 failed`. This includes `native_test_worker_entry_runs_the_unit_argv_against_the_real_binary` and all 13 `v3_*` tests.
- `cargo test --offline -p sley-test-runner --lib worker`: 6 passed. This includes `exit_statuses_are_disjoint_from_the_cli_statuses` and `input_path_entry_reads_the_binding_and_refuses_an_absent_one`.
- `cargo test --offline -p sley-cli --lib`: 0 tests, ok.
- `cargo tree --offline -e normal` for sley-cli, sley-protocol, sley-json-bridge and sley-test-runner, compared inside a python3 heredoc (all exit 0).
- An in-memory mutation probe of `check_cli_contract.revision_record_problems` (python3 heredoc importing the checker; no file written).
- Real-binary probes of the built `sley` through python3 `subprocess`:
  - `frame decode` with `--protocol-profile v2-capable --expected-version 3`, in both flag orders;
  - `methods --protocol-profile v4-capable`;
  - `__native-test-worker` with an absent path, a directory, and a closed stdout pipe;
  - `serve --protocol-profile v3-capable` fed the version 2 hello.
- The sha256 of the round-7 transcript was checked against `evidence/review/rounds/context-r7-2b0f1c9.json`, and `git log 531805ae..HEAD -- evidence/review/verdicts/cli/` is empty.

**Files read**
- `docs/spec/SLEY_CLI_V1.md` 1-537 (whole file)
- `docs/adr/ADR-0035-thin-cli-boundary.md` 1-85 (whole file)
- `crates/sley-cli/src/lib.rs` 175-583 and 855-1174
- `crates/sley-cli/src/main.rs` and `crates/sley-cli/Cargo.toml` (whole files)
- `crates/sley-cli/tests/cli.rs` 786-792, 1333-1451, 1789-1978 and 2418-2508, plus a per-test profile and hello census
- `crates/sley-test-runner/src/worker.rs` 1-22 and 290-375
- `crates/sley-test-runner/src/unit.rs` 64-113
- `crates/sley-test-runner/Cargo.toml` `[dependencies]`
- `crates/sley-protocol/src/server.rs` 641-670
- `crates/sley-protocol/src/lib.rs` 568-614 and 733-768
- `scripts/check_cli_rules.py` 10-50 plus its diff
- `docs/spec/NATIVE_TEST_ADMISSION_V1.md` 1-12 and 552-586
- `docs/spec/SMP1.md` 23, 62-65 and 743-746
- `docs/spec/SMP1_JSON_BRIDGE_V1.md` 3 and 329-356 (grep)
- `docs/WORK_PACKAGES.md` :49
- `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md` 784-963
- `evidence/review/verdicts/cli/ariadne_contract_review_revision_9-2b0f1c9.md` (whole file)

## Evidence checked

**Round-7 finding 1 (P2, undeclared v3-capable surface).** The contract now states what the code ships.
- **Profile values.** §1:45-61 lists `v2-capable|v3-capable` on every command and gives the `sley2-cli-v3` version object.
  - It matches `lib.rs:198-203` and `:564-575`.
  - `cli.rs:1323-1328` asserts the object byte-exactly.
- **Expected versions.** §1:67-73 replaces the old "exactly `v2-capable`" sentence. `v2-capable` admits `--expected-version 1|2`, `v3-capable` admits `1|2|3`, and 3 under `v2-capable` is `CLI_USAGE_INVALID` with cause `--expected-version`.
  - The code is `lib.rs:309-319`.
  - The binary probe, in both flag orders, gave exit 2 with `{"cause":"--expected-version","code":43000,...}`.
  - `v4-capable` gives cause `v4-capable`.
- **Stamping.** §2:126 now reads "(1, 2, or 3)".
- **Serve.** §2:142-150 matches the serve path (`lib.rs:516-521`, `:1129-1162`): `Server::offered_hello_v3`, `negotiate_versioned` and `Server::new_versioned`.
  - It also matches the offer itself (`server.rs:641-660`): `[1,2,3]`, `V3_ALL` filtered by `!is_reserved() || is_native_test()`, and features `CANCEL | STREAM | NATIVE_TESTS_V1`.
  - It agrees with NATIVE appendix D (`NATIVE_TEST_ADMISSION_V1.md:554-566`): 46 rows, 305 and 503 still reserved.
- **Report.** §3:207-209 defines `sley2-cli-report-v3` with selection `1 | 2 | 3 | null`. This matches `lib.rs:862-864`, `:874-878` and `:909-922`.
- **Section 9.** §9:494-504 states the profile and delegates the version 3 JSON renderings to bridge revision 12 section 11. That section exists (`SMP1_JSON_BRIDGE_V1.md:329-356`).
- **Checker.** `check_cli_contract.py` anchors 16 flattened markers. My mutation probe shows that dropping `v3-capable` from the §1 sentence fails with `spec-flat-marker:...`.

**Round-7 finding 2 (P2, worker entry and the `sley-test-runner` edge).** Closure option (a) was taken.
- **Framing.** §0:29-31 qualifies "nothing else" with the single exception. §1:74-75 excludes the entry from the command list.
- **Section 10.** §10:506-536 states the purpose, the argv, the stdin/stdout form and the status table.
- **Argv.** It matches `render_transient_unit` (`unit.rs:80-81` requires an absolute input path; `:108-110` pushes `worker_path`, `__native-test-worker`, `worker_input_path`) and the parser at `lib.rs:409-413`.
- **Statuses.** `worker.rs:28-35` and `:359-375` give 1/6/7/8, which are disjoint from §4's 0 and 2-5.
- **Dependency rule.** §5:233-241 and `check_cli_rules.py` agree:
  - an allowlist entry for the runner;
  - `runner_mentions == worker_calls` (`:142-149`);
  - exactly one worker call and one command word (`:152-158`).
- **Mutation tests.** Five mutations are pinned in `test_cli_rules.py`: another runner path, an import, a second call, a second command word, and a removed call.
- **Real-binary test.** `cli.rs:2421-2508` renders the unit and runs the argv after the worker path against `CARGO_BIN_EXE_sley`. It covers:
  - status 6 with tag 2, the detail bytes and an empty stderr;
  - status 1 with tag 1;
  - status 7 with tag 3;
  - three bad shapes (no operand, a relative path, an extra word) giving status 2 with code 43000.
- **Link footprint.** `cargo tree` shows that the runner edge adds only `sley-test-runner` itself to the binary. `sley-protocol` already links `sley-vm`, `sley-tests` and the other kernel crates. The §5 and §10 parentheticals are therefore true but not exhaustive, and I raise no finding on them.
- **ADR.** ADR-0035 decision 8 (`:68-77`) records why the worker entry lives in the CLI and says moving it to its own binary would remove the exception.

**Round-7 finding 3 (P3, ADR-0035 re-pin).**
- ADR-0035 `:3` names revision 10. `:21-28` carries dated revision 9 and 10 records, and `:30` is the date line.
- The checker anchors both records, decision 8, and the ADR current line (`adr-current-revision`, `check_cli_contract.py:213-216`). My probe with the ADR set back to revision 9 fails with `adr-current-revision`.
- §8 now carries `### Revision 9 (` and `### Revision 10 (` records (`:412-442`). Removing the revision 10 record fails with `spec-revision-record`.

**Round-7 finding 4 (P4, qualifier and wrapping).**
- §8:414-417 now reads "whose method table includes version 2's row 201", matching `SMP1.md:63-64` and `:744-745`.
- It also states the legacy non-empty-201 failure (`:418-422`).
- The only lines over 100 characters are inside the §1 code block (`:45`, `:57`, `:59`, `:61`).

**Records.**
- **Machine summary.** The `cli` diff against 2b0f1c9f shows:
  - `contract_revision` 10 and `current_delta_review` `{10, PENDING×3}`;
  - status unchanged at `S20_430_IMPLEMENTED_REVIEW_PENDING`;
  - new `_revision_9` REVISE fields for all three lanes, with dated notes. My field matches my transcript: `REVISE_0_P0_0_P1_2_P2_1_P3_1_P4`;
  - the older PASS fields kept byte-exact, with notes saying they predate revisions 9 and 10;
  - `offered_hello` naming all three offers, and a new `bounded_exceptions` entry.
- **Other records.**
  - `SMP1.md:23` names CLI revision 10.
  - `WORK_PACKAGES.md:49` carries the revision 10 marker.
  - The round-7 transcript's sha256, `79f75709…2589`, matches the round index, and no later commit touches it.

## Findings

[P4] [precision] docs/spec/SLEY_CLI_V1.md:225-227,521,529,532-533; crates/sley-cli/src/lib.rs:456-463; crates/sley-cli/src/main.rs:8-14 - §4 says the worker entry "writes nothing to standard error". §10 repeats "nothing on standard error" and lists status 8 for "the refusal words cannot be written". In the shipped binary, stdout is the line-buffered `StdoutLock`, so the newline-free refusal words always land in the buffer and `write_all` cannot fail. A broken output channel surfaces only at the flush at lib.rs:458-461, which exits 4 and writes a `CLI_IO_FAILURE` JSON object to stderr. Probe with a closed stdout pipe: exit 4, stderr `{"cause":"STREAM","code":43002,"symbol":"CLI_IO_FAILURE"}`. §10:532-533 does admit status 4, but the stderr sentences in §4 and §10 are false for that path, and status 8 is reachable only through the library call - closure: qualify both stderr sentences to except the section 4 CLI_IO_FAILURE object on a flush failure, and either scope status 8 to the library entry or make the binary produce it; add a test that drives the entry into a closed pipe.

[P4] [evidence-gap] docs/spec/SLEY_CLI_V1.md:148-150,296-300; crates/sley-cli/tests/cli.rs:1333-1451 - The new §6 evidence item requires "a version-aware serve reporting the actual selection (3, 2, or 1) in `sley2-cli-report-v3`", and §2:148-150 claims a version 2 client hello selects 2 under `v3-capable`. The committed v3-capable serve tests assert selection 3 (cli.rs:1364 and five native-row tests) and selection 1 (cli.rs:1438). None feeds a version 2 hello (`offered_v2()`) to `serve --protocol-profile v3-capable`. My binary probe shows the behavior is correct (`selected_protocol_version` 2, `sley2-cli-report-v3`, exit 0), but the evidence the contract lists is not in the suite - closure: a cli.rs test serving a version 2 client hello under `v3-capable` that asserts selection 2 in the report and frames stamped at 2 (ideally also that a v3-only tag such as 605 is refused at that selection).

[P4] [checker-coverage] scripts/check_cli_contract.py:201-217; scripts/test_cli_contract.py (unchanged in 2b0f1c9f..HEAD) - The new anchors in `revision_record_problems` have no committed negative tests: the 16 flattened markers, `spec-revision-record` and `adr-current-revision`. The sibling bridge, session and trial-runner anchors from the same commit each got revert tests (gate record §13.3). My in-memory mutations confirm the anchors fire, but nothing pins them. The §10 status 1 and status 8 rows and the stdout/stderr sentence are not anchored at all: removing them passes the checker - closure: revert cases in test_cli_contract.py for a stale ADR current line, a missing `### Revision 10 (` record and a removed flat marker, plus anchors for the remaining §10 rows.

## Assessment

Each round-7 finding in this lane, with its status:
- Round-7 [P2] undeclared v3-capable surface (profile, `[1,2,3]` offer, `sley2-cli-v3`/`sley2-cli-report-v3`, expected version 3, stamping at 3): CLOSED.
- Round-7 [P2] `__native-test-worker` entry and `sley-test-runner` edge outside §1/§5 with a widened audit: CLOSED.
- Round-7 [P3] ADR-0035 stuck at revision 8 with no revision-9 record or checker marker: CLOSED.
- Round-7 [P4] §18 paraphrase dropped SMP1's row-201 qualifier; 249-character unwrapped line: CLOSED.

Revision 10 does what gate record §13.2 says.
- **v3 surface.** The contract now states the v3-capable surface exactly as `lib.rs` and `Server::offered_hello_v3` ship it, and it agrees with NATIVE appendix D and bridge section 11.
- **Worker entry.** It is an explicit, bounded exception. The argv is shared with `render_transient_unit` and tested against the real binary. The exit statuses 1/6/7/8 are disjoint from the CLI's 2-5. The rule audit bounds the runner edge to one call and one command word, and five mutation tests pin that bound.
- **Records.** ADR-0035, the machine summary and WORK_PACKAGES are consistent, and the historical lane verdicts are preserved.

The three new findings are P4:
- the stderr and status-8 wording, which is inexact on the binary's flush-failure path;
- a missing selection-2 test under `v3-capable` (the behavior itself is correct);
- missing negative tests for the new checker anchors.

Two observations outside this lane are not counted. Both sit in `sley-test-runner`, which the NATIVE lane owns:
- The `worker.rs:3-9` module doc still describes a `WorkerReply` frame on stdout, while the code writes raw refusal words.
- The unreachable `Ok(_) => 0` arm at `worker.rs:354` would return a §4-range status with no output once N5 wires dispatch. That change would need a new CLI revision in any case.

This review covers only CLI contract revision 10 at the scoped SHA. It makes no claim about GA or release readiness.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_3_P4_PRIOR_P3_P4_CLOSED
SECTION: cli
FIELD: ariadne_contract_review_revision_10
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS: [P4] [precision] docs/spec/SLEY_CLI_V1.md:225-227,521,529,532-533; crates/sley-cli/src/lib.rs:456-463; crates/sley-cli/src/main.rs:8-14 - §4/§10 say the worker entry writes nothing to stderr and list status 8 for an unwritable output, but the binary's line-buffered StdoutLock always buffers the newline-free refusal words, so a broken channel surfaces at the lib.rs:458-461 flush as exit 4 plus a CLI_IO_FAILURE stderr object (closed-pipe probe) and 8 is library-only - closure: qualify the stderr sentences, scope or realize status 8, add a closed-pipe test | [P4] [evidence-gap] docs/spec/SLEY_CLI_V1.md:148-150,296-300; crates/sley-cli/tests/cli.rs:1333-1451 - §6 requires a v3-capable serve reporting selection 3, 2, or 1, but no test feeds a version 2 hello under v3-capable (only 3 and 1 are asserted); a probe shows the behavior is correct (selected 2) - closure: a cli.rs test asserting selection 2 and version 2 stamping under v3-capable | [P4] [checker-coverage] scripts/check_cli_contract.py:201-217; scripts/test_cli_contract.py - the new revision_record_problems anchors (flat markers, spec-revision-record, adr-current-revision) have no committed negative tests, unlike the sibling anchors from the same commit, and the §10 status 1/8 rows and the stdout/stderr sentence are unanchored - closure: revert cases in test_cli_contract.py plus anchors for the remaining §10 rows
SUMMARY: CLI revision 10 closes all four round-7 Ariadne findings. The v3-capable surface is stated exactly as shipped and anchored by the checker, and the __native-test-worker entry is a bounded exception with a shared argv, disjoint statuses 1/6/7/8, a one-call rule audit with five mutation tests, and a real-binary unit-argv test. ADR-0035 is at revision 10 with records and an anchored current line, and §8 carries SMP1's row-201 qualifier. The checkers exit 0 (PASS, revision 10), the Python suites pass 10 and 16 tests, cargo cli passes 43/43 and the worker tests 6/6. Three P4s remain: inexact stderr and status-8 wording on the flush-failure path, a missing selection-2 test under v3-capable, and missing negative tests for the new checker anchors.
