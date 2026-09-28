# Sley 2.0.3 release notes

Sley 2.0.3 extends the agent workbench with compact authoring, persistent
drafts, focused views and checked transformations. The release archive is
`sley-2.0.3-linux-x86_64.tar.gz`, containing static musl Linux x86_64
`sley` and `sley-agent` binaries.

`ga_claimed` remains `false`. The [2.0.0 known limits](SLEY-2.0.0.md#known-limits-and-what-is-not-yet-claimed)
remain applicable. Schema epoch, SSMC1, SCB1, validation semantics and the
protocol method table are unchanged.

## Workbench changes

- **Compact authoring:** opt into AF1-X with `"afx": 1`. Operands can be
  literals or nested operations; `op?Case` unwraps checked results and
  explicitly routes failures. Compact test tables expand to ordinary tests.
  Existing plain AF1 keeps its meaning.
- **Persistent drafts:** refused and valid attempts retain their authored
  frames, revisions and obligations. `try --on` layers changes; `fill`
  repairs exact JSON pointers under a revision check. Submission uses the
  selected valid revision, and head changes require explicit rebasing.
- **Focused views and diagnostics:** `view --focus f --x` shows compact
  function context. Refusals carry authored locations through expansion.
  Arithmetic diagnostics identify wrapped operands before partner literals.
- **Checked transformations:** arity changes update affected calls and
  tests by parameter name; guard extraction preserves the specified error
  order. Ambiguous changes remain explicit obligations. Other transformation
  families remain disabled.
- **Bounded search:** local candidate proposals are validated and tested,
  with limits tied to the accepted head. Search results remain advisory.

The kernel validates every proposed candidate. Drafts, source maps, tests
and search results do not grant commit authority. The workbench adds no
third-party crate. The allocator exception remains limited to the binary
and its existing integration test.

Start with `sley-agent help`; details are in `help afx`, `help drafts`,
`help search` and the [workbench contract](../spec/SLEY_AGENT_V1.md).

## Build and verify

Use the pinned Rust toolchain, its `x86_64-unknown-linux-musl` target,
Python 3 and `uv` in a clean checkout:

```sh
make release-candidate-smoke
sha256sum dist/sley-2.0.3-linux-x86_64.tar.gz
```

Packaging checks compare two clean builds, scan the archive, validate its
manifest and run the demo and conformance subset without source access.
The release records identify the exact built commit and any additional
host attestations. The GitHub release includes `SHA256SUMS`.

After downloading and checking the published checksum:

```sh
tar xzf sley-2.0.3-linux-x86_64.tar.gz
cd sley-2.0.3-linux-x86_64
./bin/sley version
./bin/sley-agent help
python3 demo/run_demo.py
```
