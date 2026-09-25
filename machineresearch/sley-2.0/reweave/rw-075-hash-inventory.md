# RW-075 hash-obligation inventory and admitted primitive (AR-02)

Status: inventory COMPLETE; exactly one raw primitive admitted
(`RAW_BLAKE3_V1`, contract `sley2-raw-hash-1`, version 1,
admitted-not-yet-reachable). `BOOTSTRAP_PROFILE_1` NOT broadened.

## Method

Every hashing/preimage site reachable from the declared SH2 compiler
closure (RW-090 codec, RW-100 checker/test planner, RW-110
lowerer/image-builder, RW-120 build driver, reserved RW-170 Witness
checker) was inspected in-tree. For each obligation: algorithm, domain
separator, exact preimage, whether preimage construction is
semantic/compiler-owned, whether the raw primitive may remain native, and
whether the compiler must receive the digest.

## Obligations

1. Semantic fingerprints — `fingerprint_type_definition`,
   `fingerprint_function` (`crates/sley-ssmc/src/fingerprint.rs`).
   Algorithm: BLAKE3-256 via `sley-id` domain
   `sley2.semantic-fingerprint.v1`. Framing: `SLEYSFP1 || u32(1) ||
   epoch || SSMC1_FIELD_SCHEMA_HASH || u32(kind 4/5) || body`, where body
   is the canonical projection (tags, slots, entity refs with self-ID
   canonicalization). Preimage construction: semantic/compiler-owned
   (projection rules, slot canonicalization, inventory scoping). Raw
   primitive may remain native: YES (BLAKE3 over the finished preimage
   bytes). Compiler receives digest: YES (fingerprints identify
   definitions/functions for the build closure and change detection).
2. Canonical value hashes — `hash_validated_value`
   (`fingerprint.rs:282`). Algorithm: BLAKE3-256 via
   `sley2.value-hash.v1`. Framing: `SLEYVHS1 || u32(1) || epoch ||
   SSMC1_FIELD_SCHEMA_HASH || type_bytes || data_bytes`. Preimage:
   semantic/compiler-owned (type/data canonicalization; caller must have
   run S20-210 validation + hashability judgment first). Native: YES (hash
   only). Compiler receives digest: YES (input hashes, result hashes,
   map-key identity via `key_bytes`).
3. Canonical SCB envelope/digest rules — `sley-mutate` codec
   (`encode_const_value`/`decode_const_value`) plus `sley-scb1` envelope
   rules. Algorithm: codec framing (not a hash) plus BLAKE3 where digested
   downstream. Preimage: semantic/compiler-owned (canonical form,
   strict-rejection policy, RW-090 ownership). Native: codec may remain as
   structural framing (it enforces byte identity, never a verdict); the
   hash over codec bytes uses the raw primitive. Compiler receives digest:
   YES where envelope digests enter build identities.
4. Executable/package identities — `host_abi::image_digest` (SHA-256 over
   exact `SLEYBC02` bytes), `exec_package::section_digest`/`package_digests`
   (SHA-256 over exact section/envelope bytes). Algorithm: SHA-256
   (`sha2`, pure Rust). Preimage: the exact derived bytes (host mechanic,
   not language semantics — the compiler constructs the bytes as data and
   receives the digest as an execution result; it does not construct a
   language-semantic preimage for these). Native: YES, remains host
   mechanics OUTSIDE the raw primitive. Compiler receives digest: YES (as
   results, for comparison and evidence — never as a semantic verdict from
   a native service).
5. Cache/build identities — `derive_cache_key`/`cache_key_preimage`
   (`SLEYBCK1 || u32(1) || epoch || field-schema hash || decoder-limits
   hash || root || entry || vm/lowerer versions || counts || flags` then
   BLAKE3 via `sley2.vm-bytecode-cache-key.v1`). Preimage: mixed — field
   concatenation is structural, but entry/root/epoch/profile selection is
   compiler-owned (build-driver closure decision, RW-120). Native:
   concatenation-plus-hash may remain host mechanic for the reference
   path; the Sley driver reconstructs the identical preimage bytes
   (including the `sley2.vm-bytecode-cache-key.v1` domain prefix, which
   the driver holds as bytes) and hashes via the raw primitive to check
   its own work. Compiler receives digest: YES.
6. Observation identities — `derive_observation_id` (`SLEYOBS1` legacy,
   `SLEYPOBS1` package-bound). Algorithm: BLAKE3 via
   `sley2.observation.v1`. Preimage: mixed — structural concatenation of
   compiler-decided inputs (hashes, limits, termination, counts, package
   digests). Native: host derives (mechanic); the Sley driver rebuilds the
   same bytes to verify evidence. Compiler receives digest: YES.
7. Entity/object/root/transaction/candidate domains (`sley-id`:
   `sley2.entity.v1`, `sley2.object.v1`, `sley2.state-root.v1`,
   `sley2.transaction.v1`, `sley2.transaction-receipt.v1`,
   `sley2.candidate.v1`, `sley2.candidate-result.v1`, policy/capability/
   query/capsule/pack/exchange/delta/merge/session/frame domains).
   Algorithm: BLAKE3-256 per domain. Preimage: semantic/compiler-owned
   where the Sley toolchain constructs the identified object (canonical
   serializations owned by RW-090..RW-120/RW-200); host-mechanic where the
   reference repository/storage layer addresses content. Rule applied:
   wherever the Sley compiler must construct the digest, it constructs the
   full domain-separated preimage as `Bytes` and hashes via the raw
   primitive; the host never offers `object_id(program)`-shaped services.
   Compiler receives digest: YES for toolchain-constructed objects.
8. Build/dependency closure digests — `package_digests` dependency section
   (entry, epoch, root, profile, limits, globals, contracts) plus the
   RW-120 input-closure digest (later). Preimage: compiler-owned (closure
   membership is a build-driver judgment). Native: hash only. Compiler
   receives digest: YES.

## Admitted primitive (exactly one)

`RAW_BLAKE3_V1` (`sley2-raw-hash-1`, v1): raw BLAKE3-256 over
Sley-constructed `Bytes`, 0..=1_048_576 bytes, 32-byte output, deterministic
`ResourceLimit` past the bound, `UnknownVariant` for any algorithm tag
other than 1, fuel 1 + `ceil(len/1024)` via `charge_action` at future
import wiring. Implementation: direct `blake3 =1.8.2` call (the pin
already carried by `sley-id`); no Sley-side crypto. Test vectors: empty
`af1349...3262`, `abc` `6437b3...2fc3`, boundaries 0/1/1024/1 MiB admit,
1 MiB + 1 refuses, tamper/determinism/no-domain pins. Allowlist:
`conformance/raw-hash/v1/raw-hash.json` (variant 1 admitted, default
deny). BLAKE3 alone IS sufficient for every compiler-constructed digest
because every such digest in the closure is BLAKE3-based with a
Sley-held domain prefix; SHA-256 obligations are host-mechanic artifact
hashes the compiler receives as results. No second primitive is admitted
without a new amendment.

## Forbidden (never provided)

`fingerprint(program)`, `object_id(program)`,
`validate_and_hash_object(program)`, `candidate_digest` over a high-level
candidate, or any native service taking programs, candidates, types, or
inventories and returning a digest or verdict. The anti-shortcut probe
(`rw075_hydration_workloads.rs`) pins their absence from `raw_hash.rs`
and `exec_package.rs`.
