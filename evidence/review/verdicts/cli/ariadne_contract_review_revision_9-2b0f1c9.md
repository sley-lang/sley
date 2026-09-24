<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2b0f1c9f4940c020565137891a49a3769cd02061; role: ariadne; field: ariadne_contract_review_revision_9; dispatched: 2026-09-23T12:02:11Z; duration_s: 427; process_exit_code: 0 -->
# Ariadne Council review — cli

Harness: claude-code
Reviewed checkpoint: 2b0f1c9f4940c020565137891a49a3769cd02061

This was a read-only review. I did not edit, create, or commit anything in the tree. The two untracked verdict files that `git status` lists existed before this review started.

**Commands run, with exit codes**
- `git rev-parse HEAD` printed `2b0f1c9f4940c020565137891a49a3769cd02061`, which matches the scope SHA.
- `git merge-base --is-ancestor ab42a3a9 HEAD` confirmed ab42a3a9 is an ancestor.
- `git diff ab42a3a9..HEAD` was read over:
  - `docs/spec/SLEY_CLI_V1.md` (+8/−5)
  - `scripts/check_cli_contract.py`
  - `scripts/test_cli_contract.py`
  - `docs/spec/SMP1.md`
  - `docs/spec/SMP1_JSON_BRIDGE_V1.md`
  - `crates/sley-protocol/src/server.rs` (hunk headers)
- `git diff ab42a3a9..HEAD --stat -- crates/sley-cli crates/sley-json-bridge crates/sley-protocol`: only `server.rs` and `server_tests.rs` changed. The CLI and bridge crates are unchanged in this range.
- `git show a8b4cddb` and `git show 8357c243`: the only two commits in the range that touch the CLI spec or checker.
- `git log -S` located where the unstated surfaces came from:
  - `"v3-capable"` entered in 0c655fc1 (2026-09-16).
  - `sley-test-runner` entered in 6867e6a9 and 955fcdc6 (2026-09-16/17).
  - All three commits are ancestors of 178873d7 and of ab42a3a9.
- `python3 scripts/check_cli_contract.py`: exit 0, `"result": "PASS"`, `"revision": 9`, `"status": "S20_430_IMPLEMENTED_REVIEW_PENDING"`, `"problems": []`.
- `python3 -m unittest scripts.test_cli_contract -v`: exit 0, 10 tests OK. This includes the 3 new `CompositionSentenceCases`.
- `python3 scripts/check_cli_rules.py`: exit 0, PASS. `method_names_audited` 46 and `method_tags_audited` 46. Its `dependencies` output lists `serde_json`, `sley-json-bridge`, `sley-protocol` and `sley-test-runner`.
- `python3 -m unittest scripts.test_cli_rules`: exit 0, 10 tests OK.
- `python3 scripts/check_smp1_contract.py`: exit 0, PASS, revision 14.
- `python3 scripts/check_smp1_json_bridge_contract.py`: exit 0, PASS, revision 11.
- `python3 -m unittest scripts.test_current_contract_review`: exit 0, 6 tests OK.
- `python3 -m unittest scripts.test_smp1_contract`: exit 0, 16 tests OK.
- `cargo test -p sley-cli --test cli`: `test result: ok. 43 passed; 0 failed`. This includes the `v3_*` tests and `native_test_worker_entry_passes_refusal_words_through_unwrapped`.

**Files read**
- `docs/spec/SLEY_CLI_V1.md` 1-409 (whole file)
- `scripts/check_cli_contract.py` 1-279 (whole file)
- `scripts/check_cli_rules.py` 10-32
- `crates/sley-cli/src/lib.rs` 195-215, 400-470, 1050-1160 (plus a grep for 201, workspace, body and v3)
- `crates/sley-cli/Cargo.toml` (whole file)
- `crates/sley-cli/tests/cli.rs` 100-219 (plus a grep for WorkspaceOpen, v3 and the worker)
- `crates/sley-protocol/src/server.rs` 595-660, 2880-2925, 3280-3320
- `crates/sley-test-runner/Cargo.toml` `[dependencies]`
- `crates/sley-test-runner/src/worker.rs` 305-335
- `docs/spec/NATIVE_TEST_ADMISSION_V1.md` 1-12, 530-600
- `docs/adr/ADR-0035-thin-cli-boundary.md` 1-40
- `docs/WORK_PACKAGES.md` :49
- the ADR-0032 and ADR-0033 hunks of 8357c243
- `machine-summary.json` `cli` section, diffed field by field against ab42a3a9
- `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md` 603-752
- `evidence/review/verdicts/cli/vulcan_surface_review_closure-178873d.md` (grep)
- `evidence/review/verdicts/protocol/ariadne_contract_review_revision_13-f073811.md` (grep)

## Evidence checked

**The re-pin itself: complete in the spec, checker, summary and work-package row.**
- Spec pins:
  - `SLEY_CLI_V1.md:3` gives revision 9 (2026-09-23).
  - The composition sentence (`:28-31`) now names SMP1 revision 14 and bridge revision 11.
  - The history sentence (`:17-22`) records the move.
  - The pin-only paragraph that a8b4cddb added under revision 8 was removed by 8357c243. That closes the revision-13 ruling that each owner re-pins through its own revision.
- Checker:
  - `check_cli_contract.py:31-33` gives 9/14/11.
  - `:247-263` cross-checks each pin against the SMP1 and bridge Status lines.
  - `composition_pin_problems` (`:151-175`) anchors the authority sentence, so the stale-`:26` blindness from the revision-13 round is closed. Its two negative tests (`test_cli_contract.py`) pass.
- Machine summary `cli`, compared with ab42a3a9:
  - `contract_revision` is 9.
  - `current_delta_review` is `{9, PENDING×3}`.
  - `status` moved from `S20_430_COMPLETE` to `S20_430_IMPLEMENTED_REVIEW_PENDING`.
  - `implementation_complete` is false, as `check_cli_contract.py:219` requires.
  - `status_note` is dated and keeps the previous note.
  - The historical lane fields and the three carried Vulcan P3s on the closeout are preserved.
- Other records: `WORK_PACKAGES.md:49` carries the revision-9 marker, and `SMP1.md:21-23` names CLI revision 9 as the current composition.

**The claim that the CLI carries 201 bodies as opaque bytes holds.**
- The CLI source has no method match arms. `check_cli_rules.py` audits 46 names and tags and passes.
- The bridge renders every body as `hex` (`SMP1_JSON_BRIDGE_V1.md:108`).
- The only body the CLI ever decodes is a failed answer's `ProtocolFailure`, used for report `codes` (`lib.rs:1066-1080`). A 201 refusal (`PROTOCOL_PAYLOAD_INVALID`) is counted the same way as any other failure.
- `sley-cli` and `sley-json-bridge` have no changes in this range.
- The server's 201 path matches SMP1 revision 14 (`server.rs:2904-2916`):
  - a non-empty body is refused;
  - version 1 answers `revision_summary`;
  - version 2 and later add optional field 9 through `head_open_summary` (`:3292-3301`).

**Section 6 byte-identity evidence is not made false by SMP1's determinism scope (`SMP1.md:748-750`).**
- The mode-comparison fixture uses legacy `Server::new` and version 1, with an empty-body `WorkspaceOpen` (`cli.rs:117-143`, `:136`).
- Under version 1, 201 answers the cache-independent `revision_summary` (`server.rs:2909-2913`).
- No capable CLI test compares 201 bytes.

**The version-1 observable change is disclosed.** A non-empty 201 body is now refused under the legacy default serve. This is the server's judgment, and `:18` states it.

**Surfaces the contract does not state** (both came from commits that landed while the package was COMPLETE at revision 8):
- **v3-capable profile.**
  - `lib.rs:201` and `:213` accept `v3-capable` and expected version 3.
  - `server.rs:645` offers `[1,2,3]`.
  - `cli.rs:1323-1328` asserts the output `{"cli":"1","contract":"sley2-cli-v3",...,"protocol_versions":[1,2,3]}`.
  - The checker requires those markers (`check_cli_contract.py:88,93,101-103`).
  - No `docs/spec` file defines `v3-capable`, `sley2-cli-v3`, or `sley2-cli-report-v3` (repository grep).
- **Worker entry.**
  - `lib.rs:404` and `:444-455` add a `__native-test-worker` command.
  - `Cargo.toml:17` adds a normal dependency on `sley-test-runner`, and that crate depends on `sley-vm`.
  - The rule audit's allowlists at `check_cli_rules.py:15-29` admit `sley-test-runner` and `sley-vm`.
- Neither surface appears in any CLI-lane transcript. The only post-0c655fc1 CLI transcript (vulcan closure at 178873d) does not mention either.

## Findings

[P2] [contract-conformance, pre-existing] docs/spec/SLEY_CLI_V1.md:38-60,110-112,159-179,363-365; crates/sley-cli/src/lib.rs:35-48,197-215,493-563,854-905; scripts/check_cli_contract.py:58,88,93,101-103 - The binary accepts `--protocol-profile v3-capable` and `--expected-version 3`, offers versions [1,2,3] (`Server::offered_hello_v3`, server.rs:645), prints the v3 method table, emits `sley2-cli-v3` metadata and a `sley2-cli-report-v3` report with selected version 3, and stamps post-hello frames at 3 (cargo test cli 43/43 pass, including `v3_version_reports_the_v3_contract` at cli.rs:1323-1328). The contract says the profile "takes exactly `v2-capable`; any other value is `CLI_USAGE_INVALID`" (:57-58), allows expected versions 1|2 only, stamps "(1 or 2)" (:111), and reports "1 | 2 | null" (:178). No docs/spec text defines the v3 CLI profile or its two contract identifiers (NATIVE_TEST_ADMISSION_V1.md:541 says only "CLI is thin SMP routing"), yet this package's checker requires the v3 crate markers. SMP1 revision 14 (SMP1.md:61-73,222-226,257,723-725) now makes version 3 a normative selection whose 201 answer is `open_summary`, and revision 9 cites "every later selection", while CLI clauses still forbid the only CLI path that reaches it - closure: a CLI revision that states the v3-capable profile (or delegates it by exact section to an owning contract): the [1,2,3] offer, the v3 table, the `sley2-cli-v3` and `sley2-cli-report-v3` shapes, and the expected-version-3 and stamping rules. It should correct :57-58, :111, :178 and §9, and add spec markers that check_cli_contract.py anchors.

[P2] [contract-conformance, pre-existing] docs/spec/SLEY_CLI_V1.md:24,54-56,200-204,214-216; crates/sley-cli/src/lib.rs:404,444-455; crates/sley-cli/Cargo.toml:17,24-25; scripts/check_cli_rules.py:15-29 - `sley __native-test-worker` is a command the contract does not list, and §1 makes unknown commands `CLI_USAGE_INVALID`. It runs `sley_test_runner::worker::run_stdio` and writes raw refusal words (a tag plus an ASCII symbol) to stdout. That output is not a frame, a report, or the version object (§1:56, §5:214-216). The crate has a normal dependency on `sley-test-runner`, which depends on `sley-vm` (a kernel crate §5 names), and dev-dependencies on `sley-test-runner` and `sley-vm`. §5:202-204 allows only `sley-protocol`, `sley-json-bridge` and `serde_json`, plus `sley-repo`, `sley-id` and `sley-scb1` for fixtures. The audit that §5:200 says "fails closed when any of these drifts" passes only because its allowlists were widened; its own output lists `sley-test-runner`. Commits 6867e6a9 and 955fcdc6 made these changes with no CLI revision, and no CLI-lane transcript records them - closure: either (a) a CLI revision that states the private worker entry and its boundary exception (the dependency, the stdout form, and why a VM-linked worker is consistent with "a transport endpoint and nothing else"), with §5 and check_cli_rules.py agreeing and a test pinning the exception; or (b) move the worker out of `sley-cli` and restore the audit allowlists to the §5 text.

[P3] [re-pin-completeness] docs/adr/ADR-0035-thin-cli-boundary.md:3-22; scripts/check_cli_contract.py:64-76 - ADR-0035 still says "the S20-430 contract is a draft at revision 8" and has no revision-9 record. Every earlier revision had a record, including the pin-only revisions 7 and 8 (SLEY_CLI_V1.md:357-358). The same repair commit 8357c243 added the sibling session profile's revision-5 record to ADR-0033. ADR_MARKERS stop at "Revision 8 record", so the checker cannot see the gap - closure: a dated revision-9 record and a revision-9 status line in ADR-0035, plus a checker ADR marker for it.

[P4] [precision] docs/spec/SLEY_CLI_V1.md:18 - The only new text in revision 9 paraphrases SMP1 revision 14's scope as "under version 2 and every later selection". That drops SMP1's qualifier "whose method table includes version 2's row 201" (SMP1.md:63-64,682,723-724); ADR-0032 keeps the qualifier. The paraphrase is true today only because version 3 is the one later selection. The sentence is also a 249-character unwrapped prose line - closure: carry the qualifier and wrap the line.

## Assessment

The revision-9 delta does what it says. The spec, checker, machine summary and work-package row consistently re-pin SMP1 revision 14 and bridge revision 11. The authority sentence is now anchored, with negative tests. The status left COMPLETE, and the current-delta review is bound to revision 9 as PENDING.

Nothing about `workspace.open` makes an existing CLI clause false:
- the CLI passes 201 bodies through as opaque bytes (hex in JSON mode);
- the report decodes only failure bodies;
- the section 6 byte-identity evidence runs at version 1, where 201 is cache-independent;
- the one version-1 change (a non-empty 201 body is now refused) is the server's judgment and is disclosed.

I cannot accept revision 9 as the governing text for this package, though. The contract is contradicted by its own implementation and checkers in two places:
- The v3-capable profile. SMP1 revision 14 now makes version 3 normative, and this profile is the only CLI route to a version-3 `open_summary`.
- The worker entry, with its kernel-linked dependency and a widened rule audit.

Both date from 2026-09-16/17. They landed while the package was COMPLETE at revision 8 with no CLI revision, and no CLI lane has reviewed them. Separately, ADR-0035 was not re-pinned. This review makes no claim about GA or release readiness.

VERDICT: REVISE_0_P0_0_P1_2_P2_1_P3_1_P4
SECTION: cli
FIELD: ariadne_contract_review_revision_9
SCOPE_SHA: 2b0f1c9f4940c020565137891a49a3769cd02061
FINDINGS: [P2] [contract-conformance, pre-existing] docs/spec/SLEY_CLI_V1.md:38-60,110-112,159-179,363-365; crates/sley-cli/src/lib.rs:35-48,197-215,493-563,854-905; scripts/check_cli_contract.py:58,88,93,101-103 - the binary accepts --protocol-profile v3-capable and --expected-version 3, offers [1,2,3], prints the v3 table, emits sley2-cli-v3/sley2-cli-report-v3 with selected version 3 and stamps frames at 3 (cargo test cli 43/43, cli.rs:1323-1328), while the contract says the profile "takes exactly v2-capable; any other value is CLI_USAGE_INVALID" (:57-58), expected 1|2, stamping "(1 or 2)" (:111) and report 1|2|null (:178); no spec defines the v3 CLI profile, yet the checker requires its markers; SMP1 r14 now makes v3 (with 201 open_summary) normative - closure: CLI revision stating or exactly delegating the v3-capable surface, correcting :57-58/:111/:178/§9, with anchored spec markers | [P2] [contract-conformance, pre-existing] docs/spec/SLEY_CLI_V1.md:24,54-56,200-204,214-216; crates/sley-cli/src/lib.rs:404,444-455; crates/sley-cli/Cargo.toml:17,24-25; scripts/check_cli_rules.py:15-29 - unlisted `__native-test-worker` command writes raw refusal words to stdout; the crate depends on sley-test-runner (-> sley-vm) and dev-depends on sley-vm contrary to §5:202-204; the §5 "fails closed" audit passes only through widened allowlists; landed in 6867e6a9/955fcdc6 with no CLI revision or lane review - closure: CLI revision stating the worker boundary exception with §5 and the audit agreeing plus a pinning test, or move the worker out and restore the allowlists | [P3] [re-pin-completeness] docs/adr/ADR-0035-thin-cli-boundary.md:3-22; scripts/check_cli_contract.py:64-76 - ADR-0035 still reads "draft at revision 8" with no revision-9 record, although revisions 7/8 have records and 8357c243 updated ADR-0033 for the sibling re-pin; the checker's ADR markers stop at revision 8 - closure: dated revision-9 record and status line plus a checker marker | [P4] [precision] docs/spec/SLEY_CLI_V1.md:18 - "under version 2 and every later selection" drops SMP1's "whose method table includes version 2's row 201" qualifier (SMP1.md:63-64,682,723-724); 249-character unwrapped line - closure: carry the qualifier and wrap
SUMMARY: Revision 9's re-pin to SMP1 revision 14 and bridge revision 11 is complete and consistent across the spec, the anchored checker (exit 0, 10/10 tests), the machine summary and WORK_PACKAGES. The opaque-bytes claim for 201 holds in code: no method arms, hex bodies, only failure bodies decoded, CLI and bridge crates unchanged, and the section 6 evidence runs at version 1. Acceptance is blocked by two pre-existing, never-reviewed contradictions between the contract and the implementation: the undocumented v3-capable profile, which SMP1 revision 14 now makes normatively reachable, and the __native-test-worker entry, whose kernel-linked dependency passes the §5 audit only through widened allowlists. ADR-0035 also still states revision 8.
