//! Native commit boundary: attempt journal, bounded admission, acceptance
//! signing adapter, and test-execution dispatch (N5b).
//!
//! The journal binds a client-generated 128-bit attempt to its workspace,
//! principal, candidate, and expected parent outside the semantic roots. The
//! six journal states are hints until reconciled with actual receipt and
//! head bytes: recovery never promotes an orphan receipt to finish an
//! attempt, and the operator-facing trichotomy is `RetrySafeRefusal`,
//! `Committed`, or `OutcomeUnknown`.
//!
//! Journal integrity uses a plain BLAKE3 checksum over the magic-prefixed
//! record. The checksum is corruption detection, not an identity: no new
//! digest domain is minted here and no identifier derives from it.
//!
//! Signature handling is cryptographic at every accepting boundary. The
//! acceptance signer adapter produces the 64-byte statement signature over the shared
//! [`admission_signature_preimage`](sley_tests::statement::admission_signature_preimage),
//! and the commit and exchange paths strictly verify RFC 8032 canonical
//! signatures, key-ID binding, signer role, workspace and
//! profile scope, and the historical validity interval against the
//! receiver-provisioned trust manifests. Measurement attestations
//! are checked the same way; the qualified supervisor that
//! produces real measurements is a separate, privileged component (N3).
//!
//! Test doubles are test-only: production commits require a configured
//! supervisor-backed executor, and [`commit_needs_executor`] refuses without
//! one before any worker or accepted-state write.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use ed25519_dalek::{Signature, Signer as _, SigningKey, VerifyingKey};

use sley_id::{
    CandidateId, NativeAdmissionProfileId, ObjectId, PrincipalId, ReceiptId, StateRoot,
    TransactionId, WorkspaceId,
};
use sley_mutate::import_entity_object;
use sley_policy::ValidatedCandidatePlan;
use sley_scb1::{ScbError, ScbErrorCode};
use sley_state_root::AcceptedStateRoot;
use sley_test_runner::{
    enforce::{check_elapsed, check_memory_evidence, floor_page_cap, runtime_max_usec},
    program::PortableTestProgram,
    protocol::{RunRequest, RunResponse, RunStatus},
};
use sley_tests::plan::{SELECTION_MODE_CANDIDATE_AFFECTED, SELECTION_MODE_EXPLICIT_ROOT};
use sley_tests::{
    HistoricalTrustPolicyV1, MeasuredTestAttestationV1, NATIVE_WALL_CAP_MILLIS,
    NativeExecutionEvidence, NativeExecutionReportV1, NativeTestApprovalV1, NativeTestPlanV1,
    NativeTestReportV1, ROLE_ACCEPTANCE, ROLE_MEASUREMENT, SelectedEntry, SupervisorConfigV1,
};

use crate::codec::TransactionErrorCode;

/// Exact attempt-journal record magic.
pub const JOURNAL_MAGIC: [u8; 8] = *b"SLEYNAT1";
/// Exact journal record version.
pub const JOURNAL_VERSION: u64 = 1;
/// Journal directory under the repository root, outside semantic roots.
pub const ATTEMPTS_DIR: &str = "attempts";
/// Settled records stay queryable but do not enter the recovery scan.
pub const SETTLED_ATTEMPTS_DIR: &str = "settled";
/// Journal filename suffix.
pub const ATTEMPT_SUFFIX: &str = ".attempt";
/// Staging filename prefix for atomic journal writes.
pub const ATTEMPT_STAGE_PREFIX: &str = ".sley-attempt-stage-";
/// Maximum journal record bytes charged before parsing.
pub const MAX_JOURNAL_BYTES: u64 = 1_024;
/// Total declared worker wall budgets admitted per native commit.
pub const MAX_COMMIT_WALL_MILLIS: u64 = NATIVE_WALL_CAP_MILLIS;

/// Client-generated 128-bit native commit attempt identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeAttemptId(pub [u8; 16]);

impl NativeAttemptId {
    /// Returns the exact attempt bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Renders the attempt identity as lowercase hex for filenames.
    #[must_use]
    pub fn hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(32);
        for byte in self.0 {
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
        out
    }
}

/// Attempt journal state, tags 1..=6 per `NATIVE_TEST_ADMISSION_V1.md` §6.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptState {
    /// Admission checks passed; no worker started.
    Admitted,
    /// At least one worker started; nothing promoted.
    Running,
    /// Refused or failed before receipt promotion; safe to retry.
    AbortedBeforePromotion,
    /// Receipt persisted; head CAS not yet confirmed.
    PromotionStarted,
    /// Head CAS confirmed for the recorded transaction and receipt.
    Committed,
    /// Promotion may or may not have completed; query history, never resubmit
    /// blindly.
    OutcomeUnknown,
}

impl AttemptState {
    /// Frozen journal wire tag.
    #[must_use]
    pub const fn tag(self) -> u64 {
        match self {
            Self::Admitted => 1,
            Self::Running => 2,
            Self::AbortedBeforePromotion => 3,
            Self::PromotionStarted => 4,
            Self::Committed => 5,
            Self::OutcomeUnknown => 6,
        }
    }

    /// Resolves one exact journal tag.
    #[must_use]
    pub const fn from_tag(tag: u64) -> Option<Self> {
        match tag {
            1 => Some(Self::Admitted),
            2 => Some(Self::Running),
            3 => Some(Self::AbortedBeforePromotion),
            4 => Some(Self::PromotionStarted),
            5 => Some(Self::Committed),
            6 => Some(Self::OutcomeUnknown),
            _ => None,
        }
    }
}

/// Durable attempt binding with its current journal state.
///
/// The committed transaction and receipt identities are recorded in the
/// `Committed` state; `PromotionStarted` records the same identities as an
/// unconfirmed promotion claim so recovery reconciles instead of guessing.
/// Every other state carries no promotion claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttemptRecord {
    /// Client-generated attempt identity.
    pub attempt_id: NativeAttemptId,
    /// Workspace the attempt was admitted in.
    pub workspace: WorkspaceId,
    /// Principal the attempt was admitted for.
    pub principal: PrincipalId,
    /// Candidate the attempt binds.
    pub candidate_id: CandidateId,
    /// Accepted parent the attempt builds on.
    pub expected_parent: TransactionId,
    /// Current journal state.
    pub state: AttemptState,
    /// Promoted transaction, recorded only when `Committed`.
    pub transaction_id: Option<TransactionId>,
    /// Promoted receipt, recorded only when `Committed`.
    pub receipt_id: Option<ReceiptId>,
}

impl AttemptRecord {
    /// Encodes the canonical journal record with its checksum trailer.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut record = Vec::with_capacity(8 + 8 + 8 + 32 * 6 + 2);
        record.extend_from_slice(&JOURNAL_MAGIC);
        record.extend_from_slice(&JOURNAL_VERSION.to_be_bytes());
        record.extend_from_slice(&self.state.tag().to_be_bytes());
        record.extend_from_slice(&self.attempt_id.0);
        record.extend_from_slice(self.workspace.as_bytes());
        record.extend_from_slice(self.principal.as_bytes());
        record.extend_from_slice(self.candidate_id.as_bytes());
        record.extend_from_slice(self.expected_parent.as_bytes());
        record.push(u8::from(self.transaction_id.is_some()));
        if let Some(transaction_id) = self.transaction_id {
            record.extend_from_slice(transaction_id.as_bytes());
        }
        record.push(u8::from(self.receipt_id.is_some()));
        if let Some(receipt_id) = self.receipt_id {
            record.extend_from_slice(receipt_id.as_bytes());
        }
        let checksum = blake3::hash(&record);
        record.extend_from_slice(checksum.as_bytes());
        record
    }

    /// Strictly parses and checksum-verifies one journal record.
    ///
    /// # Errors
    ///
    /// Returns `JournalCorrupt` for any shape, version, state, option-tag, or
    /// checksum failure.
    pub fn parse(input: &[u8]) -> Result<Self, NativeCommitError> {
        let corrupt = NativeCommitError::JournalCorrupt;
        if input.len() < 8 + 8 + 8 + 16 + 32 * 4 + 2 + 32
            || input.len() > 8 + 8 + 8 + 16 + 32 * 6 + 2 + 32
        {
            return Err(corrupt);
        }
        let (body, checksum) = input.split_at(input.len() - 32);
        if blake3::hash(body).as_bytes() != checksum {
            return Err(corrupt);
        }
        if body[..8] != JOURNAL_MAGIC {
            return Err(corrupt);
        }
        if u64::from_be_bytes(body[8..16].try_into().map_err(|_| corrupt)?) != JOURNAL_VERSION {
            return Err(corrupt);
        }
        let state = AttemptState::from_tag(u64::from_be_bytes(
            body[16..24].try_into().map_err(|_| corrupt)?,
        ))
        .ok_or(corrupt)?;
        let mut offset = 24_usize;
        let attempt_id = NativeAttemptId(take_bytes(body, &mut offset)?);
        let workspace = WorkspaceId::from_bytes(take_bytes(body, &mut offset)?);
        let principal = PrincipalId::from_bytes(take_bytes(body, &mut offset)?);
        let candidate_id = CandidateId::from_bytes(take_bytes(body, &mut offset)?);
        let expected_parent = TransactionId::from_bytes(take_bytes(body, &mut offset)?);
        let transaction_flag = *body.get(offset).ok_or(corrupt)?;
        offset += 1;
        let transaction_id = match transaction_flag {
            0 => None,
            1 => {
                let id = TransactionId::from_bytes(take_bytes(body, &mut offset)?);
                Some(id)
            }
            _ => return Err(corrupt),
        };
        let receipt_flag = *body.get(offset).ok_or(corrupt)?;
        offset += 1;
        let receipt_id = match receipt_flag {
            0 => None,
            1 => {
                let id = ReceiptId::from_bytes(take_bytes(body, &mut offset)?);
                Some(id)
            }
            _ => return Err(corrupt),
        };
        if offset != body.len() {
            return Err(corrupt);
        }
        match state {
            AttemptState::Committed => {
                if transaction_id.is_none() || receipt_id.is_none() {
                    return Err(corrupt);
                }
            }
            // PromotionStarted records the promotion claim so recovery can
            // reconcile it against actual receipt and head bytes; every
            // other pre-commit state carries no promotion claim.
            AttemptState::PromotionStarted => {}
            _ => {
                if transaction_id.is_some() || receipt_id.is_some() {
                    return Err(corrupt);
                }
            }
        }
        Ok(Self {
            attempt_id,
            workspace,
            principal,
            candidate_id,
            expected_parent,
            state,
            transaction_id,
            receipt_id,
        })
    }

    /// Returns whether two records bind the same attempt facts.
    ///
    /// State and promotion claims are excluded: a duplicate attempt ID with
    /// different bindings refuses, while the same bindings re-admit
    /// idempotently so retry-safe resubmission reconciles instead of forking.
    #[must_use]
    pub fn same_bindings(&self, other: &Self) -> bool {
        self.attempt_id == other.attempt_id
            && self.workspace == other.workspace
            && self.principal == other.principal
            && self.candidate_id == other.candidate_id
            && self.expected_parent == other.expected_parent
    }
}

/// Typed native commit-boundary failure with a stable operator symbol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeCommitError {
    /// No test executor is configured; refusal precedes any worker or write.
    ExecutorUnavailable,
    /// Bounded lock acquisition expired; no worker started, nothing written.
    BusyRetrySafe,
    /// Checkpoint expiry or disconnect before promotion; safe to retry after
    /// reconciling history.
    AbortedRetrySafe,
    /// Failure during or after promotion, or lost preservation error after
    /// launch; query history, never resubmit blindly.
    OutcomeUnknown,
    /// Duplicate attempt ID with different bindings.
    AttemptConflict,
    /// Journal record is missing where required, or fails shape/checksum.
    JournalCorrupt,
    /// No trust manifest covers the signer at all.
    TrustUnavailable,
    /// A known key lacks the role, scope, profile, or interval for this use.
    TrustRejected,
    /// Protected native plan derivation refused with its exact profile-local
    /// error; the static failure is preserved, never reinterpreted.
    Plan(sley_policy::NativePlanErrorV1),
}

impl NativeCommitError {
    /// Frozen operator-facing symbol.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::ExecutorUnavailable => "NATIVE_EXECUTOR_UNAVAILABLE",
            Self::BusyRetrySafe => "NATIVE_COMMIT_BUSY_RETRY_SAFE",
            Self::AbortedRetrySafe => "NATIVE_COMMIT_ABORTED_RETRY_SAFE",
            Self::OutcomeUnknown => "NATIVE_COMMIT_OUTCOME_UNKNOWN",
            Self::AttemptConflict => "NATIVE_ATTEMPT_CONFLICT",
            Self::JournalCorrupt => "NATIVE_JOURNAL_CORRUPT",
            Self::TrustUnavailable => "HISTORICAL_TRUST_UNAVAILABLE",
            Self::TrustRejected => "HISTORICAL_TRUST_REJECTED",
            Self::Plan(error) => error.symbol(),
        }
    }
}

impl core::fmt::Display for NativeCommitError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.symbol())
    }
}

impl std::error::Error for NativeCommitError {}

/// Acceptance-signer adapter: the configured transaction authority.
///
/// Only the commit owner invokes this, after local authorization and native
/// verification. The adapter signs the shared admission preimage with the
/// distinct acceptance key; worker data and candidate text can never invoke
/// arbitrary signing because the preimage construction is fixed here.
pub trait NativeAcceptanceSigner {
    /// Returns the raw acceptance public key claiming statements.
    fn key_id(&self) -> [u8; 32];
    /// Signs one admission preimage; returns the exact 64 signature bytes.
    fn sign(&self, preimage: &[u8]) -> [u8; 64];
}

/// In-process Ed25519 acceptance signer backed by an exact 32-byte secret.
///
/// The underlying signing key zeroizes its secret material on drop. Production
/// provisioning is responsible for reading the root-owned key file and may
/// construct this signer only after that read succeeds.
pub struct Ed25519AcceptanceSigner {
    key: SigningKey,
}

impl Ed25519AcceptanceSigner {
    /// Constructs a signer from one RFC 8032 32-byte secret seed.
    #[must_use]
    pub fn from_secret_bytes(secret: [u8; 32]) -> Self {
        Self {
            key: SigningKey::from_bytes(&secret),
        }
    }
}

impl NativeAcceptanceSigner for Ed25519AcceptanceSigner {
    fn key_id(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }

    fn sign(&self, preimage: &[u8]) -> [u8; 64] {
        self.key.sign(preimage).to_bytes()
    }
}

fn verify_ed25519_signature(
    key: &[u8; 32],
    preimage: &[u8],
    signature: &[u8; 64],
) -> Result<(), NativeCommitError> {
    let key = VerifyingKey::from_bytes(key).map_err(|_| NativeCommitError::TrustRejected)?;
    let signature = Signature::from_bytes(signature);
    key.verify_strict(preimage, &signature)
        .map_err(|_| NativeCommitError::TrustRejected)
}

/// One executed native test with its deterministic and measured evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutedNativeTest {
    /// Selected test entity, in plan order.
    pub test_entity: sley_id::EntityId,
    /// Complete native execution-report envelope with trailer.
    pub execution_stored: Vec<u8>,
    /// Complete measured-attestation envelope with trailer.
    pub attestation_stored: Vec<u8>,
    /// Complete supervisor-configuration envelope the run was enforced
    /// under; the bundle embeds every referenced configuration.
    pub supervisor_config_stored: Vec<u8>,
}

/// Failure while turning one private supervisor response into owner evidence.
/// A valid refusal without signed evidence remains an execution refusal,
/// never a synthetic test result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorEvidenceError {
    /// The response frame or its exact request binding is invalid.
    InvalidResponse(ScbErrorCode),
    /// The supervisor returned a valid refusal without signed diagnostics.
    NoEvidence {
        /// Refused or failed supervisor status.
        status: RunStatus,
        /// Stable supervisor refusal or failure code.
        code: u32,
    },
    /// The signed measurement lacks receiver trust or has an invalid signature.
    MeasurementTrust(NativeCommitError),
}

/// Verifies the private response frame and produces the exact stored evidence
/// that the native commit owner can independently check again.
///
/// This does not make the supervisor operational or trust a local test double.
/// The production executor must obtain these bytes from the authenticated
/// root service, and the commit owner still checks plan coverage and admission.
///
/// # Errors
///
/// Refuses malformed or unbound frames, missing evidence, inconsistent
/// diagnostic status, or untrusted measurement signatures.
pub fn verified_supervisor_execution(
    request: &RunRequest,
    response_frame: &[u8],
    caller_uid: u32,
    measurement_trust: &HistoricalTrustPolicyV1,
) -> Result<ExecutedNativeTest, SupervisorEvidenceError> {
    let response = RunResponse::decode_frame(response_frame)
        .map_err(|error| SupervisorEvidenceError::InvalidResponse(error.code()))?;
    if response.evidence.is_none() {
        return Err(SupervisorEvidenceError::NoEvidence {
            status: response.status,
            code: response.code,
        });
    }
    let evidence = request
        .verified_response_evidence(&response, caller_uid)
        .map_err(|error| SupervisorEvidenceError::InvalidResponse(error.code()))?;
    let status_matches_report = match (response.status, evidence.report().evidence()) {
        // A worker-owned Rejected report is also complete output. It remains
        // a test rejection when the owner compares the report, even though
        // the host measurement can truthfully claim a complete run.
        (RunStatus::Complete, _) => true,
        (
            RunStatus::Refused | RunStatus::Failed,
            sley_tests::NativeExecutionEvidence::Rejected(_),
        ) => !evidence.attestation().claims_success(),
        _ => false,
    };
    if !status_matches_report {
        return Err(SupervisorEvidenceError::InvalidResponse(
            ScbErrorCode::ContractUnknown,
        ));
    }
    verify_measurement_attestation(evidence.attestation(), request.workspace, measurement_trust)
        .map_err(SupervisorEvidenceError::MeasurementTrust)?;
    Ok(ExecutedNativeTest {
        test_entity: request.test_entity,
        execution_stored: evidence.report().stored_bytes().to_vec(),
        attestation_stored: evidence.attestation().stored_bytes().to_vec(),
        supervisor_config_stored: evidence.supervisor_config().stored_bytes().to_vec(),
    })
}

/// Builds the selected candidate test's bounded supervisor request from the
/// validator-owned proposed state and the protected native plan.
///
/// The complete source inventory and root come from validation. The only
/// caller-provided run facts are the selected test identity, a wall budget no
/// greater than its literal `TestCase` limit, and a fresh host attempt nonce.
/// This prepares transport bytes; it does not launch or attest execution.
///
/// # Errors
///
/// Refuses any candidate/plan/root/scope mismatch, invalid selected Sley
/// source, or an oversized/invalid supervisor request.
pub fn build_candidate_supervisor_request(
    plan: &NativeTestPlanV1,
    validated: &ValidatedCandidatePlan,
    test_entity: sley_id::EntityId,
    wall_ms: u64,
    nonce: [u8; 32],
) -> Result<RunRequest, ScbError> {
    let candidate = validated.candidate();
    let record = &candidate.record;
    if plan.selection_mode() != SELECTION_MODE_CANDIDATE_AFFECTED
        || plan.candidate_id() != Some(candidate.candidate_id)
        || plan.workspace() != record.workspace_id
        || plan.semantic_epoch() != record.schema_epoch_id
        || plan.parent_transaction() != record.base_transaction_id
        || plan.parent_root() != record.base_root
        || plan.proposed_root() != validated.candidate_root().root
        || plan.policy_root() != record.policy_root_id
        || plan.resource_policy().principal() != record.principal_id
    {
        return Err(ScbError::new(ScbErrorCode::ContractUnknown));
    }
    let program = PortableTestProgram::build(
        plan,
        validated.candidate_root(),
        validated.proposed_state().entities(),
        test_entity,
    )?;
    RunRequest::from_portable_program(&program, wall_ms, nonce)
}

/// Builds one explicit-root diagnostic request from the owner-loaded
/// accepted snapshot. The selected test, every object, and the complete root
/// must bind to the protected plan; transport bytes grant no commit authority.
///
/// # Errors
///
/// Refuses any plan/root/object mismatch, invalid source, or oversized
/// supervisor request before contacting the socket.
pub fn build_explicit_supervisor_request(
    plan: &NativeTestPlanV1,
    root: &AcceptedStateRoot,
    objects: &BTreeMap<ObjectId, &[u8]>,
    test_entity: sley_id::EntityId,
    wall_ms: u64,
    nonce: [u8; 32],
) -> Result<RunRequest, ScbError> {
    if plan.selection_mode() != SELECTION_MODE_EXPLICIT_ROOT
        || plan.candidate_id().is_some()
        || plan.parent_root() != root.root
        || plan.proposed_root() != root.root
        || plan.workspace() != root.record.workspace_id
        || plan.semantic_epoch() != root.record.schema_epoch_id
        || plan.policy_root() != root.record.policy_root
        || plan.resource_policy().principal() != PrincipalId::from_bytes([0; 32])
        || objects.len() != root.record.entity_bindings.len()
    {
        return Err(ScbError::new(ScbErrorCode::ContractUnknown));
    }
    let mut accepted_objects = Vec::with_capacity(objects.len());
    for (_, object_id) in &root.record.entity_bindings {
        let stored = objects
            .get(object_id)
            .ok_or_else(|| ScbError::new(ScbErrorCode::ContractUnknown))?;
        let object = import_entity_object(root.record.schema_epoch_id, stored)?;
        if object.object_id() != *object_id {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        accepted_objects.push(object);
    }
    let program = PortableTestProgram::build(plan, root, &accepted_objects, test_entity)?;
    RunRequest::from_portable_program(&program, wall_ms, nonce)
}

/// Test-execution dispatch: the qualified supervisor connection in
/// production, an explicitly test-only double in tests.
///
/// The executor receives the owner-derived plan and the validated proposed
/// state or owner-loaded accepted snapshot it was derived from. It must
/// return exactly one evidence pair per selected test in plan order; the
/// owning path re-verifies coverage and never trusts executor selection.
pub trait NativeTestExecutor {
    /// Executes every selected test in plan order.
    ///
    /// # Errors
    ///
    /// Returns the first execution refusal; test-level failures are evidence
    /// pairs with rejected content, not transport errors, so the owner can
    /// still build the rejected approval for diagnostics.
    fn execute(
        &self,
        plan: &NativeTestPlanV1,
        validated: &ValidatedCandidatePlan,
    ) -> Result<Vec<ExecutedNativeTest>, NativeCommitError>;

    /// Re-executes every selected test in plan order for explicit replay.
    ///
    /// Replay rebuilds observations without secrets: the owner supplies the
    /// stored plan and the pinned object bytes, never the original evidence
    /// (an executor that cannot see the originals cannot echo them) and
    /// never a live validation context. The owner checks coverage and
    /// compares exact bytes itself; no accepted transaction and no
    /// replacement attestation is created here.
    ///
    /// The default implementation refuses: executors that only serve the
    /// commit path keep refusing replay explicitly rather than silently
    /// reusing commit evidence.
    ///
    /// # Errors
    ///
    /// Returns `NATIVE_EXECUTOR_UNAVAILABLE` from the default refusal, or
    /// the first re-execution refusal; test-level failures stay evidence
    /// pairs with rejected content.
    fn execute_replay(
        &self,
        plan: &NativeTestPlanV1,
        objects: &BTreeMap<ObjectId, &[u8]>,
    ) -> Result<Vec<ExecutedNativeTest>, NativeCommitError> {
        let _ = (plan, objects);
        Err(NativeCommitError::ExecutorUnavailable)
    }

    /// Diagnostically executes one explicit-root plan over accepted objects
    /// for the 601 selection read: the owner supplies the stored plan, full
    /// accepted root, and object bytes keyed by object identity. The owner checks
    /// coverage and builds the diagnostic report itself; no approval,
    /// bundle, transaction, receipt, journal record, or head change
    /// results, so this entry point can never commit.
    ///
    /// The default implementation refuses: executors that only serve the
    /// commit path keep refusing diagnostics explicitly rather than
    /// silently reusing commit evidence.
    ///
    /// # Errors
    ///
    /// Returns `NATIVE_EXECUTOR_UNAVAILABLE` from the default refusal, or
    /// the first diagnostic refusal; test-level failures stay evidence
    /// pairs with rejected content.
    fn execute_diagnostic(
        &self,
        plan: &NativeTestPlanV1,
        root: &AcceptedStateRoot,
        objects: &BTreeMap<ObjectId, &[u8]>,
    ) -> Result<Vec<ExecutedNativeTest>, NativeCommitError> {
        let _ = (plan, root, objects);
        Err(NativeCommitError::ExecutorUnavailable)
    }
}

/// Returns whether the [`NativeTestExecutor`] reference is present.
///
/// The commit owner calls this before recording admission so the
/// executor-unavailable refusal precedes any journal or accepted-state
/// write.
#[must_use]
pub fn commit_needs_executor(executor: Option<&dyn NativeTestExecutor>) -> bool {
    executor.is_none()
}

/// Checks the total declared worker wall budget with checked arithmetic.
///
/// # Errors
///
/// Returns `TXN_RESOURCE_LIMIT` when the checked sum overflows or exceeds
/// the 30-second commit maximum.
pub(crate) fn check_native_wall_budget(
    plan: &NativeTestPlanV1,
) -> Result<u64, TransactionErrorCode> {
    let mut total = 0_u64;
    for entry in plan.selected() {
        total = total
            .checked_add(entry.declared_limits.wall_timeout_millis)
            .ok_or(TransactionErrorCode::ResourceLimit)?;
    }
    if total > MAX_COMMIT_WALL_MILLIS {
        return Err(TransactionErrorCode::ResourceLimit);
    }
    Ok(total)
}

/// Checks the caller's claimed admission profile against the fixed
/// descriptor the plan binds.
///
/// # Errors
///
/// Returns `TXN_RECEIPT_BINDING_MISMATCH` when the claimed identity is not
/// the fixed descriptor every plan carries.
pub(crate) fn check_admission_profile_binding(
    claimed: NativeAdmissionProfileId,
    fixed: &sley_tests::NativeAdmissionProfileV1,
    plan: &NativeTestPlanV1,
) -> Result<(), TransactionErrorCode> {
    if claimed != fixed.id() || plan.resource_policy().admission_profile() != *claimed.as_bytes() {
        return Err(TransactionErrorCode::ReceiptBindingMismatch);
    }
    Ok(())
}

/// Verifies the acceptance signer's structural trust for one statement.
///
/// The statement must name the supplied manifest, and the manifest must
/// grant the statement key the acceptance role for this workspace, admission
/// profile, and historical validation time. A manifest that never mentions
/// the key is unavailable; a mention without role, scope, profile, or
/// interval cover is rejected.
///
/// Besides the commit owner, repository exchange import uses this check to
/// verify every accepted statement against caller-supplied manifests
/// without installing trust.
///
/// # Errors
///
/// Returns `HISTORICAL_TRUST_UNAVAILABLE` or `HISTORICAL_TRUST_REJECTED`
/// per the rule above.
pub fn verify_acceptance_trust(
    statement_key: &[u8; 32],
    statement_policy: sley_id::HistoricalTrustPolicyId,
    workspace: WorkspaceId,
    admission_profile: NativeAdmissionProfileId,
    historical_time: u64,
    manifest: &HistoricalTrustPolicyV1,
) -> Result<(), NativeCommitError> {
    if statement_policy != manifest.id() {
        return Err(NativeCommitError::TrustRejected);
    }
    if manifest
        .entries()
        .iter()
        .all(|entry| &entry.key_id != statement_key)
    {
        return Err(NativeCommitError::TrustUnavailable);
    }
    if manifest.grants(
        statement_key,
        ROLE_ACCEPTANCE,
        workspace.as_bytes(),
        admission_profile.as_bytes(),
        historical_time,
    ) {
        Ok(())
    } else {
        Err(NativeCommitError::TrustRejected)
    }
}

/// Verifies one measurement attestation's structural trust.
///
/// The attestation must name the supplied measurement manifest, and the
/// manifest must grant the attestation key the measurement role for this
/// workspace, supervisor configuration, and historical run time.
///
/// Besides the commit owner, repository exchange import uses this check to
/// verify every embedded attestation against caller-supplied manifests
/// without installing trust.
///
/// # Errors
///
/// Returns `HISTORICAL_TRUST_UNAVAILABLE` or `HISTORICAL_TRUST_REJECTED`
/// per the acceptance rule.
pub fn verify_measurement_trust(
    attestation_key: &[u8; 32],
    attestation_policy: &[u8; 32],
    workspace: WorkspaceId,
    supervisor_config_id: &[u8; 32],
    historical_time: u64,
    manifest: &HistoricalTrustPolicyV1,
) -> Result<(), NativeCommitError> {
    if attestation_policy != manifest.id().as_bytes() {
        return Err(NativeCommitError::TrustRejected);
    }
    if manifest
        .entries()
        .iter()
        .all(|entry| &entry.key_id != attestation_key)
    {
        return Err(NativeCommitError::TrustUnavailable);
    }
    if manifest.grants(
        attestation_key,
        ROLE_MEASUREMENT,
        workspace.as_bytes(),
        supervisor_config_id,
        historical_time,
    ) {
        Ok(())
    } else {
        Err(NativeCommitError::TrustRejected)
    }
}

/// Verifies an acceptance statement's receiver grant and exact Ed25519
/// signature over its canonical fields 1 through 18.
///
/// # Errors
///
/// Returns the existing trust refusal when the grant, key encoding, canonical
/// preimage, or strict signature check fails.
pub fn verify_acceptance_statement(
    statement: &sley_tests::CommitAdmissionStatementV1,
    workspace: WorkspaceId,
    admission_profile: NativeAdmissionProfileId,
    manifest: &HistoricalTrustPolicyV1,
) -> Result<(), NativeCommitError> {
    let parts = statement.parts();
    verify_acceptance_trust(
        &parts.key_id,
        parts.acceptance_trust_policy_id,
        workspace,
        admission_profile,
        parts.historical_validation_time,
        manifest,
    )?;
    let unsigned = sley_tests::unsigned_statement_prefix(parts)
        .map_err(|_| NativeCommitError::TrustRejected)?;
    let preimage = sley_tests::admission_signature_preimage(&unsigned)
        .map_err(|_| NativeCommitError::TrustRejected)?;
    verify_ed25519_signature(&parts.key_id, &preimage, &parts.signature)
}

/// Verifies a measurement attestation's receiver grant and exact Ed25519
/// signature over its canonical fields 1 through 20.
///
/// # Errors
///
/// Returns the existing trust refusal when the grant, key encoding, canonical
/// preimage, or strict signature check fails.
pub fn verify_measurement_attestation(
    attestation: &sley_tests::MeasuredTestAttestationV1,
    workspace: WorkspaceId,
    manifest: &HistoricalTrustPolicyV1,
) -> Result<(), NativeCommitError> {
    let parts = attestation.parts();
    verify_measurement_trust(
        &parts.key_id,
        &parts.trust_policy_id,
        workspace,
        &parts.supervisor_config_id,
        parts.recorded_unix_millis,
        manifest,
    )?;
    let unsigned =
        sley_tests::unsigned_record_prefix(parts).map_err(|_| NativeCommitError::TrustRejected)?;
    let preimage = sley_tests::measurement_signature_preimage(&unsigned)
        .map_err(|_| NativeCommitError::TrustRejected)?;
    verify_ed25519_signature(&parts.key_id, &preimage, &parts.signature)
}

/// Independently checks an observed Sley report's complete program binding
/// and the signed host facts required for native commit admission.
///
/// A bound but non-admissible measurement returns `Ok(false)` so the owner can
/// issue a rejected approval without writing an accepted state. Cross-boundary
/// substitutions return a binding mismatch instead.
pub(crate) fn observed_measurement_admits(
    plan: &NativeTestPlanV1,
    validated: &ValidatedCandidatePlan,
    selected: SelectedEntry,
    report: &NativeExecutionReportV1,
    attestation: &MeasuredTestAttestationV1,
    config: &SupervisorConfigV1,
) -> Result<bool, TransactionErrorCode> {
    let mismatch = TransactionErrorCode::ReceiptBindingMismatch;
    if !matches!(report.evidence(), NativeExecutionEvidence::Observed { .. })
        || attestation.supervisor_config_id() != *config.id().as_bytes()
        || attestation.execution_report_id() != Some(report.report_id())
        || attestation.plan_id() != plan.plan_id()
        || attestation.test_object() != selected.test_object
        || attestation.declared_limits() != selected.declared_limits
    {
        return Err(mismatch);
    }
    let request = build_candidate_supervisor_request(
        plan,
        validated,
        selected.test_entity,
        selected.declared_limits.wall_timeout_millis,
        attestation.parts().attempt_nonce,
    )
    .map_err(|_| mismatch)?;
    request
        .verified_observed_worker_report(report.stored_bytes())
        .map_err(|_| mismatch)?;
    host_measurement_admits(&request, attestation, config)
}

fn host_measurement_admits(
    request: &RunRequest,
    attestation: &MeasuredTestAttestationV1,
    config: &SupervisorConfigV1,
) -> Result<bool, TransactionErrorCode> {
    let mismatch = TransactionErrorCode::ReceiptBindingMismatch;
    if attestation.supervisor_config_id() != *config.id().as_bytes()
        || attestation.workspace() != request.workspace
        || attestation.principal() != request.principal
        || attestation.declared_limits() != request.declared_limits
        || attestation.parts().attempt_nonce != request.nonce
    {
        return Err(mismatch);
    }
    let caller_uid = attestation.parts().caller_uid;
    let mut caller_matches = config
        .callers()
        .iter()
        .filter(|caller| caller.uid == caller_uid);
    let caller = caller_matches.next().ok_or(mismatch)?;
    if caller_matches.next().is_some()
        || caller.workspace != request.workspace
        || caller.principal != request.principal
    {
        return Err(mismatch);
    }
    let (_, expected_cap) = floor_page_cap(
        request.declared_limits.memory_bytes,
        config.parts().page_size,
    )
    .map_err(|_| mismatch)?;
    let property = |name: &str| {
        config
            .properties()
            .iter()
            .find(|property| property.name == name)
            .map(|property| property.value.as_str())
    };
    let runtime_text = property("RuntimeMaxUSec").ok_or(mismatch)?;
    let expected_runtime = runtime_max_usec(request.wall_ms).map_err(|_| mismatch)?;
    let events = attestation.memory_events();
    let expected_cap_text = expected_cap.to_string();
    let runtime_canonical = expected_runtime.to_string();
    Ok(attestation.claims_success()
        && attestation.installed_memory_cap() == expected_cap
        && property("MemoryMax") == Some(expected_cap_text.as_str())
        && runtime_text == runtime_canonical
        && check_memory_evidence(
            attestation.measured_memory_peak(),
            attestation.installed_memory_cap(),
            request.declared_limits.memory_bytes,
            events.max,
            events.oom,
            events.oom_kill,
        )
        .is_ok()
        && check_elapsed(attestation.elapsed_ns(), request.wall_ms).is_ok())
}

/// Verifies executor-returned evidence covers exactly the plan selection.
///
/// Every execution report and attestation must bind the plan, the exact
/// proposed test object, and each other; every attestation's supervisor
/// configuration must be among the returned configuration envelopes. An
/// attestation without an execution report is an explicit no-result failure:
/// it pairs only with rejected execution evidence, never with an observed
/// run.
///
/// The commit and replay owners share this check: replay coverage carries
/// no live validation context, only the stored plan.
///
/// # Errors
///
/// Returns `TXN_RECEIPT_BINDING_MISMATCH` for count, order, identity, plan
/// binding, report linkage, configuration, or duplicate-cover divergence.
pub fn check_execution_coverage(
    plan: &NativeTestPlanV1,
    executions: &[ExecutedNativeTest],
) -> Result<(), TransactionErrorCode> {
    use sley_tests::NativeExecutionEvidence;
    if executions.len() != plan.selected().len() {
        return Err(TransactionErrorCode::ReceiptBindingMismatch);
    }
    let mut seen = BTreeSet::new();
    for (execution, entry) in executions.iter().zip(plan.selected()) {
        if execution.test_entity != entry.test_entity || !seen.insert(execution.test_entity) {
            return Err(TransactionErrorCode::ReceiptBindingMismatch);
        }
        let execution_report =
            sley_tests::NativeExecutionReportV1::parse(&execution.execution_stored)
                .map_err(|_| TransactionErrorCode::ReceiptBindingMismatch)?;
        let attestation =
            sley_tests::MeasuredTestAttestationV1::parse(&execution.attestation_stored)
                .map_err(|_| TransactionErrorCode::ReceiptBindingMismatch)?;
        let config = sley_tests::SupervisorConfigV1::parse(&execution.supervisor_config_stored)
            .map_err(|_| TransactionErrorCode::ReceiptBindingMismatch)?;
        if execution_report.plan_id() != plan.plan_id()
            || execution_report.test_entity() != entry.test_entity
            || execution_report.test_object() != entry.test_object
            || attestation.plan_id() != plan.plan_id()
            || attestation.test_object() != entry.test_object
            || attestation.supervisor_config_id() != *config.id().as_bytes()
        {
            return Err(TransactionErrorCode::ReceiptBindingMismatch);
        }
        match (
            attestation.execution_report_id(),
            execution_report.evidence(),
        ) {
            (Some(expected), _) if expected == execution_report.report_id() => {}
            // Explicit no-result failures pair only with rejected evidence.
            (None, NativeExecutionEvidence::Rejected(_)) => {}
            _ => return Err(TransactionErrorCode::ReceiptBindingMismatch),
        }
    }
    Ok(())
}

/// Takes exactly `N` bytes at the cursor, advancing it past them.
fn take_bytes<const N: usize>(
    body: &[u8],
    offset: &mut usize,
) -> Result<[u8; N], NativeCommitError> {
    let bytes = <[u8; N]>::try_from(
        body.get(*offset..*offset + N)
            .ok_or(NativeCommitError::JournalCorrupt)?,
    )
    .map_err(|_| NativeCommitError::JournalCorrupt)?;
    *offset += N;
    Ok(bytes)
}

/// Returns the journal directory, creating it when absent.
fn attempts_dir(root: &Path) -> io::Result<PathBuf> {
    let directory = root.join(ATTEMPTS_DIR);
    match fs::create_dir(&directory) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            if !fs::symlink_metadata(&directory)?.is_dir() {
                return Err(io::Error::other("attempt journal is not a directory"));
            }
        }
        Err(error) => return Err(error),
    }
    Ok(directory)
}

/// Returns the journal path for one attempt without creating anything.
#[must_use]
pub fn attempt_path(root: &Path, attempt_id: NativeAttemptId) -> PathBuf {
    let active = active_attempt_path(root, attempt_id);
    if active.exists() {
        active
    } else {
        let settled = settled_attempt_path(root, attempt_id);
        if settled.exists() { settled } else { active }
    }
}

fn active_attempt_path(root: &Path, attempt_id: NativeAttemptId) -> PathBuf {
    root.join(ATTEMPTS_DIR)
        .join(format!("{}{}", attempt_id.hex(), ATTEMPT_SUFFIX))
}

fn settled_attempt_path(root: &Path, attempt_id: NativeAttemptId) -> PathBuf {
    root.join(ATTEMPTS_DIR)
        .join(SETTLED_ATTEMPTS_DIR)
        .join(format!("{}{}", attempt_id.hex(), ATTEMPT_SUFFIX))
}

fn settled_attempts_dir(root: &Path) -> io::Result<PathBuf> {
    let directory = attempts_dir(root)?.join(SETTLED_ATTEMPTS_DIR);
    match fs::create_dir(&directory) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            if !fs::symlink_metadata(&directory)?.is_dir() {
                return Err(io::Error::other(
                    "settled attempt journal is not a directory",
                ));
            }
        }
        Err(error) => return Err(error),
    }
    Ok(directory)
}

/// Reads one journal record without creating anything.
pub(crate) fn read_attempt_record(
    root: &Path,
    attempt_id: NativeAttemptId,
) -> Result<Option<AttemptRecord>, super::repository::CommitError> {
    use super::repository::CommitError;
    let path = attempt_path(root, attempt_id);
    match fs::read(&path) {
        Ok(bytes) => {
            let length = u64::try_from(bytes.len())
                .map_err(|_| CommitError::Native(NativeCommitError::JournalCorrupt))?;
            if length > MAX_JOURNAL_BYTES {
                return Err(CommitError::Native(NativeCommitError::JournalCorrupt));
            }
            AttemptRecord::parse(&bytes)
                .map(Some)
                .map_err(CommitError::Native)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(CommitError::Io(error)),
    }
}

/// Atomically persists one journal record: stage, sync, rename, sync dir.
pub(crate) fn write_attempt_record(root: &Path, record: &AttemptRecord) -> io::Result<()> {
    let directory = attempts_dir(root)?;
    let settled_directory = settled_attempts_dir(root)?;
    let final_path = active_attempt_path(root, record.attempt_id);
    let settled_path = settled_attempt_path(root, record.attempt_id);
    // A retry of an aborted attempt reactivates the same durable record.
    // Moving it first makes a crash before the new write leave the old state
    // queryable and safe to retry again.
    if !final_path.exists() {
        match fs::rename(&settled_path, &final_path) {
            Ok(()) => {
                sync_directory(&settled_directory)?;
                sync_directory(&directory)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    let stage_path = directory.join(format!(
        "{}{}-{}",
        ATTEMPT_STAGE_PREFIX,
        record.attempt_id.hex(),
        std::process::id()
    ));
    let bytes = record.encode();
    {
        let mut stage = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stage_path)?;
        stage.write_all(&bytes)?;
        stage.flush()?;
        stage.sync_all()?;
        drop(stage);
    }
    // Re-read the stage so a torn write can never become the journal.
    let staged = fs::read(&stage_path)?;
    AttemptRecord::parse(&staged).map_err(|_| {
        let _ = fs::remove_file(&stage_path);
        io::Error::other("attempt journal stage failed verification")
    })?;
    fs::rename(&stage_path, &final_path)?;
    sync_directory(&directory)?;
    // Confirm the final bytes before any caller acts on the record.
    let final_bytes = fs::read(&final_path)?;
    AttemptRecord::parse(&final_bytes)
        .map_err(|_| io::Error::other("attempt journal final failed verification"))?;
    if matches!(
        record.state,
        AttemptState::AbortedBeforePromotion
            | AttemptState::Committed
            | AttemptState::OutcomeUnknown
    ) {
        park_attempt_record(root, record.attempt_id)?;
    }
    Ok(())
}

/// Removes a validated record from the recovery scan while retaining exact
/// status and retry bindings under its attempt identity.
pub(crate) fn park_attempt_record(root: &Path, attempt_id: NativeAttemptId) -> io::Result<()> {
    let directory = attempts_dir(root)?;
    let settled_directory = settled_attempts_dir(root)?;
    fs::rename(
        active_attempt_path(root, attempt_id),
        settled_attempt_path(root, attempt_id),
    )?;
    sync_directory(&settled_directory)?;
    sync_directory(&directory)
}

/// Syncs one directory without following symlinks.
fn sync_directory(directory: &Path) -> io::Result<()> {
    let file = fs::File::open(directory)?;
    file.sync_all()
}

/// Reconciled operator-facing attempt state: journal hints checked against
/// actual receipt and head bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptStatus {
    /// No journal record for this attempt.
    Unknown,
    /// Admitted; no worker started.
    Admitted,
    /// Workers started; nothing promoted.
    Running,
    /// Refused or failed before promotion; safe to retry after reconciling.
    AbortedBeforePromotion,
    /// Receipt persisted; head CAS unconfirmed.
    PromotionStarted,
    /// Head CAS confirmed for the recorded identities.
    Committed {
        /// Promoted transaction.
        transaction_id: TransactionId,
        /// Promoted receipt.
        receipt_id: ReceiptId,
        /// Whether the accepted head still names the transaction.
        at_head: bool,
    },
    /// Promotion may or may not have completed; the head at recovery time is
    /// reported so the operator queries history instead of resubmitting.
    OutcomeUnknown {
        /// Accepted head observed while reconciling, when initialized.
        head: Option<TransactionId>,
    },
}

/// Journal-bound attempt scope for protocol-layer enforcement.
///
/// The 607 attempt-status surface binds a query to the journaled
/// workspace and candidate without consulting receipts: unknown attempts
/// stay unknown, and divergent bindings refuse before any status work.
/// The principal is reported so future authenticated sessions can check
/// it; current sessions carry a workspace but no principal, so the
/// protocol layer enforces workspace and candidate only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeAttemptScope {
    /// Workspace the attempt was admitted in.
    pub workspace: WorkspaceId,
    /// Principal the attempt was admitted for.
    pub principal: PrincipalId,
    /// Candidate the attempt binds.
    pub candidate_id: CandidateId,
    /// Accepted parent the attempt builds on.
    pub expected_parent: TransactionId,
}

/// Fresh ordinary-candidate native commit inputs not recoverable from
/// accepted state.
#[derive(Clone, Copy)]
pub struct NativeCommitInput<'a> {
    /// Accepted parent the candidate builds on.
    pub expected_parent: TransactionId,
    /// Exact candidate bytes.
    pub stored_candidate: &'a [u8],
    /// Authenticated principal.
    pub principal_id: PrincipalId,
    /// Authenticated capability tuples.
    pub capabilities: &'a [sley_policy::TrustedCandidateCapability<'a>],
    /// Historical validation time evaluated by the acceptance signer.
    pub now_unix_millis: u64,
    /// Requested local validation ceilings.
    pub limits: sley_policy::CandidateValidationLimits,
    /// Client-generated attempt binding this commit records.
    pub attempt_id: NativeAttemptId,
    /// Claimed admission descriptor; must be the fixed descriptor.
    pub admission_profile_id: NativeAdmissionProfileId,
    /// Configured local native implementation ceilings.
    pub implementation_limits: sley_tests::NativeImplementationLimits,
    /// Configured local aggregate ceilings within the hard maxima.
    pub aggregate: sley_tests::NativeAggregateLimits,
    /// Qualified test-execution dispatch; `None` refuses honestly.
    pub executor: Option<&'a dyn NativeTestExecutor>,
    /// Configured acceptance-signer adapter.
    pub acceptance_signer: &'a dyn NativeAcceptanceSigner,
    /// Receiver-provisioned measurement trust manifest.
    pub measurement_trust: &'a HistoricalTrustPolicyV1,
    /// Receiver-provisioned acceptance trust manifest.
    pub acceptance_trust: &'a HistoricalTrustPolicyV1,
}

/// Successful durable native commit result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeCommitOutput {
    transaction_id: TransactionId,
    receipt_id: ReceiptId,
    state_root: sley_state_root::AcceptedStateRoot,
    candidate_result: sley_policy::ImportedCandidateResult,
    approval_id: sley_id::NativeTestApprovalId,
    report_id: sley_id::TestReportId,
    attempt_id: NativeAttemptId,
}

impl NativeCommitOutput {
    /// Constructs the committed result; only the repository owner calls
    /// this after persisting the receipt and confirming the head CAS.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub(crate) fn new(
        transaction_id: TransactionId,
        receipt_id: ReceiptId,
        state_root: sley_state_root::AcceptedStateRoot,
        candidate_result: sley_policy::ImportedCandidateResult,
        approval_id: sley_id::NativeTestApprovalId,
        report_id: sley_id::TestReportId,
        attempt_id: NativeAttemptId,
    ) -> Self {
        Self {
            transaction_id,
            receipt_id,
            state_root,
            candidate_result,
            approval_id,
            report_id,
            attempt_id,
        }
    }

    /// Returns the new durable accepted revision identity.
    #[must_use]
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction_id
    }

    /// Returns the complete persisted native receipt identity.
    #[must_use]
    pub const fn receipt_id(&self) -> ReceiptId {
        self.receipt_id
    }

    /// Returns the committed ancestry-independent semantic root.
    #[must_use]
    pub const fn state_root(&self) -> &sley_state_root::AcceptedStateRoot {
        &self.state_root
    }

    /// Returns the fresh commit-time validation result.
    #[must_use]
    pub const fn candidate_result(&self) -> &sley_policy::ImportedCandidateResult {
        &self.candidate_result
    }

    /// Returns the native approval authorizing the test evidence.
    #[must_use]
    pub const fn approval_id(&self) -> sley_id::NativeTestApprovalId {
        self.approval_id
    }

    /// Returns the deterministic native test report.
    #[must_use]
    pub const fn report_id(&self) -> sley_id::TestReportId {
        self.report_id
    }

    /// Returns the attempt this commit recorded.
    #[must_use]
    pub const fn attempt_id(&self) -> NativeAttemptId {
        self.attempt_id
    }
}

/// Rejected native evidence returned without any accepted-state write.
///
/// A test failure produces durable diagnostic attempt records and this
/// rejected approval; the accepted head and semantic roots remain unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRejection {
    /// Rejected approval binding the failed evidence.
    pub approval: NativeTestApprovalV1,
    /// Deterministic report with the mismatch or rejection entries.
    pub report: NativeTestReportV1,
    /// Attempt the rejection was journaled under.
    pub attempt_id: NativeAttemptId,
}

/// Operator-facing native commit result: committed, or rejected evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeCommitOutcome {
    /// Receipt persisted and head CAS confirmed.
    Committed(NativeCommitOutput),
    /// Tests failed; nothing accepted, rejection returned for diagnostics.
    Rejected(NativeRejection),
}

/// Committed semantic root of a native receipt, without repository reads.
#[must_use]
pub fn native_receipt_committed_root(
    receipt: &crate::native_codec::ImportedNativeTransactionReceipt,
) -> StateRoot {
    receipt.transaction.record.committed_root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_lines)]
    fn signed_diagnostic_fixture() -> (RunRequest, RunResponse, HistoricalTrustPolicyV1) {
        use sley_test_runner::{
            program::PortableTestProgram, response::RunEvidence, unit::REQUIRED_PROPERTIES,
            worker::WorkerRequest,
        };
        use sley_tests::{
            Caller, HistoricalTrustPolicyParts, MeasuredTestAttestationParts,
            MeasuredTestAttestationV1, MemoryEvents, NativeExecutionEvidence,
            NativeExecutionReportParts, NativeExecutionReportV1, Property, REJECT_PHASE_EXECUTION,
            RejectedEvidence, SupervisorConfigParts, SupervisorConfigV1,
            TERMINATION_PRELAUNCH_REFUSED, TrustEntry, measurement_signature_preimage,
            unsigned_record_prefix,
        };

        let worker = WorkerRequest::decode_frame(include_bytes!(
            "../../../conformance/native-worker/v1/observed-input.bin"
        ))
        .expect("canonical worker vector");
        let program = PortableTestProgram::parse(&worker.program_bytes).expect("portable program");
        let request = RunRequest::from_portable_program(
            &program,
            1_000,
            sley_test_runner::nonce::random_attempt_nonce().expect("OS entropy"),
        )
        .expect("selected supervisor request");
        let report = NativeExecutionReportV1::build(NativeExecutionReportParts {
            plan_id: request.plan_id,
            test_entity: request.test_entity,
            test_object: request.test_object,
            target_object: program.selected().target_object,
            evidence: NativeExecutionEvidence::Rejected(
                RejectedEvidence::from_parts(
                    REJECT_PHASE_EXECUTION,
                    29_211,
                    "NATIVE_TEST_EXECUTION_REJECTED",
                )
                .expect("rejected evidence"),
            ),
        })
        .expect("rejected report");
        let mut properties = REQUIRED_PROPERTIES
            .iter()
            .map(|(name, value)| Property {
                name: (*name).to_owned(),
                value: (*value).to_owned(),
            })
            .collect::<Vec<_>>();
        properties.push(Property {
            name: "MemoryMax".to_owned(),
            value: "4096".to_owned(),
        });
        properties.push(Property {
            name: "RuntimeMaxUSec".to_owned(),
            value: "3000000".to_owned(),
        });
        properties.sort_by(|left, right| left.name.cmp(&right.name));
        let config = SupervisorConfigV1::build(SupervisorConfigParts {
            worker_digest: [7; 32],
            supervisor_digest: [8; 32],
            properties,
            callers: vec![Caller {
                uid: 1_000,
                workspace: request.workspace,
                principal: request.principal,
            }],
            page_size: 4_096,
            cleanup_millis: 2_000,
            launch_profile: 1,
        })
        .expect("supervisor configuration");
        let key = SigningKey::from_bytes(&[3; 32]);
        let key_id = key.verifying_key().to_bytes();
        let trust = HistoricalTrustPolicyV1::build(HistoricalTrustPolicyParts {
            policy_nonce: [11; 32],
            entries: vec![TrustEntry {
                key_id,
                role: ROLE_MEASUREMENT,
                workspaces: vec![*request.workspace.as_bytes()],
                profiles: vec![*config.id().as_bytes()],
                valid_from_unix_millis: 0,
                valid_until_unix_millis: u64::MAX,
            }],
        })
        .expect("measurement trust");
        let mut attestation_parts = MeasuredTestAttestationParts {
            key_id,
            trust_policy_id: *trust.id().as_bytes(),
            supervisor_config_id: *config.id().as_bytes(),
            plan_id: request.plan_id,
            test_object: request.test_object,
            execution_report_id: None,
            attempt_nonce: request.nonce,
            workspace: request.workspace,
            principal: request.principal,
            caller_uid: 1_000,
            declared_limits: request.declared_limits,
            installed_memory_cap: 0,
            elapsed_ns: 0,
            measured_memory_peak: 0,
            memory_events: MemoryEvents {
                max: 0,
                oom: 0,
                oom_kill: 0,
            },
            termination: TERMINATION_PRELAUNCH_REFUSED,
            complete_output: false,
            empty_cgroup_confirmed: true,
            recorded_unix_millis: 1_000,
            signature: [0; 64],
        };
        let unsigned = unsigned_record_prefix(&attestation_parts).expect("unsigned attestation");
        let preimage = measurement_signature_preimage(&unsigned).expect("signature preimage");
        attestation_parts.signature = key.sign(&preimage).to_bytes();
        let attestation =
            MeasuredTestAttestationV1::build(attestation_parts).expect("signed attestation");
        let response = RunResponse {
            status: RunStatus::Refused,
            code: 7,
            evidence: Some(
                RunEvidence::build(report, attestation, config).expect("bound evidence"),
            ),
        };
        (request, response, trust)
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn supervisor_response_requires_exact_binding_and_trusted_signature() {
        use sley_test_runner::response::RunEvidence;
        use sley_tests::{
            HistoricalTrustPolicyParts, MeasuredTestAttestationV1, ROLE_MEASUREMENT,
            TERMINATION_COMPLETE, TrustEntry, measurement_signature_preimage,
            unsigned_record_prefix,
        };

        let (request, response, trust) = signed_diagnostic_fixture();
        let frame = response.encode_frame().expect("diagnostic response frame");
        let executed = verified_supervisor_execution(&request, &frame, 1_000, &trust)
            .expect("trusted signed diagnostic is preserved");
        let evidence = response.evidence.as_ref().expect("diagnostic evidence");
        assert_eq!(executed.test_entity, request.test_entity);
        assert_eq!(executed.execution_stored, evidence.report().stored_bytes());
        assert_eq!(
            executed.attestation_stored,
            evidence.attestation().stored_bytes()
        );
        assert_eq!(
            executed.supervisor_config_stored,
            evidence.supervisor_config().stored_bytes()
        );
        let profile_only = HistoricalTrustPolicyV1::build(HistoricalTrustPolicyParts {
            policy_nonce: [12; 32],
            entries: vec![TrustEntry {
                key_id: evidence.attestation().parts().key_id,
                role: ROLE_MEASUREMENT,
                workspaces: vec![*request.workspace.as_bytes()],
                profiles: vec![
                    *request
                        .verified_program()
                        .expect("program")
                        .plan()
                        .execution_profile()
                        .as_bytes(),
                ],
                valid_from_unix_millis: 0,
                valid_until_unix_millis: u64::MAX,
            }],
        })
        .expect("profile-only trust");
        assert_ne!(
            *request
                .verified_program()
                .expect("program")
                .plan()
                .execution_profile()
                .as_bytes(),
            evidence.attestation().parts().supervisor_config_id
        );
        assert_eq!(
            verify_measurement_trust(
                &evidence.attestation().parts().key_id,
                profile_only.id().as_bytes(),
                request.workspace,
                &evidence.attestation().parts().supervisor_config_id,
                evidence.attestation().parts().recorded_unix_millis,
                &profile_only,
            ),
            Err(NativeCommitError::TrustRejected)
        );

        let mut wrong_nonce = request.clone();
        wrong_nonce.nonce[0] ^= 1;
        assert_eq!(
            verified_supervisor_execution(&wrong_nonce, &frame, 1_000, &trust),
            Err(SupervisorEvidenceError::InvalidResponse(
                ScbErrorCode::ContractUnknown
            ))
        );
        assert_eq!(
            verified_supervisor_execution(&request, &frame, 1_001, &trust),
            Err(SupervisorEvidenceError::InvalidResponse(
                ScbErrorCode::ContractUnknown
            ))
        );

        let mut invalid = response.clone();
        let old = invalid.evidence.take().expect("evidence");
        let mut parts = old.attestation().parts().clone();
        parts.signature[63] ^= 1;
        let bad_signature = MeasuredTestAttestationV1::build(parts).expect("bad signed bytes");
        invalid.evidence = Some(
            RunEvidence::build(
                old.report().clone(),
                bad_signature,
                old.supervisor_config().clone(),
            )
            .expect("bound invalid signature"),
        );
        assert_eq!(
            verified_supervisor_execution(
                &request,
                &invalid.encode_frame().expect("invalid signature frame"),
                1_000,
                &trust,
            ),
            Err(SupervisorEvidenceError::MeasurementTrust(
                NativeCommitError::TrustRejected
            ))
        );

        let mut false_refusal = response.clone();
        let old = false_refusal.evidence.take().expect("evidence");
        let mut parts = old.attestation().parts().clone();
        parts.execution_report_id = Some(old.report().report_id());
        parts.termination = TERMINATION_COMPLETE;
        parts.complete_output = true;
        parts.installed_memory_cap = 4_096;
        parts.signature = [0; 64];
        let unsigned = unsigned_record_prefix(&parts).expect("unsigned attestation");
        let preimage = measurement_signature_preimage(&unsigned).expect("signature preimage");
        parts.signature = SigningKey::from_bytes(&[3; 32]).sign(&preimage).to_bytes();
        let attestation = MeasuredTestAttestationV1::build(parts).expect("signed claim");
        false_refusal.evidence = Some(
            RunEvidence::build(
                old.report().clone(),
                attestation,
                old.supervisor_config().clone(),
            )
            .expect("bound success claim"),
        );
        assert_eq!(
            verified_supervisor_execution(
                &request,
                &false_refusal
                    .encode_frame()
                    .expect("contradictory refusal frame"),
                1_000,
                &trust,
            ),
            Err(SupervisorEvidenceError::InvalidResponse(
                ScbErrorCode::ContractUnknown
            ))
        );
        false_refusal.status = RunStatus::Complete;
        false_refusal.code = 0;
        let rejected_complete = verified_supervisor_execution(
            &request,
            &false_refusal
                .encode_frame()
                .expect("complete rejected frame"),
            1_000,
            &trust,
        )
        .expect("complete worker rejection remains a diagnostic result");
        assert_eq!(
            rejected_complete.execution_stored,
            false_refusal
                .evidence
                .as_ref()
                .expect("complete evidence")
                .report()
                .stored_bytes()
        );

        let no_evidence = RunResponse {
            status: RunStatus::Refused,
            code: 7,
            evidence: None,
        };
        assert_eq!(
            verified_supervisor_execution(
                &request,
                &no_evidence.encode_frame().expect("refusal frame"),
                1_000,
                &trust,
            ),
            Err(SupervisorEvidenceError::NoEvidence {
                status: RunStatus::Refused,
                code: 7,
            })
        );
    }

    #[test]
    fn supervisor_complete_response_preserves_native_observation() {
        use sley_test_runner::response::RunEvidence;
        use sley_tests::{
            MeasuredTestAttestationV1, NativeExecutionReportV1, TERMINATION_COMPLETE,
            measurement_signature_preimage, unsigned_record_prefix,
        };

        let (request, diagnostic, trust) = signed_diagnostic_fixture();
        let old = diagnostic.evidence.expect("diagnostic evidence");
        let report = NativeExecutionReportV1::parse(include_bytes!(
            "../../../conformance/native-worker/v1/observed-report.bin"
        ))
        .expect("canonical observed report");
        let mut parts = old.attestation().parts().clone();
        parts.execution_report_id = Some(report.report_id());
        parts.installed_memory_cap = 4_096;
        parts.elapsed_ns = 1;
        parts.measured_memory_peak = 1;
        parts.termination = TERMINATION_COMPLETE;
        parts.complete_output = true;
        parts.signature = [0; 64];
        let unsigned = unsigned_record_prefix(&parts).expect("unsigned attestation");
        let preimage = measurement_signature_preimage(&unsigned).expect("signature preimage");
        parts.signature = SigningKey::from_bytes(&[3; 32]).sign(&preimage).to_bytes();
        let attestation = MeasuredTestAttestationV1::build(parts).expect("signed measurement");
        let config = old.supervisor_config().clone();
        let response = RunResponse {
            status: RunStatus::Complete,
            code: 0,
            evidence: Some(
                RunEvidence::build(report.clone(), attestation, config).expect("evidence"),
            ),
        };
        let frame = response.encode_frame().expect("complete frame");
        let executed = verified_supervisor_execution(&request, &frame, 1_000, &trust)
            .expect("trusted native observation");
        assert_eq!(executed.execution_stored, report.stored_bytes());
        assert_eq!(executed.test_entity, request.test_entity);
        let evidence = response.evidence.as_ref().expect("complete evidence");
        assert_eq!(
            host_measurement_admits(
                &request,
                evidence.attestation(),
                evidence.supervisor_config()
            ),
            Ok(true)
        );
        let signed_change = |change: fn(&mut sley_tests::MeasuredTestAttestationParts)| {
            let mut parts = evidence.attestation().parts().clone();
            change(&mut parts);
            parts.signature = [0; 64];
            let unsigned = unsigned_record_prefix(&parts).expect("unsigned attestation");
            let preimage = measurement_signature_preimage(&unsigned).expect("signature preimage");
            parts.signature = SigningKey::from_bytes(&[3; 32]).sign(&preimage).to_bytes();
            MeasuredTestAttestationV1::build(parts).expect("changed signed attestation")
        };
        for attestation in [
            signed_change(|parts| parts.measured_memory_peak = 4_097),
            signed_change(|parts| parts.elapsed_ns = 1_000_000_000),
            signed_change(|parts| parts.memory_events.oom = 1),
            signed_change(|parts| parts.complete_output = false),
        ] {
            assert_eq!(
                host_measurement_admits(&request, &attestation, evidence.supervisor_config()),
                Ok(false)
            );
        }
    }

    fn record(state: AttemptState) -> AttemptRecord {
        let (transaction_id, receipt_id) = match state {
            AttemptState::Committed | AttemptState::PromotionStarted => (
                Some(TransactionId::from_bytes([7; 32])),
                Some(ReceiptId::from_bytes([8; 32])),
            ),
            _ => (None, None),
        };
        AttemptRecord {
            attempt_id: NativeAttemptId([9; 16]),
            workspace: WorkspaceId::from_bytes([1; 32]),
            principal: PrincipalId::from_bytes([2; 32]),
            candidate_id: CandidateId::from_bytes([3; 32]),
            expected_parent: TransactionId::from_bytes([4; 32]),
            state,
            transaction_id,
            receipt_id,
        }
    }

    #[test]
    fn journal_records_round_trip_every_state() {
        for state in [
            AttemptState::Admitted,
            AttemptState::Running,
            AttemptState::AbortedBeforePromotion,
            AttemptState::PromotionStarted,
            AttemptState::Committed,
            AttemptState::OutcomeUnknown,
        ] {
            let parsed = AttemptRecord::parse(&record(state).encode()).expect("round trips");
            assert_eq!(parsed, record(state));
            assert_eq!(parsed.state.tag(), state.tag());
            assert_eq!(AttemptState::from_tag(state.tag()), Some(state));
        }
        assert_eq!(AttemptState::from_tag(0), None);
        assert_eq!(AttemptState::from_tag(7), None);
        assert_eq!(record(AttemptState::Admitted).attempt_id.hex().len(), 32);
    }

    #[test]
    fn journal_parse_refuses_any_shape_or_checksum_damage() {
        let good = record(AttemptState::Committed).encode();
        // Truncation, extension, magic, version, state, flags, claims.
        assert!(AttemptRecord::parse(&[]).is_err());
        assert!(AttemptRecord::parse(&good[..good.len() - 1]).is_err());
        let mut long = good.clone();
        long.push(0);
        assert!(AttemptRecord::parse(&long).is_err());
        let mut magic = good.clone();
        magic[0] ^= 1;
        assert!(AttemptRecord::parse(&magic).is_err());
        let mut version = good.clone();
        version[15] ^= 1;
        assert!(AttemptRecord::parse(&version).is_err());
        let mut state = good.clone();
        state[23] = 9;
        assert!(AttemptRecord::parse(&state).is_err());
        let mut checksum = good.clone();
        let last = checksum.len() - 1;
        checksum[last] ^= 1;
        assert!(AttemptRecord::parse(&checksum).is_err());
        // Committed without promotion claims refuses.
        let mut unclaimed = record(AttemptState::Committed);
        unclaimed.transaction_id = None;
        assert!(AttemptRecord::parse(&unclaimed.encode()).is_err());
        // Pre-commit states with promotion claims refuse.
        let mut claimed = record(AttemptState::Running);
        claimed.transaction_id = Some(TransactionId::from_bytes([7; 32]));
        claimed.receipt_id = Some(ReceiptId::from_bytes([8; 32]));
        assert!(AttemptRecord::parse(&claimed.encode()).is_err());
        // PromotionStarted may carry the unconfirmed claim.
        let mut promoting = record(AttemptState::PromotionStarted);
        promoting.transaction_id = None;
        promoting.receipt_id = None;
        assert!(AttemptRecord::parse(&promoting.encode()).is_ok());
    }

    #[test]
    fn journal_bindings_compare_without_state_or_claims() {
        let first = record(AttemptState::Admitted);
        let mut second = record(AttemptState::Committed);
        assert!(first.same_bindings(&second));
        second.candidate_id = CandidateId::from_bytes([10; 32]);
        assert!(!first.same_bindings(&second));
    }

    #[test]
    fn native_commit_error_symbols_are_frozen() {
        for (error, symbol) in [
            (
                NativeCommitError::ExecutorUnavailable,
                "NATIVE_EXECUTOR_UNAVAILABLE",
            ),
            (
                NativeCommitError::BusyRetrySafe,
                "NATIVE_COMMIT_BUSY_RETRY_SAFE",
            ),
            (
                NativeCommitError::AbortedRetrySafe,
                "NATIVE_COMMIT_ABORTED_RETRY_SAFE",
            ),
            (
                NativeCommitError::OutcomeUnknown,
                "NATIVE_COMMIT_OUTCOME_UNKNOWN",
            ),
            (
                NativeCommitError::AttemptConflict,
                "NATIVE_ATTEMPT_CONFLICT",
            ),
            (NativeCommitError::JournalCorrupt, "NATIVE_JOURNAL_CORRUPT"),
            (
                NativeCommitError::TrustUnavailable,
                "HISTORICAL_TRUST_UNAVAILABLE",
            ),
            (
                NativeCommitError::TrustRejected,
                "HISTORICAL_TRUST_REJECTED",
            ),
        ] {
            assert_eq!(error.symbol(), symbol);
            assert_eq!(error.to_string(), symbol);
        }
    }

    #[test]
    fn ed25519_acceptance_signer_is_strict_and_tamper_evident() {
        let signer = Ed25519AcceptanceSigner::from_secret_bytes([0x37; 32]);
        let preimage = b"sley2 acceptance test preimage";
        let signature = signer.sign(preimage);
        assert_eq!(
            verify_ed25519_signature(&signer.key_id(), preimage, &signature),
            Ok(())
        );

        let mut changed_message = preimage.to_vec();
        changed_message[0] ^= 1;
        assert_eq!(
            verify_ed25519_signature(&signer.key_id(), &changed_message, &signature),
            Err(NativeCommitError::TrustRejected)
        );

        let mut changed_signature = signature;
        changed_signature[63] ^= 1;
        assert_eq!(
            verify_ed25519_signature(&signer.key_id(), preimage, &changed_signature),
            Err(NativeCommitError::TrustRejected)
        );
        assert_eq!(
            verify_ed25519_signature(&[0; 32], preimage, &[0; 64]),
            Err(NativeCommitError::TrustRejected)
        );
    }
}

/// Complete verified native transaction state loaded from durable bytes.
///
/// The v1 [`VerifiedRevision`](super::repository::VerifiedRevision) stays
/// frozen; native heads load through this disjoint view with the same
/// relationship, object, manifest, inventory, evidence, and pin checks the
/// versioned preflight enforces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeVerifiedRevision {
    transaction_id: TransactionId,
    receipt: crate::native_codec::ImportedNativeTransactionReceipt,
    objects: Vec<sley_mutate::EntityObject>,
}

impl NativeVerifiedRevision {
    /// Constructs the verified view; only the repository owner calls this
    /// after running every check.
    #[must_use]
    pub fn verified(
        transaction_id: TransactionId,
        receipt: crate::native_codec::ImportedNativeTransactionReceipt,
        objects: Vec<sley_mutate::EntityObject>,
    ) -> Self {
        Self {
            transaction_id,
            receipt,
            objects,
        }
    }

    /// Returns the exact verified revision identity.
    #[must_use]
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction_id
    }

    /// Returns the independently authenticated complete native receipt.
    #[must_use]
    pub const fn receipt(&self) -> &crate::native_codec::ImportedNativeTransactionReceipt {
        &self.receipt
    }

    /// Returns the registry-authorized accepted semantic root.
    #[must_use]
    pub const fn state_root(&self) -> &sley_state_root::AcceptedStateRoot {
        &self.receipt.state_root
    }

    /// Returns the registry-authorized protected policy root.
    #[must_use]
    pub const fn policy_root(&self) -> &sley_policy::AcceptedPolicyRoot {
        &self.receipt.policy_root
    }

    /// Returns every exact live entity object in state-root binding order.
    #[must_use]
    pub fn objects(&self) -> &[sley_mutate::EntityObject] {
        &self.objects
    }

    /// Returns the complete sorted non-reusable identity ledger.
    #[must_use]
    pub fn tombstoned_entities(&self) -> &[sley_id::EntityId] {
        &self.receipt.transaction.record.tombstoned_entities
    }
}
