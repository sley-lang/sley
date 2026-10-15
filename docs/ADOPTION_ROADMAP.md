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
of the combined code-and-test change. Released 2.1.1 can validate the
authoring example, report advisory test matches and submit its candidate;
those results do not complete native admission. A combined tested commit
remains a separate qualification requirement. See
[the workbench contract](spec/SLEY_AGENT_V1.md) and
[native integration PR #23](https://github.com/sley-lang/sley/pull/23).

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

## Current evidence and work in review

As of October 15, 2026, the 2.1.1 release report is maintainer evidence for
the short-function workload. The following changes are in review; they
are not described as merged features or new releases here:

| Work | Contribution and remaining boundary |
| --- | --- |
| [Arithmetic reproduction, PR #37](https://github.com/sley-lang/sley/pull/37) | Replays the published arithmetic from normalized trial records. It does not reproduce model responses or establish oracle correctness. Its README identifies the retained campaign executables separately from the published 2.1.1 binaries; exact-release runtime replication remains open. |
| [Native admission, PR #23](https://github.com/sley-lang/sley/pull/23) and [host qualification, issue #24](https://github.com/sley-lang/sley/issues/24) | Development integration and host qualification remain separate from a compatible supported release. A normal-path result does not qualify fault enforcement or recovery. |
| [Capability/application example, PR #31](https://github.com/sley-lang/sley/pull/31) | Documents executable capability and a bounded application with host scaffolding. Preserve its stated artifact/version identity; revalidate before recommending another release. |
| [Guarantees/resources, PR #29](https://github.com/sley-lang/sley/pull/29) and [draft recovery, PR #28](https://github.com/sley-lang/sley/pull/28) | Clarify supported behavior and add targeted reliability evidence without broadening authority or durability claims. |
| [Contributor review, PR #30](https://github.com/sley-lang/sley/pull/30) and [public gate, PR #27](https://github.com/sley-lang/sley/pull/27) | Make review and verification reproducible from a fresh environment. A maintainer-run example is not evidence of independent human adoption. |
| [Phase scaling, PR #36](https://github.com/sley-lang/sley/pull/36) | Separates startup, loading, projection, lowering, repeated execution and validation on a synthetic workload. It does not establish size-independent general latency. |
| [Public entry points, PR #33](https://github.com/sley-lang/sley/pull/33) | Aligns the first authoring example and adds source/site version-link checks. Link consistency is separate from artifact and runtime qualification. |

Update this table with merged/released evidence when those transitions
actually occur. Keep the historical reports and their limitations intact.

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
