# RW-080 §1.3 package-builder slice 35: v2 envelope framing

Status: PROVISIONAL C0 CONSTRUCTION. This is the candidate byte handoff owned
by RW-080 under the operator development override. It has not passed the
required contract/surface review and is not runtime authority.

## Scope and behavior

The native structural boundary now has one deterministic serialized target
for Sley-built package sections. `encode_package_envelope_v2` emits a
316-byte header that is exactly the existing package-digest preimage, followed
by five `u64` length-prefixed sections in image/constants/layouts/imports/
dependency order. Reusing the digest preimage prevents a second header
definition and leaves every existing package and section identity unchanged.

`decode_package_envelope_v2` enforces the serialized ceiling before parsing,
then truncation, magic, version, fixed profile/ABI/VM binding, per-section
ceilings, aggregate payload ceiling, absence of trailing bytes, and exact
section digests. It returns raw section bytes and repeated header identities.
It neither decodes semantic inventories nor mints admission evidence.

The previously unreachable framing errors for unknown magic, unsupported
version, truncation, and trailing data now have executable paths. A header
digest mismatch activates `PACKAGE_SECTION_DIGEST_MISMATCH`; fixed successor
binding drift reports `PACKAGE_BINDING_MISMATCH`.

## Independent candidate vector

`conformance/exec-package-envelope/v2/accepted.hex` is a 605-byte candidate
envelope with an independently reconstructed header and five sections.
`scripts/check_exec_package_envelope_v2.py` rebuilds every byte using Python's
`struct` and `hashlib`, verifies the five SHA-256 section digests, package
digest, complete-envelope checksum, and metadata record, and imports no Rust
implementation. The Rust emitter reproduces the same hex in its unit test.

Two focused Rust tests cover deterministic parity and the live refusal paths.

## Explicit remainder

The decoder does not yet hydrate constants, layouts, imports, globals, or
contracts into an `ExecutionPackage`; the Sley builder does not yet emit the
four non-image sections; and the contract/surface review has not accepted the
candidate layout. Those remain RW-080/RW-110 work. R2 remains provisional
pending the recorded independent acceptance debt.
