# Sley 2.0.2 release notes

**Release:** Sley 2.0.2 (version `2.0.2`), a point release of 2.0.1.
**Artifact:** `sley-2.0.2-linux-x86_64.tar.gz`. Its identity (commit,
SHA-256, byte size, manifest digest) is fixed by the release build and
recorded in `evidence/release/reproducibility-report.json` and the machine
summary's `release_candidate_packaging` section, not in this document. The
[v2.0.2 GitHub release](https://github.com/sley-lang/sley/releases/tag/v2.0.2)
lists the SHA-256 and ships a `SHA256SUMS` file.
**Not a GA claim.** `ga_claimed` stays `false`, and every limit in the
[2.0.0 known limits](SLEY-2.0.0.md#known-limits-and-what-is-not-yet-claimed)
still applies.

2.0.2 is about the agents that write Sley programs. Sley 2.0's agent tooling
made correct programs slow and expensive to write, and it left them
untested. The causes were defects and interface overhead, not the language:

- a TestCase refusal nobody could read;
- no readable view of a function;
- hand-assembled graph JSON with 64-hex identities;
- a tool that started a process per created entity;
- no way to run a function or a test.

2.0.2 fixes all five. It does not change SSMC1, SCB1, entity identity,
validation semantics, candidate-result bytes, the protocol or the method
table. A 2.0.1 client talks to a 2.0.2 endpoint the same way.

## The agent workbench: `sley-agent`

A new binary, built on the kernel libraries, runs entirely in process: no
Python, no `uv`, no server. Its contract is
[SLEY_AGENT_V1](../spec/SLEY_AGENT_V1.md), with decisions in
[ADR-0051](../adr/ADR-0051-agent-workbench-boundary.md). The agent guide is
`sley-agent help`, under 8 KiB.

- **`view`** renders functions as compact AV1 listings under local names:

  ```text
  fn percent(part: i64, whole: i64) -> Result<i64,MathError>   [c539786e]
    entry:
      zero = const k_0 (0)
      is_zero = eq whole, zero
      cond is_zero -> zero_whole, scale
  ```

  AV1 is output only. It is labeled non-canonical, and nothing reads it
  back.
- **`try`** compiles an AF1 frame, validates the candidate, and runs its tests
  in one call. An AF1 frame is JSON data that names functions, blocks,
  operations, types and tests by local name. The tool derives identities,
  ordinals, owner lists and result types, reuses entities matched by name,
  and deletes the ones a redefinition drops. The raw operation list still
  works, and it gains `@key` references, so no placeholder pass is needed.
- **Refusals say what and where.** A refusal prints the decision, the phase,
  the kernel's symbol, a remediation hint, and a locator:

  ```text
  c3: REFUSED ResourceLimit at phase 12 (resource analysis)
    symbol: CANDIDATE_TEST_RESOURCE_LIMIT
    where: TestCase t3 .resource_limits.memory_bytes 1000000 > grant ceiling 1000
  ```

  The locator comes from a new explain-only side channel on
  `CandidateValidationOutput` (`refusal_locator`). It is never encoded: the
  result record and its digest are unchanged.
- **`call` and `test`** execute functions and TestCases in process. Each
  function is lowered once and run on the lowered image. TestCases are compared by the kernel's own value-hash
  rule. Results are advisory and never committed.
- **`submit`** writes `final_candidate.hex`, and it can be repeated: the last
  Valid submission wins. `status` shows the submission and its tests.
- **`init`, `commit` and `export`** manage local workspaces. `init` grants
  realistic test and candidate ceilings: fuel 1,000,000, memory 16 MiB,
  output 64 KiB, and 10,000 mutations per candidate.
- **One round per mistake, not per use.** A frame that cannot compile lists
  every operation (then every terminator) that cannot resolve, one JSON
  pointer per line. A name that lives in another block is reported as that
  block's parameter (pass it on as an edge argument) or result (qualify it
  as `block.name`). A switch case key that is not a case of a Result or an
  Option names the expected keys. A literal or nested operation used as an
  operand names the fix at its exact position. A Valid candidate that runs
  no TestCase says `tests: 0 ran`. Names may contain a hyphen after the
  first character.

Response times are well inside the spec budgets (p95 of 30 whole-process
runs on the build host):

| Operation | p95 | Budget |
|---|---|---|
| `view` of one function | 4.2 ms | 150 ms |
| `find` on a package of about 100 entities | 4.2 ms | 300 ms |
| `try` of 49 operations with 5 TestCases | 12.0 ms | 500 ms |

## TestCases that work

AF1 tests that state no limits take the workbench defaults, clamped to the
policy grant. A too-large limit is refused with the limit, the value and the
ceiling named. The test suite executes every guide example, every JSON
example line of the `af1` and `tests` help topics, and every value form the
`types` topic documents.

## Packaging

The release archive ships `bin/sley-agent` next to `bin/sley`: fifteen
members, built by one locked `cargo build` and compared byte for byte across
two clean builds ([packaging revision 9](../spec/RELEASE_CANDIDATE_PACKAGING_V1.md#18-agent-workbench-binary-revision-9-2026-09-25)).
The source-free conformance subset runs the workbench from the unpacked
archive: its version, its guide bound, and the guide's first example through
`init` and `try`. `sley-agent` adds no third-party crate.

## Known limits

- A candidate that changes a function together with TestCases that target it
  can be validated, run and submitted, but not committed: that needs native
  test evidence (N5). You can commit code and tests in separate candidates.
- The dev loop cannot execute functions that declare type parameters,
  effects or contracts. That is the VM lowering profile's limit.
- The S20-640 trial tool (`bench/live/sley2_tool.py`) is unchanged, which keeps
  its preregistration pins valid. Moving the mediated campaign harness onto
  `sley-agent` belongs to the next campaign's preregistration.
