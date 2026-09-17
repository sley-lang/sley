//! Measured test attestation (`SLEYMTA1`) from `NATIVE_TEST_EXECUTION_V1.md`
//! section 7.
//!
//! The attestation binds the plan, test object, execution report (or an
//! explicit no-result failure), declared and installed resource facts,
//! measured peak/events, termination, supervisor identities, nonce, caller,
//! recorded trust time, and the daemon's Ed25519 signature. Parsing checks
//! shape, ranges, and digest; it never verifies the signature or grants
//! measurement trust. Signature verification against a receiver-provisioned
//! trust manifest is the transaction owner's job (N5); the signing preimage
//! helper here exists so verifiers share one canonical construction.

use sley_id::{
    ExecutionReportId, MeasuredTestAttestationId, NativeTestPlanId, ObjectId, PrincipalId,
    WorkspaceId,
};
use sley_scb1::{ScbError, ScbErrorCode, ScbValueCursor, encode_record, encode_uvar};
use sley_vm::native_execution::NativeDeclaredLimits;

use crate::codec::{
    MAX_SELECTED_ENTRIES, RECORD_VERSION, decode_envelope, decode_fields, expect_tags,
    preimage_bytes, read_id, read_option_id, read_uvar_value,
};
use crate::plan::{declared_record, parse_declared_limits};

/// `SLEYMTA1` envelope magic for [`MeasuredTestAttestationV1`].
pub const MEASUREMENT_MAGIC: [u8; 8] = *b"SLEYMTA1";
/// Domain-separation context prefixed to the unsigned envelope preimage.
pub const MEASUREMENT_SIGNATURE_CONTEXT: &[u8] = b"sley2.native-test-measurement-signature.v1";
/// Successful complete run with full output and confirmed teardown.
pub const TERMINATION_COMPLETE: u32 = 1;
/// Refused before worker launch (zero budget, overflow, unavailable enforcer).
pub const TERMINATION_PRELAUNCH_REFUSED: u32 = 2;
/// Wall deadline expired; the whole unit was stopped.
pub const TERMINATION_TIMEOUT: u32 = 3;
/// Worker killed without producing a complete result.
pub const TERMINATION_KILLED: u32 = 4;
/// Worker crashed or exited unknown with malformed output.
pub const TERMINATION_CRASH: u32 = 5;
/// Enforcer or telemetry failure; never a pass.
pub const TERMINATION_ENFORCER_ERROR: u32 = 6;
/// Maximum attested bindings-adjacent lists are bounded by selection cap.
pub const MAX_ATTESTATION_LIST: u64 = MAX_SELECTED_ENTRIES;

/// Cgroup memory telemetry: installed ceiling facts and breach events.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryEvents {
    /// Installed page-floor memory cap the run was limited to.
    pub max: u64,
    /// Recorded limit-breach events; success requires zero.
    pub oom: u64,
    /// Recorded out-of-memory kills; success requires zero.
    pub oom_kill: u64,
}

impl MemoryEvents {
    fn record(self) -> Result<Vec<u8>, ScbError> {
        encode_record(&[
            (1, encode_uvar(self.max)),
            (2, encode_uvar(self.oom)),
            (3, encode_uvar(self.oom_kill)),
        ])
    }

    fn parse(value: &[u8]) -> Result<Self, ScbError> {
        let fields = decode_fields(value)?;
        expect_tags(&fields, &[1, 2, 3])?;
        Ok(Self {
            max: read_uvar_value(&fields[0].1, 64)?,
            oom: read_uvar_value(&fields[1].1, 64)?,
            oom_kill: read_uvar_value(&fields[2].1, 64)?,
        })
    }
}

/// Caller-supplied attestation facts; constructing these signs nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeasuredTestAttestationParts {
    /// Raw Ed25519 measurement public key identifying the daemon signer.
    pub key_id: [u8; 32],
    /// Receiver-provisioned trust manifest the key must belong to.
    ///
    /// The `SLEYNTR1` domain is not yet promoted; N1d registers it and
    /// tightens this to its typed identity.
    pub trust_policy_id: [u8; 32],
    /// Supervisor configuration the run was enforced under.
    ///
    /// The `SLEYNHC1` domain is not yet promoted; N1d registers it and
    /// tightens this to its typed identity.
    pub supervisor_config_id: [u8; 32],
    /// Plan the measured test was selected by.
    pub plan_id: NativeTestPlanId,
    /// Canonical test object that ran.
    pub test_object: ObjectId,
    /// Deterministic execution report, or `None` for an explicit no-result
    /// failure such as pre-launch refusal or timeout without output.
    pub execution_report_id: Option<ExecutionReportId>,
    /// Host-random attempt nonce distinguishing runs.
    pub attempt_nonce: [u8; 32],
    /// Workspace the run was bound to.
    pub workspace: WorkspaceId,
    /// Authenticated principal the run was bound to.
    pub principal: PrincipalId,
    /// Caller UID the supervisor authenticated for this run.
    pub caller_uid: u32,
    /// Exact declared limits the run was admitted under.
    pub declared_limits: NativeDeclaredLimits,
    /// Page-floored memory cap actually installed, no greater than declared.
    pub installed_memory_cap: u64,
    /// Daemon-measured elapsed nanoseconds, launch through confirmed exit.
    pub elapsed_ns: u64,
    /// Daemon-measured cgroup memory peak.
    pub measured_memory_peak: u64,
    /// Cgroup memory telemetry; success requires zero breach events.
    pub memory_events: MemoryEvents,
    /// Termination outcome, one of the `TERMINATION_*` constants.
    pub termination: u32,
    /// Whether complete worker output was collected.
    pub complete_output: bool,
    /// Whether an empty cgroup was confirmed after teardown.
    pub empty_cgroup_confirmed: bool,
    /// Supervisor-recorded historical trust time in Unix milliseconds.
    pub recorded_unix_millis: u64,
    /// Daemon Ed25519 signature over [`measurement_signature_preimage`].
    ///
    /// Length-checked here; strict curve verification belongs to N5, which
    /// owns the signature-verification dependency and trust manifest.
    pub signature: [u8; 64],
}

/// Immutable canonical measured attestation; callers cannot forge one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeasuredTestAttestationV1 {
    parts: MeasuredTestAttestationParts,
    record: Vec<u8>,
    stored: Vec<u8>,
    id: MeasuredTestAttestationId,
}

impl MeasuredTestAttestationV1 {
    /// Builds a validated attestation from caller-supplied facts.
    ///
    /// # Errors
    /// Returns `SCB_CONTRACT_UNKNOWN` for a termination outside 1..=6.
    pub fn build(parts: MeasuredTestAttestationParts) -> Result<Self, ScbError> {
        validate_parts(&parts)?;
        let record = record_parts(&parts)?;
        let preimage = preimage_bytes(MEASUREMENT_MAGIC, &record)?;
        let id = MeasuredTestAttestationId::derive(&preimage);
        let mut stored = preimage;
        stored.extend_from_slice(id.as_bytes());
        Ok(Self {
            parts,
            record,
            stored,
            id,
        })
    }

    /// Strictly parses and validates one stored attestation envelope.
    ///
    /// Parsing checks shape, ranges, and digest. It does not verify the
    /// signature, authenticate the key, or attest anything.
    ///
    /// # Errors
    /// Returns the first stable SCB1 failure encountered while decoding the
    /// envelope, verifying its digest, or validating fields.
    pub fn parse(stored: &[u8]) -> Result<Self, ScbError> {
        let (record, trailer) = decode_envelope(stored, MEASUREMENT_MAGIC)?;
        let preimage = preimage_bytes(MEASUREMENT_MAGIC, &record)?;
        if MeasuredTestAttestationId::derive(&preimage).into_bytes() != trailer {
            return Err(ScbError::new(ScbErrorCode::DigestMismatch));
        }
        let fields = decode_fields(&record)?;
        expect_tags(
            &fields,
            &[
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
            ],
        )?;
        if read_uvar_value(&fields[0].1, 32)? != RECORD_VERSION {
            return Err(ScbError::new(ScbErrorCode::VersionUnsupported));
        }
        let signature: [u8; 64] = fields[20]
            .1
            .as_slice()
            .try_into()
            .map_err(|_| ScbError::new(ScbErrorCode::LengthOverflow))?;
        Self::build(MeasuredTestAttestationParts {
            key_id: read_id(&fields[1].1)?,
            trust_policy_id: read_id(&fields[2].1)?,
            supervisor_config_id: read_id(&fields[3].1)?,
            plan_id: NativeTestPlanId::from_bytes(read_id(&fields[4].1)?),
            test_object: ObjectId::from_bytes(read_id(&fields[5].1)?),
            execution_report_id: read_option_id(&fields[6].1)?.map(ExecutionReportId::from_bytes),
            attempt_nonce: read_id(&fields[7].1)?,
            workspace: WorkspaceId::from_bytes(read_id(&fields[8].1)?),
            principal: PrincipalId::from_bytes(read_id(&fields[9].1)?),
            caller_uid: u32::try_from(read_uvar_value(&fields[10].1, 32)?)
                .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?,
            declared_limits: parse_declared_limits(&fields[11].1)?,
            installed_memory_cap: read_uvar_value(&fields[12].1, 64)?,
            elapsed_ns: read_uvar_value(&fields[13].1, 64)?,
            measured_memory_peak: read_uvar_value(&fields[14].1, 64)?,
            memory_events: MemoryEvents::parse(&fields[15].1)?,
            termination: u32::try_from(read_uvar_value(&fields[16].1, 32)?)
                .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?,
            complete_output: read_bool(&fields[17].1)?,
            empty_cgroup_confirmed: read_bool(&fields[18].1)?,
            recorded_unix_millis: read_uvar_value(&fields[19].1, 64)?,
            signature,
        })
    }

    /// Canonical SCB1 record without envelope or trailer.
    #[must_use]
    pub fn record_bytes(&self) -> &[u8] {
        &self.record
    }

    /// Complete stored envelope with digest trailer.
    #[must_use]
    pub fn stored_bytes(&self) -> &[u8] {
        &self.stored
    }

    /// Domain-separated attestation identity.
    #[must_use]
    pub const fn id(&self) -> MeasuredTestAttestationId {
        self.id
    }

    /// Whether this attestation claims a successful measured run.
    ///
    /// Success requires a present execution report, complete termination,
    /// complete output, confirmed empty cgroup, and zero memory events.
    /// Whether the VM outcome matches expectations is the comparison
    /// owner's separate judgment, not this shape predicate.
    #[must_use]
    pub fn claims_success(&self) -> bool {
        self.parts.execution_report_id.is_some()
            && self.parts.termination == TERMINATION_COMPLETE
            && self.parts.complete_output
            && self.parts.empty_cgroup_confirmed
            && self.parts.memory_events.oom == 0
            && self.parts.memory_events.oom_kill == 0
    }

    /// Test entity bound indirectly through the approval binding, for
    /// cross-checks that join attestations to plan selection.
    #[must_use]
    pub const fn test_object(&self) -> ObjectId {
        self.parts.test_object
    }

    /// Raw measurement public key identifying the daemon signer.
    ///
    /// The transaction owner checks this key against the receiver-provisioned
    /// measurement trust manifest; curve verification waits on vendored
    /// Ed25519 crypto.
    #[must_use]
    pub const fn key_id(&self) -> [u8; 32] {
        self.parts.key_id
    }

    /// Plan the measurement was taken under; the bundle requires agreement.
    #[must_use]
    pub const fn plan_id(&self) -> NativeTestPlanId {
        self.parts.plan_id
    }

    /// Receiver trust manifest the signature claims; raw until N1d types it.
    ///
    /// The bundle joins these bytes against parsed trust-policy identities.
    #[must_use]
    pub const fn trust_policy_id(&self) -> [u8; 32] {
        self.parts.trust_policy_id
    }

    /// Supervisor configuration the run claims; the bundle requires the
    /// exact referenced configuration bytes to be present.
    #[must_use]
    pub const fn supervisor_config_id(&self) -> [u8; 32] {
        self.parts.supervisor_config_id
    }

    /// Execution report the measurement covers, when the attempt produced one.
    #[must_use]
    pub const fn execution_report_id(&self) -> Option<ExecutionReportId> {
        self.parts.execution_report_id
    }

    /// Workspace the run was bound to; the owner requires agreement.
    #[must_use]
    pub const fn workspace(&self) -> WorkspaceId {
        self.parts.workspace
    }

    /// Authenticated principal the run was bound to; the owner requires
    /// agreement.
    #[must_use]
    pub const fn principal(&self) -> PrincipalId {
        self.parts.principal
    }

    /// Exact declared limits the run was admitted under; the owner requires
    /// agreement with the plan entry.
    #[must_use]
    pub const fn declared_limits(&self) -> NativeDeclaredLimits {
        self.parts.declared_limits
    }

    /// Page-floored memory cap actually installed; the diagnostic owner
    /// requires no greater than declared.
    #[must_use]
    pub const fn installed_memory_cap(&self) -> u64 {
        self.parts.installed_memory_cap
    }

    /// Daemon-measured elapsed nanoseconds, launch through confirmed exit.
    #[must_use]
    pub const fn elapsed_ns(&self) -> u64 {
        self.parts.elapsed_ns
    }

    /// Daemon-measured cgroup memory peak.
    #[must_use]
    pub const fn measured_memory_peak(&self) -> u64 {
        self.parts.measured_memory_peak
    }

    /// Cgroup memory telemetry; success requires zero breach events.
    #[must_use]
    pub const fn memory_events(&self) -> MemoryEvents {
        self.parts.memory_events
    }

    /// Supervisor-recorded historical trust time; the owner evaluates the
    /// measurement grant interval at this time, not the present clock.
    #[must_use]
    pub const fn recorded_unix_millis(&self) -> u64 {
        self.parts.recorded_unix_millis
    }

    /// Validated attestation facts used by signature verifiers.
    #[must_use]
    pub const fn parts(&self) -> &MeasuredTestAttestationParts {
        &self.parts
    }
}

/// Canonical signature preimage for one unsigned attestation record.
///
/// Signs exact ASCII `sley2.native-test-measurement-signature.v1` followed
/// by `P(SLEYMTA1, fields 1..20)`: the caller supplies the canonical record
/// prefix (fields 1..20 encoded as a record), and this helper frames the
/// shared construction so signers and N5 verifiers cannot diverge.
///
/// # Errors
/// Returns `SCB_RESOURCE_LIMIT` when the prefix exceeds the stored bound,
/// which cannot happen for a parsed attestation.
pub fn measurement_signature_preimage(unsigned_record_prefix: &[u8]) -> Result<Vec<u8>, ScbError> {
    let envelope = preimage_bytes(MEASUREMENT_MAGIC, unsigned_record_prefix)?;
    let mut out = Vec::with_capacity(MEASUREMENT_SIGNATURE_CONTEXT.len() + envelope.len());
    out.extend_from_slice(MEASUREMENT_SIGNATURE_CONTEXT);
    out.extend_from_slice(&envelope);
    Ok(out)
}

fn validate_parts(parts: &MeasuredTestAttestationParts) -> Result<(), ScbError> {
    if !(TERMINATION_COMPLETE..=TERMINATION_ENFORCER_ERROR).contains(&parts.termination) {
        return Err(ScbError::new(ScbErrorCode::ContractUnknown));
    }
    Ok(())
}

fn record_parts(parts: &MeasuredTestAttestationParts) -> Result<Vec<u8>, ScbError> {
    encode_record(&[
        (1, sley_scb1::encode_uvar(RECORD_VERSION)),
        (2, parts.key_id.to_vec()),
        (3, parts.trust_policy_id.to_vec()),
        (4, parts.supervisor_config_id.to_vec()),
        (5, parts.plan_id.as_bytes().to_vec()),
        (6, parts.test_object.as_bytes().to_vec()),
        (
            7,
            crate::codec::encode_option_id(parts.execution_report_id.map(|id| *id.as_bytes()))?,
        ),
        (8, parts.attempt_nonce.to_vec()),
        (9, parts.workspace.as_bytes().to_vec()),
        (10, parts.principal.as_bytes().to_vec()),
        (11, sley_scb1::encode_uvar(u64::from(parts.caller_uid))),
        (12, declared_record(parts.declared_limits)),
        (13, sley_scb1::encode_uvar(parts.installed_memory_cap)),
        (14, sley_scb1::encode_uvar(parts.elapsed_ns)),
        (15, sley_scb1::encode_uvar(parts.measured_memory_peak)),
        (16, parts.memory_events.record()?),
        (17, sley_scb1::encode_uvar(u64::from(parts.termination))),
        (18, sley_scb1::encode_bool(parts.complete_output)),
        (19, sley_scb1::encode_bool(parts.empty_cgroup_confirmed)),
        (20, sley_scb1::encode_uvar(parts.recorded_unix_millis)),
        (21, parts.signature.to_vec()),
    ])
}

/// Canonical unsigned record prefix: fields 1..20 without the signature.
///
/// Signers encode the attestation facts, strip the signature field through
/// this helper, frame the [`measurement_signature_preimage`], and sign that.
/// Verifiers (N5) repeat the construction over parsed facts, so neither side
/// can sign or check a different byte layout.
///
/// # Errors
/// Returns the stable SCB1 failure if re-encoding the parsed facts fails,
/// which cannot happen for a validated attestation.
pub fn unsigned_record_prefix(parts: &MeasuredTestAttestationParts) -> Result<Vec<u8>, ScbError> {
    let full = record_parts(parts)?;
    let fields = decode_fields(&full)?;
    debug_assert_eq!(fields.len(), 21);
    encode_record(
        &fields[..20]
            .iter()
            .map(|(tag, value)| (*tag, value.clone()))
            .collect::<Vec<_>>(),
    )
}

fn read_bool(value: &[u8]) -> Result<bool, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let parsed = cursor.read_bool()?;
    cursor.check_finished()?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sley_scb1::encode_union;

    fn decode_hex(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex decodes"))
            .collect()
    }

    fn golden(field: &str) -> Vec<u8> {
        let text = include_str!(
            "/home/gfarch/Work/checkpoints/sley2-finish-20260915/native-approval-golden.json"
        );
        let marker = format!("\"{field}\": \"");
        let start = text.find(&marker).expect("golden field present") + marker.len();
        let end = text[start..].find('"').expect("golden field ends") + start;
        decode_hex(&text[start..end])
    }

    #[test]
    fn golden_attestation_parses_with_exact_id_and_success_claim() {
        let parsed =
            MeasuredTestAttestationV1::parse(&golden("attestation_stored")).expect("golden parses");
        assert_eq!(parsed.record_bytes(), golden("attestation_record"));
        assert_eq!(parsed.stored_bytes(), golden("attestation_stored"));
        assert_eq!(parsed.id().as_bytes().to_vec(), golden("attestation_id"));
        assert_eq!(parsed.parts.termination, TERMINATION_COMPLETE);
        assert!(parsed.claims_success());
        assert_eq!(
            parsed.parts.memory_events,
            MemoryEvents {
                max: 4096,
                oom: 0,
                oom_kill: 0,
            }
        );
        assert_eq!(parsed.parts.caller_uid, 1000);
        assert_eq!(parsed.parts.signature, [0x41; 64]);
    }

    #[test]
    fn signature_preimage_is_context_plus_unsigned_envelope() {
        use sley_scb1::encode_uvar as uvar;
        let parsed =
            MeasuredTestAttestationV1::parse(&golden("attestation_stored")).expect("parses");
        let prefix = unsigned_record_prefix(&parsed.parts).expect("unsigned prefix encodes");
        let preimage = measurement_signature_preimage(&prefix).expect("preimage encodes");
        assert!(preimage.starts_with(MEASUREMENT_SIGNATURE_CONTEXT));
        let envelope = &preimage[MEASUREMENT_SIGNATURE_CONTEXT.len()..];
        assert_eq!(&envelope[..8], b"SLEYMTA1");
        assert_eq!(&envelope[8..9], uvar(1));
        let rest = &envelope[9..];
        let mut cursor = ScbValueCursor::new(rest).expect("cursor builds");
        let framed = cursor.read_bytes().expect("prefix framed");
        cursor.check_finished().expect("no trailing bytes");
        assert_eq!(framed, prefix);
        let prefix_fields = decode_fields(&prefix).expect("prefix is a record");
        assert_eq!(prefix_fields.len(), 20);
        for (index, (tag, _)) in prefix_fields.iter().enumerate() {
            assert_eq!(*tag, u32::try_from(index + 1).expect("small tag"));
        }
    }

    #[test]
    fn success_claim_requires_every_signal() {
        let base = MeasuredTestAttestationV1::parse(&golden("attestation_stored")).expect("parses");
        let mut no_report = base.parts.clone();
        no_report.execution_report_id = None;
        assert!(
            !MeasuredTestAttestationV1::build(no_report)
                .expect("builds")
                .claims_success()
        );
        for mutate in [
            |parts: &mut MeasuredTestAttestationParts| {
                parts.termination = TERMINATION_TIMEOUT;
            },
            |parts: &mut MeasuredTestAttestationParts| {
                parts.complete_output = false;
            },
            |parts: &mut MeasuredTestAttestationParts| {
                parts.empty_cgroup_confirmed = false;
            },
            |parts: &mut MeasuredTestAttestationParts| {
                parts.memory_events.oom = 1;
            },
            |parts: &mut MeasuredTestAttestationParts| {
                parts.memory_events.oom_kill = 1;
            },
        ] {
            let mut parts = base.parts.clone();
            mutate(&mut parts);
            assert!(
                !MeasuredTestAttestationV1::build(parts)
                    .expect("builds")
                    .claims_success()
            );
        }
        for termination in TERMINATION_COMPLETE..=TERMINATION_ENFORCER_ERROR {
            let mut parts = base.parts.clone();
            parts.termination = termination;
            MeasuredTestAttestationV1::build(parts).expect("every termination builds");
        }
        let mut bad = base.parts.clone();
        bad.termination = 7;
        assert_eq!(
            MeasuredTestAttestationV1::build(bad)
                .expect_err("bad termination")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut zero = base.parts.clone();
        zero.termination = 0;
        assert_eq!(
            MeasuredTestAttestationV1::build(zero)
                .expect_err("zero termination")
                .code(),
            ScbErrorCode::ContractUnknown
        );
    }

    #[test]
    fn attestation_refusals_keep_stable_codes() {
        let stored = golden("attestation_stored");
        assert_eq!(
            MeasuredTestAttestationV1::parse(&stored[..stored.len() - 1])
                .expect_err("truncated")
                .code(),
            ScbErrorCode::LengthOverflow
        );
        let mut tampered = stored.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert_eq!(
            MeasuredTestAttestationV1::parse(&tampered)
                .expect_err("digest")
                .code(),
            ScbErrorCode::DigestMismatch
        );
        assert_eq!(
            read_bool(&[0x02]).expect_err("bad bool").code(),
            ScbErrorCode::BoolInvalid
        );
        assert_eq!(
            MeasuredTestAttestationV1::parse(&encode_union(0, &[]).expect("union encodes"))
                .expect_err("short envelope")
                .code(),
            ScbErrorCode::LengthOverflow
        );
    }
}
