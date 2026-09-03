//! S20-560 production object verifier: the repository's object store holds
//! S20-340 entity objects under the conformance schema epoch and nothing
//! else, and an entity object references no other object (entity
//! references are binding positions of the state root, which the GC planner
//! traverses itself). SMP1 appendix C composes it for `gc.dry_run` and
//! `gc.collect`.

use sley_id::{ObjectId, SchemaEpochId};
use sley_mutate::import_entity_object;
use sley_scb1::{ScbError, ScbErrorCode};
use sley_store::CanonicalVerifier;

use crate::gc::GcObjectVerifier;

/// Verifies stored entity objects under one schema epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepositoryObjectVerifier {
    epoch: SchemaEpochId,
}

impl RepositoryObjectVerifier {
    /// A verifier for objects of the given schema epoch.
    #[must_use]
    pub const fn new(epoch: SchemaEpochId) -> Self {
        Self { epoch }
    }

    /// The schema epoch every object must carry.
    #[must_use]
    pub const fn epoch(&self) -> SchemaEpochId {
        self.epoch
    }
}

impl CanonicalVerifier for RepositoryObjectVerifier {
    fn verify(&self, record: &[u8]) -> Result<ObjectId, ScbError> {
        import_entity_object(self.epoch, record)
            .map(|object| object.object_id())
            .map_err(|_| ScbError::new(ScbErrorCode::FieldUnknown))
    }
}

impl GcObjectVerifier for RepositoryObjectVerifier {
    fn references(&self, record: &[u8]) -> Result<Vec<ObjectId>, ScbError> {
        self.verify(record)?;
        Ok(Vec::new())
    }
}
