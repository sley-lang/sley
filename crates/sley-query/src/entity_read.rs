//! Bounded exact entity and signature reads over borrowed verified objects
//! (AT-MW-02, contract `docs/spec/ENTITY_READ_PROFILE_V2.md` sections 3-5).
//!
//! Pure S20-310 owner: resolves one entity, or one Function plus its ordered
//! parameters, against borrowed verified bindings and objects, charges the
//! deterministic work bound, and encodes the exact response. It performs no
//! I/O, builds no whole-root index, and owns no session or transport debit:
//! the caller reserves `work_units - 1` after [`prepare_entity_read`]
//! succeeds and encodes with [`encode_entity_read_response`], which writes
//! the preflighted bytes into one pre-sized buffer. Frame-envelope preflight
//! stays with the protocol caller, which owns envelope accounting.

use core::fmt;

use sley_id::{EntityId, ObjectId, SchemaEpochId, SessionId, StateRoot, WorkspaceId};
use sley_mutate::EntityObject;
use sley_mutate::value::EntityBodyValue;
use sley_scb1::{
    MAX_BYTE_PAYLOAD, MAX_COLLECTION_ELEMENTS, MAX_RECORD_FIELDS, MAX_STANDALONE_BYTES,
    ScbValueCursor,
};
use sley_ssmc::ParameterRole;

/// Response record field 1: the protocol version this read answers under.
pub const ENTITY_READ_RESPONSE_VERSION: u64 = 2;
/// Entity kind tag of a Function body.
const FUNCTION_KIND: u64 = 5;

/// Which exact read the caller prepares and encodes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityReadMethod {
    /// Exactly the requested stored object.
    Version,
    /// The Function first, then its parameters in declaration order.
    Signature,
}

/// Transport-neutral owner failure.
///
/// `NotCanonical` maps to `PROTOCOL_PAYLOAD_INVALID` and `BudgetExceeded` to
/// `PROTOCOL_LIMIT_EXCEEDED` at the protocol seam; the remaining variants
/// keep their stable owner codes in the failure envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityReadError {
    /// Malformed request record.
    NotCanonical,
    /// Ceiling, work, size, or checked-arithmetic refusal.
    BudgetExceeded,
    /// `QUERY_ROOT_MISMATCH`.
    RootMismatch,
    /// `QUERY_UNRESOLVED_ENTITY`.
    UnresolvedEntity,
    /// `QUERY_CLASS_NOT_APPLICABLE`.
    ClassNotApplicable,
    /// `QUERY_INTERNAL_INVARIANT`.
    InternalInvariant,
}

impl EntityReadError {
    /// Returns the stable owner symbol, if this failure keeps one.
    #[must_use]
    pub const fn owner_symbol(self) -> Option<&'static str> {
        match self {
            Self::NotCanonical | Self::BudgetExceeded => None,
            Self::RootMismatch => Some("QUERY_ROOT_MISMATCH"),
            Self::UnresolvedEntity => Some("QUERY_UNRESOLVED_ENTITY"),
            Self::ClassNotApplicable => Some("QUERY_CLASS_NOT_APPLICABLE"),
            Self::InternalInvariant => Some("QUERY_INTERNAL_INVARIANT"),
        }
    }

    /// Returns the stable owner numeric code, if this failure keeps one.
    #[must_use]
    pub const fn owner_numeric(self) -> Option<u32> {
        match self {
            Self::NotCanonical | Self::BudgetExceeded => None,
            Self::RootMismatch => Some(31_008),
            Self::UnresolvedEntity => Some(31_004),
            Self::ClassNotApplicable => Some(31_010),
            Self::InternalInvariant => Some(31_007),
        }
    }
}

impl fmt::Display for EntityReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.owner_symbol() {
            Some(symbol) => formatter.write_str(symbol),
            None => match self {
                Self::NotCanonical => formatter.write_str("ENTITY_READ_NOT_CANONICAL"),
                Self::BudgetExceeded => formatter.write_str("ENTITY_READ_BUDGET_EXCEEDED"),
                _ => formatter.write_str("ENTITY_READ_UNKNOWN"),
            },
        }
    }
}

impl std::error::Error for EntityReadError {}

/// Decoded exact request record (contract section 3).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntityReadRequest {
    /// Root the response must be bound under.
    pub expected_root: StateRoot,
    /// Target entity identity.
    pub entity: EntityId,
    /// Positive object-count ceiling.
    pub max_objects: u64,
    /// Positive response-byte ceiling; also charged as maximum encoding work.
    pub max_response_bytes: u64,
    /// Positive work ceiling.
    pub max_work: u64,
}

/// Borrowed verified revision view for one response.
///
/// The caller binds this revision to the live session root before entry; it
/// is the single `VerifiedRevision` the response is read from.
#[derive(Clone, Copy, Debug)]
pub struct EntityReadRevision<'a> {
    /// Workspace of the bound root.
    pub workspace: WorkspaceId,
    /// State root the response is bound under.
    pub root: StateRoot,
    /// Schema epoch of the bound objects.
    pub epoch: SchemaEpochId,
    /// Canonical bindings in existing binding order.
    pub bindings: &'a [(EntityId, ObjectId)],
    /// Verified objects aligned with `bindings`.
    pub objects: &'a [EntityObject],
    /// Sorted non-reusable identity ledger.
    pub tombstones: &'a [EntityId],
}

/// Negotiated ceilings plus the session budget before the dispatch charge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntityReadCeilings {
    /// Selected `max_entities`.
    pub max_entities: u64,
    /// Selected `max_response_bytes`.
    pub max_response_bytes: u64,
    /// Selected `max_work`.
    pub max_work: u64,
    /// Session budget as it stood before the dispatch charge.
    pub budget_before_dispatch: u64,
}

/// Checked preflight for one request: the caller reserves `work_units - 1`,
/// then encodes with [`encode_entity_read_response`].
///
/// Opaque and immutable: the only constructor is [`prepare_entity_read`],
/// which copies the borrowed revision view and the decoded request into the
/// plan and checks the selected ceilings there. The lifetime binds the
/// backing revision slices. The encoder consumes the plan plus the session
/// only, so no replacement method, revision, request, or limit can reach
/// encoding, and one preparation encodes once.
#[derive(Debug)]
pub struct EntityReadPlan<'rev> {
    revision: EntityReadRevision<'rev>,
    request: EntityReadRequest,
    indices: Vec<usize>,
    object_count: u64,
    stored_bytes: u64,
    work_units: u64,
    body_len: u64,
    list_len: u64,
}

impl EntityReadPlan<'_> {
    /// Returns the deterministic total charge, including the dispatch unit.
    #[must_use]
    pub const fn work_units(&self) -> u64 {
        self.work_units
    }

    /// Returns the exact response body length.
    #[must_use]
    pub const fn body_len(&self) -> u64 {
        self.body_len
    }

    /// Returns the exact returned object count.
    #[must_use]
    pub const fn object_count(&self) -> u64 {
        self.object_count
    }
}

/// Encoded complete response with its accounting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityReadOutcome {
    /// Exact response body bytes.
    pub body: Vec<u8>,
    /// Deterministic total charge, including the dispatch unit.
    pub work_units: u64,
    /// Exact response-body size.
    pub returned_bytes: u64,
    /// Exact returned object count.
    pub returned_entities: u64,
}

/// One decoded response object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityReadResponseObject {
    /// Bound entity identity.
    pub entity: EntityId,
    /// SSMC1 kind tag.
    pub kind: u64,
    /// Stored object identity.
    pub object_id: ObjectId,
    /// Exact stored object bytes.
    pub stored_bytes: Vec<u8>,
}

/// Decoded exact response body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityReadResponse {
    /// Responding workspace.
    pub workspace: WorkspaceId,
    /// Bound root.
    pub root: StateRoot,
    /// Bound schema epoch.
    pub epoch: SchemaEpochId,
    /// Responding session.
    pub session: SessionId,
    /// Requested entity.
    pub requested_entity: EntityId,
    /// Returned objects, response order.
    pub objects: Vec<EntityReadResponseObject>,
    /// Deterministic total charge.
    pub work_units: u64,
}

/// Decodes the exact request record.
///
/// # Errors
///
/// Returns `NotCanonical` on any shape defect.
pub fn decode_entity_read_request(input: &[u8]) -> Result<EntityReadRequest, EntityReadError> {
    let mut cursor = ScbValueCursor::new(input).map_err(|_| EntityReadError::NotCanonical)?;
    let fields = read_ordered_fields(&mut cursor, 5)?;
    let expected_root = StateRoot::from_bytes(request_fixed(fields.first().copied())?);
    let entity = EntityId::from_bytes(request_fixed(fields.get(1).copied())?);
    Ok(EntityReadRequest {
        expected_root,
        entity,
        max_objects: request_uvar(fields.get(2).copied())?,
        max_response_bytes: request_uvar(fields.get(3).copied())?,
        max_work: request_uvar(fields.get(4).copied())?,
    })
}

/// Runs the ordered owner checks and returns the reservation plan without
/// allocating object-sized output.
///
/// # Errors
///
/// Returns the first failing contract check.
pub fn prepare_entity_read<'rev>(
    method: EntityReadMethod,
    revision: &EntityReadRevision<'rev>,
    request: &EntityReadRequest,
    selected: &EntityReadCeilings,
) -> Result<EntityReadPlan<'rev>, EntityReadError> {
    admit_request(request, selected, &revision.root)?;
    let target = resolve_target(revision, &request.entity)?;
    let lookup_cost = lookup_cost(revision.bindings.len())?;
    let mut selection = EntitySelection {
        revision,
        request,
        selected,
        lookup_cost,
        indices: Vec::new(),
        object_count: 0,
        stored_bytes: 0,
    };
    match method {
        EntityReadMethod::Version => selection.select_version(target)?,
        EntityReadMethod::Signature => selection.select_signature(target)?,
    }
    let work = work_bound(
        selection.object_count,
        selection.stored_bytes,
        lookup_cost,
        request.max_response_bytes,
    )?;
    check_work(work, request, selected)?;
    check_collection_count(selection.object_count)?;
    let list_len = response_list_len(revision, &selection.indices)?;
    let body_len = response_body_len(list_len, work)?;
    if body_len > request.max_response_bytes || body_len > selected.max_response_bytes {
        return Err(EntityReadError::BudgetExceeded);
    }
    check_standalone_body(body_len)?;
    usize::try_from(body_len).map_err(|_| EntityReadError::BudgetExceeded)?;
    Ok(EntityReadPlan {
        revision: *revision,
        request: *request,
        indices: selection.indices,
        object_count: selection.object_count,
        stored_bytes: selection.stored_bytes,
        work_units: work,
        body_len,
        list_len,
    })
}

struct EntitySelection<'a, 'b> {
    revision: &'a EntityReadRevision<'a>,
    request: &'b EntityReadRequest,
    selected: &'b EntityReadCeilings,
    lookup_cost: u64,
    indices: Vec<usize>,
    object_count: u64,
    stored_bytes: u64,
}

impl EntitySelection<'_, '_> {
    fn select_version(&mut self, target: (usize, u64)) -> Result<(), EntityReadError> {
        check_byte_payload(target.1)?;
        let work = work_bound(
            1,
            target.1,
            self.lookup_cost,
            self.request.max_response_bytes,
        )?;
        check_work(work, self.request, self.selected)?;
        self.indices.push(target.0);
        self.object_count = 1;
        self.stored_bytes = target.1;
        Ok(())
    }

    fn select_signature(&mut self, target: (usize, u64)) -> Result<(), EntityReadError> {
        let object = self
            .revision
            .objects
            .get(target.0)
            .ok_or(EntityReadError::InternalInvariant)?;
        if u64::from(object.record().body.kind_tag()) != FUNCTION_KIND {
            return Err(EntityReadError::ClassNotApplicable);
        }
        let EntityBodyValue::Function(function) = &object.record().body else {
            return Err(EntityReadError::InternalInvariant);
        };
        check_byte_payload(target.1)?;
        let parameter_count = u64::try_from(function.parameters.len())
            .map_err(|_| EntityReadError::BudgetExceeded)?;
        let total = parameter_count
            .checked_add(1)
            .ok_or(EntityReadError::BudgetExceeded)?;
        if total > self.request.max_objects {
            return Err(EntityReadError::BudgetExceeded);
        }
        let work = work_bound(
            1,
            target.1,
            self.lookup_cost,
            self.request.max_response_bytes,
        )?;
        check_work(work, self.request, self.selected)?;
        self.indices.reserve(function.parameters.len());
        self.indices.push(target.0);
        self.object_count = 1;
        self.stored_bytes = target.1;
        for (ordinal, parameter_id) in function.parameters.iter().enumerate() {
            self.select_parameter(parameter_id, ordinal)?;
        }
        debug_assert_eq!(self.object_count, total);
        Ok(())
    }

    fn select_parameter(
        &mut self,
        parameter_id: &EntityId,
        ordinal: usize,
    ) -> Result<(), EntityReadError> {
        let parameter_index = self
            .revision
            .bindings
            .binary_search_by_key(parameter_id, |(entity, _)| *entity)
            .map_err(|_| EntityReadError::InternalInvariant)?;
        if self.revision.tombstones.binary_search(parameter_id).is_ok() {
            return Err(EntityReadError::InternalInvariant);
        }
        let parameter = self
            .revision
            .objects
            .get(parameter_index)
            .ok_or(EntityReadError::InternalInvariant)?;
        agree(parameter, self.revision, parameter_id, parameter_index)?;
        let parameter_len = stored_len(parameter)?;
        check_byte_payload(parameter_len)?;
        self.stored_bytes = self
            .stored_bytes
            .checked_add(parameter_len)
            .ok_or(EntityReadError::BudgetExceeded)?;
        self.object_count = self
            .object_count
            .checked_add(1)
            .ok_or(EntityReadError::BudgetExceeded)?;
        if self.object_count > self.request.max_objects {
            return Err(EntityReadError::BudgetExceeded);
        }
        let work = work_bound(
            self.object_count,
            self.stored_bytes,
            self.lookup_cost,
            self.request.max_response_bytes,
        )?;
        check_work(work, self.request, self.selected)?;
        let EntityBodyValue::Parameter(parameter_body) = &parameter.record().body else {
            return Err(EntityReadError::InternalInvariant);
        };
        let expected_ordinal =
            u32::try_from(ordinal).map_err(|_| EntityReadError::InternalInvariant)?;
        if parameter_body.owner != self.request.entity
            || parameter_body.role != ParameterRole::Function
            || parameter_body.ordinal != expected_ordinal
        {
            return Err(EntityReadError::InternalInvariant);
        }
        self.indices.push(parameter_index);
        Ok(())
    }
}

fn admit_request(
    request: &EntityReadRequest,
    selected: &EntityReadCeilings,
    root: &StateRoot,
) -> Result<(), EntityReadError> {
    if request.max_objects == 0 || request.max_response_bytes == 0 || request.max_work == 0 {
        return Err(EntityReadError::NotCanonical);
    }
    if request.max_objects > selected.max_entities
        || request.max_response_bytes > selected.max_response_bytes
        || request.max_work > selected.max_work
    {
        return Err(EntityReadError::BudgetExceeded);
    }
    if request.expected_root != *root {
        return Err(EntityReadError::RootMismatch);
    }
    Ok(())
}

fn resolve_target(
    revision: &EntityReadRevision<'_>,
    entity: &EntityId,
) -> Result<(usize, u64), EntityReadError> {
    if revision.tombstones.binary_search(entity).is_ok() {
        return Err(EntityReadError::UnresolvedEntity);
    }
    let index = revision
        .bindings
        .binary_search_by_key(entity, |(bound, _)| *bound)
        .map_err(|_| EntityReadError::UnresolvedEntity)?;
    let target = revision
        .objects
        .get(index)
        .ok_or(EntityReadError::InternalInvariant)?;
    agree(target, revision, entity, index)?;
    Ok((index, stored_len(target)?))
}

fn lookup_cost(bindings: usize) -> Result<u64, EntityReadError> {
    let count =
        u64::try_from(bindings).map_err(|_| EntityReadError::BudgetExceeded)?;
    let bits = if count == 0 { 0 } else { 64 - count.leading_zeros() };
    u64::from(bits)
        .checked_add(1)
        .ok_or(EntityReadError::BudgetExceeded)
}

/// Encodes the complete response for a prepared plan, consuming it.
///
/// The plan already carries every check; the writer emits the exact
/// preflighted bytes into one pre-sized buffer, copying each stored object
/// once. Frame-envelope preflight and work reservation stay with the caller.
///
/// # Errors
///
/// Returns `InternalInvariant` when the written bytes drift from the
/// preflight length.
pub fn encode_entity_read_response(
    plan: EntityReadPlan<'_>,
    session: SessionId,
) -> Result<EntityReadOutcome, EntityReadError> {
    let work_units = plan.work_units();
    let object_count = plan.object_count();
    let body_len = plan.body_len();
    let body = write_response_body(plan, session)?;
    let returned_bytes =
        u64::try_from(body.len()).map_err(|_| EntityReadError::InternalInvariant)?;
    if returned_bytes != body_len {
        return Err(EntityReadError::InternalInvariant);
    }
    Ok(EntityReadOutcome {
        body,
        work_units,
        returned_bytes,
        returned_entities: object_count,
    })
}

fn write_response_body(
    plan: EntityReadPlan<'_>,
    session: SessionId,
) -> Result<Vec<u8>, EntityReadError> {
    let indices = plan.indices;
    let capacity =
        usize::try_from(plan.body_len).map_err(|_| EntityReadError::InternalInvariant)?;
    let mut body = Vec::with_capacity(capacity);
    push_uvar(&mut body, RESPONSE_FIELDS);
    push_uvar_field(&mut body, 1, ENTITY_READ_RESPONSE_VERSION);
    push_fixed_field(&mut body, 2, plan.revision.workspace.as_bytes());
    push_fixed_field(&mut body, 3, plan.revision.root.as_bytes());
    push_fixed_field(&mut body, 4, plan.revision.epoch.as_bytes());
    push_fixed_field(&mut body, 5, session.as_bytes());
    push_fixed_field(&mut body, 6, plan.request.entity.as_bytes());
    push_uvar(&mut body, 7);
    push_uvar(&mut body, plan.list_len);
    push_uvar(&mut body, plan.object_count);
    let mut written_stored: u64 = 0;
    for index in indices {
        let object = plan
            .revision
            .objects
            .get(index)
            .ok_or(EntityReadError::InternalInvariant)?;
        let (entity, object_id) = plan
            .revision
            .bindings
            .get(index)
            .copied()
            .ok_or(EntityReadError::InternalInvariant)?;
        let stored = object.stored_bytes();
        let stored_len =
            u64::try_from(stored.len()).map_err(|_| EntityReadError::InternalInvariant)?;
        written_stored = written_stored
            .checked_add(stored_len)
            .ok_or(EntityReadError::InternalInvariant)?;
        let kind = u64::from(object.record().body.kind_tag());
        let record_len = object_record_len(kind, stored_len)?;
        let inner_len = sized_len(stored_len)?;
        push_uvar(&mut body, record_len);
        push_uvar(&mut body, OBJECT_FIELDS);
        push_fixed_field(&mut body, 1, entity.as_bytes());
        push_uvar_field(&mut body, 2, kind);
        push_fixed_field(&mut body, 3, object_id.as_bytes());
        push_uvar(&mut body, 4);
        push_uvar(&mut body, inner_len);
        push_uvar(&mut body, stored_len);
        body.extend_from_slice(stored);
    }
    push_uvar_field(&mut body, 8, plan.work_units);
    if written_stored != plan.stored_bytes
        || u64::try_from(body.len()).map_err(|_| EntityReadError::InternalInvariant)?
            != plan.body_len
    {
        return Err(EntityReadError::InternalInvariant);
    }
    Ok(body)
}

fn push_uvar(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            return;
        }
    }
}

fn push_uvar_field(out: &mut Vec<u8>, tag: u64, value: u64) {
    push_uvar(out, tag);
    let width = uvar_len(value);
    push_uvar(out, width);
    push_uvar(out, value);
}

fn push_fixed_field(out: &mut Vec<u8>, tag: u64, fixed: &[u8; 32]) {
    push_uvar(out, tag);
    push_uvar(out, 32);
    out.extend_from_slice(fixed);
}

/// Decodes an exact response body.
///
/// # Errors
///
/// Returns `NotCanonical` on any shape defect.
pub fn decode_entity_read_response(input: &[u8]) -> Result<EntityReadResponse, EntityReadError> {
    let mut cursor = ScbValueCursor::new(input).map_err(|_| EntityReadError::NotCanonical)?;
    let fields = read_ordered_fields(&mut cursor, 8)?;
    if response_uvar(fields.first().copied())? != ENTITY_READ_RESPONSE_VERSION {
        return Err(EntityReadError::NotCanonical);
    }
    let list_bytes = fields.get(6).copied().ok_or(EntityReadError::NotCanonical)?;
    let mut list = ScbValueCursor::new(list_bytes).map_err(|_| EntityReadError::NotCanonical)?;
    let elements = list
        .read_list_count()
        .map_err(|_| EntityReadError::NotCanonical)?;
    let mut objects = Vec::new();
    for _ in 0..elements {
        let element = list
            .read_sized_payload()
            .map_err(|_| EntityReadError::NotCanonical)?;
        objects.push(decode_response_object(element)?);
    }
    list.check_finished()
        .map_err(|_| EntityReadError::NotCanonical)?;
    Ok(EntityReadResponse {
        workspace: WorkspaceId::from_bytes(response_fixed(fields.get(1).copied())?),
        root: StateRoot::from_bytes(response_fixed(fields.get(2).copied())?),
        epoch: SchemaEpochId::from_bytes(response_fixed(fields.get(3).copied())?),
        session: SessionId::from_bytes(response_fixed(fields.get(4).copied())?),
        requested_entity: EntityId::from_bytes(response_fixed(fields.get(5).copied())?),
        objects,
        work_units: response_uvar(fields.get(7).copied())?,
    })
}

fn read_ordered_fields<'a>(
    cursor: &mut ScbValueCursor<'a>,
    expected: u64,
) -> Result<Vec<&'a [u8]>, EntityReadError> {
    let count = cursor
        .read_record_field_count()
        .map_err(|_| EntityReadError::NotCanonical)?;
    if count != expected {
        return Err(EntityReadError::NotCanonical);
    }
    let mut fields = Vec::with_capacity(
        usize::try_from(expected).map_err(|_| EntityReadError::NotCanonical)?,
    );
    for position in 0..expected {
        let tag = cursor
            .read_uvar(32)
            .map_err(|_| EntityReadError::NotCanonical)?;
        if tag != position.checked_add(1).ok_or(EntityReadError::NotCanonical)? {
            return Err(EntityReadError::NotCanonical);
        }
        fields.push(
            cursor
                .read_sized_payload()
                .map_err(|_| EntityReadError::NotCanonical)?,
        );
    }
    cursor
        .check_finished()
        .map_err(|_| EntityReadError::NotCanonical)?;
    Ok(fields)
}

fn decode_response_object(input: &[u8]) -> Result<EntityReadResponseObject, EntityReadError> {
    let mut cursor = ScbValueCursor::new(input).map_err(|_| EntityReadError::NotCanonical)?;
    let fields = read_ordered_fields(&mut cursor, 4)?;
    let inner = fields
        .get(3)
        .copied()
        .ok_or(EntityReadError::NotCanonical)?;
    let mut bytes = ScbValueCursor::new(inner).map_err(|_| EntityReadError::NotCanonical)?;
    let stored = bytes
        .read_bytes()
        .map_err(|_| EntityReadError::NotCanonical)?;
    bytes
        .check_finished()
        .map_err(|_| EntityReadError::NotCanonical)?;
    Ok(EntityReadResponseObject {
        entity: EntityId::from_bytes(response_fixed(fields.first().copied())?),
        kind: response_uvar(fields.get(1).copied())?,
        object_id: ObjectId::from_bytes(response_fixed(fields.get(2).copied())?),
        stored_bytes: stored.to_vec(),
    })
}

fn agree(
    object: &EntityObject,
    revision: &EntityReadRevision<'_>,
    entity: &EntityId,
    index: usize,
) -> Result<(), EntityReadError> {
    let (bound_entity, bound_object) = revision
        .bindings
        .get(index)
        .copied()
        .ok_or(EntityReadError::InternalInvariant)?;
    if bound_entity != *entity
        || object.record().entity_id != *entity
        || object.object_id() != bound_object
        || object.schema_epoch_id() != revision.epoch
    {
        return Err(EntityReadError::InternalInvariant);
    }
    Ok(())
}

fn stored_len(object: &EntityObject) -> Result<u64, EntityReadError> {
    u64::try_from(object.stored_bytes().len()).map_err(|_| EntityReadError::BudgetExceeded)
}

fn check_byte_payload(stored: u64) -> Result<(), EntityReadError> {
    let ceiling =
        u64::try_from(MAX_BYTE_PAYLOAD).map_err(|_| EntityReadError::BudgetExceeded)?;
    if stored > ceiling {
        return Err(EntityReadError::BudgetExceeded);
    }
    Ok(())
}

fn check_collection_count(count: u64) -> Result<(), EntityReadError> {
    if count > MAX_COLLECTION_ELEMENTS {
        return Err(EntityReadError::BudgetExceeded);
    }
    Ok(())
}

fn check_standalone_body(body_len: u64) -> Result<(), EntityReadError> {
    let ceiling =
        u64::try_from(MAX_STANDALONE_BYTES).map_err(|_| EntityReadError::BudgetExceeded)?;
    if body_len > ceiling {
        return Err(EntityReadError::BudgetExceeded);
    }
    Ok(())
}

/// Fixed response record field count.
const RESPONSE_FIELDS: u64 = 8;
/// Fixed response object record field count.
const OBJECT_FIELDS: u64 = 4;

fn check_record_ceilings() -> Result<(), EntityReadError> {
    if RESPONSE_FIELDS > MAX_RECORD_FIELDS || OBJECT_FIELDS > MAX_RECORD_FIELDS {
        return Err(EntityReadError::BudgetExceeded);
    }
    Ok(())
}

fn check_work(
    work: u64,
    request: &EntityReadRequest,
    selected: &EntityReadCeilings,
) -> Result<(), EntityReadError> {
    if work > request.max_work || work > selected.budget_before_dispatch {
        return Err(EntityReadError::BudgetExceeded);
    }
    Ok(())
}

fn work_bound(
    objects: u64,
    bytes: u64,
    lookup_cost: u64,
    max_response_bytes: u64,
) -> Result<u64, EntityReadError> {
    let lookups = objects
        .checked_mul(lookup_cost)
        .ok_or(EntityReadError::BudgetExceeded)?;
    let copies = bytes
        .checked_mul(2)
        .ok_or(EntityReadError::BudgetExceeded)?;
    lookups
        .checked_add(copies)
        .and_then(|sum| sum.checked_add(1))
        .and_then(|sum| sum.checked_add(max_response_bytes))
        .ok_or(EntityReadError::BudgetExceeded)
}

fn object_record_len(kind: u64, stored: u64) -> Result<u64, EntityReadError> {
    let inner = sized_len(stored)?;
    checked_sum(&[
        uvar_len(OBJECT_FIELDS),
        field_len(1, 32)?,
        field_len(2, uvar_len(kind))?,
        field_len(3, 32)?,
        field_len(4, inner)?,
    ])
}

fn response_list_len(
    revision: &EntityReadRevision<'_>,
    indices: &[usize],
) -> Result<u64, EntityReadError> {
    check_record_ceilings()?;
    let mut list_content: u64 = 0;
    for index in indices {
        let object = revision
            .objects
            .get(*index)
            .ok_or(EntityReadError::InternalInvariant)?;
        let record_len = object_record_len(
            u64::from(object.record().body.kind_tag()),
            stored_len(object)?,
        )?;
        list_content = list_content
            .checked_add(sized_len(record_len)?)
            .ok_or(EntityReadError::BudgetExceeded)?;
    }
    uvar_len(u64::try_from(indices.len()).map_err(|_| EntityReadError::BudgetExceeded)?)
        .checked_add(list_content)
        .ok_or(EntityReadError::BudgetExceeded)
}

fn response_body_len(list_len: u64, work_units: u64) -> Result<u64, EntityReadError> {
    check_record_ceilings()?;
    checked_sum(&[
        uvar_len(RESPONSE_FIELDS),
        field_len(1, uvar_len(ENTITY_READ_RESPONSE_VERSION))?,
        field_len(2, 32)?,
        field_len(3, 32)?,
        field_len(4, 32)?,
        field_len(5, 32)?,
        field_len(6, 32)?,
        field_len(7, list_len)?,
        field_len(8, uvar_len(work_units))?,
    ])
}

fn field_len(tag: u64, value_len: u64) -> Result<u64, EntityReadError> {
    uvar_len(tag)
        .checked_add(sized_len(value_len)?)
        .ok_or(EntityReadError::BudgetExceeded)
}

fn sized_len(value_len: u64) -> Result<u64, EntityReadError> {
    uvar_len(value_len)
        .checked_add(value_len)
        .ok_or(EntityReadError::BudgetExceeded)
}

fn checked_sum(parts: &[u64]) -> Result<u64, EntityReadError> {
    let mut total: u64 = 0;
    for part in parts {
        total = total
            .checked_add(*part)
            .ok_or(EntityReadError::BudgetExceeded)?;
    }
    Ok(total)
}

const fn uvar_len(value: u64) -> u64 {
    if value < 128 {
        1
    } else if value < 16_384 {
        2
    } else if value < 2_097_152 {
        3
    } else if value < 268_435_456 {
        4
    } else if value < 34_359_738_368 {
        5
    } else if value < 4_398_046_511_104 {
        6
    } else if value < 562_949_953_421_312 {
        7
    } else if value < 72_057_594_037_927_936 {
        8
    } else if value < 9_223_372_036_854_775_808 {
        9
    } else {
        10
    }
}

fn request_fixed(field: Option<&[u8]>) -> Result<[u8; 32], EntityReadError> {
    match field {
        Some(bytes) if bytes.len() == 32 => {
            let mut fixed = [0_u8; 32];
            fixed.copy_from_slice(bytes);
            Ok(fixed)
        }
        _ => Err(EntityReadError::NotCanonical),
    }
}

fn response_fixed(field: Option<&[u8]>) -> Result<[u8; 32], EntityReadError> {
    request_fixed(field)
}

fn request_uvar(field: Option<&[u8]>) -> Result<u64, EntityReadError> {
    let bytes = field.ok_or(EntityReadError::NotCanonical)?;
    let mut cursor = ScbValueCursor::new(bytes).map_err(|_| EntityReadError::NotCanonical)?;
    let value = cursor
        .read_uvar(64)
        .map_err(|_| EntityReadError::NotCanonical)?;
    cursor
        .check_finished()
        .map_err(|_| EntityReadError::NotCanonical)?;
    Ok(value)
}

fn response_uvar(field: Option<&[u8]>) -> Result<u64, EntityReadError> {
    request_uvar(field)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sley_mutate::value::{
        AdapterImportBody, BlockBody, CapabilityRequirementBody, ConstantBody, ContractBody,
        DependencyBindingBody, EffectDefBody, EntityBodyValue, EntityIdSet, EntryPointBody,
        FunctionBody, GlobalValueBody, NamespaceBody, OperationBody, PackageBody, ParameterBody,
        PolicyBindingBody, TestCaseBody, TypeDefBody, WorkspaceBody,
    };
    use sley_mutate::{EntityObjectRecord, build_entity_object};
    use sley_scb1::{encode_record, encode_uvar};
    use sley_ssmc::{
        ConstData, ConstValue, ContractKind, EffectEnvironment, EffectKind, EntryExposure,
        ExpectedOutcome, Immediate, Opcode, ParameterRole, Reachability, ResourceLimits,
        ReturnTerminator, Terminator, TypeDefForm, TypeExpr, TypeParameterDef, ValueRef,
        Visibility,
    };

    fn entity(byte: u8) -> EntityId {
        EntityId::from_bytes([byte; 32])
    }

    fn epoch() -> SchemaEpochId {
        SchemaEpochId::from_bytes([9; 32])
    }

    fn empty_set() -> EntityIdSet {
        EntityIdSet::from_unsorted(Vec::new()).unwrap()
    }

    fn set(ids: &[u8]) -> EntityIdSet {
        EntityIdSet::from_unsorted(ids.iter().map(|byte| entity(*byte)).collect()).unwrap()
    }

    fn build(byte: u8, body: EntityBodyValue) -> EntityObject {
        build_entity_object(
            epoch(),
            &EntityObjectRecord {
                entity_id: entity(byte),
                body,
                label: None,
                semantic_fingerprint: None,
            },
        )
        .unwrap()
    }

    fn workspace_body() -> EntityBodyValue {
        EntityBodyValue::Workspace(WorkspaceBody {
            packages: set(&[2]),
            root_namespace: entity(3),
            capability_requirements: empty_set(),
            contracts: empty_set(),
            tests: empty_set(),
        })
    }

    fn package_body() -> EntityBodyValue {
        EntityBodyValue::Package(PackageBody {
            workspace: entity(1),
            root_namespace: entity(3),
            dependencies: empty_set(),
            exports: empty_set(),
        })
    }

    fn namespace_body() -> EntityBodyValue {
        EntityBodyValue::Namespace(NamespaceBody {
            parent: None,
            members: empty_set(),
        })
    }

    fn typedef_body() -> EntityBodyValue {
        EntityBodyValue::TypeDef(TypeDefBody {
            type_parameters: Vec::new(),
            form: TypeDefForm::Record(Vec::new()),
            invariants: empty_set(),
            visibility: Visibility::Private,
        })
    }

    fn function_body(parameters: &[u8], entry: u8) -> EntityBodyValue {
        EntityBodyValue::Function(FunctionBody {
            type_parameters: Vec::new(),
            parameters: parameters.iter().map(|byte| entity(*byte)).collect(),
            result_type: TypeExpr::Bool,
            effects: empty_set(),
            entry_block: entity(entry),
            blocks: vec![entity(entry)],
            contracts: empty_set(),
            visibility: Visibility::Private,
        })
    }

    fn parameter_body(owner: u8, ordinal: u32) -> EntityBodyValue {
        EntityBodyValue::Parameter(ParameterBody {
            owner: entity(owner),
            role: ParameterRole::Function,
            ordinal,
            value_type: TypeExpr::Bool,
        })
    }

    fn block_body(function: u8) -> EntityBodyValue {
        EntityBodyValue::Block(BlockBody {
            function: entity(function),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::Parameter(entity(6)),
            }),
            reachability: Reachability::Required,
        })
    }

    fn operation_body(block: u8) -> EntityBodyValue {
        EntityBodyValue::Operation(OperationBody {
            block: entity(block),
            ordinal: 0,
            opcode: Opcode::BoolAnd.tag(),
            operands: vec![ValueRef::Parameter(entity(6))],
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::None,
        })
    }

    fn constant_body() -> EntityBodyValue {
        EntityBodyValue::Constant(ConstantBody {
            value: ConstValue {
                value_type: TypeExpr::Bool,
                data: ConstData::Bool(true),
            },
        })
    }

    fn global_body() -> EntityBodyValue {
        EntityBodyValue::GlobalValue(GlobalValueBody {
            value_type: TypeExpr::Bool,
            initializer: entity(9),
            visibility: Visibility::Private,
        })
    }

    fn effect_body() -> EntityBodyValue {
        EntityBodyValue::EffectDef(EffectDefBody {
            effect_kind: EffectKind::StdoutWrite,
            scope_type: TypeExpr::Unit,
            request_type: TypeExpr::Unit,
            response_type: TypeExpr::Unit,
            failure_type: TypeExpr::Unit,
            visibility: Visibility::Private,
        })
    }

    fn requirement_body() -> EntityBodyValue {
        EntityBodyValue::CapabilityRequirement(CapabilityRequirementBody {
            effect: entity(11),
            allowed_scopes: Vec::new(),
            constraint_contracts: empty_set(),
        })
    }

    fn contract_body() -> EntityBodyValue {
        EntityBodyValue::Contract(ContractBody {
            target: entity(5),
            contract_kind: ContractKind::Precondition,
            predicate: entity(5),
            bindings: Vec::new(),
            resource_limits: None,
        })
    }

    fn testcase_body() -> EntityBodyValue {
        EntityBodyValue::TestCase(TestCaseBody {
            target: entity(5),
            inputs: Vec::new(),
            effect_environment: EffectEnvironment::Replay(Vec::new()),
            expected: ExpectedOutcome::FailureCode(1),
            observations: Vec::new(),
            resource_limits: ResourceLimits {
                fuel: 1,
                memory_bytes: 1,
                output_bytes: 1,
                effect_count: 1,
                call_depth: 1,
                wall_timeout_millis: 1,
            },
        })
    }

    fn adapter_body() -> EntityBodyValue {
        EntityBodyValue::AdapterImport(AdapterImportBody {
            adapter_id: [0xA0; 32],
            abi_version: 1,
            request_type: TypeExpr::Unit,
            response_type: TypeExpr::Unit,
            failure_type: TypeExpr::Unit,
            effects: empty_set(),
        })
    }

    fn entry_point_body() -> EntityBodyValue {
        EntityBodyValue::EntryPoint(EntryPointBody {
            function: entity(5),
            exposure: EntryExposure::Local,
        })
    }

    fn policy_binding_body() -> EntityBodyValue {
        EntityBodyValue::PolicyBinding(PolicyBindingBody {
            subject: entity(4),
            requirements: empty_set(),
        })
    }

    fn dependency_binding_body() -> EntityBodyValue {
        EntityBodyValue::DependencyBinding(DependencyBindingBody {
            dependency_root: StateRoot::from_bytes([7; 32]),
            external_package: entity(2),
            local_namespace: entity(3),
        })
    }

    struct Fixture {
        workspace: WorkspaceId,
        root: StateRoot,
        session: SessionId,
        bindings: Vec<(EntityId, ObjectId)>,
        objects: Vec<EntityObject>,
        tombstones: Vec<EntityId>,
    }

    fn eighteen_kind_fixture() -> Fixture {
        let bodies = [
            workspace_body(),
            package_body(),
            namespace_body(),
            typedef_body(),
            function_body(&[6], 7),
            parameter_body(5, 0),
            block_body(5),
            operation_body(7),
            constant_body(),
            global_body(),
            effect_body(),
            requirement_body(),
            contract_body(),
            testcase_body(),
            adapter_body(),
            entry_point_body(),
            policy_binding_body(),
            dependency_binding_body(),
        ];
        let mut objects: Vec<EntityObject> = bodies
            .into_iter()
            .enumerate()
            .map(|(position, body)| {
                let byte = u8::try_from(position + 1).unwrap();
                build(byte, body)
            })
            .collect();
        objects.sort_by_key(|object| object.record().entity_id);
        let bindings = objects
            .iter()
            .map(|object| (object.record().entity_id, object.object_id()))
            .collect();
        Fixture {
            workspace: WorkspaceId::from_bytes([1; 32]),
            root: StateRoot::from_bytes([0x10; 32]),
            session: SessionId::from_bytes([0x20; 32]),
            bindings,
            objects,
            tombstones: Vec::new(),
        }
    }

    fn signature_fixture() -> Fixture {
        let function = EntityBodyValue::Function(FunctionBody {
            type_parameters: vec![TypeParameterDef { ordinal: 0 }],
            parameters: vec![entity(41), entity(42), entity(43)],
            result_type: TypeExpr::Tuple(vec![TypeExpr::Bool, TypeExpr::Text]),
            effects: set(&[44]),
            entry_block: entity(44),
            blocks: vec![entity(44)],
            contracts: set(&[44]),
            visibility: Visibility::Exported,
        });
        let types = [
            TypeExpr::Bool,
            TypeExpr::Text,
            TypeExpr::Option(Box::new(TypeExpr::Bool)),
        ];
        let mut objects = vec![build(40, function)];
        for (position, value_type) in types.into_iter().enumerate() {
            objects.push(
                build_entity_object(
                    epoch(),
                    &EntityObjectRecord {
                        entity_id: entity(41 + u8::try_from(position).unwrap()),
                        body: EntityBodyValue::Parameter(ParameterBody {
                            owner: entity(40),
                            role: ParameterRole::Function,
                            ordinal: u32::try_from(position).unwrap(),
                            value_type,
                        }),
                        label: None,
                        semantic_fingerprint: None,
                    },
                )
                .unwrap(),
            );
        }
        objects.push(build(44, namespace_body()));
        objects.sort_by_key(|object| object.record().entity_id);
        let bindings = objects
            .iter()
            .map(|object| (object.record().entity_id, object.object_id()))
            .collect();
        Fixture {
            workspace: WorkspaceId::from_bytes([1; 32]),
            root: StateRoot::from_bytes([0x10; 32]),
            session: SessionId::from_bytes([0x20; 32]),
            bindings,
            objects,
            tombstones: Vec::new(),
        }
    }

    fn rebuild(fixture: &mut Fixture, byte: u8, body: EntityBodyValue) {
        let object = build(byte, body);
        let position = fixture
            .objects
            .iter()
            .position(|existing| existing.record().entity_id == entity(byte))
            .unwrap();
        fixture.objects[position] = object;
        let rebound = fixture.objects[position].object_id();
        fixture.bindings[position] = (entity(byte), rebound);
    }

    fn view(fixture: &Fixture) -> EntityReadRevision<'_> {
        EntityReadRevision {
            workspace: fixture.workspace,
            root: fixture.root,
            epoch: epoch(),
            bindings: &fixture.bindings,
            objects: &fixture.objects,
            tombstones: &fixture.tombstones,
        }
    }

    fn ceilings() -> EntityReadCeilings {
        EntityReadCeilings {
            max_entities: 65_535,
            max_response_bytes: 67_108_864,
            max_work: 100_000_000,
            budget_before_dispatch: 100_000_000,
        }
    }

    fn request(entity_byte: u8, root: StateRoot) -> EntityReadRequest {
        EntityReadRequest {
            expected_root: root,
            entity: entity(entity_byte),
            max_objects: 65_535,
            max_response_bytes: 67_108_864,
            max_work: 100_000_000,
        }
    }

    fn encode_request(request: &EntityReadRequest) -> Vec<u8> {
        encode_record(&[
            (1, request.expected_root.as_bytes().to_vec()),
            (2, request.entity.as_bytes().to_vec()),
            (3, encode_uvar(request.max_objects)),
            (4, encode_uvar(request.max_response_bytes)),
            (5, encode_uvar(request.max_work)),
        ])
        .unwrap()
    }

    fn raw_record(count: u64, fields: &[(u64, Vec<u8>)]) -> Vec<u8> {
        let mut out = encode_uvar(count);
        for (tag, value) in fields {
            out.extend_from_slice(&encode_uvar(*tag));
            out.extend_from_slice(&encode_uvar(u64::try_from(value.len()).unwrap()));
            out.extend_from_slice(value);
        }
        out
    }

    fn roundtrip(
        method: EntityReadMethod,
        fixture: &Fixture,
        request: &EntityReadRequest,
        selected: &EntityReadCeilings,
    ) -> ((u64, u64, u64), EntityReadOutcome, EntityReadResponse) {
        let revision = view(fixture);
        let plan = prepare_entity_read(method, &revision, request, selected).unwrap();
        let scalars = (plan.work_units(), plan.body_len(), plan.object_count());
        let outcome = encode_entity_read_response(plan, fixture.session).unwrap();
        assert_eq!(
            u64::try_from(outcome.body.len()).unwrap(),
            scalars.1
        );
        assert_eq!(outcome.returned_bytes, scalars.1);
        assert_eq!(outcome.returned_entities, scalars.2);
        assert_eq!(outcome.work_units, scalars.0);
        let response = decode_entity_read_response(&outcome.body).unwrap();
        (scalars, outcome, response)
    }

    #[test]
    fn version_returns_exact_stored_object_for_all_eighteen_kinds() {
        let fixture = eighteen_kind_fixture();
        let selected = ceilings();
        for byte in 1..=18_u8 {
            let request = request(byte, fixture.root);
            let ((work, _, count), outcome, response) =
                roundtrip(EntityReadMethod::Version, &fixture, &request, &selected);
            assert_eq!(count, 1);
            assert_eq!(outcome.returned_entities, 1);
            let object = &fixture.objects[usize::from(byte - 1)];
            let stored_len = u64::try_from(object.stored_bytes().len()).unwrap();
            let lookup_cost = 6;
            let expected_work = 1 + lookup_cost + 2 * stored_len + request.max_response_bytes;
            assert_eq!(work, expected_work);
            assert_eq!(response.workspace, fixture.workspace);
            assert_eq!(response.root, fixture.root);
            assert_eq!(response.epoch, epoch());
            assert_eq!(response.session, fixture.session);
            assert_eq!(response.requested_entity, entity(byte));
            assert_eq!(response.work_units, expected_work);
            assert_eq!(response.objects.len(), 1);
            let returned = &response.objects[0];
            assert_eq!(returned.entity, entity(byte));
            assert_eq!(returned.kind, u64::from(byte));
            assert_eq!(returned.object_id, object.object_id());
            assert_eq!(returned.stored_bytes, object.stored_bytes());
        }
    }

    #[test]
    fn request_bytes_roundtrip_canonically() {
        let fixture = eighteen_kind_fixture();
        let request = request(6, fixture.root);
        let decoded = decode_entity_read_request(&encode_request(&request)).unwrap();
        assert_eq!(decoded, request);
    }

    #[test]
    fn signature_returns_function_then_declaration_ordered_parameters() {
        let fixture = signature_fixture();
        let selected = ceilings();
        let request = request(40, fixture.root);
        let ((_, _, count), _, response) =
            roundtrip(EntityReadMethod::Signature, &fixture, &request, &selected);
        assert_eq!(count, 4);
        assert_eq!(response.objects.len(), 4);
        let expected = [40_u8, 41, 42, 43];
        for (position, byte) in expected.iter().enumerate() {
            let object = &fixture.objects[position];
            let returned = &response.objects[position];
            assert_eq!(returned.entity, entity(*byte));
            assert_eq!(returned.object_id, object.object_id());
            assert_eq!(returned.stored_bytes, object.stored_bytes());
        }
        assert_eq!(response.objects[0].kind, 5);
        assert!(response.objects[1..].iter().all(|object| object.kind == 6));
    }

    #[test]
    fn signature_with_zero_parameters_returns_function_only() {
        let mut fixture = eighteen_kind_fixture();
        rebuild(&mut fixture, 5, function_body(&[], 7));
        let request = request(5, fixture.root);
        let ((_, _, count), _, response) =
            roundtrip(EntityReadMethod::Signature, &fixture, &request, &ceilings());
        assert_eq!(count, 1);
        assert_eq!(response.objects.len(), 1);
        assert_eq!(response.objects[0].entity, entity(5));
    }

    #[test]
    fn signature_on_non_function_is_class_not_applicable() {
        let fixture = eighteen_kind_fixture();
        let revision = view(&fixture);
        let request = request(4, fixture.root);
        let error =
            prepare_entity_read(EntityReadMethod::Signature, &revision, &request, &ceilings())
                .unwrap_err();
        assert_eq!(error, EntityReadError::ClassNotApplicable);
        assert_eq!(error.owner_symbol(), Some("QUERY_CLASS_NOT_APPLICABLE"));
        assert_eq!(error.owner_numeric(), Some(31_010));
    }

    #[test]
    fn unknown_and_tombstoned_entities_are_unresolved() {
        let mut fixture = eighteen_kind_fixture();
        let revision = view(&fixture);
        let missing = request(99, fixture.root);
        let error =
            prepare_entity_read(EntityReadMethod::Version, &revision, &missing, &ceilings())
                .unwrap_err();
        assert_eq!(error, EntityReadError::UnresolvedEntity);
        assert_eq!(error.owner_symbol(), Some("QUERY_UNRESOLVED_ENTITY"));
        assert_eq!(error.owner_numeric(), Some(31_004));
        fixture.tombstones = vec![entity(2)];
        let revision = view(&fixture);
        let tombstoned = request(2, fixture.root);
        assert_eq!(
            prepare_entity_read(EntityReadMethod::Version, &revision, &tombstoned, &ceilings())
                .unwrap_err(),
            EntityReadError::UnresolvedEntity
        );
    }

    #[test]
    fn wrong_expected_root_is_root_mismatch() {
        let fixture = eighteen_kind_fixture();
        let revision = view(&fixture);
        let request = request(1, StateRoot::from_bytes([0x77; 32]));
        let error =
            prepare_entity_read(EntityReadMethod::Version, &revision, &request, &ceilings())
                .unwrap_err();
        assert_eq!(error, EntityReadError::RootMismatch);
        assert_eq!(error.owner_symbol(), Some("QUERY_ROOT_MISMATCH"));
        assert_eq!(error.owner_numeric(), Some(31_008));
    }

    #[test]
    fn malformed_requests_are_not_canonical() {
        let fixture = eighteen_kind_fixture();
        let valid = encode_request(&request(1, fixture.root));
        let mut trailing = valid.clone();
        trailing.push(0);
        let mut truncated = valid.clone();
        truncated.pop();
        let fields: Vec<(u64, Vec<u8>)> = vec![
            (1, fixture.root.as_bytes().to_vec()),
            (2, entity(1).as_bytes().to_vec()),
            (3, encode_uvar(10)),
            (4, encode_uvar(1_000)),
            (5, encode_uvar(1_000)),
        ];
        let short_fixed: Vec<(u64, Vec<u8>)> = vec![
            (1, vec![0_u8; 31]),
            (2, entity(1).as_bytes().to_vec()),
            (3, encode_uvar(10)),
            (4, encode_uvar(1_000)),
            (5, encode_uvar(1_000)),
        ];
        let non_minimal_uvar: Vec<(u64, Vec<u8>)> = vec![
            (1, fixture.root.as_bytes().to_vec()),
            (2, entity(1).as_bytes().to_vec()),
            (3, vec![0x80, 0x00]),
            (4, encode_uvar(1_000)),
            (5, encode_uvar(1_000)),
        ];
        let cases = vec![
            Vec::new(),
            trailing,
            truncated,
            raw_record(4, &fields[..4]),
            raw_record(6, &[&fields[..], &[(6, encode_uvar(1))][..]].concat()),
            raw_record(5, &[fields[1].clone(), fields[0].clone(), fields[2].clone(), fields[3].clone(), fields[4].clone()]),
            raw_record(
                5,
                &[fields[0].clone(), fields[0].clone(), fields[2].clone(), fields[3].clone(), fields[4].clone()],
            ),
            raw_record(5, &short_fixed),
            raw_record(5, &non_minimal_uvar),
            [&[0x80_u8, 0x05][..], &valid[1..]].concat(),
        ];
        for input in cases {
            assert_eq!(
                decode_entity_read_request(&input),
                Err(EntityReadError::NotCanonical),
                "accepted malformed input of length {}",
                input.len()
            );
        }
    }

    #[test]
    fn ceiling_violations_are_budget_exceeded() {
        let fixture = eighteen_kind_fixture();
        let revision = view(&fixture);
        let base = request(1, fixture.root);
        let mut zero_objects = base;
        zero_objects.max_objects = 0;
        assert_eq!(
            prepare_entity_read(EntityReadMethod::Version, &revision, &zero_objects, &ceilings())
                .unwrap_err(),
            EntityReadError::NotCanonical
        );
        let mut over_selected = base;
        over_selected.max_objects = 65_536;
        assert_eq!(
            prepare_entity_read(EntityReadMethod::Version, &revision, &over_selected, &ceilings())
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
        let mut zero_bytes = base;
        zero_bytes.max_response_bytes = 0;
        assert_eq!(
            prepare_entity_read(EntityReadMethod::Version, &revision, &zero_bytes, &ceilings())
                .unwrap_err(),
            EntityReadError::NotCanonical
        );
        let mut zero_work = base;
        zero_work.max_work = 0;
        assert_eq!(
            prepare_entity_read(EntityReadMethod::Version, &revision, &zero_work, &ceilings())
                .unwrap_err(),
            EntityReadError::NotCanonical
        );
        let mut over_work = base;
        over_work.max_work = 100_000_001;
        assert_eq!(
            prepare_entity_read(EntityReadMethod::Version, &revision, &over_work, &ceilings())
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
        let signature = request(5, fixture.root);
        let mut one_object = signature;
        one_object.max_objects = 1;
        assert_eq!(
            prepare_entity_read(EntityReadMethod::Signature, &revision, &one_object, &ceilings())
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
    }

    #[test]
    fn exact_and_one_below_response_and_work_limits() {
        let fixture = signature_fixture();
        let revision = view(&fixture);
        let request = request(40, fixture.root);
        let plan =
            prepare_entity_read(EntityReadMethod::Signature, &revision, &request, &ceilings())
                .unwrap();
        // The work bound charges the ceiling itself, so narrowing the byte
        // ceiling also shrinks the body: walk down to the minimal feasible
        // ceiling and prove the step below it refuses.
        let mut floor = plan.body_len();
        loop {
            let mut probe = request;
            probe.max_response_bytes = floor - 1;
            match prepare_entity_read(
                EntityReadMethod::Signature,
                &revision,
                &probe,
                &ceilings(),
            ) {
                Err(EntityReadError::BudgetExceeded) => break,
                Ok(narrower) => {
                    assert!(narrower.body_len() < floor);
                    floor = narrower.body_len();
                }
                Err(other) => panic!("unexpected error: {other}"),
            }
        }
        assert!(floor <= plan.body_len());
        let mut exact_bytes = request;
        exact_bytes.max_response_bytes = floor;
        let exact = prepare_entity_read(
            EntityReadMethod::Signature,
            &revision,
            &exact_bytes,
            &ceilings(),
        )
        .unwrap();
        assert!(exact.body_len() <= floor);
        let outcome = encode_entity_read_response(exact, fixture.session).unwrap();
        assert!(outcome.returned_bytes <= floor);
        let mut exact_work = request;
        exact_work.max_work = plan.work_units();
        prepare_entity_read(
            EntityReadMethod::Signature,
            &revision,
            &exact_work,
            &ceilings(),
        )
        .unwrap();
        let mut below_work = request;
        below_work.max_work = plan.work_units() - 1;
        assert_eq!(
            prepare_entity_read(
                EntityReadMethod::Signature,
                &revision,
                &below_work,
                &ceilings()
            )
            .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
        let mut exact_budget = ceilings();
        exact_budget.budget_before_dispatch = plan.work_units();
        prepare_entity_read(EntityReadMethod::Signature, &revision, &request, &exact_budget)
            .unwrap();
        let mut below_budget = ceilings();
        below_budget.budget_before_dispatch = plan.work_units() - 1;
        assert_eq!(
            prepare_entity_read(EntityReadMethod::Signature, &revision, &request, &below_budget)
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
    }

    #[test]
    fn checked_overflow_is_budget_exceeded() {
        let fixture = eighteen_kind_fixture();
        let revision = view(&fixture);
        let mut request = request(1, fixture.root);
        request.max_response_bytes = u64::MAX;
        request.max_work = u64::MAX;
        let mut selected = ceilings();
        selected.max_response_bytes = u64::MAX;
        selected.max_work = u64::MAX;
        selected.budget_before_dispatch = u64::MAX;
        assert_eq!(
            prepare_entity_read(EntityReadMethod::Version, &revision, &request, &selected)
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
    }

    #[test]
    fn signature_relationship_defects_are_internal_invariant() {
        let fixture = signature_fixture();
        let missing = EntityBodyValue::Function(FunctionBody {
            type_parameters: Vec::new(),
            parameters: vec![entity(99)],
            result_type: TypeExpr::Bool,
            effects: empty_set(),
            entry_block: entity(44),
            blocks: vec![entity(44)],
            contracts: empty_set(),
            visibility: Visibility::Private,
        });
        let mut objects = vec![build(40, missing)];
        for byte in [41_u8, 42, 43, 44] {
            objects.push(fixture.objects.iter().find(|object| object.record().entity_id == entity(byte)).unwrap().clone());
        }
        objects.sort_by_key(|object| object.record().entity_id);
        let bindings = objects
            .iter()
            .map(|object| (object.record().entity_id, object.object_id()))
            .collect::<Vec<_>>();
        let broken = EntityReadRevision {
            workspace: fixture.workspace,
            root: fixture.root,
            epoch: epoch(),
            bindings: &bindings,
            objects: &objects,
            tombstones: &[],
        };
        assert_eq!(
            prepare_entity_read(EntityReadMethod::Signature, &broken, &request(40, fixture.root), &ceilings())
                .unwrap_err(),
            EntityReadError::InternalInvariant
        );
    }

    #[test]
    fn parameter_owner_ordinal_and_role_defects_are_internal_invariant() {
        for variant in 0..3_u8 {
            let mut fixture = signature_fixture();
            let mut body = parameter_body(40, 1);
            let EntityBodyValue::Parameter(inner) = &mut body else {
                unreachable!();
            };
            match variant {
                0 => inner.owner = entity(44),
                1 => inner.ordinal = 7,
                _ => inner.role = ParameterRole::Block,
            }
            rebuild(&mut fixture, 42, body);
            let revision = view(&fixture);
            assert_eq!(
                prepare_entity_read(
                    EntityReadMethod::Signature,
                    &revision,
                    &request(40, fixture.root),
                    &ceilings()
                )
                .unwrap_err(),
                EntityReadError::InternalInvariant,
                "variant {variant} accepted"
            );
        }
    }

    #[test]
    fn binding_and_epoch_disagreement_is_internal_invariant() {
        let mut fixture = eighteen_kind_fixture();
        fixture.bindings.swap(0, 1);
        let revision = view(&fixture);
        assert_eq!(
            prepare_entity_read(EntityReadMethod::Version, &revision, &request(1, fixture.root), &ceilings())
                .unwrap_err(),
            EntityReadError::InternalInvariant
        );
        let mut fixture = eighteen_kind_fixture();
        let foreign = build_entity_object(
            SchemaEpochId::from_bytes([8; 32]),
            &EntityObjectRecord {
                entity_id: entity(1),
                body: workspace_body(),
                label: None,
                semantic_fingerprint: None,
            },
        )
        .unwrap();
        fixture.objects[0] = foreign;
        fixture.bindings[0] = (entity(1), fixture.objects[0].object_id());
        let revision = view(&fixture);
        assert_eq!(
            prepare_entity_read(EntityReadMethod::Version, &revision, &request(1, fixture.root), &ceilings())
                .unwrap_err(),
            EntityReadError::InternalInvariant
        );
    }

    #[test]
    fn plan_captures_immutable_inputs() {
        let fixture = eighteen_kind_fixture();
        let revision = view(&fixture);
        let mut request = request(1, fixture.root);
        let mut selected = ceilings();
        let plan =
            prepare_entity_read(EntityReadMethod::Version, &revision, &request, &selected)
                .unwrap();
        let work = plan.work_units();
        let body_len = plan.body_len();
        let count = plan.object_count();
        request.expected_root = StateRoot::from_bytes([0x77; 32]);
        request.max_objects = 1;
        selected.max_work = 1;
        selected.budget_before_dispatch = 1;
        assert_eq!(
            prepare_entity_read(EntityReadMethod::Version, &revision, &request, &selected)
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
        let outcome = encode_entity_read_response(plan, fixture.session).unwrap();
        assert_eq!(outcome.work_units, work);
        assert_eq!(outcome.returned_bytes, body_len);
        assert_eq!(outcome.returned_entities, count);
    }

    #[test]
    fn owner_codes_are_stable_and_budget_errors_stay_transport_neutral() {
        assert_eq!(
            EntityReadError::InternalInvariant.owner_symbol(),
            Some("QUERY_INTERNAL_INVARIANT")
        );
        assert_eq!(EntityReadError::InternalInvariant.owner_numeric(), Some(31_007));
        assert_eq!(EntityReadError::NotCanonical.owner_symbol(), None);
        assert_eq!(EntityReadError::NotCanonical.owner_numeric(), None);
        assert_eq!(EntityReadError::BudgetExceeded.owner_symbol(), None);
        assert_eq!(EntityReadError::BudgetExceeded.owner_numeric(), None);
    }

    fn push_uvar(out: &mut Vec<u8>, mut value: u64) {
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if value == 0 {
                return;
            }
        }
    }

    fn push_sized(out: &mut Vec<u8>, bytes: &[u8]) {
        push_uvar(out, u64::try_from(bytes.len()).unwrap());
        out.extend_from_slice(bytes);
    }

    fn push_field(out: &mut Vec<u8>, tag: u64, value: &[u8]) {
        push_uvar(out, tag);
        push_sized(out, value);
    }

    fn manual_object_record(entity: EntityId, kind: u64, object: &EntityObject) -> Vec<u8> {
        let mut inner = Vec::new();
        push_uvar(&mut inner, u64::try_from(object.stored_bytes().len()).unwrap());
        inner.extend_from_slice(object.stored_bytes());
        let mut record = Vec::new();
        push_uvar(&mut record, 4);
        push_field(&mut record, 1, entity.as_bytes());
        let kind_bytes = {
            let mut encoded = Vec::new();
            push_uvar(&mut encoded, kind);
            encoded
        };
        push_field(&mut record, 2, &kind_bytes);
        push_field(&mut record, 3, object.object_id().as_bytes());
        push_field(&mut record, 4, &inner);
        record
    }

    fn manual_response_body(
        fixture: &Fixture,
        session: SessionId,
        requested: EntityId,
        entries: &[(EntityId, u64, &EntityObject)],
        work_units: u64,
    ) -> Vec<u8> {
        let mut list_content = Vec::new();
        push_uvar(&mut list_content, u64::try_from(entries.len()).unwrap());
        for (entity, kind, object) in entries {
            push_sized(
                &mut list_content,
                &manual_object_record(*entity, *kind, object),
            );
        }
        let version_bytes = {
            let mut encoded = Vec::new();
            push_uvar(&mut encoded, ENTITY_READ_RESPONSE_VERSION);
            encoded
        };
        let work_bytes = {
            let mut encoded = Vec::new();
            push_uvar(&mut encoded, work_units);
            encoded
        };
        let mut body = Vec::new();
        push_uvar(&mut body, 8);
        push_field(&mut body, 1, &version_bytes);
        push_field(&mut body, 2, fixture.workspace.as_bytes());
        push_field(&mut body, 3, fixture.root.as_bytes());
        push_field(&mut body, 4, epoch().as_bytes());
        push_field(&mut body, 5, session.as_bytes());
        push_field(&mut body, 6, requested.as_bytes());
        push_field(&mut body, 7, &list_content);
        push_field(&mut body, 8, &work_bytes);
        body
    }

    #[test]
    fn response_bytes_match_independent_canonical_assembly() {
        let fixture = eighteen_kind_fixture();
        let selected = ceilings();
        let request = request(9, fixture.root);
        let revision = view(&fixture);
        let plan = prepare_entity_read(EntityReadMethod::Version, &revision, &request, &selected)
            .unwrap();
        let work = plan.work_units();
        let outcome = encode_entity_read_response(plan, fixture.session).unwrap();
        let object = &fixture.objects[8];
        let expected = manual_response_body(
            &fixture,
            fixture.session,
            entity(9),
            &[(entity(9), 9, object)],
            work,
        );
        assert_eq!(outcome.body, expected);
        let response = decode_entity_read_response(&expected).unwrap();
        assert_eq!(response.objects[0].stored_bytes, object.stored_bytes());
    }

    #[test]
    fn version_one_response_body_is_not_canonical() {
        // Vector `resp_version_1` in `conformance/entity-read/v2/rejected.json`:
        // a response record naming version 1 under the version 2 decoder is
        // `NotCanonical`, which keeps no owner code, so the vector pins the
        // `response_record` layer rather than a code.
        let fixture = eighteen_kind_fixture();
        let selected = ceilings();
        let request = request(9, fixture.root);
        let revision = view(&fixture);
        let plan = prepare_entity_read(EntityReadMethod::Version, &revision, &request, &selected)
            .unwrap();
        let work = plan.work_units();
        let object = &fixture.objects[8];
        let valid = manual_response_body(
            &fixture,
            fixture.session,
            entity(9),
            &[(entity(9), 9, object)],
            work,
        );
        // Field 1 is the sized response version: record-of-8, tag 1,
        // length 1, value 2.
        assert_eq!(&valid[0..4], &[8, 1, 1, 2]);
        let mut versioned_down = valid.clone();
        versioned_down[3] = 1;
        assert_eq!(
            decode_entity_read_response(&versioned_down),
            Err(EntityReadError::NotCanonical)
        );
    }

    #[test]
    fn inner_length_defects_are_not_canonical() {
        let fixture = eighteen_kind_fixture();
        let object = &fixture.objects[8];
        let valid_inner = {
            let mut inner = Vec::new();
            push_uvar(&mut inner, u64::try_from(object.stored_bytes().len()).unwrap());
            inner.extend_from_slice(object.stored_bytes());
            inner
        };
        let overlong_inner = {
            let mut inner = Vec::new();
            push_uvar(
                &mut inner,
                u64::try_from(object.stored_bytes().len()).unwrap() + 1,
            );
            inner.extend_from_slice(object.stored_bytes());
            inner
        };
        let nonminimal_inner = {
            let mut inner = vec![
            0x80 | u8::try_from(object.stored_bytes().len() & 0x7f).unwrap(),
            0x00,
        ];
            inner.extend_from_slice(object.stored_bytes());
            inner
        };
        let mut trailing_inner = valid_inner.clone();
        trailing_inner.push(0x00);
        for inner in [overlong_inner, nonminimal_inner, trailing_inner] {
            let mut record = Vec::new();
            push_uvar(&mut record, 4);
            push_field(&mut record, 1, entity(9).as_bytes());
            let mut kind_bytes = Vec::new();
            push_uvar(&mut kind_bytes, 9);
            push_field(&mut record, 2, &kind_bytes);
            push_field(&mut record, 3, object.object_id().as_bytes());
            push_field(&mut record, 4, &inner);
            let mut list_content = Vec::new();
            push_uvar(&mut list_content, 1);
            push_sized(&mut list_content, &record);
            let mut version_bytes = Vec::new();
            push_uvar(&mut version_bytes, ENTITY_READ_RESPONSE_VERSION);
            let mut work_bytes = Vec::new();
            push_uvar(&mut work_bytes, 1);
            let mut body = Vec::new();
            push_uvar(&mut body, 8);
            push_field(&mut body, 1, &version_bytes);
            push_field(&mut body, 2, fixture.workspace.as_bytes());
            push_field(&mut body, 3, fixture.root.as_bytes());
            push_field(&mut body, 4, epoch().as_bytes());
            push_field(&mut body, 5, fixture.session.as_bytes());
            push_field(&mut body, 6, entity(9).as_bytes());
            push_field(&mut body, 7, &list_content);
            push_field(&mut body, 8, &work_bytes);
            assert_eq!(
                decode_entity_read_response(&body),
                Err(EntityReadError::NotCanonical)
            );
        }
    }

    #[test]
    fn zero_ceilings_are_not_canonical_before_root_checks() {
        let fixture = eighteen_kind_fixture();
        let revision = view(&fixture);
        let base = request(1, fixture.root);
        for ceiling in 0..3_u8 {
            let mut defective = base;
            match ceiling {
                0 => defective.max_objects = 0,
                1 => defective.max_response_bytes = 0,
                _ => defective.max_work = 0,
            }
            assert_eq!(
                prepare_entity_read(EntityReadMethod::Version, &revision, &defective, &ceilings())
                    .unwrap_err(),
                EntityReadError::NotCanonical,
                "zero ceiling {ceiling} accepted"
            );
        }
        let mut competing = base;
        competing.max_objects = 0;
        competing.expected_root = StateRoot::from_bytes([0x77; 32]);
        assert_eq!(
            prepare_entity_read(EntityReadMethod::Version, &revision, &competing, &ceilings())
                .unwrap_err(),
            EntityReadError::NotCanonical
        );
    }

    #[test]
    fn signature_preserves_nonmonotonic_declaration_order() {
        let mut fixture = signature_fixture();
        rebuild(
            &mut fixture,
            40,
            EntityBodyValue::Function(FunctionBody {
                type_parameters: Vec::new(),
                parameters: vec![entity(43), entity(41), entity(42)],
                result_type: TypeExpr::Bool,
                effects: empty_set(),
                entry_block: entity(44),
                blocks: vec![entity(44)],
                contracts: empty_set(),
                visibility: Visibility::Private,
            }),
        );
        for (byte, ordinal) in [(43_u8, 0_u32), (41, 1), (42, 2)] {
            rebuild(&mut fixture, byte, parameter_body(40, ordinal));
        }
        let request = request(40, fixture.root);
        let ((_, _, _), _, response) =
            roundtrip(EntityReadMethod::Signature, &fixture, &request, &ceilings());
        let order: Vec<EntityId> = response.objects.iter().map(|object| object.entity).collect();
        assert_eq!(order, vec![entity(40), entity(43), entity(41), entity(42)]);
    }

    #[test]
    fn parameter_binding_and_epoch_defects_rejected_in_preparation() {
        let mut fixture = signature_fixture();
        let position = fixture
            .bindings
            .iter()
            .position(|(bound, _)| *bound == entity(42))
            .unwrap();
        fixture.bindings[position].1 = ObjectId::from_bytes([0xAA; 32]);
        let revision = view(&fixture);
        assert_eq!(
            prepare_entity_read(
                EntityReadMethod::Signature,
                &revision,
                &request(40, fixture.root),
                &ceilings()
            )
            .unwrap_err(),
            EntityReadError::InternalInvariant
        );
        let mut fixture = signature_fixture();
        let foreign = build_entity_object(
            SchemaEpochId::from_bytes([8; 32]),
            &EntityObjectRecord {
                entity_id: entity(42),
                body: parameter_body(40, 1),
                label: None,
                semantic_fingerprint: None,
            },
        )
        .unwrap();
        let position = fixture
            .objects
            .iter()
            .position(|object| object.record().entity_id == entity(42))
            .unwrap();
        fixture.objects[position] = foreign;
        fixture.bindings[position] = (entity(42), fixture.objects[position].object_id());
        let revision = view(&fixture);
        assert_eq!(
            prepare_entity_read(
                EntityReadMethod::Signature,
                &revision,
                &request(40, fixture.root),
                &ceilings()
            )
            .unwrap_err(),
            EntityReadError::InternalInvariant
        );
    }

    #[test]
    fn oversized_stored_object_refused_before_copy() {
        let fixture = eighteen_kind_fixture();
        let bulky = EntityBodyValue::Function(FunctionBody {
            type_parameters: Vec::new(),
            parameters: vec![entity(60); 1_000_000],
            result_type: TypeExpr::Bool,
            effects: empty_set(),
            entry_block: entity(44),
            blocks: vec![entity(44)],
            contracts: empty_set(),
            visibility: Visibility::Private,
        });
        let object = build(50, bulky);
        assert!(u64::try_from(object.stored_bytes().len()).unwrap() > 16_777_216);
        let bindings = vec![(entity(50), object.object_id())];
        let objects = vec![object];
        let revision = EntityReadRevision {
            workspace: fixture.workspace,
            root: fixture.root,
            epoch: epoch(),
            bindings: &bindings,
            objects: &objects,
            tombstones: &[],
        };
        let mut selected = ceilings();
        selected.max_work = u64::MAX;
        selected.max_response_bytes = u64::MAX;
        selected.budget_before_dispatch = u64::MAX;
        let mut request = request(50, fixture.root);
        request.max_response_bytes = 40_000_000;
        request.max_work = u64::MAX;
        assert_eq!(
            prepare_entity_read(EntityReadMethod::Version, &revision, &request, &selected)
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
    }

    fn entity_n(n: u32) -> EntityId {
        let mut bytes = [0x60_u8; 32];
        bytes[0..4].copy_from_slice(&n.to_be_bytes());
        EntityId::from_bytes(bytes)
    }

    #[test]
    fn oversized_non_function_reports_applicability_before_resources() {
        let fixture = eighteen_kind_fixture();
        let members: Vec<EntityId> = (0..600_000_u32).map(entity_n).collect();
        let bulky = EntityBodyValue::Namespace(NamespaceBody {
            parent: None,
            members: EntityIdSet::from_unsorted(members).unwrap(),
        });
        let object = build(50, bulky);
        assert!(u64::try_from(object.stored_bytes().len()).unwrap() > 16_777_216);
        let bindings = vec![(entity(50), object.object_id())];
        let objects = vec![object];
        let revision = EntityReadRevision {
            workspace: fixture.workspace,
            root: fixture.root,
            epoch: epoch(),
            bindings: &bindings,
            objects: &objects,
            tombstones: &[],
        };
        let mut selected = ceilings();
        selected.max_work = u64::MAX;
        selected.max_response_bytes = u64::MAX;
        selected.budget_before_dispatch = u64::MAX;
        let mut request = request(50, fixture.root);
        request.max_response_bytes = 40_000_000;
        request.max_work = u64::MAX;
        assert_eq!(
            prepare_entity_read(EntityReadMethod::Signature, &revision, &request, &selected)
                .unwrap_err(),
            EntityReadError::ClassNotApplicable
        );
        assert_eq!(
            prepare_entity_read(EntityReadMethod::Version, &revision, &request, &selected)
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
    }

    #[test]
    fn response_body_has_exact_final_capacity_and_length() {
        let fixture = signature_fixture();
        let revision = view(&fixture);
        let request = request(40, fixture.root);
        let plan =
            prepare_entity_read(EntityReadMethod::Signature, &revision, &request, &ceilings())
                .unwrap();
        let outcome = encode_entity_read_response(plan, fixture.session).unwrap();
        assert_eq!(outcome.body.capacity(), outcome.body.len());
    }

    #[test]
    fn non_selected_bodies_are_never_semantically_extracted() {
        // I3b owner-boundary proof (spec sections 5 and 7): the owner
        // touches only the selected indices by binary lookup. Every
        // non-selected entry below carries a body that fails loudly if
        // semantically extracted, plus one large decoy whose bytes must
        // never enter accounting or the response copy. A valid target
        // read therefore succeeds with scalars and bytes identical to
        // the clean fixture; whole-root extraction or a root-wide cache
        // build would trip the poison first.
        let selected = ceilings();
        let clean = eighteen_kind_fixture();
        let target = request(5, clean.root);
        let (clean_scalars, clean_outcome, _) =
            roundtrip(EntityReadMethod::Signature, &clean, &target, &selected);
        assert_eq!(clean_scalars.2, 2);

        let mut poisoned = eighteen_kind_fixture();
        rebuild(&mut poisoned, 9, parameter_body(99, 7));
        rebuild(
            &mut poisoned,
            10,
            EntityBodyValue::Function(FunctionBody {
                type_parameters: Vec::new(),
                parameters: vec![entity(90), entity(91)],
                result_type: TypeExpr::Bool,
                effects: empty_set(),
                entry_block: entity(92),
                blocks: vec![entity(92)],
                contracts: empty_set(),
                visibility: Visibility::Private,
            }),
        );
        rebuild(&mut poisoned, 11, operation_body(93));
        rebuild(
            &mut poisoned,
            12,
            EntityBodyValue::Function(FunctionBody {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                result_type: TypeExpr::Tuple(
                    (0..20_000).map(|_| TypeExpr::Bool).collect(),
                ),
                effects: empty_set(),
                entry_block: entity(94),
                blocks: vec![entity(94)],
                contracts: empty_set(),
                visibility: Visibility::Private,
            }),
        );
        let decoy_len = u64::try_from(
            poisoned
                .objects
                .iter()
                .find(|object| object.record().entity_id == entity(12))
                .unwrap()
                .stored_bytes()
                .len(),
        )
        .unwrap();
        assert!(
            decoy_len > 10_000,
            "decoy must dwarf the answer; got {decoy_len}"
        );

        let revision = view(&poisoned);
        let (scalars, outcome, response) =
            roundtrip(EntityReadMethod::Signature, &poisoned, &target, &selected);
        assert_eq!(
            scalars, clean_scalars,
            "decoy bytes must not enter work accounting"
        );
        assert_eq!(
            outcome.body, clean_outcome.body,
            "decoy bytes must not enter the response copy"
        );
        assert_eq!(response.objects.len(), 2);
        // No cache or shared state: a repeated preparation agrees exactly.
        let again =
            prepare_entity_read(EntityReadMethod::Signature, &revision, &target, &selected)
                .unwrap();
        assert_eq!(
            (
                again.work_units(),
                again.body_len(),
                again.object_count()
            ),
            scalars
        );
        // Sensitivity control: the poison is real; selecting a poisoned
        // Function with a dangling parameter fails loudly.
        assert_eq!(
            prepare_entity_read(
                EntityReadMethod::Signature,
                &revision,
                &request(10, poisoned.root),
                &selected
            )
            .unwrap_err(),
            EntityReadError::InternalInvariant
        );
    }
}
