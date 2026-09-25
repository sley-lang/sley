<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: vulcan; field: vulcan_surface_review_revision_10; dispatched: 2026-09-23T13:45:20Z; duration_s: 414; process_exit_code: 0 -->
# Vulcan Council review — cli
Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

This review was read-only. I made no edits, formatting changes or commits. The temporary fixtures, FIFO and report files were created under the system temp directory, outside the tree. Commands I ran:

- **Scope:** `git rev-parse HEAD` returned `26d050e629b669ef4acbd54826ef4008c05a061d`, so the scope matches. The worktree has only untracked verdict files from other lanes.
- **Git reads:**
  - `git log --oneline 2b0f1c9f..HEAD` (12 commits) and `git diff --stat 2b0f1c9f..HEAD`.
  - `git diff 2b0f1c9f..HEAD` over:
    - `docs/spec/SLEY_CLI_V1.md`
    - `crates/sley-cli/src/lib.rs`
    - `crates/sley-cli/tests/cli.rs`
    - `crates/sley-test-runner/src/worker.rs`
    - `scripts/check_cli_rules.py` and `scripts/test_cli_rules.py`
    - `scripts/check_cli_contract.py`
    - `docs/adr/ADR-0035-thin-cli-boundary.md`
    - `docs/spec/NATIVE_TEST_ADMISSION_V1.md`
    - `docs/WORK_PACKAGES.md` (CLI row)
  - A key-by-key diff of the `machine-summary.json` `cli` section against `2b0f1c9f`.
  - `git log -1` on `docs/audits/S20_430_THIN_CLI_CLOSEOUT.md`: `c60bb620`, 2026-09-10, not touched by this delta.
- **Checkers and tests** (exit codes captured through python subprocess):
  - `python3 scripts/check_cli_contract.py`: exit 0. `"result": "PASS", "revision": 10, "status": "S20_430_IMPLEMENTED_REVIEW_PENDING", "problems": []`.
  - `python3 scripts/check_cli_rules.py`: exit 0. `"result": "PASS"`, 46 tags and 46 names, `frame_literals 1`, `encode_frame_calls 1`, `worker_calls 1`. Dependencies are `serde_json`, `sley-json-bridge`, `sley-protocol` and `sley-test-runner`.
  - `python3 -m unittest scripts/test_cli_contract.py scripts/test_cli_rules.py`: exit 0, `Ran 26 tests … OK`.
  - `cargo test -p sley-cli --locked --offline`: exit 0. `tests/cli.rs` has 43 passed and 0 failed; the other targets ran 0 tests.
  - `cargo test -p sley-test-runner --locked --offline --lib worker`: exit 0, 6 passed. This includes `exit_statuses_are_disjoint_from_the_cli_statuses` and `input_path_entry_reads_the_binding_and_refuses_an_absent_one`.
  - `cargo test -p sley-test-runner --lib unit`: exit 0, 3 passed.
- **Mutations, run in memory against the checker functions** (nothing written to disk):
  - `check_cli_rules.audit_production_source` with a second parser arm under another word, a raw-string alias of the command word, and the absolute-path guard removed.
  - `check_cli_contract.revision_record_problems` with a stale ADR line, a removed `### Revision 10 (` record, and the status 7 and status 8 table rows removed.
- **Direct probes of `$CARGO_TARGET_DIR/debug/sley`,** rebuilt from this tree about 9 minutes earlier:

  | Invocation | Exit | Output |
  |---|---:|---|
  | `version --protocol-profile v3-capable` | 0 | `sley2-cli-v3`, `[1,2,3]` |
  | `version --protocol-profile v4-capable` | 2 | cause `v4-capable` |
  | repeated `--protocol-profile` | 2 | — |
  | `frame decode --protocol-profile v2-capable --expected-version 3` | 2 | cause `--expected-version` |
  | `frame decode --protocol-profile v3-capable --expected-version 4` | 2 | cause `4` |
  | `frame decode --protocol-profile v3-capable --expected-version 3` | 0 | — |
  | `frame decode --expected-version 3` (no profile) | 2 | cause `--expected-version` |
  | `frame decode --protocol-profile v3-capable` (no version) | 2 | cause `--protocol-profile` |
  | `serve --protocol-profile v3-capable`, v2 client hello | 0 | report `sley2-cli-report-v3`, selected 2 |
  | `serve --protocol-profile v3-capable`, v3 client hello | 0 | report selected 3 |
  | worker: no operand; `relative.bin`; `./x`; path plus `extra`; `--report`; uppercase command | 2 | one 43000 JSON object on stderr |
  | worker: junk file | 1 | `\0\0\0\1SCB_LENGTH_OVERFLOW`, nothing on stderr |
  | worker: over-length header | 1 | `SCB_RESOURCE_LIMIT` |
  | worker: short body | 1 | `SCB_LENGTH_OVERFLOW` |
  | worker: `/dev/zero` | 1 | `SCB_MAGIC_INVALID` |
  | worker: `/` or a directory | 1 | `SCB_LENGTH_OVERFLOW` |
  | worker: absent file | 7 | tag 3, `NATIVE_WORKER_INPUT_UNREADABLE` |
  | worker: 5000-byte absent path | 7 | — |
  | worker: stdout to `/dev/full` (junk and absent inputs) | 4 | stderr `{"cause":"STREAM","code":43002,"symbol":"CLI_IO_FAILURE"}` |
  | worker: reader-closed pipe | 4 | the same stderr object |
  | worker: stdout to `/dev/null` or fd 1 closed | 1 | nothing on stderr |
  | worker: a FIFO | — | still blocked in `open` at the 3 s timeout |
  | `sley version $'\xff'` | 101 | Rust panic prose on stderr |
  | `sley __native-test-worker /tmp/$'\xff'.bin` | 101 | Rust panic prose on stderr |

- **Files read:**
  - `docs/spec/SLEY_CLI_V1.md` 1-75, 76-275 and 440-537
  - `crates/sley-cli/src/lib.rs` 200-330 and 396-475
  - `crates/sley-cli/src/main.rs` (whole file)
  - `crates/sley-cli/Cargo.toml`
  - `crates/sley-cli/tests/cli.rs` 1368-1452 and 2421-2506, plus the test index
  - `crates/sley-test-runner/src/worker.rs` 1-45 and 280-380
  - `crates/sley-test-runner/src/unit.rs` 50-140
  - `scripts/check_cli_contract.py` (the diff)
  - `scripts/check_cli_rules.py` (the diff)
  - `docs/adr/ADR-0035-thin-cli-boundary.md` 1-30 and 57-78
  - Status lines of `docs/spec/SMP1.md` (:3, :23) and `docs/spec/SMP1_JSON_BRIDGE_V1.md` (:3)
  - `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md` §13 (784-963)
  - My prior transcript `evidence/review/verdicts/cli/vulcan_surface_review_revision_9-2b0f1c9.md`
  - I did not read sibling revision-10 verdicts.

## Evidence checked

**Round-7 findings of this lane:**

Status: [P2] [undeclared-surface] v3-capable surface: CLOSED.
- Section 1 now lists `v2-capable|v3-capable` on every command, gives the per-profile expected-version rule and the `sley2-cli-v3` metadata, and states that 3 under `v2-capable` is refused with cause `--expected-version` (`SLEY_CLI_V1.md:40-75`).
- Section 2 stamps frames at "1, 2, or 3" (:126). It defines the `offered_hello_v3` offer and says the profile never forces selection 3 (:142-150).
- Section 3 defines `sley2-cli-report-v3` with selected version `1 | 2 | 3 | null` (:207-209), and section 9 covers the v3 profile (:494-504).
- `FLAT_SPEC_MARKERS` anchors this text. The machine-summary `offered_hello` names all three hellos.
- Every probe matches the text, and a v2 client hello under `v3-capable` selects 2.

Status: [P2] [boundary] `__native-test-worker` entry and `sley-test-runner` edge: CLOSED.
- Section 1:74-75 says the entry is not listed, and section 4:225-227 carves out its statuses.
- Section 5:233-241 names the dependency and the transitive `sley-vm`, `sley-scb1`, `sley-tests` and `sley-id`. It bounds the source to one `run_input_path(` call and one command word. `Cargo.toml` matches exactly.
- Section 5:253-254 admits the refusal words, and section 10:506-536 states the purpose, argv, stdout form and status table.
- ADR-0035 decision 8 (:68-77), the machine-summary `bounded_exceptions` field, and the audit's `worker-edge`, `worker-calls` and `worker-commands` checks all carry the exception. The audit has 6 `WorkerExceptionCases`, green.
- Residual corner cases are filed below as new findings (N1, N4), not as a reopening.

Status: [P3] [error-precedence] worker argv mismatch and colliding statuses: CLOSED.
- The parser accepts exactly `__native-test-worker <one path starting with '/'>` (`lib.rs:409-413`).
- The worker's statuses are 1, 6, 7 and 8 (`worker.rs:28-35`), with a disjointness unit test.
- `native_test_worker_entry_runs_the_unit_argv_against_the_real_binary` (`cli.rs:2421-2506`) renders `render_transient_unit` and runs `argv[worker+1..]` against `CARGO_BIN_EXE_sley`. It gets 6, 1 and 7, and 2 for three other argv shapes. My probes reproduce each of these.

Status: [P3] [record-currency] ADR-0035 and section 8 stuck at revision 8: CLOSED.
- ADR-0035:3 reads "draft at revision 10". Its revision 9 and 10 records are at :21-28 and its date line at :30.
- Section 8 has `### Revision 9 (2026-09-23)` (:412) and `### Revision 10 (2026-09-23)` (:428).
- The `adr-current-revision` and `spec-revision-record` gates fire in memory on a stale ADR line and on a removed record.

Status: [P4] [precision] revision-9 note: CLOSED. Section 8 revision 9 quotes "whose method table includes version 2's row 201" (:416). It also states that a legacy serve now fails a non-empty 201 body with a counted `PROTOCOL_PAYLOAD_INVALID` answer (:419-423). All prose is wrapped; the only lines over 100 columns are inside the section 1 usage code block.

**Composition:**
- SMP1 is at revision 15 (`SMP1.md:3`) and names CLI revision 10 back (`SMP1.md:23`).
- The bridge is at revision 12 (`SMP1_JSON_BRIDGE_V1.md:3`).
- The checker constants are 10, 15 and 12.
- The WORK_PACKAGES row and the machine summary agree: `current_delta_review` is `{10, PENDING×3}`, and the revision-9 REVISE fields are kept with dated notes.

**Carried items, not re-filed:** the three closeout P3s from my 178873d round are still open. `S20_430_THIN_CLI_CLOSEOUT.md` was last changed at `c60bb620` and still says "revision 3", and machine-summary `p3_open_count` is 3.

## Findings

[P3] [error-precedence] crates/sley-cli/src/lib.rs:453-463; crates/sley-cli/src/main.rs:8-13; crates/sley-test-runner/src/worker.rs:367-369; docs/spec/SLEY_CLI_V1.md:225-227,521,529,531-533 - The worker writes its refusal words (no newline) into the line-buffered `StdoutLock`, so `write_all` only fills the buffer and cannot fail. The real write happens at the CLI's `stdout.flush()`, which maps a failure to `CLI_IO_FAILURE` (exit 4) and writes a JSON object to stderr. Probes: stdout to `/dev/full`, and a reader-closed pipe, each exit 4 with `{"cause":"STREAM","code":43002,"symbol":"CLI_IO_FAILURE"}` on stderr. As a result:
- Section 10 table row 8, "the refusal words cannot be written", is unreachable through the shipped binary.
- Section 4:225-227 ("it writes nothing to standard error") and section 10:521 ("nothing on standard error") are false on this path.
- The code comment at `lib.rs:453-455` ("never as a CLI JSON failure") is contradicted by `:458-461`.
- Section 6 evidence and the tests cover statuses 6, 1, 7 and 2 only.

The path fails closed, but the contract gives two answers for one condition - closure evidence: one rule. Either the worker flushes inside its own output path and reports any output failure as status 8 with nothing on stderr, without CLI re-mapping; or section 4 and section 10 state status 4 with one stderr object as the output-failure outcome and row 8 is removed. Add a `/dev/full` or closed-pipe test against the real binary.

[P3] [fail-closed] crates/sley-cli/src/main.rs:6; docs/spec/SLEY_CLI_V1.md:64-67,213-227,515-516 - `std::env::args()` panics on any non-UTF-8 argument. `sley version $'\xff'` and `sley __native-test-worker /tmp/$'\xff'.bin` each exit 101 with Rust panic prose on stderr. That breaks three rules:
- section 1: exact arguments refused as `CLI_USAGE_INVALID`, and no prose on any stream;
- section 4: the exit-status table, and nothing else on stderr;
- revision 10's new section 10 rule that any other argv shape is `CLI_USAGE_INVALID`.

The defect predates this delta (`main.rs` is unchanged), and no earlier CLI verdict records it. The supervisor renders `&str` paths, so production worker argv is always UTF-8; the practical reach is a non-UTF-8 `--repository` path. It fails closed, since no frame is answered - closure evidence: read `args_os()` and map a non-UTF-8 word to `CLI_USAGE_INVALID` (exit 2, one JSON object), or declare the exception in section 4; add a test.

[P4] [resource-bound] crates/sley-test-runner/src/worker.rs:322-327,337-349; docs/spec/SLEY_CLI_V1.md:518-528 - `run_input_path` uses a plain `File::open`. It follows symlinks, blocks indefinitely on a FIFO (probe: still blocked at the 3 s timeout) and accepts a directory. A read error (EISDIR on `/` or a directory, or any I/O error) is reported as tag 1 / status 1 "malformed envelope" with `SCB_LENGTH_OVERFLOW`, so status 7 ("unreadable") covers only a failed open. Inside the unit the path is daemon-owned and `RuntimeMaxUSec` bounds a hang, so this is a note - closure evidence: open non-blocking and no-follow, or check for a regular file with `fstat`, and map read I/O errors to tag 3 / status 7; or state in section 10 that the binding's file type is the daemon's guarantee and that read errors report as malformed.

[P4] [audit-completeness] scripts/check_cli_rules.py:142-158; docs/spec/SLEY_CLI_V1.md:239-241,536 - The audit counts the literal `"__native-test-worker"` and the call text. It does not count the paths that construct `Command::NativeTestWorker`. An in-memory mutation adding `"worker2" if rest.len() == 1 => Ok(Command::NativeTestWorker { .. })` passes with `problems []` (`worker_calls` 1, `worker_commands` 1). A second command word can therefore reach the worker despite the stated "one command word" bound - closure evidence: require exactly one `Command::NativeTestWorker {` construction, or anchor the parser arm, with a mutation test.

[P4] [test-coverage] scripts/check_cli_contract.py:201-217; scripts/test_cli_contract.py (unchanged in this delta); crates/sley-cli/tests/cli.rs:1333-1452; docs/spec/SLEY_CLI_V1.md:148-150,296-300 - Two coverage gaps:
- The new `revision_record_problems` gates (`adr-current-revision`, `spec-revision-record`, `FLAT_SPEC_MARKERS`) have no negative tests. They work in memory, but the sibling packages received revert tests this round.
- Section 6 claims a `v3-capable` serve "reporting the actual selection (3, 2, or 1)", but only selections 3 and 1 are tested. My probe shows selection 2 works (v2 client hello → report selected 2).

Closure evidence: revert tests for the three gates, and a `v3-capable` serve test with a version 2 client hello.

## Assessment

Revision 10 answers every finding of my round-7 transcript with evidence I reproduced independently:
- The shipped v3-capable surface is now declared in sections 1, 2, 3 and 9 and anchored by checker markers. The binary refuses exactly what the text says it refuses.
- The worker entry and its `sley-test-runner` edge are a stated, audited exception. The spec, ADR-0035, the summary and `Cargo.toml` agree on it.
- The argv mismatch is fixed in code. A test drives the supervisor's own rendered argv against the real binary.
- The worker's own exit statuses no longer collide with section 4.
- The ADR and section 8 records are current and gated.

What remains:
- The worker's output-failure path contradicts the "nothing on stderr" and status-8 statements, because stdout buffering sends that failure through the CLI's own flush as status 4.
- Non-UTF-8 argv panics with prose and exit 101. This predates the delta.
- Three P4 notes: worker input-open semantics, a gap in the command-word audit, and missing gate and selection-2 tests.

All of these fail closed. None blocks this lane's acceptance of contract revision 10. The three carried closeout P3s stay open, so I do not append the prior-closure suffix.

VERDICT: PASS_0_P0_0_P1_0_P2_2_P3_3_P4
SECTION: cli
FIELD: vulcan_surface_review_revision_10
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS: [P3] [error-precedence] crates/sley-cli/src/lib.rs:453-463; crates/sley-cli/src/main.rs:8-13; crates/sley-test-runner/src/worker.rs:367-369; docs/spec/SLEY_CLI_V1.md:225-227,521,529,531-533 - refusal words sit in the line-buffered StdoutLock, so a write failure surfaces at the CLI flush as CLI_IO_FAILURE exit 4 with a JSON object on stderr (probes: /dev/full and closed pipe → 4 + stderr); row 8 is unreachable, the "nothing on standard error" statements (§4, §10) and the lib.rs:453-455 comment are false on this path, and the path is untested - closure: one rule (worker-internal flush → status 8 and no stderr, or the contract states status 4 + stderr and drops row 8) plus a real-binary /dev/full or closed-pipe test; [P3] [fail-closed] crates/sley-cli/src/main.rs:6; docs/spec/SLEY_CLI_V1.md:64-67,213-227,515-516 - std::env::args() panics on non-UTF-8 argv (probes: `version $'\xff'` and the worker with a non-UTF-8 path → exit 101 with panic prose on stderr), against the exact-argument refusal, the no-prose rule, the §4 status table and §10's "any other shape is CLI_USAGE_INVALID"; predates the delta; fails closed - closure: args_os() mapping non-UTF-8 words to CLI_USAGE_INVALID (exit 2, one JSON object) or a declared exception, plus a test; [P4] [resource-bound] crates/sley-test-runner/src/worker.rs:322-327,337-349; docs/spec/SLEY_CLI_V1.md:518-528 - File::open follows symlinks, blocks on a FIFO (probe: still blocked at 3 s) and accepts directories; read I/O errors report as status 1 malformed (SCB_LENGTH_OVERFLOW), so status 7 covers only a failed open - closure: no-follow non-blocking open or an fstat regular-file check with read errors → tag 3 / status 7, or §10 stating the daemon guarantee; [P4] [audit-completeness] scripts/check_cli_rules.py:142-158; docs/spec/SLEY_CLI_V1.md:239-241,536 - the audit counts the command-word literal, not Command::NativeTestWorker constructions; a second parser arm under another word passes (in-memory mutation: problems []) - closure: exactly one construction or an anchored arm, with a mutation test; [P4] [test-coverage] scripts/check_cli_contract.py:201-217; scripts/test_cli_contract.py; crates/sley-cli/tests/cli.rs:1333-1452; docs/spec/SLEY_CLI_V1.md:148-150,296-300 - the new adr-current-revision, spec-revision-record and flat-marker gates have no revert tests, and §6's v3-capable selection "(3, 2, or 1)" has no selection-2 test (the probe shows it works) - closure: gate revert tests and a v3-capable serve test with a v2 client hello
SUMMARY: All five round-7 findings of this lane are CLOSED, verified against code, checkers (contract PASS at revision 10, rules PASS, 26 unit tests OK), cargo tests (sley-cli 43/43, runner worker 6/6, unit 3/3) and direct probes of the real binary. The v3-capable surface is declared, and the worker entry is a bounded, audited exception whose argv matches the supervisor's rendering and whose own statuses 1/6/7/8 avoid section 4. New findings: the worker's output-failure path exits 4 with a stderr object, contradicting row 8 and the "nothing on stderr" statements, and non-UTF-8 argv panics with prose (predates the delta); both are P3 and fail closed. There are also three P4 notes, and the three carried closeout P3s remain open.
