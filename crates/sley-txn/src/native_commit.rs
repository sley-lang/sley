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
//! Signature handling stays structural in this slice: the acceptance signer
//! adapter produces the 64-byte statement signature over the shared
//! [`admission_signature_preimage`](sley_tests::statement::admission_signature_preimage),
//! and the commit path checks key-ID binding, signer role, workspace and
//! profile scope, and the historical validity interval against the
//! receiver-provisioned trust manifests. Curve verification waits on
//! vendored Ed25519 crypto and is never claimed. Measurement attestations
//! are checked the same structural way; the qualified supervisor that
//! produces real measurements is a separate, privileged component (N3).
//!
//! Test doubles are test-only: production commits require a configured
//! supervisor-backed executor, and [`commit_needs_executor`] refuses without
//! one before any worker or accepted-state write.

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use sley_id::{
    CandidateId, NativeAdmissionProfileId, PrincipalId, ReceiptId, StateRoot, TransactionId,
    WorkspaceId,
};
use sley_policy::ValidatedCandidatePlan;
use sley_tests::{
    HistoricalTrustPolicyV1, NATIVE_WALL_CAP_MILLIS, NativeTestApprovalV1, NativeTestPlanV1,
    NativeTestReportV1, ROLE_ACCEPTANCE, ROLE_MEASUREMENT,
};

use crate::codec::TransactionErrorCode;

/// Exact attempt-journal record magic.
pub const JOURNAL_MAGIC: [u8; 8] = *b"SLEYNAT1";
/// Exact journal record version.
pub const JOURNAL_VERSION: u64 = 1;
/// Journal directory under the repository root, outside semantic roots.
pub const ATTEMPTS_DIR: &str = "attempts";
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

/// Test-execution dispatch: the qualified supervisor connection in
/// production, an explicitly test-only double in tests.
///
/// The executor receives the owner-derived plan and the validator-owned
/// proposed state it was derived from. It must return exactly one evidence
/// pair per selected test in plan order; the commit owner re-verifies every
/// binding and never trusts executor claims about selection.
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
/// workspace, execution profile, and historical run time.
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
    execution_profile: sley_id::NativeExecutionProfileId,
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
        execution_profile.as_bytes(),
        historical_time,
    ) {
        Ok(())
    } else {
        Err(NativeCommitError::TrustRejected)
    }
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
/// # Errors
///
/// Returns `TXN_RECEIPT_BINDING_MISMATCH` for count, order, identity, plan
/// binding, report linkage, configuration, or duplicate-cover divergence.
pub(crate) fn check_execution_coverage(
    plan: &NativeTestPlanV1,
    executions: &[ExecutedNativeTest],
) -> Result<(), TransactionErrorCode> {
    use sley_tests::NativeExecutionEvidence;
    if executions.len() != plan.selected().len() {
        return Err(TransactionErrorCode::ReceiptBindingMismatch);
    }
    let mut configs = BTreeSet::new();
    for execution in executions {
        let config = sley_tests::SupervisorConfigV1::parse(&execution.supervisor_config_stored)
            .map_err(|_| TransactionErrorCode::ReceiptBindingMismatch)?;
        configs.insert(*config.id().as_bytes());
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
        if execution_report.plan_id() != plan.plan_id()
            || execution_report.test_entity() != entry.test_entity
            || execution_report.test_object() != entry.test_object
            || attestation.plan_id() != plan.plan_id()
            || attestation.test_object() != entry.test_object
            || !configs.contains(&attestation.supervisor_config_id())
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
    root.join(ATTEMPTS_DIR)
        .join(format!("{}{}", attempt_id.hex(), ATTEMPT_SUFFIX))
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
    let final_path = directory.join(format!("{}{}", record.attempt_id.hex(), ATTEMPT_SUFFIX));
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
    Ok(())
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
