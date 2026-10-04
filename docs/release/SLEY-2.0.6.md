# Sley 2.0.6 release notes

Sley 2.0.6 adds context-bound editing proposals, headerless function bodies,
integer ranges and indexed vector loops to the agent workbench. These changes
reduce the interface and control-flow scaffolding an author must repeat.
They do not establish model cost, quality or economic parity with another
language. Known-task diagnostic measurements do not establish generalization.

The Linux x86_64 archive was built from
`4c0d97c55a3c41d453777fb740b2f14b8f89a2d0`. Two clean builds on the primary
host produced identical bytes. Cross-host reproducibility is not claimed.
The archive SHA-256 is
`90e072569da7d6fc340ba850803b8e13cc0a608dc831f6ddb4c01c7996f4a054`
(7,207,463 bytes, 17 members). Provenance remains unsigned.

## Workbench changes

- **Exact context binding:** JSON `view` includes the full accepted state root.
  `try --base-root ROOT` refuses a proposal if the accepted state has changed.
  `--functions NAME,OTHER` restricts changes to the named functions and checks
  that protected graph objects remain exact. It requires root binding.
- **Headerless bodies:** `try body.json --body NAME --base-root ROOT` accepts a
  JSON statement list. Add `--familiar` to submit a familiar-text body instead.
  The accepted graph supplies parameter names and types and the return type;
  the proposal preserves the existing interface and parameter identities.
  Scope defaults to the selected function. Generic or effect-declaring
  interfaces that the current authoring format cannot restate are refused.
- **Integer ranges:** `["range", i, start, end, body]`, or familiar
  `for i in start..end`, iterates an ascending, half-open range. Bounds have the
  same integer type and evaluate once, left to right. Empty and reversed ranges
  do nothing. The index is readonly and local to the loop.
- **Indexed vectors:** `["for-indexed", i, x, xs, body]`, or familiar
  `for i, x in xs`, binds a readonly `u64` index and an element for each vector
  entry. The vector expression evaluates once. Nested loops remain supported.

The authoring forms lower to existing graph operations and use the ordinary
candidate validation path. Tests and frontends do not grant commit authority.
No canonical source representation, new opcode or third-party crate is added.
The familiar frontend remains optional and default-on.

See [the workbench specification](../spec/SLEY_AGENT_V1.md), `help structured`
and `help familiar` for the complete syntax, preservation rules and refusals.

`ga_claimed` remains `false`. The [2.0.0 known limits](SLEY-2.0.0.md#known-limits-and-what-is-not-yet-claimed)
remain applicable. Schema epoch, SSMC1, SCB1, SMP1, validation semantics,
the instruction set and the protocol method table are unchanged.

## Candidate packaging

The intended archive is `sley-2.0.6-linux-x86_64.tar.gz`, containing static musl
Linux x86_64 `sley` and `sley-agent` binaries. The pinned Rust toolchain, its
`x86_64-unknown-linux-musl` target and Python 3 are required.

From a clean checkout, the candidate builder performs two clean builds,
compares their archives, scans the content and runs the demo and conformance
subset without source access:

```sh
cargo fetch --locked
python3 scripts/build_release_candidate.py --require-clean
sha256sum dist/sley-2.0.6-linux-x86_64.tar.gz
```

This creates a local candidate and its evidence. It does not publish a release
or complete the broader release evidence gates. Checksums, exact source commit,
test results and any additional host attestations must identify the delivered
2.0.6 artifact; earlier releases' records do not qualify it.
