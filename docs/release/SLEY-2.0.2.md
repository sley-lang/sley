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
  operand names the fix at its exact position. Problems in different
  functions arrive in the same refusal, whose first line carries the first
  problem's JSON pointer. Edge argument counts and types, and operands of
  the wrong type (a `Result` passed to arithmetic, say), are named at the
  exact terminator or operation rather than per function. After a
  control-flow refusal, `also:` lines list the other problems the analysis
  sees. Names may contain a hyphen after the first character.
- **Tests travel with changes.** A Valid candidate that runs no TestCase
  says `tests: 0 ran`, and `submit` refuses a candidate that changes a
  function no test in it targets, unless `--untested` is given.
- **`br` takes the bracketed target** (`["br", ["join", "x"]]`) that
  `cond` and `switch` accept, as the guide shows. CI now runs the guide's
  inline terminator examples as well as its fenced frames.

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

The release archive ships `bin/sley-agent` next to `bin/sley` (seventeen
manifest members, one more than 2.0.1), built by one locked `cargo build` and
compared byte for byte across two clean builds ([packaging revision 9](../spec/RELEASE_CANDIDATE_PACKAGING_V1.md#18-agent-workbench-binary-revision-9-2026-09-25)).
The source-free conformance subset runs the workbench from the unpacked
archive: its version, its guide bound, and the guide's first example through
`init` and `try`. `sley-agent` adds no third-party crate.

## Build and verify

The build needs a clean checkout at the release commit, the pinned Rust
toolchain (`rust-toolchain.toml`, 1.93.0) with the `x86_64-unknown-linux-musl`
target installed, Python 3 and `uv`.

```sh
make release-candidate-smoke        # fetch, two clean builds + all release records, then verify
make release-candidate-verify       # re-check the records against the tree
sha256sum dist/sley-2.0.2-linux-x86_64.tar.gz
python3 -c 'import json; r = json.load(open("evidence/release/reproducibility-report.json")); print(r["result"], r["commits"])'
```

The digest printed by `sha256sum` must equal the SHA-256 above and the
`artifact_sha256` that the reproducibility report records for the release
commit. To reproduce the build on a second host, follow
`docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md` section 5.1.
The [Quickstart](../QUICKSTART.md#6-run-the-test-gates) lists the test gates
that run from a fresh clone. From the unpacked archive, `bin/sley-agent init`
and `bin/sley-agent try` run the guide's first example with no source tree.

## Known limits

- A candidate that changes a function together with TestCases that target it
  can be validated, run and submitted, but not committed: that needs native
  test evidence (N5). You can commit code and tests in separate candidates.
- The dev loop cannot execute functions that declare type parameters,
  effects or contracts. That is the VM lowering profile's limit.
- The S20-640 trial tool (`bench/live/sley2_tool.py`) is unchanged, which keeps
  its preregistration pins valid. Moving the mediated campaign harness onto
  `sley-agent` belongs to the next campaign's preregistration.
