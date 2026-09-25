# S20-330 Negotiated Session Campaign (2026-09-03)

Status: contract draft revision 3 written and implemented 2026-09-05;
2026-09-04 Nabu, Ariadne, and Vulcan reviews landed six P0s and 42 P1,
P2, and P3 items; revision 2 closed the P0s and the freeze-blocking P1s,
revision 3 answers every remaining item; Council re-reviews PASS in
round 2 (2026-09-05) after one residual each in round 1.

## Frontier at start

- SMP1 negotiates a digested profile and scopes request identity per
  session (S20-400 draft revision 4, S20-410 and S20-440 implemented).
- The S20-410 server issues provisional session identities under the
  handshake domain; the S20-320 full capsule reserved `Negotiated(2)`.
- The threat register names T15 (`SESSION_STALE_HANDLE`) and T47
  (`SESSION_WORKSPACE_MISMATCH`) for this package; the summary records the
  required binding (negotiated session, workspace, verified root, epoch)
  and the forbidden substitutes.

## Design brief

Contract: `docs/spec/SESSION_HANDLE_PROFILE_V1.md`, ADR-0033, stage
checker `scripts/check_session_handle_profile.py`.

- Thirty-fifth domain `sley2.session.v1`; the identity digests the
  handshake, workspace, accepted head root, epoch, and issuance ordinal.
- Request checks in order: session exists, workspace matches (T47), epoch
  matches, and for head-bound methods the accepted head equals the bound
  root; renewal rebinds explicitly.
- Handles are the binding positions of the bound root: exact, stateless,
  stale after any root advance (T15), unknown past the inventory.
- The capsule's `Negotiated` arm carries the session identity and the
  builder refuses foreign provenance.
- Codes 33000 through 33007.

## Open questions for the reviews

- Whether mutation methods should rebind the session implicitly after a
  successful commit instead of requiring an explicit renewal.
- Whether handles should also name query cursors or only entities.
- Whether a session should expire by request count in addition to the
  renewal limit and the S20-440 budget.

## Review answers recorded (2026-09-05, contract revision 2)

- **Implicit rebinding after commit: no.** All three reviewers agree
  explicit renewal is the correct model; silent rebinding would
  revalidate handles and capsules against a root the caller never
  accepted. Recorded in contract section 2 with the rationale.
- **Cursor handles: no.** A cursor is continuation state and would
  destroy the statelessness that makes T15 structural; cursors belong in
  `query.continue`'s `after`. Recorded in contract section 4 and
  section 8.
- **Request-count expiry: not needed.** The S20-440 budget bounds a
  session's work; the missing bound was the concurrent session count,
  closed by the negotiated `max_sessions` cap plus the remembered-close
  cap. Recorded in contract section 8.
- **Identity-domain choice (Ariadne P0-3 routed to Nabu): unpredictability
  within per-instance determinism.** The handshake carries no randomness,
  so twin handshakes are byte-identical and a deterministic identity is
  computable from public head state; determinism across servers was
  consumed by nothing but the twin test. The authority mints a
  per-instance nonce into the preimage; peer isolation is a transport
  obligation on the S20-420/430 boundary, tracked as threat T56.
  Recorded in ADR-0033 and contract sections 1 and 8.

## Implementation under the draft (2026-09-03)

- `sley-id` domain 35 `sley2.session.v1`; `crates/sley-protocol/src/session.rs`
  (`SessionAuthority`, positional handles, eight codes); the server issues,
  checks, renews, closes, and expands handles; `workspace.create` and
  `exchange.import` travel without a session (SMP1 revision 5); the capsule
  gains `build_context_capsule_bound` (S20-320 revision 2).
- Closeout `docs/audits/S20_330_NEGOTIATED_SESSION_CLOSEOUT.md`; summary
  status `S20_330_IMPLEMENTED_REVIEW_PENDING`; frontier re-anchored to the
  S20-420 JSON bridge.

Implementation commit: `a0c9a70`.

## Revision 2 (2026-09-05): the review round closed

- Per-instance server nonce in the identity preimage; twin inequality
  and restart forgetting replace cross-server determinism.
- True dispatch precedence (remembered close, live existence, admission,
  workspace, epoch, budget, bound root); `SESSION_UNKNOWN` reachable on
  the wire; budget follows binding.
- Sessionless genesis only: the exemption ends at the first head.
- `handle.expand` carries the expected root; old-root handles stay stale
  after renewal; the head-bound set is closed over fourteen methods.
- Negotiated `max_sessions` (eighth limit field, SMP1 revision 10,
  bridge revision 5) caps live sessions; remembered closes capped FIFO
  at the same number.
- `SESSION_BINDING_INVALID` enumerated; headless open travels under the
  S20-390 loader owner code; `ERROR_CODES_V1.md` freezes 33000-33007 per
  row; T15 owner corrected; T56 recorded with evidence.
- Contract pins SMP1 revision 10, capsule revision 3, STATE_ROOT S20-160
  normative; the stage checker binds server and registry markers,
  revision pins, and per-code numerics, and fails closed on drift.

## Revision 3 (2026-09-05): the residual P1, P2, and P3 items

Every one of the 18 P1, 13 P2, and 11 P3 items of the round was checked
against `main` before this revision. Most were already closed by
revision 2; the residuals, and the answers:

- The SMP1 pin had drifted again (Ariadne P3-4, Nabu P2-5, Vulcan
  P3-1 in their revision-8 form): revision 2 pinned SMP1 revision 10
  and SMP1 moved to 11 under it. The contract, ADR, and checker pin
  revision 11, and the checker now reads SMP1's own status line (and
  the capsule profile's) instead of matching a substring anywhere.
- `renewals` was a u16 range declared in a u32 field (Ariadne P3-3):
  the record field and `MAX_SESSION_RENEWALS` are `u16`; the limit is
  the field's full range, and `renewal_limit_is_exact` still walks it.
- The head-bound enumeration named four methods under wrong tags
  (`exchange.export` 207 for 210, `refs.recover` 209 for 214,
  `recovery` 603 for 504, `gc.dry_run` 601 for 212) and called the
  non-mutating `candidate.*` methods mutating (Ariadne P1-2, Nabu P1-2,
  Vulcan P2-1 asked for a rule a reader can apply). Section 3 now
  classifies every frozen tag into exactly one list (head-bound,
  handle expansion, caller-named, mutating, session and transport),
  and the checker parses the lists, the server's `head_bound` table,
  the `Method::tag` table, the reserved set, and the SMP1 method table,
  failing on any drift; drift injection in five shapes was verified to
  fail before commit. `checkout` names a `TransactionId` and stays
  caller-named (Vulcan P2-1 verified against the server).
- The stage checker never read `context_capsule.rs` (Nabu P1-7): it
  now binds the capsule module, the server's `bind_context_capsule`
  call, and the session authority's binding function, and requires
  every threat-matrix test name to exist in the sources (Vulcan P1-4).
- Cursor handles, request-count expiry, and implicit rebinding are
  explicit section 8 exclusions (Ariadne P2-4); the duplicate-identity
  refusal is stated unreachable by construction and retained as
  defense in depth (Nabu P3-2).
- The completion gate accepts a same-lane re-review PASS (Ariadne
  P2-3, Nabu P1-6), and at `IMPLEMENTED_REVIEW_PENDING` every FAIL
  round must be itemized in lane-prefixed open lists or superseded by
  that lane's PASS: the 42 items are itemized in the machine summary.

Verified already closed by revision 2 (no change): Ariadne P1-1,
P1-3 through P1-7, P2-1, P2-2, P2-5, P3-1, P3-2; Nabu P1-1, P1-3
through P1-5, P2-1 through P2-4, P3-1, P3-3, P3-4; Vulcan P1-1 through
P1-3, P2-2, P2-3, P3-2, P3-3. The item numbers are the machine
summary's `p1_open`, `p2_open`, and `p3_open` positions, taken from each
reviewer's emitted JSON in order.

## Re-review round 1 (2026-09-05, pinned at `bf715f9`)

All three lanes closed every prior item and each raised one new
residual (logs `s20-330-{ariadne,nabu,vulcan}-rereview-2026-09-05.log`):

- Ariadne FAIL, one P1: section 3 ordered the budget (5) before the
  bound root (6) while the server checks the root inside
  `session_check` before the budget, so an exhausted stale head-bound
  request answers `SESSION_ROOT_ADVANCED`, not `PROTOCOL_LIMIT_EXCEEDED`.
  Answer: the contract now says what the server does and always did,
  every binding check precedes the budget (root is check 5, budget 6),
  the precedence test proves the exhausted-and-stale case, and the T15
  matrix records it.
- Nabu FAIL, one P2 (the prior P2-4 residual): the checker verified the
  eight symbols and the eight numerics independently, so two swapped
  numerics passed. Answer: the checker parses `as_str` and `numeric`
  and binds each variant to its exact pair; a swapped pair was injected
  and caught before commit.
- Vulcan FAIL, one P3: the classification is five-way in the body and
  checker but "four" in the ADR and the revision history. Answer: both
  say five.

## Re-review round 2 (2026-09-05, pinned at `bec4468`)

Ariadne PASS, Nabu PASS, Vulcan PASS (logs
`s20-330-{ariadne,nabu,vulcan}-rereview-2-2026-09-05.log`): each round 1
residual closed, no new finding in the bounded diff. New
`ariadne_review`, `nabu_review`, and `vulcan_review` PASS obligations
supersede the three FAIL rounds; every 330 open claim reads zero.

## Tier 2 handoff gate (2026-09-03, at `a0c9a70`)

| Gate | Result |
|---|---|
| `make core` | PASS (965 tests) |
| `make conformance` | PASS |
| `make adversarial` | PASS |
| `make fuzz-smoke` | PASS |
| `make smp1-persistent-fuzz-smoke` | PASS |

Total 41 seconds. `make v1` skipped: subsystem handoff, not a release
boundary. Council reviews remain pending; the package is not complete.

## Commits

- contract draft revision 1: `c018cc8` (guard fix `3f598b4`).
- contract draft revision 2: the S20-330 revision 2 commit on `main`
  (see `git log --oneline`, closeout revision 2 for the file set).
- contract draft revision 3: `34ea7f0`; the re-review round 1 residuals
  land in the commit that adds the round 1 section.
