# Review + gate pass — 2026-09-22 (work branch only, successor to c1e8db56)

Scope: Sley only. No ZJX execution-authority change. No mint, signing,
publication, integration, or GA claim. `ga_claimed=false` preserved.
No source files change in this successor (REQ-10 rev5 packet, verbatim
Nabu rev5 transcript, and this record only); staging, capture,
typed-driver, lint inputs, and fuzz targets untouched. No restart of
completed work.

## 0. State verified before work

- `work/succession-sley20-arm` at `c1e8db56b15b41c9c90aaa8ec927015b6bcbe204`
  (local == `origin/work/succession-sley20-arm`), clean tracked tree.
  Newer legitimate work over the expected `b0e52e00` preserved: the
  aggregate-review successor (8 dispatches, 8 verdicts, lint PASS,
  130-line quick with 5 investigated failures; REQ-09 cleared by Ariadne
  rev3 PASS; REQ-10 settled at Nabu rev4). Sources identical between
  `b0e52e00` and `c1e8db56` (records-only delta); Nabu rev5 re-verified
  the delta (`git diff --name-status b0e52e00 c1e8db56`: 15 added files,
  zero Rust/Python/spec/fuzz).
- Main worktree `/home/gfarch/Work/workspaces/sley2` at `8966da2e`,
  dirty (21 files), never touched. `wt2` at `acbc65f0`
  (`work/finish-20260918`), clean, untouched.
- Historical evidence, checkpoint stores, release artifacts, pending
  review inputs, operator/resource holds untouched. `ga_claimed=false`
  verified in `evidence/release/ga-acceptance-report.json`.
- Batch-14 inputs and whole-batch recording procedure untouched; no new
  interface review mixed into that batch (scope `8966da2e`, index
  `batch14-index-8966da2.json` unchanged). This pass dispatches only the
  succession-scope REQ-10 rev5 via `forge-council exec`, separate jobs
  scope, single dispatch, no polling loop, no substitution.
- Toolchain pinned `1.93.0`, `--locked`, `CARGO_NET_OFFLINE=true`,
  `CARGO_TARGET_DIR=/home/gfarch/.cache/sley-agg-b0e52e00/target` (lint)
  and `/home/gfarch/.cache/sley-succ-native-test/target` (txn tests),
  both on /home outside the repository; repo `target/` fingerprints not
  rebuilt. `SLEY2_MASTER_GOAL=.../greyforge-managed-home/.../Sley2.0mastergoal.md`
  (sha256 `07791368...aace`); no new master goal created.

## 1. Review submissions (actual requests, pinned inputs, verdicts)

Prior (preserved, c1): REQ-09 rev1 (`97e50c9b`) → Ariadne FAIL
(`2fee4a94`); REQ-10 rev1 (`9f953627`) → Nabu REVISE (`2eb8fef9`);
REQ-09 rev2 (`dd9a02ce`) → Ariadne FAIL, priors CLOSED, new NTA-P1-2
(`76503db0`); REQ-10 rev2 (`a7aec1e6`) → Nabu REVISE narrowed
(`0ea0d788`); REQ-09 rev3 (`248852dc`) → Ariadne PASS
`PASS_QUALIFYING_NO_P0_NO_P1` with new NTA-P2-3 qualify, CLEARED FOR
IMPLEMENTATION (`321cfc06`); REQ-10 rev3 (`b817d219`) → Nabu REVISE
single-issue (`82caefdc`); REQ-10 rev4 (`f3b64457`) → Nabu REVISE
0P1/3P2/2P3, architecture settled (`09d41abc`). 8 dispatches,
8 verdicts, no failures. Batch-14 untouched.

This pass (1 dispatch, 1 verdict, no failure):
- Packet `evidence/review/requests/REQ-10-rev5-context-bounded-discovery.md`
  sha256 `ace99e91...4d4a7fbc3`, baseline `c1e8db56`, Nabu lane. Answers
  all five rev4 items in one record: P2-1 allowlist move + spec §9 edit
  + checker count + revision-5 bump (precedent
  `docs/audits/PHASE3_V2_OFFER_DESIGN.md:75-81`); P2-2 encoder scoping
  (field 9 on `workspace.open` path only,
  `crates/sley-protocol/src/server.rs:3244-3259` shared by `:2502` and
  `:2891`); P2-3 `TOOL_METHODS` extension + relay replacement + pin test
  or claim strike; P3 Merlin probe ownership
  (`docs/WORK_PACKAGES.md:35`, `index_cache.rs:108/242/131/203/221/288`);
  P3 denial-rationale rewrite (`runner.py:115-117`,
  `bench/sley2/tests/test_runner.py:541-546`). Concrete recommended
  change with exact identities and acceptance criteria; not open-ended.
- Dispatch: `forge-council exec nabu "<packet>" --timeout 1800
  --execute` (engine resolved: hermes; dry-run first printed the argv
  without a model call, then executed once). No auth/quota/provider
  failure. No retry loop, no reviewer substitution, no fabricated
  verdict.
- Verdict: Nabu REVISE `REVISE_0_P0_1_P1_2_P2_1_P3_1_P4`, transcript
  `evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-rev5-c1e8db56.md`
  sha256 `18f2b591...cd729012e3` (verbatim; dispatch log preserved as
  `resume-20260918/review-REQ10r5-nabu.log`). Architecture remains
  settled — the P1 is governance, not design; all five rev4 items
  verified correct against source. New items for rev6 (exact pending
  amendment, §3): P1 summary-revision/contract-drift + stale-PASS
  over-claim; P2 harness/agent split + enumeration exhaustiveness; P3
  `:8`→`:10` citation regression.

## 2. CREATE through the existing native route (exact remaining dependency)

- Legacy refusal unchanged and pinned: `codec.rs:740`
  (`build_transaction` refuses nonempty `selected_tests`) and
  `repository.rs:2241` (`commit()` refuses validation-selected tests)
  with `TXN_TEST_EVIDENCE_UNSUPPORTED` (39008). Pin test
  `codec::tests::transaction_shape_rejects_self_equal_binding_and_test_claims`
  → 1 passed, exit 0 (this pass, external target dir). No weakening,
  no diagnostic substituted for commit authority.
- v3 route health (test-only provisioning, disposable state): `cargo
  test -p sley-txn --lib --locked --offline native_commit_` → 25
  passed, 0 failed, exit 0 (this pass). Executor, acceptance signer,
  and receiver trust remain one operator-provisioned unit; calls
  without authority refuse `NATIVE_SIGNER_UNAVAILABLE` before any
  write; release keys never used, signing material never exposed.
- Contract cleared (Ariadne rev3 PASS); NTA-P2-3 fold-in specified
  (stale probe field[3] pinned to `fixed_native_admission_profile().id()`).
  Implementation reviews (Vulcan authority provisioning, Nabu executor
  wiring) remain deferred follow-ons.
- BLOCKER for the integrated proof (physical, re-verified this pass):
  no executor on this host can genuinely execute authored tests —
  `/usr/lib/sley`, `/etc/sley-test-supervisor`,
  `/run/sley-test-supervisor` all absent; the only
  `NativeTestExecutor` impls are test-only stubs (`CountingExecutor`,
  `RejectingExecutor` in `repository.rs` test module); no
  worker-outcome→`ExecutedNativeTest` bridge exists. The
  `execute_attempt → mediated agent actions → production native
  admission → trusted oracle → persisted attempt → verification`
  proof with real execution of authored tests (plus wrong-expectation,
  authority, stale, failed-execution negatives) cannot run here until
  a genuine executor is provisioned (host supervisor install =
  operator/infra authority, or a Vulcan-reviewed trial executor).
  Permitted preparatory work only this pass; no source change made.

## 3. CONTEXT discovery (exact pending amendment)

- De-literalized judge and mediated capture route retained; F1/Bool and
  private impact roles stay retired. Witness manifest-literal reads
  (`succ_witness_context.py:65-67`) are what the interface replaces.
- Nabu rev5 verifies every rev4 answer correct (move-not-copy
  disjointness/coverage, `v2/methods.json:49` + `V2_ALL` un-reserved,
  encoder sharing/field-9 freedom, Merlin predicates, over-broad
  denial, precedent exactness) and settles the architecture, but
  returns REVISE for record completion. Implementation stays gated
  behind Nabu sign-off; no semantic change lands here.
- Exact rev6 amendment (from the rev5 verdict, to be authored as
  `REQ-10-rev6`, same record): (i) set
  `sley2_trial_runner.contract_revision` to 5 in the same commit and
  add the two-line spec-vs-summary revision comparison at
  `check_sley2_trial_runner.py:291` on the
  `check_root_backed_query_profile.py:226-227` pattern (closes the
  drift class permanently); revision-qualify the three
  `S20_620_COMPLETE` review fields or move status off COMPLETE for the
  duration so revision-4 PASSes stop covering a revision-5 surface;
  (ii) state the harness/agent split explicitly — harness retains its
  own head read for envelope assembly + head-movement invariant
  (`sley2_tool.py:256-267` populating `_head`, consumed at
  `:381-382/:424/:482/:718/:751/:754/:876` and
  `mediated_sley.py:137`), agent gains `workspace.open` as its own
  afforded route with `:718` no longer fed a harness-supplied tx;
  (iii) make the phrasing enumeration exhaustive (add at least
  `bench/sley2/tests/test_runner.py:335`,
  `check_sley2_trial_runner.py:134/:236`,
  `S20_620_SLEY2_TRIAL_RUNNER_CLOSEOUT.md:5`; frozen-history sections
  `:124/:140/:156` and `PHASE3_V2_OFFER_DESIGN.md:163` defensibly left)
  or mark it a floor; (iv) fix the `:8`→`:10` citation. Then Nabu
  sign-off (no redesign expected) → implement (afford `workspace.open`,
  field 9 head-pinned, Merlin probe, tooling, task-input typedef
  amendment) → integrated
  execute_attempt→discovery→mutation→admission→oracle→append→verify
  proof incl. all five negative classes → Vulcan/Ariadne follow-ons.

## 4. Aggregate checks with correct inputs

| Check | Exact command (workdir wt-succ unless noted) | Exit | Result |
|---|---|---|---|
| Formatting+clippy+checkers | `SLEY2_MASTER_GOAL=... CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/home/gfarch/.cache/sley-agg-b0e52e00/target UV_OFFLINE=true make lint` | 0 | PASS (`fmt_clean`, 0 clippy warnings, commit `c1e8db56`, `lint_inputs_clean`; 2 new untracked review paths noted immaterial) |
| Native commit route | `CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/home/gfarch/.cache/sley-succ-native-test/target cargo test -p sley-txn --lib --locked --offline native_commit_` | 0 | 25 passed, 0 failed |
| Legacy refusal pin | `... cargo test -p sley-txn --lib --locked --offline transaction_shape_rejects_self_equal_binding_and_test_claims` | 0 | 1 passed (`TXN_TEST_EVIDENCE_UNSUPPORTED`) |
| Full `make quick` (130-line) | — | not rerun | Reused `quick-Skeep-b0e52e00.log` per freshness rules: no Rust sources, fuzz targets, or lint inputs changed since (this successor adds records only). Candidate-bound rows (#84/#85/#88/#105) stay fail-closed until operator-owned refresh/re-attestation. No exemption taken |
| Fuzz lanes | — | not rerun | D4 `fuzz-refresh-7bcc5a89.log` (7 lanes PASS) remains fresh, same reason |
| Release content gate | — | not rerun | `b0e52e00` restored-input PASS (`69907ddf` bytes, scope explicit: preserved artifact only) reused; this pass mints, rebuilds, rebinds nothing |
| `succ_debug_commit::debug_commit_repro` | — | not run | Diagnostic-only disposition retained under owning test policy; scoped `--skip` evidence cited as scoped only |

## 5. CHECKPOINTED_IN_PROGRESS — exact next actions

1. CONTEXT: author `REQ-10-rev6` answering rev5's P1/P2s per §3 (summary
   field + drift assertion; revision-qualified or de-completed status;
   harness/agent split; exhaustive-or-floor enumeration; `:10` fix)
   with Merlin probe ownership → `forge-council exec nabu ... --execute`
   sign-off → implement → integrated discovery-and-repair proof incl.
   all five negative classes → Vulcan/Ariadne follow-ons.
2. CREATE: implement the cleared delta incl. NTA-P2-3 fold-in →
   provision a genuine executor (operator/infra install or
   Vulcan-reviewed trial executor) → integrated proof with real
   execution of authored tests (wrong-expectation/authority/stale/
   failed-execution negatives) → Vulcan/Nabu implementation reviews.
3. Revalidate any code-landing successor with §4 aggregates (lint +
   full 130-line quick + affected fuzz lanes); candidate-bound records
   stay fail-closed until operator-owned refresh/re-attestation.

Remaining holds (unchanged owners): MODULE/MERGE/CORRUPT dispositions,
TYPE main adoption, DEAD tombstone semantics, EFFECT/CAP rev16, lab
coordination, AR-02, R2, C1, release signing/publication/GA, campaign
evidence. Component proof, trial capability, independent review,
release qualification, and campaign evidence remain separate.
