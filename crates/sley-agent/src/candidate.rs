//! Candidate assembly, in-process validation, and workspace handles.
//!
//! The workbench fills every envelope field mechanically from the accepted
//! head and the policy grant, derives created identities under the record
//! nonce, and binds each operation's precondition in operation order. The
//! kernel's `validate_candidate_bytes` is the only judge.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sley_id::{CandidateNonce, EntityId, PrincipalId};
use sley_mutate::{
    BoundPrecondition, CandidateExpiry, CandidateRecord, ExactContainerVersion, ExactEntityVersion,
    ExpectedIdentityAbsent, ImportedCandidate, MutationClass, MutationOperation, MutationPayload,
    PreconditionPayload,
};
use sley_policy::{
    CandidateValidationContext, CandidateValidationLimits, CandidateValidationOutput,
    PolicyResourceCeilings,
};

use crate::error::{AgentError, AgentErrorCode, Result, io};
use crate::hex;
use crate::workspace::{Head, Program, Workspace};

/// Far-future candidate expiry (2100-01-01): candidates never expire in a
/// workspace; the base binding is what goes stale.
pub const CANDIDATE_EXPIRY_MILLIS: u64 = 4_102_444_800_000;

/// The principal a workspace authors as, with its grant's ceilings.
#[derive(Clone, Debug)]
pub struct Authority {
    /// Principal identity.
    pub principal: PrincipalId,
    /// The grant's resource ceilings.
    pub ceilings: PolicyResourceCeilings,
    /// The grant's allowed mutation class tags.
    pub classes: Vec<u32>,
}

impl Authority {
    /// Chooses the authoring principal: the only grant, else the first grant
    /// that allows `CreateEntity` and `ReplaceEntityVersion`.
    ///
    /// # Errors
    ///
    /// `AGENT_WORKSPACE_INVALID` when the policy grants no principal that
    /// can author candidates.
    pub fn of(head: &Head) -> Result<Self> {
        let grants = &head.policy_root().record().principal_grants;
        let usable = |grant: &sley_policy::PrincipalGrant| {
            let classes = grant.allowed_mutation_class_tags();
            classes.binary_search(&1).is_ok() && classes.binary_search(&2).is_ok()
        };
        let chosen = if grants.len() == 1 {
            grants.first()
        } else {
            grants.iter().find(|(_, grant)| usable(grant))
        };
        let (principal, grant) = chosen.ok_or_else(|| {
            AgentError::new(
                AgentErrorCode::WorkspaceInvalid,
                "the policy root grants no principal that can author candidates",
            )
        })?;
        Ok(Self {
            principal: *principal,
            ceilings: grant.resource_ceilings(),
            classes: grant.allowed_mutation_class_tags().to_vec(),
        })
    }
}

/// A fresh candidate nonce from the operating system.
///
/// # Errors
///
/// `AGENT_IO_FAILED` when no randomness is available.
pub fn fresh_nonce() -> Result<CandidateNonce> {
    Ok(CandidateNonce::from_bytes(random32()?))
}

/// Thirty-two random bytes (`/dev/urandom`).
///
/// # Errors
///
/// `AGENT_IO_FAILED` when no randomness is available.
pub fn random32() -> Result<[u8; 32]> {
    use std::io::Read as _;
    let path = Path::new("/dev/urandom");
    let mut bytes = [0_u8; 32];
    fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|error| io(path, &error))?;
    Ok(bytes)
}

/// One planned mutation: class payload plus its target.
#[derive(Clone, Debug)]
pub struct PlannedOp {
    /// Entity kind of the target.
    pub kind: u16,
    /// Target identity (derived for creates).
    pub target: EntityId,
    /// Class payload.
    pub payload: MutationPayload,
    /// Field tag for field-level classes.
    pub field_tag: Option<u32>,
}

/// Derives the identity the `ordinal`-th create of a record receives.
#[must_use]
pub fn created_id(program: &Program, nonce: CandidateNonce, kind: u16, ordinal: u64) -> EntityId {
    EntityId::derive(program.workspace(), nonce, u32::from(kind), ordinal)
}

/// Assembles and frames one candidate record over the accepted head.
///
/// # Errors
///
/// `AGENT_CANDIDATE_INVALID` with the kernel's symbol when the record
/// cannot be built, or when a non-create targets an entity that is not live.
pub fn assemble(
    head: &Head,
    authority: &Authority,
    nonce: CandidateNonce,
    ops: Vec<PlannedOp>,
) -> Result<ImportedCandidate> {
    let state = head.state_root();
    let program = head.program();
    let mut operations = Vec::with_capacity(ops.len());
    let mut preconditions = Vec::with_capacity(ops.len());
    for (index, op) in ops.into_iter().enumerate() {
        let ordinal = u32::try_from(index).map_err(|_| {
            AgentError::new(AgentErrorCode::CandidateInvalid, "too many operations")
        })?;
        let class = op.payload.class();
        let payload = if class == MutationClass::CreateEntity {
            PreconditionPayload::ExpectedIdentityAbsent(ExpectedIdentityAbsent {
                entity_id: op.target,
            })
        } else {
            let object = program.object(&op.target).ok_or_else(|| {
                AgentError::new(
                    AgentErrorCode::CandidateInvalid,
                    format!(
                        "operation {ordinal} targets {} which is not live",
                        hex::encode(op.target.as_bytes())
                    ),
                )
            })?;
            match (class, op.field_tag) {
                (
                    MutationClass::InsertOrderedChild
                    | MutationClass::RemoveOrderedChild
                    | MutationClass::MoveOrderedChild,
                    Some(field_tag),
                ) => PreconditionPayload::ExactContainerVersion(ExactContainerVersion {
                    container_id: op.target,
                    object_id: object.object_id(),
                    field_tag,
                }),
                _ => PreconditionPayload::ExactEntityVersion(ExactEntityVersion {
                    entity_id: op.target,
                    object_id: object.object_id(),
                }),
            }
        };
        preconditions.push(BoundPrecondition {
            operation_ordinal: ordinal,
            requirement: payload.requirement(),
            payload,
        });
        operations.push(MutationOperation {
            ordinal,
            class,
            target_kind: op.kind,
            target_entity: op.target,
            field_tag: op.field_tag,
            payload: op.payload,
            precondition_ordinal: ordinal,
        });
    }
    let kernel = |symbol: String| AgentError::new(AgentErrorCode::CandidateInvalid, symbol);
    let capability = sley_policy::build_capability_summary_projection(
        authority.principal,
        state.record.workspace_id,
        head.policy_root().root(),
        state.root,
        &[],
    )
    .map_err(|error| kernel(format!("capability summary: {error}")))?;
    let record = CandidateRecord {
        format_version: 1,
        workspace_id: state.record.workspace_id,
        base_transaction_id: head.transaction_id(),
        base_root: state.root,
        schema_epoch_id: state.record.schema_epoch_id,
        policy_root_id: head.policy_root().root(),
        principal_id: authority.principal,
        capability_summary_digest: capability.digest(),
        operations,
        preconditions,
        validation_profile_id: sley_mutate::full_validation_profile_id()
            .map_err(|error| kernel(error.to_string()))?,
        candidate_nonce: nonce,
        expiry: CandidateExpiry::unix_millis(CANDIDATE_EXPIRY_MILLIS),
    };
    sley_mutate::build_candidate(&record).map_err(|error| kernel(error.to_string()))
}

/// Validates stored candidate bytes against the accepted head, in process.
///
/// # Errors
///
/// `AGENT_CANDIDATE_INVALID` only when the validator cannot render a result
/// at all; candidate invalidity is a result, not an error.
pub fn validate(
    head: &Head,
    authority: &Authority,
    stored: &[u8],
) -> Result<CandidateValidationOutput> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        });
    let context = CandidateValidationContext::new(
        head.transaction_id(),
        head.state_root(),
        head.objects(),
        head.tombstones(),
        head.policy_root(),
        authority.principal,
        &[],
        now,
        CandidateValidationLimits::full_v1(),
    )
    .map_err(|error| AgentError::new(AgentErrorCode::CandidateInvalid, format!("{error:?}")))?;
    sley_policy::validate_candidate_bytes(&context, stored)
        .map_err(|error| AgentError::new(AgentErrorCode::CandidateInvalid, format!("{error:?}")))
}

/// The program a Valid candidate proposes.
#[must_use]
pub fn proposed_program(head: &Head, output: &CandidateValidationOutput) -> Option<Program> {
    let plan = output.validated_plan()?;
    Some(Program::new(
        head.program().epoch(),
        plan.candidate_root().root,
        head.program().workspace(),
        plan.proposed_state().entities().to_vec(),
    ))
}

/// The post-candidate state for display even when the candidate is refused:
/// the pure apply of its operations to the head (no validation claim).
#[must_use]
pub fn applied_program(head: &Head, candidate: &ImportedCandidate) -> Option<Program> {
    let proposed = sley_mutate::apply_candidate_to_snapshot(
        candidate.record.schema_epoch_id,
        &candidate.record,
        head.objects(),
        &head.state_root().record.entry_points,
    )
    .ok()?;
    Some(Program::new(
        head.program().epoch(),
        head.program().root(),
        head.program().workspace(),
        proposed.entities().to_vec(),
    ))
}

/// Candidate storage under `.sley/candidates/`, addressed by short handles
/// (`c1`, `c2`, ...).
#[derive(Clone, Debug)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    /// Opens (creating) the store of a workspace.
    ///
    /// # Errors
    ///
    /// `AGENT_IO_FAILED` when the directory cannot be created.
    pub fn open(workspace: &Workspace) -> Result<Self> {
        let dir = workspace.state_dir()?.join("candidates");
        fs::create_dir_all(&dir).map_err(|error| io(&dir, &error))?;
        Ok(Self { dir })
    }

    fn handles(&self) -> Result<Vec<u64>> {
        let entries = fs::read_dir(&self.dir).map_err(|error| io(&self.dir, &error))?;
        let mut handles = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(number) = name
                .strip_prefix('c')
                .and_then(|rest| rest.strip_suffix(".hex"))
                .and_then(|digits| digits.parse().ok())
            {
                handles.push(number);
            }
        }
        handles.sort_unstable();
        Ok(handles)
    }

    /// Stores candidate bytes under the next handle.
    ///
    /// # Errors
    ///
    /// `AGENT_IO_FAILED` when the store cannot be written.
    pub fn save(&self, stored: &[u8], meta: &serde_json::Value) -> Result<String> {
        let next = self.handles()?.last().copied().unwrap_or(0) + 1;
        let handle = format!("c{next}");
        let path = self.dir.join(format!("{handle}.hex"));
        fs::write(&path, format!("{}\n", hex::encode(stored)))
            .map_err(|error| io(&path, &error))?;
        let meta_path = self.dir.join(format!("{handle}.json"));
        let mut text = serde_json::to_string_pretty(meta).unwrap_or_default();
        text.push('\n');
        fs::write(&meta_path, text).map_err(|error| io(&meta_path, &error))?;
        Ok(handle)
    }

    /// Loads the stored bytes a handle (`c3`), a file path, or raw stored
    /// hex denotes.
    ///
    /// # Errors
    ///
    /// `AGENT_HANDLE_UNKNOWN` when nothing matches.
    pub fn load(&self, reference: &str) -> Result<Vec<u8>> {
        let unknown = || {
            AgentError::new(
                AgentErrorCode::HandleUnknown,
                format!("`{reference}` is not a candidate handle, file, or stored hex"),
            )
        };
        let text =
            if reference.starts_with('c') && reference[1..].chars().all(|c| c.is_ascii_digit()) {
                let path = self.dir.join(format!("{reference}.hex"));
                fs::read_to_string(&path).map_err(|_| unknown())?
            } else if Path::new(reference).is_file() {
                fs::read_to_string(reference).map_err(|error| io(Path::new(reference), &error))?
            } else {
                reference.to_owned()
            };
        let bytes = hex::decode(text.trim()).ok_or_else(unknown)?;
        if bytes.starts_with(b"SLEYCAN1") {
            return Ok(bytes);
        }
        // A bare candidate record (what the 2.0.0 trial tool printed as
        // `record`): frame it through the kernel.
        sley_mutate::decode_candidate_record(&bytes)
            .and_then(|record| sley_mutate::build_candidate(&record))
            .map(|candidate| candidate.stored_bytes)
            .map_err(|_| unknown())
    }

    /// Reads a handle's metadata, if any.
    #[must_use]
    pub fn meta(&self, handle: &str) -> Option<serde_json::Value> {
        let text = fs::read_to_string(self.dir.join(format!("{handle}.json"))).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// The most recent handle.
    ///
    /// # Errors
    ///
    /// `AGENT_IO_FAILED` when the store cannot be listed.
    pub fn latest(&self) -> Result<Option<String>> {
        Ok(self.handles()?.last().map(|number| format!("c{number}")))
    }
}
