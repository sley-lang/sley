# Sley 2 Local Completion Frontier

Status: **S20-540 pack exchange is complete; full S20-250, S20-510 semantic comparison, S20-520 merge, the full S20-300 complete-root snapshot, the full S20-310 root-backed queries, and the full S20-320 context capsule are implemented with Council reviews pending; the S20-400 SMP1 contract is drafted and S20-410 is implemented (frame, negotiation, identity scoping, and the deterministic server over thirty-two methods) with reviews pending; S20-440 cancellation, streaming, and budgets and S20-330 negotiated sessions are implemented with reviews pending; S20-420 JSON bridge is implemented with reviews pending; S20-430 CLI is next; the Sley 2 goal remains incomplete**

This audit records the first unavailable boundary in each unfinished lane and
the one dependency-complete package that may proceed. It does not convert
restricted profiles into GA, substitute documentation for implementation, or
authorize runtime deployment, benchmarks, release, or publication.

ADR-0019 corrected the unreleased epoch-1 generic `Option<T>` declaration to
the canonical SCB1 tags (`0=None`, `1=Some<T>`). No production schema epoch,
accepted root, release artifact, or compatibility promise existed, so the
manifest and all derived evidence could be re-anchored together. The
`ConstValue` record and its closed sixteen-variant type/data contract were
already frozen by S20-345. The native implementation, independent Python
corpus, exact Rust consumer, and production candidate libFuzzer target close
proposal-only candidate construction. The subsequent restricted S20-360 slice
now owns all fourteen ordered judgments and canonical results for the explicit
executable-program-operation-free success subset. S20-390 consumes only a
fresh validator-owned plan and closes one fixed accepted-head transaction
boundary without creating named refs or runtime authority.

| Lane | Current boundary | Status |
|---|---|---|
| Semantics and queries | Full S20-240/S20-250 | The six entity bodies, the eighteen-kind impact request, and the complete-root closure judgment are implemented under the draft full profile (`docs/audits/S20_250_FULL_ENTITY_BODIES_CLOSEOUT.md`); the contract is not frozen and the package is not complete until the Ariadne, Nabu, and Vulcan reviews pass. Full S20-240 remains restricted. |
| Sessions and protocol | S20-330/S20-400 | Negotiated-session authority binding workspace, verified root, and epoch is implemented (`docs/audits/S20_330_NEGOTIATED_SESSION_CLOSEOUT.md`) and the SMP1 contract, server, cancellation, and streaming are implemented, all under draft contracts with Council reviews pending; the JSON bridge is implemented (`docs/audits/S20_420_JSON_BRIDGE_CLOSEOUT.md`) with reviews pending; the CLI is absent. |
| Mutation and transactions | Full S20-360/S20-390 | Restricted fresh validation and fixed-head atomic commit pass for executable programs without semantic operation entities and with no selected tests. Full operation analysis, selected-test evidence, policy/epoch transitions, and runtime authority remain absent. |
| Repository | Reviews of full S20-250, S20-510, and S20-520 | Canonical transactions, complete receipts, one fixed accepted head, native named refs, immutable branch origins, bounded ancestry, shared/exclusive GC coordination, the exclusive-recovery boundary over the exact 100-row crash matrix, and clone-equivalent repository exchange exist with reviewed closeouts. S20-510 semantic comparison and S20-520 merge are implemented under draft contracts (`docs/audits/S20_510_SEMANTIC_COMPARISON_CLOSEOUT.md`, `docs/audits/S20_520_MERGE_CLOSEOUT.md`) with Council reviews pending; M4 exit waits on those reviews. |
| Succession benchmark | Full S20-600/S20-610 and S20-620 | The verified legacy adapter and offline raw claim chain are mechanics only. Approved fixtures, containment, live adapters, artifact/oracle/accounting verification, protocol/CLI, and real trials are absent. |
| Adversarial | Full S20-700 | Every Section 18.5 required surface has a landed persistent target, the merge engine target is attached, and all twenty scoped targets pass their smoke gates. The complete finding register, the independent review, and the slice receipts for S20-250, S20-510, and S20-520 remain deferred with the Council lanes. |
| Supply chain and release | Full S20-710/S20-720 | Root license text approval, standards SBOM, provenance, release re-anchor, final review, all GA code, and a release artifact are absent. |

## S20-250 remains incomplete

The epoch-1 schema freezes fields for `Workspace`, `Package`, `Namespace`,
`EntryPoint`, `PolicyBinding`, and `DependencyBinding`. As of 2026-09-03 one
normative model in `sley-ssmc` owns all eighteen bodies, and the draft
`docs/spec/COMPLETE_ENTITY_IMPACT_PROFILE_V1.md` (ADR-0026) freezes the exact
workspace membership, package dependency/export, namespace parentage,
entry-point exposure, policy-subject, and external-root relationships with the
twelve existing edge kinds; the implementation, fixture, oracle, and fuzz slice
landed at `e78a1ab`. A second host model or locally invented edge kinds remain
forbidden. The package stays incomplete until the contract is frozen by the
Ariadne, Nabu, and Vulcan reviews, which were unavailable when it landed.

## Active package and terminal facts

- Restricted S20-360 candidate validation is complete. Its closed context,
  executable-program-operation-free success subset, native owner checkers, protected
  S20-370/S20-380 judgments, all-sixteen-decision independent corpus, and
  result-import fuzz target grant no authority by themselves.
- Restricted S20-390 atomic commit is complete. Fresh revalidation, non-cyclic
  transaction and receipt identities, durable object/receipt-before-head
  ordering, stale-safe fixed-head CAS, manifest-length verification, trusted
  genesis, and bounded recovery pass native, independent, fault, and fuzz
  evidence. It grants no named-ref, policy-transition, or runtime authority.
- S20-500 native refs and branches are complete locally on the one-way
  `sley-repo -> sley-txn` dependency. Exact ref names, immutable origins,
  parent/root bindings, bounded ancestry, direct-parent CAS, retry durability,
  parent-resynced layout/fan-out creation, and GC maintenance ownership are
  covered. Tier 1/Tier 2 validation and final Nabu/Ariadne/Vulcan review pass;
  the exact record is `docs/audits/S20_500_NATIVE_REFS_BRANCHES_CLOSEOUT.md`.
- S20-530 crash injection and recovery is complete under the frozen v13
  contract: exact 100-row matrix, 419 mapped tests in one captured Tier 2
  closeout at `8f7c763`, three freeze and three implementation receipts, and
  the full checker PASS at `cc0f92f`; the exact record is
  `docs/audits/S20_530_CRASH_RECOVERY_CLOSEOUT.md`. The accepted state is
  verified at its commits (ADR-0024): `make quick` runs the read-only
  acceptance anchor and `make s20-530-verify` reruns the frozen checker in an
  isolated clone.
- S20-540 pack exchange is complete under the frozen Repository Exchange v1
  contract (ADR-0025): composed exchange of the exact S20-170 pack, the
  receipts of the exported ancestry, the accepted head, and visible branch
  pairs; clone-equivalent import with the stage-marker write guard,
  interruption rows X-01 to X-07, a frozen fixture with an independent Python
  oracle, and a persistent fuzz slice; the exact record is
  `docs/audits/S20_540_REPOSITORY_EXCHANGE_CLOSEOUT.md`.
- The full S20-250 entity bodies are implemented (`e78a1ab`).
  S20-510 semantic comparison is implemented (`6ecfe89`).
  S20-520 merge is implemented;
  the full S20-300 complete-root snapshot is implemented
  (`docs/audits/S20_300_FULL_COMPLETE_ROOT_SNAPSHOT_CLOSEOUT.md`);
  the full S20-310 root-backed queries are implemented
  (`docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md`);
  the full S20-320 context capsule is implemented
  (`docs/audits/S20_320_FULL_CONTEXT_CAPSULE_CLOSEOUT.md`). Each sits
  under a draft contract with Council reviews pending; the merge engine
  target is attached as the eleventh Section 18.5 surface with the arm-2
  snapshot decoder, root-query engine, and capsule builder targets beside
  it; the S20-400 SMP1 contract is drafted (`docs/spec/SMP1.md` revision 3,
  ADR-0032) and S20-410 is implemented
  (`docs/audits/S20_410_SMP1_FRAME_CLOSEOUT.md`) and S20-440 is implemented
  (`docs/audits/S20_440_SMP1_CANCEL_STREAM_CLOSEOUT.md`);
  S20-330 is implemented
  (`docs/audits/S20_330_NEGOTIATED_SESSION_CLOSEOUT.md`); S20-420 is implemented
  (`docs/audits/S20_420_JSON_BRIDGE_CLOSEOUT.md`), so the S20-430 CLI is the
  next dependency-complete package.
- The epoch re-anchor has architecture and semantic review. Vulcan's first
  re-anchor pass found stale machine-summary query/capsule vectors; after those
  and later adapter/report identities were corrected, the focused re-review
  passed with no report-grade findings.
- S20-350 candidate construction is proposal-only. Native tests build and
  import candidate bytes, but no candidate has been executed, semantically
  validated, applied, committed, or treated as authority.
- The focused Codex semantic/security review and bounded S20-360/S20-390
  specialist reviews are recorded in their closeouts. Earlier packages with
  deferred Forge review retain their original evidence status.
- `make v2` and `make release-check` remain fail-closed `NOT_IMPLEMENTED`
  gates and are not success evidence.
- No `sley-cli` crate exists; the protocol, JSON bridge, comparison, and
  merge production modules exist under draft contracts.
- No real benchmark trial, release artifact, publication authority, provider
  spend, push, tag, upload, or deployment exists in this goal.

`python3 scripts/check_local_completion_frontier.py` fails if the recorded
boundary drifts. The next action is S20-430: the CLI wrapper over the
protocol and the JSON bridge, reporting only what the protocol judges and
duplicating no semantics, while the pending S20-250, S20-510, S20-520,
S20-300, S20-310, S20-320, S20-400, S20-330, and S20-420 reviews land as
revisions; nothing in it is treated as GA.
