# AT-MW-02 phase-2 runtime correction record

Status: CORRECTED_UNVALIDATED, 2026-09-08; implementation owner: Muse 1.3
(OpenCode Go). Base: `3f56d62`. Production baseline for all fail-before
evidence: `c99503ff` (server byte-identical through the tests-only units;
`lib.rs` differed only by the ignored fixture emitter). Scope:
`crates/sley-protocol/src/lib.rs`, `crates/sley-protocol/src/server.rs`,
`crates/sley-protocol/src/server_tests.rs`, `crates/sley-protocol/src/session.rs`
(`cfg(test)` wrong-epoch seam only), this record. No phase-1 audits,
`.forge`, foreign files, or global configuration touched.

No compiler, test, Clippy, or formatter was run in this unit per the root
division of labor: root owns direct compile, identical per-defect
pass-after, remaining coverage, the full targeted protocol suite, and
Clippy at the pinned checkpoint. Any compiler or runtime defect returns to
the implementation owner. Pass-after state below is therefore predicted
from source, not executed.

## Fail-before receipts (all direct, root-qualified)

Seven runs at `bf6821f2` (`phase2-root-negative/qualified-fail-before.json`,
sha `6065d40a...18e1ec`):
R1 unnegotiated (`bc3b4045...101f453`): second unoffered request answers
40009 instead of 40007 at max_inflight 1. R1 stale (`6e686fa2...5c2b14b1a`):
renewal refuses after the stale-root entity request at max_inflight 1.
R2 order (`fd044132...9117271b`): stale unoffered request reports
METHOD_UNSUPPORTED instead of SESSION_ROOT_ADVANCED. R2 debit
(`d76cb8d6...2e887f28a6`): three live unoffered admissions leave budget 3
instead of 0. R4 wire (`285082e7...1bde3592`): one-below full wire length
succeeds with streaming disabled. R5 hello (`bf931175...75220d9da8b`):
Hello2 under expected 2 validates Ok instead of FrameInvalid (header
level only). R6 version (`05f08204...f276952f3bc43`): offers [3] negotiate
successfully.

Two runs at `24161f35` (`r3-qualified-fail-before.json`, sha
`ad61cad8...abb8c198d89`): R3 aggregate (`2d3d3bb2...c56f829f7`): real
seven-object method-307 query passes imports and preparation, yet the
server answers success instead of the outer-Bytes refusal. R3 writer
(`49fafa46...f75c108f2fa134d`): the direct writer accepts a body one byte
above 16777216 after exact-limit encoder/writer controls pass.

Two runs at `3f56d62` (`completion-qualified-fail-before.json`, sha
`6075264f...0726f9ce545`): R5 authenticated (`99a35129...189768debcc4`):
authenticated Hello2 decodes successfully under expected 2. R3 metadata
(`b4eba762...19866c10337aa`): `frame_total_len` accepts body_len 16777217
and returns envelope length 16777426 after exact-limit metadata succeeds.

Each receipt records the assertions after its first panic as unexecuted;
those branches are pass-after obligations, not baseline evidence. The
aggregate fixture proves structural inventory admission (imports plus
`prepare_verified_entity_read` success) on the serving revision; it claims
no full candidate semantic validation.

## Corrections

R1 (admitted-slot release): `dispatch_entity_read` now releases the
admitted slot on session/head binding failure with no debit, matching the
generic path. Previously the early `admits` return and the `?` binding
return skipped `complete`, leaking one inflight slot per terminal refusal.

R2 (order and debit): `dispatch_entity_read` now runs session binding,
then budget exhaustion, then method negotiation. The dispatch unit is
charged once before negotiation, and negotiation runs inside the outcome
path whose cleanup cannot be skipped, so a funded unoffered method costs
exactly 1 and stays viable. Binding failures cost 0; exhaustion costs 0
beyond prior debits. The generic path already held this order and is
unchanged.

R3 (aggregate Bytes cap): `frame_total_len` refuses body_len above the
inherited SCB `MAX_BYTE_PAYLOAD` with `LimitExceeded` before bounds
encoding, output allocation, or work reservation. Serving preflight and
the direct writer inherit it through this shared helper. Return stays the
envelope length excluding the 8-byte prefix.

R4 (full wire fit): serving preflight and `encode_single_frame_direct`
compare the checked complete wire length (envelope plus the 8-byte prefix)
against the outgoing ceiling. The prefix numeric value and frozen ingress
semantics are unchanged; bounds are not silently enlarged and entity
responses still never stream.

R5 (Hello wire 1): `validate_for_version` keeps the exact
expected-version claim checks, then rejects any Hello naming a version
other than 1 with `FrameInvalid`. Hello1/expected1 succeeds,
Hello1/expected2 stays `Downgrade`, Hello2/expected1 stays
`VersionUnsupported`, Hello2/expected2 is malformed. The wire decoder
inherits this through `from_payload_for_version`.

R6 (explicit versions 1/2 only): `negotiate_versioned` refuses a
greatest-common selection outside 1/2 with `VersionUnsupported` before
establishment, with no fallback; `Server::new_versioned` inherits it.
`from_tag_versioned` claims nothing for an undefined version. Legacy
`negotiate`, constructors, v1 bytes/tables, opaque intersections, and
reserved-tag refusal are unchanged.

Retained invariants, verified by source against the corrected paths:
the one owned `VerifiedRevision` from admission serves preparation and
encoding with no second load; pre-reservation failures cost 1, binding
failures 0, post-reservation failures full W without refund; no generic
body-byte debit applies on the entity path.

## R7 coverage (correct-invariant, not fail-before bugs)

`repair_entity_read_uses_single_admitted_head_load`: a `cfg(test)`
counter at `Server::head`, the single head-loading funnel (all other
`accepted_head` uses route through it; explicit `verified_revision(id)`
reads are a separate surface and uncounted), asserts exactly one load per
answer on success and on the pre-reservation failure path, with the
response root and entity bound to the admitted revision. A wrapper-level
counter would have missed another direct load; this one cannot.

`repair_wrong_epoch_refuses_entity_read_without_debit`: new minimal
`cfg(test)` epoch-rebinding seam on the authority (production API
unchanged) drives two sequential `SESSION_EPOCH_MISMATCH` (33003)
refusals at max_inflight 1 with zero debit. Renewal after epoch drift is
not covered: `renew_session` also requires epoch agreement.

`repair_one_below_session_budget_refuses_before_reserve`: a true W-1
session budget with identical target/request/M refuses with
`LimitExceeded` before reservation at dispatch-only debit. The existing
`repair_exact_and_one_below_session_budget` name overstates its coverage:
it proves exact-W success followed by zero-budget exhaustion, not W-1;
kept unrenamed so root's recorded target identities hold. Request
max_work boundaries stay covered separately by the over-work debit case.

Source-reviewed, not runtime-asserted: the R4 `wire_for` fixpoint
(width stability under the target server's own limit bytes) is test-side
arithmetic pinning the exact/one-below pair to the same uvar widths; the
runtime assertion is the emitted-bytes equality and the refusal itself.

## Pass-after and review obligations (root-owned)

Rerun the eleven identical negative targets on the pinned candidate so
every assertion after each baseline panic executes; run the three new R7
tests, the full targeted `sley-protocol` suite, and Clippy
`--all-targets -- -D warnings`. Then pinned independent review before any
vector authoring or contract synchronization.
