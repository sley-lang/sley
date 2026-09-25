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

Every digest below was recomputed from the retained records and matches
the qualification records exactly. The three index files:
`phase2-root-negative/qualified-fail-before.json`
`6065d40a2f97e3dbbe61aa3687a416a51d6482e0b3e6ac4a8812abc09a1181ec`;
`phase2-tests-only-validation/r3-qualified-fail-before.json`
`ad61cad8a587d2e7db9de8558738e1eabb8c198d896fca1eac3d4b976d6ecbef`;
`phase2-tests-only-validation/completion-qualified-fail-before.json`
`6075264f04a0527d4e28464532606dcc1d71112598577b969277d0726f9ce545`.

Seven runs at
`bf6821f2a1d52ae0931dec56e4332888c48ff9cb`.
R1 unnegotiated
(`bc3b40454b772a95459b0acf30c8cbff845e8382a8b61f494d5e9d9fe101f453`):
second unoffered request answers 40009 instead of 40007 at max_inflight
1. R1 stale
(`6e686fa280d77a6fa2f0ee9a9a34ad377be2de2016f49e57385ea2f5c2b14b1a`):
renewal refuses after the stale-root entity request at max_inflight 1.
R2 order
(`fd0441326c5f1f0190a836370baae5eaafc7549cd98e32a8d918b6469117271b`):
stale unoffered request reports METHOD_UNSUPPORTED instead of
SESSION_ROOT_ADVANCED. R2 debit
(`d76cb8d631fe45bd3d516551960f116d46a1cbf9dd644fa8d6617d2e887f28a6`):
three live unoffered admissions leave budget 3 instead of 0. R4 wire
(`285082e747cca28ef20f20090436a7794d11d72507f938fbf5afa37f1bde3592`):
one-below full wire length succeeds with streaming disabled. R5 hello
(`bf93117586d01d5e62ebfa2f0c3d43833e47f94eaf05df881e44d75220d9da8b`):
Hello2 under expected 2 validates Ok instead of FrameInvalid (header
level only). R6 version
(`05f0820479747ed4e223c5a90c40b91b6b3802a5b110bd92321f276952f3bc43`):
offers [3] negotiate successfully.

Two runs at
`24161f35dc4361a6d1c4859948b0c514dbd9df76`.
R3 aggregate
(`2d3d3bb29e1da542b81a9d43fdfa2e3157856a05d159dbeae951585c56f829f7`):
real seven-object method-307 query passes imports and preparation, yet
the server answers success instead of the outer-Bytes refusal. R3 writer
(`49fafa46635eeb7c1b48640fc1a1f75c108f2fa134d919d1b0b483ca53f7462f`):
the direct writer accepts a body one byte above 16777216 after
exact-limit encoder/writer controls pass.

Two runs at
`3f56d6239eb2663f8fba4a79c9244d1b781b69f1`.
R5 authenticated
(`99a351290811375c33cf85e6809bd8fd16120b247a243d509aac189768debcc4`):
authenticated Hello2 decodes successfully under expected 2. R3 metadata
(`b4eba762b0fa72fa37ba04210f10e66c10337aa540925ddfb2d3a9259842ff76`):
`frame_total_len` accepts body_len 16777217 and returns envelope length
16777426 after exact-limit metadata succeeds.

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
counter in `Server::head` asserts exactly one call through that funnel
per answer, on success and on the pre-reservation failure path, with the
response root and entity bound to the admitted revision. Source
inspection shows the entity path prepares and encodes only against the
retained revision with no other head or explicit revision load on that
path. The counter itself instruments `Server::head` only, not
`verified_revision(id)` or any future direct repository load: the
earlier cannot-miss wording is withdrawn to this bounded combination of
counter evidence plus source inspection.

`repair_wrong_epoch_refuses_entity_read_without_debit`: new minimal
`cfg(test)` epoch-rebinding seam on the authority (production API
unchanged) drives two sequential `SESSION_EPOCH_MISMATCH` (33003)
refusals at max_inflight 1 with zero debit. Renewal after epoch drift is
not covered: `renew_session` also requires epoch agreement.

`repair_one_below_session_budget_refuses_before_reserve`: selected
max_work and request max_work both stay at the observed work W, so the
request range stays admitted. One genuine admitted dispatch on a
malformed owner body (PayloadInvalid, debit 1) leaves actual remaining
budget W-1; the legal unchanged target/request/M then refuses with
LimitExceeded before reservation at one more dispatch debit with no
events, at max_inflight 1. The first revision of this test negotiated
W-1 against request M W and proved only the request-range gate; it is
replaced by the evidence above. The existing
`repair_exact_and_one_below_session_budget` name overstates its coverage:
it proves exact-W success followed by zero-budget exhaustion, not W-1;
kept unrenamed so root's recorded target identities hold. Request
max_work boundaries stay covered separately by the over-work debit case.

`entity_read_budget_debit_table_is_exact` now runs at max_inflight 1
and, after clearing the post-reservation encoding fault, performs a
successful real read asserting the response and the cumulative budget
(earlier debits plus retained full-W fault debit plus the next full W),
closing the post-reservation cleanup and viability proof without
weakening the prior phase assertions.

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
