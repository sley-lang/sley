//! The frozen `HOST_ABI_V1` native boundary surface (RW-070).
//!
//! This module publishes the exact native host ABI constants the later
//! seed-absent toolchain may rely on and nothing else: the import-identity
//! rule, the three admitted bridge rows' frozen parameters, the derived
//! executable-image prefix rule, and the resource ceilings that bind the
//! boundary. It introduces no new execution semantics: lowering, judgment,
//! and execution live where they already live (`lower`, `extended`,
//! `execute`, `bootstrap`). The structural image prefix check below is a
//! memory-safety shape check only; it never type-checks, lowers, validates
//! programs, discharges witnesses, or assembles compiler output.
//!
//! Machine record: `conformance/host-abi/v1/host-abi.json` (authoritative
//! for values); contract doc: `docs/spec/HOST_ABI_V1.md` (authoritative for
//! rationale and rules). `scripts/check_host_abi_v1.py` verifies the
//! record, the doc, and these constants agree on every `make quick`.

use core::fmt;

use sha2::{Digest as _, Sha256};
use sley_id::EntityId;
use sley_ssmc::{
    BuiltinCase, BuiltinFailureKind, FunctionRefValue, FunctionType, Immediate, IntegerWidth,
    MemberId, NamedType, TypeExpr, VariantImmediate,
};

use crate::lower::{
    BlockSlot, BytecodeBlock, BytecodeFunction, BytecodeSwitchArgument, BytecodeSwitchCase,
    BytecodeSwitchEdge, BytecodeTargetEdge, BytecodeTerminator, Instruction, Register,
};

/// Frozen host ABI identity.
pub const HOST_ABI_IDENTITY: &str = "HOST_ABI_V1";
/// Frozen host ABI contract.
pub const HOST_ABI_CONTRACT: &str = "sley2-host-abi-1";
/// Frozen host ABI version.
pub const HOST_ABI_VERSION: u32 = 1;

/// Successor host ABI identity (RW-075 correction, AR-02).
///
/// `HOST_ABI_V1` (version 1, three imports) is preserved byte-identical as
/// history. `HOST_ABI_V2` (version 2, four imports including `RHW1`) is the
/// current R2 candidate. Live execution resolves all four through the one
/// shared authority; the v1 record/gate history remains replayable because
/// v2 is a strict superset (no v1 vector uses `RHW1`).
pub const HOST_ABI_V2_IDENTITY: &str = "HOST_ABI_V2";
/// Successor host ABI contract.
pub const HOST_ABI_V2_CONTRACT: &str = "sley2-host-abi-2";
/// Successor host ABI version.
pub const HOST_ABI_V2_VERSION: u32 = 2;

/// Frozen import-identity prefix: twelve ASCII bytes `SLY1/BRIDGE/`.
pub const BRIDGE_IDENTITY_PREFIX: &[u8; 12] = b"SLY1/BRIDGE/";
/// Frozen entry code: `host-bytes-to-u8vector`.
pub const BRIDGE_CODE_B2V1: [u8; 4] = *b"B2V1";
/// Frozen entry code: `host-u8vector-to-bytes`.
pub const BRIDGE_CODE_V2B1: [u8; 4] = *b"V2B1";
/// Frozen entry code: `vector-push`.
pub const BRIDGE_CODE_PSH1: [u8; 4] = *b"PSH1";
/// Successor entry code: `raw-blake3-256` (RW-075 correction, AR-02).
///
/// Pure bytes-to-digest primitive over Sley-constructed preimages only.
/// Admitted in `HOST_ABI_V2` / `BOOTSTRAP_PROFILE_2`; absent from the
/// frozen `HOST_ABI_V1` record (preserved byte-identical as history).
pub const BRIDGE_CODE_RHW1: [u8; 4] = *b"RHW1";
/// Frozen adapter ABI version carried by every admitted row.
pub const BRIDGE_ABI_VERSION: u32 = 1;

/// Frozen bridge capacity: no bridge byte string or octet vector exceeds
/// 2^20 bytes/elements (symmetric with the execution cell cap).
pub const HOST_ABI_BRIDGE_MAX_ITEMS: usize = 1_048_576;
/// Frozen per-element bridge fuel charged through `charge_action`.
pub const HOST_ABI_BRIDGE_ELEMENT_FUEL: u64 = 1;
/// Frozen bridge-capacity failure code under `BuiltinFailure(Index)`.
pub const HOST_ABI_BRIDGE_CAPACITY_CODE: u16 = 2;

/// Frozen derived-image magic for the extended profile.
pub const IMAGE_MAGIC_SLEYBC02: &[u8; 8] = b"SLEYBC02";
/// Frozen derived-image format version (`u32`, big-endian, value 1).
pub const IMAGE_VERSION: u32 = 1;
/// Frozen derived-image byte ceiling (the lowering encoder bound).
pub const IMAGE_MAX_BYTES: usize = 67_108_864;
/// Minimum structural image length: 8 magic bytes plus 4 version bytes.
pub const IMAGE_MIN_BYTES: usize = 12;

/// Builds the frozen 32-byte import identity for one entry code: twelve
/// ASCII bytes `SLY1/BRIDGE/`, the four-byte code, sixteen zero bytes.
///
/// This is the single identity construction the manifest, the fixtures,
/// and the tests share; resolution itself stays with
/// `extended::resolve_bridge_entry`, so this constructor cannot admit an
/// import on its own.
#[must_use]
pub const fn bridge_identity(code: [u8; 4]) -> [u8; 32] {
    let mut identity = [0_u8; 32];
    let mut index = 0;
    while index < 12 {
        identity[index] = BRIDGE_IDENTITY_PREFIX[index];
        index += 1;
    }
    let mut cursor = 0;
    while cursor < 4 {
        identity[12 + cursor] = code[cursor];
        cursor += 1;
    }
    identity
}

/// Frozen structural image failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageError {
    /// Magic prefix is not `SLEYBC02`.
    UnknownMagic,
    /// Format version is not the frozen `IMAGE_VERSION`.
    UnsupportedVersion,
    /// Bytes end before the structure does.
    Truncated,
    /// More than [`IMAGE_MAX_BYTES`] bytes are present.
    Oversized,
    /// Trailing bytes follow the last callee body.
    TrailingData,
    /// A tag, shape, or count the frozen layout cannot carry.
    Malformed,
    /// Supplied bytes do not match the expected manifest identity.
    DigestMismatch,
    /// A manifest-approved binding does not match the execution: cache key
    /// or import set.
    BindingMismatch,
}

impl ImageError {
    /// Returns the stable symbolic code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownMagic => "IMAGE_UNKNOWN_MAGIC",
            Self::UnsupportedVersion => "IMAGE_UNSUPPORTED_VERSION",
            Self::Truncated => "IMAGE_TRUNCATED",
            Self::Oversized => "IMAGE_OVERSIZED",
            Self::TrailingData => "IMAGE_TRAILING_DATA",
            Self::Malformed => "IMAGE_MALFORMED",
            Self::DigestMismatch => "IMAGE_DIGEST_MISMATCH",
            Self::BindingMismatch => "IMAGE_BINDING_MISMATCH",
        }
    }
}

impl fmt::Display for ImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::error::Error for ImageError {}

/// Checks the structural prefix of derived-image bytes: length floor and
/// ceiling, magic, and format version.
///
/// This is a memory-safety shape check only. It parses no instructions,
/// resolves no types, judges no semantics, assembles nothing, repairs
/// nothing, and falls back to nothing: any failure is a deterministic
/// typed refusal. Semantic admission stays with
/// `bootstrap::judge_bootstrap_profile` and lowering/execution judgment.
/// Full structural validation lives in [`load_image`].
///
/// # Errors
///
/// Returns the exact structural refusal for the first failed check, in
/// length, magic, version order.
pub fn check_image_prefix(bytes: &[u8]) -> Result<(), ImageError> {
    if bytes.len() < IMAGE_MIN_BYTES {
        return Err(ImageError::Truncated);
    }
    if bytes.len() > IMAGE_MAX_BYTES {
        return Err(ImageError::Oversized);
    }
    if bytes[..8] != *IMAGE_MAGIC_SLEYBC02 {
        return Err(ImageError::UnknownMagic);
    }
    let version = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    if version != IMAGE_VERSION {
        return Err(ImageError::UnsupportedVersion);
    }
    Ok(())
}

/// One structurally loaded derived image: its manifest identity plus the
/// entry body and callee table in ascending function-id order, exactly as
/// [`load_image`] parsed them. Structure only: loading judges no opcode,
/// type, reference, or resource semantics. Whether a loaded image may
/// execute, and under which inventories and bindings, is decided by the
/// expected-digest check in `execute_loaded_image` plus the gate and the
/// execution request — never by the loader.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedImage {
    /// SHA-256 over the exact supplied bytes this structure parsed from.
    pub digest: [u8; 32],
    /// The entry function body.
    pub entry: BytecodeFunction,
    /// Callee bodies in ascending function-id order.
    pub callees: Vec<BytecodeFunction>,
}

/// Derives the manifest identity of derived-image bytes: SHA-256 over the
/// exact bytes, the same digest conformance records as `bytecode_sha256`.
///
/// The hash library pinned here (`sha2`, pure Rust) is the RW-070 supply-chain
/// selection for image identity only; it carries no language semantics.
#[must_use]
pub fn image_digest(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

/// Loads and structurally validates derived-image bytes: prefix, one entry
/// body, the callee count, each callee body, then end-of-input.
///
/// Every bound is checked before it is trusted: overruns refuse with
/// [`ImageError::Truncated`], trailing bytes with
/// [`ImageError::TrailingData`], unknown tags and shapes with
/// [`ImageError::Malformed`]. Widths, counts, opcodes, and versions are
/// carried structurally — the loader accepts any well-formed encoding and
/// judges none of it: semantic validity (admitted opcodes, type agreement,
/// reference integrity, resource ceilings) stays entirely with lowering
/// judgment and the profile gate, which re-derive it from canonical
/// program state rather than trusting the image. The returned structure
/// carries the byte identity it parsed from, so callers can verify the
/// manifest-approved digest before executing anything.
///
/// # Errors
///
/// Returns the exact structural refusal for the first malformed position.
pub fn load_image(bytes: &[u8]) -> Result<LoadedImage, ImageError> {
    check_image_prefix(bytes)?;
    let mut cursor = Cursor {
        bytes,
        pos: IMAGE_MIN_BYTES,
    };
    let entry = cursor.body()?;
    let callees = cursor.counted(Cursor::body)?;
    if cursor.pos != bytes.len() {
        return Err(ImageError::TrailingData);
    }
    Ok(LoadedImage {
        digest: image_digest(bytes),
        entry,
        callees,
    })
}

/// A bounds-checked cursor over untrusted image bytes.
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], ImageError> {
        let end = self.pos.checked_add(count).ok_or(ImageError::Malformed)?;
        if end > self.bytes.len() {
            return Err(ImageError::Truncated);
        }
        let taken = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(taken)
    }

    fn u16_be(&mut self) -> Result<u16, ImageError> {
        let taken = self.take(2)?;
        Ok(u16::from_be_bytes([taken[0], taken[1]]))
    }

    fn u32_be(&mut self) -> Result<u32, ImageError> {
        let taken = self.take(4)?;
        Ok(u32::from_be_bytes([taken[0], taken[1], taken[2], taken[3]]))
    }

    fn u64_be(&mut self) -> Result<u64, ImageError> {
        let taken = self.take(8)?;
        Ok(u64::from_be_bytes([
            taken[0], taken[1], taken[2], taken[3], taken[4], taken[5], taken[6], taken[7],
        ]))
    }

    fn count(&mut self) -> Result<usize, ImageError> {
        let raw = self.u64_be()?;
        usize::try_from(raw).map_err(|_| ImageError::Malformed)
    }

    fn counted<T>(
        &mut self,
        mut item: impl FnMut(&mut Self) -> Result<T, ImageError>,
    ) -> Result<Vec<T>, ImageError> {
        let count = self.count()?;
        let mut items = Vec::new();
        for _ in 0..count {
            items.push(item(self)?);
        }
        Ok(items)
    }

    fn raw_id(&mut self) -> Result<EntityId, ImageError> {
        let taken = self.take(32)?;
        let mut id = [0_u8; 32];
        id.copy_from_slice(taken);
        Ok(EntityId::from_bytes(id))
    }

    fn member_id(&mut self) -> Result<MemberId, ImageError> {
        let taken = self.take(32)?;
        let mut id = [0_u8; 32];
        id.copy_from_slice(taken);
        Ok(MemberId::from_bytes(id))
    }

    /// The encoder's `entity` shape: a `u32` version word that must be 1,
    /// then 32 identity bytes.
    fn entity(&mut self) -> Result<EntityId, ImageError> {
        if self.u32_be()? != 1 {
            return Err(ImageError::Malformed);
        }
        self.raw_id()
    }

    fn registers(&mut self) -> Result<Vec<Register>, ImageError> {
        self.counted(Self::u32_be)
    }

    fn type_expr(&mut self, depth: usize) -> Result<TypeExpr, ImageError> {
        if depth > sley_ssmc::MAX_TYPE_DEPTH {
            return Err(ImageError::Malformed);
        }
        let next = depth.saturating_add(1);
        match self.u32_be()? {
            1 => Ok(TypeExpr::Unit),
            2 => Ok(TypeExpr::Bool),
            3 => Ok(TypeExpr::SInt(IntegerWidth::from_bits(self.u16_be()?))),
            4 => Ok(TypeExpr::UInt(IntegerWidth::from_bits(self.u16_be()?))),
            5 => Ok(TypeExpr::F32),
            6 => Ok(TypeExpr::F64),
            7 => Ok(TypeExpr::Bytes),
            8 => Ok(TypeExpr::Text),
            9 => Ok(TypeExpr::Tuple(
                self.counted(|cursor| cursor.type_expr(next))?,
            )),
            10 => Ok(TypeExpr::Named(NamedType {
                definition: self.entity()?,
                arguments: self.counted(|cursor| cursor.type_expr(next))?,
            })),
            11 => Ok(TypeExpr::Vector(Box::new(self.type_expr(next)?))),
            12 => Ok(TypeExpr::OrderedMap {
                key: Box::new(self.type_expr(next)?),
                value: Box::new(self.type_expr(next)?),
            }),
            13 => Ok(TypeExpr::Option(Box::new(self.type_expr(next)?))),
            14 => Ok(TypeExpr::Result {
                ok: Box::new(self.type_expr(next)?),
                error: Box::new(self.type_expr(next)?),
            }),
            15 => Ok(TypeExpr::FunctionRef(FunctionType {
                parameters: self.counted(|cursor| cursor.type_expr(next))?,
                result: Box::new(self.type_expr(next)?),
                effects: self.counted(Self::entity)?,
            })),
            16 => Ok(TypeExpr::AdapterHandle(self.entity()?)),
            17 => Ok(TypeExpr::CapabilityToken(self.entity()?)),
            18 => Ok(TypeExpr::LocalCell(Box::new(self.type_expr(next)?))),
            19 => Ok(TypeExpr::TypeParameter(self.u32_be()?)),
            20 => Ok(TypeExpr::BuiltinFailure(match self.u16_be()? {
                1 => BuiltinFailureKind::Arithmetic,
                2 => BuiltinFailureKind::Index,
                3 => BuiltinFailureKind::DuplicateKey,
                4 => BuiltinFailureKind::ContractViolation,
                5 => BuiltinFailureKind::Capability,
                _ => return Err(ImageError::Malformed),
            })),
            _ => Err(ImageError::Malformed),
        }
    }

    fn immediate(&mut self) -> Result<Immediate, ImageError> {
        match self.u32_be()? {
            1 => Ok(Immediate::None),
            2 => Ok(Immediate::Entity(self.entity()?)),
            3 => Ok(Immediate::Index(self.u32_be()?)),
            4 => Ok(Immediate::Field(self.member_id()?)),
            5 => Ok(Immediate::Variant(VariantImmediate {
                definition: self.entity()?,
                member_id: self.member_id()?,
            })),
            6 => {
                let taken = self.take(32)?;
                let mut digest = [0_u8; 32];
                digest.copy_from_slice(taken);
                Ok(Immediate::Observation(digest))
            }
            7 => Ok(Immediate::Function(FunctionRefValue {
                function: self.entity()?,
                type_arguments: self.counted(|cursor| cursor.type_expr(1))?,
            })),
            _ => Err(ImageError::Malformed),
        }
    }

    fn edge(&mut self) -> Result<BytecodeTargetEdge, ImageError> {
        Ok(BytecodeTargetEdge {
            target: self.u32_be()?,
            arguments: self.registers()?,
        })
    }

    fn switch_edge(&mut self) -> Result<BytecodeSwitchEdge, ImageError> {
        Ok(BytecodeSwitchEdge {
            target: self.u32_be()?,
            arguments: self.counted(|cursor| match cursor.u32_be()? {
                1 => Ok(BytecodeSwitchArgument::Value(cursor.u32_be()?)),
                2 => Ok(BytecodeSwitchArgument::CasePayload),
                _ => Err(ImageError::Malformed),
            })?,
        })
    }

    fn case_key(&mut self) -> Result<sley_ssmc::CaseKey, ImageError> {
        match self.u32_be()? {
            1 => Ok(sley_ssmc::CaseKey::Member(self.member_id()?)),
            2 => Ok(sley_ssmc::CaseKey::Builtin(match self.u32_be()? {
                1 => BuiltinCase::None,
                2 => BuiltinCase::Some,
                3 => BuiltinCase::Ok,
                4 => BuiltinCase::Err,
                _ => return Err(ImageError::Malformed),
            })),
            _ => Err(ImageError::Malformed),
        }
    }

    fn terminator(&mut self) -> Result<BytecodeTerminator, ImageError> {
        match self.u32_be()? {
            1 => Ok(BytecodeTerminator::Return(self.u32_be()?)),
            2 => Ok(BytecodeTerminator::Branch(self.edge()?)),
            3 => Ok(BytecodeTerminator::CondBranch {
                condition: self.u32_be()?,
                if_true: self.edge()?,
                if_false: self.edge()?,
            }),
            4 => Ok(BytecodeTerminator::VariantSwitch {
                value: self.u32_be()?,
                cases: self.counted(|cursor| {
                    Ok(BytecodeSwitchCase {
                        case_key: cursor.case_key()?,
                        edge: cursor.switch_edge()?,
                    })
                })?,
            }),
            5 => Ok(BytecodeTerminator::Trap {
                code: self.u32_be()?,
                payload: match self.u32_be()? {
                    1 => None,
                    2 => Some(self.u32_be()?),
                    _ => return Err(ImageError::Malformed),
                },
            }),
            _ => Err(ImageError::Malformed),
        }
    }

    fn body(&mut self) -> Result<BytecodeFunction, ImageError> {
        let function = self.raw_id()?;
        let parameter_registers = self.registers()?;
        let register_types = self.counted(|cursor| cursor.type_expr(1))?;
        let result_type = self.type_expr(1)?;
        let entry_block: BlockSlot = self.u32_be()?;
        let blocks = self.counted(|cursor| {
            Ok(BytecodeBlock {
                slot: cursor.u32_be()?,
                parameter_registers: cursor.registers()?,
                instructions: cursor.counted(|cursor| {
                    Ok(Instruction {
                        opcode: cursor.u32_be()?,
                        operands: cursor.registers()?,
                        results: cursor.registers()?,
                        immediate: cursor.immediate()?,
                    })
                })?,
                terminator: cursor.terminator()?,
                reachability: cursor.u32_be()?,
            })
        })?;
        Ok(BytecodeFunction {
            function,
            parameter_registers,
            register_types,
            result_type,
            entry_block,
            blocks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_abi_constants_bind_the_landed_bridge_values() {
        assert_eq!(
            HOST_ABI_BRIDGE_MAX_ITEMS,
            crate::extended::BRIDGE_MAX_ITEMS,
            "manifest capacity must equal the landed judgment/execution cap"
        );
        assert_eq!(
            HOST_ABI_BRIDGE_ELEMENT_FUEL,
            crate::extended::BRIDGE_ELEMENT_FUEL,
            "manifest fuel must equal the landed per-element charge"
        );
        assert_eq!(
            HOST_ABI_BRIDGE_CAPACITY_CODE,
            crate::extended::BRIDGE_CAPACITY_CODE,
            "manifest capacity code must equal the landed Index code"
        );
        assert_eq!(
            IMAGE_MAX_BYTES,
            crate::lower::MAX_BYTECODE_BYTES,
            "manifest image ceiling must equal the landed encoder bound"
        );
    }

    #[test]
    fn bridge_identity_construction_is_exact() {
        let mut expected = [0_u8; 32];
        expected[..12].copy_from_slice(b"SLY1/BRIDGE/");
        expected[12..16].copy_from_slice(b"B2V1");
        assert_eq!(bridge_identity(BRIDGE_CODE_B2V1), expected);
        expected[12..16].copy_from_slice(b"PSH1");
        assert_eq!(bridge_identity(BRIDGE_CODE_PSH1), expected);
    }

    #[test]
    fn image_prefix_check_is_structural_only() {
        let mut valid = Vec::new();
        valid.extend_from_slice(IMAGE_MAGIC_SLEYBC02);
        valid.extend_from_slice(&IMAGE_VERSION.to_be_bytes());
        check_image_prefix(&valid).expect("frozen prefix admits");
        assert_eq!(
            check_image_prefix(&[]),
            Err(ImageError::Truncated),
            "empty bytes truncate"
        );
        assert_eq!(
            check_image_prefix(b"SLEYBC01\x00\x00\x00\x01"),
            Err(ImageError::UnknownMagic),
            "restricted magic is not the bootstrap image"
        );
        let mut wrong_version = Vec::new();
        wrong_version.extend_from_slice(IMAGE_MAGIC_SLEYBC02);
        wrong_version.extend_from_slice(&2_u32.to_be_bytes());
        assert_eq!(
            check_image_prefix(&wrong_version),
            Err(ImageError::UnsupportedVersion),
            "future versions refuse without fallback"
        );
    }
}
