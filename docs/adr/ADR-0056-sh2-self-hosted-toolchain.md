# ADR-0056: The SH2 self-hosted toolchain

Status: accepted for implementation 2026-10-12 under REWEAVE-1.0
(ADR-0049, ADOPT-REWEAVE-2026-09-06) and the operator's standing direction
that self-hosting is a primitive of Sley 2.x. Evidence is recorded in
`docs/release/SH2_SELF_HOSTING.md`.

## Context

REWEAVE §10 makes `SH2_SELF_HOSTED_TOOLCHAIN` mandatory. The program
codec, the semantic checker, the lowerer, the package builder, and the
build driver must run as Sley programs. They must rebuild their own
executable closure without the seed compiler, on a pinned native
execution substrate.

Before this ADR the repository held bounded Sley components (RW-080 to
RW-120), each constructed by hand as SSMC graphs in Rust. The lowerer
consumed native-prepared lowering facts, the checker covered seven toy
projections, and no Sley-built image of the whole toolchain existed. The
hand construction cost roughly 80,000 lines of Rust for the codec alone,
which made a complete toolchain impractical by that method.

## Decision

1. **A structured C0 seed assembler** (`crates/sley-selfhost/src/seed.rs`).
   It is Rust code that emits typed SSMC1 graphs: variables threaded
   through block parameters, structured control flow, matches, checked
   arithmetic, and host-bridge calls. It is not a Sley source language. It
   parses no text, and its output becomes canonical state exactly once
   (`canonical.rs`). From then on the toolchain's source of truth is its
   canonical graph root `S`, which typed mutations change.
2. **A new toolchain written through the assembler**
   (`crates/sley-selfhost/src/toolchain/`). It has a strict SCB1 codec and
   state-root decoder, type-environment and function-graph validation,
   extended operation judgment and the bootstrap gate, `SLEYBC02`
   lowering, semantic fingerprints, `EXEC_PACKAGE_V3` assembly, and the
   build driver `drv_build`. Each module is differentially tested against
   the native reference (the oracle) on valid and mutated inputs. The
   bounded RW-080 components stay in place as history.
3. **A successor profile line**, `BOOTSTRAP_PROFILE_3` / `HOST_ABI_V3` /
   `EXEC_PACKAGE_V3`. It adds only pure byte primitives: large BLAKE3,
   SHA-256, concat, slice, length, and per-type push rows. These are what
   a complete self-build needs and profile 2 cannot express.
   No opcode, type, effect, or capability broadens; earlier lines stay
   byte-identical (see the three spec documents).
4. **VM execution mechanics** (native remainder, REWEAVE §10.3): register
   liveness, ownership-aware in-place updates, shared views, and
   constant sharing. Without them the interpreter copied whole values on
   every update, which made self-compilation quadratic. Every charge,
   count, and observation is unchanged, and the full `sley-vm` suite
   (frozen vectors included) passes.
5. **A pinned host without a compiler** (`crates/sley-host`). It admits a
   v3 package only by an explicitly pinned package identity
   (`sley_vm::host_admission`), stages the build request, and executes.
   Built statically for musl, it links no native compiler service, which
   a symbol audit verifies. Stages after C1 run inside an empty bubblewrap
   root that holds only that binary, `S`, and the preceding package.
6. **The bootstrap proof** (`scripts/sh2_bootstrap.sh`, REWEAVE §13): `C1 =
   C0.build(S, P)`, `C2 = H.run(C1, S)`, `C3 = H.run(C2, S)`, with byte
   equality of C2 and C3, and of C1 and C2 under the unique deterministic
   encoding. A changed-input proof applies a typed mutation to `S` and
   rebuilds it with the prior Sley toolchain.

## Consequences

- The Rust implementation remains the reference, the oracle, and the
  recovery asset. Nothing is deleted (REWEAVE §14.4).
- The native remainder is exactly that of REWEAVE §10.3: image loading and
  execution, primitive values, the byte bridges, raw BLAKE3/SHA-256, and
  file staging. No native service checks, lowers, judges, or assembles.
- The toolchain's program domain is the bootstrap-profile program: one
  entry point, no effects, no contracts, no type parameters, no recursion,
  and eleven entity kinds. Anything outside it refuses with a typed
  `TOOL_*` code rather than a silent fallback.
- Recorded narrowings relative to the native reference:
  - **Multi-fault precedence.** A list or record reads all of its own
    framing before it visits its children. Suppose a later sibling's
    framing and an earlier child's content are both malformed. The SCB
    walker then reports the framing error, and both implementations
    refuse. Single-defect inputs report the native code.
  - **Labels.** Entity labels must be ASCII. A non-ASCII label refuses
    with `TOOL_LABEL_OUT_OF_DOMAIN`.
  - **Entity kinds.** The seven kinds outside the bootstrap domain refuse
    with `TOOL_KIND_OUT_OF_DOMAIN`.
  - **The checker's unreachable branches.** `TYPE_*` 21006, 21010, 21012
    and 21013 are guarded but cannot arise after the loader's checks.
    The visit-budget path 21018 is implemented without a corpus case.
- **Fault seeds** (REWEAVE §15). Build-time switches of the seed
  assembler (`seed::with_faults`) construct seven deliberately defective
  toolchains:
  - a skipped magic check;
  - a skipped set-order check;
  - a wrong operation type rule;
  - a disabled dominance check;
  - a lowering register off by one;
  - a wrong package digest;
  - a disabled root-binding check.

  The differential suite detects each one. Inactive seeds emit no code, so
  canonical S is unchanged.
- The proof runs on one host (x86_64 Linux, static musl host). A second
  pinned host family is future work and is not claimed.
