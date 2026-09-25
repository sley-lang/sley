# BOOTSTRAP_PROFILE_2

Status: frozen successor (RW-075 correction, 2026-09-06). Version 2.
Current R2 candidate. `BOOTSTRAP_PROFILE_1` v1 preserved byte-identical as history.

- Profile digest: `fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459`
  (raw-byte SHA-256 of `conformance/bootstrap-profile/v2/profile.json`;
  `scripts/check_bootstrap_profile_2.py` verifies the binding).
- Contract: `sley2-bootstrap-profile-2`. Machine record:
  `conformance/bootstrap-profile/v2/profile.json` (authoritative for values);
  this document (authoritative for rationale and rules).
- Supersedes: `BOOTSTRAP_PROFILE_1` v1
  (`4f2691504b5c756eae1f5ef01e6e998cc4cd628d4b524b038b10d583bfefd630`,
  `conformance/bootstrap-profile/v1/profile.json`,
  contract `sley2-bootstrap-profile-1`).
  Relation: strict superset adding exactly one import row (`RHW1`);
  no opcode/type/effect/capability broadening. V1 vectors (20) replay
  unchanged under v2 (same `accepted.json` bytes,
  `878f3009690af8a14223667e497d4579a74e1345eea39cf8e75870fdbc7735ad`).

## What this is

`BOOTSTRAP_PROFILE_2` is the exact semantic/execution subset sufficient to
build the Sley toolchain, plus the one narrow raw-hash primitive the
self-hosted compiler needs to construct its own identity digests. It is a
static subset of `EXTENDED_V1`, judged by the production gate
`sley_vm::bootstrap::judge_bootstrap_profile` through the one shared
resolution authority (`resolve_bridge_entry`, now admitting four entries).
It is not a new execution semantics: lowering and execution under
`EXTENDED_V1` are unchanged, and no execution entry calls the gate.

V1 (`BOOTSTRAP_PROFILE_1`, 42 opcodes, 3 imports) is retained as historical
evidence (frozen digest, closure vectors, reviews, `BOOTSTRAP_READY TRUE`,
`R2_ARCHITECTURE_FAIL`). V2 (42 opcodes, 4 imports) is the current R2
candidate that closes premium finding AR-02.

## Bindings

- Schema epoch `08`*32, state root `09`*32 (closure evidence binding,
  same convention as v1 and the vm-extended vectors).
- VM: `vm_version [1,0,0]`, `lowering_profile 2`, `lowerer_version
  [2,0,0]`, bytecode `SLEYBC02`, cache profile `EXTENDED_V1` (unchanged).

## Permitted opcodes (42, unchanged)

Same 42 tags as v1 (E1 data with base Booleans, E2 checked integers, E4
records/variants/maps, E5 cells and value hashing, E6 direct calls).
Opcode 161 admitted only through genuine bridge resolution, never by tag
membership. Exclusions unchanged (E3 floats, E7a 144, E7 145/160/162,
E5 193/194 with v1 rationale).

## Control flow, types, failure registry, ceilings (unchanged)

Identical to v1 (CFG backedge loop Form A, acyclic calls, 256 live frames;
types Unit/Bool/SInt/UInt/Bytes/Text/Tuple/Named/Vector/OrderedMap/Option/
Result/BuiltinFailure/LocalCell-execution-only; failures Arithmetic 1-3,
Index 1-2, DuplicateKey 1, ContractViolation 1, Capability 1-4; same
code-pinned ceilings and reference budgets). No limit moved on the basis
of this correction.

## Permitted native imports (exact, default-deny; 4 rows)

Only the four reviewed pure primitives, as genuine epoch-1
`AdapterImport` rows (identity `SLY1/BRIDGE/<code>` zero-padded to 32
bytes, equal adapter identity, `abi_version` 1, empty effect list):

- `B2V1` (Unit, Bytes → Vector⟨UInt(8)⟩);
- `V2B1` (Unit, Vector⟨UInt(8)⟩ → Bytes);
- `PSH1` (Vector⟨T⟩, T → Vector⟨T⟩, any bootstrap T; monomorphized,
  at most one push row per closed inventory);
- `RHW1` (Unit, Bytes → Bytes, exact 32-byte digest on success).

`RHW1` (`raw-blake3-256`, identity
`534c59312f4252494447452f5248573100000000000000000000000000000000`):
Sley supplies the complete domain-separated preimage as `Bytes`
(0..=1_048_576 bytes per call); the host returns BLAKE3-256 over exactly
those bytes as `Bytes` of length 32 and adds nothing. Over-bound refuses
as a typed `Err(BuiltinFailure(Index), 2)` value (never truncation).
Fuel `1 + ceil(len/1024)` via `raw_hash_fuel`, charged up front through
`charge_action` (bridge-symmetric with the 1 MiB ceiling). Unknown
identities/versions deny with `VM_LOWER_OPCODE_UNSUPPORTED` (default
deny; no fallback, no negotiation). Adding an algorithm needs a new owner
amendment plus review, never analogy.

Large preimages (retired composition rule, RW-075 round 12): the
primitive is stateless one-shot BLAKE3, so hashing chunks independently
cannot reproduce BLAKE3 over the concatenation — and only the full
preimage carries canonical identity. The former `SLEYCHNK1`
hash-of-chunks construction is therefore retired: no chunked digest
function shares the identity channels, and the `SLEYCHNK1` domain is
reserved and must not be used. Preimages longer than 1 MiB refuse as
the typed `Err(BuiltinFailure(Index), 2)` value above, both for carried
constants (refused already at admission under the successor preimage
bound) and for computed values (refused at execution). Measured suite
property: every real Sley-side `RHW1` preimage in-tree is <= ~1 KiB
and uses single-shot (evidence:
`machineresearch/sley-2.0/reweave/rw-075-ar02-evidence.md`); a future
streaming primitive needs its own domain constant and a gated
implementation, never a silent redefinition.

What `RHW1` is not: not `fingerprint(program)`, `object_id(program)`,
`validate_and_hash_object(program)`, `candidate_digest` over a high-level
candidate, or any native semantic digest service. Every semantic preimage
(including `sley-id` domain prefixes and `SLEYSFP1`/`SLEYVHS1` framing)
is constructed by Sley as bytes; the host adds no domain, prefix,
canonicalization, or judgment. Not a general hash framework. Not
hand-rolled crypto: direct call into audited `blake3 =1.8.2` already
pinned by `sley-id`. Host-only SHA-256 artifact hashes remain host
mechanics outside this primitive.

Unknown import identity: reject. Unknown version: reject. Known identity
with wrong schema/profile/epoch: reject. Known native function not present
in the positive manifest: reject. Extension needs a new S20-230 owner
amendment plus profile/manifest review.

## Closure evidence

20 vectors in `conformance/bootstrap-profile/v2/accepted.json` (same bytes
as v1, SHA-256 `878f3009690af8a14223667e497d4579a74e1345eea39cf8e75870fdbc7735ad`),
every one gate-admitted pre-emission under the successor gate (superset:
no v1 vector uses `RHW1`). `RHW1` callability is proved separately in
`crates/sley-vm/tests/rw075_raw_callable.rs` (vectors, boundaries, fuel,
tamper, negatives, preimage ownership, workloads) against the reference
`blake3` call. Cache-key inputs unchanged (twelve frozen fields).

## Dependency closure

Successor remains dependency-closed: same VM/lowering/epoch/root/cache
profile as v1 plus the one audited `blake3 =1.8.2` pin already carried by
`sley-id` (no new crate, no new network, no new toolchain, no FFI).
