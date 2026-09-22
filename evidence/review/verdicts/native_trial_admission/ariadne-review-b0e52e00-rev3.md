Full trace complete at baseline. Verdict follows.

```
═══════════════════════════════════════════
VERDICT — REQ-09 rev3 CREATE native trial admission contract delta
═══════════════════════════════════════════

Reviewer: Ariadne (contract lane). Date: 2026-09-21.
Packet: evidence/review/requests/REQ-09-rev3-create-native-trial-admission.md
Packet sha256: 248852dcff331475fdae4d2d46a06804f64221c06361216863fd8cd13f756078
  (verified by independent sha256sum, matches dispatch)
Baseline: b0e52e00a2bacfd7a874f0b4a7973a352508cebe on work/succession-sley20-arm
  (verified via git rev-parse HEAD)
Supersedes: REQ-09 rev2 (dd9a02ce…177d1), verdict FAIL_NEW_P1_OPEN_NO_P0
Scope key: native_trial_admission
Tree state: tracked tree clean; 8 untracked paths (REQ-09/-rev2/-rev3,
  REQ-10/-rev2/-rev3, two verdict dirs). Immaterial to the delta.
Mode: read-only. No file writes performed.

PRIOR FINDING DISPOSITION

NTA-P1-2 (was BLOCKING) — CLOSED. Mechanism re-derived end to end at baseline,
not accepted on the packet's assertion. I walked every refusal point between
entry and the freshness gate to confirm the probe actually arrives there:

  Precheck. A fresh session binds bound_root to the current head
  (session.rs:342-350, :346). commit_native_route reads session_root from the
  authority record (server.rs:2972-2976) and compares it to head_binding_mixed()
  (:2977-2978). Fresh session at H1 ⇒ EQUAL ⇒ no RootAdvanced (:2979-2981);
  control falls through to field decode (:2983-2987) and commit_native (:3010).

  Path to the gate (commit_native_inner, repository.rs:3129-3281), with
  expected_parent = pre_tx and actual = H1:
    :3144 read_receipt_any_readonly(pre_tx) — pre_tx is a previously accepted
          receipt, so this resolves. Note the belt-and-braces arm at :3146-3151:
          a read failure with expected_parent != actual ALSO returns StaleRoot.
          Both branches converge on the pinned symbol.
    :3187 validate_candidate_bytes against the OLD parent — passes, and this is
          load-bearing by design, not luck: the comment at :3141-3143 states
          validation runs against expected_parent precisely so head movement
          does not disturb binding completeness.
    :3210 check_admission_profile_binding — see NTA-P2-3 below.
    :3242 read_attempt_record(input.attempt_id) — with a DISTINCT attempt_id this
          returns None (native_commit.rs:794, NotFound ⇒ Ok(None)), takes the
          :3243-3252 arm, writes the Admitted record, and never enters the
          Some(existing) arm at all.
    :3275 actual != expected_parent ⇒ transition to AbortedBeforePromotion,
          return CommitError::StaleRoot.
  CommitError::code() maps StaleRoot to the exact literal "STALE_ROOT"
  (repository.rs:724, re-read this revision), satisfying the frozen
  exact-equality pin at sley2_live_judge.py:2457-2460.

  The replay inversion is closed by construction, and more strongly than the
  packet claims — see observation 1.

  Collateral check the packet does not make, and which I verified because the
  fix introduces a write on a refusal path: the stale probe now persists an
  AttemptRecord (:3251) before refusing. write_attempt_record targets
  attempts_dir (native_commit.rs:800-802), not the head, so the judge's
  subsequent ORACLE_STALE_PARTIAL_WRITE check (sley2_live_judge.py:2461-2468,
  fresh session must still read H1) is unaffected. The aborted journal entry is
  the documented intent (:3272-3274). No false partial-write positive.

NEW FINDINGS

NTA-P2-3 (QUALIFYING) — The packet pins three of the four native payload fields
for the stale probe; field[3] admission_profile_id is left unstated.
  Rev3 claims to pin "the full probe shape," and for the hazard it targets it
  does. But the native record is arity 4 (server.rs:2983): field[0] candidate,
  field[1] expected_parent, field[2] attempt_id, field[3] admission_profile
  (:2987). The legacy body the probe encodes today carries (parent, principal,
  now, stored) — sley2_live_judge.py:238-251 — so BOTH field[2] and field[3]
  are new probe-supplied values with no legacy analogue. Rev3 derives field[2]
  and is silent on field[3].
  This matters because the binding check sits UPSTREAM of the freshness gate:
  check_admission_profile_binding (repository.rs:3210 → native_commit.rs:516-525)
  refuses with ReceiptBindingMismatch whenever claimed != fixed.id(), at :3210,
  sixty-five lines before the stale gate at :3275. A probe carrying the wrong
  profile id never reaches STALE_ROOT.
  Classified P2, not P1, and I want the reasoning on record because it is the
  exact axis on which NTA-P1-2 was blocking:
    - Exactly one value is admissible (fixed_native_admission_profile().id(),
      native_test_plan.rs:209), so this is code-determined, not a contract
      choice between plausible alternatives.
    - The accepted _commit_candidate call must already supply that same value to
      succeed, so a shared constant is the natural implementation.
    - Failure is LOUD and self-identifying: TXN_RECEIPT_BINDING_MISMATCH
      (codec.rs:335) is not "STALE_ROOT", so the judge rejects
      ORACLE_STALE_UNREFUSED and prints the observed symbol
      (sley2_live_judge.py:2459-2460). It cannot produce a false acceptance.
  That is the decisive difference from NTA-P1-2, where the natural choice was
  wrong AND silently inverted the oracle into a fabricated last-write-wins
  defect report. Here the worst case is a self-diagnosing red run. Pin field[3]
  to the fixed profile id in the delta text for completeness; do not hold
  implementation for it.

OBSERVATIONS (not findings, no action required)
  1. Rev3's mechanism description for the replay exclusion is imprecise in the
     packet's favor. It states that same_bindings (native_commit.rs:276-282)
     "cannot match." In fact a distinct attempt_id makes read_attempt_record
     return None at repository.rs:3242, so same_bindings is never consulted and
     the Some(existing) arm is never entered. The conclusion — replay branch
     unreachable — holds, and holds more strongly than written. No correction
     required; noted so the record reflects the true control flow.
  2. Rev3 cites reviewer assumption 4 from the rev2 verdict as anticipating this
     re-derivation. Assumption 4's trigger was a change to the probe's CANDIDATE
     identity; rev3 preserves same-candidate-bytes/old-parent and instead changes
     session freshness and attempt_id. The assumption's condition was therefore
     never activated. Harmless framing, but the closure rests on attempt_id
     distinctness directly, not on that assumption.
  3. Bookkeeping correction accepted and independently confirmed: 10 PROFILE_ARGS
     occurrences = 1 definition (bench/sley2/runner.py:151) + 9 uses (:450, :452,
     :453, :1384, :1388, :1436, :1529, :1568, :1596). Rev3's "nine consumption
     sites (1 definition + 9 uses)" is correct as written. Observation 1 from the
     rev2 verdict is discharged.
  4. Checker pins re-confirmed unchanged at baseline: check_sley2_trial_runner.py
     :250 asserts the tuple equals exactly ("--protocol-profile","v2-capable"),
     and the guard-marker set at :259-266 carries "protocol_version != 2". The
     per-profile edits the packet commits to remain necessary and sufficient.
  5. Authority precondition, flagged for the Vulcan lane rather than as a finding:
     the native route refuses NATIVE_SIGNER_UNAVAILABLE at server.rs:2988-2991
     before commit_native runs. The stale probe inherits this. It is satisfied in
     practice because the probe only executes after an accepted native commit in
     the same run, which already required provisioned authority. No contract
     objection; noted so the dependency is explicit.

ASSUMPTIONS STATED
  1. "Tree clean" read as tracked-sources clean; the eight untracked
     request/verdict paths are treated as immaterial.
  2. Contract-delta lane only. Vulcan (authority provisioning) and Nabu
     (executor wiring) judgments remain deferred per packet scope; I make no
     security or wiring finding.
  3. All findings derive from static source trace at b0e52e00 only — no build,
     no test execution, no runtime observation (read-only constraint). The v3
     runner profile does not exist at this baseline, so the probe cannot be
     executed here; NTA-P1-2's closure and NTA-P2-3 are both code-path
     derivations, not executed reproductions.
  4. I assume the probe's candidate bytes remain byte-identical to the accepted
     call's (rev3 item 1). If implementation varies the candidate, validation at
     repository.rs:3187 runs against a different plan and both the profile
     binding at :3210 and the closure above must be re-derived.
  5. I assume "fresh random 16 bytes" means cryptographically unpredictable and
     never persisted across probes; a PRNG reseeded per run that could repeat a
     prior probe's id would reopen the journal path at :3253.

VERDICT: PASS — PASS_QUALIFYING_NO_P0_NO_P1.
Prior findings: NTA-P1-1, NTA-P2-1, NTA-P2-2, NTA-P3-1 (closed at rev2) and
NTA-P1-2 (closed here) — all five CLOSED. New findings: NTA-P2-3 (qualifying).
No P0, no open P1.
Disposition: cleared for implementation. Fold NTA-P2-3 into the delta text at
implementation time; it does not warrant a rev4 review cycle. Rev3 resolves the
blocker correctly and for the right reason — the two hazards rev2 conflated are
now separated, each with its own mechanism (fresh session for the precheck,
distinct attempt_id for the journal), and the frozen oracle is left untouched
while becoming genuinely refusal-asserting. Directions 1 (trial-only serve flag)
and 3 (additive NativeTestExecutor adapter) continue to raise no contract
objection.
Re-review trigger: none required for this scope key. Re-open if implementation
changes the probe's candidate identity (assumption 4), reuses attempt_ids across
probes (assumption 5), or alters any frozen symbol or the exact-equality pin at
sley2_live_judge.py:2457-2460.

REGISTER ENTRY PROPOSED (no register write performed — read-only)
  Verdict path: evidence/review/verdicts/native_trial_admission/ariadne-review-b0e52e00-rev3.md
  Scope key: native_trial_admission
  Finding id: NTA-P2-3 (new) in evidence/review/finding-register.json
  Closure to record: NTA-P1-2

Evidence artifacts: none written (read-only constraint honored). Every finding is
reproducible from the cited path:line anchors at baseline b0e52e00.

Escalation note for nabu: the rev2 escalation stands and is now sharper. The
generalized hazard is that the native route adds TWO caller-supplied fields with
no legacy analogue — attempt_id and admission_profile — and both are evaluated
BEFORE the freshness gate (repository.rs:3210 and :3242, versus :3275). Any
frozen oracle migrating from the legacy encoder to the native one must derive
both, or it will observe an upstream symbol instead of its pinned one. attempt_id
is the dangerous half because it fails silently into acceptance; admission_profile
fails loudly. Recommend auditing the remaining refusal oracles for this
two-field derivation gap before the native profile lands, rather than fixing them
one packet at a time as they surface.
```

Three notes on what I did beyond checking the packet's arithmetic:

I verified mechanisms, not citations. Every line reference in rev3 is accurate, but accuracy of citation is not the same as soundness of claim — so I walked the whole path from `commit_native_route` entry to the freshness gate looking for anything that refuses first. That search is what produced NTA-P2-3.

I deliberately did not inflate NTA-P2-3 to blocking. It is structurally the same gap as the blocker I raised in rev2 (an unpinned native field), and there is an obvious temptation to treat it identically for consistency. But the failure modes differ in kind: reusing an attempt_id silently manufactures a false defect report, while a wrong admission profile produces a distinct symbol the judge prints verbatim. Only the silent one justifies holding implementation.

The one thing I could not do at this baseline is execute the probe — the v3 runner profile does not exist yet, so both the closure and the new finding are static derivations. Assumptions 3 through 5 mark exactly where that limitation bites.
