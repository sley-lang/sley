# HOST_ABI_V1

Status: frozen (RW-070, 2026-09-06). Version 1.

- Host ABI digest: `e6de00b820a094ec2abc7a6ae43263d7340c2bf1a38a6231e7426fda0f04ecc2`
  (raw-byte SHA-256 of `conformance/host-abi/v1/host-abi.json`;
  `scripts/check_host_abi_v1.py` verifies the binding on every
  `make quick`).
- Contract: `sley2-host-abi-1`. Machine record:
  `conformance/host-abi/v1/host-abi.json` (authoritative for values);
  this document (authoritative for rationale and rules).
- Rust surface: `sley_vm::host_abi` (frozen constants, import-identity
  constructor, structural image prefix check and loader) plus
  `sley_vm::execute_loaded_image` (validation-before-execution runner over
  the manifest-approved `ApprovedImage` binding: digest, cache key, import
  set). Conformance fixtures:
  `crates/sley-vm/tests/rw070_host_abi_freeze.rs` (37 tests, public
  surface only).

## What this is

`HOST_ABI_V1` is the exact native host ABI, executable-image boundary, and
permitted-import contract required by `BOOTSTRAP_PROFILE_1`
(`4f269150…efd630`, contract `sley2-bootstrap-profile-1`) and SH2. It is the
boundary strong enough that later C1/C2/C3 evidence can prove the compiler is
genuinely Sley-owned rather than a thin wrapper around hidden native compiler
services. It implements no self-hosted compiler: RW-080+ owns the toolchain
graph, RW-090+ owns the Sley codec/checker/lowerer programs.

It freezes no new execution semantics. Lowering and execution under
`EXTENDED_V1` are unchanged; no execution entry calls the profile gate; the
structural image prefix check is a memory-safety shape check only. Where a
canonical value encoding already exists it is bound by identity (SSMC1,
S20-350, S20-210); the ABI transports canonical values and never becomes a
second semantic language.

## Bindings

- Schema epoch `08`*32, state root `09`*32 (closure evidence binding, same
  convention as the bootstrap and vm-extended vectors).
- VM: `vm_version [1,0,0]`, `lowering_profile 2`, `lowerer_version [2,0,0]`,
  bytecode `SLEYBC02`, cache profile `EXTENDED_V1`.
- Profile: `BOOTSTRAP_PROFILE_1` digest
  `4f2691504b5c756eae1f5ef01e6e998cc4cd628d4b524b038b10d583bfefd630`.
- Charter: `host-boundary.json` digest
  `d935d238a4d75d154df128aad630411ca3fdcc18e4db084dcd2317ab73bdb18a`
  (byte-stable; this freeze cites it, never rewrites it).
- Cache-key preimage fields are exactly the twelve frozen inputs listed in
  the machine record (domain `SLEYBCK1` + u32 1, epoch, field-schema hash,
  decoder-limits hash, root, entry, vm versions, lowering profile, lowerer
  version, type-argument count, adapter-ABI count, ABI flags). The key is
  derived before emission, independently of image bytes and adapter rows
  (`EXTENDED_V1` fixes `adapter_abi_entries` to 0). The key does NOT
  contain the image digest, the profile digest, limits, or an
  import-manifest digest: the image digest is the SHA-256 of the derived
  bytes, limits bind the observation preimage, import rows bind through
  lowering/gate judgment against the carried inventory (canonical program
  state, not a key field), and the profile binds through gate admission
  reports plus manifest/doc/checker wiring. Gate admission plus observation
  identity bind program, image, imports, and limits end to end.

## Permitted native imports (exact, default-deny)

Only the three reviewed pure primitives, as genuine epoch-1 `AdapterImport`
rows (identity `SLY1/BRIDGE/<code>` zero-padded to 32 bytes, equal adapter
identity, `abi_version` 1, empty effect list), under owner amendment A1
(S20-230 section 1.5):

- `B2V1` `host-bytes-to-u8vector` (Unit, Bytes to Vector<UInt(8)>);
- `V2B1` `host-u8vector-to-bytes` (Unit, Vector<UInt(8)> to Bytes, exact
  8-bit width only);
- `PSH1` `vector-push` (Vector<T>, T to Vector<T>, any bootstrap T,
  response exactly `Vector` of the request type).

Push instantiates by monomorphization (landed: the UInt(8) representative
and the Unit trail, each in its own closed inventory). A closed inventory
carries at most one push row (gate-enforced): import identities are
globally distinct per S20-230 section 2, so identity-level closure is
row-level closure. Lowering, execution, and surcharge still select by
identity plus exact per-use schemas, so the EXTENDED_V1 layer serves any
conforming row without shadowing; the bootstrap subset admits one. A
closure needing two element types needs a new admission, not a second row.
Unknown
identities remain denied and imply no extensible registry. One shared row
predicate underlies every path: `resolve_bridge_entry` (lowering,
execution, surcharge, and gate row checks) and `resolve_push_row`
(per-use push selection in lowering, execution, and surcharge) validate
push rows through the same frozen-field and relationship checks; the gate
adds bootstrap-type membership on the resolved row. Functions
declare no effects; no `AdapterCall` judgment is delegated
anywhere. The gate admits exactly the referenced rows and names them in
its report, in inventory order.

Unknown import identity: reject. Unknown version: reject. Known identity
with wrong schema/profile/epoch: reject. Known native function not present
in the positive manifest: reject. A development helper existing in the Rust
tree does not make it part of the ABI. Extension of the frozen import set
requires a new S20-230 owner amendment plus profile/manifest review.

## Operation and value ABI

- Invocation shape: frozen `adapter_invoke` (tag 161), exactly two operands
  (`scope`, `request`), exactly one result, `Entity` immediate naming a
  carried import. Operation tags are the frozen SSMC tags; versions are
  `u32` with value 1.
- Request binding: operand value types must equal the carried row
  scope/request types exactly; the result must equal exactly
  `Result<response, BuiltinFailure(Index)>`. Mismatches refuse with
  `VM_LOWER_SIGNATURE_MISMATCH`; execution revalidates the same binding
  and reports a mismatch as an internal fault, never a mistyped value.
- Values are canonical `ConstValue` by identity to SSMC1/S20-350. Octet
  range is enforced by the `UInt(8)` element type; the converter enforces
  only the length cap. `Unit` scopes carry no data and are rechecked.
  `LocalCell` never crosses the boundary. Maps arrive sorted; the VM
  refuses unsorted maps without sorting them.
- Failure representation is a typed value: `Err(BuiltinFailure(Index))`,
  code 2 for bridge-capacity refusal over `BRIDGE_MAX_ITEMS` = 1,048,576.
  Capacity refusal is a value, never a trap, termination, or truncation.
- Resource accounting: per-instruction/terminator dispatch charges plus
  bridge fuel charged up front through `charge_action` (1 per element,
  1 per push); a starved budget terminates without performing the work.
- Cancellation is the deterministic `cancel_at_fuel` mark only
  (`VM_EXEC_CANCELLED`); no live OS cancellation exists in-tree.
- Anything outside `BOOTSTRAP_PROFILE_1` refuses before execution with the
  shared lowering vocabulary; no silent fallback to a broader profile ever
  occurs (`VM_LOWER_CACHE_KEY_UNSUPPORTED` pins the cache-key side).

## Executable image boundary

- Format `SLEYBC02`, version 1: magic, `u32` big-endian version, entry
  body, `u64` big-endian callee count, callee bodies in ascending
  function-id order. Identity is the SHA-256 of the exact derived bytes;
  re-lowering reproduces byte-identical output.
- Canonical program state lives in the repository (typed SSMC values,
  content-addressed objects); the image is derived executable state and
  never substitutes for it. Entry point is the entry `EntityId`; profile,
  epoch, root, import manifest, and limits bind through the cache key and
  observation identity.
- The native host may load and execute an already-valid derived image. It
  MUST NOT compile SSMC on behalf of the toolchain, type-check as a hidden
  service, perform CFG/effect/contract/Witness judgment, synthesize
  compiler output, repair malformed images, or silently fall back to the
  Rust seed compiler. Image loading is not compilation.
- Validation before execution through `sley_vm::host_abi::load_image`:
  structural prefix (magic, version, length floor/ceiling) then full body
  parse (entry, callee count, each callee) then end-of-input, returning the
  structure plus the SHA-256 of the parsed bytes (`sha2`, pinned for image
  identity only). The loader is structural only: it carries widths, counts,
  opcodes, and versions without judging any of them. Supplied bytes execute
  through `sley_vm::execute_loaded_image`, which verifies the digest equals
  the manifest-approved `ApprovedImage` binding (digest, cache key,
  import set), each verified before anything runs (`IMAGE_DIGEST_MISMATCH`,
  `IMAGE_BINDING_MISMATCH`), re-derives the cache key from caller
  epoch/root/decoded entry/profile, validates inputs with the exact S20-270
  checks against decoded register types, and runs the single shared runner
  — no lowering, no SSMC reads, no semantic re-derivation. Malformed images
  refuse with the typed `IMAGE_*` vocabulary (`IMAGE_UNKNOWN_MAGIC`,
  `IMAGE_UNSUPPORTED_VERSION`, `IMAGE_TRUNCATED`, `IMAGE_OVERSIZED`,
  `IMAGE_TRAILING_DATA`, `IMAGE_MALFORMED`, `IMAGE_DIGEST_MISMATCH`,
  `IMAGE_BINDING_MISMATCH`); corruption, truncation, and trailing data
  refuse through the loader itself. Nothing is repaired, normalized, or
  tolerated. End-to-end association is encoded: manifest digest, loader
  digest match, caller inventories under epoch/root, re-derived cache key
  verified against the approved binding, import-set equality, observation.
  The residual trust root is identical on both paths (the caller holds the
  true manifest and inventories); toolchain genuineness is established by
  the later fixed-point and self-change evidence this boundary makes
  testable.

## SH2 anti-shortcut

The following are denied by test and never qualify as primitives or host
mechanics: `compile(program)`, `validate(program)`, `typecheck(program)`,
`lower(program)`, `assemble_ssmc(program)`,
`decode_and_validate_schema(program)`, `discharge_witness(program/value)`,
`construct_compiler_image(high_level_graph)`. The equivalent internal Rust
functions (seed references in `sley-scb1`, `sley-ssmc`, `sley-check`,
lower/execute paths) are unreachable through the frozen registry: every
spelling refuses with `VM_LOWER_OPCODE_UNSUPPORTED`, pinned by
`rw070_native_compiler_services_are_not_admitted`. There is no arbitrary
symbol lookup, dynamic library loading, FFI, hidden Cargo/rustc path,
registry-broadening environment switch, or production-altering debug/test
feature: the bootstrap closure crates (`sley-vm`, `sley-check`,
`sley-ssmc`, `sley-id`, `sley-mutate`) contain no `std::fs`, `std::env`,
`std::process`, `std::net`, or `unsafe` outside tests, and every crate
carries `#![forbid(unsafe_code)]`.

## Portability

Intrinsically portable: import identities, schemas, and denial rules;
operation tags and canonical value representation; image magic, version,
layout order, and digest rule; capacity and fuel constants; failure symbols
and refusal precedence; cache-key preimage order and encodings. Current
platform: Linux x86-64 under toolchain 1.93.0 minimal; no multi-platform
qualification is claimed. The ABI encodes no absolute paths, usernames,
filesystem layout, Rust type names, compiler-private symbols, pointer-width
or endianness accidents, or build-directory identities (pack bytes carry no
store path per RW-060).

## Seed-absence preparation

RW-070 does NOT prove seed absence. When C1/C2/C3 exist, the R3/R4
environment removes Cargo, rustc, the seed compiler executable, the
development tree, compiler-service endpoints, and test-only helpers; the
remaining host must still execute a valid Sley-built compiler image using
only this frozen ABI. That condition is defined here and tested later; no
C1/C2/C3 population or passing claim belongs to this package.

## Closure evidence

37 integration fixtures (`rw070_host_abi_freeze.rs`, public surface only):
exact invocation per primitive, composed round-trip with growth, typed
capacity refusal, deterministic image load/execution/observation, stable
and bound cache identity, loader round-trip fidelity, through-loader
tamper refusal (magic, version, truncation, callee-count, trailing data)
and malformed-tag refusal, per-use push selection without shadowing
(single and combined closures, both inventory orders), loaded execution
with lowering-path parity (value, observation, cache key, input-refusal
codes), manifest-identity verification, epoch re-keying, and negatives
for unknown identity, wrong version, foreign adapter identity, effectful
rows, unreferenced rows, wrong request/result schemas, non-u8 width,
restricted-profile use, wrong VM version, nonzero ABI flags, corruption,
truncation, trailing data, wrong magic/version, oversize,
compiler-service attempts, helper injection, dev version zero,
gate/lowering denial parity, and deterministic cancellation. Unit pins in
`host_abi.rs` bind every manifest constant to its landed value. The RW-060
lifecycle plus the 20 closure vectors serve as the real-world fixture.
Extension needs a new owner amendment, not an analogy.
