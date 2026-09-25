# S20-530 post-acceptance aging decision (2026-09-02)

Status: OPERATOR DECISION REQUIRED (prepared by the Claude orchestrator; no
bound path was changed to prepare it)

Owner: operator (successor rule); Claude orchestrator (preparation)

## Why a decision is needed before any further development

The v13 acceptance (acceptance commit `034cc75`, validated commit `8f7c763`)
is complete, but the gate it leaves inside `make quick` freezes the whole
repository. Verified against the committed checker
(`scripts/check_s20_530_crash_recovery.py`, contract digest `839459ca…`):

- With `implementation_complete` true, `execution_binding_problem` (called
  from `require_implementation`) requires, at HEAD: a clean tree with no
  untracked files (`workspace_input_dirtiness`, which counts every path
  except the three output paths); the validated commit as an ancestor; no
  changed path between the validated commit and HEAD other than the output
  paths and the two narrative lanes (`non_output_changes_since`); and current
  bytes and modes of every non-output, non-narrative path equal to the
  validated commit's blobs (`workspace_input_hashes` covers every tracked and
  untracked non-ignored file).
- Therefore any later commit that touches any crate, script, spec, ADR,
  Makefile, lockfile, conformance, oracle, bench, evidence, or `docs/audits`
  file fails the checker with `non-output input changed after validation` and
  so fails `make quick`. Only `machineresearch/` and `docs/WORK_PACKAGES.md`
  may change (v12 rule).
- Independently of that workspace binding, the public-API phase
  (`public_api_baseline_problem`, reached through
  `require_public_recovery_api`) requires the production Rust source-path
  inventory of `sley-store`, `sley-txn`, and `sley-repo` to equal exactly the
  frozen list and the unconditional module declarations of
  `crates/sley-repo/src/lib.rs` to be exactly `gc` and `refs`. Any new module
  in an owner crate fails that phase even if the workspace binding were
  relaxed. S20-540 (pack exchange), S20-510 (comparison), and S20-520 (merge)
  all add modules to `sley-repo`.
- The first post-acceptance obligation already collides with the gate. The
  local completion frontier still records S20-530 as pending in three places:
  `docs/audits/S20_LOCAL_COMPLETION_FRONTIER.md` (bound audit doc),
  `scripts/check_local_completion_frontier.py` (bound script, hardcodes
  `s20_530_implementation_complete: False` and `next_authority_safe_package:
  S20-530-CRASH-INJECTION-AND-RECOVERY`), and the `local_completion_frontier`
  and `s20_700_remaining_surface_audit` sections of `machine-summary.json`.
  The frontier checker passes at HEAD only because it does not cross-check the
  S20-530 section; re-anchoring it requires editing a bound doc and a bound
  script.

Precedent: S20-390 and S20-500 age by marker checks. Their checkers
(`check_transaction_contract.py`, `check_ref_branch_contract.py`) bind file
presence and text markers, not source bytes, and their closeout evidence is
historical at its commit. S20-530 is the first package whose checker binds
the whole workspace after acceptance; the v12 amendment relaxed it only for
the narrative lanes because the campaign needed to land its own checkpoints.

## Options

### A. Historical acceptance (recommended)

The accepted S20-530 state is a fact about commits `8f7c763` (validated) and
`034cc75` (acceptance, final checker confirmation recorded in the stop
checkpoint, final checker PASS at `cc0f92f` on 2026-09-03T01:06Z). Verify it
there, not at HEAD.

1. `make quick` replaces the `check_s20_530_crash_recovery.py` line with a
   new light anchor check, `scripts/check_s20_530_acceptance_anchor.py` (new
   file, not bound by any frozen digest). It verifies: `8f7c763` and `034cc75`
   are ancestors of HEAD; the closeout evidence, the 28 retained logs, the
   v13 freeze evidence, the test plan, the spec, the ADR, the checker, the
   runner, and the reconciler at HEAD are byte-identical to their blobs at
   `034cc75`; the `s20_530_crash_recovery` section of the machine summary
   still records `implementation_complete: true`, contract set `0257eddd…`,
   freeze commit `25055783…`, validated commit `8f7c763…`, source set
   `468490e1…`, and the six receipt session ids; and
   `docs/WORK_PACKAGES.md` still carries the S20-530 row markers.
2. The full checker stays runnable as the authoritative historical gate:
   a new `make s20-530-verify` target clones the repository with
   `git clone --no-hardlinks` into a scratch directory, checks out the
   acceptance-confirmed commit, copies the main repository's exact
   `.git/config` (the checker freezes it), and runs the no-argument checker
   there (about 95 minutes). It must print
   `S20-530 crash-recovery contract check: PASS (100 exact matrix rows;
   implementation_complete=True)`.
3. A new `docs/adr/ADR-0024-accepted-package-aging.md` states the rule for
   every accepted package: closeouts are verified at their validated and
   acceptance commits; later development that changes owner sources is
   governed by the ordinary Tier 1/Tier 2 gates, by the mapped tests that
   remain in the crates (the 419 S20-530 tests still run under `cargo test`
   and `make check-changed`), and by the next package's own contract; a
   package is re-validated only when its own contract is amended or the
   operator orders a re-validation. ADR-0023, the spec, the checker, and the
   evidence stay byte-frozen because they are historical authority at
   `034cc75`; the new ADR supersedes only the post-acceptance binding
   sentence of ADR-0023 v12 and says so.
4. Frontier re-anchor in one commit: the frontier audit doc, the frontier
   checker, and the summary's `local_completion_frontier` and
   `s20_700_remaining_surface_audit` sections move to
   `S20_530_IMPLEMENTATION_COMPLETE`, `s20_530_implementation_complete: true`,
   `next_authority_safe_package: S20-540-PACK-EXCHANGE`,
   `next_dependency_complete_package: S20-540-PACK-EXCHANGE`.
5. `docs/WORK_PACKAGES.md` S20-530 row: complete at `034cc75`, historical
   gate per ADR-0024; S20-540 row unchanged (already dependency-complete).

Cost: about 2 to 3 hours of deterministic local work plus one
`make s20-530-verify` run, no Council spend required. A bounded Maat consult
on the doctrine change and a Nabu consult on the anchor design are optional
and recommended if the Council lane is available (the `claude-cli` token
expires 2026-09-03T11:50Z).

Risk: a regression in the recovery code after `8f7c763` is caught by the
mapped tests and by the changing package's review, not by the exact checker.
Mitigation: `make s20-530-verify` can be pointed at any later commit by an
operator-ordered re-validation, and the anchor keeps the accepted evidence
immutable.

### B. v14 owned-surface amendment

Amend the checker so that after acceptance it binds only the S20-530 owned
surface (spec, ADR, checker, runner, reconciler, test plan, freeze evidence,
the four `OWNER_SOURCES`, the public API sources, and the mapped tests) and
drops the workspace-wide binding.

Cost: one more full amendment loop: v14 design, spec and ADR paragraphs,
checker change, scanner parity, contract set, plan rebuild (about 20
minutes), three freeze receipts, a captured closeout (about 76 minutes),
three implementation receipts, and the final checker (about 95 minutes):
roughly 6 hours wall clock plus six Council reviews.

Why it does not unblock M4: the public-API phase still rejects any new module
in `sley-repo`, so S20-540, S20-510, and S20-520 would each need a v15+
re-validation. B helps only development outside the three owner crates.

### C. Status quo: re-validate on every bound-path change

Every change repeats the full loop (about 6 hours and six Council reviews per
change). Not viable for a multi-package roadmap.

## Recommendation

Option A. It is consistent with how S20-390 and S20-500 already age, it keeps
the checker and its evidence immutable and verifiable at their commits, it
unblocks the frontier re-anchor and S20-540 immediately, and it needs no
Council spend. Exactness is preserved where it was promised (at the validated
commit) instead of being silently redefined at HEAD.

Confidence: high that A is the only option that unblocks the roadmap without
recurring six-hour loops; moderate that the operator prefers it over B's
stronger live binding of the owner crates.

Counterargument considered: the campaign's exactness standard could be read
as requiring live binding forever. That reading makes every later package a
re-validation of S20-530, which the DAG never intended (S20-540 lists only
S20-170 and S20-500 as dependencies), and it contradicts the aging already
accepted for S20-390 and S20-500.

## Next development package after the decision

S20-540 pack exchange: dependencies S20-170 (`S20_170_COMPLETE`) and S20-500
(`COMPLETE_NATIVE_REFS_BRANCHES_BOUNDARY`) are complete; owner Merlin; owned
paths `sley-repo` and `sley-conformance`; acceptance: clean import
reconstructs root, refs, and ancestry per profile; focused gate: the
clone-equivalent test; release implication: M4 exit. S20-510 stays blocked by
the six absent full S20-250 entity bodies, and S20-520 depends on S20-510.

## Exact steps once A is approved

1. Write ADR-0024 and the anchor script; edit the Makefile `quick` target;
   re-anchor the frontier doc, frontier checker, and summary sections; update
   the S20-530 work-package row. One commit per coherent change set.
2. Run `make quick` (Tier 1) and `python3 scripts/check_local_completion_frontier.py`
   to PASS at the new HEAD.
3. Run `make s20-530-verify` once (about 95 minutes, detached) and record the
   PASS line in the stop checkpoint.
4. Open S20-540 with `forge intent start` and a Nabu design consult on the
   clone-equivalent profile.

## Prototype evidence for Option A (narrative lane only, no bound path changed)

A prototype `check_s20_530_acceptance_anchor.py` (about 190 lines, read-only
Git object commands only) was run against the repository at `cc0f92f` and in
a scratch clone with controls:

| Control | Expected | Result |
|---|---|---|
| clean clone at HEAD | PASS | PASS |
| unrelated crate change committed (`crates/sley-repo/src/lib.rs`) | PASS | PASS |
| spec byte change committed | FAIL | FAIL: spec changed after the acceptance commit |
| retained log edited in the working tree | FAIL | FAIL: working-tree bytes differ from HEAD |
| summary `implementation_complete` flipped | FAIL | FAIL: machine summary field differs |
| HEAD moved to the validated commit (acceptance not an ancestor) | FAIL | FAIL: acceptance commit is not an ancestor |
| closeout evidence rewritten with acceptance-identical bytes | PASS | PASS |

Runtime under one second. The prototype is kept at
`machineresearch/sley-2.0/prototypes/check_s20_530_acceptance_anchor.py`
(narrative lane, not on the `scripts/` bound path) and is not authority until
the operator approves Option A and it lands under `scripts/` with ADR-0024,
the Makefile change, and the frontier re-anchor.
