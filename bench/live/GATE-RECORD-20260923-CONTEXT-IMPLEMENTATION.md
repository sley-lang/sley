# CONTEXT bounded discovery implementation (REQ-10) — 2026-09-23 (work branch only)

Scope: Sley only. No ZJX execution-authority change. No mint, signing,
publication, integration, or GA claim. `ga_claimed=false` preserved. No
review was dispatched and no verdict was written: every lane field for the
revision 5 surface is still unwritten, and nothing here is implementation
acceptance.

## 0. State and inputs

- Worktree `wt-succ-context`, branch `work/succ-context-impl`, base
  `ab42a3a9` (`work/succession-sley20-arm`, REQ-10 rev9 verdict filed).
  Implementation commits: `c2e37384` (Rust: probe + field 9) and `44f18e2b`
  (trial-runner revision 5, tool/gateway/tooling, judge continuation fix,
  CONTEXT discovery proofs). This record lands in the commit after them.
  Every gate below ran on `44f18e2b` with a clean tracked tree.
- Design implemented: REQ-10 rev4 architecture as settled through rev9
  (packets `evidence/review/requests/REQ-10-rev{4..9}-*.md`) plus every
  remedy the rev9 verdict names
  (`nabu_architecture_review-rev9-95d42fda.md`,
  `REVISE_0_P0_1_P1_1_P2_3_P3_2_P4`).
- Toolchain `1.93.0`, `--locked --offline`,
  `CARGO_TARGET_DIR=/home/gfarch/Work/checkpoints/target-succ-context`,
  `SLEY2_MASTER_GOAL=.../greyforge-managed-home/machineresearch/Sley2.0mastergoal.md`,
  `UV_OFFLINE=true`; long Python suites ran with
  `TMPDIR=/home/gfarch/Work/checkpoints/succ-context-tmp` after one `/tmp`
  ENOSPC (section 5). Binaries: `sley` and
  `succ_live_judge_cases-42d77a02eb446c77` built from these sources.

## 1. Change list (file:line at `44f18e2b`)

Rust (`c2e37384`):
- `crates/sley-repo/src/index_cache.rs:256` `cached_complete_root_snapshot_id`:
  the Merlin-owned (S20-300) metadata-only probe — `read_record` +
  `accept_cached`, no `fresh_snapshot` fallthrough, no write-back; absent /
  unreadable / non-file / discarded records are `None`; only a guard naming
  another repository is an error. Test `:569`
  `probe_reports_only_a_materialized_snapshot_and_never_builds` (cold `None`
  and no file written; `Some(id)` after the query-path build; hit survives
  object-store removal; a discarded record is `None` with no rewrite); guard
  refusal added to `guard_for_another_repository_is_refused`.
- `crates/sley-protocol/src/server.rs:2899` `workspace_open` → `:2908`
  `materialized_head_snapshot` (fail-open like the cache, contract section 5)
  → `:3274` `head_open_summary` appends field 9 only on this path;
  `:3285` `revision_summary_fields` is the shared eight-field list and
  `revision_summary` (used by `revision_read`) re-encodes exactly it, so
  `revision.read` bytes are unchanged. Absence is structural; `counted(.., 1)`
  unchanged, so no `omitted`/`truncated` signal exists on this route.
- `crates/sley-protocol/src/server_tests.rs:619`
  `workspace_open_discloses_only_the_materialized_head_snapshot`: cold body ==
  `revision.read` body, omitted 0, not truncated, no cache file; an unbound
  `query.root` is refused `QUERY_SNAPSHOT_MISMATCH` and materializes the
  snapshot; warm body = the eight fields byte-for-byte + `[9, 32, id]` with
  `id == request.snapshot_id()`; `revision.read` byte-identical after; the
  preimage rebuilt from the disclosed id is answered and equals the engine
  response.

Trial runner, checker, spec, records (`44f18e2b`):
- `bench/sley2/runner.py:114` `workspace.open` appended last to
  `ARM_AFFORDANCES` and removed from `ARM_DENIED_METHODS` (move, not copy);
  `:116-123` denial rationale rewritten by risk; nineteen at `:153`, `:451`,
  `:936`, `:941`.
- `bench/sley2/tests/test_runner.py:335` docstring nineteen; `:384` the
  version 1 offer literal `16 → 17` (`seventeen`, a coupled count the
  `eighteen` grep could not see); `:550` `ArmAffordanceTests` docstring;
  `:586` positive admission assertions folded into the existing coverage test
  (test count stays 23).
- `scripts/check_sley2_trial_runner.py:134-135` docstring and anchor
  `holds nineteen names`; `:182-186` Fix 1: one anchored
  (`^Status: S20-620 contract draft, revision (\d+)`, `re.M`), converted
  extraction after `status = section.get("status")` and before the
  `IMPLEMENTATION_STATUSES` block, plus the drift assertion
  `machine-summary:contract_revision:{summary!r}!=spec:{spec!r}`;
  `:243-244` delta-pin comment revision 5; `:265` count `!= 19`; `:298-301`
  Fix 2 `completion-unbound-review:{key}` on `f"{key}_revision_{spec_revision}"`;
  `:306` payload reads the hoisted `spec_revision` (no second extraction).
- `docs/spec/SLEY2_TRIAL_RUNNER_V1.md:3` Status revision 5 (2026-09-23);
  `:11-15` revision 5 sentence; `:40` section 1 nineteen; `:296-303` section 9
  list with `workspace.open` last and `holds nineteen names`; `:309` new
  bullet: the accepted-head opener, field 9 on the `workspace.open` path only,
  metadata-only probe, builds stay on the query path, and the field-9
  omission constraint (structural absence, never `bounds.omitted` or
  `bounds.truncated`; cold path = open, refused bounded query that
  materializes, open again).
- Terminator literal decision (rev9 P3): KEPT VERBATIM. `Those two entity
  names` is unchanged; the antecedent is clarified by a following parenthetical
  without backticks (`(the revision 4 entity reads above, not the revision 5
  opener)`), so `_spec_allowlist` still ends its region at the literal and
  extracts exactly nineteen names (checker PASS; A4 cases below).
- `machineresearch/sley-2.0/machine-summary.json:4330` status
  `S20_620_IMPLEMENTED_REVIEW_PENDING`; `:4332` `contract_revision` 5; `:4350`
  `implementation_complete` false; `:4373` `status_note` rewritten (states
  what the WORK_PACKAGES row and closeout now read). Base review fields keep
  `PASS_0_P0_0_P1_0_P2_0_P3` byte-exact. No `_revision_4` fields and no
  `_revision_5` fields were written (section 7).
- `docs/WORK_PACKAGES.md:59` S20-620 row: revision 5, status
  `S20_620_IMPLEMENTED_REVIEW_PENDING`, revision 5 lane reviews pending;
  revision 4 facts kept as a labelled history clause.
- `docs/audits/S20_620_SLEY2_TRIAL_RUNNER_CLOSEOUT.md:3` new Revision 5
  status paragraph; `:18` revision 4 paragraph now past tense. Frozen history
  (Revision 4 attestation, revision 2 record) untouched.

Agent-facing tool, gateway, shim, tooling:
- `bench/live/sley2_tool.py:10-13` false "pinned ... by test" clause struck and
  replaced by the named pin test; `:25-28` module docstring lists `open`,
  `revision <tx-hex>`, `side ours|theirs` (rev9 P4); `:88-93` comment;
  `:113` `TOOL_METHODS` gains `workspace.open` (edit 4.0, nineteen, runner
  order); `:119` `SUMMARY_METHODS` + `:427` `_summary_view` (framing-only
  display of revision summaries; `snapshot` key only when field 9 is present);
  `:760` `open` command → guarded `_view(session, "workspace.open", "")`;
  `:765` `revision` takes exactly one 64-hex tx (`_hex(rest[0], 64)`), never
  reading `session.head`; `:1078-1081` `_command_evidence` binds
  `{"opened": True}` / `{"tx": ...}`. Harness `_read_head` and its six
  consumers unchanged.
- `bench/live/mediated_sley.py:49` edit 4.1 `open` in `ALLOWED_COMMANDS`;
  `:220` capture-vocabulary note (`raw:denied` → `raw:workspace.open`, new
  label `open`, neither a bounded paging route).
- `bench/live/mediated_shim.py:24` `open` in `READ_COMMANDS`.
- `bench/live/tooling.py:87-89` edit 4.2: fused `sig`/`revision` line fixed,
  `open` and `revision TX_HEX` lines; `:107-112` the `open`/`revision`
  sentence.

Callers, witnesses, judge, docs:
- `bench/live/mediated_client.py:98` `open_head` (open, then `revision <tx>`
  of the reported tx) replaces every no-argument `revision` call;
  `:258`/`:289` agent-side root-query preimage encoder and response decoder;
  `:426` `context_discover_and_repair`; `:631` `seq_context`; `:1104` the
  `context MODE` sequence (the old identities-as-arguments `context_pos` is
  removed).
- `bench/live/succ_witness_context.py:80` manifest reads (old `:65-67`)
  replaced by interface discovery; `succ_witness_sig.py:141`,
  `succ_witness_type_full.py:167` open + `revision <tx>`.
- `bench/fixtures/sley2_live_judge.py:3165` (direct audit) and `:3474`
  (mediated audit): a `query.continue` page keeps the chain open iff it is
  itself truncated (section 7, new finding).
- `bench/live/MEDIATED-INPUT-ACCOUNTING.md` rows 4 and 7 and the CONTEXT
  task-spec gate bullet; `bench/live/SUCCESSION-COVERAGE.md` CONTEXT row
  blocker text; `bench/live/mediated_attempt.py` staging comment.

Tests (new or changed): `bench/live/tests/test_sley2_tool.py`
(`Sley2ToolSurfacePinTests.test_tool_methods_equal_runner_allowlist_in_order`,
`test_dispatch_arity_refuses_before_any_server_contact`,
`test_open_and_raw_workspace_open_are_guarded_agent_reads`, revision callers),
`test_tooling.py` (`Sley2CommandSurfacePinTests`: ALLOWED_COMMANDS ==
documented commands by line-start match, argument forms, shim phases, module
docstring), `test_mediated_gateway.py`
(`test_revision_without_tx_is_a_recorded_refusal` + callers),
`test_agent_access.py` and `test_mediated_access.py` (multi-page chain
accepted; truncated continuation without follow rejected), `test_mediated_attempt.py`
(exchange counts 2→3, 5→7 for open + revision), `test_mediated_context.py`
(seven integrated proofs, section 4).

## 2. Mechanical sweep (rev9 SUMMARY request)

### 2a. Every agent command name, across every enumeration (at `44f18e2b`)

| command | `sley2_tool.dispatch` | `_command_evidence` | `ALLOWED_COMMANDS` | shim phase | TOOLING.md line | module docstring | stand-in uses | disposition |
|---|---|---|---|---|---|---|---|---|
| `inventory` | yes | yes | yes | read | yes | yes | yes (TYPE/STALE) | unchanged |
| `side` | yes | yes | yes | read | yes | yes (added) | — | docstring line added |
| `read` | yes | yes | yes | read | yes | yes | yes | unchanged |
| `sig` | yes | yes | yes | read | yes (line unfused) | yes | — | TOOLING line split from `revision` |
| `open` | yes (new) | yes (new) | yes (edit 4.1) | read (added) | yes (edit 4.2) | yes | yes | ADDED everywhere |
| `revision` | yes, 1 tx arg | yes (`tx`) | yes | read | `revision TX_HEX` | `revision <tx-hex>` | yes (after `open`) | ARITY CHANGED everywhere |
| `caps` | yes | — (no args) | yes | read | yes | yes (`caps \| budgets`) | yes | unchanged |
| `budgets` | yes | — (no args) | yes | read | yes | yes (`caps \| budgets`) | — | unchanged |
| `raw` | yes (19 methods) | yes | yes | read | yes | yes | yes | method set widened by `workspace.open` |
| `inspect` | yes | — | yes | read | yes | yes | — | unchanged |
| `validate` | yes | — | yes | compose | yes | yes | — | unchanged |
| `append` | yes | yes | yes | compose | yes | yes | — | unchanged |
| `compose` | yes | yes | yes | compose | yes | yes | yes | unchanged |
| `propose` | yes | yes | yes | compose | yes | yes | yes | unchanged |
| `finish` | yes | yes | yes | finish | yes | yes | yes | unchanged |
| `resolve` | — (gateway-local) | — | gateway-only (`GATEWAY_COMMANDS`) | compose | — | — | yes (TYPE) | unchanged, not a tool command |
| `commit` | — | — | — (denied) | default | — | — | yes (refusal probes) | unchanged, denied by design |

Method-level enumerations (nineteen names): `ARM_AFFORDANCES`
(`bench/sley2/runner.py:95-115`), `TOOL_METHODS` (`sley2_tool.py:94-114`),
spec section 9 list (`SLEY2_TRIAL_RUNNER_V1.md:296-302`), checker count
(`check_sley2_trial_runner.py:265`), checker anchor (`:135`) and terminator
(`:139`, literal kept), version 1 offer literal (`test_runner.py:384`, 17),
denied tuple (`runner.py:124-149`, 24 names). Now mechanical:
`TOOL_METHODS == ARM_AFFORDANCES` (new pin test), `ARM_AFFORDANCES ==` spec
order and `len == 19` (checker), allowlist ∪ denylist == SMP1 v2 table
(existing test), `ALLOWED_COMMANDS ==` TOOLING command lines and ⊆ shim
phases (new pin tests), module docstring lists `open` / `revision <tx-hex>`
(new test). Still editorial: `_command_evidence` cases and the stand-in.

### 2b. Every `"revision"` call site at base `ab42a3a9`, with disposition

| base site | kind | disposition at `44f18e2b` |
|---|---|---|
| `bench/live/sley2_tool.py:717` | dispatcher (no-arg, harness `session.head["tx"]`) | argument form `:765`, `_hex(rest[0], 64)`; no `session.head` read |
| `bench/live/sley2_tool.py:24` | module docstring | `revision <tx-hex>` + `open` (`:25-28`) |
| `bench/live/tooling.py:87` | TOOLING.md (fused with `sig`) | split; `revision TX_HEX` (`:89`) |
| `bench/live/mediated_sley.py:49`, `mediated_shim.py:24` | command sets | name kept; `open` added beside it |
| `bench/live/mediated_client.py:113` (`discover_type_roles`) | adapter | `open_head` (open + `revision <tx>`) |
| `mediated_client.py:298` (old `seq_context`) | adapter | sequence replaced by `context_discover_and_repair` (uses `open_head`) |
| `mediated_client.py:381` (`_finish_skeleton`) | adapter | `open_head` |
| `mediated_client.py:438` (`seq_type`), `:595` (`seq_stale`) | adapter | `open_head(step)` |
| `mediated_client.py:727` (`refusal_probe`) | adapter | `open_head`; first read = open + revision |
| `mediated_client.py:750` (`two_phase`) | adapter | `open_head(first)`; exchange count 2 → 3 (`test_mediated_attempt.py`) |
| `mediated_client.py:760` (`denied_then_finish`), `:771` (`no_finish`) | adapter | `open_head`; `denied_then_finish` count 5 → 7 |
| `bench/live/succ_witness_sig.py:139` | witness | open + `revision <tx>` (`:141-142`); re-run ACCEPTED |
| `bench/live/succ_witness_type_full.py:165` | witness | open + `revision <tx>` (`:167-168`); re-run ACCEPTED |
| `bench/live/tests/test_sley2_tool.py:68,72` | test (pack/binary refusal) | `open` + a success control, so the refusals are the pack/binary ones |
| `test_sley2_tool.py:92` | test | open, `revision <tx>`, and field equality (no `snapshot` on revision) |
| `test_sley2_tool.py:177,186` | test (head unmoved) | `open` body before/after |
| `test_sley2_tool.py:236,259,277` | test (ledger/chain) | `open` |
| `bench/live/tests/test_agent_access.py:67,88` | test | `open` |
| `test_agent_access.py:100,101` | test | `open` then `revision <tx>` |
| `bench/live/tests/test_mediated_gateway.py:62` | test (not in the rev9 list) | `open` then `revision <tx>`; new `test_revision_without_tx_is_a_recorded_refusal` |
| `test_mediated_gateway.py:120,128,137,146,154` | test (not in the rev9 list; adjudication only needs an exchange) | `open` |
| `bench/live/capture_demo.py:373`, `tests/test_trusted_capture.py:62`, `tests/test_mediated_access.py:136`, `tests/test_acceptance_repairs.py:537,538,552` | synthetic capture/chain labels, never dispatched | unchanged (label strings only) |
| `bench/sley2/runner.py:1408`, `bench/sley2/entity_read_edit_demo.py:183`, judge `sley2_live_judge.py:3613` | server method `revision.read` with an explicit tx (not the tool command) | unchanged; `revision.read` bytes unchanged |

Negative tests for the no-argument form: `test_sley2_tool.py`
`test_dispatch_arity_refuses_before_any_server_contact` (no argument, 31/33
bytes, non-hex, two arguments, `open 00` — refused with no server contact and
no harness head read), `test_mediated_gateway.py`
`test_revision_without_tx_is_a_recorded_refusal`, and the integrated
`test_context_no_argument_revision_is_refused_and_counted`.

## 3. Tests and gates (all at `44f18e2b` unless noted)

| Check | Command | Exit | Result |
|---|---|---|---|
| sley-repo probe tests | `cargo test --locked --offline -p sley-repo --lib index_cache` | 0 | 9 passed |
| server field-9 test | `cargo test --locked --offline -p sley-protocol --lib workspace_open` | 0 | 1 passed |
| touched crates | `cargo test --locked --offline --no-fail-fast -p sley-repo -p sley-protocol -p sley-cli` | 101 | 639 passed, 1 failed, 25 ignored; the one failure is `succ_debug_commit::debug_commit_repro` (panics on missing `SUCC_DEBUG_REPO`; documented diagnostic-only disposition) |
| formatting | `cargo fmt --all --check` | 0 | clean |
| trial-runner tests | `python3 -m unittest discover -s bench/sley2/tests -t .` | 0 | Ran 23 tests, OK |
| trial-runner checker | `python3 scripts/check_sley2_trial_runner.py` | 0 | PASS, revision 5, status `S20_620_IMPLEMENTED_REVIEW_PENDING`, problems [] |
| live suites | `python3 -m unittest discover -s bench/live/tests -t .` (binaries bound, bwrap) | 1 | Ran 214 tests, 1 error: `test_context_no_argument_revision_is_refused_and_counted` hit `OSError: [Errno 28] No space left on device` in `/tmp` (tmpfs); rerun alone with `TMPDIR` on `/home`: Ran 1, OK. No other failure |
| integrated CONTEXT proofs | `python3 -m unittest -v bench.live.tests.test_mediated_context` | 0 | Ran 7 tests in 1010.8s, OK |
| runner smoke over the real endpoint | `python3 -m bench.sley2.runner smoke --sley <target>/debug/sley --evidence-dir <scratch>` | 0 | PASS; `affordances` 19; all 8 checks true |
| lint | `make lint` (pinned env) | 0 | PASS: fmt clean, 0 clippy warnings, commit `44f18e2b`, clean tree; tracked `evidence/build/lint-report.json` restored with `git checkout` and not committed |
| quick | recipe driven line by line, keep-going (130 lines) | — | 116 exit 0, 14 fail (section 5) |
| witnesses | `bench/live/succ_witness_{context pos,context neg,sig pos,type_full pos}` | 0 | logs `bench/live/succ-trials-20260923/`: CONTEXT pos ACCEPTED (whole_store_reads=0, refusals=1 = the warm query), CONTEXT neg refused at validation phase 6 (tag 9), SIG ACCEPTED, TYPE-FULL ACCEPTED |

## 4. Integrated proof (execute_attempt, scripted stand-in only)

`bench/live/tests/test_mediated_context.py` drives the real confinement,
gateway, endpoint, trusted capture, fixture oracle, append, and
`verify_attempts`; the stand-in command is `mediated_client.py context MODE`
with the mode as its only argument (no entity id, member literal, or manifest
value; the old `succ_witness_context.py:65-67` manifest reads are gone).
Positive path, as observed: `open` (no field 9: cold) → `revision <tx>` →
bounded `query.root` refused `QUERY_SNAPSHOT_MISMATCH` (materializes) →
`open` (field 9) → `revision <tx>` → class 4 over type definitions (1 page, 1
typedef) → read (Record form) → class 14 from the typedef in 3-entity pages:
`query.root` + three `query.continue` (total 10) → class-2 kind probes → read
the three constants of the record type → one propose (typedef + 3 constants,
Valid) → finish → real oracle ACCEPTED → `VERIFIED_LIVE_EVIDENCE`.

| Test | Mode | Outcome asserted |
|---|---|---|
| `test_context_discovery_and_repair_accepted_end_to_end` | pos | accepted; verified; cold→warm binding; 3 continuations; 3 impacted constants; no `inventory`/`side` |
| `test_context_incomplete_discovery_rejects` | stop_early (7-entity page, not continued) | finished with a complete repair, yet rejected `QUERY_REQUIRED_FACT_OMITTED` "truncated page without continuation" |
| `test_context_incomplete_impact_cannot_finish` | incomplete (typedef only) | production validation refuses; `harness_failure` `CAPTURE_GATE_NO_FINAL` |
| `test_context_inconsistent_continuation_rejects` | extra_continue | rejected "inconsistent continuation" |
| `test_context_exceeded_budget_rejects` | overbudget (7 full constant listings) | rejected "cumulative ... over budget" (each response under the per-response bound) |
| `test_context_missing_evidence_never_accepts` | pos, capture exchanges lost before the oracle | judge rejected "mediated exchanges missing"; attempt `harness_failure` with a `CAPTURE_*` code |
| `test_context_no_argument_revision_is_refused_and_counted` | noarg_revision | first exchange `revision` with no argument is a captured failed response; attempt accepted |

Deterministic stand-in only; none of this is a model trial or campaign
evidence.

## 5. `make quick` (130 recipe lines, driven keep-going at `44f18e2b`)

`make -k quick` alone stops the single `quick` recipe at its first failing
line (`-k` continues other targets, not later lines of one recipe), so the
recipe was driven line by line exactly as the Makefile lists it, with the
pinned environment and `TMPDIR` on `/home`. 116 lines exit 0; tracked tree
clean afterwards. Failing lines:

| # | Line | Exit | Cause | Class |
|---|---|---|---|---|
| 42 | `check_merge_persistent_fuzz_slice.py` | 1 | `proof-record-predates-lane-change` | affected fuzz lane: the lane paths include the changed `sley-repo`/`sley-protocol` sources; needs a fuzz refresh at this source (not run here) |
| 43 | `check_semantic_delta_persistent_fuzz_slice.py` | 1 | same | same |
| 48 | `check_smp1_persistent_fuzz_slice.py` | 1 | same | same |
| 49 | `check_smp1_json_bridge_persistent_fuzz_slice.py` | 1 | same | same |
| 109 | `check_pack_persistent_fuzz_slice.py` | 1 | same | same |
| 114 | `check_exchange_persistent_fuzz_slice.py` | 1 | same | same |
| 115 | `check_merge_judgment_fuzz_slice.py` | 1 | same | same |
| 83 | `build_candidate_content_report.py --check` | 1 | `PACKAGE_MANIFEST_INVALID`: `evidence/runtime/s20-720-release-candidate/evidence.json` absent (fresh tree; passes only with the preserved candidate bytes restored, per the 2026-09-21 record) | candidate-bound, expected |
| 84 | `check_reproducibility_and_independent_conformance.py` | 1 | `reproducibility-report:stale:69907ddf904a:6-surface-files-changed` (now naming `server.rs`, `server_tests.rs`, `index_cache.rs` beside the three earlier succession test files) | candidate-bound, expected |
| 85 | `check_standards_sbom_and_provenance.py` | 1 | `closure:unverifiable`, `sbom:drift`, `provenance:drift` | candidate-bound, expected |
| 86 | `check_finding_register.py` | 1 | `finding-register:obligations-drift`, `obligations-digest`, `drift`. Derived drift caused by this change: exactly three obligations differ — the `sley2_trial_runner` ariadne/nabu/vulcan rows change `package_status` `S20_620_COMPLETE` → `S20_620_IMPLEMENTED_REVIEW_PENDING` (dispositions and states unchanged). | NEW, caused here; `evidence/review/finding-register.json` NOT regenerated (section 7) |
| 88 | `check_decision_dossier.py` | 1 | `test-inventory:drift` | candidate-bound, expected (pre-existing) |
| 105 | `check_supply_chain_audit.py` | 1 | secret-scan counters drift (`evidence/security/T54/secret-scan.json`) | candidate-bound, expected (pre-existing) |
| 130 | `cargo test --workspace --locked` | 101 | 28 suites; 1025 passed, 1 failed, 31 ignored; the failure is `succ_debug_commit::debug_commit_repro` only | documented diagnostic-only disposition (pre-existing) |

#129 `cargo check --workspace --locked` exit 0; #128 `git diff --check` exit 0;
#80 `check_sley2_trial_runner.py` exit 0.

## 6. Fix 1 A4 two-direction demonstration (scratch copies outside the repo)

Script: `git archive 44f18e2b scripts docs machineresearch bench conformance`
into four scratch directories under the session scratchpad, one summary edit
each, then `python3 scripts/check_sley2_trial_runner.py` in each. The PASS
values written into the `bound` copy exist only in that scratch copy to
exercise the gate; no lane verdict exists and none was written to the tree.

```
source commit: 44f18e2bc08ad64e5eccd28125a4c16eb5f6f5ea
--- case drift: summary contract_revision 4 vs spec revision 5 (must emit machine-summary:contract_revision:4!=spec:5)
{"result": "FAIL", "revision": 5, "status": "S20_620_IMPLEMENTED_REVIEW_PENDING", "problems": ["machine-summary:contract_revision:4!=spec:5"]}
exit 1
--- case aligned: summary 5 vs spec 5, status REVIEW_PENDING (must emit neither code)
{"result": "PASS", "revision": 5, "status": "S20_620_IMPLEMENTED_REVIEW_PENDING", "problems": []}
exit 0
--- case unbound: status COMPLETE, base PASS fields, no _revision_5 fields (must emit completion-unbound-review x3)
{"result": "FAIL", "revision": 5, "status": "S20_620_COMPLETE", "problems": ["completion-unbound-review:ariadne_contract_review", "completion-unbound-review:nabu_architecture_review", "completion-unbound-review:vulcan_surface_review"]}
exit 1
--- case bound: status COMPLETE with PASS _revision_5 fields (must emit neither code)
{"result": "PASS", "revision": 5, "status": "S20_620_COMPLETE", "problems": []}
exit 0
```

## 7. Deviations from the packets, decisions, and new findings

1. No `_revision_4` fields. The brief mentions "revision_4 fields ... per
   Fix 2.1", but rev7 Fix 2.1 was superseded by rev8 Fix 3 (binding starts at
   revision 5; the rev4 lane verdicts have no transcript) and the rev8
   verdict ratified that trade. Followed rev8: base fields byte-exact, no
   `_revision_4`/`_revision_5` fields; status `REVIEW_PENDING`; COMPLETE only
   after real revision-5-bound PASS reviews with `on <40-hex>` notes.
2. No corpus task-input amendment. The corpus is frozen and digest-pinned
   (`check_benchmark_fixtures.py` corpus_sha256), so the "name the target
   typedef" amendment of the settled design was not made. The stand-in
   instead selects the store's unique Record-form type definition by a
   bounded class-4 listing — task intent ("add a required record field"),
   not a supplied identity. In a store with several record typedefs this
   selection is ambiguous; the amendment remains a governance item.
3. Judge change beyond the packets (new finding, needs Vulcan review): both
   access audits set the continuation-pending flag to `False` after every
   `query.continue`, so (a) any chain longer than two pages was rejected as
   "inconsistent continuation" (the positive proof failed on this), and (b)
   stopping on a truncated continuation page was accepted — a hidden
   truncation hole. Now a continuation page keeps the chain open iff it is
   itself truncated (`omitted` on a continuation page counts entities earlier
   pages returned — server `total_count - returned` — so it is not a "more"
   signal). The four new regressions fail on the base judge (2 errors: chain
   rejected; 2 failures: truncated stop accepted) and pass now. This tightens
   (b) and removes a false rejection; no other audit rule changed.
4. The direct (unmediated) audit tracks continuation per tool invocation
   (one command per invocation), so paged discovery across invocations cannot
   pass it; the direct witness reads the closure in one untruncated 16-entity
   page. Paged discovery is proved on the mediated route. Not changed.
5. `docs/spec/SMP1.md` row 201 (`workspace.open` response =
   `revision_summary` of the accepted head) was not edited: a normative SMP1
   profile text change is the deferred Ariadne composition lane (rev4:
   "Ariadne receives only the resulting profile text"). The trial-runner spec
   section 9 states the field-9 behavior.
6. Probe failure mode: `materialized_head_snapshot` is fail-open (maintenance
   or probe trouble → field 9 absent), matching the cache contract's
   fail-open rule; `workspace.open` gains no new failure.
7. `revision` / `open` views are decoded (framing-only `_summary_view`);
   previously `revision` returned `decoded: null`. Display only.
8. Pin placement: the TOOL_METHODS pin lives in a binary-free class in
   `test_sley2_tool.py` (the tool tests skip without a binary). The TOOLING pin
   matches command lines at line start and requires set equality, which is
   stricter than the substring check the verdict described; on the base tree
   it fails for `revision`, which the fused line hid from a line-start match.
9. Additional coupled sites found beyond the rev9 inventory:
   `test_runner.py:384` (16-name version 1 literal),
   `test_mediated_gateway.py` (six `revision` calls), module docstring `side`.
10. Finding register (#86): not regenerated at `44f18e2b`; regenerated in the
    follow-up (section 10.2, commit `e0146f8d`).

## 8. Widened-token disposition (Fix 3 / C4)

Tokens: `revision 4`, `S20_620_COMPLETE`, `reviews PASS`, `Council reviews
PASS`, `implementation_complete`, `eighteen-method`, `eighteen`
(`git grep -n -F` over tracked files at `44f18e2b`; 585 hit lines before this
record). S20-620-domain hits, each dispositioned:

| Hit | Disposition |
|---|---|
| `docs/WORK_PACKAGES.md:59` (`revision 4`, `Council reviews PASS`, `S20_620_COMPLETE`) | updated: present tense is revision 5 / REVIEW_PENDING; the tokens remain only inside the labelled history clause |
| `machineresearch/sley-2.0/machine-summary.json:4350` (`implementation_complete`) | updated to `false` |
| `machine-summary.json:4373` (`S20_620_COMPLETE`) | updated: status_note names it as the previous status |
| `scripts/check_sley2_trial_runner.py:28` (`S20_620_COMPLETE`) | unchanged live constant (`COMPLETE_STATUS`) |
| `scripts/check_sley2_trial_runner.py:196` (`implementation_complete`) | unchanged live check (derives from status) |
| `docs/spec/SLEY2_TRIAL_RUNNER_V1.md:9` (`eighteen`) | frozen history: the revision 4 sentence of the preamble |
| `docs/spec/SLEY2_TRIAL_RUNNER_V1.md:303` (`revision 4`) | updated: the terminator clarification (names the revision 4 entity reads) |
| `docs/audits/S20_620_SLEY2_TRIAL_RUNNER_CLOSEOUT.md:6,13` | updated: new Revision 5 status paragraph (refers to revision 4 as history) |
| closeout `:18` (`revision 4`, `eighteen`) | updated to past tense ("implemented") |
| closeout `:97`, `:137`, `:139`, `:153`, `:169`, `:178` | frozen history (revision 2 record and Revision 4 attestation) |
| `docs/audits/PHASE3_V2_OFFER_DESIGN.md:148,149,163` | frozen history (revision 4 design record) |

Every other hit is listed in Appendix A by file and line with a rule-based
disposition: review transcripts, gate records, campaign records, and retained
logs are frozen history; the rest are other packages, contracts, sections, or
the eighteen SSMC1 entity kinds. The rule was checked per line: no hit line
classified "unrelated" mentions S20-620, `sley2_trial_runner`, the trial
runner, the allowlist, `ARM_AFFORDANCES`, or `TOOL_METHODS` (the eight lines
that did were all review logs / campaign records / a retained quick log and
are classified frozen history).

## 9. Not done / open

- Revision 5 Ariadne, Nabu (implementation acceptance), and Vulcan reviews:
  not dispatched (out of scope for this pass). Status stays
  `S20_620_IMPLEMENTED_REVIEW_PENDING`; the allowlist widening remains the
  allowlist owner's ratification. Vulcan should also see the judge change
  (section 7.3).
- Fuzz refresh for the seven affected lanes (#42, #43, #48, #49, #109, #114,
  #115): not run.
- Finding-register regeneration (#86): done in the follow-up (section 10.2).
- Corpus task-input amendment: not made (section 7.2).
- TOOLING.md root-query documentation: done in the follow-up (section
  10.1). No live-model trial ran, so live-model usability is documented, not
  demonstrated; the per-invocation continuation scope limits a one-shot
  `sley-tool` user to untruncated pages (section 10.1).
- `docs/spec/SMP1.md` workspace.open row: not updated (section 7.5).
- `raw workspace.open` and `open` are reachable on the direct and mediated
  routes; the harness still performs its own `workspace.open` inside every
  session (bookkeeping, recorded in the session transcript like other
  harness reads, not labelled separately).
- Rev9 P3 (malformed rev8 transcript hash in the committed rev9 packet): a
  packet-authoring item for the next review packet, not an implementation
  item; `sha256sum` of the committed rev8 transcript here is
  `e617e0c1db180d38db658cf288b49804ebd360a21da89dc914e5c8d539b4b13b`,
  matching the rev9 verdict's stated value.

## 10. Follow-up (coordinator request before the revision-5 dispatch)

### 10.1 Agent contract documents the discovery queries (`8db44154`)

- `bench/live/tooling.py` (`SLEY2_TOOLING` → TOOLING.md): the nineteen `raw`
  methods (equal to `TOOL_METHODS`, test-pinned); the `query.root` /
  `query.continue` request layout (magic, versions, the four ids from
  `open`, completeness/limits profile, the five limits with the server's
  accepted ranges from `validate_limits`, paging flag, cursor encoding,
  class bodies for classes 2, 4, 14 with the canonical ascending-seed rule)
  and the response layout (echo, total/returned/truncated/next cursor,
  class results); the snapshot binding and cold-snapshot warm-up; the
  paging=1 refusal; continuation; the per-scope continuation audit; budgets
  (1048576 bytes per reply, the judge and capture per-response bound;
  4194304 bytes per trial, the judge cumulative bound); whole-store routes.
  Derived from the server decoder (`server.rs` `decode_root_query`), the
  engine encoder/paging (`sley-query/src/root_query.rs`), and the judge/
  capture constants, not from the stand-in.
- Stand-in: the cold-snapshot warm query is now a documented class-4 request
  (class 1 removed), so every body it sends uses classes 2/4/14 only.
- `bench/live/tests/test_tooling.py` `RootQueryContractTests` (5 tests): the
  documented block is pinned line for line; an encoder written only to the
  documented layout reproduces the conformance vectors' request identities
  (`blake3("sley2.root-query.v1" + body)` for `class-02`, `class-04`,
  `class-14`, `page-namespaces-1/2`; a one-byte change does not match) and
  decodes the vectors' engine response records (the stand-in's decoder
  agrees field for field); every body the stand-in sends in all six CONTEXT
  modes parses strictly under the documented grammar against a fake server
  that answers only in the documented layout; the documented raw-method list
  equals `TOOL_METHODS`. The ALLOWED_COMMANDS↔TOOLING pin is unaffected (no
  command added; it still passes).
- Finding (documented, not changed): the direct audit resets continuation
  state per tool invocation and the mediated audit keys it by gateway
  session id, which the one-shot shim sets per process
  (`shim-<pid>`). A model driving `sley-tool` one invocation at a time can
  therefore never satisfy a truncated page; TOOLING.md says so and tells it
  to size pages to complete. The stand-in's paged proof uses one gateway
  session (`mediated_transport.Gateway`, staged in production scratch but
  not documented in TOOLING.md). Whether multi-invocation continuation
  should be supported (stable scope id, or a cursor-bound audit) is a
  surface decision for the revision-5 Vulcan/Ariadne lanes.

### 10.2 Finding register chain (`e0146f8d`)

Ran `build_finding_register` → `build_ga_acceptance_report` →
`build_decision_dossier` → `sync_evidence_counters`, then the same four
again (second pass: no change, `sync` `changed: []`). Result:
- `evidence/review/finding-register.json`: the three `sley2_trial_runner`
  lane obligations now read `package_status`
  `S20_620_IMPLEMENTED_REVIEW_PENDING`, with dispositions and states
  unchanged (the revision-4 `PASS_0_P0_0_P1_0_P2_0_P3` values, state PASS);
  `sley2_trial_runner` left `complete_packages` (37 → 36);
  `complete_packages_with_open_reviews` and `mid_string_complete_packages`
  stay empty; obligations 484, open reviews 22, result
  `FINDING_REGISTER_OPEN`; obligations/register digests rotated.
- `machineresearch/sley-2.0/machine-summary.json`
  `finding_register.complete_packages` 37 → 36 (sync).
- `evidence/release/ga-acceptance-report.json`: only the carried
  obligations/register/report digests changed; GA states unchanged
  (EVIDENCED 31, GATED 4, AWAITS_REVIEW 17); `ga_claimed` false.
- `evidence/release/decision-dossier.json`: no change.
- `scripts/generate_supply_chain_evidence.py`: run last, after this record's
  commit (it scans tracked files); its result and commit are reported with
  the follow-up, not here.

Checkers at `e0146f8d`: `check_finding_register.py` exit 0 (problems []),
`retire_review_claims.py --check` 0, `build_ga_acceptance_report.py --check`
0, `check_sley2_trial_runner.py` 0, `check_local_completion_frontier.py` 0,
`check_decision_dossier.py` 1 (`test-inventory:drift` only, the
pre-existing candidate-bound failure; not part of the requested chain).

### 10.3 Re-run gates

| Check | Command | Exit | Result |
|---|---|---|---|
| live suites | `python3 -m unittest discover -s bench/live/tests -t .` (binaries bound, bwrap, `TMPDIR` on `/home`) at `e0146f8d` | 0 | Ran 219 tests in 1998.5s, OK (includes the 7 integrated CONTEXT proofs and the 5 new contract tests) |
| trial-runner tests | `python3 -m unittest discover -s bench/sley2/tests -t .` | 0 | Ran 23 tests, OK |
| lint | `make lint` at `e0146f8d` | 0 | PASS, 0 clippy warnings, fmt clean, clean tree; `lint-report.json` restored |

Environment note: one earlier re-run attempt was aborted because `/tmp`
(tmpfs) filled. The fixture oracle subprocess does not inherit `TMPDIR`
(`bench/live/oracle.py` passes an explicit environment), so judge
workspaces for the 10,011-entity CONTEXT pack land in `/tmp` whatever the
test's `TMPDIR`. Not changed (frozen oracle environment mapping).

## 11. Revision-5 review round at `5b538f3` and repairs (2026-09-23)

### 11.1 Round

Transcripts committed unchanged in `149f9142`, sha256 equal to the round
index: Ariadne `ariadne_contract_review_revision_5-5b538f3.md`
(`6c708655…970`) REVISE_0_P0_1_P1_1_P2_3_P3_2_P4; Nabu
`nabu_architecture_review_revision_5-5b538f3.md` (`5de991a8…4c9`)
REVISE_0_P0_0_P1_1_P2_4_P3_4_P4; Vulcan
`vulcan_surface_review_revision_5-5b538f3.md` (`a0bb48d1…c1d`)
REVISE_0_P0_0_P1_1_P2_3_P3_2_P4; index
`evidence/review/rounds/context-r5-5b538f3.json`. The three verdicts are
recorded as `sley2_trial_runner.<lane>_revision_5` with dated, scoped notes;
the base revision-4 PASS fields gained dated notes so the finding register
keeps the three REVISE rounds open (without them the register folded the
REVISE rounds under the undated base PASS — a misrepresentation this
avoids). Status stays `S20_620_IMPLEMENTED_REVIEW_PENDING`.

Repair commits: `dda51a16` (Rust), `a8b4cddb` (SMP1 revision 13, S20-300
revision 4, S20-620 section 9, summaries, register), `2563d577` (judge,
tool, gateway, capture, TOOLING, stand-in, tests, records), `2485d350`
(register/GA/dossier/sync chain, two passes to convergence), `1450577a`
(`generate_supply_chain_evidence.py`, last), and this record's commit.

### 11.2 Per-finding closure

| Finding | Disposition | Where / evidence |
|---|---|---|
| Ariadne P1 / Nabu P2 — SMP1 owns method 201 | FIXED. SMP1 revision 13: row 201 request corrected to none; appendix A row 201 and `open_summary` grammar (S20-390 fields 1-8 + optional S20-300 field 9, version 2 selections only, structural absence, one item, non-empty body refused); revision-12 "unchanged" statements amended; version 1 bytes unchanged (now true by a version gate). Owning package moved: `protocol` status `S20_400_COMPLETE` → `S20_400_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING`, `contract_revision` 13, `current_delta_review` 13 PENDING×3, `contract_complete` false; WORK_PACKAGES S20-400 row, ADR-0032. S20-620 §9 now cites SMP1; Boundary sentence true. | `a8b4cddb`, `dda51a16`; `check_smp1_contract.py` PASS rev 13; `workspace_open_under_version_1_never_carries_field_9`, `workspace_open_v2_discloses_only_the_materialized_head_snapshot`. Owner-lane (Ariadne) review of revision 13 NOT run. |
| Ariadne P2 — S20-300 admits the probe | FIXED. Profile revision 4 §5 "Identity probe": exact rules (four acceptance rules, at most one file read, no object, no build, no write-back, no delete, discard = absence, guard rule), sole consumer, pointer-not-evidence; §8 evidence, §9 exclusion corrected; checker revision 4 with a probe-caller allowlist gate (`server.rs` only); summary `S20_300_FULL_COMPLETE` → `S20_300_FULL_IMPLEMENTED_REVIEW_PENDING`, `cache_consumers` updated; ADR-0029, WORK_PACKAGES row. | `a8b4cddb`; `check_complete_root_index_snapshot_profile.py` PASS rev 4. Revision 4 reviews NOT run. |
| Ariadne P3 — §9 precision | FIXED. "Only when … any probe failure is absence"; warm-up refusal is `QUERY_SNAPSHOT_MISMATCH` only when the engine answers, else the owner code, and materialization only if the cache write succeeds. Pinned by a test that observed the owner-code case on the TYPE fixture (`INDEX_SNAPSHOT_ROOT_INCOMPLETE`, no snapshot ever disclosed). | `a8b4cddb`; `test_mediated_gateway.test_warm_up_owner_refusal_wins_and_materializes_nothing` |
| Ariadne P3 — one-shot route cannot page | FIXED by the alternative the finding allows: the multi-invocation continuation decision landed (chain-bound audit, Vulcan P2 row); TOOLING states the rule; sentence pins in `test_documented_layout_is_pinned`. Review of that decision is the next round's. | `2563d577` |
| Ariadne P3 / Nabu P3 — stale records | FIXED. SUCCESSION-COVERAGE CONTEXT row and MEDIATED-INPUT-ACCOUNTING corrected (layout documented, 8 integrated proofs, live-model usability documented not demonstrated). | `2563d577` |
| Ariadne P4 / Vulcan P4 — `workspace.open` ignores a body | FIXED for `workspace.open` (non-empty → `PROTOCOL_PAYLOAD_INVALID`, both versions, tested). The other body-ignoring methods named (session.capabilities/budgets, exchange.export, refs.recover, recovery) are unchanged pre-existing behavior, not agent-reachable except via denied methods / the two session reads. | `dda51a16` |
| Ariadne P4 — ADR-0036 status, §9 heading | FIXED. ADR status refreshed to revision 5; heading "9. Clarifications (revision 2 onward)"; checker anchors verbatim (PASS). | `a8b4cddb` |
| Nabu P3 — opener lock behavior | FIXED in part, premise corrected. The probe now takes the boundary shared **without waiting** and never initializes it (no create_dir/fsync). Correction: the opener already took a blocking shared maintenance lock before this change — `head()` → `accepted_head()` acquires it (`crates/sley-txn/src/repository.rs:881-884`) — so blocking behind an exclusive GC/import owner is pre-existing S20-390 behavior, now stated in the docstring and §9. A protocol-level contention test is not possible for the probe alone (the head load blocks first); I wrote one, it hung for exactly that reason, and it was removed. | `dda51a16`, `a8b4cddb` |
| Nabu P3 — continuation scope authority | FIXED (option (a)): cursor-bound audit, trial-wide, works on the documented one-shot route. | `2563d577` (Vulcan P2 row) |
| Nabu P3 — tool identity | FIXED. `TOOL_VERSION` "2"; `MEDIATED_TOOL_VERSION` derived from it; prior-version rejection tests on both routes. | `2563d577`; `test_agent_access.test_prior_tool_version_rejects`, `test_mediated_access.test_prior_tool_version_rejects` |
| Nabu P4 — witness logs lack identity | FIXED. The witness emits `source <commit>` (and whether tracked files differed); logs re-run at the repair head (section 11.3). | `2563d577` + log commit |
| Nabu P4 — test-only module dependency | FIXED by docstring and accounting row 5 (route-neutral scripted agent, test-only on both routes). | `2563d577` |
| Nabu P4 — budgets pinned as literals | FIXED. `test_documented_budgets_are_the_enforced_constants` ties the numbers to `sley2_tool.MAX_RESPONSE_BYTES`, the judge's `MAX_RESPONSE_BYTES` and `AGENT_CUMULATIVE_RESPONSE_BUDGET`, and the capture's per-response and trial caps. | `2563d577` |
| Nabu P4 — corpus amendment ratification | OPEN. Needs the corpus owner in the REQ-10 design chain; not an implementation action. | — |
| Vulcan P2 — unconditional continuation discharge | FIXED. The tool derives each answered root-query page's binding (query key = request with cursor elided, request cursor, truncation, next cursor) from the exact bodies on the trusted side; the gateway carries it into the capture response record; both audits use one trial-wide `_ContinuationLedger`: only a successful `query.continue` of the same query at the open page's exact next cursor discharges; refused, unbound, forged-label, skipping, past-the-end, other-query, and surplus continues never discharge; routes without continuation (cursor-bearing `query.root`, `query.restricted`, `refs.list`) stay open. Regressions: 13 new mediated-audit cases (10 of them FAIL/ERROR on the pre-repair judge, run with the `2563d577^` judge file), 5 new direct-audit cases, and the integrated `fake_discharge` mode (refused continue + imitation label + `ff…ff` cursor → rejected "inconsistent continuation"). | `2563d577` |
| Vulcan P3 — open capture labels | FIXED. `audit_label` closed set; denied/unknown commands record as `denied` on every capture path (handle, gateway loop, pump, ingress server); the mediated audit rejects any label outside `AUDIT_LABELS`. | `2563d577`; `test_denied_commands_record_under_the_fixed_label`, `test_forged_continue_label_rejects` |
| Vulcan P3 — surface contract / scope | FIXED. Continuation no longer depends on scope; TOOLING says so; regressions: cross-scope bound continue accepted, cross-scope wrong cursor rejected. The staged transport is not documented in TOOLING (no longer needed for paging). | `2563d577` |
| Vulcan P3 — budget units | FIXED. TOOLING states the per-reply bound in reply bytes (hex doubles the body) with the safe `max_response_bytes` 524000 and the 8388608 capture backstop; the stand-in requests 524000; `test_a_maximal_documented_page_fits_the_per_reply_bound` covers both routes' envelopes. | `2563d577` |
| Vulcan P4 — probe resource claims | FIXED. Non-waiting guard; §9 and S20-300 §5 state the one-file bounded decode (≤ `MAX_SNAPSHOT_RECORD_BYTES`) instead of "metadata-only". | `dda51a16`, `a8b4cddb` |

Deviation to disclose: the consumer contracts that pin the SMP1 revision
(`SMP1_JSON_BRIDGE_V1.md`, `SLEY_CLI_V1.md`, `SESSION_HANDLE_PROFILE_V1.md`,
ADR-0033, and their checkers' `SMP1_REVISION`) were re-pinned to 13 as
pin-only edits with an explicit re-pin sentence and no consumer revision
bump or status move. The CLI's own precedent re-pinned through a CLI
revision (revision 4 and 5). The S20-330, S20-420 and S20-430 owners should
confirm or require revisions; `test_smp1_contract.py` now derives the SMP1
revision from its Status line instead of the literal 12.

### 11.3 Gates at the repair head

| Check | Command | Exit | Result |
|---|---|---|---|
| server tests | `cargo test --locked --offline -p sley-protocol --lib workspace_open` | 0 | 2 passed |
| touched crates | `cargo test --locked --offline --no-fail-fast -p sley-repo -p sley-protocol -p sley-cli` | 101 | 640 passed, 1 failed (`debug_commit_repro`, documented diagnostic), 25 ignored |
| SMP1 family | `check_smp1_contract.py`, `check_cli_contract.py`, `check_smp1_json_bridge_contract.py`, `check_session_handle_profile.py`, `test_smp1_contract.py`, `test_current_contract_review.py`, `test_session_handle_profile.py`, `test_cli_contract.py`, `generate_smp1_json_bridge_table.py --check` | 0 each | PASS / OK |
| S20-300 | `check_complete_root_index_snapshot_profile.py` | 0 | PASS, revision 4 |
| trial runner | `check_sley2_trial_runner.py` | 0 | PASS, revision 5 |
| register | `check_finding_register.py` | 0 | PASS (28 open reviews incl. the 3 S20-620 REVISE rounds and 3 SMP1 current-delta PENDING) |
| live suites | `python3 -m unittest discover -s bench/live/tests -t .` at `1450577a` (binaries bound, bwrap, `TMPDIR` on `/home`) | 0 | Ran 243 tests, OK (includes the 8 integrated CONTEXT proofs) |
| integrated CONTEXT proofs | `python3 -m unittest -v bench.live.tests.test_mediated_context` (repair tree) | 0 after one rerun | 8 tests: 7 OK in the first run, `test_context_exceeded_budget_rejects` failed on a binary relink by a concurrent cargo run (environment note below) and passed alone; all 8 pass again inside the 243-test run |
| trial-runner tests | `python3 -m unittest discover -s bench/sley2/tests -t .` | 0 | Ran 23 tests, OK |
| witnesses | `succ_witness_context.py pos|neg`, `succ_witness_sig.py pos`, `succ_witness_type_full.py pos` at `1450577a` | 0 each | CONTEXT pos ACCEPTED, CONTEXT neg refused at validation phase 6, SIG ACCEPTED, TYPE-FULL ACCEPTED; logs committed in `5233dd9e` |
| quick | recipe keep-going (130 lines) at `1450577a` | — | 118 exit 0; failing: fuzz proof records predating lane changes #42, #43, #48, #49, #109, #114, #115 (fuzz refresh not run); candidate-bound #83, #84, #85, #88 (`test-inventory:drift`); #130 `cargo test --workspace` 1026 passed / 1 failed (`debug_commit_repro`) / 31 ignored. #86 and #105 now pass. |
| lint | `make lint` at `5233dd9e` | 0 | PASS, 0 clippy warnings, fmt clean, clean tree; `lint-report.json` restored and not committed |

Environment notes: a first full integrated run failed one test with
"mediated tool/binary identity mismatch" because a concurrent
`cargo test -p sley-cli` relinked the served binary mid-attempt; rerun
alone it passed, and no cargo ran during the final suite runs. A stray
in-repository `target/` directory created by one unscoped cargo command was
deleted before any commit (never tracked).

### 11.4 Still open

- Re-review of revision 5 (all three lanes), SMP1 revision 13 current-delta
  review, S20-300 revision 4 reviews; consumer-owner confirmation of the
  pin-only re-pins.
- Fuzz refresh for the seven affected lanes.
- Corpus task-input amendment ratification (Nabu P4).
- Live-model trial.

## 12. Round 6 at `f073811` and repairs (2026-09-23)

### 12.1 Round

Transcripts committed unchanged in `1a32ceca` with the index
`evidence/review/rounds/context-r6-f073811.json`. The index lists eight
reviews. The Ariadne S20-620 revision-6 NO_VERDICT stub is a process
artifact: it is listed under `omitted_process_artifacts` and not filed,
and the coordinator re-dispatches that lane. Verdicts:

- SMP1 revision 13: Ariadne REVISE (1 P1, 4 P3, 3 P4); Nabu REVISE (1 P1,
  1 P2, 4 P3, 1 P4); Vulcan REVISE (1 P2, 3 P3, 2 P4).
- S20-300 revision 4: Ariadne PASS (2 P3, 3 P4); Nabu REVISE (2 P3, 2 P4);
  Vulcan REVISE (1 P2, 2 P3, 1 P4).
- S20-620 revision 6: Nabu PASS (2 P3, 3 P4); Vulcan PASS (3 P4).

Each verdict is recorded as `<lane>_revision_<N>` with a dated, scoped note.

Repair commits:

- `40a84a5c`: S20-300 revision 5 (code, spec, checker, checker tests,
  server tests).
- `8357c243`: SMP1 revision 14, NATIVE_TEST_ADMISSION revision 6, consumer
  re-pins, S20-620 revision 6, summary, WORK_PACKAGES.
- `c21ff945`: judge docstrings, coverage record, witness provenance.
- `df3a45ac`: witness logs at `c21ff945`.
- `46118639`: fuzz lockfile.
- `c2664f10`: fuzz proofs.
- `50fca15f`: entrypoints anchor.
- `59a26519`: register/GA/dossier/sync chain.
- `5f501d39`: T54.
- `7fcb313b`: S20-710 mirror of the new lock digest and edge.
- `746d5b30`: T54, last.
- `d307a22f`: scratch-workspace leak fix (section 12.6).
- This record's commit.

### 12.2 The version rule (all three SMP1 P1/P2 findings, S20-300 Ariadne P4)

One rule, code unchanged: `workspace.open` answers `open_summary` (fields
1-8 plus optional field 9) under version 2 and under every later selection
whose method table includes version 2's row 201. Version 3 is such a
selection, because `NATIVE_TEST_ADMISSION_V1` appendix D defines it as the
union of the SMP1 version 1 and version 2 tables. The server gate
`protocol_version >= PROTOCOL_VERSION_V2` already implemented this. The
text and its pins changed:

- SMP1 revision 14 (history item 14) states the rule in the `open_summary`
  scope paragraph, in row 201 and in the history. It also says the explicit
  entrypoints admit selections 1, 2 and 3, with version 3 defined in
  NATIVE_TEST_ADMISSION appendices C/D.
- S20-300 revision 5 §5 names the consumer the same way.
- NATIVE_TEST_ADMISSION revision 6 (dated amendment) re-pins appendix D
  from SMP1 revision 12 to revision 14. It also names the version 3 effect:
  row 201 answers `open_summary` under version 3 as under version 2.
- ENTITY_READ_PROFILE_V2 §2 carries a dated amendment for the row 201
  exception.
- `check_smp1_contract.py` requires the NATIVE_TEST_ADMISSION pin
  "`docs/spec/SMP1.md` at revision N" to name the current contract
  revision, and `test_smp1_contract.py` refuses a stale pin.
- A new server test, `workspace_open_v3_answers_the_version_2_open_summary`,
  checks version 3: a cold open equals `revision.read`, a warm open's field
  9 equals the snapshot the root query materialized, and a body is refused.

The appendix D heading "(machine-readable, revision 5)" is left unchanged.
The table generator keys on it, and the table itself did not change.

### 12.3 Per-finding closure

| Finding | Disposition | Where / evidence |
|---|---|---|
| SMP1 Ariadne P1, Nabu P1, Vulcan P2 — version gate vs specs; no v3 test | FIXED by section 12.2 (the specs follow the code; a version 3 test is added). | `8357c243`, `40a84a5c`; `workspace_open_v3_answers_the_version_2_open_summary` |
| SMP1 Nabu P2 — NATIVE_TEST_ADMISSION pins SMP1 revision 12 | FIXED. Revision 6 re-pins appendix D to SMP1 revision 14 and adds a status-history sentence. The SMP1 checker anchors the pin, with a refusal test. | `8357c243`; `WorkspaceOpenAnchorCases` |
| SMP1 Ariadne P3, Nabu P3, Vulcan P3 — version 1 byte-for-byte overclaim | FIXED. The version 1 claim is limited to conforming (empty-body) requests. The text states that a non-empty 201 body is now refused under every version (before revision 13 it was answered with success). Row 201 cells are corrected, and ADR-0032 and `protocol.status_note` now match. | `8357c243` |
| SMP1 Ariadne P3 / Nabu P3 / Vulcan P3 — pin-only re-pins; CLI :26 stale; CLI checker blind | FIXED per the ruling: each owner re-pins through its own revision. Bridge revision 11, CLI revision 9 and session handle revision 5 each add a history sentence, fix their stale pin lines and drop the pin-only paragraphs, and their checkers are bound to the new SMP1/bridge revisions. The CLI checker also anchors the composition sentence (`composition_pin_problems`, with negative tests in `CompositionSentenceCases`), flattens whitespace for pin text, and moves its work-package marker to revision 9. Statuses leave COMPLETE (`S20_420/430/330_IMPLEMENTED_REVIEW_PENDING`), and each `current_delta_review` is PENDING at its new revision. | `8357c243`; the three checkers PASS |
| SMP1 Ariadne P3 / Nabu P3 — SMP1-family fuzz proofs stale | FIXED. Fresh proofs for all eighteen persistent-fuzz slices at the clean head `46118639`. Seven were staled by the round-6 delta and eleven more by the new `libc` dependency of `sley-repo`. Every slice checker exits 0. | `c2664f10` |
| SMP1 Ariadne P3 — entrypoints "admit only 1 and 2" (pre-existing) | FIXED. SMP1 now says the explicit entrypoints admit selections 1, 2 and 3, with version 3 defined by NATIVE_TEST_ADMISSION appendices C/D. The checker anchors this as `entrypoints-admit-version-3`, with a revert case. | `8357c243`, `50fca15f` |
| SMP1 Nabu P3 / Vulcan P4 — undefined `-- field 9 optional` notation | FIXED. The appendix A preamble defines optional-by-omission `[n: T]` record fields, and `open_summary` uses `[9: IndexSnapshotId]`. | `8357c243` |
| SMP1 Vulcan P3 — field 9 is "the identity the queries bind" | FIXED. The text now reads "Field 9 is a pointer, not evidence". It equals the bound identity only when the cache is honest (the S20-300 §5 residual). `capsule` always binds a fresh snapshot, so a client that adopts field 9 keeps the `QUERY_SNAPSHOT_MISMATCH` cross-check. | `8357c243` |
| SMP1 Ariadne P4 / Nabu P4 — non-waiting overstated; head load blocks | FIXED. One sentence states that the opener still loads the head under the S20-390 blocking shared acquisition and that only the probe adds no wait. New test `workspace_open_probe_is_non_waiting_and_never_rewrites_a_discarded_record`: while an exclusive guard is held, `materialized_head_snapshot` returns None promptly, and a corrupted cache yields an 8-field body with the cache bytes unchanged. | `8357c243`, `40a84a5c` |
| SMP1 Nabu P4 / Vulcan P4 — determinism scope | FIXED. The body is deterministic given the head plus the derived cache state. Appendix B's "equal repository state" and section 10's byte-identity now include that state for this body. | `8357c243` |
| SMP1 Ariadne P4 — owner column | FIXED. The v1-table preamble states the row 201 owner-cell exception. | `8357c243` |
| SMP1 Ariadne P4 — no anchors for revision-13 text | FIXED. `WORKSPACE_OPEN_ANCHORS` covers the row 201 cells, the `open_summary` grammar and scope, optionality, the pointer sentence, the version 1 statement, the entrypoints and the native pin. Matching flattens whitespace. Revert cases are in `test_smp1_contract.py`. | `8357c243`, `50fca15f` |
| SMP1 Vulcan P4 — other "empty" rows ignore a body | RECORDED, not changed. Appendix A states that rows 102, 103, 104, 210, 214 and 504 still ignore a non-empty body, a pre-existing deviation from section 4. | `8357c243` |
| S20-300 Ariadne P3 — "only read-only derived query surfaces" | FIXED. The profile preamble, the ERROR_CODES S20-300 paragraph, the WORK_PACKAGES row and the `index_cache.rs` module doc now name the identity probe as the one non-query hit reader. | `40a84a5c`, `8357c243` |
| S20-300 Ariadne P3 / Nabu P3 / Vulcan P2 — completion accepts the revision-3 base PASS | FIXED (the trial-runner Fix 2 pattern). COMPLETE now requires `<lane>_revision_5` PASS; without it the checker reports `completion-unbound-review:<lane>`. `test_complete_root_index_snapshot_profile.py`, now in `make quick`, covers four cases: the baseline passes, a flip on the base fields is refused, a flip on revision-4 fields is refused, and a bound PASS is admitted. | `40a84a5c` |
| S20-300 Ariadne P4 / Nabu P4 / Vulcan P3 — probe gate bypassable | FIXED. `probe_gate_problems` scans `crates/` and `fuzz/` for the identifier, with comments and `use` items stripped. Only `crates/sley-repo/src/index_cache.rs` and crate `tests/` directories are exempt, and aliases are refused. The consumer must reference the probe exactly once, inside `fn materialized_head_snapshot(`. That function must call `acquire_shared_repository_maintenance_nonblocking(` and must never initialize or wait. Eight negative tests. | `40a84a5c` |
| S20-300 Ariadne P4 / Vulcan P3 — lstat→open TOCTOU (symlink/FIFO) | FIXED. `read_record` opens with `O_NOFOLLOW \| O_NONBLOCK` and checks `is_file` on the open handle. Tests: `a_symlink_to_a_valid_record_is_absence_for_the_probe` and `a_fifo_at_the_cache_path_answers_at_once`. New direct dependency `libc =0.2.189` in `sley-repo`; it was already in the registry set, and both lockfiles plus the S20-710 mirror are updated. | `40a84a5c`, `46118639`, `7fcb313b` |
| S20-300 Nabu P3 — raw root equality instead of `covers` | FIXED. All three guard checks now use `RepositoryMaintenanceGuard::covers`. `a_non_canonical_repository_spelling_is_covered_by_its_guard` covers a parent-symlink spelling and a `..` spelling; `covers` itself refuses a final-component symlink. | `40a84a5c` |
| S20-300 Nabu P4 — no contention / discarded-record test | FIXED by the server test in the SMP1 Ariadne P4 / Nabu P4 row. | `40a84a5c` |
| S20-300 Vulcan P4 — "metadata-only"; module doc | FIXED. The docs now describe a bounded decode of one file (at most `MAX_SNAPSHOT_RECORD_BYTES`) with no build, write-back or delete. The module doc names the probe. | `40a84a5c` |
| S20-620 Nabu P3 — field-9 version gate | FIXED by section 12.2. | — |
| S20-620 Nabu P3 — in-place amendment under revision 5 | FIXED by a revision bump. S20-620 is now revision 6, with a status sentence naming the §9 text it answers. The `_revision_5` REVISE history is kept, and the round-6 PASS verdicts bind as `_revision_6`. The checker passes and still refuses COMPLETE until Ariadne files a revision-6 verdict. | `8357c243` |
| S20-620 Nabu P4 — CLI :26; consumer confirmations untracked | FIXED. CLI :26 now pins SMP1 revision 14, through CLI revision 9. The three owner confirmations are now `current_delta_review` PENDING records on S20-330/420/430, and the register lists them as open reviews. | `8357c243`, `59a26519` |
| S20-620 Nabu P4 / Vulcan P4 — witness dirty flag; SIG/TYPE-FULL lack a source line | FIXED. `bench/live/witness_provenance.py` records the source commit and the exact tracked dirty paths, excluding the `bench/live/succ-trials-*` outputs (4 tests). All three witness scripts emit this line. The four logs were re-run at `c21ff945`; each reads "clean apart from witness outputs", CONTEXT pos, SIG and TYPE-FULL finish with judge exit 0, and CONTEXT neg is refused at validation phase 6 (tag 9). | `c21ff945`, `df3a45ac` |
| S20-620 Vulcan P4 — judge docstrings describe the old scope rule | FIXED. Both audit docstrings now describe `_ContinuationLedger`: query-key and cursor binding across the trial, with `after` verified equal to the previous page's `next`. The SUCCESSION-COVERAGE paragraph is marked superseded. | `c21ff945` |
| S20-620 Vulcan P4 — SMP1 version-1 overclaim | FIXED (see the SMP1 row). | `8357c243` |
| S20-620 Nabu P4 — corpus amendment ratification | OPEN. Only the corpus owner can ratify it; no implementation change closes it. | — |

Register note. On the first regeneration, the register folded the S20-300
revision-4 REVISE rounds and the SMP1 revision-13 REVISE rounds as
historical. The cause was older PASS fields that carried no date: the
S20-300 `*_final_review` fields and the protocol base and `*_review` fields.
Each of those fields now has a note naming the commit and date it was last
set (`e73644ce`, 2026-09-14; `5e2f2b93`, 2026-09-10; `7221dd6e`,
2026-09-05). Their values are unchanged. With the notes, the register keeps
all five rounds open: 42 open reviews and 31 complete packages, and
`ga_claimed` stays false. The session stale-safety GA criterion moves to
AWAITS_REVIEW together with S20-330.

### 12.4 Gates at the repair head

| Check | Exit | Result |
|---|---|---|
| `cargo fmt --check`; `cargo clippy -D warnings` (sley-repo, sley-protocol, sley-cli, all targets) | 0 | clean |
| `cargo test --offline --locked --no-fail-fast -p sley-repo -p sley-protocol -p sley-cli -- --skip debug_commit_repro` | 0 | 645 passed, 0 failed |
| SMP1 family, S20-300, trial runner, capsule, root-backed and required-index checkers, plus their `test_*.py` (including the new `test_complete_root_index_snapshot_profile.py`) | 0 each | PASS / OK |
| all 18 persistent-fuzz slice checkers | 0 each | PASS at `46118639` |
| `check_finding_register.py`, `check_supply_chain_audit.py` | 0 | PASS |
| witnesses (4) at `c21ff945` | 0 each | see the witness row in 12.3 |
| `bench/sley2/tests` | 0 | 23 tests OK |
| `bench/live/tests`, first run at `5f501d39` | 1 | 247 tests, 246 OK. `test_context_incomplete_discovery_rejects` returned `harness_failure` while `/tmp` was out of inodes (tmpfs 100% IUse; see 12.6). Re-run alone after `/tmp` was freed, it passed (OK, 174 s). |
| `bench/live/tests` at `d307a22f` (leak fix; binaries copied out of the target directory; private `TMPDIR=/home/gfarch/Work/checkpoints/sct-r6`) | 0 | Ran 254 tests, OK (includes the 8 integrated CONTEXT proofs and `test_scratch_cleanup`). `/tmp` inodes went from 101471 to 101472 across the run. The private root held only a `uv-*.lock` afterwards, with no `sley2-*` directory. |
| `bench/live/tests`, discarded attempt at `d307a22f` | 1 | Run under a longer private `TMPDIR` (`…/succ-context-tmp/live-r6b`). 8 mediated CONTEXT and 13 mediated-attempt tests failed with `harness_failure` or an empty capture. The same single test failed under `…/live-r6c` and passed under `…/succ-context-tmp`, which is consistent with the mediated gateway's AF_UNIX socket path exceeding the 108-byte limit. That cause is inferred and not otherwise verified. The full suite then passed under the shorter root (row above). |
| `make quick` recipe, keep-going (131 lines, at `5f501d39`) | — | 124 exit 0. Failing: the candidate-bound #82 (the only failing release test is `namespace-not-attestation-bound`), #83, #84, #85 and #88 (`test-inventory:drift`). #106 failed on the S20-710 lock-digest mirror; `7fcb313b` fixed it and it now exits 0. #131 `cargo test --workspace`: 1031 passed, 1 failed (`debug_commit_repro`, the documented env-bound diagnostic), 31 ignored. |
| `make lint` at `d307a22f` | 0 | PASS: fmt clean, 0 clippy warnings, lint inputs clean. The only dirty path was this record, uncommitted at the time, so `working_tree_clean` was false. `lint-report.json` was restored and not committed. |
| CONTEXT witness pos at `d307a22f` (untracked log) | 0 | judge exit 0; the workspace line reads "(removed at exit)" and the private root held no `sley2-*` directory afterwards |

### 12.5 Still open

- Re-dispatch Ariadne on S20-620 revision 6 (the round-6 stub is a
  process artifact).
- Reviews of SMP1 revision 14, S20-300 revision 5, and the consumer
  re-pins (bridge 11, CLI 9, session handle 5). All sit at
  `*_IMPLEMENTED_REVIEW_PENDING` / PENDING.
- Corpus task-input amendment ratification.
- The other empty-request rows still ignore a body (recorded, not changed).
- Live-model trial.
- `bench/live/capture_demo.py` still keeps its run roots on purpose
  (demo evidence, "run root kept at"). It is not covered by the leak fix.
- Mediated runs need a short `TMPDIR`, because the gateway socket path
  must fit AF_UNIX (inferred, see 12.4).

### 12.6 Scratch-workspace leak (`d307a22f`)

The coordinator found `/tmp` (tmpfs, 1M inodes) exhausted. The judge's
`sley2-judge-*` scratch copy (`bench/fixtures/sley2_live_judge.py`,
`workdir = Path(tempfile.mkdtemp(prefix="sley2-judge-"))`) was never
removed. Read-only store and tool files also defeated
`shutil.rmtree(..., ignore_errors=True)` in the `sley2-pristine-`,
`sley2-corrupt-` and `sley2-merge-` helpers and in the witnesses'
`sley2-*-witness-*` workspaces. About fifty leaked runs of ~19.6k files
each filled the tmpfs. The fix:

- `bench/live/scratch.py`: `remove_scratch` restores owner write (and
  search, on directories) on the failing path's parent and on the path
  itself, then retries. For a directory it could not list, it removes that
  directory whole once it is accessible. It raises `ScratchRemovalError` if
  the tree still exists. `scratch_root` gives a run a private root:
  `tempfile.tempdir` and `TMPDIR` point into it, so child processes land
  there too, and the root is removed on every exit path.
- The judge removes its scratch copy in a `finally` on accept, reject and
  harness-error paths. A failed removal becomes a harness error
  (`LIVE_SLEY2_JUDGE_INVALID: scratch removal`). The pristine and merge
  helpers remove their workdir when staging fails. The corrupt helper and
  both cleanups use the same removal. The merge side cleanups still ignore
  a session-close error, as before, but no longer swallow a removal
  failure.
- Every witness entrypoint runs under `scratch_root`, and its log line now
  says "(removed at exit)". `prove_merge_production.py` uses
  `remove_scratch`.
- `bench/live/tests/test_scratch_cleanup.py` (7 tests) covers read-only
  and unlistable trees, a demonstration that the old `ignore_errors`
  removal leaves the tree, loud failure, and `scratch_root` removal on an
  exception. It also runs judge paths in a private root and asserts no
  `sley2-*` remains: harness error, the real binary, and a stuck removal,
  which must become a harness error. The harness-error case fails on the
  previous judge (`sley2-judge-41cwje_x` left behind) and passes on the
  fixed judge.

All my runs since then use a private `TMPDIR` on `/home`.

## Appendix A. Non-domain widened-token hits by file (line numbers at `44f18e2b`)

### A.1 Frozen history (review transcripts, request packets, gate records, campaign records, retained logs)

- `bench/live/GATE-RECORD-20260922-REV5-REVIEW-LINT.md` [S20_620_COMPLETE]: L133
- `evidence/review/S20_710_ACCEPTANCE_MAP.md` [S20_620_COMPLETE]: L350
- `evidence/review/finding-register.json` [S20_620_COMPLETE]: L6870,6884,6898
- `evidence/review/qualification-historical-closures-2026-09-15.md` [revision 4]: L9
- `evidence/review/qualification-record-correction-review-2026-09-15.md` [revision 4]: L27
- `evidence/review/requests/REQ-02-s20-730.md` [revision 4]: L8
- `evidence/review/requests/REQ-05-entity-read-scoped.md` [revision 4]: L5
- `evidence/review/requests/REQ-10-rev5-context-bounded-discovery.md` [eighteen, revision 4]: L20,32,35,40
- `evidence/review/requests/REQ-10-rev6-context-bounded-discovery.md` [S20_620_COMPLETE, eighteen, eighteen-method, implementation_complete, revision 4]: L49,52,54,147,155,156,159,176,177,184
- `evidence/review/requests/REQ-10-rev7-context-bounded-discovery.md` [Council reviews PASS, S20_620_COMPLETE, eighteen, eighteen-method, implementation_complete, reviews PASS, revision 4]: L114,115,121,122,123,124,126,130
- `evidence/review/requests/REQ-10-rev9-context-bounded-discovery.md` [eighteen]: L86,88,93
- `evidence/review/verdicts/cli/delta_review_nabu-1023a31.md` [implementation_complete]: L6
- `evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-rev4-b0e52e00.md` [eighteen, revision 4]: L11,17
- `evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-rev5-c1e8db56.md` [S20_620_COMPLETE, eighteen, eighteen-method, revision 4]: L13,20,21,66,70,74,76,78,89
- `evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-rev6-95d42fda.md` [Council reviews PASS, S20_620_COMPLETE, eighteen, eighteen-method, implementation_complete, reviews PASS, revision 4]: L75,76,83,87,91,139
- `evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-rev7-95d42fda.md` [Council reviews PASS, S20_620_COMPLETE, eighteen, eighteen-method, reviews PASS, revision 4]: L46,59,75,79,80
- `evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-rev8-95d42fda.md` [S20_620_COMPLETE, eighteen, revision 4]: L71,133,163,189
- `evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-rev9-95d42fda.md` [eighteen]: L54
- `evidence/review/verdicts/decision_dossier/ariadne-e050fe7.md` [eighteen]: L10
- `evidence/review/verdicts/decision_dossier/ariadne_contract_review-a809906.md` [revision 4]: L25
- `evidence/review/verdicts/decision_dossier/nabu_architecture_review-a809906.md` [eighteen, revision 4]: L23,298,305
- `evidence/review/verdicts/decision_dossier/vulcan_surface_review-a809906.md` [revision 4]: L7,100
- `evidence/review/verdicts/finding_register/ariadne_contract_review-db1bc62.md` [revision 4]: L1,3,24,62,132
- `evidence/review/verdicts/finding_register/nabu_architecture_review-db1bc62.md` [revision 4]: L29
- `evidence/review/verdicts/finding_register/vulcan_surface_review-db1bc62.md` [revision 4]: L6
- `evidence/review/verdicts/json_bridge/delta_review_nabu-1023a31.md` [implementation_complete]: L6
- `evidence/review/verdicts/release_candidate_packaging/ariadne-400895e.md` [Council reviews PASS, implementation_complete, reviews PASS, revision 4]: L7,9,19,20
- `evidence/review/verdicts/release_candidate_packaging/ariadne_contract_review-3320ca9.md` [implementation_complete]: L20
- `evidence/review/verdicts/release_candidate_packaging/ariadne_contract_review-79fdcc6.md` [implementation_complete]: L11
- `evidence/review/verdicts/release_candidate_packaging/ariadne_contract_review-b58ac1e.md` [implementation_complete]: L11
- `evidence/review/verdicts/release_candidate_packaging/nabu-400895e.md` [Council reviews PASS, implementation_complete, reviews PASS, revision 4]: L7,17
- `evidence/review/verdicts/release_candidate_packaging/nabu_architecture_review-1a9f0aa.md` [implementation_complete]: L43
- `evidence/review/verdicts/release_candidate_packaging/nabu_architecture_review-6589c6e.md` [implementation_complete]: L15
- `evidence/review/verdicts/release_candidate_packaging/nabu_architecture_review-7622776.md` [implementation_complete]: L45
- `evidence/review/verdicts/release_candidate_packaging/nabu_architecture_review-76ae15a.md` [implementation_complete]: L52
- `evidence/review/verdicts/release_candidate_packaging/nabu_architecture_review-79fdcc6.md` [implementation_complete]: L15
- `evidence/review/verdicts/release_candidate_packaging/nabu_architecture_review-8f774d0.md` [implementation_complete]: L15
- `evidence/review/verdicts/release_candidate_packaging/nabu_architecture_review-b58ac1e.md` [implementation_complete]: L15
- `evidence/review/verdicts/release_candidate_packaging/nabu_architecture_review-c04539b.md` [implementation_complete]: L38
- `evidence/review/verdicts/release_candidate_packaging/nabu_architecture_review-c67b072.md` [implementation_complete]: L42
- `evidence/review/verdicts/release_candidate_packaging/nabu_architecture_review-db53894.md` [implementation_complete]: L38
- `evidence/review/verdicts/release_candidate_packaging/vulcan-400895e.md` [Council reviews PASS, implementation_complete, reviews PASS, revision 4]: L7,15
- `evidence/review/verdicts/reproducibility_and_independent_conformance/ariadne_contract_review-a809906.md` [implementation_complete, revision 4]: L17,24,52
- `evidence/review/verdicts/reproducibility_and_independent_conformance/ariadne_contract_review-c5973c9.md` [revision 4]: L65
- `evidence/review/verdicts/reproducibility_and_independent_conformance/nabu_architecture_review-a809906.md` [implementation_complete]: L20
- `evidence/review/verdicts/reproducibility_and_independent_conformance/vulcan_surface_review-0bcc9c6.md` [revision 4]: L46
- `evidence/review/verdicts/reproducibility_and_independent_conformance/vulcan_surface_review-c5973c9.md` [revision 4]: L18
- `evidence/review/verdicts/required_contract_index/delta_review_nabu-43f2f5b.md` [implementation_complete]: L42
- `evidence/review/verdicts/root_backed_query_profile/ariadne_contract_review-a4b6029.md` [revision 4]: L43,58
- `evidence/review/verdicts/root_backed_query_profile/ariadne_contract_review-a809906.md` [revision 4]: L221
- `evidence/review/verdicts/root_backed_query_profile/ariadne_contract_review_closure-178873d.md` [revision 4]: L34
- `evidence/review/verdicts/root_backed_query_profile/ariadne_entity_read_review-9ae09a1.md` [revision 4]: L36
- `evidence/review/verdicts/root_backed_query_profile/nabu_architecture_review-a4b6029.md` [revision 4]: L46
- `evidence/review/verdicts/root_backed_query_profile/nabu_entity_read_review-9ae09a1.md` [revision 4]: L24,54
- `evidence/review/verdicts/s20_700_remaining_surface_audit/vulcan_review_closure-db53894.md` [eighteen]: L50
- `evidence/review/verdicts/session_handle_profile/delta_review-a4b6029.md` [revision 4]: L18,23
- `evidence/review/verdicts/standards_sbom_and_provenance/ariadne_contract_review-a809906.md` [implementation_complete, revision 4]: L59,254
- `evidence/review/verdicts/standards_sbom_and_provenance/ariadne_contract_review-abf4ff0.md` [revision 4]: L26
- `evidence/review/verdicts/standards_sbom_and_provenance/ariadne_contract_review-e464ed4.md` [revision 4]: L24
- `evidence/review/verdicts/standards_sbom_and_provenance/nabu_architecture_review-70283ce.md` [implementation_complete]: L56,198
- `evidence/review/verdicts/standards_sbom_and_provenance/vulcan_surface_review-70283ce.md` [implementation_complete]: L46
- `evidence/review/verdicts/standards_sbom_and_provenance/vulcan_surface_review-abf4ff0.md` [revision 4]: L33
- `evidence/review/verdicts/standards_sbom_and_provenance/vulcan_surface_review_revision_9-178873d.md` [implementation_complete]: L31
- `evidence/review/vm-nabu-correction-review-2026-09-15.md` [eighteen, implementation_complete]: L8,75
- `evidence/validation/native-slice-completion-ledger-v1.json` [revision 4]: L146
- `evidence/validation/s20-500-ref-branch-contract-freeze-v1.json` [implementation_complete]: L28
- `evidence/validation/s20-530-crash-recovery-closeout-v1.json` [implementation_complete]: L4
- `evidence/validation/s20-530-crash-recovery-contract-freeze-v1.json` [implementation_complete]: L37
- `evidence/validation/s20-530-crash-recovery-contract-freeze-v10.json` [implementation_complete]: L79
- `evidence/validation/s20-530-crash-recovery-contract-freeze-v11.json` [implementation_complete]: L87
- `evidence/validation/s20-530-crash-recovery-contract-freeze-v12.json` [implementation_complete]: L5,97
- `evidence/validation/s20-530-crash-recovery-contract-freeze-v13.json` [implementation_complete]: L5,91
- `evidence/validation/s20-530-crash-recovery-contract-freeze-v2.json` [implementation_complete]: L47
- `evidence/validation/s20-530-crash-recovery-contract-freeze-v3.json` [implementation_complete]: L57
- `evidence/validation/s20-530-crash-recovery-contract-freeze-v4.json` [implementation_complete]: L70
- `evidence/validation/s20-530-crash-recovery-contract-freeze-v5.json` [implementation_complete]: L70
- `evidence/validation/s20-530-crash-recovery-contract-freeze-v6.json` [implementation_complete]: L73
- `evidence/validation/s20-530-crash-recovery-contract-freeze-v7.json` [implementation_complete]: L67
- `evidence/validation/s20-530-crash-recovery-contract-freeze-v8.json` [implementation_complete]: L83
- `evidence/validation/s20-530-crash-recovery-contract-freeze-v9.json` [implementation_complete]: L79
- `evidence/validation/s20-530-crash-recovery-logs-v1/tier2-04-cargo-test-workspace-locked.log` [eighteen]: L9179,9212
- `evidence/validation/zjx-transport-readiness-logs-v1/make-quick-remaining-steps.log` [eighteen, implementation_complete]: L14,38,53,325,595,627,868
- `evidence/validation/zjx-transport-readiness-logs-v1/make-quick.log` [S20_620_COMPLETE, implementation_complete]: L12,874,913
- `machineresearch/sley-2.0/reviews/arch-tighten-ariadne-r2-2026-09-08.log` [revision 4]: L13
- `machineresearch/sley-2.0/reviews/reweave-rw040-nabu-2026-09-06.log` [implementation_complete]: L8
- `machineresearch/sley-2.0/reviews/s20-310-ariadne-contract-review-2026-09-04.log` [eighteen]: L32
- `machineresearch/sley-2.0/reviews/s20-310-nabu-architecture-review-2026-09-04.log` [eighteen]: L34,73
- `machineresearch/sley-2.0/reviews/s20-420-ariadne-contract-review-2026-09-04.log` [revision 4]: L5,16,52,60,83
- `machineresearch/sley-2.0/reviews/s20-420-nabu-architecture-review-2026-09-04.log` [revision 4]: L5,13,40,48,66
- `machineresearch/sley-2.0/reviews/s20-420-vulcan-surface-review-2026-09-04.log` [revision 4]: L5,7,50,53,78
- `machineresearch/sley-2.0/reviews/s20-430-ariadne-contract-review-2026-09-04.log` [revision 4]: L40,84
- `machineresearch/sley-2.0/reviews/s20-430-nabu-architecture-review-2026-09-04.log` [revision 4]: L33
- `machineresearch/sley-2.0/reviews/s20-510-ariadne-contract-review-2026-09-04.log` [eighteen]: L23
- `machineresearch/sley-2.0/reviews/s20-510-nabu-architecture-review-2026-09-04.log` [eighteen, implementation_complete]: L15,53,77
- `machineresearch/sley-2.0/reviews/s20-520-ariadne-contract-review-2026-09-04.log` [revision 4]: L64
- `machineresearch/sley-2.0/reviews/s20-520-nabu-architecture-review-2026-09-04.log` [implementation_complete]: L37,67
- `machineresearch/sley-2.0/reviews/s20-620-vulcan-surface-review-2026-09-04.log` [S20_620_COMPLETE, revision 4]: L35,72,74
- `machineresearch/sley-2.0/reviews/s20-720-ariadne-contract-review-2026-09-04.log` [implementation_complete]: L68,95
- `machineresearch/sley-2.0/reviews/s20-750-ariadne-contract-review-2026-09-04.log` [revision 4]: L84
- `machineresearch/sley-2.0/reviews/s20-750-nabu-architecture-review-2026-09-04.log` [revision 4]: L76
- `machineresearch/sley-2.0/reviews/s20-770-ariadne-contract-review-2026-09-04.log` [eighteen]: L11
- `machineresearch/sley-2.0/reviews/s20-770-vulcan-surface-review-2026-09-04.log` [implementation_complete]: L19
- `machineresearch/sley-2.0/reviews/s20-780-ariadne-contract-review-2026-09-04.log` [eighteen]: L43,57
- `machineresearch/sley-2.0/reviews/s20-780-nabu-architecture-review-2026-09-04.log` [eighteen]: L36,52
- `machineresearch/sley-2.0/reviews/s20-780-nabu-rereview-2-2026-09-05.log` [revision 4]: L2
- `machineresearch/sley-2.0/reviews/s20-780-vulcan-surface-review-2026-09-04.log` [eighteen]: L32,44,48
- `machineresearch/sley-2.0/s20-250-full-entity-bodies-campaign-2026-09-03.md` [eighteen]: L71
- `machineresearch/sley-2.0/s20-260-270-vm-extended-opcode-campaign-2026-09-03.md` [revision 4]: L64
- `machineresearch/sley-2.0/s20-300-full-complete-root-snapshot-campaign-2026-09-03.md` [eighteen]: L24
- `machineresearch/sley-2.0/s20-320-full-context-capsule-campaign-2026-09-03.md` [revision 4]: L92,93,95
- `machineresearch/sley-2.0/s20-330-negotiated-session-campaign-2026-09-03.md` [reviews PASS, revision 4]: L6,12
- `machineresearch/sley-2.0/s20-390-extended-semantic-profile-campaign-2026-09-03.md` [revision 4]: L12,13
- `machineresearch/sley-2.0/s20-410-smp1-frame-campaign-2026-09-03.md` [eighteen, revision 4]: L32,170
- `machineresearch/sley-2.0/s20-440-smp1-cancel-stream-campaign-2026-09-03.md` [revision 4]: L3,43
- `machineresearch/sley-2.0/s20-520-merge-campaign-2026-09-03.md` [revision 4]: L4,44,65,71
- `machineresearch/sley-2.0/s20-530-final-checker-v13-confirmation-2026-09-02.log` [implementation_complete]: L2
- `machineresearch/sley-2.0/s20-530-post-acceptance-aging-decision-2026-09-02.md` [implementation_complete]: L15,42,72,83,98,189
- `machineresearch/sley-2.0/s20-530-resume-2026-09-01.md` [implementation_complete]: L475,598,704
- `machineresearch/sley-2.0/s20-530-stop-checkpoint-2026-09-01.md` [implementation_complete]: L81
- `machineresearch/sley-2.0/s20-530-stop-checkpoint-2026-09-02.md` [implementation_complete]: L40,62,110,142,150,161,167,188,197
- `machineresearch/sley-2.0/s20-530-v12-log-count-and-narrative-lane-amendment-design.md` [implementation_complete]: L20
- `machineresearch/sley-2.0/s20-530-v13-limit-semantic-field-amendment-design.md` [implementation_complete]: L7
- `machineresearch/sley-2.0/s20-530-v5-contract-freeze-review-packet.json` [implementation_complete]: L70
- `machineresearch/sley-2.0/s20-620-sley2-trial-runner-campaign-2026-09-03.md` [revision 4]: L51
- `machineresearch/sley-2.0/s20-630-succession-accounting-campaign-2026-09-03.md` [revision 4]: L3,4,8

### A.2 Unrelated (another package, contract, or summary section, or the eighteen SSMC1 entity kinds; no S20-620 reference on the line)

- `.forge/slices/at-mw-02-implement-reviewed-entity-and-signature-protocol-reads.json` [reviews PASS]: L43
- `README.md` [eighteen]: L56,60,90
- `conformance/complete-entity-impact/v1/accepted.json` [eighteen]: L237
- `conformance/complete-root-index-snapshot/v1/accepted.json` [eighteen]: L9,18
- `crates/sley-mutate/README.md` [eighteen]: L7,8
- `crates/sley-mutate/src/codec.rs` [eighteen]: L5593
- `crates/sley-mutate/src/generated.rs` [eighteen]: L569
- `crates/sley-mutate/src/lib.rs` [eighteen]: L276
- `crates/sley-policy/src/complete_entities.rs` [eighteen]: L2
- `crates/sley-protocol/src/server.rs` [revision 4]: L1576
- `crates/sley-query/README.md` [eighteen]: L17
- `crates/sley-query/src/complete_root.rs` [eighteen]: L4,40,403,1502,1525
- `crates/sley-query/src/context_capsule.rs` [revision 4]: L431,571,578
- `crates/sley-query/src/entity_read.rs` [eighteen]: L1289,1401,1426,1427,1458,1488,1500,1512,1532,1544,1613,1728,1792,1804,1820,1860,2732,2760,2789,2855,3009,3015
- `crates/sley-query/src/lib.rs` [eighteen]: L121
- `crates/sley-query/src/root_query.rs` [eighteen, revision 4]: L453,2805
- `crates/sley-repo/src/compare.rs` [eighteen]: L1716
- `crates/sley-repo/src/complete_root.rs` [eighteen]: L185
- `crates/sley-repo/src/merge.rs` [revision 4]: L3254
- `crates/sley-ssmc/README.md` [eighteen]: L3
- `crates/sley-vm/tests/rw080_codec_program/dependency_binding_decode.rs` [eighteen]: L1527,1532,1541,1574,1579,1588,18139,18153,18183,18232,18261,18291
- `docs/WORK_PACKAGES.md` [Council reviews PASS, eighteen, implementation_complete, reviews PASS, revision 4]: L30,31,32,35,36,37,38,46,48,49,50,52,53,54
- `docs/adr/ADR-0014-mutation-schema-codegen.md` [eighteen]: L7,19
- `docs/adr/ADR-0017-candidate-contract-freeze.md` [eighteen]: L22
- `docs/adr/ADR-0023-crash-recovery-boundary.md` [implementation_complete]: L301
- `docs/adr/ADR-0024-accepted-package-aging.md` [implementation_complete]: L12,13,52
- `docs/adr/ADR-0025-repository-exchange-boundary.md` [revision 4]: L115
- `docs/adr/ADR-0026-complete-entity-model-boundary.md` [eighteen]: L11,20
- `docs/adr/ADR-0027-semantic-comparison-boundary.md` [eighteen]: L15
- `docs/adr/ADR-0028-merge-composition-boundary.md` [revision 4]: L3
- `docs/adr/ADR-0029-complete-root-index-snapshot-boundary.md` [eighteen]: L15,27
- `docs/adr/ADR-0031-context-capsule-boundary.md` [revision 4]: L3
- `docs/adr/ADR-0032-smp1-transport-boundary.md` [reviews PASS]: L4
- `docs/adr/ADR-0033-negotiated-session-boundary.md` [reviews PASS, revision 4]: L4,7,11,14
- `docs/audits/S20_250_FULL_ENTITY_BODIES_CLOSEOUT.md` [eighteen]: L13,28,64
- `docs/audits/S20_300_FULL_COMPLETE_ROOT_SNAPSHOT_CLOSEOUT.md` [eighteen]: L12,53
- `docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md` [revision 4]: L50
- `docs/audits/S20_320_FULL_CONTEXT_CAPSULE_CLOSEOUT.md` [eighteen, revision 4]: L3,76
- `docs/audits/S20_390_ATOMIC_COMMIT_CLOSEOUT.md` [revision 4]: L163
- `docs/audits/S20_410_SMP1_FRAME_CLOSEOUT.md` [eighteen]: L76
- `docs/audits/S20_420_JSON_BRIDGE_CLOSEOUT.md` [eighteen]: L5
- `docs/audits/S20_440_SMP1_CANCEL_STREAM_CLOSEOUT.md` [revision 4]: L3,27,39
- `docs/audits/S20_520_MERGE_CLOSEOUT.md` [revision 4]: L5,60,101,112,148,165,177
- `docs/audits/S20_530_CRASH_RECOVERY_CLOSEOUT.md` [implementation_complete]: L45,49,131
- `docs/audits/S20_630_SUCCESSION_ACCOUNTING_CLOSEOUT.md` [revision 4]: L3,5,50
- `docs/audits/S20_700_COMPLETE_ROOT_JUDGMENT_PERSISTENT_SLICE.md` [eighteen]: L11
- `docs/audits/S20_700_COMPLETE_ROOT_SNAPSHOT_PERSISTENT_SLICE.md` [eighteen]: L33
- `docs/audits/S20_700_CONTEXT_CAPSULE_PERSISTENT_SLICE.md` [eighteen]: L13
- `docs/audits/S20_700_ROOT_QUERY_PERSISTENT_SLICE.md` [eighteen]: L13
- `docs/audits/S20_LOCAL_COMPLETION_FRONTIER.md` [eighteen]: L39,51
- `docs/audits/SLEY-2.0-ARCHITECTURE-TIGHTENING-AUDIT.md` [eighteen, implementation_complete, revision 4]: L485,591,962,969,972,2030
- `docs/spec/CLEAN_ROOM_DISPOSITION_REGISTER_V1.md` [revision 4]: L7,12
- `docs/spec/COMPLETE_ENTITY_IMPACT_PROFILE_V1.md` [eighteen]: L12,38,41,91,132,195,247,250,253,262
- `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md` [eighteen]: L25
- `docs/spec/CONTEXT_CAPSULE_PROFILE_V1.md` [revision 4]: L3
- `docs/spec/CRASH_RECOVERY_MATRIX_V1.md` [implementation_complete]: L1353
- `docs/spec/DECISION_DOSSIER_V1.md` [eighteen]: L365,369
- `docs/spec/ENTITY_READ_PROFILE_V2.md` [eighteen]: L324
- `docs/spec/EPOCH_MIGRATION_POLICY_V1.md` [revision 4]: L247
- `docs/spec/ERROR_CODES_V1.md` [eighteen, revision 4]: L201,318
- `docs/spec/FINDING_REGISTER_V1.md` [revision 4]: L543
- `docs/spec/MUTATION_SCHEMA_V1.md` [eighteen]: L21,55,98,111
- `docs/spec/MUTATION_VALUE_CODEC_V1.md` [eighteen]: L17,36,70
- `docs/spec/NATIVE_TEST_ADMISSION_V1.md` [revision 4]: L430
- `docs/spec/REPOSITORY_EXCHANGE_V1.md` [revision 4]: L13
- `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` [eighteen]: L151
- `docs/spec/SEMANTIC_COMPARISON_V1.md` [eighteen]: L29,58
- `docs/spec/SESSION_HANDLE_PROFILE_V1.md` [revision 4]: L3,8,12,26,210
- `docs/spec/SLEY_CLI_V1.md` [revision 4]: L7
- `docs/spec/SMP1.md` [revision 4]: L20
- `docs/spec/SMP1_JSON_BRIDGE_V1.md` [revision 4]: L7
- `docs/spec/SUCCESSION_ACCOUNTING_V1.md` [revision 4]: L3
- `docs/spec/TRANSACTION_MODEL_V1.md` [revision 4]: L4
- `fuzz/targets/complete_root_judgment.rs` [eighteen]: L6
- `fuzz/targets/root_query_engine.rs` [eighteen]: L6
- `machineresearch/sley-2.0/09-mutation-and-transaction-model.md` [eighteen]: L14,38,58
- `machineresearch/sley-2.0/25-evidence-gaps.md` [eighteen]: L43
- `machineresearch/sley-2.0/machine-summary.json` [eighteen, implementation_complete, reviews PASS]: L156,173,198,225,257,300,329,348,430,469,609,703,731,780,800,825,862,897,915,1004,1024,1051,1091,1184,1219,1244,1288,1307,1338,1376,1478,1514,1536,1562,1586,1614,1651,1813,1844,1865,1891,1922,2182,3823,3838,4325,4398,4423,4513,5440,5442,5455,5639,6647,7036,7088,7177,7252,7300,7346,7389,7396
- `machineresearch/sley-2.0/prototypes/check_s20_530_acceptance_anchor.py` [implementation_complete]: L152
- `machineresearch/sley-2.0/reweave/rw-040-m2-gap-list-slice-2.json` [implementation_complete]: L43
- `machineresearch/sley-2.0/reweave/rw-090-test-case-schema-projection.md` [eighteen]: L25
- `machineresearch/sley-2.0/sley2-development-checkpoint-2026-08-30.md` [implementation_complete]: L136
- `scripts/build_finding_register.py` [revision 4]: L9,1727
- `scripts/check_candidate_contract_freeze.py` [eighteen]: L22
- `scripts/check_capability_token.py` [implementation_complete]: L143
- `scripts/check_cli_contract.py` [implementation_complete]: L191
- `scripts/check_complete_entity_impact_profile.py` [implementation_complete]: L226
- `scripts/check_complete_root_index_snapshot_profile.py` [implementation_complete]: L135
- `scripts/check_context_capsule_profile.py` [implementation_complete]: L135
- `scripts/check_decision_dossier.py` [implementation_complete]: L187,327
- `scripts/check_finding_register.py` [implementation_complete]: L167,267
- `scripts/check_local_completion_frontier.py` [implementation_complete]: L81,83,96,171
- `scripts/check_merge_spec.py` [implementation_complete]: L203
- `scripts/check_mutation_schema.py` [eighteen]: L75,102
- `scripts/check_mutation_value_codecs.py` [implementation_complete]: L356
- `scripts/check_raw_baseline_runner.py` [implementation_complete]: L134
- `scripts/check_ref_branch_contract.py` [implementation_complete]: L185,197,304,305,306,307,411
- `scripts/check_release_candidate_packaging.py` [implementation_complete]: L195
- `scripts/check_repository_exchange_spec.py` [implementation_complete]: L196
- `scripts/check_reproducibility_and_independent_conformance.py` [implementation_complete]: L360,516
- `scripts/check_root_backed_query_profile.py` [implementation_complete]: L235
- `scripts/check_s20_530_acceptance_anchor.py` [implementation_complete]: L62,253
- `scripts/check_s20_530_crash_recovery.py` [implementation_complete]: L10036,10068,10078,10100,42433,42435,42458
- `scripts/check_schema_epoch_spec.py` [implementation_complete]: L50
- `scripts/check_semantic_comparison_spec.py` [implementation_complete]: L236,263
- `scripts/check_session_handle_profile.py` [implementation_complete]: L481
- `scripts/check_smp1_json_bridge_contract.py` [implementation_complete]: L262
- `scripts/check_standards_sbom_and_provenance.py` [implementation_complete]: L243,448
- `scripts/check_succession_accounting.py` [implementation_complete]: L135
- `scripts/check_vm_extended_opcode_profile.py` [implementation_complete]: L174
- `scripts/generate_complete_entity_impact_fixtures.py` [eighteen]: L86
- `scripts/generate_complete_root_index_snapshot_fixtures.py` [eighteen]: L70
- `scripts/generate_mutation_schema.py` [eighteen]: L257
- `scripts/generate_smp1_json_bridge_table.py` [revision 4]: L30,55
- `scripts/run_s20_530_validation.py` [implementation_complete]: L1913
- `scripts/test_current_contract_review.py` [implementation_complete]: L139,157,160,178,181
- `scripts/verify_s20_530_accepted_state.py` [implementation_complete]: L63

Non-domain hit lines: 566; domain hit lines (section 8 table): 19.
