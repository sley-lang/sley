# Review request REQ-09 — CREATE native trial admission contract delta

- Baseline tree: `b0e52e00a2bacfd7a874f0b4a7973a352508cebe` on
  `work/succession-sley20-arm` (verify: `git rev-parse HEAD`; tree clean).
  Packet identity: sha256 of this file at dispatch (recorded in the gate record).
- Lane: Ariadne (contract delta). Explicitly out of scope for this packet:
  Vulcan security review of the test-authority provisioning unit (follows on the
  implementation; no release keys, no trust relaxation proposed here) and Nabu
  architecture review of the supervised-worker executor wiring.
- Governing clauses: `docs/spec/NATIVE_TEST_ADMISSION_V1.md` rev 5 (§6 lifecycle;
  Appendix A v2 envelopes; Appendix C rev 5 served commit route; Appendix D
  machine table); SMP1 method-tag ownership by negotiation (v3 + native-tests
  bit routes the existing `commit` tag to the native payload, never by
  sniffing — `crates/sley-protocol/src/server.rs:1220-1230`); CLI contract
  sections 1/2/9 (`crates/sley-cli/src/lib.rs`); trial contract
  (`bench/sley2/runner.py`: `PROFILE_ARGS` v2-capable, 18-name agent allowlist,
  `commit` denied at `ARM_DENIED_METHODS`); legacy selected-test refusal
  preserved (`crates/sley-txn/src/repository.rs:2239-2243`,
  `crates/sley-txn/src/codec.rs:286,333,1380`, unit-pinned).
- Current limitation (verified by source trace, no new feature assumed): the
  serve path never provisions commit authority — both `Server` constructors
  leave `native_authority: None` (`server.rs:517,556`); no CLI flag calls
  `set_native_authority` (`server.rs:688`); so every served native commit
  refuses `NATIVE_SIGNER_UNAVAILABLE` (`server.rs:2988-2991`) before any write.
  The trial pins `v2-capable`/protocol 2, the agent allowlist carries no commit
  route, and the judge encodes only the legacy 4-field commit body
  (`bench/fixtures/sley2_live_judge.py:228-251`). Served route, library route
  (`TransactionRepository::commit_native`), and protocol tests (App. C rev5,
  `server_tests.rs:4251+`) exist; the trial/runner integration does not.
- Proposed delta (smallest integration, additive unless noted):
  1. CLI: one trial-only serve flag provisioning `NativeAuthority` as a single
     unit from existing test fixture authority — supervised-worker executor
     (`crates/sley-test-runner/src/unit.rs:109` spawn of `__native-test-worker`,
     `worker::run_stdio`), test acceptance signer, receiver test trust
     manifests (pattern: `server_tests.rs:4341-4361`). Without the flag the
     server refuses exactly as today. No release keys; no caller keys accepted
     (`server.rs:281-286`).
  2. New reviewed trial profile identity (name pinned in implementation; never
     the old `v2-capable` string): serve/handshake `--protocol-profile
     v3-capable` for the trial serve process only; agent allowlist UNCHANGED
     (the agent never commits); judge gains a native commit encoder for the
     4-field payload (candidate_bytes, expected_parent, attempt_id, fixed
     admission profile id — `server.rs:2983-2987`) used only under the new
     profile; claim fields and `check_sley2_trial_runner.py` bindings extended
     for the new identity; old profile bytes, digests, and bindings untouched.
  3. Additive `NativeTestExecutor` adapter over the supervised worker (new code
     only; no frozen behavior change; all existing `NativeTestExecutor` impls
     stay test-only).
- Affected run/tool bindings: trial serve invocation, handshake negotiation,
  judge `_commit_candidate`, run claim rows, `check_sley2_trial_runner.py`,
  fixture digests for the new profile only.
- Executable acceptance criteria: the typed CREATE candidate (new functions +
  targeting tests, authored through the agent interface) commits through the
  new profile with production admission deriving and executing the required
  selected tests; acceptance binds candidate, parent, execution evidence, and
  receipt; wrong test expectations, missing authority (`NATIVE_SIGNER_UNAVAILABLE`),
  stale parent (stale-handle checks), and failed execution (rejection, head
  unchanged, attempt journaled for 607) all refuse with preserved symbols;
  legacy-profile runs remain byte-identical (old run/tool digests verify).
- Constraints: read-only review. No file writes. Verdict in reply text only, in
  the REQ verdict format. No register field exists for this scope yet; propose
  the verdict naming convention in the reply.
