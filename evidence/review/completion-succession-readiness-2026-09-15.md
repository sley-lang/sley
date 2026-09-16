# Independent succession readiness review — 2026-09-15

## Conclusion

The current runner cannot execute or verify a complete, genuine succession campaign. Authorization removes an operator gate; it does not supply the missing implementations. There is useful reusable infrastructure, but finishing requires product work, executable fixtures and semantic oracles, live adapters, evidence verification, accounting extensions, real trials, and final release validation. It is not honest to promote the current smoke claims or to record all GA requirements as review-only.

This review is read-only against `/home/gfarch/Work/workspaces/sley2`. No harness, model, benchmark, or external process was launched. This report is the only authored artifact. The original goal reviewed is `/home/gfarch/Work/workspaces/greyforge-managed-home/machineresearch/Sley2.0mastergoal.md`; repository README retains an obsolete location.

## Minimum honest campaign

1. All 15 frozen corpus tasks, each through **raw_files**, frozen **sley_1_2_0**, and **sley_2_0**. Zerolang is optional if unavailable, and its actual availability must be reported.
2. Both **small-model and large-model** runs. This is explicit in master goal §16.6 and S20-640, and `docs/WORK_PACKAGES.md:61`. Within each model comparison, use the same exact pinned model version, configuration, task statement, trial count/seeds, hardware, cache policy, retry policy, and action/context/wall budgets across all three arms.
3. One seed per model would be the arithmetic minimum of **90 attempts** (15 × 3 × 2); the goal does not stipulate a numeric minimum seed count. This is not a statistical sufficiency endorsement. Freeze the repetition count and analysis before running; multiple seeds are appropriate for the explicit S20-640 statistical/replay audit. Five seeds would produce 450 attempts. Do not decide the sample size by stopping when a threshold first passes.
4. Equivalent executable initial fixtures for each task and arm, with the frozen intent unchanged. There are currently task descriptions, not the required 45 runnable task/arm fixtures. The three oracle cases for clamp, for example, are not sufficient alone to prove preservation of all unrelated entities or absence of signature changes: structural invariants and exact before/after facts must also be checked.
5. An independent strict oracle evaluating the actual resulting program, execution, transactions, policy, tests, and collateral changes. A model's own acceptance declaration or a digest of its output is not an oracle. The Sley 2 adapter must actually use its native semantic store and SMP1; a Python/JSON simulation producing intended answers cannot establish the native product's succession.
6. Preserve every scheduled attempt, failure, timeout, provider error, prompt, tool call, candidate, before/after root, receipt, execution/test report, token accounting receipt, and environment manifest. Retrying under a frozen policy must not erase the failed attempt. Preserve genuine failures across later product improvements rather than replacing them with repaired successes.

There is no guarantee that honest trials will pass the improvement thresholds. In particular, strictly lower collateral in a multi-entity class is impossible against a legacy result that already has zero collateral everywhere. Likewise, zero median repairs in both arms cannot provide a 25% reduction; the alternative accepted-change improvement must then pass. The user can authorize work, but cannot make these empirical statements true by approval.

## Concrete implementation gaps

| Gap | Direct evidence | Consequence / required work |
|---|---|---|
| Raw execution is not implemented | `bench/raw/runner.py:35-38`: `offline_injected`, external commands `forbidden`, and all observations unverified; README explicitly excludes provider, oracle, raw-workspace copier, and artifact verifier | Add a separately versioned live campaign boundary preserving the existing offline contract. Implement actual raw fixture staging, bounded file/edit/check tools, model calls, oracle, and process isolation. Do not simply change the evidence-status string. |
| Legacy execution is version smoke only | `bench/legacy/README.md`; `bench/legacy/runner.py` stages frozen archive and executes only `bin/sley --version` | Implement structural task commands against the exact preserved artifact in a disposable workspace, plus real containment and evidence collection. Do not use the separately owned live Sley 1.2 checkout or copy its source into Sley 2. |
| Sley 2 runner is a proposal/read-only measured surface | `bench/sley2/runner.py:97-154`: `ARM_AFFORDANCES` excludes commit, execute, tests, branches, merge, import/export; `run_scripted_trial` at :903 onward has no acceptance/commit driver | A versioned campaign runner must provide an explicit bounded task state machine. Either expose the necessary narrowly authorized methods or let the harness carry out model-requested commits/testing with full accounting. No hidden semantic repair by the harness. Keep bulk-store dumping denied. Stale/merge/corruption tasks need multiple-session/branch/import orchestration. |
| Sley 2 model/oracle implementations are absent | `bench/sley2/README.md`; `run_scripted_trial` accepts injected Protocols and smoke agent attempts no task | Implement live provider adapter with exact identity and usage receipts, strict oracle, clocks/timeouts, and bounded trace/artifact store. Current adapter is asked only `run(handle)`; frozen task/prompt exposure needs to be explicit and verified. |
| Selected-test commits are explicitly unsupported in the real product | `crates/sley-txn/src/repository.rs:1892` rejects nonempty selected tests; `crates/sley-txn/src/codec.rs:721` rejects selected tests or test-result refs with `TXN_TEST_EVIDENCE_UNSUPPORTED` | Implement validated, bound test execution and test-result/receipt persistence through validation, commit, import, recovery, and replay. Required CREATE/SIG/TEST/MERGE tasks include test requirements. A separate Python test passing cannot substitute for canonical native tests and their receipt binding. |
| Protocol test endpoints are reserved/refused | `crates/sley-protocol/src/server.rs:978-986`: `TestsSelected` and `TestsAffected` return unsupported | Implement thin dispatch to the test owner, including stable protocol contracts and conformance. Do not move test semantics into the bridge. |
| Remaining semantic profiles need implementation/design | `docs/spec/CONTRACT_TEST_PROFILE_V1.md`, `docs/spec/EPOCH_MIGRATION_POLICY_V1.md:129-134`; bootstrap opcode exclusions in `crates/sley-vm/src/bootstrap.rs:1024-1039` | Audit current owner implementations before inheriting stale frontier prose. Epoch 1 permanently rejects four contract kinds and `test_observe`; the successor epoch must preserve existing epoch meaning. Production fingerprint requirements are a new profile, not inherently an epoch. Capability narrowing/effect execution require actual owner semantics. Existing adapter-invoke/bootstrap implementations must be reused and assessed rather than assumed absent from old prose. |
| No legacy chain verifier | `bench/accounting/report.py:535-554`, reserved `legacy/claims.jsonl` | Add a genuine legacy chain producer/verifier and register it. Accounting intentionally refuses a premature legacy chain. |
| No artifact/provenance verification promotion | raw/Sley2 claims explicitly `UNVERIFIED_INJECTED_DIGEST_CLAIMS`; accounting `derive_evidence_status` derives from status strings but does not inspect model/oracle artifacts | Build a verifier resolving every referenced digest to immutable actual bytes and checking model usage, tool traces, oracle inputs/results, roots, and receipts. Only its verified output can feed a live verifier. Anchoring/durable custody must cover chain truncation as well as mutation. |
| Three mandatory thresholds cannot ever pass in current accounting | `bench/accounting/report.py:73-78` and :367: required-check bypass, collateral comparison, mutation reconstruction are emitted `NOT_EVALUATED` | Add explicit measured evidence and derivations for all three. Preserve threshold meanings; do not delete their rows. `scripts/build_ga_acceptance_report.py:181-239` correctly demands exact coverage, every row PASS, complete arms, and verified evidence. |
| Section 22 extends beyond current numeric accounting | Master §22.5–22.7 | Bind repeated/independent encoding, pack reconstruction, VM determinism, ancestry reconstruction, and kernel-coherence evidence to the final implementation/candidate. Optional external-arm policy is not a numeric proxy. |
| Product/release gates are placeholders | `scripts/gate_status.py` always emits `NOT_IMPLEMENTED` and exits 2; `scripts/check_local_completion_frontier.py` expects that historical placeholder | Implement real orchestrated `v2` and `release-check` gates after product inputs exist; update pinned historical checks through an explicit reviewed contract change. Returning success unconditionally would falsify completion. |

## Recommended implementation order

### 1. Publish an accurate dependency ledger immediately

Reconcile the GA report's 17 `AWAITS_REVIEW` labels and frontier claims with concrete missing product paths above. Record user authority separately from technical satisfaction. Keep the already completed two-host reproducibility and scoped Council repairs closed. Do not repeat old build work merely because stale prose says pending.

### 2. Define and implement the native test/production execution boundary

First deliver one end-to-end canonical TestCase through selected/affected test selection, execution report, validation, accepted commit receipt, export/import, and replay. Then cover test failures, stale roots, report substitution, and crash/recovery. Resolve epoch/profile decisions explicitly; preserve epoch-1 vectors and accepted artifacts. Complete required effect/capability and production fingerprint behavior. This is the highest-risk product dependency for the frozen corpus.

### 3. Build executable fixtures and independent oracle before model spend

For every frozen task, write equivalent arm fixtures and a positive known-correct result plus deliberately incorrect variants. Prove that the oracle rejects each forbidden outcome, not merely wrong sample output. Use native Sley 2 graph/VM/transaction paths, including the 10,000-entity bounded-context fixture. Freeze oracle/fixture digests only after independent review. Fixture/oracle bug fixes require a new manifest and preserved earlier version; semantic task changes require corpus v2, retaining v1.

### 4. Implement a live campaign package with explicit custody

Reuse canonical manifests, digest chains, trace codecs, frozen artifact verification, and exact-ratio accounting, but introduce a distinct reviewed live contract. Implement all three adapters, containment, usage receipts, create-only artifacts, externally anchored terminal receipts, and independent evidence verification. Add task workflows for commit/test/merge/stale/corrupt cases without turning the harness into the agent's solver. Failures must become retained records even if provider/harness failure occurs before model output.

### 5. Finish measured accounting and audit methodology

Add legacy verification and the three currently uncomputed conditions. Include all-attempt and explicitly labeled non-harness-failure companion statistics, as the existing accounting contract requires. Produce a frozen small/large campaign manifest including model IDs, budgets, scheduling/cache rules, seeds, retry policy, and expected slots. Review fairness using equivalent training material; avoid giving native prebuilt semantic answers that raw/legacy must rediscover.

### 6. Execute, verify, improve honestly if necessary

Run all preregistered slots. Recompute every acceptance from retained artifacts using the independent verifier; derive exact metrics and the complete threshold table. Record negative results. If thresholds fail, improve the product using a new explicit campaign version while preserving the failed campaign; do not weaken thresholds or selectively re-run failed slots into the old denominator.

### 7. Close release against the actual final source

Run complete `v2` and `release-check`, repeat final candidate builds on both hosts only after new source stabilizes, bind SBOM/provenance/content checks and all actual demonstrations, obtain final independent review, and derive the final decision dossier. §20.12 requires the packaged artifact to create, modify, execute, test, branch, merge, export, and import without source; the earlier 12-step scoped demo must be checked against that complete requirement. Only then record a truthful PASS/ALPHA_COMPLETE/FAIL decision and perform authorized publication steps appropriate to that decision.

## Feasibility statement

The existing machinery is a sound starting point for a real implementation. **It is not a runnable completion solution today.** Live adapters and fixtures are missing, and native canonical-test commit support is an actual product gap. No read-only review can promise that a finite run will prove superiority. The next useful implementation unit is canonical test execution through accepted transaction receipts, alongside fixture/oracle design; spending on a full model campaign before that would mostly measure known unsupported paths.

The original goal expressly says that failure to meet succession thresholds leaves Sley 2 alpha regardless of implementation completeness (§26.7). Neither broad operator approval nor zero open scoped review findings overrides that empirical condition.
