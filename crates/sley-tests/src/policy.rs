//! Native resource policy (`SLEYNRP1`) from `NATIVE_TEST_ADMISSION_V1.md` section 2.
//!
//! The policy binds the protected policy root, authenticated principal,
//! capability summary, exact grant ceilings, effective validation limits,
//! implementation ceilings, aggregate maxima, the fixed wall cap, and the
//! admission-profile identity. Exact effective bytes enter the plan and the
//! approval; prose descriptions never substitute for them.

use sley_id::{CapabilitySummaryDigest, NativeResourcePolicyId, PolicyRootId, PrincipalId};
use sley_scb1::{ScbError, ScbErrorCode, encode_record, encode_uvar};
use sley_vm::native_execution::NativeImplementationLimits;

use crate::codec::{
    POLICY_MAGIC, RECORD_VERSION, decode_envelope, decode_fields, expect_tags, preimage_bytes,
    read_id, read_uvar_value,
};

/// Fixed supervisor wall ceiling in milliseconds; the record pins this value.
pub const NATIVE_WALL_CAP_MILLIS: u64 = 30_000;

/// Exact six-field principal grant record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrantCeilings {
    /// Maximum fuel the principal grant allows one test to declare.
    pub max_fuel: u64,
    /// Maximum memory bytes the principal grant allows one test to declare.
    pub max_memory_bytes: u64,
    /// Maximum output bytes the principal grant allows one test to declare.
    pub max_output_bytes: u64,
    /// Maximum effects the principal grant allows one test to declare.
    pub max_effect_count: u64,
    /// Maximum mutations in the granting capability scope.
    pub max_mutation_count: u64,
    /// Maximum adapter calls in the granting capability scope.
    pub max_adapter_calls: u64,
}

impl GrantCeilings {
    fn record(self) -> Result<Vec<u8>, ScbError> {
        integer_record(&[
            self.max_fuel,
            self.max_memory_bytes,
            self.max_output_bytes,
            self.max_effect_count,
            self.max_mutation_count,
            self.max_adapter_calls,
        ])
    }

    fn parse(value: &[u8]) -> Result<Self, ScbError> {
        let fields = decode_fields(value)?;
        expect_tags(&fields, &[1, 2, 3, 4, 5, 6])?;
        Ok(Self {
            max_fuel: read_uvar_value(&fields[0].1, 64)?,
            max_memory_bytes: read_uvar_value(&fields[1].1, 64)?,
            max_output_bytes: read_uvar_value(&fields[2].1, 64)?,
            max_effect_count: read_uvar_value(&fields[3].1, 64)?,
            max_mutation_count: read_uvar_value(&fields[4].1, 64)?,
            max_adapter_calls: read_uvar_value(&fields[5].1, 64)?,
        })
    }
}

/// Exact nine-field effective validation context record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidationLimits {
    /// Maximum candidate operations.
    pub max_operations: u32,
    /// Maximum candidate preconditions.
    pub max_preconditions: u32,
    /// Maximum candidate bytes.
    pub max_candidate_bytes: u64,
    /// Maximum decoded value bytes.
    pub max_decoded_value_bytes: u64,
    /// Maximum graph work.
    pub max_graph_work: u64,
    /// Maximum selected tests under static validation.
    pub max_selected_tests: u32,
    /// Maximum entities.
    pub max_entities: u32,
    /// Maximum test call depth under static validation.
    pub max_test_call_depth: u64,
    /// Maximum test wall timeout in milliseconds under static validation.
    pub max_test_wall_timeout_millis: u64,
}

impl ValidationLimits {
    fn record(self) -> Result<Vec<u8>, ScbError> {
        encode_record(&[
            (1, encode_uvar(u64::from(self.max_operations))),
            (2, encode_uvar(u64::from(self.max_preconditions))),
            (3, encode_uvar(self.max_candidate_bytes)),
            (4, encode_uvar(self.max_decoded_value_bytes)),
            (5, encode_uvar(self.max_graph_work)),
            (6, encode_uvar(u64::from(self.max_selected_tests))),
            (7, encode_uvar(u64::from(self.max_entities))),
            (8, encode_uvar(self.max_test_call_depth)),
            (9, encode_uvar(self.max_test_wall_timeout_millis)),
        ])
    }

    fn parse(value: &[u8]) -> Result<Self, ScbError> {
        let fields = decode_fields(value)?;
        expect_tags(&fields, &[1, 2, 3, 4, 5, 6, 7, 8, 9])?;
        let u32_field = |index: usize| {
            u32::try_from(read_uvar_value(&fields[index].1, 32)?)
                .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))
        };
        Ok(Self {
            max_operations: u32_field(0)?,
            max_preconditions: u32_field(1)?,
            max_candidate_bytes: read_uvar_value(&fields[2].1, 64)?,
            max_decoded_value_bytes: read_uvar_value(&fields[3].1, 64)?,
            max_graph_work: read_uvar_value(&fields[4].1, 64)?,
            max_selected_tests: u32_field(5)?,
            max_entities: u32_field(6)?,
            max_test_call_depth: read_uvar_value(&fields[7].1, 64)?,
            max_test_wall_timeout_millis: read_uvar_value(&fields[8].1, 64)?,
        })
    }
}

/// Exact eight-field native aggregate ceiling record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeAggregateLimits {
    /// Maximum selected tests per native commit.
    pub max_selected_tests: u32,
    /// Maximum summed declared fuel.
    pub max_fuel: u64,
    /// Maximum summed requested memory ceilings.
    pub max_memory_sum: u64,
    /// Maximum summed output ceilings.
    pub max_output_sum: u64,
    /// Maximum summed declared effects.
    pub max_effect_sum: u64,
    /// Maximum summed call-depth limits.
    pub max_depth_sum: u64,
    /// Maximum summed wall limits in milliseconds.
    pub max_wall_millis_sum: u64,
    /// Maximum aggregate evidence bytes.
    pub max_evidence_bytes: u64,
}

impl NativeAggregateLimits {
    /// Immutable native-v1 aggregate maxima; local values may only tighten.
    pub const HARD_MAXIMA: Self = Self {
        max_selected_tests: 256,
        max_fuel: 1_000_000_000,
        max_memory_sum: 17_179_869_184,
        max_output_sum: 16_777_216,
        max_effect_sum: 1_000_000,
        max_depth_sum: 65_536,
        max_wall_millis_sum: 30_000,
        max_evidence_bytes: 50_331_648,
    };

    /// Whether every field respects its native-v1 maximum. Zero is literal.
    #[must_use]
    pub const fn within_hard_maxima(self) -> bool {
        self.max_selected_tests <= Self::HARD_MAXIMA.max_selected_tests
            && self.max_fuel <= Self::HARD_MAXIMA.max_fuel
            && self.max_memory_sum <= Self::HARD_MAXIMA.max_memory_sum
            && self.max_output_sum <= Self::HARD_MAXIMA.max_output_sum
            && self.max_effect_sum <= Self::HARD_MAXIMA.max_effect_sum
            && self.max_depth_sum <= Self::HARD_MAXIMA.max_depth_sum
            && self.max_wall_millis_sum <= Self::HARD_MAXIMA.max_wall_millis_sum
            && self.max_evidence_bytes <= Self::HARD_MAXIMA.max_evidence_bytes
    }

    fn record(self) -> Result<Vec<u8>, ScbError> {
        encode_record(&[
            (1, encode_uvar(u64::from(self.max_selected_tests))),
            (2, encode_uvar(self.max_fuel)),
            (3, encode_uvar(self.max_memory_sum)),
            (4, encode_uvar(self.max_output_sum)),
            (5, encode_uvar(self.max_effect_sum)),
            (6, encode_uvar(self.max_depth_sum)),
            (7, encode_uvar(self.max_wall_millis_sum)),
            (8, encode_uvar(self.max_evidence_bytes)),
        ])
    }

    fn parse(value: &[u8]) -> Result<Self, ScbError> {
        let fields = decode_fields(value)?;
        expect_tags(&fields, &[1, 2, 3, 4, 5, 6, 7, 8])?;
        let parsed = Self {
            max_selected_tests: u32::try_from(read_uvar_value(&fields[0].1, 32)?)
                .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?,
            max_fuel: read_uvar_value(&fields[1].1, 64)?,
            max_memory_sum: read_uvar_value(&fields[2].1, 64)?,
            max_output_sum: read_uvar_value(&fields[3].1, 64)?,
            max_effect_sum: read_uvar_value(&fields[4].1, 64)?,
            max_depth_sum: read_uvar_value(&fields[5].1, 64)?,
            max_wall_millis_sum: read_uvar_value(&fields[6].1, 64)?,
            max_evidence_bytes: read_uvar_value(&fields[7].1, 64)?,
        };
        if parsed.within_hard_maxima() {
            Ok(parsed)
        } else {
            Err(ScbError::new(ScbErrorCode::ResourceLimit))
        }
    }
}

/// Caller-supplied policy facts; constructing these grants no authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeResourcePolicyParts {
    /// Protected policy root this projection was derived from.
    pub policy_root: PolicyRootId,
    /// Authenticated principal the ceilings were granted to.
    pub principal: PrincipalId,
    /// Canonical capability-summary digest of the granting context.
    pub capability_summary: CapabilitySummaryDigest,
    /// Exact principal grant ceilings.
    pub grant: GrantCeilings,
    /// Exact effective validation limits.
    pub validation: ValidationLimits,
    /// Exact implementation ceilings, within native hard maxima.
    pub implementation: NativeImplementationLimits,
    /// Exact aggregate ceilings, within native hard maxima.
    pub aggregate: NativeAggregateLimits,
    /// Opaque native admission-profile identity bound by later verifiers.
    pub admission_profile: [u8; 32],
}

/// Immutable canonical resource policy; external callers cannot forge one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeResourcePolicyV1 {
    parts: NativeResourcePolicyParts,
    id: NativeResourcePolicyId,
}

impl NativeResourcePolicyV1 {
    /// Builds a validated policy from caller-supplied facts.
    ///
    /// # Errors
    /// Returns `SCB_RESOURCE_LIMIT` when implementation or aggregate ceilings
    /// exceed their native-v1 hard maxima.
    pub fn build(parts: NativeResourcePolicyParts) -> Result<Self, ScbError> {
        if !parts.implementation.within_hard_maxima() || !parts.aggregate.within_hard_maxima() {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        let record = record_parts(&parts)?;
        let preimage = preimage_bytes(POLICY_MAGIC, &record)?;
        Ok(Self {
            parts,
            id: NativeResourcePolicyId::derive(&preimage),
        })
    }

    /// Strictly parses and validates one stored policy envelope.
    ///
    /// Parsing checks shape, ranges, and hard maxima. It does not authenticate
    /// the principal, capabilities, or policy root; those are N4/N5 duties.
    ///
    /// # Errors
    /// Returns the first stable SCB1 failure encountered while decoding the
    /// envelope, verifying its digest, or validating its fields.
    pub fn parse(stored: &[u8]) -> Result<Self, ScbError> {
        let (record, trailer) = decode_envelope(stored, POLICY_MAGIC)?;
        let preimage = preimage_bytes(POLICY_MAGIC, &record)?;
        if NativeResourcePolicyId::derive(&preimage).into_bytes() != trailer {
            return Err(ScbError::new(ScbErrorCode::DigestMismatch));
        }
        let fields = decode_fields(&record)?;
        expect_tags(&fields, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10])?;
        if read_uvar_value(&fields[0].1, 32)? != RECORD_VERSION {
            return Err(ScbError::new(ScbErrorCode::VersionUnsupported));
        }
        if read_uvar_value(&fields[8].1, 64)? != NATIVE_WALL_CAP_MILLIS {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        Self::build(NativeResourcePolicyParts {
            policy_root: PolicyRootId::from_bytes(read_id(&fields[1].1)?),
            principal: PrincipalId::from_bytes(read_id(&fields[2].1)?),
            capability_summary: CapabilitySummaryDigest::from_bytes(read_id(&fields[3].1)?),
            grant: GrantCeilings::parse(&fields[4].1)?,
            validation: ValidationLimits::parse(&fields[5].1)?,
            implementation: parse_implementation_limits(&fields[6].1)?,
            aggregate: NativeAggregateLimits::parse(&fields[7].1)?,
            admission_profile: read_id(&fields[9].1)?,
        })
    }

    /// Canonical SCB1 record without envelope or trailer.
    ///
    /// # Errors
    /// Returns `SCB_RESOURCE_LIMIT` when a nested value exceeds epoch limits.
    pub fn record_bytes(&self) -> Result<Vec<u8>, ScbError> {
        record_parts(&self.parts)
    }

    /// Complete canonical envelope and digest trailer.
    ///
    /// # Panics
    /// Panics only when a previously built policy no longer re-encodes, which
    /// cannot happen for validated parts.
    #[must_use]
    pub fn stored_bytes(&self) -> Vec<u8> {
        let record = self.record_bytes().expect("built policy re-encodes");
        let mut stored = preimage_bytes(POLICY_MAGIC, &record).expect("built policy fits");
        stored.extend_from_slice(self.id.as_bytes());
        stored
    }

    /// Resource policy identity over the exact envelope preimage.
    #[must_use]
    pub const fn policy_id(&self) -> NativeResourcePolicyId {
        self.id
    }

    /// Bound protected policy root.
    #[must_use]
    pub const fn policy_root(&self) -> PolicyRootId {
        self.parts.policy_root
    }

    /// Bound authenticated principal.
    #[must_use]
    pub const fn principal(&self) -> PrincipalId {
        self.parts.principal
    }

    /// Bound capability-summary digest.
    #[must_use]
    pub const fn capability_summary(&self) -> CapabilitySummaryDigest {
        self.parts.capability_summary
    }

    /// Bound grant ceilings.
    #[must_use]
    pub const fn grant(&self) -> GrantCeilings {
        self.parts.grant
    }

    /// Bound validation limits.
    #[must_use]
    pub const fn validation(&self) -> ValidationLimits {
        self.parts.validation
    }

    /// Bound implementation ceilings.
    #[must_use]
    pub const fn implementation(&self) -> NativeImplementationLimits {
        self.parts.implementation
    }

    /// Bound aggregate ceilings.
    #[must_use]
    pub const fn aggregate(&self) -> NativeAggregateLimits {
        self.parts.aggregate
    }

    /// Bound opaque admission-profile identity.
    #[must_use]
    pub const fn admission_profile(&self) -> [u8; 32] {
        self.parts.admission_profile
    }
}

fn record_parts(parts: &NativeResourcePolicyParts) -> Result<Vec<u8>, ScbError> {
    encode_record(&[
        (1, encode_uvar(RECORD_VERSION)),
        (2, parts.policy_root.as_bytes().to_vec()),
        (3, parts.principal.as_bytes().to_vec()),
        (4, parts.capability_summary.as_bytes().to_vec()),
        (5, parts.grant.record()?),
        (6, parts.validation.record()?),
        (7, implementation_record(parts.implementation)),
        (8, parts.aggregate.record()?),
        (9, encode_uvar(NATIVE_WALL_CAP_MILLIS)),
        (10, parts.admission_profile.to_vec()),
    ])
}

/// Native-v1 implementation-limit layout, field-for-field identical to the
/// VM owner's private record (`sley-vm` `records.rs`): instructions, live
/// value units, output value units, call depth, report bytes. The cross-crate
/// pin test there fails loudly on any reorder.
fn implementation_record(limits: NativeImplementationLimits) -> Vec<u8> {
    integer_record(&[
        limits.max_instructions,
        limits.max_value_units,
        limits.max_output_units,
        limits.max_call_depth,
        limits.max_report_bytes,
    ])
    .expect("five small fields always encode")
}

pub(crate) fn parse_implementation_limits(
    value: &[u8],
) -> Result<NativeImplementationLimits, ScbError> {
    let fields = decode_fields(value)?;
    expect_tags(&fields, &[1, 2, 3, 4, 5])?;
    let parsed = NativeImplementationLimits {
        max_instructions: read_uvar_value(&fields[0].1, 64)?,
        max_value_units: read_uvar_value(&fields[1].1, 64)?,
        max_output_units: read_uvar_value(&fields[2].1, 64)?,
        max_call_depth: read_uvar_value(&fields[3].1, 64)?,
        max_report_bytes: read_uvar_value(&fields[4].1, 64)?,
    };
    if parsed.within_hard_maxima() {
        Ok(parsed)
    } else {
        Err(ScbError::new(ScbErrorCode::ResourceLimit))
    }
}

fn integer_record(values: &[u64]) -> Result<Vec<u8>, ScbError> {
    let mut fields = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let tag =
            u32::try_from(index + 1).map_err(|_| ScbError::new(ScbErrorCode::ResourceLimit))?;
        fields.push((tag, encode_uvar(*value)));
    }
    encode_record(&fields)
}

#[cfg(test)]
pub(crate) fn sample_parts() -> NativeResourcePolicyParts {
    NativeResourcePolicyParts {
        policy_root: PolicyRootId::from_bytes([0x10; 32]),
        principal: PrincipalId::from_bytes([0x11; 32]),
        capability_summary: CapabilitySummaryDigest::from_bytes([0x12; 32]),
        grant: GrantCeilings {
            max_fuel: 1_000_000,
            max_memory_bytes: 1_073_741_824,
            max_output_bytes: 65_536,
            max_effect_count: 0,
            max_mutation_count: 100,
            max_adapter_calls: 0,
        },
        validation: ValidationLimits {
            max_operations: 1_000,
            max_preconditions: 64,
            max_candidate_bytes: 65_536,
            max_decoded_value_bytes: 65_536,
            max_graph_work: 100_000,
            max_selected_tests: 16,
            max_entities: 1_024,
            max_test_call_depth: 64,
            max_test_wall_timeout_millis: 5_000,
        },
        implementation: NativeImplementationLimits::HARD_MAXIMA,
        aggregate: NativeAggregateLimits {
            max_selected_tests: 16,
            max_fuel: 100_000_000,
            max_memory_sum: 8_589_934_592,
            max_output_sum: 1_048_576,
            max_effect_sum: 0,
            max_depth_sum: 1_024,
            max_wall_millis_sum: 10_000,
            max_evidence_bytes: 8_388_608,
        },
        admission_profile: [0x13; 32],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{decode_fields as raw_fields, preimage_bytes as raw_pre};

    const GOLDEN_RECORD: &str = "0a010101022010101010101010101010101010101010101010101010101010101010101010100320111111111111111111111111111111111111111111111111111111111111111104201212121212121212121212121212121212121212121212121212121212121212051b060103c0843d0205808080800403038080040401000501640601000625090102e807020140030380800404038080040503a08d060601100702800808014009028827071c05010480ade204020480808020030480808020040280020503808010082708010110020480c2d72f030580808080200403808040050100060280080702904e0804808080040903b0ea010a201313131313131313131313131313131313131313131313131313131313131313";
    const GOLDEN_PREIMAGE: &str = "534c45594e525031019c020a010101022010101010101010101010101010101010101010101010101010101010101010100320111111111111111111111111111111111111111111111111111111111111111104201212121212121212121212121212121212121212121212121212121212121212051b060103c0843d0205808080800403038080040401000501640601000625090102e807020140030380800404038080040503a08d060601100702800808014009028827071c05010480ade204020480808020030480808020040280020503808010082708010110020480c2d72f030580808080200403808040050100060280080702904e0804808080040903b0ea010a201313131313131313131313131313131313131313131313131313131313131313";
    const GOLDEN_ID: &str = "fc18cbb305ec5e94f332a6d0d19003611e486e2f5e8997176be246a7a3177ecd";

    fn decode_hex(hex: &str) -> Vec<u8> {
        let bytes = hex.as_bytes();
        let mut out = Vec::with_capacity(hex.len() / 2);
        for chunk in bytes.chunks_exact(2) {
            out.push(
                u8::from_str_radix(core::str::from_utf8(chunk).expect("hex ascii"), 16)
                    .expect("hex"),
            );
        }
        out
    }

    #[test]
    fn policy_matches_independent_python_golden() {
        let policy = NativeResourcePolicyV1::build(sample_parts()).expect("sample builds");
        assert_eq!(
            hex_of(&policy.record_bytes().expect("record")),
            GOLDEN_RECORD
        );
        let preimage = raw_pre(POLICY_MAGIC, &decode_hex(GOLDEN_RECORD)).expect("preimage");
        assert_eq!(hex_of(&preimage), GOLDEN_PREIMAGE);
        assert_eq!(hex_of(policy.policy_id().as_bytes()), GOLDEN_ID);
        let parsed = NativeResourcePolicyV1::parse(&policy.stored_bytes()).expect("roundtrip");
        assert_eq!(parsed, policy);
        assert_eq!(parsed.aggregate(), sample_parts().aggregate);
        assert_eq!(
            parsed.implementation(),
            NativeImplementationLimits::HARD_MAXIMA
        );
    }

    #[test]
    fn policy_rejects_ceiling_violations() {
        let mut over = sample_parts();
        over.aggregate = NativeAggregateLimits {
            max_fuel: NativeAggregateLimits::HARD_MAXIMA.max_fuel + 1,
            ..NativeAggregateLimits::HARD_MAXIMA
        };
        assert_eq!(
            NativeResourcePolicyV1::build(over)
                .expect_err("aggregate")
                .code(),
            ScbErrorCode::ResourceLimit
        );
        let mut over_impl = sample_parts();
        over_impl.implementation = NativeImplementationLimits {
            max_instructions: NativeImplementationLimits::HARD_MAXIMA.max_instructions + 1,
            ..NativeImplementationLimits::HARD_MAXIMA
        };
        assert_eq!(
            NativeResourcePolicyV1::build(over_impl)
                .expect_err("implementation")
                .code(),
            ScbErrorCode::ResourceLimit
        );
        assert!(NativeAggregateLimits::HARD_MAXIMA.within_hard_maxima());
    }

    #[test]
    fn policy_parse_rejects_tampered_wall_cap() {
        let record = decode_hex(GOLDEN_RECORD);
        let fields = raw_fields(&record).expect("golden fields");
        let mut tampered = fields;
        for (tag, value) in &mut tampered {
            if *tag == 9 {
                *value = encode_uvar(NATIVE_WALL_CAP_MILLIS + 1);
            }
        }
        let record = encode_record(&tampered).expect("tampered encodes");
        let preimage = raw_pre(POLICY_MAGIC, &record).expect("preimage");
        let id = NativeResourcePolicyId::derive(&preimage);
        let mut stored = preimage;
        stored.extend_from_slice(id.as_bytes());
        assert_eq!(
            NativeResourcePolicyV1::parse(&stored)
                .expect_err("wall cap")
                .code(),
            ScbErrorCode::ContractUnknown
        );
    }

    fn hex_of(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use core::fmt::Write as _;
            write!(out, "{byte:02x}").expect("hex formatting never fails");
        }
        out
    }
}
