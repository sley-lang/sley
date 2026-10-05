# Sley 2.0.7 release notes

Sley 2.0.7 is a small point release. It adds an explicit reference execution
mode, a constant-lookup optimization for large compact execution plans, a
clearer scoped-proposal diagnostic and packaging that derives its version from
the Cargo workspace. It does not establish model cost, quality or economic
parity with another language.

ARCHIVE_IDENTITY

## Runtime and workbench changes

- **Reference execution:** `sley-agent call --reference` runs individual and
  batch calls on the reference interpreter instead of the optional compact
  execution plan. Embedders select it with `ExecutionMode::Reference` through
  `VerifiedImage::load_with_mode` or `Executor::with_execution_mode`. Automatic
  selection remains the default. Candidate validation, input admission,
  checked arithmetic, fuel, instruction accounting, observation identities and
  acceptance rules are unchanged; deterministic result, fuel and instruction
  fields match, while measured `vm_micros` may differ. The switch is for
  differential validation and profiling.
- **Ordered constant lookup:** compact execution binds current constants on
  every run. When both the constant-reference count and the inventory have at
  least 16 entries and the inventory is strictly ordered, it uses binary
  lookup; smaller, unsorted or duplicate inventories keep linear first-match
  lookup, and missing or incompatible constants take the existing reference
  path. No index or value is persisted. A local probe on a targeted 96-guard
  fixture measured about 38.6% lower warm time with identical outcomes; no
  meaningful cold-run gain was shown, and this is not an end-to-end or
  general-workload speedup claim.
- **Scoped proposal diagnostic:** when a scoped proposal's function lacks its
  name, the refusal now points at `/fns/N`, names the `fn` key, and calls out
  a `name` key when one was supplied.

See [the workbench specification](../spec/SLEY_AGENT_V1.md) and
[the VM notes](../../crates/sley-vm/README.md) for the exact contracts.

`ga_claimed` remains `false`. The [2.0.0 known limits](SLEY-2.0.0.md#known-limits-and-what-is-not-yet-claimed)
remain applicable. Schema epoch, SSMC1, SCB1, SMP1, validation semantics,
the instruction set and the protocol method table are unchanged. No new
third-party crate is added.

## Candidate packaging

The archive is `sley-2.0.7-linux-x86_64.tar.gz`, containing static musl Linux
x86_64 `sley` and `sley-agent` binaries. Packaging and SBOM versions now come
from the Cargo workspace version, and staging refuses an inventory that names
stale workspace package versions. The candidate publication flag requires the
exact version tag in the recorded operator decision.

From a clean checkout, the candidate builder performs two clean builds,
compares their archives, scans the content and runs the demo and conformance
subset without source access:

```sh
cargo fetch --locked
python3 scripts/build_release_candidate.py --require-clean
sha256sum dist/sley-2.0.7-linux-x86_64.tar.gz
```

This creates a local candidate and its evidence. Checksums, exact source
commit, test results and any host attestations must identify the delivered
2.0.7 artifact; earlier releases' records do not qualify it.
