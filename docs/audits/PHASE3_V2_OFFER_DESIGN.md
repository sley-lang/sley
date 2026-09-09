# Phase-3 v2 endpoint offer design

Status: REVIEWED_DESIGN, 2026-09-09; owner: opencode integrator.
Ariadne PASS (round 5), Vulcan PASS (round 4), Nabu PASS (round 2,
carried forward). P1 may proceed.
Ends when P1-P3 land and AT-MW-02 I4b consumes the offer.

Slice: `.forge/slices/phase-3-v2-endpoint-offer.json` (lane `feat/phase-3-v2-endpoint-offer`).
Baseline: `main@7544ec1` (AT-MW-02 merged, locator re-pinned).
Authority: `docs/spec/ENTITY_READ_PROFILE_V2.md` sections 6-7 (reviewed contract `48070373`);
SMP1 rev 12 negotiation; SLEY_CLI_V1 rev 5 hello pin; SLEY2_TRIAL_RUNNER_V1 ARM pin.

## Problem

Methods 306 `entity.version` and 307 `entity.signature` are fully specified
(V2 contract), served under a v2 selection (`Method::introduced_in`,
`from_tag_versioned`, server dispatch), and present in the v2 bridge table
(43 rows), but no production offer carries them: `sley hello` serves
`Server::offered_hello` (v1, 41 methods, pinned by SLEY_CLI_V1 section 2),
and the trial runner's `ARM_AFFORDANCES` is v1-pinned, so naming 306/307
fails the handshake (`HANDSHAKE_FAILED`, runner.py missing-check) rather
than dispatching. `Server::offered_hello_versioned` exists but is test-only.

## Decisions (revision 3: round-2/3 findings incorporated)

1. **v1 default frozen.** `sley hello` output bytes, the 41-method v1 offer,
   bridge default-embed parity, and all v1 byte vectors stay unchanged.
   No v1 pin is touched.
2. **Adopt the declared section 9 surface; no new flag.** SLEY_CLI_V1
   section 9 already declares the phase-3 surface, and it forbids a
   `--protocol-version` alias, so the revision-1 proposal of
   `sley hello --protocol-version 2` is withdrawn. P1 implements
   `--protocol-profile v2-capable` for `hello`, `methods`, `version`,
   `serve`, `frame encode`, and `frame decode` exactly as section 9
   specifies: the `[1,2]` offer without forcing selection 2,
   `--expected-version 1|2` on the frame commands with exact wire
   semantics, `sley2-cli-v2` capable metadata carrying
   `protocol_profile` and offered `protocol_versions`, and
   `sley2-cli-report-v2` with `protocol_profile` and the actual
   `selected_protocol_version`. Implementation adds a separate
   profile-aware entrypoint/options carrier; the existing four-field
   `ServeOptions` and the `serve` legacy wrapper are preserved.
   CLI rev 5 to 6 retires the "declared, not implemented" caveat, and
   P1 synchronizes every record that repeats it: ADR-0035 (rev-6
   record), the WORK_PACKAGES S20-430 row, the machine-summary `cli`
   section (`contract_revision` 6, status, current-delta review, and
   the `offered_hello` field records BOTH the legacy default
   `Server::offered_hello` and the capable
   `Server::offered_hello_versioned` path), and
   `scripts/check_cli_contract.py` (`SPEC_REVISION` 6 plus
   capable-runtime markers; legacy markers retained for the default
   path, which stays legacy-only). The checker amendment is
   fail-closed: exact revision-6/capable-runtime markers for the ADR
   and work-package records, so stale rev-5/pending prose cannot pass
   on generic markers alone. Reciprocally, the SMP1 composition text
   (SMP1.md current-composition paragraph, still revision 12: a
   composition-only update, no SMP1 bump) moves its CLI pin from
   revision 5 to 6, with `scripts/check_smp1_contract.py`
   (reverse-pin) and `scripts/test_smp1_contract.py` (`CLI_PIN` plus
   shadow cases) updated in the same commit.
3. **Serve path is version-aware end to end.** A hello-only selector
   cannot admit a v2 session: under the profile, `serve` offers
   `Server::offered_hello_versioned`, negotiates with
   `negotiate_versioned` / `negotiate_identity_versioned`, and serves
   through `Server::new_versioned` (server.rs:270), so decoding,
   dispatch, and response framing all follow the selection. Legacy
   `serve` remains the wrapper over `Server::new` with byte-identical
   behavior. A legacy client hello against a capable server still
   negotiates v1 and filters 306/307 per rev-12 rules.
4. **No SMP1 revision bump.** Revision 12 already defines v1/v2
   negotiation and the explicit version-aware entrypoints; the offer
   rule for the CLI profile lives in SLEY_CLI_V1. If P1 surfaces any
   normative SMP1 change, the bridge, session, and required-index pins
   synchronize reciprocally per V2 contract section 6 before merge.
5. **Allowlist admission (P2).** Append `entity.version` and
   `entity.signature` to `ARM_AFFORDANCES` in tuple order (after
   `compare`, before `handle.expand`). Neither is in
   `ARM_DENIED_METHODS`, so the denied invariant holds.
   `arm_affordances_digest` rotates by construction; no digest literal
   is pinned anywhere, so the rotation rides in claims. Bump
   SLEY2_TRIAL_RUNNER_V1.
6. **Bind the claim digest to one immutable snapshot (P2).**
   `run_scripted_trial` guards the handle with its `affordances`
   argument (runner.py:864,958,980) but the claim recomputes the
   digest from global `ARM_AFFORDANCES` (runner.py:1058), so a
   differing supplied list could exercise one surface while claiming
   another; worse, the guard closes over the mutable input list while
   `EndpointHandle` independently tuples it (handle.py:35-38), so
   mid-trial mutation could split exercised admission from the final
   digest. P2 snapshots `tuple(affordances)` once before execution
   and uses that single tuple for guard checks, handle exposure, and
   `_canonical_sha256`. Pre-existing defect, fixed here because P2
   owns the digest rotation.
7. **Runner version selection (P3).** v1 runs keep `sley hello` plus
   legacy `serve`. v2 runs use the profile hello AND launch
   `serve --protocol-profile v2-capable`; `request_frame` stamps the
   actual selected version instead of hardcoded 1 (runner.py:288-298,
   323-330), otherwise a v2 run negotiates v1 or trips
   `PROTOCOL_DOWNGRADE`. The allowlist/denied coverage test gains
   positive admission assertions: both new names allowed, not
   denied, present in the v2 offer, absent from the v1 offer. Prove
   serving with a live v2 round trip of 306/307 against the trial
   endpoint; that round trip is the serving prerequisite for I4b,
   not I4b itself.
8. **Out of scope.** I4b demonstrations stay in AT-MW-02; no method
   beyond 306/307; no schema-epoch, identity, or frame-format change;
   no v1 behavior change of any kind.

## Open questions for review (round 4)

- Ariadne: is the CLI rev-6 synchronization list (decision 2: spec,
  ADR-0035, WORK_PACKAGES row, machine-summary `cli`, checker markers)
  complete, with legacy markers correctly retained for the default path?
- Vulcan: do the profile-`serve` + selected-version stamping (decision
  7) and the single-snapshot digest binding (decision 6) close the
  round-3 blockers without opening new ones?
- Nabu PASS (round 2) carries forward: revision 3 changes no
  architectural conclusion (same two affordances, same owner boundary);
  Nabu is not re-dispatched unless a round-4 finding alters the
  serving architecture.

## Review record

- Round 1 (2026-09-09, revision 1): Ariadne FAIL (3 findings: serve path
  missing, `--protocol-version` contradicts CLI rev 5 section 9, SMP1
  rev-13 bump unnecessary/incomplete); Vulcan FAIL (3 findings: v2
  serving mode underspecified, claim/exercise digest gap in
  `run_scripted_trial`, coverage test lacks positive admission); Nabu
  no result (handoff timed out). All six findings incorporated in
  revision 2.
- Round 2 (2026-09-09, revision 2): Nabu PASS (high confidence, no
  blockers; owner boundary preserved, adapter thin, delta widens no
  whole-root path; notes `compare` as a pre-existing extraction route
  untouched by this delta). Ariadne dispatch timed out; Vulcan dispatch
  timed out. Both retried as round 3.
- Round 3 (2026-09-09, revision 2): Ariadne FAIL (1 blocker: CLI rev-6
  must synchronize ADR-0035, WORK_PACKAGES S20-430 row, machine-summary
  `cli` section, and `check_cli_contract.py` markers; serve route and
  ServeOptions compatibility otherwise pass). Vulcan FAIL (2 blockers:
  runner v2 path must launch profile `serve` and stamp the selected
  version; digest fix needs one immutable snapshot shared by guard,
  handle, and digest). All three findings incorporated in revision 3,
  re-dispatched as round 4.
- Round 4 (2026-09-09, revision 3): Vulcan PASS (high confidence, no
  blockers). Ariadne FAIL (3 blockers: SMP1 composition CLI pin +
  reverse-pin checker/test hardcode rev 5; checker markers not
  fail-closed for revised records; machine-summary `offered_hello`
  names only the legacy path). Incorporated in revision 4.
- Round 5 (2026-09-09, revision 4): Ariadne PASS, no blockers. One
  implementation note (not blocking): add `offered_hello` to the
  structured summary assertions (`check_cli_contract.py:160-168`
  expected map), folded into P1. Design gate complete:
  Ariadne PASS, Vulcan PASS, Nabu PASS.

## Handoff to AT-MW-02 I4b (P4, 2026-09-09)

I4b's dependency is satisfied: the v2 offer is live and the arm can
name both methods. What I4b consumes:

- Offer: `sley hello --protocol-profile v2-capable` (versions [1,2],
  43 methods); `sley serve --protocol-profile v2-capable` negotiates
  and serves version-aware; SLEY_CLI_V1 rev 6 specifies the surface.
- Allowlist: `ARM_AFFORDANCES` holds eighteen names including
  `entity.version` and `entity.signature`; claim digests bind the
  per-trial snapshot (SLEY2_TRIAL_RUNNER_V1 rev 4).
- Serving proof: live v2 round trip sends `entity.version` with an
  empty body and gets 40008 past method resolution (smoke evidence
  `v2_entity_read_dispatched`); the v1 surface cannot name 306
  (42003 rejection, `v1_entity_read_still_gated`). Trials run
  selection 2 end to end (seed, open, requests stamped 2).
- Pre-session frames carry identifier 0; session-bound traffic starts
  at 1 (`run_scripted_trial` enforces the version explicitly).

Not claimed: I4b demonstrations themselves (no real entity read was
performed; the round trip proves dispatch, not retrieval); S20-430
rev-6 and S20-620 rev-4 Council contract reviews (machine-summary
`current_delta_review` stays PENDING on both — the P0 rounds reviewed
this design, not the contract deltas); smoke fully green
(`no_failed_answers` still red on the pre-existing scripted
`handle.expand` body `00`, which fails identically under v1 and needs
a genuine expected-root body under S20-620 maintenance); the
pre-existing `check_error_symbol_registration` drift (fails
identically on the pristine base). Merge to main needs operator
approval and is not taken here.
