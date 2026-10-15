# BOOTSTRAP_PROFILE_3

Status: SH2 successor (2026-10-12). Version 3. The profile the SH2
self-hosted toolchain is written in and compiles. `BOOTSTRAP_PROFILE_1` and
`BOOTSTRAP_PROFILE_2` are preserved byte-identical as history.

- Profile digest: `8cc5677cd47d803cacccdaa53da2686a81eaff6a76d9546c9edc846fa6b24ced`
  (raw-byte SHA-256 of `conformance/bootstrap-profile/v3/profile.json`; the
  Rust literal is `sley_vm::BOOTSTRAP_PROFILE_3_DIGEST`).
- Contract: `sley2-bootstrap-profile-3`. Machine record:
  `conformance/bootstrap-profile/v3/profile.json` (authoritative for
  values); this document (authoritative for rationale and rules).
- Supersedes: `BOOTSTRAP_PROFILE_2` (`fb2d8cc8...847459`). Relation: strict
  superset. It adds exactly the `HOST_ABI_V3` byte primitives and
  discriminated push identities. No opcode, type, effect, or capability is
  broadened, and v2 closure vectors replay unchanged.
- Gate: `sley_vm::bootstrap::judge_bootstrap_profile` with
  `BootstrapProfileVersion::V3`; the self-hosted gate is the toolchain's
  `jg_gate`.

## Why a successor profile

`BOOTSTRAP_PROFILE_2` was sized for bounded toolchain slices. A toolchain
that decodes, checks, lowers, and packages its own complete closure needs
five things that profile 2 cannot express. Each one is supplied by a pure,
byte-unaware primitive, never by a compiler service:

1. **Package identity.** Execution-package section and package digests are
   SHA-256 (`EXEC_PACKAGE_V2`). Profile 2 exposes only BLAKE3, and only up
   to 1 MiB, so a Sley-built package could not carry its own identity.
   `RSH2` adds raw SHA-256 over complete Sley-built bytes.
2. **Large digests.** The state root and the toolchain image exceed 1 MiB.
   `RHW2` is the `RHW1` primitive with the package ceiling (64 MiB). It
   remains single-shot over the complete preimage. The retired
   `SLEYCHNK1` chunk composition stays retired.
3. **Output assembly.** Building a multi-megabyte image byte by byte through
   `Vector<UInt(8)>` costs about 128 host bytes per output byte. `BCT1`
   concatenates `Bytes` pieces in order.
4. **Large inputs.** The 1 MiB conversion bridge cannot convert a 3 MB
   state root, so inputs are read in windows. `BSL1` slices `Bytes`, and
   `BLN1` reports a length without converting.
5. **Typed growth.** Profile 2 admits one push row per closed inventory, so
   a program can grow vectors of only one element type. Profile 3 gives
   each monomorphized push row its own identity. The `SLY1/BRIDGE/PSH1`
   prefix is followed by a big-endian `u32` discriminator in bytes 16..20.

## Rules

- Everything in `BOOTSTRAP_PROFILE_2` holds: 42 opcodes, acyclic calls, no
  effects, no contracts, no type parameters, the bootstrap type subset,
  inventory coverage, and the Bytes-constant preimage bound.
- Under v3 the gate additionally admits `RHW2`, `RSH2`, `BCT1`, `BSL1`, and
  `BLN1` rows (exact frozen schemas; see `HOST_ABI_V3.md`). It also admits
  `PSH1` rows with a nonzero discriminator. Each such row is a genuine
  epoch-1 `AdapterImport` whose adapter identity equals its entity identity
  (abi 1, `Index` failure, empty effects), and every row must be referenced
  by a reached `adapter_invoke`.
- Under v1 and v2 those rows still refuse with `VM_LOWER_OPCODE_UNSUPPORTED`.
  `EXTENDED_V1` lowering resolves them (the RW-075 precedent for `RHW1`):
  lowering acceptance is a strict superset, and `lowerer_version` and every
  existing image byte are unchanged.

## What the new primitives are not

None of them parses tags, dispatches on schema, enforces canonical form,
judges programs, assembles images by itself, or returns a verdict. Each is
a pure function over caller-owned values: a digest, a concatenation, a
slice, or a length. This is the `host-boundary.json` `must_not` list, held
by construction.
