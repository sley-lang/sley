# Sley 2.0.5 release notes

Sley 2.0.5 extends the agent workbench with structured function bodies,
checked integer conversion, an optional familiar text frontend for authoring
proposals and experimental residual authoring, and it makes verified
execution faster. The release archive is `sley-2.0.5-linux-x86_64.tar.gz`,
containing static musl Linux x86_64 `sley` and `sley-agent` binaries.

There is no separate 2.0.4 release; the runtime work prepared under that
number is part of 2.0.5.

`ga_claimed` remains `false`. The [2.0.0 known limits](SLEY-2.0.0.md#known-limits-and-what-is-not-yet-claimed)
remain applicable. Schema epoch, SSMC1, SCB1, SMP1, validation semantics,
the instruction set and the protocol method table are unchanged.

## Workbench changes

- **Structured function bodies** (ADR-0054): an AF1-X function may carry
  `"body"`, a list of statements (`let`, `var`, `set`, `if`, `for`, `while`,
  `return`, `ok`, `fail`, `trap`) over checked expressions, instead of
  `"blocks"`. The workbench builds the blocks, loop edges and state passing,
  then compiles the result like any AF1-X function; `view --after` shows the
  lowered blocks. Refusals point at the authored statement, including
  compiler refusals inside lowered blocks. Names are block-scoped; a nested
  block cannot redeclare a name declared outside it. See `help structured`.
- **Checked integer conversion:** `["to", T, x]` converts between integer
  types. It is exact for every value that fits; a value outside `T` fails
  like a checked operation (`to?`, `to?Case`). It is built from existing
  checked operations, without a new opcode, and costs time linear in the bit
  width.
- **Optional familiar frontend** (ADR-0055): `sley-agent try --familiar
  <file | ->` reads a small statement syntax and parses it into exactly the
  structured-body frame, which then takes the ordinary path. The switch must
  be given explicitly; the text is kept as the draft's input record, never as
  program state, and later steps read the structured frame. Unsupported or
  ambiguous text is refused with its line and column. The frontend is the
  cargo feature `familiar` (default-on); building with
  `--no-default-features` removes it and changes nothing else. See
  `help familiar`.
- **Residual authoring (experimental, opt-in):** `sley-agent residual`
  constructs repetitive structure from typed fragments, applies sparse
  identity-bound edits, and plans remaining decisions within bounded CPU,
  wall, work and heap budgets. Candidates still take the ordinary AF1-X path
  and the kernel's validation. The default help does not include it; see
  `help residual`.
- **Diagnostics:** JSON `try` results carry `next` and `tests_failed`;
  change summaries report `existing_export` and `visibility`; an unknown
  opcode has a dedicated diagnostic.

The kernel validates every proposed candidate. Frontends, drafts, source
maps, tests and plans do not grant commit authority. The workbench adds no
third-party crate.

## Constitution

ADR-0055 records an operator-authorized amendment: optional, noncanonical
authoring frontends may accept textual proposals when justified by machine
utility, provided they lower deterministically through the ordinary typed
candidate and validation path and never become program authority or a
prerequisite of the graph-native lifecycle. The anti-goal row and the
contribution rule that prohibit a source syntax or parser are narrowed to
exactly that case, and the anti-goal checker verifies the frontend's
confinement. No canonical text, projection, formatter or language service is
added.

## Runtime

Verified execution of scalar graphs uses compact values; admitted inputs and
collections are reused across executions; checked arithmetic operands stay
compact; integer validation is reused during input admission; and canonical
encoding borrows record fields and encodes integer scratch bytes without heap
allocation. Results are unchanged.

The agent binary's allocator exception (ADR-0052) gains heap observations for
residual planning; it stays confined to the binary's allocator module.

## Build and verify

Use the pinned Rust toolchain, its `x86_64-unknown-linux-musl` target,
Python 3 and `uv` in a clean checkout:

```sh
make release-candidate-smoke
sha256sum dist/sley-2.0.5-linux-x86_64.tar.gz
```

Packaging checks compare two clean builds, scan the archive, validate its
manifest and run the demo and conformance subset without source access.
The release records identify the exact built commit and any additional
host attestations. The GitHub release includes `SHA256SUMS`.

After downloading and checking the published checksum:

```sh
tar xzf sley-2.0.5-linux-x86_64.tar.gz
cd sley-2.0.5-linux-x86_64
./bin/sley version
./bin/sley-agent help
python3 demo/run_demo.py
```
