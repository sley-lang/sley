//! Private, signed manager observation for a reaped OOM worker.
//!
//! A reaped cgroup has no readable `memory.events`. This diagnostic records
//! only facts retained by systemd and never becomes a native test result.

use sley_scb1::{ScbError, ScbErrorCode, ScbValueCursor, encode_record, encode_uvar};
use sley_tests::SupervisorConfigV1;

use crate::config::sha256_bytes;
use crate::protocol::{RunRequest, RunResponse, RunStatus};
use crate::response::{MAX_RESPONSE_CONFIG_BYTES, MAX_RESPONSE_FRAME_BYTES};

const VERSION: u64 = 1;
const SIGNATURE_CONTEXT: &[u8] = b"sley2.native-manager-oom.v1";
const MAX_CLAIM_BYTES: usize = 512;
const MAX_EVIDENCE_BYTES: usize = MAX_RESPONSE_CONFIG_BYTES + MAX_CLAIM_BYTES + 64;

fn mismatch() -> ScbError {
    ScbError::new(ScbErrorCode::ContractUnknown)
}

fn read_uvar(bytes: &[u8], bits: u8) -> Result<u64, ScbError> {
    let mut cursor = ScbValueCursor::new(bytes)?;
    let value = cursor.read_uvar(bits)?;
    cursor.check_finished()?;
    Ok(value)
}

fn fixed<const N: usize>(bytes: &[u8]) -> Result<[u8; N], ScbError> {
    bytes
        .try_into()
        .map_err(|_| ScbError::new(ScbErrorCode::LengthOverflow))
}

/// A signed assertion of the exact manager OOM result, SIGKILL status,
/// installed cap, retained peak, and confirmed empty cgroup. The assertion is
/// valid only after [`crate::manager::verify_reaped_oom_unit`] succeeds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagerOomClaimV1 {
    /// SHA-256 of the exact canonical private request frame.
    pub request_sha256: [u8; 32],
    /// Kernel-authenticated caller UID.
    pub caller_uid: u32,
    /// Measurement signer public key.
    pub key_id: [u8; 32],
    /// Receiver-provisioned measurement trust policy.
    pub trust_policy_id: [u8; 32],
    /// Exact supervisor configuration identity.
    pub supervisor_config_id: [u8; 32],
    /// Typed manager `MemoryMax` for the failed worker.
    pub installed_memory_cap: u64,
    /// Typed manager `MemoryPeak` retained after OOM.
    pub memory_peak: u64,
    /// Typed manager `ExecMainPID` retained after OOM.
    pub exec_main_pid: u32,
    /// Monotonic launch-through-confirmed-reap duration.
    pub elapsed_ns: u64,
    /// Host historical trust time in Unix milliseconds.
    pub recorded_unix_millis: u64,
    /// Ed25519 signature over [`Self::signature_preimage`].
    pub signature: [u8; 64],
}

impl ManagerOomClaimV1 {
    fn fields(&self) -> Vec<(u32, Vec<u8>)> {
        vec![
            (1, encode_uvar(VERSION)),
            (2, self.request_sha256.to_vec()),
            (3, encode_uvar(u64::from(self.caller_uid))),
            (4, self.key_id.to_vec()),
            (5, self.trust_policy_id.to_vec()),
            (6, self.supervisor_config_id.to_vec()),
            (7, encode_uvar(self.installed_memory_cap)),
            (8, encode_uvar(self.memory_peak)),
            (9, encode_uvar(u64::from(self.exec_main_pid))),
            (10, encode_uvar(self.elapsed_ns)),
            (11, encode_uvar(self.recorded_unix_millis)),
            (12, self.signature.to_vec()),
        ]
    }

    fn validate(&self) -> Result<(), ScbError> {
        if self.request_sha256 == [0; 32]
            || self.key_id == [0; 32]
            || self.trust_policy_id == [0; 32]
            || self.supervisor_config_id == [0; 32]
            || self.installed_memory_cap == 0
            || self.memory_peak == 0
            || self.memory_peak > self.installed_memory_cap
            || self.exec_main_pid == 0
            || self.elapsed_ns == 0
            || self.recorded_unix_millis == 0
        {
            return Err(mismatch());
        }
        Ok(())
    }

    /// Canonical domain-separated bytes signed by the root supervisor.
    ///
    /// # Errors
    /// Refuses inconsistent claim fields or an oversized record.
    pub fn signature_preimage(&self) -> Result<Vec<u8>, ScbError> {
        self.validate()?;
        let fields = self.fields();
        let unsigned = encode_record(&fields[..11])?;
        let mut preimage = Vec::with_capacity(SIGNATURE_CONTEXT.len() + unsigned.len());
        preimage.extend_from_slice(SIGNATURE_CONTEXT);
        preimage.extend_from_slice(&unsigned);
        Ok(preimage)
    }

    /// Encodes the signed claim. Parsing and encoding grant no trust.
    ///
    /// # Errors
    /// Refuses inconsistent fields or an oversized record.
    pub fn encode_record(&self) -> Result<Vec<u8>, ScbError> {
        self.validate()?;
        let encoded = encode_record(&self.fields())?;
        if encoded.len() > MAX_CLAIM_BYTES {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        Ok(encoded)
    }

    /// Strictly parses a bounded canonical signed claim.
    ///
    /// # Errors
    /// Refuses malformed, noncanonical, or inconsistent fields.
    pub fn parse_record(bytes: &[u8]) -> Result<Self, ScbError> {
        if bytes.len() > MAX_CLAIM_BYTES {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        let mut cursor = ScbValueCursor::new(bytes)?;
        if cursor.read_record_field_count()? != 12 {
            return Err(ScbError::new(ScbErrorCode::FieldMissing));
        }
        let mut fields = Vec::with_capacity(12);
        for expected in 1..=12 {
            if cursor.read_uvar(32)? != expected {
                return Err(ScbError::new(ScbErrorCode::FieldOrder));
            }
            fields.push(cursor.read_sized_payload()?.to_vec());
        }
        cursor.check_finished()?;
        if read_uvar(&fields[0], 64)? != VERSION {
            return Err(ScbError::new(ScbErrorCode::VersionUnsupported));
        }
        let claim = Self {
            request_sha256: fixed(&fields[1])?,
            caller_uid: u32::try_from(read_uvar(&fields[2], 32)?)
                .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?,
            key_id: fixed(&fields[3])?,
            trust_policy_id: fixed(&fields[4])?,
            supervisor_config_id: fixed(&fields[5])?,
            installed_memory_cap: read_uvar(&fields[6], 64)?,
            memory_peak: read_uvar(&fields[7], 64)?,
            exec_main_pid: u32::try_from(read_uvar(&fields[8], 32)?)
                .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?,
            elapsed_ns: read_uvar(&fields[9], 64)?,
            recorded_unix_millis: read_uvar(&fields[10], 64)?,
            signature: fixed(&fields[11])?,
        };
        if claim.encode_record()? != bytes {
            return Err(mismatch());
        }
        Ok(claim)
    }
}

/// Private no-result OOM diagnostic and its exact enforced configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunManagerOomEvidence {
    claim: ManagerOomClaimV1,
    config: SupervisorConfigV1,
}

impl RunManagerOomEvidence {
    /// Joins the manager claim to the exact configuration it names.
    ///
    /// # Errors
    /// Refuses mismatched configuration or oversized bytes.
    pub fn build(claim: ManagerOomClaimV1, config: SupervisorConfigV1) -> Result<Self, ScbError> {
        claim.encode_record()?;
        if claim.supervisor_config_id != *config.id().as_bytes() {
            return Err(mismatch());
        }
        if config.stored_bytes().len() > MAX_RESPONSE_CONFIG_BYTES {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        Ok(Self { claim, config })
    }

    /// Parsed signed manager claim; signature trust is checked by the receiver.
    #[must_use]
    pub const fn claim(&self) -> &ManagerOomClaimV1 {
        &self.claim
    }

    /// Exact enforced supervisor configuration.
    #[must_use]
    pub const fn supervisor_config(&self) -> &SupervisorConfigV1 {
        &self.config
    }

    /// Encodes this bounded private evidence pair.
    ///
    /// # Errors
    /// Refuses oversized or inconsistent evidence.
    pub fn encode_record(&self) -> Result<Vec<u8>, ScbError> {
        let record = encode_record(&[
            (1, self.claim.encode_record()?),
            (2, self.config.stored_bytes().to_vec()),
        ])?;
        if record.len() > MAX_EVIDENCE_BYTES || record.len() > MAX_RESPONSE_FRAME_BYTES - 64 {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        Ok(record)
    }

    /// Strictly parses the bounded manager diagnostic and configuration.
    ///
    /// # Errors
    /// Refuses malformed or mismatched artifacts.
    pub fn parse_record(bytes: &[u8]) -> Result<Self, ScbError> {
        if bytes.len() > MAX_EVIDENCE_BYTES {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        let mut cursor = ScbValueCursor::new(bytes)?;
        if cursor.read_record_field_count()? != 2 || cursor.read_uvar(32)? != 1 {
            return Err(ScbError::new(ScbErrorCode::FieldMissing));
        }
        let claim = ManagerOomClaimV1::parse_record(cursor.read_sized_payload()?)?;
        if cursor.read_uvar(32)? != 2 {
            return Err(ScbError::new(ScbErrorCode::FieldOrder));
        }
        let config = SupervisorConfigV1::parse(cursor.read_sized_payload()?)?;
        cursor.check_finished()?;
        let evidence = Self::build(claim, config)?;
        if evidence.encode_record()? != bytes {
            return Err(mismatch());
        }
        Ok(evidence)
    }
}

impl RunRequest {
    /// Checks exact request, authenticated UID, configuration, and failure
    /// shape. This does not verify the manager claim's signature.
    ///
    /// # Errors
    /// Refuses cross-request substitution or inconsistent OOM facts.
    pub fn verified_manager_oom_evidence<'a>(
        &self,
        response: &'a RunResponse,
        caller_uid: u32,
    ) -> Result<&'a RunManagerOomEvidence, ScbError> {
        if response.status != RunStatus::Failed
            || response.code != crate::service::RUN_FAILURE_MANAGER_OOM
            || response.evidence.is_some()
            || response.no_result.is_some()
        {
            return Err(mismatch());
        }
        self.verified_program()?;
        let evidence = response.manager_oom.as_ref().ok_or_else(mismatch)?;
        let claim = evidence.claim();
        let request_bytes = self.encode_frame()?;
        let expected_cap =
            crate::response::expected_config_cap(self, evidence.supervisor_config())?;
        if claim.request_sha256 != sha256_bytes(&request_bytes)
            || claim.caller_uid != caller_uid
            || claim.installed_memory_cap != expected_cap
            || !evidence.supervisor_config().callers().iter().any(|entry| {
                entry.uid == caller_uid
                    && entry.workspace == self.workspace
                    && entry.principal == self.principal
            })
        {
            return Err(mismatch());
        }
        Ok(evidence)
    }
}
