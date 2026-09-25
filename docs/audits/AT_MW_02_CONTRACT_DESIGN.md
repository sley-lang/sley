# AT-MW-02 contract design

Status: REVIEWED_DESIGN, 2026-09-08; owner: Codex integrator.
Ends when the reviewed contract and its implementation supersede this record.

## Implementation decision

Nabu, Ariadne and Vulcan independently returned `pass`, high confidence,
on corrective candidate `48070373d59eb2241ad7fed4157d9016b514500f`. Both
semantic findings and the budget-phase finding are resolved. The integrator
accepts this design for local implementation under the active operator
directive; Muse 1.3 through OpenCode Go owns the coding slice. Any required
semantic deviation returns to the integrator for design and review.

The open-question list below records the questions reviewed: declaration
signature completeness and the explicit query-owner/host-work distinction
were accepted; compatibility and BoundedContext mappings were checked;
the exact failure phase order was corrected. Required dependent contract
and consumer synchronization belongs to implementation acceptance.

Review provenance remains independent manual-role subagents applying the
canonical Council roles, with no ForgeNode runtime-registry qualification
claim. The runtime gate, independent vectors, edit demonstrations, and
AT-MW-02 finding remain open.

Baseline: `b5793a5e5beb4df70d0aa41d3005a8aaf6190dd2`.
The architecture audit found that current query responses omit entity bodies;
whole-revision checkout is excluded by the benchmark's bounded-context arm.
The result is an actual inability to construct most existing-body mutations.

The integrator selected two protocol-version-2 methods using exact stored
objects. GetSignature includes ordered Parameter objects because Function
objects contain only their identities; returning a Function alone cannot
satisfy the required parameter-type facts. Nominal definitions and inferred
closures remain separate query obligations. No new canonical serializer,
schema epoch or persistent object identity is proposed.

The first Muse design attempt used the wrong OpenCode Zen route and terminated
on insufficient balance. The corrected OpenCode Go Muse 1.3 contributor call
started successfully but its external authority-file read was auto-rejected
by noninteractive permissions. Neither attempt wrote these documents or
produced authoritative design evidence. Codex authored the design after the
operator clarified the split: integrator thinks/plans/designs, Muse codes
through OpenCode Go. Private run records are retained outside the repository.

## Requirements and implementation ownership

| Requirement | Existing seam | Planned verification |
|---|---|---|
| Exact entity bytes | `VerifiedRevision::objects`, state-root binding order | 18-kind oracle byte equality and ObjectId check |
| Complete declaration signature | `FunctionGraph`, `Parameter`, canonical mutation body decoder | ordered parameter/type/effect fixtures and signature edit |
| Root/session safety | `ProtocolServer::head_bound`, `session_check`, session authority | stale/root/workspace/epoch/renewal negatives |
| Version-safe addition | SMP1 section 4, framing and hello negotiation | preserved v1 vectors and mixed-version cases |
| Bounded query work | borrowed verified objects plus size preflight | exact limits, no whole-root extraction, allocation instrumentation |
| Independent protocol context | trial runner ARM_AFFORDANCES | actual stdio EC1a and signature edit, create, validate |
| Consumer consistency | bridge/CLI contracts and generated metadata | synchronized pins, bridge vectors, contract-index check |

Implementation should divide into a pure entity-read owner, a verified-repo
adapter, versioned protocol serving, independent vectors and consumer tests.
One Muse worker owns the interacting runtime files during the implementation
slice; independent review uses a pinned detached tree.

## Questions requiring review before freeze

1. Confirm whether declaration signatures plus separately root-bound named
   definition reads satisfy the inherited GetSignature requirement.
2. Confirm the explicit distinction between bounded owner work and inherited
   complete-revision verification/session-admission work. If a total request
   bound is required, add a separate verified bounded-reader dependency;
   do not claim the current complete-revision loader has that property.
3. Check mixed-version hello compatibility and the version-aware codec API
   against actual code before freeze; BoundedContext maps its eight existing
   fields explicitly and adds none.
4. Assess the conservative fixed work formula and the failure precedence
   when signature structural inconsistency and budget exhaustion coexist.

A bounded compatibility inspection found that current Hello validation
accepts unknown numeric method tags and negotiation preserves their
intersection. The initial draft would have tightened that behavior for
306/307. The corrected draft explicitly preserves legacy helpers and their
opaque intersections, while separately version-aware negotiation filters
known v2 tags under a v1 selection. Tests must include those previously
unknown numbers; ordinary method-vector checks alone would miss the change.

## First independent review

Candidate `7810f359dcc33a297350ea165e3ec58bedd60c70` received independent
manual-role reviews using the canonical Nabu, Ariadne and Vulcan role
instructions. Normal ForgeNode dispatch was queued behind a foreign live
review and was canceled before a worker started; role lookup independently
reported profile topology drift. These reviews do not certify the installed
ForgeNode runtime registry. Complete private JSON receipts retain provenance.

Nabu accepted the architecture: the inherited requirement bounds model
context, not all host verification work. Ariadne confirmed declaration
signature completeness. Ariadne and Vulcan required an explicit ordered
reservation/debit table; Ariadne also required synchronization of the closed
session method classification. The revised draft contains those corrections
and adds budget-observation regression requirements. Corrective review is
pending; no contract freeze follows solely from this first review.

Baseline Tier 2 `cargo test -p sley-protocol` passed 44 tests, with two
intentional fixture emitters ignored, zero failures and no warnings. The
private revision receipt binds that pre-design baseline only. No runtime
implementation, contract freeze, AT-MW-02 closure or release pass is claimed.
