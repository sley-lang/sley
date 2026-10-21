//! Workspace discovery, first-use seeding, and accepted-head loading.
//!
//! A workspace is a directory holding `repo/` (the served repository) and,
//! before first use, `base.pack` (the seed exchange). Workbench state lives
//! in `.sley/` beside the repository and never inside it; the submission is
//! `final_candidate.hex`. Nothing here writes to `repo/` except the one
//! privileged seeding import of an empty repository.
//!
//! Workspaces need the `native` feature. [`Head`] and [`Program`] do not: a
//! head can also be built in memory from [`HeadParts`].

use std::collections::BTreeMap;
#[cfg(feature = "native")]
use std::fs;
#[cfg(feature = "native")]
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use sley_id::{EntityId, SchemaEpochId, StateRoot, TransactionId, WorkspaceId};
use sley_mutate::EntityObject;
use sley_mutate::value::EntityBodyValue;
use sley_policy::AcceptedPolicyRoot;
use sley_state_root::AcceptedStateRoot;
#[cfg(feature = "native")]
use sley_txn::{AcceptedHead, TransactionRepository};

#[cfg(feature = "native")]
use crate::error::io;
use crate::error::{AgentError, AgentErrorCode, Result};

/// Served repository directory name.
pub const REPO_DIR: &str = "repo";
/// Seed exchange file name.
pub const SEED_PACK: &str = "base.pack";
/// Workbench state directory name.
pub const STATE_DIR: &str = ".sley";
/// Submission file name (read by evaluators).
pub const SUBMISSION: &str = "final_candidate.hex";
/// Optional starter-provided name map.
pub const NAMES_FILE: &str = "names.json";

/// One located workspace.
#[cfg(feature = "native")]
#[derive(Clone, Debug)]
pub struct Workspace {
    dir: PathBuf,
}

#[cfg(feature = "native")]
impl Workspace {
    /// Locates the workspace containing `start` (or `start` itself): the
    /// nearest directory, walking up, that holds `repo/` or `base.pack`.
    ///
    /// # Errors
    ///
    /// `AGENT_WORKSPACE_NOT_FOUND` when no ancestor qualifies.
    pub fn locate(start: &Path) -> Result<Self> {
        let mut current = Some(start);
        while let Some(dir) = current {
            if dir.join(REPO_DIR).is_dir() || dir.join(SEED_PACK).is_file() {
                return Ok(Self {
                    dir: dir.to_path_buf(),
                });
            }
            current = dir.parent();
        }
        Err(AgentError::new(
            AgentErrorCode::WorkspaceNotFound,
            format!(
                "no directory at or above {} holds repo/ or base.pack",
                start.display()
            ),
        ))
    }

    /// Uses `dir` as the workspace without searching.
    #[must_use]
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Returns the workspace directory.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Returns the served repository path.
    #[must_use]
    pub fn repo(&self) -> PathBuf {
        self.dir.join(REPO_DIR)
    }

    /// Returns (creating it when absent) the workbench state directory.
    ///
    /// # Errors
    ///
    /// `AGENT_IO_FAILED` when the directory cannot be created.
    pub fn state_dir(&self) -> Result<PathBuf> {
        let path = self.dir.join(STATE_DIR);
        fs::create_dir_all(&path).map_err(|error| io(&path, &error))?;
        Ok(path)
    }

    /// Imports `base.pack` into an empty or absent `repo/`. A repository
    /// that already has content is left untouched.
    ///
    /// # Errors
    ///
    /// `AGENT_WORKSPACE_INVALID` when the repository is empty and there is
    /// no seed, or the kernel refuses the import.
    pub fn ensure_seeded(&self) -> Result<()> {
        let repo = self.repo();
        let empty = match fs::read_dir(&repo) {
            Ok(mut entries) => entries.next().is_none(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(error) => return Err(io(&repo, &error)),
        };
        if !empty {
            return Ok(());
        }
        let seed = self.dir.join(SEED_PACK);
        let bytes = fs::read(&seed).map_err(|error| {
            AgentError::new(
                AgentErrorCode::WorkspaceInvalid,
                format!(
                    "repo/ is empty and {} is unreadable: {error}",
                    seed.display()
                ),
            )
        })?;
        let epoch = epoch()?;
        let verifier =
            move |b: &[u8]| sley_mutate::import_entity_object(epoch, b).map(|o| o.object_id());
        sley_repo::import_repository_exchange(&repo, &bytes, &verifier).map_err(|error| {
            AgentError::new(
                AgentErrorCode::WorkspaceInvalid,
                format!("seed import refused: {}", error.code()),
            )
        })?;
        Ok(())
    }

    /// Loads the accepted head, seeding first when needed.
    ///
    /// # Errors
    ///
    /// `AGENT_WORKSPACE_INVALID` when the head cannot be loaded.
    pub fn head(&self) -> Result<Head> {
        self.ensure_seeded()?;
        self.read_head()
    }

    /// Loads an existing accepted head without seeding or writing the workspace.
    ///
    /// # Errors
    ///
    /// `AGENT_WORKSPACE_INVALID` when no accepted head can be loaded.
    pub fn read_head(&self) -> Result<Head> {
        let head = TransactionRepository::new(self.repo())
            .accepted_head()
            .map_err(|error| {
                AgentError::new(
                    AgentErrorCode::WorkspaceInvalid,
                    format!("accepted head unavailable: {}", error.code()),
                )
            })?;
        Ok(Head::new(head))
    }
}

/// The exact schema epoch every epoch-1 repository uses.
///
/// # Errors
///
/// `AGENT_WORKSPACE_INVALID` if the conformance registry is unavailable.
pub fn epoch() -> Result<SchemaEpochId> {
    sley_state_root::conformance_epoch_id().map_err(|error| {
        AgentError::new(
            AgentErrorCode::WorkspaceInvalid,
            format!("schema epoch unavailable: {error}"),
        )
    })
}

/// The accepted head plus its program view.
///
/// The program view copies every live object, so it is built on first use:
/// a command that only validates or runs a candidate never needs it.
#[derive(Clone, Debug)]
pub struct Head {
    state: HeadState,
    program: OnceLock<Program>,
}

/// Where a head's facts live: a repository's verified accepted head
/// (`native`), or a plain in-memory state ([`HeadParts`]).
// The repository head stays inline, as `Head` always held it; the rarer
// in-memory parts are boxed.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
enum HeadState {
    #[cfg(feature = "native")]
    Accepted(AcceptedHead),
    Parts(Box<HeadParts>),
}

/// The facts of an accepted head held in memory, without a repository: what
/// candidate assembly, validation and execution read. The embed API builds
/// one for a fresh genesis ([`crate::genesis::genesis_head`]).
#[derive(Clone, Debug)]
pub struct HeadParts {
    /// The accepted transaction identity the candidate binds as its base.
    pub transaction_id: TransactionId,
    /// The accepted state root.
    pub state_root: AcceptedStateRoot,
    /// The protected policy root.
    pub policy_root: AcceptedPolicyRoot,
    /// The live objects in binding order.
    pub objects: Vec<EntityObject>,
    /// The tombstone ledger.
    pub tombstones: Vec<EntityId>,
}

impl Head {
    /// A head over in-memory facts. Nothing checks that `transaction_id`
    /// names a durable transaction: the kernel's candidate validation judges
    /// a candidate against exactly these facts.
    #[must_use]
    pub fn from_parts(parts: HeadParts) -> Self {
        Self {
            state: HeadState::Parts(Box::new(parts)),
            program: OnceLock::new(),
        }
    }

    #[cfg(feature = "native")]
    const fn new(head: AcceptedHead) -> Self {
        Self {
            state: HeadState::Accepted(head),
            program: OnceLock::new(),
        }
    }

    /// Returns the accepted transaction.
    #[must_use]
    pub const fn transaction_id(&self) -> TransactionId {
        match &self.state {
            #[cfg(feature = "native")]
            HeadState::Accepted(head) => head.transaction_id(),
            HeadState::Parts(parts) => parts.transaction_id,
        }
    }

    /// Returns the accepted state root.
    #[must_use]
    pub const fn state_root(&self) -> &AcceptedStateRoot {
        match &self.state {
            #[cfg(feature = "native")]
            HeadState::Accepted(head) => head.state_root(),
            HeadState::Parts(parts) => &parts.state_root,
        }
    }

    /// Returns the protected policy root.
    #[must_use]
    pub const fn policy_root(&self) -> &AcceptedPolicyRoot {
        match &self.state {
            #[cfg(feature = "native")]
            HeadState::Accepted(head) => head.policy_root(),
            HeadState::Parts(parts) => &parts.policy_root,
        }
    }

    /// Returns the live objects in binding order.
    #[must_use]
    pub fn objects(&self) -> &[EntityObject] {
        match &self.state {
            #[cfg(feature = "native")]
            HeadState::Accepted(head) => head.objects(),
            HeadState::Parts(parts) => &parts.objects,
        }
    }

    /// Returns the tombstone ledger.
    #[must_use]
    pub fn tombstones(&self) -> &[EntityId] {
        match &self.state {
            #[cfg(feature = "native")]
            HeadState::Accepted(head) => head.tombstoned_entities(),
            HeadState::Parts(parts) => &parts.tombstones,
        }
    }

    /// Returns the schema epoch of the accepted state (the program view's).
    #[must_use]
    pub const fn epoch(&self) -> SchemaEpochId {
        self.state_root().record.schema_epoch_id
    }

    /// Returns the workspace identity of the accepted state (the program
    /// view's).
    #[must_use]
    pub const fn workspace(&self) -> WorkspaceId {
        self.state_root().record.workspace_id
    }

    /// Returns the accepted program view, building it on first use.
    #[must_use]
    pub fn program(&self) -> &Program {
        self.program.get_or_init(|| {
            let state = self.state_root();
            Program::new(
                state.record.schema_epoch_id,
                state.root,
                state.record.workspace_id,
                self.objects().to_vec(),
            )
        })
    }
}

/// One immutable program state: the accepted head or a candidate's
/// proposed state. Objects are sorted by entity id.
#[derive(Clone, Debug)]
pub struct Program {
    epoch: SchemaEpochId,
    root: StateRoot,
    workspace: WorkspaceId,
    objects: Vec<EntityObject>,
    index: BTreeMap<EntityId, usize>,
}

impl Program {
    /// Builds a program view over `objects`.
    #[must_use]
    pub fn new(
        epoch: SchemaEpochId,
        root: StateRoot,
        workspace: WorkspaceId,
        mut objects: Vec<EntityObject>,
    ) -> Self {
        objects.sort_by_key(|object| object.record().entity_id);
        let index = objects
            .iter()
            .enumerate()
            .map(|(position, object)| (object.record().entity_id, position))
            .collect();
        Self {
            epoch,
            root,
            workspace,
            objects,
            index,
        }
    }

    /// Returns the schema epoch.
    #[must_use]
    pub const fn epoch(&self) -> SchemaEpochId {
        self.epoch
    }

    /// Returns the state root the program is addressed under.
    #[must_use]
    pub const fn root(&self) -> StateRoot {
        self.root
    }

    /// Returns the workspace identity.
    #[must_use]
    pub const fn workspace(&self) -> WorkspaceId {
        self.workspace
    }

    /// Returns every object, sorted by entity id.
    #[must_use]
    pub fn objects(&self) -> &[EntityObject] {
        &self.objects
    }

    /// Returns one object.
    #[must_use]
    pub fn object(&self, id: &EntityId) -> Option<&EntityObject> {
        self.index.get(id).map(|position| &self.objects[*position])
    }

    /// Returns one entity body.
    #[must_use]
    pub fn body(&self, id: &EntityId) -> Option<&EntityBodyValue> {
        self.object(id).map(|object| &object.record().body)
    }

    /// Returns whether the entity is live.
    #[must_use]
    pub fn contains(&self, id: &EntityId) -> bool {
        self.index.contains_key(id)
    }
}
