//! The admitted raw cryptographic primitive for the Sley compiler closure
//! (RW-075 repair of AR-02).
//!
//! Design principle: Sley constructs the exact semantic/domain-separated
//! preimage as ordinary `Bytes` values using bootstrap-profile operations;
//! the host performs a narrow audited primitive hash over those bytes and
//! nothing else. The host never sees a program, a candidate, a type, or any
//! other high-level object on this path, so no compiler semantics can hide
//! inside a native digest service.
//!
//! This module provides exactly one primitive:
//!
//! `RAW_BLAKE3_V1` — raw BLAKE3-256 over Sley-constructed bytes.
//!
//! * identity: `RAW_BLAKE3_V1`, contract `sley2-raw-hash-1`, version 1;
//! * input: exact `Bytes` (Sley `Bytes` value bytes, length 0..=`RAW_HASH_MAX_BYTES`);
//! * output: exact 32-byte digest (`Bytes` of length 32 on the Sley side);
//! * bounds: [`RAW_HASH_MAX_BYTES`] = `1_048_576` (the frozen 1 MiB bridge
//!   ceiling, NOT the 64 MiB image ceiling — compiler preimages are
//!   constructed through the 1 MiB bridge and chunked by the Sley driver
//!   when larger);
//! * failure: deterministic `ResourceLimit` past the bound; unknown algorithm
//!   variants refuse with `UnknownVariant` (default deny);
//! * fuel: [`raw_hash_fuel`] = 1 + `ceil(len/1024)`, charged by the caller
//!   through the existing `charge_action` mechanism when this primitive is
//!   invoked as a host import (RW-080 wires the import; RW-075 admits the
//!   contract and ships the implementation plus vectors only, so
//!   `BOOTSTRAP_PROFILE_1` is NOT broadened here);
//! * test vectors: standard BLAKE3 vectors plus boundary vectors at 0,
//!   1, 1024, 1 MiB, and 1 MiB + 1 (refusal), plus tamper tests.
//!
//! What this primitive is NOT:
//!
//! * NOT `fingerprint(program)`, `object_id(program)`,
//!   `validate_and_hash_object(program)`, `candidate_digest(candidate)`, or
//!   any other native semantic digest service. Those would conceal compiler
//!   semantics (preimage construction, domain separation, canonicalization)
//!   inside native code. Every semantic preimage — including the `sley-id`
//!   domain prefixes (e.g. `sley2.value-hash.v1`, `SLEYSFP1`, `SLEYVHS1`,
//!   `SLEYBCK1`, `SLEYOBS1`) — is constructed by Sley as bytes. The host
//!   adds no domain, no prefix, no canonicalization, and no judgment.
//! * NOT a general hash framework. Exactly one algorithm variant is
//!   admitted. Unknown variants refuse; adding a variant needs a new owner
//!   amendment plus review, never analogy.
//! * NOT hand-rolled cryptography. This is a direct call into the audited
//!   `blake3 =1.8.2` crate already pinned by `sley-id`; no Sley-side
//!   reimplementation exists or is permitted.
//!
//! Host-only artifact integrity hashes (SHA-256 over derived image bytes via
//! `host_abi::image_digest`, SHA-256 over package sections via
//! `exec_package`) remain host mechanics: the compiler does not construct
//! those preimages as language semantics, it receives those digests as
//! execution results. They are inventoried in
//! `machineresearch/sley-2.0/reweave/rw-075-hash-inventory.md` and are NOT
//! part of this primitive.
//!
//! Machine record: `conformance/raw-hash/v1/raw-hash.json` (authoritative
//! for identity/version/bounds/vectors); contract doc:
//! `docs/spec/RAW_HASH_V1.md`.

use core::fmt;

/// Admitted raw-hash primitive identity.
pub const RAW_HASH_IDENTITY: &str = "RAW_BLAKE3_V1";
/// Admitted raw-hash primitive contract.
pub const RAW_HASH_CONTRACT: &str = "sley2-raw-hash-1";
/// Admitted raw-hash primitive version.
pub const RAW_HASH_VERSION: u32 = 1;
/// The single admitted algorithm variant tag (BLAKE3-256).
pub const RAW_HASH_ALGORITHM_BLAKE3_256: u32 = 1;
/// Maximum raw-hash input bytes: the frozen 1 MiB bridge ceiling.
pub const RAW_HASH_MAX_BYTES: usize = 1_048_576;
/// Output digest bytes (BLAKE3-256).
pub const RAW_HASH_OUTPUT_BYTES: usize = 32;

/// Raw-hash primitive failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RawHashError {
    /// Input exceeds [`RAW_HASH_MAX_BYTES`].
    ResourceLimit,
    /// Unknown algorithm variant (default deny; exactly
    /// [`RAW_HASH_ALGORITHM_BLAKE3_256`] admits).
    UnknownVariant,
}

impl RawHashError {
    /// Returns the stable symbolic code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResourceLimit => "RAW_HASH_RESOURCE_LIMIT",
            Self::UnknownVariant => "RAW_HASH_UNKNOWN_VARIANT",
        }
    }
}

impl fmt::Display for RawHashError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::error::Error for RawHashError {}

/// Fuel required to hash `len` bytes: 1 + `ceil(len/1024)`, minimum 1.
///
/// The caller charges this through the existing `charge_action` mechanism;
/// the primitive itself performs no accounting so it stays a pure function.
#[must_use]
pub fn raw_hash_fuel(len: usize) -> u64 {
    1_u64.saturating_add((len as u64).div_ceil(1024))
}

/// Raw BLAKE3-256 over exactly the supplied bytes (RW-075 AR-02).
///
/// The caller supplies the complete preimage — including any `sley-id`
/// domain prefix or `SLEYSFP1`/`SLEYVHS1` framing the Sley compiler built.
/// This function adds nothing, canonicalizes nothing, and judges nothing:
/// it hashes the bytes and returns the 32-byte digest.
///
/// # Errors
///
/// `ResourceLimit` when `preimage.len() > RAW_HASH_MAX_BYTES`.
pub fn raw_blake3_256(preimage: &[u8]) -> Result<[u8; 32], RawHashError> {
    if preimage.len() > RAW_HASH_MAX_BYTES {
        return Err(RawHashError::ResourceLimit);
    }
    Ok(*blake3::hash(preimage).as_bytes())
}

/// Raw hash with an explicit algorithm selector (default deny).
///
/// Exactly [`RAW_HASH_ALGORITHM_BLAKE3_256`] admits; every other variant —
/// including 0, future tags, and SHA-256 selectors — refuses with
/// `UnknownVariant` without hashing anything. There is no fallback and no
/// negotiation: adding an algorithm needs a new amendment plus review.
///
/// # Errors
///
/// `UnknownVariant` for any unadmitted algorithm, `ResourceLimit` for an
/// over-bound preimage under the admitted algorithm.
pub fn raw_hash_variant(algorithm: u32, preimage: &[u8]) -> Result<[u8; 32], RawHashError> {
    if algorithm != RAW_HASH_ALGORITHM_BLAKE3_256 {
        return Err(RawHashError::UnknownVariant);
    }
    raw_blake3_256(preimage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_hash_standard_vectors_match_blake3() {
        assert_eq!(
            raw_blake3_256(b"").unwrap(),
            [
                0xaf, 0x13, 0x49, 0xb9, 0xf5, 0xf9, 0xa1, 0xa6, 0xa0, 0x40, 0x4d, 0xea, 0x36, 0xdc,
                0xc9, 0x49, 0x9b, 0xcb, 0x25, 0xc9, 0xad, 0xc1, 0x12, 0xb7, 0xcc, 0x9a, 0x93, 0xca,
                0xe4, 0x1f, 0x32, 0x62,
            ],
            "BLAKE3 empty vector"
        );
        let digest = raw_blake3_256(b"abc").unwrap();
        let expected = *blake3::hash(b"abc").as_bytes();
        assert_eq!(digest, expected, "BLAKE3 abc vector via reference call");
        assert_eq!(
            digest,
            [
                0x64, 0x37, 0xb3, 0xac, 0x38, 0x46, 0x51, 0x33, 0xff, 0xb6, 0x3b, 0x75, 0x27, 0x3a,
                0x8d, 0xb5, 0x48, 0xc5, 0x58, 0x46, 0x5d, 0x79, 0xdb, 0x03, 0xfd, 0x35, 0x9c, 0x6c,
                0xd5, 0xbd, 0x9d, 0x85,
            ],
            "BLAKE3 abc pinned bytes"
        );
    }

    #[test]
    fn raw_hash_boundary_vectors() {
        raw_blake3_256(&[]).expect("0 bytes admits");
        raw_blake3_256(&[0x61]).expect("1 byte admits");
        raw_blake3_256(&vec![0x55; 1024]).expect("1024 bytes admits");
        raw_blake3_256(&vec![0x55; RAW_HASH_MAX_BYTES]).expect("exactly 1 MiB admits");
        assert_eq!(
            raw_blake3_256(&vec![0x55; RAW_HASH_MAX_BYTES + 1]),
            Err(RawHashError::ResourceLimit),
            "1 MiB + 1 refuses without hashing"
        );
    }

    #[test]
    fn raw_hash_tamper_changes_digest() {
        let first = raw_blake3_256(b"sley compiler preimage v1").unwrap();
        let second = raw_blake3_256(b"sley compiler preimage v2").unwrap();
        assert_ne!(first, second, "one flipped byte changes the digest");
        assert_eq!(
            raw_blake3_256(b"sley compiler preimage v1").unwrap(),
            first,
            "same preimage is deterministic"
        );
    }

    #[test]
    fn raw_hash_unknown_variants_deny_without_hashing() {
        for variant in [0, 2, 3, 99, u32::MAX] {
            assert_eq!(
                raw_hash_variant(variant, b"abc"),
                Err(RawHashError::UnknownVariant),
                "variant {variant} must deny"
            );
        }
        raw_hash_variant(RAW_HASH_ALGORITHM_BLAKE3_256, b"abc").expect("admitted variant hashes");
    }

    #[test]
    fn raw_hash_fuel_schedule_is_exact() {
        assert_eq!(raw_hash_fuel(0), 1);
        assert_eq!(raw_hash_fuel(1), 2);
        assert_eq!(raw_hash_fuel(1024), 2);
        assert_eq!(raw_hash_fuel(1025), 3);
        assert_eq!(raw_hash_fuel(RAW_HASH_MAX_BYTES), 1025);
    }

    #[test]
    fn raw_hash_adds_no_domain() {
        let sley_built = {
            let mut preimage = Vec::new();
            preimage.extend_from_slice(b"sley2.value-hash.v1");
            preimage.extend_from_slice(b"payload");
            preimage
        };
        assert_eq!(
            raw_blake3_256(&sley_built).unwrap(),
            *blake3::hash(&sley_built).as_bytes(),
            "host adds no domain prefix of its own"
        );
    }
}
