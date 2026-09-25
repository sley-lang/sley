//! Canonical SLEY native v2 transaction and complete-receipt records (N5a).
//!
//! Format 2 carries exactly the format-1 transaction fields 1-19 with native
//! metadata plus field 20 (`NativeTestApprovalId`), and exactly the format-1
//! receipt fields 1-9 plus field 10 (complete native evidence bundle),
//! field 11 (`HistoricalAdmissionContextV1`) and field 12 (signed
//! `CommitAdmissionStatementV1`). Context and statement are mandatory even for
//! empty selections; the receipt stores bytes, never external paths.
//!
//! The format-1 builders and importers are untouched: they keep refusing
//! nonempty test references and v2-magicked bytes. New native APIs build v2,
//! and [`ImportedReceipt`] dispatches both formats behind common relationship
//! accessors so mixed v1/v2 ancestry verifies without format contagion. No
//! v2 genesis exists: genesis remains format 1.
//!
//! Native receipt bounds are enforced here, ahead of any acceptance: at most
//! 256 selected tests per native commit, at most 256 KiB per execution
//! report, and a checked aggregate evidence cap of exactly 48 MiB covering
//! bundle, context, and statement together so candidate, root, policy,
//! manifest, and envelope still fit the 64 MiB standalone ceiling. These
//! stricter native implementation bounds do not change epoch-1 decoder
//! limits.
//!
//! Signature handling is structural in this slice: the statement and
//! attestation envelopes parse, key IDs bind to the recorded trust policies,
//! and roles/intervals are checked by later owners. The native commit and
//! exchange owners strictly verify the embedded Ed25519 signatures after
//! this structural codec has reconstructed their canonical preimages.

use std::collections::BTreeSet;

use sley_id::{
    CandidateId, CandidateResultId, CapabilitySummaryDigest, CommitAdmissionStatementId, EntityId,
    HistoricalAdmissionContextId, NativeEvidenceBundleId, NativeTestApprovalId, ObjectId,
    PolicyRootId, PrincipalId, ReceiptId, SchemaEpochId, StateRoot, TestReportId, TransactionId,
    ValidationProfileId, WorkspaceId,
};
use sley_scb1::{ScbError, ScbErrorCode, encode_bytes, encode_list};
use sley_tests::{
    CommitAdmissionStatementV1, HistoricalAdmissionContextV1, MAX_EXECUTION_REPORT_STORED,
    NativeEvidenceBundleV1, NativeTestApprovalV1, NativeTestPlanV1, NativeTestReportV1,
};

use crate::codec::{
    ChangedBinding, CommitMetadata, ImportedTransactionReceipt, MANIFEST_FIELD_COUNT,
    ObjectManifestEntry, TransactionCodecError, TransactionErrorCode, TransactionKind,
    append_digest, decode_changed_bindings, decode_commit_metadata, decode_envelope,
    decode_fixed_list, decode_list_payloads, decode_option_bytes, decode_option_fixed,
    decode_required_record, encode_changed_binding, encode_commit_metadata, encode_envelope,
    encode_fixed_list, encode_option_bytes, encode_option_fixed, read_bytes, read_fixed, read_u32,
    read_u64, txn_error, validate_changed_bindings, validate_manifest, validate_sorted_ids,
};
use sley_mutate::ImportedCandidate;
use sley_policy::{
    AcceptedPolicyRoot, CandidateDecision, ImportedCandidateResult, ValidationContextDigest,
    import_candidate_result, import_policy_root,
};
use sley_state_root::{AcceptedStateRoot, import_state_root};

/// Exact native transaction-core envelope magic.
pub const NATIVE_TRANSACTION_MAGIC: [u8; 8] = *b"SLEYTXN2";
/// Exact native complete-receipt envelope magic.
pub const NATIVE_RECEIPT_MAGIC: [u8; 8] = *b"SLEYRCP2";
/// Exact native envelope and record format version.
pub const NATIVE_FORMAT_VERSION: u32 = 2;
/// Exact native transaction record field count: v1 fields plus approval.
pub const NATIVE_TRANSACTION_FIELD_COUNT: u64 = 20;
/// Exact native receipt record field count: v1 fields plus bundle, context,
/// statement.
pub const NATIVE_RECEIPT_FIELD_COUNT: u64 = 12;
/// Maximum selected tests per native commit (admission §5).
pub const MAX_NATIVE_SELECTED_TESTS: usize = 256;
/// Maximum aggregate evidence bytes: bundle, context, and statement together.
///
/// Exactly 48 MiB, matching the bundle record ceiling, so candidate, root,
/// policy, manifest, and envelope still fit the 64 MiB standalone ceiling.
pub const MAX_NATIVE_EVIDENCE_BYTES: usize = 50_331_648;

/// Native v2 transaction record: v1 fields 1-19 plus field 20.
///
/// `selected_tests` carries the native plan's selected set (possibly empty
/// for the explicit empty selection) and `test_result_refs` carries exactly
/// one native report ID, including the explicit empty report. Only the
/// ordinary-candidate kind is a v2 wire state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeTransactionRecord {
    /// Exact record format version (always 2).
    pub format_version: u32,
    /// Closed transaction kind (always ordinary candidate).
    pub transaction_kind: TransactionKind,
    /// Exact workspace.
    pub workspace_id: WorkspaceId,
    /// Ordered transaction parents (exactly one for ordinary commits).
    pub parent_transaction_ids: Vec<TransactionId>,
    /// Ordered semantic roots aligned with the transaction parents.
    pub parent_roots: Vec<StateRoot>,
    /// Exact semantic schema epoch.
    pub schema_epoch_id: SchemaEpochId,
    /// Exact protected policy root.
    pub policy_root_id: PolicyRootId,
    /// Authenticated principal for ordinary commits.
    pub principal_id: Option<PrincipalId>,
    /// Exact candidate identity for ordinary commits.
    pub candidate_id: Option<CandidateId>,
    /// Exact fresh commit-time result identity for ordinary commits.
    pub candidate_result_id: Option<CandidateResultId>,
    /// Exact fresh trusted validation-context digest.
    pub validation_context_digest: Option<ValidationContextDigest>,
    /// Exact validation profile.
    pub validation_profile_id: Option<ValidationProfileId>,
    /// Ancestry-independent committed semantic root.
    pub committed_root: StateRoot,
    /// Exact sorted binding diff.
    pub changed_entity_bindings: Vec<ChangedBinding>,
    /// Authenticated capability projection for ordinary commits.
    pub capability_summary_digest: Option<CapabilitySummaryDigest>,
    /// Validator-selected tests: exactly the native plan's selected set.
    pub selected_tests: Vec<EntityId>,
    /// Exact test report references: exactly one native report ID.
    pub test_result_refs: Vec<TestReportId>,
    /// Complete non-reusable identity ledger after this transaction.
    pub tombstoned_entities: Vec<EntityId>,
    /// Deterministic profile metadata.
    pub commit_metadata: CommitMetadata,
    /// Native approval authorizing the test evidence (field 20).
    pub native_approval_id: NativeTestApprovalId,
}

/// Canonical native transaction bytes and derived identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedNativeTransaction {
    /// Strictly decoded record.
    pub record: NativeTransactionRecord,
    /// Derived parent-bound revision identity.
    pub transaction_id: TransactionId,
    /// Exact digest preimage.
    pub preimage: Vec<u8>,
    /// Exact bytes including the transaction trailer.
    pub stored_bytes: Vec<u8>,
}

/// Native v2 receipt record: v1 fields 1-9 plus fields 10-12.
///
/// Field 10 is the complete native evidence bundle, field 11 the historical
/// admission context, field 12 the signed commit-admission statement. All
/// three are mandatory even for empty selections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeTransactionReceiptRecord {
    /// Exact record format version (always 2).
    pub format_version: u32,
    /// Transaction lookup and revision identity.
    pub transaction_id: TransactionId,
    /// Exact canonical native transaction bytes including trailer.
    pub stored_transaction: Vec<u8>,
    /// Exact candidate bytes for ordinary commits.
    pub stored_candidate: Option<Vec<u8>>,
    /// Exact fresh commit-time result bytes for ordinary commits.
    pub stored_candidate_result: Option<Vec<u8>>,
    /// Exact registry-authorized committed root bytes.
    pub stored_state_root: Vec<u8>,
    /// Exact registry-authorized protected policy bytes.
    pub stored_policy_root: Vec<u8>,
    /// Exact sorted changed-postimage object manifest.
    pub object_manifest: Vec<ObjectManifestEntry>,
    /// Exact receipt-before-head durability profile.
    pub durability_profile: u32,
    /// Complete native evidence bundle bytes (field 10).
    pub stored_evidence_bundle: Vec<u8>,
    /// Historical admission context bytes (field 11).
    pub stored_admission_context: Vec<u8>,
    /// Signed commit-admission statement bytes (field 12).
    pub stored_commit_statement: Vec<u8>,
}

/// Strictly verified native receipt with parsed evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedNativeTransactionReceipt {
    /// Strictly decoded outer record.
    pub record: NativeTransactionReceiptRecord,
    /// Independently authenticated receipt identity.
    pub receipt_id: ReceiptId,
    /// Exact outer receipt digest preimage.
    pub preimage: Vec<u8>,
    /// Exact outer bytes including receipt trailer.
    pub stored_bytes: Vec<u8>,
    /// Strictly imported nested native transaction.
    pub transaction: ImportedNativeTransaction,
    /// Strictly imported candidate.
    pub candidate: ImportedCandidate,
    /// Shape-verified, non-authoritative nested candidate result.
    pub candidate_result: ImportedCandidateResult,
    /// Registry-authorized committed semantic root.
    pub state_root: AcceptedStateRoot,
    /// Registry-authorized protected policy root.
    pub policy_root: AcceptedPolicyRoot,
    /// Parsed and cross-bound native evidence bundle.
    pub bundle: NativeEvidenceBundleV1,
    /// Parsed historical admission context.
    pub context: HistoricalAdmissionContextV1,
    /// Parsed commit-admission statement.
    pub statement: CommitAdmissionStatementV1,
}

/// One pinned native test with its exact proposed objects.
///
/// The repository owner resolves each pin against the committed root's
/// entity bindings and the object store; this summary carries what the
/// receipt evidence bound, not an independent re-derivation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeEvidencePin {
    /// Selected test entity.
    pub test_entity: EntityId,
    /// Exact proposed test object.
    pub test_object: ObjectId,
    /// Target function entity.
    pub target_function: EntityId,
    /// Exact proposed target object.
    pub target_object: ObjectId,
}

/// Receipt evidence bindings verified without trusted history access.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeEvidenceSummary {
    /// Approved test plan identity.
    pub plan_id: sley_id::NativeTestPlanId,
    /// Deterministic test report identity.
    pub report_id: TestReportId,
    /// Native approval identity.
    pub approval_id: NativeTestApprovalId,
    /// Evidence bundle identity.
    pub bundle_id: NativeEvidenceBundleId,
    /// Historical admission context identity.
    pub context_id: HistoricalAdmissionContextId,
    /// Commit-admission statement identity.
    pub statement_id: CommitAdmissionStatementId,
    /// Approved static subset the fresh result must name exactly.
    pub static_selected: Vec<EntityId>,
    /// Pinned tests in test-ID order.
    pub pins: Vec<NativeEvidencePin>,
}

/// Admitted evidence budget accounting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeEvidenceBudget {
    /// Checked aggregate evidence bytes admitted.
    pub aggregate_bytes: usize,
    /// Checked largest single execution envelope admitted.
    pub largest_report_bytes: usize,
    /// Admitted selected-test count.
    pub selected_count: usize,
}

/// Returns the exact sorted trust policy IDs referenced by one native
/// receipt: the acceptance statement's manifest plus every embedded
/// measurement attestation's manifest, deduplicated.
///
/// This is the receipt's contribution to the repository exchange trust
/// union. It authorizes nothing by itself; importers match it exactly
/// against caller-supplied manifests without installing trust.
///
/// # Errors
///
/// Returns the first nested attestation envelope failure.
pub fn native_receipt_trust_policy_ids(
    receipt: &ImportedNativeTransactionReceipt,
) -> Result<Vec<[u8; 32]>, TransactionCodecError> {
    let mut ids = BTreeSet::new();
    ids.insert(
        *receipt
            .statement
            .parts()
            .acceptance_trust_policy_id
            .as_bytes(),
    );
    for embedded in receipt.bundle.measurements() {
        let attestation = sley_tests::MeasuredTestAttestationV1::parse(&embedded.stored)?;
        ids.insert(attestation.trust_policy_id());
    }
    Ok(ids.into_iter().collect())
}

/// Versioned imported receipt behind common relationship accessors.
///
/// Mixed v1/v2 ancestry verifies through these accessors without format
/// contagion: a parent link only requires identity agreement, never format
/// equality.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportedReceipt {
    /// Format-1 receipt, boxed: both verified receipt variants exceed the
    /// inline-variant threshold and only move on import paths.
    V1(Box<ImportedTransactionReceipt>),
    /// Format-2 native receipt with parsed evidence, boxed likewise.
    V2(Box<ImportedNativeTransactionReceipt>),
}

impl ImportedReceipt {
    /// Returns the record format version (1 or 2).
    #[must_use]
    pub const fn format_version(&self) -> u32 {
        match self {
            Self::V1(receipt) => receipt.record.format_version,
            Self::V2(receipt) => receipt.record.format_version,
        }
    }

    /// Returns the nested transaction identity.
    #[must_use]
    pub const fn transaction_id(&self) -> TransactionId {
        match self {
            Self::V1(receipt) => receipt.transaction.transaction_id,
            Self::V2(receipt) => receipt.transaction.transaction_id,
        }
    }

    /// Returns the independently authenticated receipt identity.
    #[must_use]
    pub const fn receipt_id(&self) -> ReceiptId {
        match self {
            Self::V1(receipt) => receipt.receipt_id,
            Self::V2(receipt) => receipt.receipt_id,
        }
    }

    /// Returns the ancestry-independent committed semantic root.
    #[must_use]
    pub const fn committed_root(&self) -> StateRoot {
        match self {
            Self::V1(receipt) => receipt.transaction.record.committed_root,
            Self::V2(receipt) => receipt.transaction.record.committed_root,
        }
    }

    /// Returns the ordered transaction parents.
    #[must_use]
    pub fn parent_transaction_ids(&self) -> &[TransactionId] {
        match self {
            Self::V1(receipt) => &receipt.transaction.record.parent_transaction_ids,
            Self::V2(receipt) => &receipt.transaction.record.parent_transaction_ids,
        }
    }

    /// Returns the exact outer receipt bytes including trailer.
    #[must_use]
    pub fn stored_bytes(&self) -> &[u8] {
        match self {
            Self::V1(receipt) => &receipt.stored_bytes,
            Self::V2(receipt) => &receipt.stored_bytes,
        }
    }

    /// Returns the registry-authorized committed state root.
    #[must_use]
    pub const fn state_root(&self) -> &sley_state_root::AcceptedStateRoot {
        match self {
            Self::V1(receipt) => &receipt.state_root,
            Self::V2(receipt) => &receipt.state_root,
        }
    }

    /// Returns the registry-authorized protected policy root.
    #[must_use]
    pub const fn policy_root(&self) -> &sley_policy::AcceptedPolicyRoot {
        match self {
            Self::V1(receipt) => &receipt.policy_root,
            Self::V2(receipt) => &receipt.policy_root,
        }
    }

    /// Returns the strictly imported candidate, when the format carries one.
    #[must_use]
    pub const fn candidate(&self) -> Option<&sley_mutate::ImportedCandidate> {
        match self {
            Self::V1(receipt) => receipt.candidate.as_ref(),
            Self::V2(receipt) => Some(&receipt.candidate),
        }
    }

    /// Returns the complete non-reusable identity ledger.
    #[must_use]
    pub fn tombstoned_entities(&self) -> &[EntityId] {
        match self {
            Self::V1(receipt) => &receipt.transaction.record.tombstoned_entities,
            Self::V2(receipt) => &receipt.transaction.record.tombstoned_entities,
        }
    }

    /// Returns the exact workspace this receipt was accepted in.
    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        match self {
            Self::V1(receipt) => receipt.transaction.record.workspace_id,
            Self::V2(receipt) => receipt.transaction.record.workspace_id,
        }
    }

    /// Returns the transaction kind (genesis exists only in format 1:
    /// no v2 genesis is a wire state).
    #[must_use]
    pub const fn transaction_kind(&self) -> TransactionKind {
        match self {
            Self::V1(receipt) => receipt.transaction.record.transaction_kind,
            Self::V2(receipt) => receipt.transaction.record.transaction_kind,
        }
    }
}

/// Builds a canonical native v2 transaction core.
///
/// Genesis is not a v2 wire state: only the ordinary-candidate kind builds.
/// `test_result_refs` must name exactly one native report ID, including the
/// explicit empty report; `selected_tests` may be empty only for the
/// explicit empty selection the receipt evidence proves.
///
/// # Errors
///
/// Returns the first shape, kind, count, or binding failure.
pub fn build_native_transaction(
    record: &NativeTransactionRecord,
) -> Result<ImportedNativeTransaction, TransactionCodecError> {
    validate_native_transaction_record(record)?;
    let payload = encode_native_transaction_record(record)?;
    let preimage =
        encode_envelope(NATIVE_TRANSACTION_MAGIC, &payload).map_err(TransactionCodecError::Scb)?;
    let transaction_id = TransactionId::derive(&preimage);
    let stored_bytes =
        append_digest(&preimage, transaction_id.as_bytes()).map_err(TransactionCodecError::Scb)?;
    Ok(ImportedNativeTransaction {
        record: record.clone(),
        transaction_id,
        preimage,
        stored_bytes,
    })
}

/// Imports and validates native v2 transaction bytes.
///
/// # Errors
///
/// Returns the first envelope, digest, shape, kind, or binding failure.
pub fn import_native_transaction(
    input: &[u8],
) -> Result<ImportedNativeTransaction, TransactionCodecError> {
    let envelope = decode_envelope(input, NATIVE_TRANSACTION_MAGIC)?;
    let transaction_id = TransactionId::derive(envelope.preimage);
    if envelope.trailer != transaction_id.as_bytes() {
        return Err(ScbError::new(ScbErrorCode::DigestMismatch).into());
    }
    let record = decode_native_transaction_record(envelope.payload)?;
    validate_native_transaction_record(&record)?;
    Ok(ImportedNativeTransaction {
        record,
        transaction_id,
        preimage: envelope.preimage.to_vec(),
        stored_bytes: input.to_vec(),
    })
}

/// Builds a canonical native v2 complete receipt.
///
/// Evidence parses, bounds, and cross-bindings are checked before any nested
/// candidate bytes are trusted; nested roots, candidate, and result then go
/// through the same registry authorization as format 1.
///
/// # Errors
///
/// Returns the first format, envelope, digest, evidence, bound, binding, or
/// nested-registry failure.
pub fn build_native_transaction_receipt(
    record: &NativeTransactionReceiptRecord,
) -> Result<ImportedNativeTransactionReceipt, TransactionCodecError> {
    if record.format_version != NATIVE_FORMAT_VERSION {
        return Err(txn_error(TransactionErrorCode::FormatVersion));
    }
    if record.durability_profile != crate::codec::DURABILITY_PROFILE_RECEIPT_BEFORE_HEAD_V1 {
        return Err(txn_error(TransactionErrorCode::FieldShape));
    }
    validate_manifest(&record.object_manifest)?;

    let transaction = import_native_transaction(&record.stored_transaction)?;
    if transaction.transaction_id != record.transaction_id {
        return Err(txn_error(TransactionErrorCode::ReceiptBindingMismatch));
    }
    let summary = verify_native_evidence_bindings(
        transaction.transaction_id,
        &transaction.record,
        &record.stored_evidence_bundle,
        &record.stored_admission_context,
        &record.stored_commit_statement,
    )?;
    let bundle = NativeEvidenceBundleV1::parse(&record.stored_evidence_bundle)
        .map_err(TransactionCodecError::Scb)?;
    let context = HistoricalAdmissionContextV1::parse(&record.stored_admission_context)
        .map_err(TransactionCodecError::Scb)?;
    let statement = CommitAdmissionStatementV1::parse(&record.stored_commit_statement)
        .map_err(TransactionCodecError::Scb)?;
    debug_assert_eq!(bundle.id(), summary.bundle_id);
    debug_assert_eq!(context.id(), summary.context_id);
    debug_assert_eq!(statement.id(), summary.statement_id);

    let state_registry = sley_state_root::conformance_registry()
        .map_err(sley_state_root::StateRootError::from)
        .map_err(TransactionCodecError::StateRoot)?;
    let state_root = import_state_root(&state_registry, &record.stored_state_root)?;
    let policy_registry = sley_policy::conformance_registry()
        .map_err(sley_policy::PolicyRootError::from)
        .map_err(TransactionCodecError::PolicyRoot)?;
    let policy_root = import_policy_root(&policy_registry, &record.stored_policy_root)?;

    if transaction.record.committed_root != state_root.root
        || transaction.record.workspace_id != state_root.record.workspace_id
        || transaction.record.schema_epoch_id != state_root.record.schema_epoch_id
        || transaction.record.policy_root_id != state_root.record.policy_root
        || transaction.record.policy_root_id != policy_root.root()
        || transaction.record.workspace_id != policy_root.record().workspace_id
        || transaction.record.commit_metadata.durability_profile != record.durability_profile
    {
        return Err(txn_error(TransactionErrorCode::ReceiptBindingMismatch));
    }

    let candidate = record
        .stored_candidate
        .as_deref()
        .map(sley_mutate::import_candidate)
        .transpose()?;
    let candidate_result = record
        .stored_candidate_result
        .as_deref()
        .map(import_candidate_result)
        .transpose()?;
    match transaction.record.transaction_kind {
        TransactionKind::TrustedGenesis => {
            return Err(txn_error(TransactionErrorCode::InternalInvariant));
        }
        TransactionKind::OrdinaryCandidate => {
            let candidate = candidate
                .as_ref()
                .ok_or_else(|| txn_error(TransactionErrorCode::ReceiptBindingMismatch))?;
            let result = candidate_result
                .as_ref()
                .ok_or_else(|| txn_error(TransactionErrorCode::ReceiptBindingMismatch))?;
            validate_native_ordinary_nested(&transaction.record, candidate, result, &summary)?;
        }
    }
    validate_native_manifest_binding(&transaction.record, &record.object_manifest)?;

    let payload = encode_native_receipt_record(record)?;
    let preimage =
        encode_envelope(NATIVE_RECEIPT_MAGIC, &payload).map_err(TransactionCodecError::Scb)?;
    let receipt_id = ReceiptId::derive(&preimage);
    let stored_bytes =
        append_digest(&preimage, receipt_id.as_bytes()).map_err(TransactionCodecError::Scb)?;
    Ok(ImportedNativeTransactionReceipt {
        record: record.clone(),
        receipt_id,
        preimage,
        stored_bytes,
        transaction,
        candidate: candidate
            .ok_or_else(|| txn_error(TransactionErrorCode::ReceiptBindingMismatch))?,
        candidate_result: candidate_result
            .ok_or_else(|| txn_error(TransactionErrorCode::ReceiptBindingMismatch))?,
        state_root,
        policy_root,
        bundle,
        context,
        statement,
    })
}

/// Imports and cross-verifies a persisted native v2 receipt.
///
/// Rebuilds the identical checks as the builder over the stored bytes; the
/// derived receipt identity must match the trailer.
///
/// # Errors
///
/// Returns the first envelope, digest, evidence, bound, binding, or
/// nested-registry failure.
pub fn import_native_transaction_receipt(
    input: &[u8],
) -> Result<ImportedNativeTransactionReceipt, TransactionCodecError> {
    let envelope = decode_envelope(input, NATIVE_RECEIPT_MAGIC)?;
    let receipt_id = ReceiptId::derive(envelope.preimage);
    if envelope.trailer != receipt_id.as_bytes() {
        return Err(ScbError::new(ScbErrorCode::DigestMismatch).into());
    }
    let record = decode_native_receipt_record(envelope.payload)?;
    let rebuilt = build_native_transaction_receipt(&record)?;
    if rebuilt.receipt_id != receipt_id {
        return Err(txn_error(TransactionErrorCode::ReceiptBindingMismatch));
    }
    Ok(rebuilt)
}

/// Imports either receipt format behind the versioned receipt enum.
///
/// Closed dispatch on the envelope magic: format-1 bytes take the frozen v1
/// path unchanged, format-2 bytes take the native path. Unknown magics
/// refuse exactly like a wrong-magic envelope.
///
/// # Errors
///
/// Returns the first envelope or nested failure of the selected format.
pub fn import_receipt_any(input: &[u8]) -> Result<ImportedReceipt, TransactionCodecError> {
    if input.len() < NATIVE_TRANSACTION_MAGIC.len() {
        return Err(ScbError::new(ScbErrorCode::LengthOverflow).into());
    }
    let mut magic = [0_u8; 8];
    magic.copy_from_slice(&input[..8]);
    if magic == crate::codec::RECEIPT_MAGIC {
        return crate::codec::import_transaction_receipt(input)
            .map(|receipt| ImportedReceipt::V1(Box::new(receipt)));
    }
    if magic == NATIVE_RECEIPT_MAGIC {
        return import_native_transaction_receipt(input)
            .map(|receipt| ImportedReceipt::V2(Box::new(receipt)));
    }
    Err(ScbError::new(ScbErrorCode::MagicInvalid).into())
}

/// Checks native evidence bounds with checked arithmetic.
///
/// Counts the selected tests, every execution envelope, and the aggregate
/// bundle/context/statement bytes against the frozen admission ceilings.
/// Lengths come from the caller so unknown-large inputs refuse before any
/// allocation the bounds themselves would forbid.
///
/// # Errors
///
/// Returns `TXN_RESOURCE_LIMIT` for any ceiling breach or for arithmetic
/// that would overflow before the comparison.
pub fn check_native_evidence_bounds(
    bundle_len: usize,
    context_len: usize,
    statement_len: usize,
    execution_lens: &[usize],
    selected_count: usize,
) -> Result<NativeEvidenceBudget, TransactionCodecError> {
    if selected_count > MAX_NATIVE_SELECTED_TESTS {
        return Err(txn_error(TransactionErrorCode::ResourceLimit));
    }
    let mut largest_report_bytes = 0_usize;
    for &len in execution_lens {
        if len > MAX_EXECUTION_REPORT_STORED {
            return Err(txn_error(TransactionErrorCode::ResourceLimit));
        }
        largest_report_bytes = largest_report_bytes.max(len);
    }
    let aggregate_bytes = bundle_len
        .checked_add(context_len)
        .and_then(|sum| sum.checked_add(statement_len))
        .ok_or_else(|| txn_error(TransactionErrorCode::ResourceLimit))?;
    if aggregate_bytes > MAX_NATIVE_EVIDENCE_BYTES {
        return Err(txn_error(TransactionErrorCode::ResourceLimit));
    }
    Ok(NativeEvidenceBudget {
        aggregate_bytes,
        largest_report_bytes,
        selected_count,
    })
}

/// Verifies receipt evidence cross-bindings without trusted history access.
///
/// Parses bundle, context, and statement through their owning codecs,
/// enforces the evidence bounds, then checks every cross-binding the receipt
/// claims: approval covers the bundled plan and report, the report covers
/// the plan, the transaction names the plan's selected set and the single
/// report, and the statement binds transaction, approval, bundle, context,
/// candidate, result, parents, and committed root.
///
/// # Errors
///
/// Returns the first envelope, bound, or binding failure.
pub(crate) fn verify_native_evidence_bindings(
    transaction_id: TransactionId,
    transaction: &NativeTransactionRecord,
    stored_bundle: &[u8],
    stored_context: &[u8],
    stored_statement: &[u8],
) -> Result<NativeEvidenceSummary, TransactionCodecError> {
    let bundle =
        NativeEvidenceBundleV1::parse(stored_bundle).map_err(TransactionCodecError::Scb)?;
    let context =
        HistoricalAdmissionContextV1::parse(stored_context).map_err(TransactionCodecError::Scb)?;
    let statement =
        CommitAdmissionStatementV1::parse(stored_statement).map_err(TransactionCodecError::Scb)?;
    let plan = NativeTestPlanV1::parse(bundle.plan_stored()).map_err(TransactionCodecError::Scb)?;
    let approval = NativeTestApprovalV1::parse(bundle.approval_stored())
        .map_err(TransactionCodecError::Scb)?;
    let report = NativeTestReportV1::parse(bundle.test_report_stored())
        .map_err(TransactionCodecError::Scb)?;

    let execution_lens = bundle
        .executions()
        .iter()
        .map(|item| item.stored.len())
        .collect::<Vec<_>>();
    check_native_evidence_bounds(
        stored_bundle.len(),
        stored_context.len(),
        stored_statement.len(),
        &execution_lens,
        transaction.selected_tests.len(),
    )?;

    let selected = plan
        .selected()
        .iter()
        .map(|entry| entry.test_entity)
        .collect::<Vec<_>>();
    if approval.plan_id() != plan.plan_id()
        || approval.test_report_id() != report.report_id()
        || report.plan_id() != plan.plan_id()
        || report.proposed_root() != plan.proposed_root()
        || transaction.selected_tests != selected
        || transaction.test_result_refs != [report.report_id()]
        || transaction.native_approval_id != approval.id()
        || transaction.committed_root != plan.proposed_root()
    {
        return Err(txn_error(TransactionErrorCode::ReceiptBindingMismatch));
    }
    report
        .verify_plan_coverage(&plan)
        .map_err(TransactionCodecError::Scb)?;
    if statement.transaction_id() != transaction_id
        || statement.native_approval_id() != approval.id()
        || statement.bundle_id() != bundle.id()
        || statement.historical_context_id() != context.id()
        || approval.historical_context_id() != context.id().as_bytes()
    {
        return Err(txn_error(TransactionErrorCode::ReceiptBindingMismatch));
    }
    let pins = plan
        .selected()
        .iter()
        .map(|entry| NativeEvidencePin {
            test_entity: entry.test_entity,
            test_object: entry.test_object,
            target_function: entry.target_function,
            target_object: entry.target_object,
        })
        .collect::<Vec<_>>();
    Ok(NativeEvidenceSummary {
        plan_id: plan.plan_id(),
        report_id: report.report_id(),
        approval_id: approval.id(),
        bundle_id: bundle.id(),
        context_id: context.id(),
        statement_id: statement.id(),
        static_selected: plan.static_selected_ids().to_vec(),
        pins,
    })
}

fn validate_native_transaction_record(
    record: &NativeTransactionRecord,
) -> Result<(), TransactionCodecError> {
    if record.format_version != NATIVE_FORMAT_VERSION {
        return Err(txn_error(TransactionErrorCode::FormatVersion));
    }
    if record.parent_transaction_ids.len() != record.parent_roots.len() {
        return Err(txn_error(TransactionErrorCode::ParentShape));
    }
    validate_sorted_ids(&record.selected_tests, TransactionErrorCode::FieldShape)?;
    validate_sorted_ids(
        &record.tombstoned_entities,
        TransactionErrorCode::TombstoneInvalid,
    )?;
    if record.test_result_refs.len() != 1 {
        return Err(txn_error(TransactionErrorCode::FieldShape));
    }
    validate_changed_bindings(&record.changed_entity_bindings, record.transaction_kind)?;
    // Genesis remains format 1: no v2 genesis shortcut exists.
    if record.transaction_kind != TransactionKind::OrdinaryCandidate {
        return Err(txn_error(TransactionErrorCode::FieldShape));
    }
    // The native record carries exactly the spec-fixed native metadata:
    // commit profile 2, semantic profile 3, receipt-before-head durability 1.
    // v1 triples are not v2 wire states, and v1 decoders already refuse the
    // native triple, so neither format partially interprets the other.
    if record.commit_metadata != CommitMetadata::native_v1() {
        return Err(txn_error(TransactionErrorCode::FieldShape));
    }
    if record.parent_transaction_ids.len() != 1
        || record.principal_id.is_none()
        || record.candidate_id.is_none()
        || record.candidate_result_id.is_none()
        || record.validation_context_digest.is_none()
        || record.validation_profile_id.is_none()
        || record.capability_summary_digest.is_none()
    {
        return Err(txn_error(TransactionErrorCode::ParentShape));
    }
    Ok(())
}

fn validate_native_ordinary_nested(
    transaction: &NativeTransactionRecord,
    candidate: &ImportedCandidate,
    result: &ImportedCandidateResult,
    summary: &NativeEvidenceSummary,
) -> Result<(), TransactionCodecError> {
    if result.record.decision != CandidateDecision::Valid {
        return Err(txn_error(TransactionErrorCode::ResultNotValid));
    }
    let Some(parent_transaction_id) = transaction.parent_transaction_ids.first() else {
        return Err(txn_error(TransactionErrorCode::ParentShape));
    };
    let Some(parent_root) = transaction.parent_roots.first() else {
        return Err(txn_error(TransactionErrorCode::ParentShape));
    };
    // Unlike format 1, the fresh static result's selected set is the static
    // selection, while the transaction carries the stronger native selection.
    // The result must name exactly the plan's approved static subset.
    if result.record.selected_tests != summary.static_selected {
        return Err(txn_error(TransactionErrorCode::ResultBindingMismatch));
    }
    if transaction.candidate_id != Some(candidate.candidate_id)
        || transaction.candidate_result_id != Some(result.candidate_result_id)
        || transaction.validation_context_digest != Some(result.record.validation_context_digest)
        || transaction.validation_profile_id != Some(result.record.validation_profile_id)
        || transaction.committed_root
            != result
                .record
                .candidate_root
                .ok_or_else(|| txn_error(TransactionErrorCode::ResultBindingMismatch))?
        || result.record.candidate_id != Some(candidate.candidate_id)
        || candidate.record.base_transaction_id != *parent_transaction_id
        || candidate.record.base_root != *parent_root
        || candidate.record.workspace_id != transaction.workspace_id
        || candidate.record.schema_epoch_id != transaction.schema_epoch_id
        || candidate.record.policy_root_id != transaction.policy_root_id
        || transaction.principal_id != Some(candidate.record.principal_id)
        || transaction.capability_summary_digest != Some(candidate.record.capability_summary_digest)
        || transaction.validation_profile_id != Some(candidate.record.validation_profile_id)
    {
        return Err(txn_error(TransactionErrorCode::ResultBindingMismatch));
    }
    Ok(())
}

fn validate_native_manifest_binding(
    transaction: &NativeTransactionRecord,
    manifest: &[ObjectManifestEntry],
) -> Result<(), TransactionCodecError> {
    let expected = transaction
        .changed_entity_bindings
        .iter()
        .filter_map(|binding| binding.postimage)
        .collect::<BTreeSet<_>>();
    let actual = manifest
        .iter()
        .map(|entry| entry.object_id)
        .collect::<BTreeSet<_>>();
    if expected == actual {
        Ok(())
    } else {
        Err(txn_error(TransactionErrorCode::ObjectInventoryMismatch))
    }
}

fn encode_native_transaction_record(
    record: &NativeTransactionRecord,
) -> Result<Vec<u8>, TransactionCodecError> {
    use sley_scb1::{encode_record, encode_uvar};
    let parents = encode_fixed_list(
        record
            .parent_transaction_ids
            .iter()
            .map(TransactionId::as_bytes),
    )?;
    let parent_roots = encode_fixed_list(record.parent_roots.iter().map(StateRoot::as_bytes))?;
    let changed = record
        .changed_entity_bindings
        .iter()
        .map(encode_changed_binding)
        .collect::<Result<Vec<_>, _>>()?;
    let selected = encode_fixed_list(record.selected_tests.iter().map(EntityId::as_bytes))?;
    let reports = encode_fixed_list(record.test_result_refs.iter().map(TestReportId::as_bytes))?;
    let tombstones = encode_fixed_list(record.tombstoned_entities.iter().map(EntityId::as_bytes))?;
    encode_record(&[
        (1, encode_uvar(u64::from(record.format_version))),
        (2, encode_uvar(u64::from(record.transaction_kind.tag()))),
        (3, record.workspace_id.as_bytes().to_vec()),
        (4, parents),
        (5, parent_roots),
        (6, record.schema_epoch_id.as_bytes().to_vec()),
        (7, record.policy_root_id.as_bytes().to_vec()),
        (
            8,
            encode_option_fixed(record.principal_id.as_ref().map(PrincipalId::as_bytes))?,
        ),
        (
            9,
            encode_option_fixed(record.candidate_id.as_ref().map(CandidateId::as_bytes))?,
        ),
        (
            10,
            encode_option_fixed(
                record
                    .candidate_result_id
                    .as_ref()
                    .map(CandidateResultId::as_bytes),
            )?,
        ),
        (
            11,
            encode_option_fixed(
                record
                    .validation_context_digest
                    .as_ref()
                    .map(ValidationContextDigest::as_bytes),
            )?,
        ),
        (
            12,
            encode_option_fixed(
                record
                    .validation_profile_id
                    .as_ref()
                    .map(ValidationProfileId::as_bytes),
            )?,
        ),
        (13, record.committed_root.as_bytes().to_vec()),
        (14, encode_list(&changed)?),
        (
            15,
            encode_option_fixed(
                record
                    .capability_summary_digest
                    .as_ref()
                    .map(CapabilitySummaryDigest::as_bytes),
            )?,
        ),
        (16, selected),
        (17, reports),
        (18, tombstones),
        (19, encode_commit_metadata(record.commit_metadata)?),
        (20, record.native_approval_id.as_bytes().to_vec()),
    ])
    .map_err(TransactionCodecError::Scb)
}

fn decode_native_transaction_record(
    input: &[u8],
) -> Result<NativeTransactionRecord, TransactionCodecError> {
    let fields = decode_required_record(input, NATIVE_TRANSACTION_FIELD_COUNT)?;
    Ok(NativeTransactionRecord {
        format_version: read_u32(fields[0])?,
        transaction_kind: TransactionKind::from_tag(read_u32(fields[1])?)
            .ok_or_else(|| txn_error(TransactionErrorCode::KindInvalid))?,
        workspace_id: WorkspaceId::from_bytes(read_fixed(fields[2])?),
        parent_transaction_ids: decode_fixed_list(fields[3])?
            .into_iter()
            .map(TransactionId::from_bytes)
            .collect(),
        parent_roots: decode_fixed_list(fields[4])?
            .into_iter()
            .map(StateRoot::from_bytes)
            .collect(),
        schema_epoch_id: SchemaEpochId::from_bytes(read_fixed(fields[5])?),
        policy_root_id: PolicyRootId::from_bytes(read_fixed(fields[6])?),
        principal_id: decode_option_fixed(fields[7])?.map(PrincipalId::from_bytes),
        candidate_id: decode_option_fixed(fields[8])?.map(CandidateId::from_bytes),
        candidate_result_id: decode_option_fixed(fields[9])?.map(CandidateResultId::from_bytes),
        validation_context_digest: decode_option_fixed(fields[10])?
            .map(ValidationContextDigest::from_bytes),
        validation_profile_id: decode_option_fixed(fields[11])?
            .map(ValidationProfileId::from_bytes),
        committed_root: StateRoot::from_bytes(read_fixed(fields[12])?),
        changed_entity_bindings: decode_changed_bindings(fields[13])?,
        capability_summary_digest: decode_option_fixed(fields[14])?
            .map(CapabilitySummaryDigest::from_bytes),
        selected_tests: decode_fixed_list(fields[15])?
            .into_iter()
            .map(EntityId::from_bytes)
            .collect(),
        test_result_refs: decode_fixed_list(fields[16])?
            .into_iter()
            .map(TestReportId::from_bytes)
            .collect(),
        tombstoned_entities: decode_fixed_list(fields[17])?
            .into_iter()
            .map(EntityId::from_bytes)
            .collect(),
        commit_metadata: decode_commit_metadata(fields[18])?,
        native_approval_id: NativeTestApprovalId::from_bytes(read_fixed(fields[19])?),
    })
}

fn encode_native_receipt_record(
    record: &NativeTransactionReceiptRecord,
) -> Result<Vec<u8>, TransactionCodecError> {
    use sley_scb1::{encode_record, encode_uvar};
    let manifest = record
        .object_manifest
        .iter()
        .map(|entry| {
            encode_record(&[
                (1, entry.object_id.as_bytes().to_vec()),
                (2, encode_uvar(entry.stored_length)),
            ])
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(TransactionCodecError::Scb)?;
    encode_record(&[
        (1, encode_uvar(u64::from(record.format_version))),
        (2, record.transaction_id.as_bytes().to_vec()),
        (3, encode_bytes(&record.stored_transaction)?),
        (4, encode_option_bytes(record.stored_candidate.as_deref())?),
        (
            5,
            encode_option_bytes(record.stored_candidate_result.as_deref())?,
        ),
        (6, encode_bytes(&record.stored_state_root)?),
        (7, encode_bytes(&record.stored_policy_root)?),
        (8, encode_list(&manifest)?),
        (9, encode_uvar(u64::from(record.durability_profile))),
        (10, encode_bytes(&record.stored_evidence_bundle)?),
        (11, encode_bytes(&record.stored_admission_context)?),
        (12, encode_bytes(&record.stored_commit_statement)?),
    ])
    .map_err(TransactionCodecError::Scb)
}

fn decode_native_receipt_record(
    input: &[u8],
) -> Result<NativeTransactionReceiptRecord, TransactionCodecError> {
    let fields = decode_required_record(input, NATIVE_RECEIPT_FIELD_COUNT)?;
    let object_manifest = decode_list_payloads(fields[7])?
        .into_iter()
        .map(|payload| {
            let fields = decode_required_record(payload, MANIFEST_FIELD_COUNT)?;
            Ok(ObjectManifestEntry {
                object_id: ObjectId::from_bytes(read_fixed(fields[0])?),
                stored_length: read_u64(fields[1])?,
            })
        })
        .collect::<Result<Vec<_>, TransactionCodecError>>()?;
    Ok(NativeTransactionReceiptRecord {
        format_version: read_u32(fields[0])?,
        transaction_id: TransactionId::from_bytes(read_fixed(fields[1])?),
        stored_transaction: read_bytes(fields[2])?,
        stored_candidate: decode_option_bytes(fields[3])?,
        stored_candidate_result: decode_option_bytes(fields[4])?,
        stored_state_root: read_bytes(fields[5])?,
        stored_policy_root: read_bytes(fields[6])?,
        object_manifest,
        durability_profile: read_u32(fields[8])?,
        stored_evidence_bundle: read_bytes(fields[9])?,
        stored_admission_context: read_bytes(fields[10])?,
        stored_commit_statement: read_bytes(fields[11])?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::DURABILITY_PROFILE_RECEIPT_BEFORE_HEAD_V1;

    fn id<T>(byte: u8, constructor: impl FnOnce([u8; 32]) -> T) -> T {
        constructor([byte; 32])
    }

    fn ordinary_record() -> NativeTransactionRecord {
        NativeTransactionRecord {
            format_version: NATIVE_FORMAT_VERSION,
            transaction_kind: TransactionKind::OrdinaryCandidate,
            workspace_id: id(1, WorkspaceId::from_bytes),
            parent_transaction_ids: vec![id(7, TransactionId::from_bytes)],
            parent_roots: vec![id(8, StateRoot::from_bytes)],
            schema_epoch_id: id(2, SchemaEpochId::from_bytes),
            policy_root_id: id(3, PolicyRootId::from_bytes),
            principal_id: Some(id(9, PrincipalId::from_bytes)),
            candidate_id: Some(id(10, CandidateId::from_bytes)),
            candidate_result_id: Some(id(11, CandidateResultId::from_bytes)),
            validation_context_digest: Some(id(12, ValidationContextDigest::from_bytes)),
            validation_profile_id: Some(id(13, ValidationProfileId::from_bytes)),
            committed_root: id(4, StateRoot::from_bytes),
            changed_entity_bindings: vec![ChangedBinding {
                entity_id: id(5, EntityId::from_bytes),
                preimage: None,
                postimage: Some(id(6, ObjectId::from_bytes)),
                mutation_ordinals: vec![0],
            }],
            capability_summary_digest: Some(id(14, CapabilitySummaryDigest::from_bytes)),
            selected_tests: vec![id(15, EntityId::from_bytes)],
            test_result_refs: vec![id(16, TestReportId::from_bytes)],
            tombstoned_entities: Vec::new(),
            commit_metadata: CommitMetadata::native_v1(),
            native_approval_id: id(17, NativeTestApprovalId::from_bytes),
        }
    }

    fn decode_hex(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex decodes"))
            .collect()
    }

    fn golden(field: &str) -> Vec<u8> {
        let text = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../sley-tests/golden/native-final-golden.json"
        ));
        let marker = format!("\"{field}\": \"");
        let start = text.find(&marker).expect("golden field present") + marker.len();
        let end = text[start..].find('"').expect("golden field ends") + start;
        decode_hex(&text[start..end])
    }

    fn golden_bundle() -> Vec<u8> {
        golden("bundle_stored")
    }

    fn golden_context() -> Vec<u8> {
        golden("context_stored")
    }

    fn golden_statement() -> Vec<u8> {
        golden("statement_stored")
    }

    /// Rebinds the golden statement to a freshly built core identity.
    ///
    /// The golden statement binds the vector script's synthetic core; the
    /// real committer builds the core first and obtains the statement over
    /// its derived identity, so the fixture repeats exactly that rebind.
    /// Signature bytes stay the opaque golden vector: structural only.
    fn golden_statement_for(core_id: TransactionId) -> Vec<u8> {
        let parsed =
            CommitAdmissionStatementV1::parse(&golden_statement()).expect("statement parses");
        let mut parts = *parsed.parts();
        parts.transaction_id = core_id;
        CommitAdmissionStatementV1::build(parts)
            .expect("rebind builds")
            .stored_bytes()
            .to_vec()
    }

    /// Builds a v2 core whose bindings match the golden empty-selection
    /// evidence: candidate, result, parents, roots, principal, and report
    /// come from the parsed golden parts, so only the evidence under test
    /// can refuse. Validation-context, profile, and capability digests stay
    /// synthetic here: they bind the fresh static result, which only the
    /// N5b commit path verifies against live validation output.
    fn golden_core() -> (NativeTransactionRecord, TransactionId) {
        let bundle = NativeEvidenceBundleV1::parse(&golden_bundle()).expect("golden bundle parses");
        let plan = NativeTestPlanV1::parse(bundle.plan_stored()).expect("golden plan parses");
        let approval =
            NativeTestApprovalV1::parse(bundle.approval_stored()).expect("golden parses approval");
        let report =
            NativeTestReportV1::parse(bundle.test_report_stored()).expect("golden parses report");
        let statement =
            CommitAdmissionStatementV1::parse(&golden_statement()).expect("statement parses");
        let mut record = ordinary_record();
        record.workspace_id = plan.workspace();
        record.parent_transaction_ids = vec![plan.parent_transaction()];
        record.parent_roots = vec![plan.parent_root()];
        record.schema_epoch_id = plan.semantic_epoch();
        record.policy_root_id = plan.policy_root();
        record.candidate_id = Some(approval.parts().candidate);
        record.candidate_result_id = Some(approval.parts().static_result);
        record.principal_id = Some(statement.parts().principal);
        record.committed_root = plan.proposed_root();
        record.selected_tests = plan
            .selected()
            .iter()
            .map(|entry| entry.test_entity)
            .collect();
        record.test_result_refs = vec![report.report_id()];
        record.native_approval_id = approval.id();
        let built = build_native_transaction(&record).expect("golden core builds");
        (record, built.transaction_id)
    }

    #[test]
    fn native_transaction_round_trip_is_parent_bound_and_non_cyclic() {
        let built = build_native_transaction(&ordinary_record()).unwrap();
        assert!(built.stored_bytes.starts_with(&NATIVE_TRANSACTION_MAGIC));
        assert_eq!(
            import_native_transaction(&built.stored_bytes).unwrap(),
            built
        );
        assert_eq!(built.transaction_id, TransactionId::derive(&built.preimage));
        assert!(
            !built
                .preimage
                .windows(32)
                .any(|window| { window == built.transaction_id.as_bytes() })
        );
    }

    #[test]
    fn native_transaction_rejects_genesis_and_bad_shapes() {
        let mut genesis = ordinary_record();
        genesis.transaction_kind = TransactionKind::TrustedGenesis;
        assert_eq!(
            build_native_transaction(&genesis).unwrap_err().code(),
            "TXN_FIELD_SHAPE"
        );

        let mut no_refs = ordinary_record();
        no_refs.test_result_refs = Vec::new();
        assert_eq!(
            build_native_transaction(&no_refs).unwrap_err().code(),
            "TXN_FIELD_SHAPE"
        );

        let mut two_refs = ordinary_record();
        two_refs
            .test_result_refs
            .push(id(18, TestReportId::from_bytes));
        assert_eq!(
            build_native_transaction(&two_refs).unwrap_err().code(),
            "TXN_FIELD_SHAPE"
        );

        let mut unsorted = ordinary_record();
        unsorted.selected_tests = vec![id(15, EntityId::from_bytes), id(15, EntityId::from_bytes)];
        assert_eq!(
            build_native_transaction(&unsorted).unwrap_err().code(),
            "TXN_FIELD_SHAPE"
        );

        let mut legacy = ordinary_record();
        legacy.format_version = 1;
        assert_eq!(
            build_native_transaction(&legacy).unwrap_err().code(),
            "TXN_FORMAT_VERSION"
        );

        let mut anonymous = ordinary_record();
        anonymous.principal_id = None;
        assert_eq!(
            build_native_transaction(&anonymous).unwrap_err().code(),
            "TXN_PARENT_SHAPE"
        );

        // The explicit empty selection builds: emptiness is proven by the
        // receipt evidence, not by the core shape.
        let mut empty = ordinary_record();
        empty.selected_tests = Vec::new();
        build_native_transaction(&empty).expect("empty selection is a core shape");
    }

    #[test]
    fn native_transaction_requires_the_spec_fixed_native_metadata() {
        // Appendix A fixes the v2 metadata to commit profile 2, semantic
        // profile 3, durability 1: v1 triples are not v2 wire states.
        let mut restricted = ordinary_record();
        restricted.commit_metadata = CommitMetadata::restricted_v1();
        assert_eq!(
            build_native_transaction(&restricted).unwrap_err().code(),
            "TXN_FIELD_SHAPE"
        );
        let mut extended = ordinary_record();
        extended.commit_metadata = CommitMetadata::extended_operations_v1();
        assert_eq!(
            build_native_transaction(&extended).unwrap_err().code(),
            "TXN_FIELD_SHAPE"
        );
        // And the v1 path refuses the native triple in turn: neither format
        // partially interprets the other.
        let legacy = crate::codec::build_transaction(&crate::codec::TransactionRecord {
            format_version: 1,
            transaction_kind: TransactionKind::OrdinaryCandidate,
            workspace_id: id(1, WorkspaceId::from_bytes),
            parent_transaction_ids: vec![id(7, TransactionId::from_bytes)],
            parent_roots: vec![id(8, StateRoot::from_bytes)],
            schema_epoch_id: id(2, SchemaEpochId::from_bytes),
            policy_root_id: id(3, PolicyRootId::from_bytes),
            principal_id: Some(id(9, PrincipalId::from_bytes)),
            candidate_id: Some(id(10, CandidateId::from_bytes)),
            candidate_result_id: Some(id(11, CandidateResultId::from_bytes)),
            validation_context_digest: Some(id(12, ValidationContextDigest::from_bytes)),
            validation_profile_id: Some(id(13, ValidationProfileId::from_bytes)),
            committed_root: id(4, StateRoot::from_bytes),
            changed_entity_bindings: Vec::new(),
            capability_summary_digest: Some(id(14, CapabilitySummaryDigest::from_bytes)),
            selected_tests: Vec::new(),
            test_result_refs: Vec::new(),
            tombstoned_entities: Vec::new(),
            commit_metadata: CommitMetadata::native_v1(),
        });
        assert_eq!(legacy.unwrap_err().code(), "TXN_FIELD_SHAPE");
    }

    #[test]
    fn v1_importers_refuse_v2_bytes_and_vice_versa() {
        let native = build_native_transaction(&ordinary_record()).unwrap();
        assert_eq!(
            crate::codec::import_transaction(&native.stored_bytes)
                .unwrap_err()
                .code(),
            "SCB_MAGIC_INVALID"
        );
        let mut receipt_magic = NATIVE_RECEIPT_MAGIC.to_vec();
        receipt_magic.extend_from_slice(&[0_u8; 40]);
        assert_eq!(
            crate::codec::import_transaction_receipt(&receipt_magic)
                .unwrap_err()
                .code(),
            "SCB_MAGIC_INVALID"
        );

        let legacy = crate::codec::build_transaction(&crate::codec::TransactionRecord {
            format_version: 1,
            transaction_kind: TransactionKind::TrustedGenesis,
            workspace_id: id(1, WorkspaceId::from_bytes),
            parent_transaction_ids: Vec::new(),
            parent_roots: Vec::new(),
            schema_epoch_id: id(2, SchemaEpochId::from_bytes),
            policy_root_id: id(3, PolicyRootId::from_bytes),
            principal_id: None,
            candidate_id: None,
            candidate_result_id: None,
            validation_context_digest: None,
            validation_profile_id: None,
            committed_root: id(4, StateRoot::from_bytes),
            changed_entity_bindings: Vec::new(),
            capability_summary_digest: None,
            selected_tests: Vec::new(),
            test_result_refs: Vec::new(),
            tombstoned_entities: Vec::new(),
            commit_metadata: CommitMetadata::restricted_v1(),
        })
        .unwrap();
        assert_eq!(
            import_native_transaction(&legacy.stored_bytes)
                .unwrap_err()
                .code(),
            "SCB_MAGIC_INVALID"
        );
    }

    #[test]
    fn evidence_bounds_are_exact_at_every_ceiling() {
        let ok = check_native_evidence_bounds(1024, 512, 256, &[262_144], 256).unwrap();
        assert_eq!(ok.aggregate_bytes, 1792);
        assert_eq!(ok.largest_report_bytes, 262_144);
        assert_eq!(ok.selected_count, 256);

        assert_eq!(
            check_native_evidence_bounds(1024, 512, 256, &[262_144], 257)
                .unwrap_err()
                .code(),
            "TXN_RESOURCE_LIMIT"
        );
        assert_eq!(
            check_native_evidence_bounds(1024, 512, 256, &[262_145], 1)
                .unwrap_err()
                .code(),
            "TXN_RESOURCE_LIMIT"
        );
        assert_eq!(
            check_native_evidence_bounds(MAX_NATIVE_EVIDENCE_BYTES, 1, 0, &[], 0)
                .unwrap_err()
                .code(),
            "TXN_RESOURCE_LIMIT"
        );
        check_native_evidence_bounds(MAX_NATIVE_EVIDENCE_BYTES, 0, 0, &[], 0)
            .expect("exact aggregate ceiling admits");
        assert_eq!(
            check_native_evidence_bounds(usize::MAX, 1, 0, &[], 0)
                .unwrap_err()
                .code(),
            "TXN_RESOURCE_LIMIT"
        );
    }

    #[test]
    fn golden_empty_evidence_bindings_verify_with_exact_summary() {
        let (record, core_id) = golden_core();
        let summary = verify_native_evidence_bindings(
            core_id,
            &record,
            &golden_bundle(),
            &golden_context(),
            &golden_statement_for(core_id),
        )
        .expect("golden empty evidence binds");
        assert_eq!(summary.bundle_id.as_bytes().to_vec(), golden("bundle_id"));
        assert_eq!(summary.context_id.as_bytes().to_vec(), golden("context_id"));
        assert_eq!(
            summary.approval_id.as_bytes().to_vec(),
            golden("approval_id")
        );
        assert_eq!(summary.report_id.as_bytes().to_vec(), golden("report_id"));
        assert_eq!(summary.plan_id.as_bytes().to_vec(), golden("plan_id"));
        // The statement is rebound to the fresh core, so its identity is the
        // rebound envelope's, not the vector script's synthetic one.
        let rebound =
            CommitAdmissionStatementV1::parse(&golden_statement_for(core_id)).expect("rebound");
        assert_eq!(summary.statement_id, rebound.id());
        let plan = NativeTestPlanV1::parse(&golden_bundle_plan_stored()).expect("plan parses");
        assert_eq!(summary.static_selected, plan.static_selected_ids().to_vec());
        assert_eq!(summary.pins.len(), record.selected_tests.len());
    }

    fn golden_bundle_plan_stored() -> Vec<u8> {
        NativeEvidenceBundleV1::parse(&golden_bundle())
            .expect("bundle parses")
            .plan_stored()
            .to_vec()
    }

    #[test]
    fn evidence_binding_mutations_refuse_before_nested_trust() {
        let (_, probe_core) = golden_core();
        let statement_bytes = golden_statement_for(probe_core);
        let bind = |record: &NativeTransactionRecord,
                    core_id: TransactionId,
                    statement: &[u8]|
         -> Result<NativeEvidenceSummary, TransactionCodecError> {
            verify_native_evidence_bindings(
                core_id,
                record,
                &golden_bundle(),
                &golden_context(),
                statement,
            )
        };
        let (mut record, core_id) = golden_core();

        // Wrong report reference.
        record.test_result_refs = vec![id(77, TestReportId::from_bytes)];
        assert_eq!(
            bind(&record, core_id, &statement_bytes).unwrap_err().code(),
            "TXN_RECEIPT_BINDING_MISMATCH"
        );

        // Selected set diverges from the plan.
        let (mut record, core_id) = golden_core();
        record.selected_tests.push(id(78, EntityId::from_bytes));
        assert_eq!(
            bind(&record, core_id, &statement_bytes).unwrap_err().code(),
            "TXN_RECEIPT_BINDING_MISMATCH"
        );

        // Committed root diverges from the proposed root.
        let (mut record, core_id) = golden_core();
        record.committed_root = id(79, StateRoot::from_bytes);
        assert_eq!(
            bind(&record, core_id, &statement_bytes).unwrap_err().code(),
            "TXN_RECEIPT_BINDING_MISMATCH"
        );

        // Approval identity diverges.
        let (mut record, core_id) = golden_core();
        record.native_approval_id = id(80, NativeTestApprovalId::from_bytes);
        assert_eq!(
            bind(&record, core_id, &statement_bytes).unwrap_err().code(),
            "TXN_RECEIPT_BINDING_MISMATCH"
        );

        // Statement rebound to a different bundle refuses.
        let statement = CommitAdmissionStatementV1::parse(&golden_statement()).unwrap();
        let mut parts = *statement.parts();
        parts.bundle_id = sley_id::NativeEvidenceBundleId::from_bytes([81_u8; 32]);
        let rebound = CommitAdmissionStatementV1::build(parts).unwrap();
        let (record, core_id) = golden_core();
        assert_eq!(
            bind(&record, core_id, rebound.stored_bytes())
                .unwrap_err()
                .code(),
            "TXN_RECEIPT_BINDING_MISMATCH"
        );

        // Statement rebound to a different transaction core refuses.
        let mut parts = *statement.parts();
        parts.transaction_id = id(82, TransactionId::from_bytes);
        let rebound = CommitAdmissionStatementV1::build(parts).unwrap();
        assert_eq!(
            bind(&record, core_id, rebound.stored_bytes())
                .unwrap_err()
                .code(),
            "TXN_RECEIPT_BINDING_MISMATCH"
        );

        // Missing statement refuses even for the empty selection: context
        // and statement are mandatory, never omitted evidence fields.
        assert!(bind(&record, core_id, &[]).is_err());
        // Corrupt context refuses before any nested bytes are trusted.
        let mut corrupt = golden_context();
        corrupt[10] ^= 1;
        assert!(
            verify_native_evidence_bindings(
                core_id,
                &record,
                &golden_bundle(),
                &corrupt,
                &statement_bytes,
            )
            .is_err()
        );
    }

    #[test]
    fn receipt_builder_refuses_garbage_nested_bytes_without_accepting() {
        let (record, core_id) = golden_core();
        let core = build_native_transaction(&record).expect("core builds");
        assert_eq!(core.transaction_id, core_id);
        let receipt = NativeTransactionReceiptRecord {
            format_version: NATIVE_FORMAT_VERSION,
            transaction_id: core_id,
            stored_transaction: core.stored_bytes.clone(),
            stored_candidate: Some(vec![0_u8; 16]),
            stored_candidate_result: Some(vec![0_u8; 16]),
            stored_state_root: vec![0_u8; 16],
            stored_policy_root: vec![0_u8; 16],
            object_manifest: Vec::new(),
            durability_profile: DURABILITY_PROFILE_RECEIPT_BEFORE_HEAD_V1,
            stored_evidence_bundle: golden_bundle(),
            stored_admission_context: golden_context(),
            stored_commit_statement: golden_statement_for(core_id),
        };
        // Evidence is valid, so refusal names the nested registry failure:
        // nothing is accepted on garbage history bytes.
        let refusal = build_native_transaction_receipt(&receipt)
            .expect_err("garbage nested refuses")
            .code();
        assert!(!refusal.is_empty());
        assert_ne!(refusal, "TXN_RECEIPT_BINDING_MISMATCH");
    }

    #[test]
    fn receipt_dispatch_routes_both_magics_without_contagion() {
        // v1-magic garbage takes the v1 path: any error except wrong-magic.
        let mut v1_garbage = crate::codec::RECEIPT_MAGIC.to_vec();
        v1_garbage.extend_from_slice(&[0_u8; 40]);
        let code = crate::codec::import_transaction_receipt(&v1_garbage)
            .expect_err("v1 garbage refuses")
            .code();
        assert_ne!(code, "SCB_MAGIC_INVALID");
        let dispatch_code = import_receipt_any(&v1_garbage)
            .expect_err("dispatch refuses v1 garbage on v1 path")
            .code();
        assert_eq!(dispatch_code, code);

        // v2-magic garbage takes the v2 path: any error except wrong-magic.
        let mut v2_garbage = NATIVE_RECEIPT_MAGIC.to_vec();
        v2_garbage.extend_from_slice(&[0_u8; 40]);
        let dispatch_code = import_receipt_any(&v2_garbage)
            .expect_err("dispatch refuses v2 garbage on v2 path")
            .code();
        assert_ne!(dispatch_code, "SCB_MAGIC_INVALID");

        // Unknown magic refuses as wrong-magic; short input as length.
        assert_eq!(
            import_receipt_any(&[9_u8; 40]).unwrap_err().code(),
            "SCB_MAGIC_INVALID"
        );
        assert!(import_receipt_any(&[9_u8; 4]).is_err());
    }
}
