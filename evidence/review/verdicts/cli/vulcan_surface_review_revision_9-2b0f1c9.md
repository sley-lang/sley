<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2b0f1c9f4940c020565137891a49a3769cd02061; role: vulcan; field: vulcan_surface_review_revision_9; dispatched: 2026-09-23T12:15:56Z; duration_s: 474; process_exit_code: 0 -->
# Vulcan Council review — cli
Harness: claude-code
Reviewed checkpoint: 2b0f1c9f4940c020565137891a49a3769cd02061

This review was read-only. I made no edits, formatting changes or commits. Commands I ran:

- **Scope:** `git rev-parse HEAD` returned `2b0f1c9f4940c020565137891a49a3769cd02061`, so the scope matches. The worktree has only untracked verdict files from other lanes.
- **Git reads:**
  - `git diff ab42a3a9..HEAD -- docs/spec/SLEY_CLI_V1.md`: 13 lines changed.
  - `--stat` over the spec and `machine-summary.json`.
  - `git log ab42a3a9..HEAD` over the CLI spec, crate, checkers and summary. Only `a8b4cddb` and `8357c243` touch the spec and checker. `crates/sley-cli` has no diff since the base.
  - `git show a8b4cddb` and `git show 8357c243` on the CLI spec, `check_cli_contract.py` and `test_cli_contract.py`.
  - `git diff ab42a3a9..HEAD -- crates/sley-protocol/src/server.rs` and the bridge spec.
  - `git show ab42a3a9:docs/spec/SMP1.md`.
  - `git log -S` on the v3 crate markers (`0c655fc1`, 2026-09-16) and the `sley-test-runner` allowlist (`955fcdc6`, 2026-09-17).
- **Checkers and tests** (run through python subprocess; exit codes captured):
  - `python3 scripts/check_cli_contract.py`: exit 0. `"result": "PASS", "revision": 9, "status": "S20_430_IMPLEMENTED_REVIEW_PENDING", "problems": []`.
  - `python3 scripts/check_cli_rules.py`: exit 0. `"result": "PASS"`, 46 tags and 46 names, 1 frame literal, 1 encode call. `"dependencies": ["serde_json","sley-json-bridge","sley-protocol","sley-test-runner"]`.
  - `python3 -m unittest scripts/test_cli_contract.py scripts/test_cli_rules.py`: exit 0, `Ran 20 tests … OK`.
  - `python3 scripts/generate_smp1_json_bridge_table.py --check --protocol-version {1,2,3}`: exit 0 each, `"result": "PASS"` for v1, v2 and v3 `methods.json`.
  - `cargo test -p sley-cli --locked`: `tests/cli.rs` `43 passed; 0 failed`. The other targets ran 0 tests.
  - `cargo test -p sley-protocol --locked --lib workspace_open`: 4 passed.
    - `workspace_open_under_version_1_never_carries_field_9`
    - `workspace_open_v2_discloses_only_the_materialized_head_snapshot`
    - `workspace_open_v3_answers_the_version_2_open_summary`
    - `workspace_open_probe_is_non_waiting_and_never_rewrites_a_discarded_record`
  - Direct probes of the built binary `$CARGO_TARGET_DIR/debug/sley`, which was just rebuilt from this tree:

    | Invocation | Exit | Output |
    |---|---:|---|
    | `version --protocol-profile v3-capable` | 0 | `{"cli":"1","contract":"sley2-cli-v3","protocol_profile":"v3-capable","protocol_versions":[1,2,3]}` |
    | `version --protocol-profile v4-capable` | 2 | 43000 |
    | `frame decode --protocol-profile v3-capable --expected-version 3` | 0 | — |
    | `frame decode --protocol-profile v2-capable --expected-version 3` | 2 | — |
    | `methods --protocol-profile v3-capable` | — | 46 rows including 600–607 |
    | `__native-test-worker` with input `junk` | 1 | `\x00\x00\x00\x01SCB_LENGTH_OVERFLOW` on stdout |
    | `__native-test-worker /run/sley-test-supervisor/input/9f2c.bin` | 2 | stderr `{"cause":"__native-test-worker","code":43000,"symbol":"CLI_USAGE_INVALID"}` |

- **Files read:**
  - `docs/spec/SLEY_CLI_V1.md` 1-410 (whole file)
  - `docs/spec/SMP1.md` 1-100, 210-269, 320-349, 640-780
  - `docs/spec/SMP1_JSON_BRIDGE_V1.md` (the diff)
  - `docs/spec/NATIVE_TEST_ADMISSION_V1.md` 1-12, 530-594
  - `crates/sley-protocol/src/server.rs` (the diff: 1283, 2887-2933, 3282-3318)
  - `crates/sley-protocol/src/server_tests.rs` 612-640
  - `crates/sley-cli/src/lib.rs` 170-470, 488-567
  - `crates/sley-cli/src/main.rs` (whole file)
  - `crates/sley-cli/Cargo.toml`
  - `crates/sley-cli/tests/cli.rs` 60-299, 2418-2457
  - `crates/sley-json-bridge/src/lib.rs` 50-72, 846, 883
  - `crates/sley-test-runner/src/worker.rs` 293-343
  - `crates/sley-test-runner/src/unit.rs` 60-135, 196-216
  - `crates/sley-test-runner/Cargo.toml`
  - `scripts/check_cli_contract.py` 60-280
  - `scripts/check_cli_rules.py` 1-60
  - `docs/adr/ADR-0035-thin-cli-boundary.md` 1-25
  - `docs/WORK_PACKAGES.md:49`
  - `packaging/sley-test-supervisor/README.md` 1-20
  - `machine-summary.json` `cli` section, diffed against `ab42a3a9`
  - My prior closure transcript `evidence/review/verdicts/cli/vulcan_surface_review_closure-178873d.md`
- **Sibling verdicts:** I read the finding lines of the sibling revision-9 verdicts (Ariadne and Nabu at 2b0f1c9) only after I had found the v3 profile and the dependency drift myself, through `lib.rs` and the `check_cli_rules.py` output. I re-verified every fact I rely on. The argv/exit-status finding (#3) is my own and does not appear in either sibling verdict.

## Evidence checked

**The re-pin itself is mechanically complete.**
- The composition sentence names `docs/spec/SMP1.md` revision 14 and the bridge at revision 11 (`SLEY_CLI_V1.md:28-31`).
- Both documents carry those revisions on their Status lines: SMP1 at `SMP1.md:3` and the bridge at `SMP1_JSON_BRIDGE_V1.md:3`.
- SMP1 names the reverse composition: bridge revision 11 and CLI revision 9 (`SMP1.md:21-23`).
- The checker constants are `SPEC_REVISION 9`, `SMP1_REVISION 14` and `BRIDGE_REVISION 11`. `composition_pin_problems` anchors the normative sentence, and it has stale-SMP1 and stale-bridge revert tests (`test_cli_contract.py`, 20/20 green).
- The `a8b4cddb` in-place "no CLI revision" SMP1-13 note was withdrawn in favour of a dated revision 9, which matches the revision-13 ruling.
- In the machine summary, `current_delta_review` is `{9, PENDING×3}` and the status is `S20_430_IMPLEMENTED_REVIEW_PENDING`, with `implementation_complete false`.
- The revision-8 history pin line (`:359`) is retained as history.

**SMP1 revision 14 does not falsify any CLI clause about 201 bytes or the method table.**
- **Byte mode:** the CLI hands frame bytes to the server unchanged (§2:95-98).
- **JSON mode:** the bridge renders `body` as hex (`sley-json-bridge/src/lib.rs:846` render, `:883` parse). The 201 body is therefore opaque in both modes, and the "opaque bytes" claim holds.
- The generated v1, v2 and v3 method tables have no body columns. They regenerate from SMP1 rev 14 without drift. Row 201 is identical in all three: `{family: repository, name: workspace.open, owner: S20-390, reserved: false, tag: 201}`.
- **Server behaviour:** `workspace_open(body)` refuses a non-empty body with `PROTOCOL_PAYLOAD_INVALID` before loading the head. The probe runs only when `protocol_version >= 2`. Version 1 returns the eight-field `revision_summary` byte for byte (server_tests 636-639).
- **Report:** such a refusal is a failed answer whose `ProtocolFailure` decodes. It counts in `failed_answers` and `codes`, per §3, and the exit status does not move. The CLI's only 201 request (`cli.rs:136`) has an empty body on the legacy selection, so the §6 legacy byte-identity evidence is unaffected.
- **Byte identity under the capable profiles:** the CLI claims none, so SMP1's derived-cache-state determinism scope (`SMP1.md:747-750`) falsifies nothing here.

**Version 3 enters the composed authority with this re-pin.**
- At the base, SMP1 revision 12's frame entrypoints "admit only selections 1 and 2" (`git show ab42a3a9:docs/spec/SMP1.md:234`).
- Revision 14 admits 1, 2 and 3, and names NATIVE_TEST_ADMISSION appendices C and D as the owner of version 3 (`SMP1.md:220-227, 255-259`).
- The CLI revision-9 note itself cites "every later selection".
- Revision 9 is therefore the first CLI pin whose authority admits version 3. That makes the CLI's own version-3 and worker surface a question for this revision (findings 1-3).

**Carried items, not re-filed here:** the three closeout P3s from my 178873d closure round are still open. They are tracked in `machine-summary cli.p3_open`, and `docs/audits/S20_430_THIN_CLI_CLOSEOUT.md` was last touched at `c60bb620` (2026-09-10).

## Findings

[P2] [undeclared-surface] docs/spec/SLEY_CLI_V1.md:38-60,110-112,159-179,363-398 - The contract says `--protocol-profile` "takes exactly `v2-capable`; any other value is `CLI_USAGE_INVALID`" (:57-58). It allows `--expected-version 1|2` only, stamps post-handshake frames "(1 or 2)", limits `selected_protocol_version` to `1 | 2 | null`, and describes only the `[1,2]` offer. The binary at this SHA accepts `v3-capable` (probe exit 0 with `sley2-cli-v3`, `[1,2,3]`). It accepts `--expected-version 3`, offers `Server::offered_hello_v3`, prints the 46-row v3 table, and emits `sley2-cli-report-v3` (crates/sley-cli/src/lib.rs:35-48,196-216,304-314,491-566). No contract defines this surface: NATIVE_TEST_ADMISSION_V1.md:541 says only "CLI is thin SMP routing", and its appendix D (:545-577) has method rows only. This package's gate enforces the contradiction. check_cli_contract.py:88,93,101-103 requires the v3 markers in the crate, has no matching spec markers, and still pins `offered_hello` to the legacy and v2 hellos (:218). This is a fail-closed refusal the contract states but the binary does not perform. SMP1 revision 14 (SMP1.md:220-227,255-259) now makes version 3 a normative selection with a 201 `open_summary` answer, and this surface is the only CLI path to it - closure evidence: a CLI revision that either declares the v3-capable profile (offer, expected version 3, stamping, v3 metadata/report shapes, v3 table) or defers it by exact section to a contract that actually defines it; §1/§2/§3/§9 made true; spec markers in check_cli_contract.py; machine-summary `offered_hello` updated; the checker and tests green.

[P2] [boundary] crates/sley-cli/src/lib.rs:269-275,404,444-455; crates/sley-cli/Cargo.toml:17,23-24; scripts/check_cli_rules.py:15-30 - `sley __native-test-worker` is a command the contract does not list. §1:54-56 makes an unknown command `CLI_USAGE_INVALID`, and §1:56 and §5:214-216 forbid output that is not a frame, a report or the version object. The worker writes raw tag+ASCII refusal words to stdout (probe: `\x00\x00\x00\x01SCB_LENGTH_OVERFLOW`). Its exit statuses 1/2/3 pass through unwrapped (worker.rs:329-343): 1 is not in the §4 table (:184-196), and 2 and 3 collide with `CLI_USAGE_INVALID` and `CLI_INPUT_INVALID` with no stderr object. The normal dependency on `sley-test-runner` links `sley-vm`, `sley-scb1` and `sley-tests` into the transport binary. `sley-vm` and `sley-scb1` are crates §5:206-209 names as kernel, and the machine-summary `forbidden` list includes `kernel_dependency` and `prose_output`. §5:202-204 says the crate depends "only" on three crates. The audit that §5:200 says "fails closed when any of these drifts" passes only because `955fcdc6` widened its allowlists without a CLI revision, and its own PASS output lists the fourth dependency - closure evidence: either a CLI revision that admits the worker entry and runner edge as a stated boundary exception (dependency, stdout form, exit-status space, why a VM-linked worker fits "a transport endpoint and nothing else"), with §1/§4/§5, the ADR-0035 decision, the machine summary and check_cli_rules.py agreeing and a test pinning the exception; or the worker moved to its own binary and the §5 allowlists restored.

[P3] [error-precedence] crates/sley-test-runner/src/unit.rs:102-110,206-211 vs crates/sley-cli/src/lib.rs:272-273,404 - The supervisor's transient unit renders `<worker_path> __native-test-worker <worker_input_path>`, and its own test pins this three-word argv. The CLI accepts the worker entry only with no further words. `cli.rs:2452-2456` pins `__native-test-worker extra` as `CLI_USAGE_INVALID`, and the binary probe with the rendered path returns exit 2 with 43000 on stderr. The only production-shaped invocation therefore never reaches the worker. The doc comment claiming the supervisor spawns "exactly this argv" is false. That usage refusal exits 2, the same status as the worker's own `NATIVE_WORKER_EXECUTION_NOT_WIRED` refusal (worker.rs:339-342). A supervisor keying on exit status cannot tell "argv rejected before the worker" from "worker refused: not wired", so the mismatch is masked until N5 wires execution. It fails closed today, since nothing executes on either path - closure evidence: one argv contract, in which either the supervisor stops passing the path and pipes the frame, or the CLI accepts exactly one absolute input path; worker exit statuses disjoint from the §4 CLI statuses; and a test that drives the argv from `render_transient_unit` against the real `sley` binary.

[P3] [record-currency] docs/adr/ADR-0035-thin-cli-boundary.md:3-22; docs/spec/SLEY_CLI_V1.md:266-360; scripts/check_cli_contract.py:64-76 - ADR-0035 still reads "the S20-430 contract is a draft at revision 8" and has no revision-9 record; its only "revision 9" text is the revision-7 bridge pin at :16. `docs/WORK_PACKAGES.md:49` nevertheless cites "(revision 9, 2026-09-23, ADR-0035: re-pins SMP1 revision 14 …". The same repair commit `8357c243` refreshed ADR-0032, ADR-0033 and ADR-0036 but not ADR-0035. Revisions 3 and 5-8 each have a §8 history entry, but revision 9 has none. `ADR_MARKERS` stop at "Revision 8 record", so the gap passes the checker - closure evidence: a dated revision-9 status and record in ADR-0035, a §8 revision-9 entry, and an ADR marker in check_cli_contract.py, with the checker green.

[P4] [precision] docs/spec/SLEY_CLI_V1.md:17-19 - The revision-9 note paraphrases SMP1 as "version 2 and every later selection". It drops SMP1's qualifier "whose method table includes version 2's row 201" (SMP1.md:63-64,682,722-725). It also says "no CLI clause or behavior changes", but the default legacy `sley serve` now answers a non-empty 201 body with a failed `PROTOCOL_PAYLOAD_INVALID` answer, counted in `failed_answers` and `codes`. The server that revision 8 composed ignored that body (the pre-change `self.workspace_open()` call took no body). The CLI code is unchanged, but the invocation's output changes for non-conforming input; SMP1:72-75 calls this its one version-1 observable change. Line 18 is 249 columns in a hard-wrapped paragraph - closure evidence: carry the qualifier, state the legacy-default fail-closed tightening for non-empty 201 bodies, and wrap the line.

## Assessment

The revision-9 delta does what it claims. The composition sentence, Status line, checker constants and machine summary all move together to SMP1 revision 14 and bridge revision 11, and the checker anchors the normative sentence with revert tests. SMP1 revision 14's `workspace.open` changes falsify no CLI statement about bytes or the method table:
- 201 bodies travel opaquely in both modes (raw frames, hex JSON).
- The generated tables have no body columns and regenerate without drift.
- The legacy byte-identity evidence uses an empty-body version-1 open, whose bytes are unchanged.
- A refused non-empty body is an ordinary counted failed answer.

The revision cannot be accepted as a true description of the surface, though. This re-pin is the first CLI pin whose authority admits protocol version 3. At this SHA the `sley` binary ships an undeclared v3-capable profile and a private `__native-test-worker` entry backed by a VM-linked dependency. The contract still states exact refusals ("any other value is `CLI_USAGE_INVALID`", unknown commands refused, three dependencies only) that the binary does not perform. The package's own gates enforce the undeclared surface: crate markers require the v3 strings, and the rule audit's allowlist was widened silently. The worker entry also has a live argv mismatch with its only launcher, and its exit statuses collide with the CLI's. Nothing executes today, so this is fail-closed, but the mismatch is hidden. All of these predate the delta, from `0c655fc1`, `6867e6a9` and `955fcdc6` on 2026-09-16/17; no CLI revision or CLI-lane review has recorded them since revision 8. The two P2s must be answered by a CLI revision, or by moving the surfaces out of the crate, before this lane can accept.

VERDICT: REVISE_0_P0_0_P1_2_P2_2_P3_1_P4
SECTION: cli
FIELD: vulcan_surface_review_revision_9
SCOPE_SHA: 2b0f1c9f4940c020565137891a49a3769cd02061
FINDINGS: [P2] [undeclared-surface] docs/spec/SLEY_CLI_V1.md:38-60,110-112,159-179,363-398 - contract says --protocol-profile "takes exactly v2-capable; any other value is CLI_USAGE_INVALID", expected version 1|2, stamping "(1 or 2)", selected 1|2|null, [1,2] offer; binary accepts v3-capable (probe exit 0, sley2-cli-v3, [1,2,3]), --expected-version 3, offered_hello_v3, the v3 table and sley2-cli-report-v3 (crates/sley-cli/src/lib.rs:35-48,196-216,304-314,491-566); no contract defines it (NATIVE_TEST_ADMISSION_V1.md:541,545-577); check_cli_contract.py:88,93,101-103 requires the v3 crate markers while :218 pins offered_hello without v3; SMP1 rev 14 (SMP1.md:220-227,255-259) newly admits version 3 - closure: CLI revision declaring or exactly deferring the v3-capable surface, §1/§2/§3/§9 made true, spec markers, machine-summary offered_hello updated, checker green; [P2] [boundary] crates/sley-cli/src/lib.rs:269-275,404,444-455; crates/sley-cli/Cargo.toml:17,23-24; scripts/check_cli_rules.py:15-30 - undeclared `__native-test-worker` command writes raw refusal words to stdout with pass-through exits 1/2/3 (outside/colliding with §4), links sley-vm/sley-scb1/sley-tests via sley-test-runner against §5:202-209 and machine-summary forbidden kernel_dependency/prose_output; the §5 fail-closed audit passes only because 955fcdc6 widened its allowlists without a CLI revision - closure: CLI revision admitting the entry and edge as a stated exception with §1/§4/§5, ADR-0035, machine summary, audit and a pinning test agreeing, or the worker moved to its own binary and the allowlists restored; [P3] [error-precedence] crates/sley-test-runner/src/unit.rs:102-110,206-211 vs crates/sley-cli/src/lib.rs:272-273,404 - the supervisor renders `<worker> __native-test-worker <input_path>` but the CLI refuses any extra word as CLI_USAGE_INVALID exit 2 (cli.rs:2452-2456; binary probe confirms), the same exit as the worker's NATIVE_WORKER_EXECUTION_NOT_WIRED (worker.rs:339-342), so the launcher cannot distinguish the two refusals and the mismatch is masked - closure: one argv contract, disjoint worker exit statuses, and a test driving render_transient_unit's argv against the real binary; [P3] [record-currency] docs/adr/ADR-0035-thin-cli-boundary.md:3-22; docs/spec/SLEY_CLI_V1.md:266-360; scripts/check_cli_contract.py:64-76 - ADR-0035 still says draft at revision 8 with no revision-9 record while WORK_PACKAGES.md:49 cites ADR-0035 for revision 9; no §8 revision-9 entry; ADR_MARKERS stop at revision 8 - closure: ADR-0035 revision-9 status and record, §8 entry, checker marker; [P4] [precision] docs/spec/SLEY_CLI_V1.md:17-19 - the note drops SMP1's "whose method table includes version 2's row 201" qualifier, and "no CLI clause or behavior changes" omits that legacy-default serve now fails a non-empty 201 body (PROTOCOL_PAYLOAD_INVALID, counted) that revision 8's composed server answered; line 18 is 249 columns - closure: carry the qualifier, state the fail-closed tightening, wrap the line
SUMMARY: The revision-9 pin move is complete and mechanically anchored: composition sentence, status lines, checker constants and machine summary agree on SMP1 14 and bridge 11, and SMP1 rev 14's workspace.open changes falsify no CLI statement about 201 bytes or the method table. 201 bodies are opaque in both modes, the generated tables regenerate without drift, and CLI 43/43 and server 201 4/4 tests pass. However, this is the first CLI pin whose authority admits version 3, and the binary ships an undeclared v3-capable profile and a VM-linked `__native-test-worker` entry. These contradict the contract's exact refusal, exit-status and dependency rules, while the package's own gates enforce them. The worker entry also has a masked argv/exit-status mismatch with its launcher. REVISE until a CLI revision declares or removes these surfaces.
