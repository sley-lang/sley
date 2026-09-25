Re-review complete. All four prior findings close; one new blocking finding falls out of the P1-1 fix itself.

```
═══════════════════════════════════════════
VERDICT — REQ-09 rev2 CREATE native trial admission contract delta
═══════════════════════════════════════════

Reviewer: Ariadne (contract lane). Date: 2026-09-21.
Packet: evidence/review/requests/REQ-09-rev2-create-native-trial-admission.md
Packet sha256: dd9a02ce4ec0a63514c578850de1df07017209b45ddee61acc35d105a4f177d1 (verified, matches dispatch)
Baseline: b0e52e00a2bacfd7a874f0b4a7973a352508cebe on work/succession-sley20-arm (verified via git rev-parse HEAD)
Supersedes: REQ-09 rev1 (97e50c9b…20da), verdict FAIL_NEW_P1_OPEN_NO_P0
Scope key: native_trial_admission
Tree state: tracked tree clean; 6 untracked paths (REQ-09/-rev2, REQ-10/-rev2, two verdict dirs). Immaterial to the delta.
Mode: read-only. No file writes performed.

PRIOR FINDING DISPOSITIONS (each re-traced at baseline, not accepted on assertion)

NTA-P1-1 (was BLOCKING) — CLOSED.
  The fresh-session mechanism is sound and I verified the mechanism end to end,
  not just the citation. A new session binds bound_root to the current head
  (session.rs:346), so the native precheck (server.rs:2977-2982) compares
  session_root against the advanced head, finds them EQUAL, and falls through
  instead of returning RootAdvanced. Control then reaches commit_native →
  commit_native_inner, whose freshness gate (repository.rs:3275-3281) returns
  CommitError::StaleRoot for a moved parent, mapping to "STALE_ROOT"
  (repository.rs:724). Corroborated by the existing unit test
  native_commit_stale_parent_refuses_before_execution (repository.rs:24532-24548,
  asserts CommitError::StaleRoot). The frozen exact-equality pin at
  sley2_live_judge.py:2457-2460 is therefore reachable unchanged under the new
  profile. The fresh-session pattern cited at :2462 is real. Treating
  SESSION_ROOT_ADVANCED as expected-non-acceptance on the reused-session path
  rather than a substitute pin is the correct contract call. No frozen symbol
  changes, no substring matching.

NTA-P2-1 (was QUALIFYING) — CLOSED.
  Named second constant plus byte-identical PROFILE_ARGS for existing consumers
  is the right shape. Consumption-site count re-verified: 10 PROFILE_ARGS
  occurrences = 1 definition (runner.py:151) + 9 uses (:450, :452, :453, :1384,
  :1388, :1436, :1529, :1568, :1596). The packet says "eight current consumption
  sites"; the true count is nine. Bookkeeping slip, not a contract defect — the
  requirement "PROFILE_ARGS stays byte-identical for all current consumers" is
  unambiguous and covers all nine. Checker pin disclosure is accurate and now
  complete: check_sley2_trial_runner.py:250 asserts the tuple equals exactly
  ("--protocol-profile","v2-capable") via the AST extractor _runner_profile
  (:116-130, keyed on the literal name PROFILE_ARGS), and the :262 guard-marker
  set carries "protocol_version != 2". Note for the implementer: because the
  extractor is keyed on the constant NAME, the new constant is invisible to the
  checker until :250 is made per-profile — exactly the edit the packet commits to.

NTA-P2-2 (was QUALIFYING) — CLOSED.
  Stating the allowlist digest and the stamp rule as a pair resolves the
  collision. Verified the two are separable in fact: the digest check
  (runner.py:933-934) calls arm_affordances_digest(), which recomputes from
  ARM_AFFORDANCES at runtime (runner.py:474-476), while the stamp check is an
  independent branch (runner.py:939-940). Making the stamp guard per-profile
  leaves the digest byte-identical across both profiles. Stamp-2 path
  byte-identical is achievable as claimed.

NTA-P3-1 (was ADVISORY) — CLOSED as an enumeration item.
  Both commit sites are now named correctly: sley2_live_judge.py:1188
  (_commit_candidate) and :2449 (stale probe, direct _encode_commit_body +
  _raw_request). Profile-gated encoder selection with mutual unreachability is
  the correct mitigation for the arity-4 field-semantics collision. The
  enumeration is closed — but see NTA-P1-2, which is a consequence of the
  remedy this finding mandates, not a restatement of it.

NEW FINDINGS

NTA-P1-2 (BLOCKING) — Native encoder on the stale probe requires an attempt_id
the packet does not specify, and the natural choice makes the stale oracle
observe a COMMIT instead of STALE_ROOT.
  Closing P3-1 obliges the :2449 stale probe to emit the native 4-field payload,
  whose field[2] is attempt_id (server.rs:2986). The legacy body it encodes today
  has no such field, and the packet never states how attempt_id is derived for
  the probe. This is a contract decision, not an implementation detail, because
  the journal is keyed on it:
    - commit_native_inner reads the persisted attempt record by attempt_id
      (repository.rs:3242, read_attempt_record(&self.root, input.attempt_id)).
    - same_bindings compares attempt_id, workspace, principal, candidate_id,
      expected_parent (native_commit.rs:276-282).
    - The stale probe deliberately replays the SAME candidate bytes against the
      SAME old parent pre_tx (sley2_live_judge.py:2448) — so if it reuses the
      attempt_id from the accepted _commit_candidate call, all five bindings
      match, state is Committed, and the journal takes the replay branch
      (repository.rs:3266-3267 → replay_committed_attempt, :3514-3535) returning
      NativeCommitOutcome::Committed BEFORE the freshness gate at :3275 ever runs.
    - The probe then sees a non-failed response and the oracle rejects
      ORACLE_LAST_WRITE_WINS (sley2_live_judge.py:2450-2451) — a false positive
      for last-write-wins on a repository that actually refused nothing, because
      it was asked to replay a commit it already performed.
  The idempotency comment at repository.rs:3230-3231 ("a retry-safe resubmission
  reconciles instead of forking an attempt") actively invites the reuse reading,
  so this is the likely implementation, not a strained one. A fresh attempt_id
  per probe reaches StaleRoot correctly; reuse inverts the oracle. Required: the
  delta must state that the stale probe mints a distinct attempt_id (and that
  the frozen stale oracle asserts refusal, never replay), or explicitly exclude
  the probe from native encoding. Not resolvable in implementation without a
  contract decision — the same class of symbol-substitution defect as P1-1, one
  layer deeper.

OBSERVATIONS (not findings, no action required)
  1. Consumption-site count is nine, not eight (see NTA-P2-1). Correct in the
     next revision for accuracy; does not change the mandated edit.
  2. arm_affordances_digest() is self-derived from ARM_AFFORDANCES rather than
     compared against an external frozen literal, so it is a self-consistency
     check, not an independent pin. Pre-existing at baseline, unchanged by this
     delta, and the checker's own 18-name spec cross-check
     (check_sley2_trial_runner.py:252-258) supplies the external anchor. Noted
     only so the packet does not over-claim the digest as an independent control.
  3. Rev1 claim re-verification: the packet asserts the reviewer's rev1 citations
     stand and does not restate them. Spot-confirmed the load-bearing ones
     (server.rs:2988-2991 NATIVE_SIGNER_UNAVAILABLE before any write;
     runner.py:123 commit in ARM_DENIED_METHODS; runner.py:151 v2 pin). Accepted.

ASSUMPTIONS STATED
  1. "Tree clean" read as tracked-sources clean; the six untracked request/verdict
     paths are treated as immaterial.
  2. Contract-delta lane only. Vulcan (authority provisioning) and Nabu
     (executor wiring) judgments remain deferred per packet scope; I make no
     security or wiring finding.
  3. All findings derive from static source trace at b0e52e00 only — no build,
     no test execution, no runtime observation (read-only constraint). NTA-P1-2
     is a code-path derivation, not an executed reproduction; the v3 runner
     profile does not yet exist, so it cannot be executed at this baseline.
  4. NTA-P1-2 assumes the stale probe keeps its current "same candidate bytes,
     old parent" shape (sley2_live_judge.py:2446-2449). If the revision also
     changes the probe's candidate identity, the binding comparison changes and
     the finding must be re-derived.

VERDICT: FAIL — FAIL_NEW_P1_OPEN_NO_P0.
Prior findings: NTA-P1-1, NTA-P2-1, NTA-P2-2, NTA-P3-1 — all four CLOSED.
New findings: NTA-P1-2 (blocking). No P0.
Disposition: changes required before implementation. Rev2 is a substantive
improvement — the stale-symbol contract decision is correct and the binding
scope is now honestly stated. The single remaining blocker is a direct
consequence of the P3-1 remedy: extending the native encoder to the stale probe
introduces an attempt_id the contract must pin, or the journal's replay branch
silently converts a refusal oracle into an acceptance.
Re-review trigger: rev3 stating stale-probe attempt_id derivation (distinct per
probe) with the frozen stale oracle asserting refusal-not-replay, or explicit
exclusion of the probe from native encoding. Directions 1 (trial-only serve
flag) and 3 (additive NativeTestExecutor adapter) continue to raise no contract
objection.

REGISTER ENTRY PROPOSED (no register write performed — read-only)
  Verdict path: evidence/review/verdicts/native_trial_admission/ariadne-review-b0e52e00-rev2.md
  Scope key: native_trial_admission
  Finding id: NTA-P1-2 (new) in evidence/review/finding-register.json
  Closures to record: NTA-P1-1, NTA-P2-1, NTA-P2-2, NTA-P3-1

Evidence artifacts: none written (read-only constraint honored). Every finding
is reproducible from the cited path:line anchors at baseline b0e52e00.

Escalation note for nabu: NTA-P1-2 generalizes beyond this packet. The native
attempt journal's replay branch (repository.rs:3242-3269) returns Committed
ahead of the freshness gate, so ANY frozen oracle that probes refusal by
resubmitting identical bindings will observe acceptance rather than its pinned
refusal symbol once it moves to the native route. Worth auditing other refusal
oracles for resubmission-shaped probes before the native profile lands — the
prior escalation on STALE_ROOT pinning and this one are the same root cause
surfacing at two different layers.
```

Assumption worth flagging explicitly: I verified mechanisms, not just citations — the P1-1 closure rests on tracing `session.rs:346` → precheck fall-through → `repository.rs:3275`, plus the corroborating unit test at `repository.rs:24532`. The new blocker is a static derivation; it cannot be executed at this baseline because the v3 runner profile does not exist yet.
