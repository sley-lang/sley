# Initial workload and adoption roadmap

This roadmap addresses the adoption priorities in
[issue #21](https://github.com/sley-lang/sley/issues/21), especially findings
1, 4, 5 and 13. It separates the initial evaluation target, evidence needed
for a decision, and the work that supplies that evidence. It does not change
the language contracts or the accepted self-hosting requirements.

## Initial evaluation target

Start with an agent authoring a **short, deterministic function within the
documented Python-dialect subset**, with a specified input/output contract
and independently checked behavior. The dialect is a noncanonical input to
the ordinary typed candidate path; the accepted program remains a Sley
graph. See [the frontend contract](spec/SLEY_AGENT_V1.md) and
[ADR-0057](adr/ADR-0057-python-dialect-and-authoring-parity.md).

This target matches the scope of the
[2.1.1 authoring-cost report](release/SLEY-2.1.1.md#authoring-cost). That
report measures function creation, with feedback and retries. It does not
establish editing-workload savings, general application economics or
independent adoption. Host I/O, orchestration and packaging supplied by
another language must be identified and counted when evaluating an
application that uses a Sley function.

The next evaluation extends that target to a small change in an existing
program: retrieve the relevant state, modify the function and its targeting
tests, diagnose a failure, and review the resulting state. Measure this as
a separate workload. A successful function-generation result cannot stand
in for an editing result.

For the complete workflow, success also requires authoritative admission
of the combined code-and-test change. Released 2.1.4 still refuses an ordinary
commit that selects TestCases. The native supervisor is integrated in source
but absent from the release archive and not qualified for installed use; see
[the 2.1.4 compatibility boundary](release/SLEY-2.1.4.md#compatibility).
Advisory test matches and submission do not complete native admission.

## Evidence required for an adoption decision

Before inspecting a new campaign's final results, record the exact release
artifacts, model versions, tasks, tool access, guidance, budgets, stopping
rules and independent correctness criteria. Keep development, held-out and
editing conditions distinct. Preserve the existing frozen succession
campaign and its protocol; a new comparison receives its own identity.

| Question | Evidence needed |
| --- | --- |
| Does the agent complete the intended task? | Independent behavior checks, first-attempt and eventual success rates, all failed tasks and repair rounds, plus the accepted state/receipt when the task requires commit. Compilation or an advisory match alone is insufficient for a tested-commit task. |
| What does a correct completed task cost? | Auditable usage for setup, supplied documentation, input/output, tool feedback, retries and failures; explicit denominators, cache treatment and provider-exposed reasoning usage or identified accounting gaps. Report expenditure and success together. |
| Is the comparison fair? | Competent mainstream tools with structured diagnostics, focused retrieval and execution feedback; matched budgets and task semantics. Use a matched tooling comparison or an ablation to separate representation benefits from harness and guidance benefits. |
| Can another evaluator reproduce it? | Exact artifact hashes, task/oracle and harness identities, model/accounting details, replay instructions and independently checked results. Arithmetic reproduction and independent model/runtime replication are separate evidence. |
| Does the complete change workflow work? | Compatible released core and tools, candidate-bound native evidence, combined code-and-test admission, stale/mismatch refusal, supported resource enforcement and recovery evidence on the intended hosts. Preserve the admission checks. |
| Is the workflow usable beyond its maintainer? | A useful application with explicit host scaffolding, a capability matrix, and a fresh-environment contribution that includes diagnosis, review, conflict handling and maintenance. Count the setup and review work. |

Declare the acceptable quality floor and any cost-parity margin before the
campaign. Report uncertainty and per-workload outcomes rather than selecting
only a favorable aggregate. A gate that tolerates overhead is a parity or
tolerance result, not a savings result. An efficiency claim requires the
measured comparison to support it without hiding failures or quality loss.
If a result does not support that claim, retain the result, fix the observed
limitation and evaluate a separately identified revision.

The decision is scoped to the demonstrated workload and supported platform.
Broader adoption needs its own evidence; passing a benchmark does not make
Sley generally available or establish correctness against unstated intent.

## Near-term order

The order below prioritizes closing the complete-function workflow and its
adoption evidence. Items with independent prerequisites can proceed in
parallel, but their results do not waive another item's acceptance criteria.

1. **Make the economic result reproducible (findings 1–3).** Complete the
   exact-artifact, correctness and accounting evidence for the initial
   workload; evaluate competent matched tooling and held-out conditions.
   Preserve the current reports and distinguish any new campaign from them.
2. **Finish authoritative tested changes (finding 4).** Qualify the native
   integration and recovery paths, then compatible core/tools packages.
   Keep the released restriction visible until the supported path passes.
3. **Demonstrate useful application scope (finding 5).** Maintain an
   executable capability matrix and a small application with tests,
   packaging and an explicit division between Sley and host scaffolding.
   Revalidate it for the release being recommended.
4. **Measure maintenance and outside use (findings 3, 8–12).** Run the
   separate editing comparison and fresh-environment review workflow.
   Include draft recovery, resource/guarantee labels, phase-separated
   scaling and reproducible contributor gates in the acceptance evidence.
5. **Expand from demonstrated demand (finding 13).** Choose broader
   workloads or platform obligations from observed user needs and the
   preceding results. State which user task improves, how it will be
   measured and what compatibility/maintenance work it adds.

There is no calendar or release promise attached to this order.

## Current disposition of audit #21

Updated October 22, 2026 against integrated main `39d5e62` and the published
[2.1.4 release notes](release/SLEY-2.1.4.md). The fixes, documentation and
supervisor source below entered the preceding
[2.1.2 release](release/SLEY-2.1.2.md). Releases 2.1.3 and 2.1.4 add VM
performance changes; 2.1.4 also adds inline Python-dialect tests, record
classes and shorter agent feedback. Subsequent work merged in main is not
yet another published release: [2.1.5, PR #58](https://github.com/sley-lang/sley/pull/58)
remains a draft at this observation.
The 2.1.1 authoring-cost and October 15 scaling records retain their original
artifact identities. They are not new measurements of 2.1.4. The separate
2.1.4 runtime measurements and scripted tokenizer counts have their own
scope; tokenizer counts are not model spend or an editing campaign.

| Finding | Disposition and evidence | Remaining work |
| --- | --- | --- |
| 1. Completed-task economics | Accepted, partially addressed. [PR #37](https://github.com/sley-lang/sley/pull/37) merged the [offline arithmetic package](../bench/authoring-cost/2.1.1/README.md); the issue also records the separately scoped 2.1.1 retained-answer replay. | Exact-artifact end-to-end accounting and independent model replication remain distinct from arithmetic reproduction or replay. No new campaign was run for this update. |
| 2. Fair tooling and held-out comparison | Accepted, open under the evaluation criteria above. Existing reports keep their declared tasks, harnesses and accounting. | A preregistered matched-tooling comparison or ablation, held-out tasks and complete success/cost accounting for the evaluated artifact. |
| 3. Actual authoring interface | Partially addressed by the Python-dialect [frontend contract](spec/SLEY_AGENT_V1.md), scoped 2.1.1 report, and the [inline tests and feedback released in 2.1.4](release/SLEY-2.1.4.md#python-dialect-and-try-output-50-51). The original AF1-only premise is historical. | Separate editing-workload validity, repair and read/write overhead measurements. Scripted token counts do not establish live-model savings. |
| 4. Authoritative code-and-test commit | Source integration merged in [PR #23](https://github.com/sley-lang/sley/pull/23), with diagnostics/refusal coverage in [PR #42](https://github.com/sley-lang/sley/pull/42). | [Issue #24](https://github.com/sley-lang/sley/issues/24): installed host, recovery/isolation and compatible core/tools/package qualification. The 2.1.4 archive omits the supervisor; selected-test commit still refuses. |
| 5. Capability matrix and application | Accepted, open for the current release. The old [PR #31](https://github.com/sley-lang/sley/pull/31) was closed with its 2.0.6 evidence preserved. | Current-artifact capability/application demonstration, with host scaffolding, tests, packaging and maintenance explicitly identified. |
| 6. Verification guarantees | Addressed in [PR #29](https://github.com/sley-lang/sley/pull/29): [guarantee matrix](GUARANTEES.md) distinguishes validity, policy, advisory matches, admission and identity. | Keep labels scoped; this documentation does not prove application correctness. |
| 7. Advisory resource declarations | Addressed by the explicit reporting alternative in [PR #29](https://github.com/sley-lang/sley/pull/29). [2.1.2 notes](release/SLEY-2.1.2.md#test-reporting) list applied fuel, five unapplied declarations and fixed VM caps. | Native enforcement requires its own host evidence; an advisory match remains insufficient. |
| 8. Draft allocation and interruption | The [workspace contract](spec/SLEY_AGENT_V1.md#1-workspace) describes reserved complete candidate bytes and optional metadata. [PR #28](https://github.com/sley-lang/sley/pull/28) adds exhaustion and metadata-publication regression coverage. | Preserve the documented recovery boundary; this is not atomic two-file publication or power-loss durability. |
| 9. Phase-separated scaling | Scoped evidence merged in [PR #36](https://github.com/sley-lang/sley/pull/36): [method and results](bench/SCALING-PHASES.md), retained samples and phase timings. | Measurements are synthetic and revision-bound; 2.1.4 runtime benchmarks do not establish size-independent workbench latency. |
| 10. Outside review/maintenance workflow | Accepted, open for the current release. The old [PR #30](https://github.com/sley-lang/sley/pull/30) was closed with its 2.0.6 example retained. | A fresh-environment current-artifact change/review/diagnosis/conflict demonstration; distinguish maintainer execution from independent adoption. |
| 11. Public entry points | Source checks and the first authoring path merged in [PR #33](https://github.com/sley-lang/sley/pull/33). [PUBLIC_ENTRYPOINTS](PUBLIC_ENTRYPOINTS.md) defines offline and live checks. | Source and seven live site surfaces passed the 2.1.4 consistency check on October 22. Repeat per release; this verifies links/versions, not binary behavior or benchmark claims. |
| 12. Contributor gate | Addressed in [PR #27](https://github.com/sley-lang/sley/pull/27): `make contributor-check`, documented in [CONTRIBUTING](../CONTRIBUTING.md). | Keep maintainer governance inputs and release waivers distinct. The [two master-goal check waivers in 2.1.4](release/SLEY-2.1.4.md#waived-checks) are not test passes, and the notes explicitly record that the operator has not reconfirmed their carry-forward. |
| 13. Initial target and priorities | Addressed by [PR #38](https://github.com/sley-lang/sley/pull/38) and this roadmap. | Execute the ordered work above; scope and costs remain explicit. |

The umbrella remains open for its missing workflow and adoption evidence.
Previously completed native fault cases remain evidence for their exact
unreleased revisions; they must not be relabeled as 2.1.4 host qualification.
Nothing here grants authority to modify a supervisor service, access another
host, change trust material or start a model campaign.

## Self-hosting and broader platform work

Self-hosting remains an accepted Sley requirement under
[ADR-0056](adr/ADR-0056-sh2-self-hosted-toolchain.md). Its user-facing
objective is a reproducible semantic toolchain that can rebuild and evolve
its own canonical program without a compiler in the pinned execution host.
The [self-hosting record](release/SH2_SELF_HOSTING.md) supplies evidence for
that objective, not for lower authoring cost or independent adoption.

Maintaining the bootstrap proof, host ABI, package compatibility and recovery
reference consumes engineering and test capacity that could otherwise
close the workflow and adoption gaps above. Parallel self-hosting or
platform work should name the user task or accepted requirement it serves,
its bounded engineering/compute allocation, the adoption work competing for
that capacity, and the evidence that will justify its next step. Report
actual resource use where measured; an unrecorded cost is unknown, not zero.

Preserve the existing accepted self-hosting artifacts and mandatory
compatibility checks. This roadmap neither rolls them back nor treats them
as a substitute for the economics, native-admission and application evidence.
