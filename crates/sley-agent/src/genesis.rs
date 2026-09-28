//! Local workspace lifecycle: `init` (a fresh trusted genesis), `commit`
//! (a Valid candidate into the local repository), and `export` (the
//! repository as an exchange pack).
//!
//! `init` is the local equivalent of `workspace.create`: it writes one
//! genesis whose policy grants its principal the workbench defaults. It
//! never touches an existing repository. `commit` goes through the kernel's
//! transaction engine, which validates the candidate again.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use sley_id::{GenesisNonce, PrincipalId, TransactionId, WorkspaceId};
use sley_mutate::value::{EntityBodyValue, EntityIdSet, NamespaceBody};
use sley_mutate::{EntityObjectRecord, MutationClass, build_entity_object};
use sley_policy::{
    CandidateValidationLimits, PolicyResourceCeilings, PolicyRootBuilder, PrincipalGrantBuilder,
};
use sley_state_root::StateRootBuilder;
use sley_store::ObjectStore;
use sley_txn::{CommitInput, TransactionRepository, TrustedGenesisInput};

use crate::candidate::Authority;
use crate::error::{AgentError, AgentErrorCode, Result, io};
use crate::workspace::{self, Head, REPO_DIR, SEED_PACK, Workspace};

/// Workbench policy ceilings for `init`: fuel, memory, and output sized for
/// real tests, and room for large candidates (`SLEY_AGENT_V1.md` section 7).
pub const INIT_CEILINGS: PolicyResourceCeilings =
    PolicyResourceCeilings::new(1_000_000, 16_777_216, 65_536, 0, 10_000, 0);

/// The mutation classes an `init` grant allows.
const INIT_CLASSES: [MutationClass; 11] = [
    MutationClass::CreateEntity,
    MutationClass::ReplaceEntityVersion,
    MutationClass::DeleteEntityBinding,
    MutationClass::SetScalarField,
    MutationClass::ReplaceTypedField,
    MutationClass::RetargetReference,
    MutationClass::InsertOrderedChild,
    MutationClass::RemoveOrderedChild,
    MutationClass::MoveOrderedChild,
    MutationClass::AddTest,
    MutationClass::ReplaceTest,
];

fn kernel(what: &str, detail: impl std::fmt::Display) -> AgentError {
    AgentError::new(
        AgentErrorCode::WorkspaceInvalid,
        format!("{what}: {detail}"),
    )
}

/// Creates a workspace at `dir` with a fresh genesis and writes its
/// `base.pack`. `seed` makes the genesis identities reproducible.
///
/// # Errors
///
/// `AGENT_WORKSPACE_INVALID` when `dir` already holds a repository or a
/// seed, or the kernel refuses a genesis fact.
pub fn init(
    dir: &Path,
    seed: Option<[u8; 32]>,
    ceilings: PolicyResourceCeilings,
) -> Result<Workspace> {
    let repo = dir.join(REPO_DIR);
    if dir.join(SEED_PACK).exists()
        || fs::read_dir(&repo).is_ok_and(|mut entries| entries.next().is_some())
    {
        return Err(AgentError::new(
            AgentErrorCode::WorkspaceInvalid,
            format!("{} already holds a repository or base.pack", dir.display()),
        ));
    }
    fs::create_dir_all(&repo).map_err(|error| io(&repo, &error))?;
    let nonce = match seed {
        Some(seed) => seed,
        None => crate::candidate::random32()?,
    };
    let workspace_id = WorkspaceId::derive(GenesisNonce::from_bytes(nonce));
    let principal = PrincipalId::from_bytes(*workspace_id.as_bytes());
    let mut grant = PrincipalGrantBuilder::new(ceilings);
    for class in INIT_CLASSES {
        grant = grant.mutation_class(class);
    }
    let grant = grant
        .build()
        .map_err(|error| kernel("grant", format!("{error:?}")))?;
    let registry = sley_policy::conformance_registry()
        .map_err(|error| kernel("policy registry", format!("{error:?}")))?;
    let policy = PolicyRootBuilder::new(workspace_id)
        .principal_grant(principal, grant)
        .build(&registry)
        .map_err(|error| kernel("policy", format!("{error:?}")))?;
    let epoch = workspace::epoch()?;
    let store = ObjectStore::new(&repo);
    let verifier =
        move |b: &[u8]| sley_mutate::import_entity_object(epoch, b).map(|o| o.object_id());
    // The contract and test roots are two empty namespace objects, as in
    // every trusted genesis the repository tests build.
    let mut anchors = Vec::new();
    for byte in [1_u8, 2] {
        let mut bytes = *workspace_id.as_bytes();
        bytes[0] ^= byte;
        let object = build_entity_object(
            epoch,
            &EntityObjectRecord {
                entity_id: sley_id::EntityId::from_bytes(bytes),
                body: EntityBodyValue::Namespace(NamespaceBody {
                    parent: None,
                    members: EntityIdSet::from_unsorted(Vec::new())
                        .map_err(|error| kernel("anchor", format!("{error:?}")))?,
                }),
                label: None,
                semantic_fingerprint: None,
            },
        )
        .map_err(|error| kernel("anchor", format!("{error:?}")))?;
        store
            .put(object.object_id(), object.stored_bytes(), &verifier)
            .map_err(|error| kernel("anchor store", format!("{error:?}")))?;
        anchors.push(object.object_id());
    }
    let state_registry = sley_state_root::conformance_registry()
        .map_err(|error| kernel("state registry", format!("{error:?}")))?;
    let state = StateRootBuilder::new(workspace_id, anchors[0], anchors[1], policy.root())
        .build(&state_registry)
        .map_err(|error| kernel("state root", format!("{error:?}")))?;
    let genesis = TransactionRepository::new(&repo)
        .initialize_trusted_genesis(TrustedGenesisInput::new(&state, &policy, &[], &[]))
        .map_err(|error| kernel("genesis", error.code()))?;
    sley_repo::BranchRepository::new(&repo)
        .create_branch("main", genesis.transaction_id())
        .map_err(|error| kernel("branch", format!("{error:?}")))?;
    let workspace = Workspace::at(dir);
    export(&workspace, &dir.join(SEED_PACK))?;
    Ok(workspace)
}

/// Commits a Valid candidate into the workspace repository.
///
/// # Errors
///
/// `AGENT_SUBMISSION_REFUSED` when the transaction engine refuses it
/// (including candidates that carry `TestCases`, which 2.0 cannot commit
/// until native test evidence lands).
pub fn commit(head: &Head, repo: &Path, stored: &[u8]) -> Result<TransactionId> {
    let authority = Authority::of(head)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        });
    let output = TransactionRepository::new(repo)
        .commit(CommitInput::new(
            head.transaction_id(),
            stored,
            authority.principal,
            &[],
            now,
            CandidateValidationLimits::full_v1(),
        ))
        .map_err(|error| {
            AgentError::new(
                AgentErrorCode::SubmissionRefused,
                format!("commit refused: {}", error.code()),
            )
        })?;
    Ok(output.transaction_id())
}

/// Writes the repository as an exchange pack.
///
/// # Errors
///
/// `AGENT_WORKSPACE_INVALID` when the kernel refuses the export,
/// `AGENT_IO_FAILED` when the file cannot be written.
pub fn export(workspace: &Workspace, path: &Path) -> Result<usize> {
    let epoch = workspace::epoch()?;
    let verifier =
        move |b: &[u8]| sley_mutate::import_entity_object(epoch, b).map(|o| o.object_id());
    let pack = sley_repo::export_repository_exchange(&workspace.repo(), &verifier)
        .map_err(|error| kernel("export", error.code()))?;
    fs::write(path, &pack.stored_bytes).map_err(|error| io(path, &error))?;
    Ok(pack.stored_bytes.len())
}
