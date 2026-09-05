//! Master context capsule over one root-backed query (S20-320 full, contract
//! `docs/spec/CONTEXT_CAPSULE_PROFILE_V1.md`).
//!
//! A capsule restates the question, copies the provenance the engine bound,
//! states exact omission and continuation status, copies the exact
//! `SLEYRQR1` record, and reorganizes its facts into raw-identity
//! dictionaries. It can only be built from a `RootQueryRequest` and the
//! `RootQueryResponse` produced for it, and it is never a session.

// Variant, field, and getter names are the contract's own names; the
// contract text is their documentation.
#![allow(missing_docs)]

use core::fmt;

use sley_id::{
    ContextCapsuleId, EntityId, IndexSnapshotId, ObjectId, RootQueryId, SchemaEpochId,
    SemanticFingerprint, SessionId, StateRoot, WorkspaceId,
};

use crate::root_query::{Cursor, RootQueryRequest, RootQueryResponse, RootQueryResult};
use crate::{ImpactKind, QueryError};

const MAGIC: &[u8; 8] = b"SLEYCCP1";
const RESPONSE_MAGIC: &[u8; 8] = b"SLEYRQR1";
const FORMAT_VERSION: u32 = 1;
const PROFILE_VERSION: u32 = 1;
const SESSION_BINDING_NONE: u32 = 1;
const SESSION_BINDING_NEGOTIATED: u32 = 2;
const COMPLETENESS_COMPLETE: u32 = 1;
const COMPLETENESS_PAGE: u32 = 2;
const FLAG_FALSE: u32 = 1;
const FLAG_TRUE: u32 = 2;
const OPTION_NONE: u32 = 1;
const OPTION_SOME: u32 = 2;
const TRAILER_BYTES: usize = 32;

/// Largest root-backed response a capsule may copy.
pub const MAX_CONTEXT_CAPSULE_SOURCE_BYTES: u64 = 33_554_432;
/// Entity dictionary ceiling.
pub const MAX_CONTEXT_CAPSULE_ENTITIES: usize = 65_535;
/// Relationship table ceiling.
pub const MAX_CONTEXT_CAPSULE_RELATIONSHIPS: usize = 400_000;
/// Ceiling of each of the roots, objects, and fingerprints tables.
pub const MAX_CONTEXT_CAPSULE_TABLE: usize = 65_535;
/// Complete capsule record ceiling.
pub const MAX_CONTEXT_CAPSULE_BYTES: u64 = 67_108_864;
/// Charged derivation and encoding work ceiling.
pub const MAX_CONTEXT_CAPSULE_WORK: u64 = 100_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextCapsuleErrorCode {
    SourceInvalid,
    DictionaryInvalid,
    ResourceLimit,
    InternalInvariant,
}

impl ContextCapsuleErrorCode {
    pub const ALL: [Self; 4] = [
        Self::SourceInvalid,
        Self::DictionaryInvalid,
        Self::ResourceLimit,
        Self::InternalInvariant,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceInvalid => "CONTEXT_CAPSULE_SOURCE_INVALID",
            Self::DictionaryInvalid => "CONTEXT_CAPSULE_DICTIONARY_INVALID",
            Self::ResourceLimit => "CONTEXT_CAPSULE_RESOURCE_LIMIT",
            Self::InternalInvariant => "CONTEXT_CAPSULE_INTERNAL_INVARIANT",
        }
    }

    #[must_use]
    pub const fn numeric(self) -> u32 {
        match self {
            Self::SourceInvalid => 32_008,
            Self::DictionaryInvalid => 32_009,
            Self::ResourceLimit => 32_010,
            Self::InternalInvariant => 32_011,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextCapsuleError(ContextCapsuleErrorCode);

impl ContextCapsuleError {
    #[must_use]
    pub const fn new(code: ContextCapsuleErrorCode) -> Self {
        Self(code)
    }

    #[must_use]
    pub const fn code(&self) -> ContextCapsuleErrorCode {
        self.0
    }
}

impl fmt::Display for ContextCapsuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for ContextCapsuleError {}

impl From<QueryError> for ContextCapsuleError {
    fn from(_: QueryError) -> Self {
        Self(ContextCapsuleErrorCode::ResourceLimit)
    }
}

fn fail<T>(code: ContextCapsuleErrorCode) -> Result<T, ContextCapsuleError> {
    Err(ContextCapsuleError(code))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapsuleCompleteness {
    Complete,
    Page,
}

impl CapsuleCompleteness {
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Complete => COMPLETENESS_COMPLETE,
            Self::Page => COMPLETENESS_PAGE,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextRelationship {
    pub dependent_index: u32,
    pub dependency_index: u32,
    pub kind: ImpactKind,
}

/// The master context capsule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextCapsule {
    capsule_id: ContextCapsuleId,
    query_id: RootQueryId,
    snapshot_id: IndexSnapshotId,
    root: StateRoot,
    schema_epoch: SchemaEpochId,
    workspace_id: WorkspaceId,
    class_tag: u32,
    session: Option<SessionId>,
    completeness: CapsuleCompleteness,
    truncated: bool,
    total_count: u64,
    returned: u64,
    omitted: u64,
    after: Option<Cursor>,
    next_after: Option<Cursor>,
    entities: Vec<EntityId>,
    kinds: Vec<u32>,
    relationships: Vec<ContextRelationship>,
    roots: Vec<StateRoot>,
    objects: Vec<(u32, ObjectId)>,
    fingerprints: Vec<(u32, SemanticFingerprint)>,
    response_bytes: u64,
    record: Vec<u8>,
}

impl ContextCapsule {
    #[must_use]
    pub const fn capsule_id(&self) -> ContextCapsuleId {
        self.capsule_id
    }

    #[must_use]
    pub const fn query_id(&self) -> RootQueryId {
        self.query_id
    }

    #[must_use]
    pub const fn snapshot_id(&self) -> IndexSnapshotId {
        self.snapshot_id
    }

    #[must_use]
    pub const fn root(&self) -> StateRoot {
        self.root
    }

    #[must_use]
    pub const fn schema_epoch(&self) -> SchemaEpochId {
        self.schema_epoch
    }

    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub const fn class_tag(&self) -> u32 {
        self.class_tag
    }

    /// The negotiated session the capsule was built under, if any
    /// (S20-330; `None` is the fixed arm 1).
    #[must_use]
    pub const fn session(&self) -> Option<SessionId> {
        self.session
    }

    #[must_use]
    pub const fn completeness(&self) -> CapsuleCompleteness {
        self.completeness
    }

    #[must_use]
    pub const fn is_truncated(&self) -> bool {
        self.truncated
    }

    #[must_use]
    pub const fn total_count(&self) -> u64 {
        self.total_count
    }

    #[must_use]
    pub const fn returned(&self) -> u64 {
        self.returned
    }

    #[must_use]
    pub const fn omitted(&self) -> u64 {
        self.omitted
    }

    #[must_use]
    pub const fn after(&self) -> Option<Cursor> {
        self.after
    }

    #[must_use]
    pub const fn next_after(&self) -> Option<Cursor> {
        self.next_after
    }

    #[must_use]
    pub fn entities(&self) -> &[EntityId] {
        &self.entities
    }

    #[must_use]
    pub fn kinds(&self) -> &[u32] {
        &self.kinds
    }

    #[must_use]
    pub fn relationships(&self) -> &[ContextRelationship] {
        &self.relationships
    }

    #[must_use]
    pub fn roots(&self) -> &[StateRoot] {
        &self.roots
    }

    #[must_use]
    pub fn objects(&self) -> &[(u32, ObjectId)] {
        &self.objects
    }

    #[must_use]
    pub fn fingerprints(&self) -> &[(u32, SemanticFingerprint)] {
        &self.fingerprints
    }

    #[must_use]
    pub const fn response_bytes(&self) -> u64 {
        self.response_bytes
    }

    #[must_use]
    pub fn record(&self) -> &[u8] {
        &self.record
    }
}

struct Work(u64);

impl Work {
    fn charge(&mut self, amount: u64) -> Result<(), ContextCapsuleError> {
        self.0 = self
            .0
            .checked_add(amount)
            .ok_or_else(|| ContextCapsuleError::new(ContextCapsuleErrorCode::ResourceLimit))?;
        if self.0 > MAX_CONTEXT_CAPSULE_WORK {
            return fail(ContextCapsuleErrorCode::ResourceLimit);
        }
        Ok(())
    }
}

struct Facts {
    entities: Vec<EntityId>,
    kinds: Vec<u32>,
    relationships: Vec<ContextRelationship>,
    roots: Vec<StateRoot>,
    objects: Vec<(u32, ObjectId)>,
    fingerprints: Vec<(u32, SemanticFingerprint)>,
}

/// Builds the master context capsule over one bound request and response.
///
/// # Errors
///
/// Fails without a capsule on source disagreement, resource ceilings,
/// dictionary defects, or an impossible encoding invariant.
pub fn build_context_capsule(
    request: &RootQueryRequest,
    response: &RootQueryResponse,
) -> Result<ContextCapsule, ContextCapsuleError> {
    build_capsule(request, response, None)
}

/// Builds the capsule under a negotiated session (S20-330, S20-320 full
/// revision 3): the session binding arm is `Negotiated(2)` with the
/// session identity.
///
/// This is the authority-delegated encoding primitive, not the supported
/// construction path. It takes no provenance because there is no
/// provenance for the caller to declare: the response's provenance is
/// engine-verified, and whether that provenance equals a live session's
/// binding is known only to the session authority. The supported path is
/// `SessionAuthority::bind_context_capsule`, which refuses unknown and
/// closed sessions and fails `CONTEXT_CAPSULE_SOURCE_INVALID` when the
/// response's workspace, root, or epoch differs from the session's
/// authority-held binding. Call this primitive directly only when no
/// provenance claim is being made (fixture emission, fuzz encoding
/// coverage); a capsule built this way carries the session identity but
/// no authority verified the binding.
///
/// # Errors
///
/// Otherwise as `build_context_capsule`.
pub fn build_context_capsule_session(
    request: &RootQueryRequest,
    response: &RootQueryResponse,
    session: SessionId,
) -> Result<ContextCapsule, ContextCapsuleError> {
    build_capsule(request, response, Some(session))
}

fn build_capsule(
    request: &RootQueryRequest,
    response: &RootQueryResponse,
    session: Option<SessionId>,
) -> Result<ContextCapsule, ContextCapsuleError> {
    validate_source(request, response)?;
    let mut work = Work(0);
    let facts = derive_facts(request, response, &mut work)?;
    let completeness = if !response.truncated() && request.after().is_none() {
        CapsuleCompleteness::Complete
    } else {
        CapsuleCompleteness::Page
    };
    let omitted = response
        .total_count()
        .checked_sub(response.returned())
        .ok_or_else(|| ContextCapsuleError::new(ContextCapsuleErrorCode::SourceInvalid))?;
    let mut record = encode_preimage(
        request,
        response,
        session,
        completeness,
        omitted,
        &facts,
        &mut work,
    )?;
    let capsule_id = ContextCapsuleId::derive(&record);
    append(&mut record, capsule_id.as_bytes(), &mut work)?;
    Ok(ContextCapsule {
        capsule_id,
        query_id: response.query_id(),
        snapshot_id: response.snapshot_id(),
        root: response.root(),
        schema_epoch: response.schema_epoch(),
        workspace_id: response.workspace_id(),
        class_tag: response.class_tag(),
        session,
        completeness,
        truncated: response.truncated(),
        total_count: response.total_count(),
        returned: response.returned(),
        omitted,
        after: request.after(),
        next_after: response.next_after(),
        entities: facts.entities,
        kinds: facts.kinds,
        relationships: facts.relationships,
        roots: facts.roots,
        objects: facts.objects,
        fingerprints: facts.fingerprints,
        response_bytes: response.response_bytes(),
        record,
    })
}

fn validate_source(
    request: &RootQueryRequest,
    response: &RootQueryResponse,
) -> Result<(), ContextCapsuleError> {
    let record = response.record();
    if request.query_id() != response.query_id()
        || request.snapshot_id() != response.snapshot_id()
        || request.root() != response.root()
        || request.query().tag() != response.class_tag()
        || record.get(..8) != Some(RESPONSE_MAGIC.as_slice())
        || u64::try_from(record.len()).ok() != Some(response.response_bytes())
        || response.returned() != to_u64(count_items(response.result()))?
        || response.returned() > response.total_count()
        || response.truncated() != response.next_after().is_some()
    {
        return fail(ContextCapsuleErrorCode::SourceInvalid);
    }
    if response.response_bytes() > MAX_CONTEXT_CAPSULE_SOURCE_BYTES {
        return fail(ContextCapsuleErrorCode::ResourceLimit);
    }
    Ok(())
}

fn count_items(result: &RootQueryResult) -> usize {
    match result {
        RootQueryResult::RootSummary(_)
        | RootQueryResult::Entity { .. }
        | RootQueryResult::Fingerprint(_) => 1,
        RootQueryResult::Entities(items) => items.len(),
        RootQueryResult::DependencyRows(items) => items.len(),
        RootQueryResult::InventoryEntries(items) => items.len(),
        RootQueryResult::EntryRows(items) => items.len(),
        RootQueryResult::Roots(items) => items.len(),
        RootQueryResult::Edges(items) => items.len(),
    }
}

#[allow(clippy::too_many_lines)]
fn derive_facts(
    request: &RootQueryRequest,
    response: &RootQueryResponse,
    work: &mut Work,
) -> Result<Facts, ContextCapsuleError> {
    let mut ids: Vec<EntityId> = request.query().named_entities();
    work.charge(to_u64(ids.len())?)?;
    let mut roots = Vec::new();
    match response.result() {
        RootQueryResult::RootSummary(_)
        | RootQueryResult::Entity { .. }
        | RootQueryResult::Fingerprint(_) => {}
        RootQueryResult::Entities(items) => {
            work.charge(to_u64(items.len())?)?;
            ids.extend(items.iter().copied());
        }
        RootQueryResult::DependencyRows(rows) => {
            work.charge(to_u64(rows.len())?)?;
            for row in rows {
                ids.push(row.binding);
                ids.push(row.external_package);
                ids.push(row.local_namespace);
                roots.push(row.dependency_root);
            }
        }
        RootQueryResult::InventoryEntries(entries) => {
            work.charge(to_u64(entries.len())?)?;
            ids.extend(entries.iter().map(|entry| entry.entity));
        }
        RootQueryResult::EntryRows(rows) => {
            work.charge(to_u64(rows.len())?)?;
            for row in rows {
                ids.push(row.entry_point);
                ids.push(row.function);
            }
        }
        RootQueryResult::Roots(items) => {
            work.charge(to_u64(items.len())?)?;
            roots.extend(items.iter().copied());
        }
        RootQueryResult::Edges(edges) => {
            work.charge(to_u64(edges.len())?)?;
            for edge in edges {
                ids.push(edge.dependent);
                ids.push(edge.dependency);
            }
        }
    }
    ids.sort_unstable();
    ids.dedup();
    roots.sort_unstable();
    roots.dedup();
    if ids.len() > MAX_CONTEXT_CAPSULE_ENTITIES || roots.len() > MAX_CONTEXT_CAPSULE_TABLE {
        return fail(ContextCapsuleErrorCode::ResourceLimit);
    }
    work.charge(to_u64(ids.len())?)?;
    let index_of = |entity: EntityId| -> Result<u32, ContextCapsuleError> {
        ids.binary_search(&entity)
            .ok()
            .and_then(|index| u32::try_from(index).ok())
            .ok_or_else(|| ContextCapsuleError::new(ContextCapsuleErrorCode::DictionaryInvalid))
    };
    let mut kinds = vec![0_u32; ids.len()];
    let mut relationships = Vec::new();
    let mut objects = Vec::new();
    let mut fingerprints = Vec::new();
    let subject = request.query().named_entities().first().copied();
    match response.result() {
        RootQueryResult::Entity {
            kind,
            object_id,
            fingerprint,
        } => {
            let entity = subject
                .ok_or_else(|| ContextCapsuleError::new(ContextCapsuleErrorCode::SourceInvalid))?;
            let index = index_of(entity)?;
            kinds[index as usize] = kind.tag();
            objects.push((index, *object_id));
            if let Some(fingerprint) = fingerprint {
                fingerprints.push((index, *fingerprint));
            }
        }
        RootQueryResult::Fingerprint(Some(fingerprint)) => {
            let entity = subject
                .ok_or_else(|| ContextCapsuleError::new(ContextCapsuleErrorCode::SourceInvalid))?;
            fingerprints.push((index_of(entity)?, *fingerprint));
        }
        RootQueryResult::InventoryEntries(entries) => {
            for entry in entries {
                work.charge(1)?;
                kinds[index_of(entry.entity)? as usize] = entry.kind.tag();
            }
        }
        RootQueryResult::Edges(edges) => {
            if edges.len() > MAX_CONTEXT_CAPSULE_RELATIONSHIPS {
                return fail(ContextCapsuleErrorCode::ResourceLimit);
            }
            for edge in edges {
                work.charge(2)?;
                relationships.push(ContextRelationship {
                    dependent_index: index_of(edge.dependent)?,
                    dependency_index: index_of(edge.dependency)?,
                    kind: edge.kind,
                });
            }
        }
        _ => {}
    }
    Ok(Facts {
        entities: ids,
        kinds,
        relationships,
        roots,
        objects,
        fingerprints,
    })
}

fn encode_preimage(
    request: &RootQueryRequest,
    response: &RootQueryResponse,
    session: Option<SessionId>,
    completeness: CapsuleCompleteness,
    omitted: u64,
    facts: &Facts,
    work: &mut Work,
) -> Result<Vec<u8>, ContextCapsuleError> {
    let mut out = Vec::new();
    append(&mut out, MAGIC, work)?;
    push_u32(&mut out, FORMAT_VERSION, work)?;
    push_u32(&mut out, PROFILE_VERSION, work)?;
    append(&mut out, response.workspace_id().as_bytes(), work)?;
    append(&mut out, response.schema_epoch().as_bytes(), work)?;
    append(&mut out, response.root().as_bytes(), work)?;
    append(&mut out, response.snapshot_id().as_bytes(), work)?;
    append(&mut out, response.query_id().as_bytes(), work)?;
    match session {
        None => push_u32(&mut out, SESSION_BINDING_NONE, work)?,
        Some(session) => {
            push_u32(&mut out, SESSION_BINDING_NEGOTIATED, work)?;
            append(&mut out, session.as_bytes(), work)?;
        }
    }
    let mut question = Vec::new();
    crate::root_query::encode_question(&mut question, request)?;
    append(&mut out, &question, work)?;
    push_u32(&mut out, completeness.tag(), work)?;
    push_u32(&mut out, flag(response.truncated()), work)?;
    push_u64(&mut out, response.total_count(), work)?;
    push_u64(&mut out, response.returned(), work)?;
    push_u64(&mut out, omitted, work)?;
    encode_cursor(&mut out, response.next_after(), work)?;
    push_u64(&mut out, response.response_bytes(), work)?;
    append(&mut out, response.record(), work)?;
    push_u64(&mut out, to_u64(facts.entities.len())?, work)?;
    for entity in &facts.entities {
        append(&mut out, entity.as_bytes(), work)?;
    }
    push_u64(&mut out, to_u64(facts.kinds.len())?, work)?;
    for kind in &facts.kinds {
        push_u32(&mut out, *kind, work)?;
    }
    push_u64(&mut out, to_u64(facts.relationships.len())?, work)?;
    for relationship in &facts.relationships {
        push_u32(&mut out, relationship.dependent_index, work)?;
        push_u32(&mut out, relationship.dependency_index, work)?;
        push_u32(&mut out, relationship.kind.tag(), work)?;
    }
    push_u64(&mut out, to_u64(facts.roots.len())?, work)?;
    for root in &facts.roots {
        append(&mut out, root.as_bytes(), work)?;
    }
    push_u64(&mut out, to_u64(facts.objects.len())?, work)?;
    for (index, object) in &facts.objects {
        push_u32(&mut out, *index, work)?;
        append(&mut out, object.as_bytes(), work)?;
    }
    push_u64(&mut out, to_u64(facts.fingerprints.len())?, work)?;
    for (index, fingerprint) in &facts.fingerprints {
        push_u32(&mut out, *index, work)?;
        append(&mut out, fingerprint.as_bytes(), work)?;
    }
    if out
        .len()
        .checked_add(TRAILER_BYTES)
        .and_then(|len| u64::try_from(len).ok())
        .is_none_or(|len| len > MAX_CONTEXT_CAPSULE_BYTES)
    {
        return fail(ContextCapsuleErrorCode::ResourceLimit);
    }
    Ok(out)
}

const fn flag(value: bool) -> u32 {
    if value { FLAG_TRUE } else { FLAG_FALSE }
}

fn encode_cursor(
    out: &mut Vec<u8>,
    cursor: Option<Cursor>,
    work: &mut Work,
) -> Result<(), ContextCapsuleError> {
    match cursor {
        None => push_u32(out, OPTION_NONE, work),
        Some(cursor) => {
            push_u32(out, OPTION_SOME, work)?;
            push_u32(out, cursor.tag(), work)?;
            match cursor {
                Cursor::Entity(entity) => append(out, entity.as_bytes(), work),
                Cursor::Root(root) => append(out, root.as_bytes(), work),
                Cursor::Edge(edge) => {
                    append(out, edge.dependent.as_bytes(), work)?;
                    append(out, edge.dependency.as_bytes(), work)?;
                    push_u32(out, edge.kind.tag(), work)
                }
            }
        }
    }
}

fn push_u32(out: &mut Vec<u8>, value: u32, work: &mut Work) -> Result<(), ContextCapsuleError> {
    append(out, &value.to_be_bytes(), work)
}

fn push_u64(out: &mut Vec<u8>, value: u64, work: &mut Work) -> Result<(), ContextCapsuleError> {
    append(out, &value.to_be_bytes(), work)
}

fn append(out: &mut Vec<u8>, bytes: &[u8], work: &mut Work) -> Result<(), ContextCapsuleError> {
    work.charge(to_u64(bytes.len())?)?;
    let next = out
        .len()
        .checked_add(bytes.len())
        .and_then(|len| u64::try_from(len).ok())
        .ok_or_else(|| ContextCapsuleError::new(ContextCapsuleErrorCode::ResourceLimit))?;
    if next > MAX_CONTEXT_CAPSULE_BYTES {
        return fail(ContextCapsuleErrorCode::ResourceLimit);
    }
    out.extend_from_slice(bytes);
    Ok(())
}

fn to_u64(value: usize) -> Result<u64, ContextCapsuleError> {
    u64::try_from(value)
        .map_err(|_| ContextCapsuleError::new(ContextCapsuleErrorCode::ResourceLimit))
}

#[cfg(test)]
#[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
mod tests {
    use super::*;
    use crate::root_query::tests::{Borrowed, Owned, all_classes, id};
    use crate::{
        ImpactKind, ModeledEntityKind, QueryLimits, RootQuery, build_root_query_request,
        execute_root_query,
    };

    /// Session identity of the frozen `class-01-bound` fixture vector: the
    /// arm is proved under a fixed session, never a live one.
    const FIXTURE_SESSION_ID: [u8; 32] = [0x5E; 32];

    fn capsule(
        input: &crate::RootQueryInput<'_>,
        query: RootQuery,
        limits: QueryLimits,
        allow: bool,
        after: Option<Cursor>,
    ) -> (RootQueryRequest, RootQueryResponse, ContextCapsule) {
        let request = build_root_query_request(input, query, limits, allow, after).unwrap();
        let response = execute_root_query(input, &request).unwrap();
        let capsule = build_context_capsule(&request, &response).unwrap();
        (request, response, capsule)
    }

    #[test]
    fn every_class_yields_a_complete_capsule_with_exact_facts() {
        let owned = Owned::new();
        let borrowed = Borrowed::new(&owned);
        let input = borrowed.input();
        let full = QueryLimits::profile_maximum();
        let capsules: Vec<ContextCapsule> = all_classes()
            .into_iter()
            .map(|query| capsule(&input, query, full, false, None).2)
            .collect();
        for (index, capsule) in capsules.iter().enumerate() {
            assert_eq!(capsule.class_tag(), index as u32 + 1);
            assert_eq!(capsule.completeness(), CapsuleCompleteness::Complete);
            assert!(!capsule.is_truncated());
            assert_eq!(capsule.omitted(), 0);
            assert_eq!(capsule.total_count(), capsule.returned());
            assert_eq!(capsule.kinds().len(), capsule.entities().len());
            assert_eq!(&capsule.record()[..8], b"SLEYCCP1");
            assert!(
                capsule
                    .record()
                    .windows(8)
                    .any(|window| window == b"SLEYRQR1")
            );
            assert!(capsule.entities().windows(2).all(|pair| pair[0] < pair[1]));
        }
        // Class 1 has no identities; class 2 carries the object and the kind.
        assert!(capsules[0].entities().is_empty());
        assert_eq!(capsules[1].entities(), &[id(0x07)]);
        assert_eq!(capsules[1].kinds(), &[ModeledEntityKind::Function.tag()]);
        assert_eq!(
            capsules[1].objects(),
            &[(0, ObjectId::from_bytes([0x87; 32]))]
        );
        assert_eq!(capsules[1].fingerprints().len(), 1);
        // Class 3 indexes the question entity.
        assert_eq!(capsules[2].entities(), &[id(0x06)]);
        assert_eq!(
            capsules[2].fingerprints(),
            &[(0, SemanticFingerprint::from_bytes([0x66; 32]))]
        );
        // Class 7 carries the dependency root; class 11 the root list.
        assert_eq!(capsules[6].roots(), &[StateRoot::from_bytes([0x09; 32])]);
        assert_eq!(capsules[10].roots(), &[StateRoot::from_bytes([0x09; 32])]);
        // Class 8 states member kinds; class 12 and 13 have relationships
        // whose indexes invert to the exact edge endpoints.
        // Class 8 states every member kind; only the question's namespace is
        // unknown to the payload.
        assert_eq!(
            capsules[7]
                .kinds()
                .iter()
                .filter(|kind| **kind == 0)
                .count(),
            1
        );
        assert_eq!(capsules[7].kinds().len(), capsules[7].entities().len());
        for capsule in [&capsules[11], &capsules[12]] {
            assert!(!capsule.relationships().is_empty());
            for relationship in capsule.relationships() {
                assert!((relationship.dependent_index as usize) < capsule.entities().len());
                assert!((relationship.dependency_index as usize) < capsule.entities().len());
            }
        }
        assert!(capsules[13].relationships().is_empty());
        for _ in 0..128 {
            let again: Vec<ContextCapsule> = all_classes()
                .into_iter()
                .map(|query| capsule(&input, query, full, false, None).2)
                .collect();
            assert_eq!(again, capsules);
        }
    }

    #[test]
    fn page_capsules_state_omission_and_cover_the_walk() {
        let owned = Owned::new();
        let borrowed = Borrowed::new(&owned);
        let input = borrowed.input();
        let paged = QueryLimits {
            max_returned_entities: 2,
            max_returned_edges: 3,
            ..QueryLimits::profile_maximum()
        };
        let query = RootQuery::ListEntitiesByKind {
            kind: ModeledEntityKind::Namespace,
        };
        let (_, _, first) = capsule(&input, query.clone(), paged, true, None);
        assert_eq!(first.completeness(), CapsuleCompleteness::Page);
        assert!(first.is_truncated());
        assert_eq!(first.total_count(), 3);
        assert_eq!(first.returned(), 2);
        assert_eq!(first.omitted(), 1);
        assert!(first.next_after().is_some());
        let (_, _, second) = capsule(&input, query, paged, true, first.next_after());
        assert_eq!(second.completeness(), CapsuleCompleteness::Page);
        assert!(!second.is_truncated());
        assert_eq!(second.total_count(), 3);
        assert_eq!(second.returned(), 1);
        assert_eq!(second.omitted(), 2);
        assert_eq!(second.after(), first.next_after());
        assert!(second.next_after().is_none());
        let mut walk: Vec<EntityId> = first.entities().to_vec();
        walk.extend(second.entities());
        assert_eq!(walk, vec![id(0x02), id(0x04), id(0x05)]);
        assert_ne!(first.capsule_id(), second.capsule_id());
        let edges = RootQuery::ListDirectDependencies {
            entity: id(0x01),
            kinds: vec![
                ImpactKind::Ownership,
                ImpactKind::Capability,
                ImpactKind::Contract,
                ImpactKind::TestTarget,
            ],
        };
        let (_, _, page) = capsule(&input, edges, paged, true, None);
        assert_eq!(page.relationships().len(), 3);
        assert_eq!(page.omitted(), page.total_count() - 3);
    }

    #[test]
    fn foreign_and_drifted_sources_and_the_code_table_are_exact() {
        let owned = Owned::new();
        let borrowed = Borrowed::new(&owned);
        let input = borrowed.input();
        let full = QueryLimits::profile_maximum();
        let (request_a, response_a, _) =
            capsule(&input, RootQuery::GetRootSummary, full, false, None);
        let (request_b, response_b, _) =
            capsule(&input, RootQuery::ListEntryPoints, full, false, None);
        assert_eq!(
            build_context_capsule(&request_a, &response_b)
                .unwrap_err()
                .code(),
            ContextCapsuleErrorCode::SourceInvalid
        );
        assert_eq!(
            build_context_capsule(&request_b, &response_a)
                .unwrap_err()
                .code(),
            ContextCapsuleErrorCode::SourceInvalid
        );
        assert_eq!(ContextCapsuleErrorCode::ALL.len(), 4);
        assert_eq!(ContextCapsuleErrorCode::ALL[0].numeric(), 32_008);
        assert_eq!(ContextCapsuleErrorCode::ALL[3].numeric(), 32_011);
        assert_eq!(
            ContextCapsuleErrorCode::DictionaryInvalid.as_str(),
            "CONTEXT_CAPSULE_DICTIONARY_INVALID"
        );
    }

    #[test]
    fn a_session_bound_capsule_carries_the_negotiated_arm() {
        let owned = Owned::new();
        let borrowed = Borrowed::new(&owned);
        let input = borrowed.input();
        let full = QueryLimits::profile_maximum();
        let (request, response, plain) =
            capsule(&input, RootQuery::GetRootSummary, full, false, None);
        let session = SessionId::from_bytes([0x5E; 32]);
        // The primitive binds the arm with no provenance to declare; the
        // authority's refusal of foreign provenance is pinned by
        // `SessionAuthority::bind_context_capsule` in `sley-protocol`.
        let bound = build_context_capsule_session(&request, &response, session).unwrap();
        assert_eq!(bound.session(), Some(session));
        assert_eq!(plain.session(), None);
        assert_ne!(bound.capsule_id(), plain.capsule_id());
        assert_eq!(bound.entities(), plain.entities());
        assert!(
            bound
                .record()
                .windows(32)
                .any(|window| window == session.as_bytes())
        );
    }

    fn hex(bytes: &[u8]) -> String {
        use core::fmt::Write as _;
        bytes.iter().fold(String::new(), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
    }

    /// Emits the frozen context capsule vectors for
    /// `scripts/generate_context_capsule_fixtures.py`. The bound vector
    /// reuses the class-01 question and proves the `Negotiated` arm under
    /// the fixed fixture session; the authority verification behind the
    /// arm is covered by `SessionAuthority::bind_context_capsule`, not by
    /// fixture bytes.
    ///
    /// Prints `CONTEXT_CAPSULE_VECTOR|id|query_id|capsule_id|record_hex|
    /// completeness|total|returned|omitted|session_binding|session_id_hex`
    /// with an empty session field for the unbound arm.
    #[test]
    #[ignore = "fixture refresh emitter; run through the generator script"]
    fn emit_context_capsule_vectors_for_fixture_refresh() {
        let owned = Owned::new();
        let borrowed = Borrowed::new(&owned);
        let input = borrowed.input();
        let full = QueryLimits::profile_maximum();
        let emit = |label: &str,
                    query: RootQuery,
                    limits: QueryLimits,
                    allow: bool,
                    after: Option<Cursor>,
                    session: Option<SessionId>| {
            let (request, response, _) = capsule(&input, query, limits, allow, after);
            let capsule = match session {
                None => build_context_capsule(&request, &response).unwrap(),
                Some(session) => {
                    build_context_capsule_session(&request, &response, session).unwrap()
                }
            };
            println!(
                "CONTEXT_CAPSULE_VECTOR|{label}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
                hex(capsule.query_id().as_bytes()),
                hex(capsule.capsule_id().as_bytes()),
                hex(capsule.record()),
                capsule.completeness().tag(),
                capsule.total_count(),
                capsule.returned(),
                capsule.omitted(),
                capsule
                    .session()
                    .map_or(SESSION_BINDING_NONE, |_| SESSION_BINDING_NEGOTIATED),
                capsule
                    .session()
                    .map_or(String::new(), |id| hex(id.as_bytes())),
            );
            capsule
        };
        for (index, query) in all_classes().into_iter().enumerate() {
            emit(
                &format!("class-{:02}", index + 1),
                query,
                full,
                false,
                None,
                None,
            );
        }
        // The bound arm over the class-01 question under the fixed
        // fixture session.
        emit(
            "class-01-bound",
            all_classes().into_iter().next().unwrap(),
            full,
            false,
            None,
            Some(SessionId::from_bytes(FIXTURE_SESSION_ID)),
        );
        let paged = QueryLimits {
            max_returned_entities: 2,
            max_returned_edges: 3,
            ..full
        };
        let namespaces = RootQuery::ListEntitiesByKind {
            kind: ModeledEntityKind::Namespace,
        };
        let first = emit(
            "page-namespaces-1",
            namespaces.clone(),
            paged,
            true,
            None,
            None,
        );
        emit(
            "page-namespaces-2",
            namespaces,
            paged,
            true,
            first.next_after(),
            None,
        );
        let edges = RootQuery::ListDirectDependencies {
            entity: id(0x01),
            kinds: vec![
                ImpactKind::Ownership,
                ImpactKind::Capability,
                ImpactKind::Contract,
                ImpactKind::TestTarget,
            ],
        };
        let first = emit("page-edges-1", edges.clone(), paged, true, None, None);
        emit("page-edges-2", edges, paged, true, first.next_after(), None);
    }
}
