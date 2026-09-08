# AT-MW-02 phase-1 repair record

Status: REPAIRED_PENDING_REVIEW, 2026-09-08; owner: Muse 1.3 (OpenCode Go).
Scope: `crates/sley-query/src/entity_read.rs`,
`crates/sley-repo/src/entity_read.rs`, this record.
Base: `e443850`; reviewed contract `docs/spec/ENTITY_READ_PROFILE_V2.md`
at `aff5164` unchanged. Accepts Ariadne findings A1-A5 plus R1-R2 (Nabu
concurs on A2 design). No protocol, session, bridge, CLI, or runner
changes.

## Defects and corrections

A1 (canonical wire): response object field 4 now carries the canonical SCB
Bytes value (`len || exact stored object`), encoded, decoded with
`ScbValueCursor::read_bytes` plus exact exhaustion, and counted with its
inner prefix in checked body preflight. Inherited SCB ceilings
(`MAX_BYTE_PAYLOAD`, `MAX_COLLECTION_ELEMENTS`, `MAX_STANDALONE_BYTES`,
record field counts) are enforced with checked arithmetic before
encoding or output allocation; violations are `BudgetExceeded`. Stored
object bytes are still copied verbatim; no entity-body reserialization.

A2 (plan binding): `EntityReadPlan<'rev>` is opaque with private fields;
its only constructor is `prepare_entity_read`, which copies the borrowed
revision view and the decoded request into the plan and checks the
selected ceilings there. The lifetime binds the backing revision slices.
`encode_entity_read_response` takes the plan by value plus `SessionId`
and keeps no `Clone`; one preparation encodes once. All binding, epoch,
parameter relationship, and preflight checks run in preparation; the
redundant `verify_plan` (including its early Signature return at base
`e443850` `entity_read.rs:659`) is removed. Read-only scalar accessors:
`work_units`, `body_len`, `object_count`. The plan reserves no work and
certifies no session authority; reservation and live session/root
admission stay with later protocol work. The thin verified-repo adapter
borrows the local owned `VerifiedRevision`.

A3 (failure codes): zero in any unsigned request ceiling is `NotCanonical`;
only positive over-ceiling limits are `BudgetExceeded`. Shape, zero-limit,
positive-range, then root precedence is preserved, including a competing
zero plus wrong-root case.

A4 (allocation): the nested owned record/list encoder is replaced with a
direct writer into one `Vec::with_capacity(body_len)`, emitting checked
canonical prefixes and copying each stored object exactly once. Shared
`sley-scb1` codecs are untouched; only this fixed response wrapper has a
private framing writer. The `1 + K*L + 2*B + max_response_bytes` formula,
tokens, and budget are unchanged. One buffer and one stored copy per
object are established by source inspection of the writer, not by
allocation instrumentation or measured counts.

A5 (precedence): the target byte-payload ceiling is enforced after method
applicability instead of in target resolution. `Version` checks it
immediately; `Signature` establishes the Function first, then checks it
before parameter traversal or copying. An oversized non-Function answers
`ClassNotApplicable` for `Signature` and `BudgetExceeded` for `Version`.

## Evidence

Fail-before receipt (6 discriminating failures, exit 101):
`phase1-repair/fail-before.json`. Five behavioral assertions:
independent canonical assembly, inner length defects, zero ceilings,
updated zero assertions in the ceiling matrix, oversized object; plus
exact final capacity/body length agreement, which is limited evidence
rather than an allocation-count measurement. Pre-fix exposures (passed
characterizations, defect demonstrated, then removed with the old API):
encoder accepted a substituted expected root and mutated work without
length error. The `verify_plan` early return is structural evidence at
base `e443850` `entity_read.rs:659`; no executable claim is made for it.
Syntax or compiler errors were never counted as reproduction; historic
failing logs are preserved unrelabeled.

Final A5 fail-before receipt (1 discriminating failure, exit 101):
`phase1-final-repair/fail-before.json`. The oversized non-Function
`Signature` case answered `BudgetExceeded` instead of
`ClassNotApplicable`; `Version` behavior was already correct.

Pass-after receipts (exit 0): `phase1-repair/pass-after-query-lib.json`
(88 passed, 4 ignored), `phase1-repair/pass-after-repo-lib.json`
(367 passed, 3 ignored), `phase1-repair/pass-after-clippy.json`
(`cargo clippy --locked -p sley-query -p sley-repo --all-targets --
-D warnings`, clean), plus `phase1-final-repair/` counterparts for this
correction. New coverage: nonmonotonic parameter declaration order,
parameter ObjectId/epoch preparation rejection over sorted bindings,
immutable request/view capture under consuming encode, byte-ceiling floor
with one-under refusal, over-ceiling refusal before copy, oversized
non-Function applicability precedence, exact final capacity/body length.

## Unresolved

None in this repair scope. Whole AT-MW-02 remains incomplete; protocol
integration, independent vectors, and bounded-context demonstrations are
subsequent phases. Stopping here for root integration review.
