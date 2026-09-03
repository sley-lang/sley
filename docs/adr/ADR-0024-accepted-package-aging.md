# ADR-0024: Accepted package aging and historical closeout verification

Status: accepted for post-acceptance package aging, first applied to S20-530

Date: 2026-09-03

## Context

S20-530 was accepted at commit `034cc75` (validated commit `8f7c763`,
contract set `0257eddd…`, v13 freeze commit `25055783…`) and its full contract
checker, `scripts/check_s20_530_crash_recovery.py`, passed at commit
`cc0f92f` with `implementation_complete=True`. That checker was designed for
exactness during the closeout campaign: with `implementation_complete` true it
requires, at HEAD, a clean tree with no untracked files, the validated commit
as an ancestor, no changed path between the validated commit and HEAD other
than the three output paths and the two narrative lanes (`machineresearch/`,
`docs/WORK_PACKAGES.md`, ADR-0023 v12), and byte and mode equality of every
other tracked path with the validated commit. Its public-API phase also
requires the production Rust source inventory of `sley-store`, `sley-txn`, and
`sley-repo` to equal a frozen list and the module graph of
`crates/sley-repo/src/lib.rs` to be exactly `gc` and `refs`.

Those rules make the accepted closeout exact at the commit where it was
accepted. They also make every later commit that touches any crate, script,
specification, ADR, audit, manifest, or evidence path fail `make quick`. The
first post-acceptance obligation, re-anchoring the local completion frontier,
already needs a bound audit document and a bound script. The next
dependency-complete package, S20-540 pack exchange, adds a module to
`sley-repo`, which the public-API phase rejects regardless of the workspace
binding. Re-validating S20-530 for every later commit would repeat a loop of
about six hours and six Council reviews per change.

Earlier accepted packages already age differently. The S20-390 and S20-500
checkers bind file presence and text markers, not source bytes, and their
closeout evidence is historical authority at its commit. S20-530 is the first
package whose checker binds the whole workspace after acceptance, because the
campaign needed exactness while the contract was still being amended.

## Decision

An accepted package's closeout is a fact about the commits where it was
validated, accepted, and confirmed. It is verified there, not at every later
HEAD.

1. The full S20-530 checker remains byte-frozen and authoritative at the
   accepted state. `make s20-530-verify` runs
   `scripts/verify_s20_530_accepted_state.py`, which clones the repository into
   an isolated directory, checks out the confirmation commit `cc0f92f` on a
   local `main` branch, copies the source repository's exact `.git/config`, and
   runs the no-argument checker there. It must print
   `S20-530 crash-recovery contract check: PASS (100 exact matrix rows;
   implementation_complete=True)`.
2. `make quick` runs `scripts/check_s20_530_acceptance_anchor.py` instead of
   the full checker. The anchor verifies that the validated, acceptance, and
   confirmation commits are ancestors of HEAD; that the specification,
   ADR-0023, the checker, the runner, the reconciler, the v13 freeze evidence,
   the test plan, the closeout evidence, and the 28 retained logs at HEAD are
   the same Git blobs as at the acceptance commit and that the working tree
   matches them; that the retained final-checker confirmation log has its
   recorded digest and PASS record; that the machine summary still records the
   accepted S20-530 facts and the six receipt sessions exactly; and that the
   work-package table carries the S20-530 row markers. It uses only read-only
   Git object commands.
3. The frozen S20-530 surfaces are immutable history: the specification,
   ADR-0023, the checker, the runner, the reconciler, the freeze evidence, the
   test plan, the closeout evidence, and the retained logs may not change. A
   change to any of them is a new contract version and requires a fresh
   freeze, closeout, and receipts under the S20-530 contract's own rules.
4. Later development that changes the owner sources of an accepted package
   (for S20-530: `crates/sley-store/src/lib.rs`,
   `crates/sley-txn/src/repository.rs`, `crates/sley-repo/src/refs.rs`,
   `crates/sley-repo/src/gc.rs`, and the mapped tests) is governed by the
   ordinary Tier 1 and Tier 2 gates, by the mapped tests that remain in the
   crates and run under `cargo test` and `make check-changed`, and by the
   changing package's own contract and review. Such a change does not re-open
   the accepted closeout. The operator may order a re-validation at any later
   commit with `make s20-530-verify` pointed at that commit through
   `--commit`, after a fresh closeout and receipts have been produced for it.
5. This ADR supersedes only the post-validation binding rule of ADR-0023 v12
   at HEADs after the confirmation commit. ADR-0023 itself is unchanged and
   remains the contract authority at the accepted state.
6. Future packages whose checkers bind the whole workspace after acceptance
   adopt the same pattern: a frozen full checker verified at the accepted
   state and a read-only anchor in `make quick`.

## Consequences

- The accepted S20-530 evidence stays exact, immutable, and verifiable at its
  commits, and every later `make quick` proves that it has not been rewritten.
- The local completion frontier can be re-anchored and S20-540 can start
  without re-validating S20-530.
- Regressions in the recovery code after `8f7c763` are caught by the 419
  mapped tests and by the changing package's review, not by the exact
  checker. That is the same standard already applied to S20-390 and S20-500.
- No Council review, provider spend, push, or publication is required by this
  decision. Bounded Nabu and Vulcan reviews of the anchor and this rule are
  recorded in `docs/audits/S20_530_CRASH_RECOVERY_CLOSEOUT.md` when they
  complete.

## Rejected alternatives

- A v14 checker amendment that binds only the S20-530 owned surface after
  acceptance. It costs one more full freeze loop and still rejects any new
  module in `sley-repo`, so it does not unblock S20-540, S20-510, or S20-520.
- Re-validating S20-530 on every bound-path change. About six hours and six
  Council reviews per change; not viable for a multi-package roadmap.
- Removing the S20-530 checker without an anchor. The accepted evidence could
  then be rewritten silently.
