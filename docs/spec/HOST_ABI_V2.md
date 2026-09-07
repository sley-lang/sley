# HOST_ABI_V2

Status: frozen successor (RW-075 correction, 2026-09-06). Version 2.
Current R2 candidate. `HOST_ABI_V1` v1 preserved byte-identical as history.

- Host ABI digest: `bc564653302a73eb5f998427250a2bb7cd87f5685ef12619bd4ae1f1b2af70d5`
  (raw-byte SHA-256 of `conformance/host-abi/v2/host-abi.json`;
  `scripts/check_host_abi_v2.py` verifies the binding).
- Contract: `sley2-host-abi-2`. Machine record:
  `conformance/host-abi/v2/host-abi.json` (authoritative for values);
  this document (authoritative for rationale and rules).
- Rust surface: `sley_vm::host_abi` (`BRIDGE_CODE_RHW1`,
  `HOST_ABI_V2_IDENTITY/CONTRACT/VERSION`) plus
  `sley_vm::extended` (`BridgeEntry::RawHash`, `BridgeKind::RawHash`,
  shared `resolve_bridge_entry`) plus `sley_vm::raw_hash` (pure primitive).
- Supersedes: `HOST_ABI_V1` v1
  (`e6de00b820a094ec2abc7a6ae43263d7340c2bf1a38a6231e7426fda0f04ecc2`,
  `conformance/host-abi/v1/host-abi.json`, contract `sley2-host-abi-1`).
  Relation: strict superset adding exactly `RHW1`; no other broadening;
  default-deny retained.

## What this is

`HOST_ABI_V2` is the exact native host ABI, executable-image boundary, and
permitted-import contract required by `BOOTSTRAP_PROFILE_2`
(`fb2d8cc8...847459`, contract `sley2-bootstrap-profile-2`) and SH2. It
closes premium finding AR-02: the self-hosted compiler's necessary
identity/hash operations are available through the exact callable
primitive contract `RHW1` (`RAW_BLAKE3_V1`, `sley2-raw-hash-1`, v1).

It freezes no new execution semantics. Lowering and execution under
`EXTENDED_V1` are unchanged; no execution entry calls the profile gate;
the structural image prefix check is a memory-safety shape check only.

## Bindings

- Schema epoch `08`*32, state root `09`*32 (unchanged).
- VM: `vm_version [1,0,0]`, `lowering_profile 2`, `lowerer_version [2,0,0]`,
  bytecode `SLEYBC02`, cache profile `EXTENDED_V1` (unchanged).
- Profile: `BOOTSTRAP_PROFILE_2` digest
  `fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459`.
- Charter: `host-boundary.json` digest
  `d935d238a4d75d154df128aad630411ca3fdcc18e4db084dcd2317ab73bdb18a`
  (byte-stable; cited, never rewritten).
- Cache-key preimage: same twelve frozen fields as v1 (domain `SLEYBCK1`
  + u32 1, epoch, field-schema hash, decoder-limits hash, root, entry,
  VM/lowering versions, counts, flags). The key does not contain the image
  digest, profile digest, limits, or import-manifest digest (v1 rationale
  preserved).

## Permitted native imports (exact, default-deny; 4 rows)

`B2V1`/`V2B1`/`PSH1` unchanged from v1 (same identities, schemas, fuel,
capacity code 2, monomorphization, single-push-row rule, shared
`resolve_bridge_entry`/`resolve_push_row` authority). Plus:

- `RHW1` `raw-blake3-256` (Unit, Bytes → Bytes, exact 32-byte digest):
  identity `SLY1/BRIDGE/RHW1` zero-padded
  (`534c59312f4252494447452f5248573100000000000000000000000000000000`),
  equal adapter identity, `abi_version` 1, empty effect list, `Index`
  failure type. Request `Bytes` (0..=1_048_576 per call, the frozen 1 MiB
  bridge ceiling); response `Bytes` (32 bytes on success); failure
  `BuiltinFailure(Index)` code 2 for over-bound (typed value, never trap
  or truncation); fuel `1 + ceil(len/1024)` via `charge_action` up front;
  determinism byte-exact (audited `blake3 =1.8.2`); host adds no domain.
  Large preimages refuse typed per `BOOTSTRAP_PROFILE_2.md` (the former
  `SLEYCHNK1` composition rule is retired: no chunked construction
  carries canonical identity); the primitive stays stateless one-shot.

Unknown identity/version/row, wrong schema/profile/epoch, and unlisted
native functions all refuse with the frozen vocabularies
(`VM_LOWER_OPCODE_UNSUPPORTED` for unapproved identity, `VM_LOWER_SIGNATURE_MISMATCH`
for schema mismatch, typed `Err(Index, 2)` for over-bound). Extension needs
a new owner amendment plus review, never analogy.

## Operation and value ABI (unchanged except the fourth row)

Invocation shape frozen (`adapter_invoke` tag 161, two operands, one
result, `Entity` immediate); `u32` version 1; exact scope/request/result
binding (`Result<response, BuiltinFailure(Index)>`); canonical
`ConstValue`; `UInt(8)` octet authority; rechecked `Unit` scopes;
no-cell crossing; sorted-map refusal; typed capacity failure; up-front
fuel; deterministic `cancel_at_fuel`. `RHW1` follows the same shape:
operands `(Unit, Bytes)`, result `Result<Bytes, BuiltinFailure(Index)>`,
revalidated at judgment and execution (mismatch is an internal fault,
never a mistyped value).

## Executable image boundary (unchanged)

`SLEYBC02` version 1 layout, SHA-256 digest identity, entry/profile/
epoch/root/manifest/limit bindings, canonical-vs-derived separation, host
MAY load/execute valid images and MUST NOT compile/type-check/judge/
synthesize/repair/fall back, structural `load_image` decoder, `IMAGE_*`
vocabulary — all preserved from v1. Native structural parsing/loading of
an already-created image is permitted; high-level compiler image
construction/lowering remains Sley-owned (see `rw-075-correction.md`
§SLEYBC02 boundary: no native `encode_sleybc02(semantic_program)` service
is reachable; the `lower::encode_function` byte emitter is seed-reference
outside the clean closure; Sley supplies all layout/order/tag decisions).

## SH2 anti-shortcut (extended)

V1 eight compiler-service spellings remain denied, plus the new
forbidden semantic digest services on the raw-hash path:
`fingerprint(program)`, `object_id(program)`,
`validate_and_hash_object(program)`, `candidate_digest(high_level_candidate)`,
or any native service taking programs/candidates/types/inventories and
returning a digest or verdict. Pinned absent from `raw_hash.rs` and
`exec_package.rs` by marker and behavioral probes. No arbitrary symbol
lookup, FFI, hidden Cargo/rustc path, registry-broadening switch, or
production-altering test feature.

## Portability, seed absence, closure evidence

Portability classification unchanged (plus `RHW1` identities/schemas/
denial/fuel/digest rule as portable). Seed-absence condition unchanged,
now on this ABI alone (three primitives plus `RHW1`). Closure evidence:
v1 37 fixtures preserved as history; successor `RHW1` callability proved
in `rw075_raw_callable.rs` (exact invocation, round-trip growth with
hashing, typed capacity refusal, vectors, tamper, fuel, unknown/wrong-
version negatives, gate admission, package binding, workloads).
