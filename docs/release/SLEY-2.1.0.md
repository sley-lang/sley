# Sley 2.1.0 release notes

Sley 2.1.0 is the self-hosted release. Sley's semantic toolchain is now a
Sley program. It decodes canonical program state, checks types and
control-flow graphs, judges operations, lowers to bytecode, fingerprints
the judged closure, and assembles the executable package. Running on a
pinned native host, it rebuilds its own executable closure from its
canonical graph root, without the Rust seed compiler. The rebuild is a
byte-identical fixed point.

This meets the `SH2_SELF_HOSTED_TOOLCHAIN` level of the self-hosting plan (sections 10–15).
The decision record is
[ADR-0056](../adr/ADR-0056-sh2-self-hosted-toolchain.md); the full
implementation and evidence record is
[SH2_SELF_HOSTING.md](SH2_SELF_HOSTING.md).

The Linux x86_64 archive was built from
`0716c127de973a00bcb163780a125ad502858244`. Two clean builds on the primary
host produced identical bytes. Cross-host reproducibility is not claimed.
The archive SHA-256 is
`3ab596958624e09a8525d05c73f5e6cf340c4281e0860d7872a24bc05df8c7a1`
(7,275,190 bytes, 17 members). Provenance remains unsigned. The release
also carries `sley-2.1.0-selfhost-linux-x86_64.tar.gz`, the self-rebuild
bundle described [below](#reproduce-it).

## The bootstrap

| Stage | Built by | Where |
|---|---|---|
| `C1 = C0.build(S)` | the Rust seed compiler | ordinary build |
| `C2 = H.run(C1, S)` | the Sley toolchain C1 | pinned host, empty sandbox, seed absent |
| `C3 = H.run(C2, S)` | the Sley toolchain C2 | pinned host, empty sandbox, seed absent |

C1, C2 and C3 are byte-identical. `S` is the toolchain's canonical graph
root: 62,322 entity objects. One self-build executes 783,179,198 VM
instructions and takes about ten minutes on one x86_64 core. Two separate
runs produced the same bytes.

Stages 2 and 3 run in a bubblewrap root that holds only the static host
binary, `S`, the preceding toolchain package and an output directory. There
is no shell, no Rust toolchain, no source tree and no network. A symbol
audit shows the host links none of the native compile, check, lower, judge
or package-admission entry points. The host admits a toolchain package
only by an explicitly pinned package identity.

## The changed-input proof

A self-build that only reproduces itself could be a copy. 2.1.0 also proves
the toolchain really compiles:

- **A typed change.** A typed `ReplaceEntityVersion` candidate changes one
  constant in `S`: a diagnostic code, from 10000 to 10900.
- **Rebuilt by Sley.** The previous Sley toolchain rebuilds the changed
  `S`, seed absent. The result:
  - equals the Rust seed's build of the same input, byte for byte;
  - is its own fixed point;
  - reports the new code on a malformed request, where the old toolchain
    reports the old one.
- **A fake builder is rejected.** A "builder" that ignores its input and
  returns the prior image fails the same check.
- **Other programs compile identically.** The Sley toolchain builds an
  unrelated program byte-identically to the seed.

Omitted, corrupted and substituted inputs are refused with typed codes, and
no output is written.

## The Sley toolchain

| Responsibility | Module (`crates/sley-selfhost/src/toolchain/`) |
|---|---|
| Canonical SCB1 decoding and strict rejection | `rt`, `scb`, `codec`, `root` |
| Type-environment and function-graph validation | `check` |
| Operation judgment and the bootstrap-profile gate | `judge` |
| SSMC-to-`SLEYBC02` lowering | `lower` |
| Semantic fingerprints | `fingerprint` |
| `EXEC_PACKAGE_V3` assembly | `package` |
| The build driver | `inv`, `driver` |

Each module is tested differentially against the Rust reference. The SCB
walker, codec and root decoder agree on accept/reject across 20,052
corrupted inputs. The checker reports the same first failure on 359
mutants and accepts all 271 toolchain functions. The judge shows zero code
deviations on 3,523 operation and gate mutants. Lowering, fingerprints and
packages are byte-identical. Seven deliberately defective toolchains
("fault seeds") are each detected.

The toolchain is written through a structured seed assembler
(`crates/sley-selfhost/src/seed.rs`). It emits typed SSMC1 graphs once.
From then on the toolchain's source of truth is its canonical graph root,
changed by typed mutations like any other Sley program.

## New in the platform

- **`BOOTSTRAP_PROFILE_3`, `HOST_ABI_V3` and `EXEC_PACKAGE_V3`.** This
  successor profile line adds only pure byte primitives: BLAKE3 over large
  inputs, SHA-256, concatenation, slicing, length, and per-type push rows.
  No opcode, type, effect or capability broadens. Profiles 1 and 2 stay
  byte-identical. See [BOOTSTRAP_PROFILE_3](../spec/BOOTSTRAP_PROFILE_3.md),
  [HOST_ABI_V3](../spec/HOST_ABI_V3.md) and
  [EXEC_PACKAGE_V3](../spec/EXEC_PACKAGE_V3.md).
- **A faster interpreter.** The VM's execution mechanics changed: register
  liveness, ownership-aware in-place updates, shared views and shared
  constants. Workloads that update large values no longer copy them on
  every step; one vector-fill benchmark went from 21 s to 96 ms. Every
  charge, count and observation is unchanged, and the full `sley-vm` suite,
  frozen vectors included, passes.
- **`sley-host`.** A pinned host with no compiler service. It runs a pinned
  `EXEC_PACKAGE_V3` toolchain on a staged build request.

## Reproduce it

From a clean checkout with Rust, the `x86_64-unknown-linux-musl` target
and bubblewrap installed:

```sh
scripts/sh2_bootstrap.sh WORKDIR       # C1, C2, C3 and the fixed point
scripts/sh2_changed_input.sh WORKDIR   # typed change, fake builder, refusals
```

Together they take about an hour. The differential suites that compare
each Sley module with the Rust reference run in release builds
(`cargo test --release -p sley-selfhost`); debug builds skip them. The bootstrap script also assembles the
self-rebuild bundle in `WORKDIR/dist`: the host, the toolchain image, `S`
and a `rebuild.sh` that repeats the self-build with nothing else. The
release's `sley-2.1.0-selfhost-linux-x86_64.tar.gz` is that bundle, with a
README and the license notices; `./rebuild.sh` (bubblewrap required) prints
`fixed point: the rebuilt toolchain is byte-identical`.

## Scope

The native remainder is the one section 10.3 of the self-hosting plan allows:
- bounded execution of the derived image;
- primitive values;
- the byte bridges;
- raw BLAKE3 and SHA-256;
- file staging.

No native service checks, lowers, judges or assembles. The Rust
implementation remains the reference, the oracle and the recovery asset.

The Sley toolchain's program domain is the bootstrap profile: one entry
point, and no effects, contracts, type parameters or recursion. Anything
outside it is refused with a typed `TOOL_*` code. The proof ran on Linux
x86_64; a second host family is future work.

`ga_claimed` remains `false`. Schema epoch, SSMC1, SCB1, SMP1, validation
semantics and the protocol method table are unchanged. No new third-party
crate is added.

## Candidate packaging

The archive is `sley-2.1.0-linux-x86_64.tar.gz`. It contains static musl
Linux x86_64 `sley` and `sley-agent` binaries, as in 2.0.7. From a clean
checkout:

```sh
cargo fetch --locked
python3 scripts/build_release_candidate.py --require-clean
sha256sum dist/sley-2.1.0-linux-x86_64.tar.gz
```

Checksums, the exact source commit, test results and any host attestations
must identify the delivered 2.1.0 artifact. Records from earlier releases
do not qualify it.
