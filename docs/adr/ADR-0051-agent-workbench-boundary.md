# ADR-0051: the agent workbench as a separate tool over the kernel libraries

Status: accepted for Sley 2.0.2 (2026-09-25). The operator activated
`SLEY-2.0.2-BR` ("implement the Sley 2.0.2 spec"). Its open decisions
(section 9) are taken at the spec's stated defaults, as recorded under
Decision 9. The contract is `docs/spec/SLEY_AGENT_V1.md`.

Date: 2026-09-25

## Context

Sley 2.0's agent tooling made correct programs slow and expensive to write,
and it left them untested. The causes were defects and interface overhead,
not the language:

- TestCase refusals that the tool never decoded;
- no readable view of a function;
- hand-assembled SSA JSON with 64-hex identities and a two-pass
  propose/compose;
- a tool that spawned a Python process per created entity;
- no way to run a function or a test.

Sley's constitution makes machine generation efficiency a goal (MPI-0,
tightening spec). It forbids a source syntax or parser, canonical text, and
textual review gates (`docs/ANTI_GOALS.md`), and it keeps the CLI a
transport endpoint with no semantics (ADR-0035).

## Decision

1. **A separate binary.** The workbench is `crates/sley-agent`, binary
   `sley-agent`. It sits at the tool layer of ADR-0003, beside the CLI,
   bridge and benchmarks. No kernel crate depends on it. `sley-cli` stays
   the transport endpoint of ADR-0035, and its rule audit is unchanged.
2. **No private validation rules.** Every verdict is the kernel's
   `validate_candidate_bytes` or `TransactionRepository::commit`. The
   workbench fills envelope fields mechanically from the accepted head and
   the policy grant. Its derived outputs (views, hints, locators, execution
   results) are advisory. They are never admission evidence, never signed,
   and never committed.
3. **AV1 is output only.** AV1 is visibly labeled derived debug notation
   (ADR-0001). No product component reads it. Authoring input is JSON only.
4. **AF1 is data, not source syntax.** An AF1 frame is a closed JSON schema
   of named definitions and operation lists over the closed opcode table,
   with type shorthand drawn from a fixed constructor table. It has no
   expression grammar and nothing is evaluated. It compiles client-side into
   the existing mutation candidate record, and the kernel never sees it.
   The raw operation path remains available and sufficient. This is the
   disposition of AF1 against the anti-goal row "Sley source syntax or
   parser". It is not an exception to that row: no parser crate, grammar or
   `.sley` file exists.
5. **Local names are metadata.** Names come from object labels, a
   workspace name map, or positions. They never carry identity and never
   enter canonical bytes. Candidates cannot set labels (CreateEntity builds
   unlabelled objects), so the workspace map records the names that AF1
   creates.
6. **Explain is a library side channel.** `CandidateValidationOutput`
   carries an optional `RefusalLocator` that the refusing phase fills. It is
   never encoded, and the candidate result record, its bytes and its digest
   are unchanged. No SMP1 method is added. The CANDIDATE_RESULT_V1 corpora
   and the `sley-policy` suite pin this. For phase 7 the workbench adds an
   advisory analysis of the one refused function, which runs only after the
   refusal.
7. **The dev loop executes through the VM owner.** `call` and `test` lower
   each function once per program state and run the lowered image with
   `execute_loaded_image`. TestCase outcomes are compared with the kernel's
   value-hash and trap-code rule. The protected native-test path (N3 to N5)
   is untouched.
8. **Local lifecycle.** `init` writes one trusted genesis into an empty
   directory, the local equivalent of `workspace.create`. `commit` goes
   through the transaction engine. Neither can change the judging roots of
   an existing repository.
9. **Spec decisions at their defaults.**
   - The explain route is the library explain mode (item 6).
   - AF1 is disposed as in item 4.
   - Workbench and benchmark ceilings are fuel 1,000,000, memory 16 MiB,
     output 64 KiB and 10,000 mutations (`SLEY_AGENT_V1.md` section 7).
   - Track B (a writable text projection) is not adopted.
   - Clarity is reported, not gated.
   - Publishing 2.0.2 is the operator's decision.
10. **The session lives in the workspace, not in a daemon.** BR-10 asks for
    one repository session per workspace kept alive across commands, in
    process or as a local socket daemon. The workbench keeps that session
    on disk instead: the repository, the name maps, the candidate handles
    and the submission live under the workspace. Each command reopens them
    in process, with no child process and no `uv`. The requirement exists
    for latency, and the budgets are met without a live process. The p95 of
    30 whole-process runs is 4.2 ms for `view` (budget 150 ms), 4.2 ms for
    `find` on about 100 entities (300 ms) and 12.0 ms for `try` of 49
    operations with 5 TestCases (500 ms). A daemon would add lifecycle,
    staleness and cleanup failure modes for no measurable gain. It stays a
    later option if a workload misses the budgets.

## Consequences

- Agents read `sley-agent view` and write `sley-agent try` frames. A
  typical exchange is one call per fix, and every refusal names its symbol,
  its locator and a hint.
- The 2.0.0 trial tool (`bench/live/sley2_tool.py`) is unchanged. Its
  S20-640 preregistration pins stay valid. Moving the mediated harness onto
  the workbench binary belongs to the next campaign's preregistration.
- Opcodes, entity kinds or refusal symbols added in later versions need rows
  in `crates/sley-agent/src/opcodes.rs` and `crates/sley-agent/data/hints.json`.
