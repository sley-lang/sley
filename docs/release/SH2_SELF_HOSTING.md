# Sley SH2: the self-hosted toolchain

Status: implementation record for `SH2_SELF_HOSTED_TOOLCHAIN` (REWEAVE §10).
Decision record: `docs/adr/ADR-0056-sh2-self-hosted-toolchain.md`.
Reproduce with `scripts/sh2_bootstrap.sh WORKDIR` and then
`scripts/sh2_changed_input.sh WORKDIR`.

## The claim

Sley's semantic toolchain is a Sley program. It decodes canonical program
state, checks it, lowers it, fingerprints it, and assembles the executable
package. Running on the pinned native host, it rebuilds its own executable
closure from its canonical graph root without the Rust seed compiler, and
the rebuild is a byte-identical fixed point.

This is a self-hosted semantic toolchain on a native runtime, not a
completely Sley-written operating system, runtime, repository service, or
product (REWEAVE §10.3).

## Toolchain closure (Sley-owned, REWEAVE §10.2)

| Responsibility | Sley module (`crates/sley-selfhost/src/toolchain/`) |
|---|---|
| Canonical SCB1 decoding with strict rejection and bounded traversal | `rt`, `scb`, `codec`, `root` |
| Type-environment and function-graph validation (`TYPE_*`, `GRAPH_*`, `CFG_*`) | `check` |
| Extended operation judgment and the bootstrap-profile gate | `judge` |
| SSMC-to-`SLEYBC02` lowering and image encoding | `lower` |
| Semantic fingerprints of the judged closure | `fingerprint` |
| `EXEC_PACKAGE_V3` sections, digests, and envelope | `package` |
| The build driver: input closure, ordering, assembly | `inv`, `driver` |

No responsibility above delegates its judgment to a native compile, check,
lower, validate, or discharge service.

## Native remainder (REWEAVE §10.3), inventoried

| Native responsibility | Where | Bound |
|---|---|---|
| Bounded execution of the derived image | `sley_vm::execute` | fuel, instruction, value-unit, output, cell, and 256-frame ceilings |
| Allocation, resource accounting, primitive value operations | `sley_vm::execute`, `extended` | charges unchanged from `EXTENDED_V1` |
| Execution mechanics: liveness moves, in-place updates, shared views, shared constants | `execute/liveness.rs`, `execute/owned.rs` | no semantic effect (full `sley-vm` suite plus unit assertions) |
| Image loading and structural memory-safety validation | `sley_vm::host_abi::load_image` | structural only |
| Byte bridges `B2V1 V2B1 PSH1 BCT1 BSL1 BLN1` | `extended.rs` | pure and byte-unaware (`HOST_ABI_V3`) |
| Raw digests `RHW1 RHW2 RSH2` (audited `blake3 =1.8.2`, `sha2`) | `extended.rs`, `raw_hash.rs` | over Sley-built bytes only; no domain or judgment added |
| Package hydration (structural) and pinned-identity admission | `exec_package.rs`, `host_admission.rs` | no gate, no re-lowering |
| Build-request staging (file reads) and output writing | `crates/sley-host` | stages bytes; never compiles, patches, or repairs |

Outside the self-rebuild closure: the Rust reference implementation (the
seed compiler `C0`, oracle, and recovery asset), the seed assembler, the
differential test suites, and the packaging scripts.

## Proof obligations and where they are met

| REWEAVE | Obligation | Evidence |
|---|---|---|
| §13.1–13.2 | `C1 = C0.build(S, P)`, `C2 = H.run(C1)`, `C3 = H.run(C2)`, `C2 == C3` | `scripts/sh2_bootstrap.sh` |
| §13.3 | Stages 2 and 3 run seed-absent in an isolated environment | empty bubblewrap root; static host; symbol audit |
| §13.4 | Changed input via typed mutation, rebuilt by the prior Sley toolchain; fake builder rejected | `scripts/sh2_changed_input.sh` |
| §13.5 | Self-rebuild package closure | `WORKDIR/dist` (host, toolchain image, S, `rebuild.sh`, `SHA256SUMS`) |
| §14.2 | Paired Rust/Sley checks on accepted and rejected programs | `crates/sley-selfhost/tests/toolchain_*.rs` |
| §15 | Canonical, semantic, and executable parity; refusal parity; failure preservation | differential and mutation suites; omitted, corrupt and substituted inputs |
| §15 | Fault seeds are detected | `crates/sley-selfhost/tests/toolchain_faults.rs` (seven seeds) |

## Evidence (2026-10-12)

Recorded run: repository `07b43cf`, clean tree, on one x86_64 Linux host.
The receipts, comparisons, manifests and logs are in
`evidence/reweave/sh2-bootstrap-20261012/`. Paths are normalized to
`WORKDIR` and `REPO`.

### Canonical S and C1 (native seed)

| Item | Value |
|---|---|
| State root | `3f60d7b9179bb34fe7a19ddcb87786ecf9b7434fe9e45cd8a698872a5f29e17e` |
| Objects | 62,322 entity objects (15,766,603 bytes); root 4,113,552 bytes |
| C1 package digest | `389c314cf1ddf36a6964346629ba82a84e4c94dd706f69dd4f6591ed851235ca` |
| C1 envelope | 2,525,082 bytes, SHA-256 `7382fc9d93a27c34ded8c46ed5365719eed3af5549cc84fd5ca0f783dd98e029` |
| C0 build | 36.7 s |

### Fixed point (§13.1–13.3)

| Stage | Toolchain | Status | VM instructions | Wall time | Output SHA-256 |
|---|---|---|---|---|---|
| C2 = H.run(C1, S) | C1 | ok | 783,179,198 | 603 s | `7382fc9d…` |
| C3 = H.run(C2, S) | C2 | ok | 783,179,198 | 604 s | `7382fc9d…` |

C1, C2 and C3 are byte-identical. An earlier independent run produced the
same C1, C2 and C3 bytes, so S, C1 and the fixed point are reproducible.

The host (`sley-host`, SHA-256 `f8a20ad2…`) is a static-pie musl binary.
Its 2,361 symbols include no native compile, check, lower, judge or
admission-authority entry point. Stages after C1 run in a bubblewrap root
that holds only the host, S, the preceding package and an output
directory: no shell, no Rust toolchain, no sources and no network.

### Changed input and fake-builder rejection (§13.4)

1. **Typed candidate.** `cb97f2a2…` replaces constant `9bc36f1d…`
   (`SCB_MAGIC_INVALID`, 10000 → 10900). The state root moves from
   `3f60d7b9…` to `eb24e774…`.
2. **C2′.** The prior Sley toolchain C2 rebuilt S′ seed-absent into C2′
   (package `6b2f8f5a…`). C2′:
   - differs from C2;
   - is byte-identical to the native C0 build of S′;
   - rebuilds itself to the same bytes (C3′ == C2′).
3. **Changed behavior.** On a build request whose first object has a
   corrupt magic, C2 refuses with `toolchain-error-10000` and C2′ with
   `toolchain-error-10900`. Neither writes output.
4. **Fake builder.** It ignores its input and returns the prior image,
   C2. Its output still refuses the same request with
   `toolchain-error-10000`, so it is rejected.
5. **Use closure.** C2 compiles an unrelated program (the fake builder's
   own S, 35 objects) byte-identically to C0.

### Failure preservation (§15)

| Input | Result |
|---|---|
| One object omitted | `toolchain-error-29002`, no output |
| Corrupt object | the diagnostic code above, no output |
| Substituted object (same entity, other body) | `29002` (`toolchain_faults.rs`) |

### Self-rebuild package closure (§13.5)

`WORKDIR/dist` holds:
- the host;
- the toolchain image (`7382fc9d…`);
- S;
- `rebuild.sh`;
- `SHA256SUMS` (`cc0f5363…`).

A copy outside the repository passed its checksums and ran `rebuild.sh`
under `env -i`, in 600 s. The rebuilt toolchain was byte-identical.

### Differential and fault-seed suites (§14.2, §15)

| Module | Corpus | Result |
|---|---|---|
| SCB walker | 5,887 mutated encodings | accept/reject parity |
| Codec | 11,760 corrupted objects | parity; sampled exact decodes |
| Root decoder | 2,405 corruptions | parity |
| Checker | 359 mutated cases; all 271 toolchain functions | identical first failure |
| Judge and gate | 3,300 operation and 223 gate mutants | 0 code deviations |
| Lowerer | toolchain closures up to 972 KB | byte-identical |
| Fingerprints, package | toolchain closures | identical |
| Fault seeds | 7 defective toolchains | all detected |
