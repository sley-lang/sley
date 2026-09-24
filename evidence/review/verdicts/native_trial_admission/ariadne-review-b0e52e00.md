Review complete. All nine source citations in REQ-09 verified against the baseline tree; four blocking/qualifying findings identified.

═══════════════════════════════════════════
VERDICT — REQ-09 CREATE native trial admission contract delta
═══════════════════════════════════════════

Reviewer: Ariadne (contract lane). Date: 2026-09-21.
Packet: evidence/review/requests/REQ-09-create-native-trial-admission.md
Packet sha256: 97e50c9b895be0df7fc3834f4b4ae0e4745edab5cf5435ec5a19be6ac29620da (verified, matches dispatch).
Baseline: b0e52e00a2bacfd7a874f0b4a7973a352508cebe on work/succession-sley20-arm (verified via git rev-parse HEAD).
Tree state: NOT clean — two untracked files (REQ-09, REQ-10 request packets). Tracked tree clean; no tracked-source drift. Packet claim "tree clean" is inaccurate as literally stated but immaterial to the delta.
Mode: read-only. No file writes performed.

PACKET CLAIMS CORROBORATED (independently traced, not accepted on assertion)
  Both Server constructors leave native_authority: None — server.rs:517, 556. CONFIRMED.
  set_native_authority exists at server.rs:688 with no non-test caller; only server_tests.rs:4404, 4932, 5006. CONFIRMED — serve path cannot provision authority today.
  Native commit refuses NATIVE_SIGNER_UNAVAILABLE before any write — server.rs:2988-2991, after the 4-field decode. CONFIRMED.
  commit tag routed to the native payload by negotiation, never sniffing — server.rs:1223-1229, gated on native_tests_live() (v3 + FEATURE_NATIVE_TESTS_V1, server.rs:1358-1362). CONFIRMED.
  NativeAuthority accepts no caller signing keys — server.rs:279-286. CONFIRMED.
  Supervised worker spawn of __native-test-worker — unit.rs:109. CONFIRMED.
  Trial pins v2-capable — runner.py:151; commit denied at ARM_DENIED_METHODS — runner.py:123. CONFIRMED.
  Legacy selected-test refusal preserved — repository.rs:2239-2243 (TestEvidenceUnsupported), codec.rs:286, 333, 1380 (TXN_TEST_EVIDENCE_UNSUPPORTED, unit-pinned). CONFIRMED.
  Judge encodes only the legacy 4-field commit body — sley2_live_judge.py:228-251. CONFIRMED.

FINDINGS

P1-1 (BLOCKING) — Stale-parent symbol is NOT preserved under the new profile; acceptance criterion unsatisfiable as written.
  The native route performs a session-root precheck (server.rs:2977-2982) returning SessionErrorCode::RootAdvanced → SESSION_ROOT_ADVANCED. The legacy commit path (server.rs:3046) has no such precheck and falls through to the repository, yielding CommitError::StaleRoot → "STALE_ROOT" (repository.rs:701, 724; stale detection at repository.rs:3146-3149). The judge's stale oracle reuses the same session after the head advances to H1 and pins the symbol by exact equality: `if code != "STALE_ROOT"` → ORACLE_STALE_UNREFUSED (sley2_live_judge.py:2449-2461), with the comment explicitly forbidding substring matches. Under the new profile that oracle observes SESSION_ROOT_ADVANCED and rejects. The packet's criterion "stale parent (stale-handle checks) ... refuse with preserved symbols" conflates two distinct frozen symbols. The delta must state which symbol the new profile pins, extend the judge's frozen mapping accordingly, or drive the stale probe from a fresh session so the repository path is reached. Not resolvable in implementation without a contract decision.

P2-1 (QUALIFYING) — "Old bindings untouched" contradicts the actual shape of the profile binding.
  PROFILE_ARGS is one module-level constant (runner.py:151) consumed by eight sites, including non-trial hello/version probes (runner.py:450-453) and the v2 conformance pair (runner.py:1436). Scoping v3-capable to "the trial serve process only" requires splitting the constant. check_sley2_trial_runner.py:250 asserts the tuple equals exactly ("--protocol-profile","v2-capable") and :262 requires the literal guard "protocol_version != 2". Both pins must change. The packet claims old bindings untouched while item 2 necessarily edits them; requires a named second constant and explicit statement of which checker pins move.

P2-2 (QUALIFYING) — Allowlist-unchanged claim collides with the version stamp guard.
  runner.py:933-940 binds the 18-name allowlist digest and the `protocol_version != 2` stamp in the same fail-closed path (HANDSHAKE_FAILED). A v3 handshake fails there even with the allowlist byte-identical. The packet's "agent allowlist UNCHANGED" is true of the name set but not of the guard that consumes it. Specify which guard admits 3 and how the allowlist digest stays frozen across both profiles.

P3-1 (ADVISORY) — Affected-bindings list is incomplete.
  Two judge call sites issue commits: sley2_live_judge.py:1188 (_commit_candidate) and :2449 (stale probe, direct _encode_commit_body + _raw_request). The packet names only _commit_candidate. Both need the native encoder under the new profile, or the stale probe must be explicitly excluded. Note also that legacy and native payloads share arity 4 but differ entirely in field semantics (parent/principal/now/stored vs candidate_bytes/expected_parent/attempt_id/admission_profile_id) — arity collision means a mis-scoped encoder fails deep rather than at decode.

OBSERVATION (not a finding) — Item 2 is partly already present: PROFILE_V3_CAPABLE = "v3-capable" already exists in the CLI (sley-cli/src/lib.rs:42, ProtocolProfile::V3Capable, offer at :526, :563). The "new reviewed trial profile identity" is a runner/judge binding task, not a CLI profile addition. Scope estimate should shrink accordingly.

ASSUMPTIONS STATED
  1. "Tree clean" read as tracked-sources clean; untracked request packets treated as immaterial.
  2. Contract-delta lane only. No Vulcan security judgment on authority provisioning, no Nabu judgment on executor wiring, per packet scope.
  3. Findings derived from static source trace at b0e52e00 only; no build, no test execution, no runtime observation (read-only constraint).
  4. Judge stale-oracle session reuse inferred from the code path at sley2_live_judge.py:2443-2461; not executed under a v3 profile, which does not yet exist in the runner.

VERDICT: FAIL — FAIL_NEW_P1_OPEN_NO_P0.
Disposition: changes required before implementation. P1-1 requires a contract decision on the stale symbol; P2-1 and P2-2 require the packet to state binding edits it currently disclaims. P3-1 closable in the revised packet.
Re-review trigger: revised packet resolving P1-1 and restating items 2/3 binding scope. Direction 3 (additive NativeTestExecutor adapter) and direction 1 (trial-only serve flag) raise no contract objection as written.

REGISTER NAMING CONVENTION PROPOSED (no register field exists for this scope)
  Verdict path: evidence/review/verdicts/native_trial_admission/ariadne-review-b0e52e00.md
  Scope key: native_trial_admission (matches existing snake_case scope dirs, e.g. s20_390_atomic_commit)
  Finding ids: NTA-P1-1, NTA-P2-1, NTA-P2-2, NTA-P3-1 in evidence/review/finding-register.json
  Verdict token vocabulary: reuse the house form — FAIL_NEW_P1_OPEN_NO_P0 here; PASS_PRIOR_P0_P1_P2_P3_CLOSED_NO_NEW_P0_P1_P2_P3_P4 on re-review closure.

Evidence artifacts: none written (read-only constraint honored). All findings reproducible from the cited path:line anchors at baseline b0e52e00.

Escalation note for nabu: P1-1 is a cross-lane symbol-contract question — the native route's session-root precheck changes observable refusal symbols for any existing oracle that pins repository-level codes. Worth checking whether other frozen oracles beyond the sley2 judge pin STALE_ROOT against a reused session.
