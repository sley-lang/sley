# Sley SH2: the self-hosted toolchain

Status: implementation record for `SH2_SELF_HOSTED_TOOLCHAIN` (REWEAVE §10).
Decision record: `docs/adr/ADR-0056-sh2-self-hosted-toolchain.md`.
Reproduce with `scripts/sh2_bootstrap.sh WORKDIR`.

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
| §13.4 | Changed input via typed mutation, rebuilt by the prior Sley toolchain; fake builder rejected | `sley-sh2 mutate-constant`; changed-behavior check |
| §14.2 | Paired Rust/Sley checks on accepted and rejected programs | `crates/sley-selfhost/tests/toolchain_*.rs` |
| §15 | Canonical, semantic, and executable parity; refusal parity; failure preservation | differential and mutation suites |
