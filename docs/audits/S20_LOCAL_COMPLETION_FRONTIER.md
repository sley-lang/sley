# Sley 2 Local Completion Frontier

Status: **S20-530 crash recovery is complete; S20-540 pack exchange is next; the Sley 2 goal remains incomplete**

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
| Semantics and queries | Full S20-240/S20-250 | Six entity bodies remain outside the semantic-core ownership and exact impact contract; complete-root extraction is absent. |
| Sessions and protocol | S20-330/S20-400 | Verified workspace/root/epoch and negotiated-session authority are absent. S20-400 still waits for the full root-backed S20-310 contract despite restricted mutation/transaction inputs. |
| Mutation and transactions | Full S20-360/S20-390 | Restricted fresh validation and fixed-head atomic commit pass for executable programs without semantic operation entities and with no selected tests. Full operation analysis, selected-test evidence, policy/epoch transitions, and runtime authority remain absent. |
| Repository | S20-540 | Canonical transactions, complete receipts, one fixed accepted head, native named refs, immutable branch origins, bounded ancestry, shared/exclusive GC coordination, and the exclusive-recovery boundary over the exact 100-row crash matrix now exist. S20-500 and S20-530 closeouts pass with final Nabu/Ariadne/Vulcan receipts; S20-530 ages under ADR-0024. Semantic comparison, merge, and clone equivalence remain absent. |
| Succession benchmark | Full S20-600/S20-610 and S20-620 | The verified legacy adapter and offline raw claim chain are mechanics only. Approved fixtures, containment, live adapters, artifact/oracle/accounting verification, protocol/CLI, and real trials are absent. |
| Adversarial | Full S20-700 | Candidate, candidate-result, and transaction/receipt production targets are attached and passing. The merge production boundary remains absent, so the eleventh required Section 18.5 surface cannot yet be fuzzed. |
| Supply chain and release | Full S20-710/S20-720 | Root license text approval, standards SBOM, provenance, release re-anchor, final review, all GA code, and a release artifact are absent. |

## S20-250 remains incomplete

The epoch-1 schema freezes fields for `Workspace`, `Package`, `Namespace`,
`EntryPoint`, `PolicyBinding`, and `DependencyBinding`, but the current
`sley-ssmc` semantic core models only kinds 4 through 15. Before full S20-250
can land, one normative model must own all eighteen bodies and freeze the exact
workspace membership, package dependency/export, namespace parentage,
entry-point exposure, policy-subject, and external-root relationships. A
second host model or locally invented edge kinds remain forbidden.

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
- S20-510 semantic comparison remains blocked by the six absent full S20-250
  entity bodies and complete-root impact semantics. S20-540 pack exchange
  (dependencies S20-170 and S20-500 complete) is the next authority-safe
  package.
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
- No `sley-protocol`, `sley-json-bridge`, or `sley-cli` crate and no comparison
  or merge production module exists.
- No real benchmark trial, release artifact, publication authority, provider
  spend, push, tag, upload, or deployment exists in this goal.

`python3 scripts/check_local_completion_frontier.py` fails if the recorded
boundary drifts. The next action is the smallest dependency-complete S20-540
pack-exchange slice: a frozen clone-equivalence profile over the S20-170 pack
and the S20-500 refs, without treating it as merge, comparison, or GA.
