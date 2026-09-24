# Review request REQ-09 rev2 — CREATE native trial admission contract delta

- Supersedes: `REQ-09-create-native-trial-admission.md`
  (sha256 `97e50c9b...20da`, verdict FAIL `FAIL_NEW_P1_OPEN_NO_P0`,
  transcript `evidence/review/verdicts/native_trial_admission/ariadne-review-b0e52e00.md`).
  All four findings below are answered; nothing else changed in the proposal.
- Baseline tree: `b0e52e00a2bacfd7a874f0b4a7973a352508cebe` on
  `work/succession-sley20-arm`. Lane: Ariadne (contract delta).
  Vulcan authority-provisioning review and Nabu executor-wiring review remain
  deferred to the implementation, as in rev1.
- Rev1 claims re-verified by the reviewer (server.rs:517,556,688,1223-1229,
  2988-2991; runner.py:123,151; repository.rs:2239-2243; codec.rs:286,333,1380;
  sley2_live_judge.py:228-251; unit.rs:109) stand and are not restated.

## Answers

- NTA-P1-1 (stale symbol): proposed contract decision — the new profile pins
  the repository-level symbol by driving the stale probe from a FRESH session
  (the judge already opens fresh sessions; pattern at
  `sley2_live_judge.py:2463`), so the native route reaches the repository
  stale path and the frozen `"STALE_ROOT"` exact-equality pin
  (`sley2_live_judge.py:2457-2460`, sole pinning site in the judge) is
  preserved unchanged. A reused-session probe under the new profile observes
  `SESSION_ROOT_ADVANCED` (native precheck `server.rs:2977-2982`); that symbol
  is recorded as expected-non-acceptance on the reused-session path, never as
  a substitute pin. No frozen symbol changes; no substring matching.
- NTA-P2-1 (profile binding): restated item 2 — introduce a named second
  constant (e.g. `NATIVE_TRIAL_PROFILE_ARGS = ("--protocol-profile",
  "v3-capable")`); `PROFILE_ARGS` stays byte-identical for all eight current
  consumption sites (hello/version probes at `runner.py:450-453`, v2
  conformance pair at `:1436`, trial serve/handshake). Only the trial serve
  and handshake call sites for the new profile use the new constant.
  Checker pins that move, explicitly: `check_sley2_trial_runner.py:250`
  (tuple assertion becomes per-profile) and the `:262` guard-marker set
  (gains the new-profile admission rule); old-profile assertions unchanged.
- NTA-P2-2 (version stamp): the 18-name set is unchanged AND its digest stays
  frozen; the guard at `runner.py:933-940` becomes per-profile (admits stamp
  3 only under the new profile identity; stamp 2 path byte-identical).
  Allowlist digest and stamp rule are stated as a pair in the revised text.
- NTA-P3-1 (second commit site): both judge commit sites named —
  `sley2_live_judge.py:1188` (`_commit_candidate`) and `:2449` (stale probe,
  direct `_encode_commit_body` + `_raw_request`). Both switch to the native
  encoder under the new profile via profile-gated encoder selection (legacy
  encoder unreachable under the new profile and vice versa), mitigating the
  arity-4 field-semantics collision the reviewer notes.
- Observation accepted: no CLI profile addition — `PROFILE_V3_CAPABLE`
  exists (`sley-cli/src/lib.rs:42`, offer at `:526,:563`). Item scope is
  runner/judge bindings + the trial-only serve flag (direction 1, no contract
  objection) + the additive executor adapter (direction 3, no contract
  objection).

## Unchanged acceptance criteria (rev1), with the P1-1 pin stated

Typed CREATE candidate through the agent interface → native commit under the
new profile → production admission derives/executes required selected tests →
acceptance binds candidate, parent, evidence, receipt. Negatives: wrong
expectations refuse (preserved symbol); missing authority
(`NATIVE_SIGNER_UNAVAILABLE`); stale parent via fresh session pins
`STALE_ROOT` exactly; failed execution rejects with head unchanged and attempt
journaled for 607. Legacy-profile runs byte-identical (old run/tool digests
verify).

- Constraints: read-only review. No file writes. Verdict in reply text only,
  in the REQ verdict format, against scope key `native_trial_admission`.
