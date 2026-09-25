# Gate record — source checkpoint 34dfbba5 (2026-09-21, work branch only)

## Source / toolchain / binary identities

- Source checkpoint: `34dfbba5` (`work/succession-sley20-arm`, pushed to
  `origin/work/succession-sley20-arm`; parent `f68055dd`). Rust tree
  byte-identical to `f68055dd` (`git diff f68055dd 34dfbba5 -- crates/
  oracle/` empty; Python + docs only).
- Toolchain: pinned `1.93.0` active (`rust-toolchain.toml` channel 1.93.0;
  `rustc 1.93.0`, `cargo 1.93.0`).
- Test binaries (built from Rust-identical `f68055dd`, reused with that
  recorded justification; no Rust change in this pass):
  - `target/debug/sley` sha256
    `95c4cc0b224ccd29e0f3b59294c3e2bb3a27243b3c8e2b2b6a58f2662fa62867`
  - `target/debug/deps/succ_live_judge_cases-42d77a02eb446c77` sha256
    `e5adf2a1ee1f911dd5718e14945c8d78a873b0c7798ed9ada26f2094ade3f7fa`
- Gate worktree: detached `/tmp/sley-gate-34dfbba5` at `34dfbba5` (main's
  dirty worktree never cleaned, reset, or moved; wt2 untouched). Build
  storage: user-owned `/home/dev/.cache/sley-gate-34dfbba5-target`
  (`dev`, on `/home`); pinned toolchain; `--locked`.
- Raw logs (persistent, resume dir): `gate-34dfbba5-lint.log`,
  `gate-34dfbba5-capture-demo-all.log`,
  `gate-34dfbba5-capture-demo-typepos.log`.

## Outcomes (exit codes exact)

| Check | Command (workdir) | Exit | Result |
|---|---|---|---|
| Focused + integrated attempt-path suites | `python3 -m unittest discover -s bench/live/tests` (wt-succ, `SLEY2_SLEY_BINARY` + `SUCC_JUDGE_TEST_BINARY` bound) | 0 | 170 tests OK (incl. 13 mediated-attempt incl. 2 real-oracle proofs, 9 gateway, 16 trusted-capture) |
| `cargo fmt --check` | `cargo fmt --all --check` (gate worktree) | 0 | PASS |
| `make lint` | `make lint` (gate worktree, `CARGO_TARGET_DIR` above) | 2 (FAIL) | `clippy_clean: false`, 61 pedantic diagnostics under `-D warnings`, all in `crates/sley-repo/tests/succ_live_emit.rs` (52: doc-backticks, complex types, casts, separators incl. `1_812_500`/`9_223_372_036_854_775_807_i64`, long fns) + `succ_live_judge_cases.rs` (9: doc-backticks/list-indent, redundant closures, casts). Pre-existing baseline defect on this branch: zero Rust files changed in this pass (`git diff` empty), so none is attributable to it. `fmt_clean: true`, `lint_inputs_clean: true`, `working_tree_clean: true` in the gate worktree |
| `capture_demo.py all` | `python3 bench/live/capture_demo.py all` (wt-succ, binaries bound) | 0 | all 5 steps PASS (access/refusal/inject/storage/type_pos with discovery: 13 exchanges reconciled, judge ACCEPTED, adjudicated accepted) |
| `make quick` | — | not run | Blocked behind the lint FAIL (aggregate stops early) + execution limits; exact resume command below. No exemption claimed; focused suites + fmt + capture_demo executed as independently runnable later checks, aggregate failure preserved |
| Source-bound fuzz refresh + freshness | — | not run | Multi-hour; freshness rules unchanged, no exemptions taken. Last refresh logs predate this checkpoint. Exact resume command below |

## Next commands (exact)

1. Lint repair (pre-existing diagnostics, unrelated to this pass):
   `git worktree add --detach /tmp/sley-lint-repair <sha>` then address
   the 61 pedantic clippy diagnostics in
   `crates/sley-repo/tests/succ_live_emit.rs` +
   `succ_live_judge_cases.rs` (or record a parsed lint-baseline
   decision through the owning review lane — never a silent exemption),
   then `make lint` (exit 0 required) in that worktree.
2. `make quick` in the clean detached worktree at the post-lint
   checkpoint (needs `SLEY2_MASTER_GOAL=.../greyforge-managed-home/
   machineresearch/Sley2.0mastergoal.md`, user-owned `CARGO_TARGET_DIR`
   on `/home`).
3. Source-bound fuzz refresh: `fuzz-refresh.sh` in `sley2-c0-mint`
   (multi-hour, authorized resources), then freshness checks
   (`transcribe_fuzz_proofs.py`, slice checkers); record log names.
4. Implementation handoffs: CONTEXT mediated-evidence judge route +
   member/impact contract (`MEDIATED-INPUT-ACCOUNTING.md` § "not
   established"); CREATE typed LineItem/tax entry execution (task-2
   note: driver needs Record/Sequence/Result-Money value support +
   judge entry-mapping without precomputed intermediates).

## Holds (unchanged)

`ga_claimed=false` (`evidence/release/ga-acceptance-report.json:337`
verified). No integration, signing, publication, lab, AR-02, R2, C1,
or release action. Commits pushed only to
`origin/work/succession-sley20-arm`. Main stays dirty at `8966da2e`;
wt2 at `acbc65f0`; checkpoints/packs/logs/review inputs/artifacts
untouched. Candidate-bound release reports kept separate from this
development evidence.

## Addendum — D4 gates at 7bcc5a89 (+ proof refresh 03982595)

- Source checkpoint: `7bcc5a89` (`work/succession-sley20-arm`;
  D1 staging split `c75aa3ee`, emitter lint `fbe48d29`, typed driver
  `05b2da6a`, typed CREATE `47ac2dcc`, mediated CONTEXT `05b49112`,
  D4 lint repairs `7bcc5a89`), plus fuzz-proof refresh `03982595`
  (same branch, fast-forward). Pushed to
  `origin/work/succession-sley20-arm`.
- Toolchain: pinned `1.93.0` (`rustc 1.93.0`, `cargo 1.93.0`);
  `--locked`, `CARGO_NET_OFFLINE=true`.
- Binaries (wt-succ `target/debug`, rebuilt from this source):
  `sley` sha256
  `95c4cc0b224ccd29e0f3b59294c3e2bb3a27243b3c8e2b2b6a58f2662fa62867`
  (unchanged: no library sources touched);
  `succ_live_judge_cases-42d77a02eb446c77` sha256
  `48c558956bb9258a78d4eba1a058992841bdb934617e15afd59c0f909612d745`
  (rebuilt: D2 typed driver).
- Gate worktrees (detached, main never cleaned/reset/moved, wt2
  untouched): `wt-lint-fbe48d29` (lint repair), `wt-d4-05b49112`
  (superseded), `wt-d4-7bcc5a89` at `7bcc5a89` (gates).
  Build storage: user-owned `/home/dev/.cache/sley-d4-7bcc5a89/target`.
- Raw logs (persistent, resume dir): `lint-Q-05b49112.log` (FAIL, 6
  diagnostics) → repaired on branch → `lint-R-7bcc5a89.log` (PASS);
  `quick-R-7bcc5a89.log` (PASS through line 86); `quick-R3-7bcc5a89.log`
  (keep-going attempt, same stop); `quick-R4-7bcc5a89.log`
  (lines-88+ remainder, explicit); `fuzz-refresh-7bcc5a89.log`
  (7 lane runs, all PASS); `cargo-test-7bcc5a89.log` (workspace
  suite, all ok).

| Check | Command (workdir) | Exit | Result |
|---|---|---|---|
| `make lint` | `make lint` (clean tree, user-owned target dir) | 0 | PASS: fmt clean, clippy 0 warnings (`lint-report.v1`) |
| `make quick` lines 1–86 | `make quick` (clean tree) | stops 45 | all spec/fixture checks PASS; all fuzz slices PASS incl. 7 refreshed lanes (exchange, merge, merge_judgment, pack, semantic_delta, smp1, smp1_json_bridge re-proofed at 7bcc5a89 and transcribed) |
| `make quick` line 87 | `build_candidate_content_report.py --check` | 1 | BLOCKED (structural, pre-existing): missing ignored `evidence/runtime/s20-720-release-candidate/evidence.json` in a fresh tree; satisfying it needs `release-candidate-build`, which rebinds candidate-bound release reports and needs main-line attestation — forbidden/out-of-scope for a work branch (would fail identically on any work-branch commit) |
| quick remainder (88+) | explicit per-line runs (clean tree) | 0 except noted | contract/invariant/freeze/frontier checks PASS; `git diff --check` PASS; `cargo check --workspace` PASS; `cargo test --workspace` all ok EXCEPT pre-existing env-gated `succ_debug_commit::debug_commit_repro` (missing `SUCC_DEBUG_*` env; untouched temporary helper) and one environmental `check_transaction_contract` FAIL (absent operator `SLEY2_MASTER_GOAL` file) |
| `cargo test --workspace` | `-- --skip debug_commit_repro` (clean tree) | 0 | 74 result lines, all ok, zero failures |
| bench/live suites | `python3 -m unittest discover -s bench/live/tests` (wt-succ, binaries bound) | 0 | 198 tests OK (incl. 10 driver-unit, 23 judge-unit, 13 mediated-audit-unit, 3 context_pos + TYPE/STALE campaign-path integration proofs) |

- Fuzz refresh (source-bound, existing tooling/dependency order):
  7 stale lanes (lane inputs include `crates/sley-repo`, touched by
  test-only changes) re-run (`run_*_persistent_fuzz.py`, all PASS),
  transcribed via `transcribe_fuzz_proofs.py` with a work-branch note,
  committed as `03982595`; slice checkers green at the new commit.
  No crashes, no new artifacts.
- Freshness/proof checks: slice checkers inside `quick` (all green);
  `succ_live_packs_frozen` green (CREATE/CONTEXT manifest re-emits
  changed judge JSON only; all packs byte-identical).
- Retained gates (not implementation gaps): co-commit review gate
  for single-trial CREATE code+tests (TXN_TEST_EVIDENCE_UNSUPPORTED;
  two-harness-commit mechanics proven, witness round-1 commit
  explicitly logged); CONTEXT discovery review gate (member literal
  + impact set undisclosed; no permitted bounded enumeration route);
  release-candidate attestation scope (above); live-model campaign
  (held throughout).
