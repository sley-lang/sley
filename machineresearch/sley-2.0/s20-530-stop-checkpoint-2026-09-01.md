# S20-530 stop checkpoint (2026-09-01)

Status: PAUSED AT V10 AMENDMENT CANDIDATE, OPERATOR-DIRECTED

Owner: Claude orchestrator

Checkpoint time: 2026-09-01T17:16:58-04:00

## Repository state

- Repository: `/home/greyforge/sley2`, branch `main`.
- The v10 amendment candidate is committed at this checkpoint (see the
  commits directly preceding this file's commit): the cor07 helper repair in
  `crates/sley-repo/src/refs.rs`, the amended and resealed
  `docs/spec/CRASH_RECOVERY_MATRIX_V1.md`,
  `docs/adr/ADR-0023-crash-recovery-boundary.md`, and
  `scripts/check_s20_530_crash_recovery.py`, plus the design doc
  `s20-530-v10-cor07-helper-environment-amendment-design.md` and the full
  campaign narrative in `s20-530-resume-2026-09-01.md`.
- `evidence/validation/s20-530-crash-recovery-logs-v1/` holds untracked
  NON-AUTHORITATIVE logs from the two failed closeout attempts, kept
  deliberately for diagnosis. `tier2-03-cargo-test-p-sley-repo-lib-locked.log`
  documents the cor07 `mkfifo` failure. They are intentionally not committed,
  matching the prior convention that only captured-run evidence from a
  passing closeout is committed.
- Nothing was pushed, deployed, or published.
- All background work was stopped cleanly before this checkpoint; the stopped
  v10 plan rebuild writes its output only at completion, so the committed
  plan file is untouched (still v9-bound from commit `c4d6e35`).

## What is complete today

1. The v9 governance gap was closed exactly as the 2026-08-31 stop checkpoint
   required: fresh Nabu, Ariadne, and Vulcan `PASS_CONTRACT_FREEZE` receipts
   were obtained through the recovered Council lane, bound into the v9
   evidence, and verified `PASS_TRUSTED_LOCAL_REVIEW_RECEIPTS` (commit
   `424126e`). The v9 freeze became an authoritative reviewed freeze.
2. The OpenClaw 2026.8.1 sqlite session migration was repaired: all six v8
   receipt files hash-verified and restored to the checker-read path, and a
   byte-fidelity-proven sqlite export path established for new sessions.
3. The test plan was rebuilt and committed bound to the v9 contract set
   (`c4d6e35`).
4. Closeout attempt 1 failed closed on umask-derived `0664` working modes;
   repaired by normalizing every tracked file to its Git-recorded mode.
5. Closeout attempt 2 failed one mapped test: cor07 spawned `mkfifo`, absent
   from the pinned isolated tool path. Repaired with the pure-std
   `plant_non_regular_socket` technique; `sley-repo` lib suite 303/303.
6. The repair rebinds frozen partition digests, so the committed v10
   amendment candidate refreezes: refs.rs scanner parity, scanner contract
   digest, limit source-set digest, both ledger digests, partition
   fingerprint `f335c2f5…`, evidence identity v9 to v10, and the two governed
   documents. Frozen contract self-integrity passes. New v10 contract set:
   `d7bc0a1069d06845d1581b47ffd192ac1a9683f38f3750b737f5f5c0be38c697`.

## Explicitly not done

- The v10-bound test plan was NOT built (the rebuild was stopped by this
  checkpoint; the committed plan is v9-bound and the runner will reject it
  against the v10 checker).
- The v10 freeze evidence file does NOT exist yet
  (`evidence/validation/s20-530-crash-recovery-contract-freeze-v10.json`);
  the checker's `FREEZE_EVIDENCE` already points to it, so freeze-evidence
  loads fail closed until it is written.
- No v10 freeze reviews were requested. No implementation receipts exist.
- The captured closeout has never passed. The machine summary still points to
  the last fully-consistent authoritative state and was deliberately not
  touched today.

## Exact resume point

1. Rebuild the test plan:
   `/usr/bin/python3 -B scripts/build_s20_530_test_plan.py
   --contract-set-sha256 d7bc0a1069d06845d1581b47ffd192ac1a9683f38f3750b737f5f5c0be38c697`
   (about 19 minutes; writes the 12 MB plan).
2. Run the Tier 2 refreeze command set (py_compile, ruff format/check on
   checker and runner, `git diff --check`, sanitized runner self-test with
   `env -u GIT_EDITOR`, `cargo test -p sley-repo --lib --locked`, the
   reconciler against the new plan, partition regeneration equality, frozen
   self-integrity) and assemble
   `s20-530-crash-recovery-contract-freeze-v10.json` mirroring the v9
   structure: same `review_obligations` constant, `implementation_complete:
   false`, empty `reviews`/`review_receipt_verification`, refreshed
   `deterministic_inputs` and partition block, `resolved_findings` carrying
   v9's list plus `cor07_helper_execution_environment`.
3. Commit, then dispatch fresh Nabu, Ariadne, and Vulcan contract-freeze
   reviews with new nonces against the v10 contract set and the v10
   review-free evidence digest (session-id pattern
   `forge-<role>-s20-530-contract-freeze-<UTC>-<nonce8>`). Export each
   session from `~/.openclaw/agents/<role>/agent/openclaw-agent.sqlite`
   (`SELECT event_json FROM transcript_events/trajectory_runtime_events WHERE
   session_id=? ORDER BY seq`, newline-joined plus trailing newline) to
   `~/.openclaw/agents/<role>/sessions/<sid>.jsonl` and
   `<sid>.trajectory.jsonl`, bind receipts, run
   `--verify-review-receipts contract_freeze`, insert the printed record,
   re-verify, commit.
4. Run the captured closeout once with a sanitized environment
   (`env -u GIT_EDITOR /usr/bin/python3 -I -B
   scripts/run_s20_530_validation.py`) on an exactly clean tree with every
   tracked file at its Git-recorded mode. Do not write into the repository
   while it runs.
5. On PASS: collect `PASS_IMPLEMENTATION` receipts (they additionally bind
   `source_set_sha256` and `validated_commit` from the closeout evidence),
   verify, update the machine summary, and commit.

Helper tooling from this session lives in the session scratchpad
(`prepare_v9_reviews.py`, `prepare_ariadne_round2.py`,
`collect_v9_receipts.py`) and is disposable; regenerate equivalents against
the v10 digests rather than reusing v9 request material. Never reuse nonces
or session IDs.

## Known operational hazards for the resume session

- Files created by tools inherit umask `0664`; the closeout compares working
  modes to the committed tree (`0644`/`0755`). Normalize before running.
- The frozen execution `PATH` is the isolated tool-bin; test code must not
  spawn external binaries.
- Any byte change to a scanned production-source file after freeze
  invalidates the partition freeze and receipts: amend and refreeze rather
  than patching around it.
- Long-running background tasks in the previous session were externally
  stopped several times; the operator confirmed the stops were not
  deliberate. If it recurs, ask the operator before assuming intent.

## Completion estimate at stop

- S20-530 governed closeout: about 92%, high confidence (the v10 refreeze
  added review work that v9 had already banked).
- Overall Sley 2.0 roadmap: 51%, moderate confidence, unchanged.

No next work package is authorized by this record.
