# RAW_HASH_V1

Status: admitted (RW-075, 2026-09-06). Version 1. Admitted-not-yet-reachable:
the contract, implementation, and vectors land here; host-import wiring
belongs to the RW-080 construction manifest. `BOOTSTRAP_PROFILE_1` is NOT
broadened by this admission.

- Primitive digest record:
  `785205fb49490237cbec7ffe2fc4c2b0f98014b9aae76cc54795921e5d969f72`
  (raw-byte SHA-256 of `conformance/raw-hash/v1/raw-hash.json`;
  `scripts/check_exec_package_v1.py` verifies the binding on every
  `make quick`).
- Contract: `sley2-raw-hash-1`. Machine record:
  `conformance/raw-hash/v1/raw-hash.json` (authoritative for values); this
  document (authoritative for rationale and rules).
- Rust surface: `sley_vm::raw_hash` (`raw_blake3_256`,
  `raw_hash_variant`, `raw_hash_fuel`, `RawHashError`). Full obligation
  inventory:
  `machineresearch/sley-2.0/reweave/rw-075-hash-inventory.md`.

## Design principle

Sley constructs the exact semantic/domain-separated preimage as ordinary
`Bytes` using bootstrap-profile operations. The host performs a narrow
audited primitive hash over those bytes and nothing else.

## The primitive

`RAW_BLAKE3_V1`: raw BLAKE3-256 over Sley-constructed bytes. Input is
exact `Bytes` of length 0..=1_048_576 (the frozen 1 MiB bridge ceiling,
not the 64 MiB image ceiling; larger compiler preimages are chunked by the
Sley driver). Output is the exact 32-byte digest. Over-bound inputs refuse
with deterministic `ResourceLimit` (never truncation). Fuel is 1 plus
`ceil(len/1024)`, charged by the caller through `charge_action` when wired
as an import. Standard BLAKE3 vectors are pinned (empty `af1349...3262`,
`abc` `6437b3...2fc3`); boundary vectors at 0, 1, 1024, 1 MiB admit and
1 MiB + 1 refuses; tamper tests prove one flipped byte changes the digest.

Exactly algorithm variant 1 admits. Unknown variants refuse with
`UnknownVariant` without hashing (default deny; no fallback, no
negotiation). Adding an algorithm needs a new owner amendment plus review.

## Failure codes

The `RawHashError` symbols below are assigned by this contract (owner
adoption, governance wave). Code spelling, precedence, and RW-075/RW-080
semantics are unchanged by this table. Numeric codes are unassigned at
this revision; the owner freezes them by amendment.

| Numeric | Symbolic |
|---|---|
| unassigned | `RAW_HASH_RESOURCE_LIMIT` |
| unassigned | `RAW_HASH_UNKNOWN_VARIANT` |

`RAW_HASH_RESOURCE_LIMIT` refuses a preimage longer than 1,048,576 bytes
without hashing. `RAW_HASH_UNKNOWN_VARIANT` refuses an algorithm tag other
than 1 without hashing (default deny).

## What this is not

Not `fingerprint(program)`, `object_id(program)`,
`validate_and_hash_object(program)`, `candidate_digest` over a high-level
candidate, or any other native semantic digest service. Such services
would conceal compiler semantics (preimage construction, domain
separation, canonicalization) inside native code. Every semantic preimage
— including `sley-id` domain prefixes and `SLEYSFP1`/`SLEYVHS1` framing —
is constructed by Sley as bytes; the host adds nothing. Not a general hash
framework. Not hand-rolled cryptography: a direct call into the audited
`blake3 =1.8.2` crate already pinned by `sley-id`.

Host-only SHA-256 artifact hashes (derived image bytes, package sections)
remain host mechanics and are outside this primitive.
