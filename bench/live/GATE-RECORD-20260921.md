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
  storage: user-owned `/home/gfarch/.cache/sley-gate-34dfbba5-target`
  (`gfarch`, on `/home`); pinned toolchain; `--locked`.
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
