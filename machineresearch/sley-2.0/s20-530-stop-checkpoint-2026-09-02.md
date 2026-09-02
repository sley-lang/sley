# S20-530 stop checkpoint (2026-09-02)

Status: V13 ACCEPTED (FREEZE REVIEWED, CLOSEOUT PASSED, IMPLEMENTATION RECEIPTS
BOUND, MACHINE SUMMARY COMPLETE); FINAL CHECKER CONFIRMATION RUN PENDING

Owner: Claude orchestrator

Checkpoint time: 2026-09-02T19:13:52Z

## Repository state at this commit

- Repository: `/home/greyforge/sley2`, branch `main`. This checkpoint is the
  last narrative commit before the authoritative closeout. After it, the
  intended changes are the output paths: `evidence/validation/
  s20-530-crash-recovery-closeout-v1.json`, `evidence/validation/
  s20-530-crash-recovery-logs-v1/*.log`, and
  `machineresearch/sley-2.0/machine-summary.json`.
- The v12 closeout (attempt 5, validated commit `7277af6`) passed and three
  `PASS_IMPLEMENTATION` receipts were bound (`f677b1a`), but the first full
  checker run exposed one more latent implementation-gate defect (the LIMIT
  frozen-default field in the generic semantic check); the v13 amendment
  repairs it, and the v12 outputs were removed from the tree (`0748588`).
- The v13 contract candidate is committed (`3abb51c`), contract set
  `0257eddda95d24dba657b993eee981443e6c215e93f7cb04387a8d444a9c7caa`; its
  freeze evidence, receipts, plan binding, closeout, and implementation
  receipts are recorded by later commits and the output paths. The v12 freeze
  (`f12bfecb`, contract set `939c2aea…`) remains immutable history with
  partition fingerprint
  `867bd1d9ca7de09a2e928d1fa5acf84123e192434d7a60d947621b72a11c8487`, test
  plan `256ee2df…` (rows=100, tests=419) and receipts nabu `…20260902T103652-1198b414`, ariadne
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
   `contract_freeze_evidence` v13, `contract_set_sha256`, `freeze_commit`
   (the v13 evidence addition commit), `contract_reviews`, `validation_evidence`,
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

- S20-530 governed closeout: 97 percent, high confidence (v13 freeze receipts,
  one captured run, and three implementation receipts remain; all three steps
  have already succeeded once under v12).
- Overall Sley 2.0 roadmap: 53 percent, moderate confidence.

## State at 2026-09-02T19:13:52Z (supersedes the sections above where they differ)

- v13 is frozen and reviewed: freeze evidence
  `evidence/validation/s20-530-crash-recovery-contract-freeze-v13.json` in its
  single addition commit `250557833364ae1f85dc67290ff6c7c5f0c7fdba`, contract
  set `0257eddda95d24dba657b993eee981443e6c215e93f7cb04387a8d444a9c7caa`,
  plan `5e051a6bb2a89fd54d50cb706eb87a603df8ccc0a9764c1b7e063690f947ee44`
  (rows=100, tests=419), receipts nabu `…20260902T173505-4e2f40e0`, ariadne
  `…20260902T173505-ee0cd3fa`, vulcan `…20260902T173505-d7eb9f55`, all
  `PASS_CONTRACT_FREEZE`, verified `PASS_TRUSTED_LOCAL_REVIEW_RECEIPTS`.
- The authoritative closeout (attempt 6) PASSED on validated commit
  `8f7c7630c3ba786478aa0066ba35c271feff2036`; its evidence (reviews empty,
  payload `7a312c0c…`, source set `468490e1…`) and 28 logs are committed as
  output paths. `machine-summary.json` still says
  `implementation_complete: false` and points at the v8 freeze; the checker's
  no-argument run therefore still fails at the machine summary, as it has
  throughout the campaign.
- Blocker: implementation reviews cannot be dispatched until the Codex OAuth
  usage window resets (`The usage limit has been reached`) or the operator
  allows another model in `agents.defaults.modelPolicy.allow`.

## Exact resume point

1. Tree must be clean at or after `HEAD` of this checkpoint; do not change any
   bound path (crates, scripts, `docs/spec`, `docs/adr`, plan, manifests,
   conformance, oracle, bench, evidence). Narrative lanes (`machineresearch/`,
   `docs/WORK_PACKAGES.md`) may change.
2. Request three `PASS_IMPLEMENTATION` reviews with fresh nonces, one at a
   time, `forge agent --bounded --thinking xhigh --timeout 3000`, session
   pattern `forge-<role>-s20-530-implementation-<UTC>-<nonce8>`. The request
   payload comes from `phase_review_request_payload(role,
   "PASS_IMPLEMENTATION", nonce, contract_set, review_payload_sha256(closeout
   evidence), validation.source_set_sha256, validation.validated_commit)`; the
   message carries `S20_530_REVIEW_REQUEST_JSON=` and the expected
   `S20_530_REVIEW_VERDICT_JSON=` line, the dirty-tree note is no longer
   needed (outputs are committed), and a decision budget of about 20 minutes
   (at 1800 s two reviewers timed out mid-investigation).
3. Export each session from `~/.openclaw/agents/<role>/agent/openclaw-agent.sqlite`
   (`transcript_events` and `trajectory_runtime_events` for the session id,
   `event_json` newline-joined with a trailing newline) to
   `~/.openclaw/agents/<role>/sessions/<sid>.jsonl` and
   `<sid>.trajectory.jsonl`; bind the review rows in
   `phase_review_field_order("PASS_IMPLEMENTATION")` order into the closeout
   evidence `reviews`; run `--verify-review-receipts implementation`, insert
   the printed record as `review_receipt_verification`, re-verify.
4. Update `machine-summary.json` (`status`
   `CONTRACT_FROZEN_IMPLEMENTATION_COMPLETE`, `implementation_complete: true`,
   `contract_freeze_evidence` v13, `contract_set_sha256` `0257eddd…`,
   `freeze_commit` `25055783…`, `contract_reviews` from the v13 freeze
   evidence, `validation_evidence`, `validated_commit`, `source_set_sha256`,
   `implementation_reviews` from the closeout evidence). Commit the closeout
   evidence and machine summary together.
5. Run `/usr/bin/python3 -I -B scripts/check_s20_530_crash_recovery.py`
   (about 95 minutes). It must print `S20-530 crash-recovery contract check:
   PASS (100 exact matrix rows; implementation_complete=True)`. Then append a
   completion note to this checkpoint (narrative lane) and commit.

## State at 2026-09-02T22:58:01Z (supersedes the sections above where they differ)

- Implementation receipts were obtained on `claude-cli/claude-opus-5` after the
  operator directed the switch away from the exhausted OpenAI account (gateway
  policy and per-agent `claude-cli:local` auth profiles added; see the resume
  record). All three are `PASS_IMPLEMENTATION`, verified
  `PASS_TRUSTED_LOCAL_REVIEW_RECEIPTS`.
- Acceptance commit `034cc75abb59d743ea30b2f2a410205a018cb25f`:
  `machine-summary.json` says `implementation_complete: true`, bound to the v13
  freeze `25055783…`, contract set `0257eddd…`, validated commit `8f7c763`,
  source set `468490e1…`.
- Remaining: the full checker run at the accepted state (about 95 minutes),
  which is the `make quick` gate; it was relaunched right after this checkpoint
  and is expected to print `S20-530 crash-recovery contract check: PASS (100
  exact matrix rows; implementation_complete=True)`. If it fails, the failure
  message names the defect; repair follows the amendment loop (v14) exactly as
  v11 through v13 did.
- Do not change any bound path until that run passes; narrative lanes
  (`machineresearch/`, `docs/WORK_PACKAGES.md`) remain free to change.

## Completion estimate at this checkpoint

- S20-530 governed closeout: 99 percent, high confidence (only the
  confirmation run and a completion note remain; the implementation path was
  already exercised clean against the real evidence).
- Overall Sley 2.0 roadmap: 53 percent, moderate confidence. S20-530 was the
  last blocker on the M4 crash-recovery lane; S20-510, S20-520, and S20-540
  (comparison, merge, pack exchange) and the M5 benchmark packages remain.
