# Sley 2.1.6 release notes

Sley 2.1.6 is a point release for the familiar frontend. Deeply nested source now gets a clear
diagnostic with its line and column, nested built-in calls parse in linear time, and a new embed
API lets Rust hosts compile and run Python-dialect programs in memory, natively or in
WebAssembly. Fuel, instruction, value-unit and output-unit accounting is unchanged. The kernel,
SSMC1, SCB1, SMP1 rules, the instruction set, admission and commit semantics are untouched.

## Nesting diagnostics in the familiar frontend (#60)

- **A clear diagnostic for deep nesting.** The familiar frontend bounds nesting at
  `familiar::MAX_NESTING` (124) in both dialects. It counts every recursive entry the text
  writes: parentheses, calls, subscripts, unary `-`, `not`, `**` exponents, blocks,
  `else if` / `elif` arms, nested types such as `Vec<Vec<T>>` or `list[list[T]]`, and nested
  test values. Text past the bound is refused with `AGENT_FRAME_INVALID` at its exact line and
  column ("this nests more than 124 levels deep"), with a hint to name the inner part with a
  variable or a helper function. The check runs before any recursive pass, so every later pass
  works on input of known depth.
- **Every accepted program reloads.** 124 is the deepest body a JSON frame can carry: frames,
  including a draft's `frame.json`, are read with a 127-level JSON nesting limit, and a body lies
  three levels inside the frame. So every program the frontend accepts also loads again as a
  draft on the next step.
- **Linear-time nested built-in calls.** A parenthesized first argument to `sum(`, `max(` and
  the other built-ins is parsed once per level, so nested built-in calls parse in linear time.
- **Room for real programs.** Typical programs nest fewer than 25 levels. Every familiar program
  in the examples, the Arena bots and the benchmarks compiles byte-identically to 2.1.5; the
  deepest of them nests 20 levels and the longest `elif` chain has 5 arms.
- **Documented.** [SLEY_AGENT_V1](../spec/SLEY_AGENT_V1.md) section 5.5 and the
  [embedding guide](../EMBEDDING.md) state the limit, and the declared-limits report records
  it.

`tests/deep_nesting.rs` drives 14 hostile shapes through `compile_python` and the parser, and
boundary tests check that every maximum case compiles and reloads.

## The embed API (#59)

The new module `sley_agent::embed` lets another Rust program compile and run Sley:

- `compile_python` turns Python-dialect source into a module in memory, along the same path as
  `sley-agent try --familiar` over a fresh workspace.
- `Module::call` runs an exported function with an exact fuel limit and returns the value, a
  trap, or the limit that stopped it, with the fuel and instructions used.
- The new default `native` feature carries workspaces, repositories, drafts, the CLI and the
  `sley-agent` binary. Without it, the library is the pure core plus `embed`, and it builds for
  `wasm32-unknown-unknown` without host imports, with results identical to the native build.

[`docs/EMBEDDING.md`](../EMBEDDING.md) documents the API. Sley Arena
([sleylang.org/arena](https://sleylang.org/arena)) uses it for its match runner and player kit.

## Documentation (#48)

The [adoption roadmap](../ADOPTION_ROADMAP.md) records a disposition for each of the 13 findings
of the adoption audit (#21) against the published 2.1.4 release.

## Compatibility

Programs written for 2.1.5 keep their meaning and their answers, and their fuel and instruction
counts are unchanged. Familiar text nested deeper than 124 levels is refused with
`AGENT_FRAME_INVALID` at its position. The default features of `sley-agent` are now
`familiar` and `native`, so default builds are unchanged; a build of the CLI without the
familiar frontend is now `--no-default-features --features native`. Schema epoch, SSMC1, SCB1,
SMP1, validation semantics, the instruction set and the protocol method table are unchanged. No
third-party dependency was added to the binaries. `ga_claimed` remains `false`.

The native test supervisor is still source only, as in 2.1.2. It is not in the release archive and
not qualified on an installed host, so committing a candidate that selects TestCases still
refuses with `TXN_TEST_EVIDENCE_UNSUPPORTED`.

## Design-specification checks

For Sley 2.1.6, `scripts/check_candidate_result_contract.py` and
`scripts/check_transaction_contract.py` ran on the maintainer host with the
Sley 2.0 design specification available, and both passed. The waiver carried forward from the operator
decision of 2026-10-19 (Magister) covered 2.1.2 through 2.1.5 only and is not needed for this
release. The machine summary's `release_decision` records this.

## Candidate packaging

The archive is `sley-2.1.6-linux-x86_64.tar.gz`. It contains static musl Linux x86_64 `sley`
and `sley-agent` binaries. From a clean checkout:

```sh
cargo fetch --locked
python3 scripts/build_release_candidate.py --require-clean
sha256sum dist/sley-2.1.6-linux-x86_64.tar.gz
```

The Linux x86_64 archive was built from `ad281d1b3ce6565dc63b67b463260f94df254a1a`. Two clean
builds on the primary host produced identical bytes. Cross-host reproducibility is not claimed.
The archive SHA-256 is `0751911baa8a74dbd5d9751c3ee0c65b9a91e70016b37540a155f318c3f1bfab`
(7,675,247 bytes, 17 members). Provenance remains unsigned.

The persistent-fuzz proofs for this release (all 18 lanes) were run at the built commit with
nightly-2026-02-27 and clang 18.1.8 on the runners' canonical toolchain layout. The GA acceptance
report is unchanged from 2.1.5: of 52 criteria, 31 are evidenced, 17 await review and 4 are gated.
