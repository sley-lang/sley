# S20-530 stop checkpoint (2026-09-02)

Status: V12 FROZEN AND REVIEWED; AUTHORITATIVE CLOSEOUT (ATTEMPT 5) RUNNING OR
COMPLETE; RESULTS LIVE ONLY IN THE OUTPUT PATHS

Owner: Claude orchestrator

Checkpoint time: 2026-09-02T11:01:31Z

## Repository state at this commit

- Repository: `/home/greyforge/sley2`, branch `main`. This checkpoint is the
  last narrative commit before the authoritative closeout. After it, the
  intended changes are the output paths: `evidence/validation/
  s20-530-crash-recovery-closeout-v1.json`, `evidence/validation/
  s20-530-crash-recovery-logs-v1/*.log`, and
  `machineresearch/sley-2.0/machine-summary.json`.
- The v12 contract is frozen and reviewed: freeze evidence
  `evidence/validation/s20-530-crash-recovery-contract-freeze-v12.json` in its
  single addition commit `f12bfecb744664250915001eef55092efb47ef88`, contract
  set `939c2aea801500b915e0a47d1c39585757755454bf7da1469b76ec64dd72b416`,
  partition fingerprint
  `867bd1d9ca7de09a2e928d1fa5acf84123e192434d7a60d947621b72a11c8487`, test
  plan `256ee2dfbe4c63402d023ab7184bfd0ba0cb9c54861bd98699b9353eeb6141d8`
  (rows=100, tests=419), receipts nabu `…20260902T103652-1198b414`, ariadne
  `…20260902T103652-a084b9d0`, vulcan `…20260902T103652-13ee2fff`, all
  `PASS_CONTRACT_FREEZE`, verified `PASS_TRUSTED_LOCAL_REVIEW_RECEIPTS`. The
  v11 freeze (`a35bbb2`) remains immutable history.
- Nothing was pushed, deployed, or published. Council model calls were the only
  external spend.

## How to read the result

1. If `machine-summary.json` `s20_530_crash_recovery.implementation_complete`
   is `true`, the authoritative closeout passed, three `PASS_IMPLEMENTATION`
   receipts were bound and verified, and `python3
   scripts/check_s20_530_crash_recovery.py` (part of `make quick`) passed at
   that HEAD. S20-530 is complete and M4 is unblocked by this package.
2. If it is still `false`, the closeout or its receipts did not complete after
   this checkpoint; the resume point below applies.

## Exact resume point (only if step 2 applies)

1. `git status` must show only output paths (or nothing). Move any stale
   closeout outputs to `~/archive/` before rerunning.
2. Run the closeout once on the exactly clean tree:
   `env -u GIT_EDITOR /usr/bin/python3 -I -B scripts/run_s20_530_validation.py`
   (about 75 minutes: 65 pre-execution, then 3 test lists and 25 commands).
3. Request three `PASS_IMPLEMENTATION` reviews (fresh nonces, session pattern
   `forge-<role>-s20-530-implementation-<UTC>-<nonce8>`) binding the closeout
   evidence's `source_set_sha256`, `validated_commit`, and review-free payload;
   export each session from `~/.openclaw/agents/<role>/agent/openclaw-agent.sqlite`
   to `~/.openclaw/agents/<role>/sessions/<sid>.jsonl` and
   `<sid>.trajectory.jsonl`; bind, run
   `--verify-review-receipts implementation`, insert the record, re-verify.
4. Update `machine-summary.json` (`implementation_complete: true`,
   `contract_freeze_evidence` v12, `contract_set_sha256`, `freeze_commit`
   `f12bfecb744664250915001eef55092efb47ef88`, `contract_reviews`, `validation_evidence`,
   `implementation_reviews`), commit the three output paths together, then
   run the no-argument checker to PASS.

## Hazards

- After this checkpoint, commit only the output paths until the final checker
  passes; afterwards only the narrative lanes (`machineresearch/`,
  `docs/WORK_PACKAGES.md`) may change without re-validation (v12 rule).
- The Council dispatch lock serializes agent runs machine-wide; other
  sessions may hold it.
- Files created by tools inherit umask `0002`; the closeout compares working
  modes with Git modes. Keep created files at `0644`.
- Harness-tracked background shells have been killed externally; run long
  steps detached and watch their logs.

## Operator note

After acceptance the S20-530 gate inside `make quick` re-opens on any later
commit that changes a bound path (crates, scripts, spec, ADR, plan, manifests,
conformance, oracle, bench, evidence). Only the narrative lanes are exempt
(v12). Continuing development (S20-540 and later) therefore needs either a
re-validation per change or a successor decision on how accepted packages
age; this is an operator decision and was not taken here.

## Completion estimate at this checkpoint

- S20-530 governed closeout: 97 percent, high confidence (one captured run and
  three implementation receipts remain, both already demonstrated in the
  attempt-4 dry run).
- Overall Sley 2.0 roadmap: 53 percent, moderate confidence.
