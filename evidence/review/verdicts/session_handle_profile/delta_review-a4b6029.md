# session_handle_profile current-delta review — session r4 (delta f1684a9) at scope a4b6029

Scope: `a4b60294e4dd75133fa08f92bf16022fb05bb807` (verified `git rev-parse HEAD`).
Delta: `f1684a9` (session contract revision 3 → 4: protocol-v2 classification
extension only). No prior council verdicts at this revision; summary
`session_handle_profile.current_delta_review.{ariadne,nabu,vulcan}` all PENDING,
which is the legal state for `S20_330_IMPLEMENTED_REVIEW_PENDING` (checker
`check_current_delta_review` accepts PENDING outside frozen/complete).

Method: read `docs/spec/SESSION_HANDLE_PROFILE_V1.md` (full, 389 lines),
`scripts/check_session_handle_profile.py` (full, 571 lines), `git show f1684a9
--stat` plus the session-profile diff, and the summary section; re-derived the
claims below against `crates/sley-protocol/src/server.rs`, `src/lib.rs`
(registry/method table), `docs/spec/SMP1.md` (r12), and
`docs/spec/ENTITY_READ_PROFILE_V2.md` §4 instead of trusting the checker.

Checker result: `python3 scripts/check_session_handle_profile.py` → exit 0,
`result PASS`, `problems []`, `revision 4`, `smp1_revision 12`,
`capsule_revision 3`, `status S20_330_IMPLEMENTED_REVIEW_PENDING`.

## Ariadne (contract) — PASS, 0 findings

- Status pin verified: spec header says revision 4; SMP1 header is r12 and
  capsule header is r3, matching the composed-authority pins (checker
  `smp1-revision-pin` / `capsule-revision-pin` logic confirmed by reading both
  headers directly).
- v1 partition complete and unchanged: prose lists unchanged; code `ALL` still
  41 tags, reserved still {305,503,601,602}, `session.open` (100) still precedes
  all checks. Extension paragraph adds exactly {306 `entity.version`,
  307 `entity.signature`} to head-bound in v2 only, no other list changed.
- v1-refusal ordering cross-checks out: spec says v1 sessions refuse 306/307
  with `PROTOCOL_METHOD_UNSUPPORTED` before checks, citing SMP1 §2 and
  ENTITY_READ §4. Server `dispatch()` resolves via `from_tag_versioned`
  (which returns `MethodUnsupported` for v1 selections) before liveness,
  admission, and `session_check`; SMP1 §2 states the refusal lands at
  method-tag validity before session routing with `NEVER`/no-dispatch-unit
  semantics, and ENTITY_READ §4 keeps admission order otherwise unchanged.
  No contradiction; retryability/cost live in SMP1, correctly not restated.
- §9 revision-4 note is accurate (v2-only join; v1 partition, handle bytes,
  capsule arm, code rows unchanged); `33000 through 33007` frozen per-row in
  ERROR_CODES_V1 with the r4 sentence (checker `error-codes:*` clean); ADR-0033
  and WORK_PACKAGES r4/SMP1-r12 markers present (checker `adr-*`,
  `work-package-revision` clean).

## Nabu (architecture) — PASS, 0 findings

- No forked table: `head_bound_versioned` = `Self::head_bound(method) ||
  matches!(EntityVersion | EntitySignature)` (server.rs), i.e. legacy set
  union exactly the two additions; `V2_ALL − ALL` = {EntityVersion,
  EntitySignature}; `ENTITY_VERSION_TAG`/`ENTITY_SIGNATURE_TAG` resolve to
  306/307. v1 partition equality (`covered_v1 == v1 tags`) holds.
- Single-head-load retained path: `session_check_retained` loads the binding
  once and both the legacy and entity-read paths funnel through
  `authority.check_session` with the same binding; `entity_read` prepares over
  the retained `VerifiedRevision` only.
- Order preserved on both paths: tag validity → liveness → admission →
  binding → budget → negotiation. Entity path does binding, then budget, then
  `profile.admits` negotiation, matching its documented inherited order.
- Version gating is airtight: on a version-aware server with a v1 selection,
  entity tags are refused in `from_tag_versioned` before routing, so
  `head_bound_versioned` is unreachable for them; on legacy servers `from_tag`
  + `head_bound` are v1-only and textually unchanged by the delta.

## Vulcan (surface) — PASS, 0 findings

- Wire-observable v1 behavior: 306/307 → `PROTOCOL_METHOD_UNSUPPORTED` before
  any session/admission/budget state is touched (no slot admitted, no debit).
- Wire-observable v2 behavior: `SESSION_UNKNOWN` / `PROTOCOL_SESSION_CLOSED`
  / workspace / epoch / `SESSION_ROOT_ADVANCED` (stale bound root) precedences
  preserved via the shared authority check; owner-level `QUERY_*` (e.g.
  `QUERY_ROOT_MISMATCH` for expected-root mismatch) fires only after, per
  ENTITY_READ §4 — consistent with the profile's claim set.
- Required evidence (§7) intact: T15/T47/T56 `matrix.json` files exist, report
  `PASS` with non-empty test lists, and every named test resolves to a real
  `fn` in `session.rs`/`server_tests.rs` (checker threat-evidence clean). The
  delta adds no new §7 matrix obligation and makes no runtime/capability
  claim — phase-3 runtime and independent vectors are explicitly declared
  outstanding in the delta commit message and spec status, not silently
  claimed.

---
VERDICT: PASS / SECTION: session_handle_profile / FIELD: current_delta_review.ariadne / SCOPE_SHA: a4b60294e4dd75133fa08f92bf16022fb05bb807 / FINDINGS:
(none)
---
VERDICT: PASS / SECTION: session_handle_profile / FIELD: current_delta_review.nabu / SCOPE_SHA: a4b60294e4dd75133fa08f92bf16022fb05bb807 / FINDINGS:
(none)
---
VERDICT: PASS / SECTION: session_handle_profile / FIELD: current_delta_review.vulcan / SCOPE_SHA: a4b60294e4dd75133fa08f92bf16022fb05bb807 / FINDINGS:
(none)
