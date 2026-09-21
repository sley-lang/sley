# Admission / discovery / gate-input closure pass — 2026-09-21 (work branch only, successor to 988edf15)

Scope: Sley only. No ZJX execution-authority change. No restart of the completed
staging split, typed driver, capture architecture, or lint repair. No mint,
signing, publication, integration, or GA claim. `ga_claimed=false` preserved.

## 0. State verified before work

- `work/succession-sley20-arm` at `988edf15d9f6f664c0d453413e503709568637d1`
  (clean tree), parent `03982595` (fuzz refresh of 7 lanes for `7bcc5a89`),
  grandparent `7bcc5a89` (D4 lint repairs). Matches the expected checkpoint,
  implementation source, and fuzz refresh. Pushed to `origin/work/succession-sley20-arm`.
- Main worktree `/home/gfarch/Work/workspaces/sley2` left dirty at `8966da2e`
  with the same 21 batch-14 repair files (later committed as `acbc65f0` on
  `work/finish-20260918`); never cleaned, reset, or moved by this pass.
- `wt2` at `acbc65f0` (`work/finish-20260918`), clean, untouched.
- Historical evidence, pending review inputs, release artifacts, checkpoint
  stores, and operator/resource holds untouched. `ga_claimed=false` verified
  in `evidence/release/ga-acceptance-report.json` at this commit.
- Toolchain pinned `1.93.0` (`rustc 1.93.0`, `cargo 1.93.0`), `--locked`,
  `CARGO_NET_OFFLINE=true` where cargo ran. Binaries reused from this source
  (no library sources touched): `target/debug/sley` sha256
  `95c4cc0b224ccd29e0f3b59294c3e2bb3a27243b3c8e2b2b6a58f2662fa62867`,
  `target/debug/deps/succ_live_judge_cases-42d77a02eb446c77` sha256
  `48c558956bb9258a78d4eba1a058992841bdb934617e15afd59c0f909612d745`
  (identical to the D4 gate record). User-owned build storage for fresh cargo
  runs: `/home/gfarch/.cache/sley-succ-native-test/target` (on `/home`,
  outside the repository); the repo `target/` fingerprints were not rebuilt.
- No source files changed in this pass (this record only), so the
  source-bound lint (`lint-R-7bcc5a89.log` PASS) and fuzz evidence
  (`fuzz-refresh-7bcc5a89.log`, 7 lanes PASS, transcribed as `03982595`)
  are reused under the governing freshness rules; nothing was rerun
  indiscriminately and no exemption was taken.

## 1. CREATE through the existing native admission route

Contract read: `docs/spec/NATIVE_TEST_ADMISSION_V1.md` (rev 5) incl. §6
lifecycle, Appendix A v2 envelopes, Appendix C SMP v3 native surfaces
(601/602 diagnostic, 605 paging, 606 replay, 607 status — all live;
commit route live in rev 5), Appendix D machine table.

- Current trial profile/payload: `bench/sley2/runner.py::PROFILE_ARGS =
  ("--protocol-profile", "v2-capable")`, `PROTOCOL_VERSION = 2`;
  `bench/live/sley2_tool.py::TOOL_METHODS` is the frozen 18-name allowlist
  (no `commit`, `merge.commit`, `exchange.import/export`, `execute`, …).
  The agent never commits. The judge harness commits through
  `bench/fixtures/sley2_live_judge.py::_commit_candidate` →
  `Session._raw_request("commit", _encode_commit_body(...))`, a legacy
  4-field payload (parent tx, principal, now, STORED bytes). Established
  by direct source trace this pass (no new feature assumed).
- Refusal owner: legacy commit admission. `crates/sley-txn/src/repository.rs`
  `commit()` refuses nonempty `validation.result().record.selected_tests`
  with `TransactionErrorCode::TestEvidenceUnsupported`
  (`TXN_TEST_EVIDENCE_UNSUPPORTED`, 39008); `crates/sley-txn/src/codec.rs`
  `build_transaction` refuses any nonempty `selected_tests`/`test_result_refs`
  the same way (unit-pinned). The CREATE single candidate (new functions +
  targeting tests) is refused there because validation auto-selects tests
  targeting affected functions. The legacy refusal was not disabled and no
  diagnostic report was substituted for commit authority.
- v3 route status: implemented. `TransactionRepository::commit_native`
  (`NativeCommitInput`) derives the stronger native plan (existing selected +
  affected-closure proposals + every created/replaced live TestCase +
  protected required), executes the selected set through the configured
  executor in raw-ID order, and commits the v2 transaction
  (`commit_profile=2, semantic_profile=3`) with the singleton native test
  report, bundle, historical context, and acceptance signature. Proven this
  pass with existing authorized test-only provisioning on disposable state:
  `cargo test -p sley-txn --lib --locked --offline native_commit_` → 25
  passed, 0 failed (incl. `native_commit_empty_selection_is_a_complete_live_positive`;
  executor-absent, stale-parent, untrusted-signer/measurement, and durability-cut
  refusals all green). Executor, acceptance signer, and both receiver trust
  manifests are one operator-provisioned unit; a call without authority refuses
  `NATIVE_SIGNER_UNAVAILABLE` before any write, and signing material was never
  exposed to the agent.
- Remaining restriction (configuration + trial-tooling scope, not a demonstrated
  production defect): the trial pins `v2-capable`/protocol 2, carries no `commit`
  in the agent allowlist, and the judge encodes the legacy commit body — so the
  v3 payload (`candidate_bytes, expected_parent, attempt_id, admission_profile`
  under negotiated v3 + native-tests bit) is never attempted. Candidate limits
  also bind single-trial shape (`sley2_tool._assemble` 64-op cap; trial
  workspaces never advance between judge runs, hence the two-harness-commit
  structure). No semantic platform change is proposed: the smallest runner-side
  integration is a v3-capable trial-profile option + native commit route with
  operator/test authority provisioning, adopted only behind frozen
  trial-profile review (never silently negotiating a different profile while
  retaining old run/tool digests). Required test-only server provisioning for a
  protocol-level single-trial proof is unavailable to the agent; recorded as the
  exact dependency. The two-harness-commit mechanics stay as component evidence,
  not single-trial acceptance evidence. Selected-test execution, stale-parent
  checks, atomic acceptance, candidate limits, and failure retention are
  preserved.

## 2. CONTEXT bounded discovery (wording reconciled, gate retained)

- Reconciliation: the de-literalized judge (any added member via pre-image diff
  + structural complete-closure check; re-emitted manifest) supersedes the
  earlier F1/Bool manifest literals. F1/Bool and the private impact-role list
  are no longer requirements; the gate wording that still names them is stale
  and must be read against `MEDIATED-INPUT-ACCOUNTING.md` ("not established":
  CONTEXT task-spec gate retained) and `SUCCESSION-COVERAGE.md`.
- Actually missing: (a) the task's intended target and change semantics in
  authorized inputs (corpus says "add a required record field", no identity or
  type); (b) a permitted starting capsule or query handle naming where discovery
  begins; (c) a usable root/snapshot-bound path to enumerate affected entities
  in a bounded way. Traced owner APIs: `crates/sley-query` (`root_query`,
  `query`, `capsule`, `context_capsule`, `snapshot`, `complete_root`),
  `crates/sley-repo` (`root_query`, `index_cache`), served as `query.root /
  query.restricted / query.continue`, `capsule`, `revision.read`, `refs.list /
  refs.resolve`, `entity.version / entity.signature`, `handle.expand` — all
  already in the trial allowlist as bounded paging routes with
  omitted/truncated/continuation and cumulative-budget accounting. None of them
  enumerates a typedef's users in a bounded permitted way: inventory is
  whole-store (forbidden by the no-whole-store rule), reads need IDs, and server
  queries need an unmintable snapshot. The deterministic stand-in still receives
  member identities as invocation arguments (disclosed scaffolding, never a
  fairness proof), so discovery itself remains the review gate.
- Contracted path (no private manifest, precomputed repair, or assembled answer
  set): the task (or a permitted starting capsule/query handle) names the
  target; the agent walks bounded capsule/query/continuation pages; an
  owner-derived impact query may return affected entities only as the contracted
  capability with visible provenance, bounds, omissions, and cost; then agent
  mutation → production admission → real oracle → append → verify. Proven
  mechanics this pass (existing capabilities, real oracle, no new surface):
  `test_mediated_access` 13/13 OK (synthetic captures: bindings, chains,
  continuations, whole-store/commit/budget fail-closed, one clean bounded pass);
  `test_judge_create_entry` 23/23 OK; mediated CONTEXT pos/incomplete-impact/
  inconsistent-continuation proofs stand on the prior integrated runs
  (`test_mediated_context.py` through `execute_attempt` + real oracle +
  `verify_attempts`). Full `execute_attempt → bounded capsule/query/continuation
  → agent mutation → production admission → real oracle → append → verify` with
  every starting fact from authorized inputs remains the pending proof, blocked
  on the exact interface decision: name the field (or starting handle) and open
  the bounded impact-enumeration route. If no existing generic mechanism covers
  it, the amendment + executable regression plan go behind the governing review
  gate; no semantic change lands here. No-whole-store, complete-impact,
  cumulative budgets, continuation behavior, and evidence-only vs
  discovery-capable success stay separated.

## 3. Release gate: restored inputs, not a rebuild (proven)

Checker read: `scripts/build_candidate_content_report.py` (`build_for_candidate`
→ `reproducibility.select_attestation(repro, candidate=candidate)` →
`build_report`). Binding is the 4-tuple commit + artifact_sha256 +
manifest_digest + artifact_size_bytes (`binds_candidate`); the selected
attestation must be admissible (REPRODUCIBLE, clean tree, no differing members).
It is not a requirement that current HEAD be main.

- Located the original matching bytes in the authorized checkpoint store
  `/home/gfarch/Work/checkpoints/sley2-candidate-69907dd/`:
  `dist/sley-2.0.0-linux-x86_64.tar.gz` sha256
  `ca7317a954729e8e2eeadc3608755f162e9cae2ef14cd52188f72d4cef7ff0b0`,
  2507942 bytes; `evidence/runtime/s20-720-release-candidate/evidence.json`
  with commit `69907ddf904a7d3df48558425273cc58179a7077`, manifest digest
  `3433cb378bad5c294fbe875e1c854e47b2ec3612a5e3a40d78684a5dd667a441`,
  result PASS — exactly matching the tracked attestation (`primary`,
  `evidence/release/reproducibility-report.json`) and tracked content report
  (`evidence/release/candidate-content-checks.json`, PASS) at this commit.
- Restored only those two ignored inputs into the disposable gate worktree
  `/tmp/sley-gate-restore-988edf15` (detached at `988edf15`; main and wt2 never
  touched) and ran `python3 scripts/build_candidate_content_report.py --check`
  → `{"contract": "sley2.candidate-content-checks.v1", "result": "PASS"}`,
  exit 0. Control: the same restore into a disposable worktree at the source
  commit `69907ddf` (whose tracked report still binds the previous candidate
  `02756fb1`) correctly refuses (`no admissible attestation binds the built
  candidate`, exit 1) — confirming the binding is per-candidate, not per-HEAD.
- Nothing was fabricated, borrowed across bytes/commits, rebound to this
  work-branch source, minted, or rebuilt; tracked release records were not
  rewritten (`--check` only, disposable tree). Correction to the blanket
  explanation: the gate does not "fail on every work branch" as such — it fails
  wherever the ignored runtime evidence/artifact bytes are absent (fresh trees),
  and passes wherever the exact preserved bytes are restored, regardless of
  branch. Scope explicit: this PASS proves the preserved artifact's content
  (candidate `69907ddf` / artifact `ca7317a9…`); it does not qualify `988edf15`
  as a newly built release candidate. Both disposable worktrees were removed
  after the runs (`git worktree remove --force`).

## 4. Environmental check failures resolved where possible

- `SLEY2_MASTER_GOAL` located through the continuation records:
  `/home/gfarch/Work/workspaces/greyforge-managed-home/machineresearch/Sley2.0mastergoal.md`
  (sha256 `077913685537541154a6da758fd64a9997d9e023952ba03e28527a5bfd74aace`,
  identical to `/home/gfarch/machinelibrary/Sley2.0mastergoal.md`). No
  replacement master goal was created. With the variable set, both affected
  checkers PASS at this commit, exit 0:
  `scripts/check_transaction_contract.py` → PASS
  (`s20-390-restricted-atomic-commit-v1`); without it → FAIL
  `master:unavailable:…:set-SLEY2_MASTER_GOAL`, exit 1 (original failure
  preserved). `scripts/check_candidate_result_contract.py` → PASS
  (`s20-360-restricted-candidate-validation-v1`, 16 decisions, 37 validator
  symbols); without it → `master:unavailable`, exit 1 (original failure
  preserved).
- `succ_debug_commit::debug_commit_repro` (`crates/sley-repo/tests/succ_debug_commit.rs`):
  documented diagnostic only — requires `SUCC_DEBUG_REPO` + `SUCC_DEBUG_CANDIDATE`
  (+ optional `SUCC_DEBUG_PRINCIPAL`) and panics intentionally after printing.
  No fixture environment is specified or authorized in this pass, so it was not
  run; its explicit not-run disposition is retained under the owning test
  policy. The scoped suite `cargo test --workspace -- --skip debug_commit_repro`
  evidence from the D4 gate (74 result lines, all ok) is cited as scoped only —
  never labeled an unfiltered full-suite pass — and both the original failure
  and the scoped result are preserved.

## 5. Focused validation at this successor (no source changes)

| Check | Exact command (workdir wt-succ unless noted) | Exit | Result |
|---|---|---|---|
| Judge/create entry regressions | `python3 -m unittest bench.live.tests.test_judge_create_entry` | 0 | 23 tests OK |
| Mediated access audit regressions | `SLEY2_SLEY_BINARY=…/target/debug/sley python3 -m unittest bench.live.tests.test_mediated_access -v` | 0 | 13 tests OK |
| Native commit route (test-only provisioning, external target) | `CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/home/gfarch/.cache/sley-succ-native-test/target cargo test -p sley-txn --lib --locked --offline native_commit_` | 0 | 25 passed, 0 failed |
| Legacy refusal pin | `… cargo test -p sley-txn --lib --locked --offline transaction_rejects` (codec `selected_tests` → `TXN_TEST_EVIDENCE_UNSUPPORTED`) | 0 | 1 passed |
| Transaction contract (env-gated) | `SLEY2_MASTER_GOAL=…/Sley2.0mastergoal.md python3 scripts/check_transaction_contract.py` | 0 | PASS |
| Candidate-result contract (env-gated) | `SLEY2_MASTER_GOAL=… python3 scripts/check_candidate_result_contract.py` | 0 | PASS |
| Formatting | `cargo fmt --all --check` | 0 | PASS |
| Release content gate (restored inputs, disposable tree) | `python3 scripts/build_candidate_content_report.py --check` (in `/tmp/sley-gate-restore-988edf15`) | 0 | PASS (preserved artifact only; see §3) |
| Full `make quick` / `make lint` aggregates | — | not run | Not claimed; D4 evidence reused per freshness rules (§0). No exemption taken |

Review dispatch: none in this pass (no source changes, no new review packets).
Provider availability verified live rather than assumed: `codex-cli 0.155.1`
and `claude 2.1.273 (Claude Code)` binaries present; no quota assumption
carried, and no verdicts were fabricated or substituted.

Commits: this record only, to `origin/work/succession-sley20-arm`. Main stays
dirty at `8966da2e`; wt2 at `acbc65f0`. Independent work may continue while any
dependent review is held.

## Remaining prerequisites (unchanged holds)

Co-commit runner integration + frozen trial-profile review (§1); CONTEXT
interface decision + full bounded-discovery proof (§2); no new release build,
operator release/GA decision, signing, publication, lab/second-host attestation,
AR-02/R2/C1, EFFECT/CAP rev16, TYPE main adoption, DEAD tombstone semantics,
MODULE/MERGE/CORRUPT dispositions, or live-model campaign evidence — all held.
Component proof, complete trial capability, independent review, release
qualification, and campaign evidence remain separate.
