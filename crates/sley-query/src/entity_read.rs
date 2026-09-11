//! Bounded exact entity and signature reads over borrowed verified objects
//! (AT-MW-02, contract `docs/spec/ENTITY_READ_PROFILE_V2.md` sections 3-5).
//!
//! Pure S20-310 owner: resolves one entity, or one Function plus its ordered
//! parameters, against borrowed verified bindings and objects, charges the
//! deterministic work bound, and encodes the exact response. It performs no
//! I/O, builds no whole-root index, and owns no session or transport debit:
//! [`prepare_entity_read`] runs every check and returns a borrowed selection
//! without allocating object-sized output; the caller runs the frame-envelope
//! preflight and reserves `work_units - 1`; [`capture_entity_read_selection`]
//! then copies the selected bytes and [`encode_entity_read_response`] writes
//! the preflighted bytes into one pre-sized buffer. Frame-envelope preflight
//! stays with the protocol caller, which owns envelope accounting.

use core::fmt;

use sley_id::{EntityId, ObjectId, SchemaEpochId, SessionId, StateRoot, WorkspaceId};
use sley_scb1::{
    ScbValueCursor, MAX_BYTE_PAYLOAD, MAX_COLLECTION_ELEMENTS, MAX_RECORD_FIELDS,
    MAX_STANDALONE_BYTES,
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

/// Borrowed narrow view of one verified stored object for one response.
///
/// The view carries exactly the scalar surface the S20-310 owner may use:
/// identity, kind tag, object identity, epoch, stored bytes, and the
/// Function/Parameter relationship facts that signature selection needs.
/// Every other body is opaque (`Other`). The view borrows the verified
/// revision through the repository adapter, which owns the projection from
/// stored objects; the owner never names a mutation-owned type, so
/// `sley-query` keeps the `sley-query -> sley-check -> sley-ssmc` dependency
/// direction that the S20-250 contract requires.
#[derive(Clone, Copy, Debug)]
pub struct EntityReadObject<'a> {
    /// Bound entity identity.
    pub entity: EntityId,
    /// SSMC1 kind tag.
    pub kind: u16,
    /// Stored object identity.
    pub object_id: ObjectId,
    /// Schema epoch encoded in the object envelope.
    pub epoch: SchemaEpochId,
    /// Exact stored object bytes.
    pub stored_bytes: &'a [u8],
    /// Narrow relationship facts for signature selection.
    pub body: EntityReadBody<'a>,
}

/// Borrowed narrow body facts for signature selection.
///
/// Only Function parameters and Parameter links are exposed; all sixteen
/// other kinds are opaque. This is a read-only projection for one purpose,
/// in the same category as the S20-250 `ImpactEntity` view, not a second
/// body model.
#[derive(Clone, Copy, Debug)]
pub enum EntityReadBody<'a> {
    /// Function body: ordered parameter identities in declaration order.
    Function {
        /// Ordered parameter identities.
        parameters: &'a [EntityId],
    },
    /// Parameter body: back-link to the owning function.
    Parameter {
        /// Owning entity identity.
        owner: EntityId,
        /// Parameter role.
        role: ParameterRole,
        /// Declaration ordinal.
        ordinal: u32,
    },
    /// Any other kind: opaque except for its tag.
    Other {
        /// SSMC1 kind tag.
        kind: u16,
    },
}

/// Borrowed verified revision view for one response.
///
/// The caller binds this revision to the live session root before entry; it
/// names the bound root facts plus the bindings and tombstones. Objects are
/// never carried here: the caller supplies them one index at a time through
/// the `object_at` lookup, so the owner touches only the selected indices
/// and builds no root-wide view.
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

/// Checked selection for one request: the caller runs the frame preflight
/// and reserves `work_units - 1`, then captures with
/// [`capture_entity_read_selection`].
///
/// Borrowing and allocation-free by construction: the only object data held
/// here are the borrowed [`EntityReadObject`] views resolved during
/// selection, so no object-sized output exists before the caller establishes
/// the frame ceiling and the reservation. The borrow checker pins the views
/// to the revision `prepare_entity_read` selected from: while a selection
/// lives, no replacement revision, request, limit, or object can reach the
/// selected bytes, and one selection captures once.
#[derive(Debug)]
pub struct EntityReadSelection<'o> {
    workspace: WorkspaceId,
    root: StateRoot,
    epoch: SchemaEpochId,
    request: EntityReadRequest,
    views: Vec<EntityReadObject<'o>>,
    object_count: u64,
    stored_bytes: u64,
    work_units: u64,
    body_len: u64,
    list_len: u64,
}

/// Checked preflight for one request: the caller runs the frame preflight,
/// reserves `work_units - 1`, captures with
/// [`capture_entity_read_selection`], then encodes with
/// [`encode_entity_read_response`].
///
/// Opaque and immutable: the only constructor is
/// [`capture_entity_read_selection`], which copies the selected objects'
/// exact bytes out of a checked selection after the caller established the
/// frame ceiling and the reservation. The encoder consumes the plan plus the
/// session only, so no replacement method, revision, request, limit, or
/// object can reach encoding, and one preparation encodes once.
#[derive(Debug)]
pub struct EntityReadPlan {
    workspace: WorkspaceId,
    root: StateRoot,
    epoch: SchemaEpochId,
    request: EntityReadRequest,
    selected: Vec<SelectedObject>,
    object_count: u64,
    stored_bytes: u64,
    work_units: u64,
    body_len: u64,
    list_len: u64,
}

/// One selected object captured into the plan after the caller established
/// the frame ceiling and the reservation.
///
/// The stored bytes are copied only after the full ceiling, work, length,
/// frame-preflight, and reservation checks succeed, preserving the
/// refuse-before-result-allocation order of `ENTITY_READ_PROFILE_V2`
/// section 5: no output allocation precedes the reservation.
#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedObject {
    entity: EntityId,
    kind: u64,
    object_id: ObjectId,
    stored: Vec<u8>,
}

impl<'o> EntityReadSelection<'o> {
    /// Returns the deterministic total charge, including the dispatch unit.
    ///
    /// The caller reserves `work_units - 1` after the frame preflight.
    #[must_use]
    pub const fn work_units(&self) -> u64 {
        self.work_units
    }

    /// Returns the exact response body length for the frame preflight.
    #[must_use]
    pub const fn body_len(&self) -> u64 {
        self.body_len
    }

    /// Returns the exact returned object count.
    #[must_use]
    pub const fn object_count(&self) -> u64 {
        self.object_count
    }

    /// Returns the borrowed selected views in response order.
    ///
    /// Test and audit access only: capturing, not re-resolution, consumes
    /// these views, so the selected bytes cannot drift between selection
    /// and capture.
    #[must_use]
    pub fn views(&self) -> &[EntityReadObject<'o>] {
        &self.views
    }
}

impl EntityReadPlan {
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

/// Runs the ordered owner checks and returns the borrowed selection without
/// allocating object-sized output.
///
/// `view_at` resolves one binding index to its borrowed narrow view from the
/// caller-supplied `ctx`. The context is passed by value (it is only ever a
/// shared reference the caller already holds), so the resolved views borrow
/// the caller's revision directly instead of a function-local closure: the
/// owner calls it only for the selected indices, so no root-wide view is
/// ever built. No stored bytes are copied here: the caller runs the frame
/// preflight and the reservation against the returned selection, then
/// copies with [`capture_entity_read_selection`].
///
/// # Errors
///
/// Returns the first failing contract check.
pub fn prepare_entity_read<'a, Ctx: Copy>(
    method: EntityReadMethod,
    revision: &EntityReadRevision<'_>,
    request: &EntityReadRequest,
    selected: &EntityReadCeilings,
    ctx: Ctx,
    view_at: fn(Ctx, usize) -> Option<EntityReadObject<'a>>,
) -> Result<EntityReadSelection<'a>, EntityReadError> {
    admit_request(request, selected, &revision.root)?;
    let object_at = |index: usize| view_at(ctx, index);
    let target = resolve_target(revision, &request.entity, &object_at)?;
    let lookup_cost = lookup_cost(revision.bindings.len())?;
    let mut selection = EntitySelection {
        revision,
        request,
        selected,
        object_at: &object_at,
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
    let mut views = Vec::with_capacity(selection.indices.len());
    for index in &selection.indices {
        let view = view_at(ctx, *index).ok_or(EntityReadError::InternalInvariant)?;
        let (bound_entity, _) = revision
            .bindings
            .get(*index)
            .copied()
            .ok_or(EntityReadError::InternalInvariant)?;
        agree(&view, revision, &bound_entity, *index)?;
        views.push(view);
    }
    let list_len = response_list_len(&views)?;
    let body_len = response_body_len(list_len, work)?;
    if body_len > request.max_response_bytes || body_len > selected.max_response_bytes {
        return Err(EntityReadError::BudgetExceeded);
    }
    check_standalone_body(body_len)?;
    usize::try_from(body_len).map_err(|_| EntityReadError::BudgetExceeded)?;
    Ok(EntityReadSelection {
        workspace: revision.workspace,
        root: revision.root,
        epoch: revision.epoch,
        request: *request,
        views,
        object_count: selection.object_count,
        stored_bytes: selection.stored_bytes,
        work_units: work,
        body_len,
        list_len,
    })
}

/// Copies the selected objects' exact bytes out of a checked selection.
///
/// The caller runs this only after the frame preflight and the
/// `work_units - 1` reservation succeed, so no output allocation precedes
/// either gate. Capture consumes the held views instead of re-resolving
/// through the lookup: the bytes cannot drift between selection and
/// capture, and the surviving identity checks re-pin each view to the
/// revision the selection was bound under.
///
/// # Errors
///
/// Returns the first failing byte-ceiling or identity check. A failure here
/// is an unexpected post-reservation failure: the caller retains the
/// complete debit and returns no object bytes.
pub fn capture_entity_read_selection(
    selection: EntityReadSelection<'_>,
) -> Result<EntityReadPlan, EntityReadError> {
    let EntityReadSelection {
        workspace,
        root,
        epoch,
        request,
        views,
        object_count,
        stored_bytes,
        work_units,
        body_len,
        list_len,
    } = selection;
    let mut captured = Vec::with_capacity(views.len());
    for view in &views {
        let stored_len =
            u64::try_from(view.stored_bytes.len()).map_err(|_| EntityReadError::BudgetExceeded)?;
        check_byte_payload(stored_len)?;
        captured.push(SelectedObject {
            entity: view.entity,
            kind: u64::from(view.kind),
            object_id: view.object_id,
            stored: view.stored_bytes.to_vec(),
        });
    }
    Ok(EntityReadPlan {
        workspace,
        root,
        epoch,
        request,
        selected: captured,
        object_count,
        stored_bytes,
        work_units,
        body_len,
        list_len,
    })
}

struct EntitySelection<'a, 'b, 'c> {
    revision: &'a EntityReadRevision<'a>,
    request: &'b EntityReadRequest,
    selected: &'b EntityReadCeilings,
    object_at: &'c dyn Fn(usize) -> Option<EntityReadObject<'c>>,
    lookup_cost: u64,
    indices: Vec<usize>,
    object_count: u64,
    stored_bytes: u64,
}

impl EntitySelection<'_, '_, '_> {
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
        let object = (self.object_at)(target.0).ok_or(EntityReadError::InternalInvariant)?;
        if u64::from(object.kind) != FUNCTION_KIND {
            return Err(EntityReadError::ClassNotApplicable);
        }
        let EntityReadBody::Function { parameters } = object.body else {
            return Err(EntityReadError::InternalInvariant);
        };
        check_byte_payload(target.1)?;
        let parameter_count =
            u64::try_from(parameters.len()).map_err(|_| EntityReadError::BudgetExceeded)?;
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
        self.indices.reserve(parameters.len());
        self.indices.push(target.0);
        self.object_count = 1;
        self.stored_bytes = target.1;
        for (ordinal, parameter_id) in parameters.iter().enumerate() {
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
        let parameter =
            (self.object_at)(parameter_index).ok_or(EntityReadError::InternalInvariant)?;
        agree(&parameter, self.revision, parameter_id, parameter_index)?;
        let parameter_len = stored_len(&parameter)?;
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
        let EntityReadBody::Parameter {
            owner,
            role,
            ordinal: actual,
        } = parameter.body
        else {
            return Err(EntityReadError::InternalInvariant);
        };
        let expected_ordinal =
            u32::try_from(ordinal).map_err(|_| EntityReadError::InternalInvariant)?;
        if owner != self.request.entity
            || role != ParameterRole::Function
            || actual != expected_ordinal
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

fn resolve_target<'o>(
    revision: &EntityReadRevision<'_>,
    entity: &EntityId,
    object_at: &'o dyn Fn(usize) -> Option<EntityReadObject<'o>>,
) -> Result<(usize, u64), EntityReadError> {
    if revision.tombstones.binary_search(entity).is_ok() {
        return Err(EntityReadError::UnresolvedEntity);
    }
    let index = revision
        .bindings
        .binary_search_by_key(entity, |(bound, _)| *bound)
        .map_err(|_| EntityReadError::UnresolvedEntity)?;
    let target = object_at(index).ok_or(EntityReadError::InternalInvariant)?;
    agree(&target, revision, entity, index)?;
    Ok((index, stored_len(&target)?))
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
    plan: EntityReadPlan,
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
    plan: EntityReadPlan,
    session: SessionId,
) -> Result<Vec<u8>, EntityReadError> {
    let selected = plan.selected;
    let capacity =
        usize::try_from(plan.body_len).map_err(|_| EntityReadError::InternalInvariant)?;
    let mut body = Vec::with_capacity(capacity);
    push_uvar(&mut body, RESPONSE_FIELDS);
    push_uvar_field(&mut body, 1, ENTITY_READ_RESPONSE_VERSION);
    push_fixed_field(&mut body, 2, plan.workspace.as_bytes());
    push_fixed_field(&mut body, 3, plan.root.as_bytes());
    push_fixed_field(&mut body, 4, plan.epoch.as_bytes());
    push_fixed_field(&mut body, 5, session.as_bytes());
    push_fixed_field(&mut body, 6, plan.request.entity.as_bytes());
    push_uvar(&mut body, 7);
    push_uvar(&mut body, plan.list_len);
    push_uvar(&mut body, plan.object_count);
    let mut written_stored: u64 = 0;
    for object in &selected {
        let stored_len =
            u64::try_from(object.stored.len()).map_err(|_| EntityReadError::InternalInvariant)?;
        written_stored = written_stored
            .checked_add(stored_len)
            .ok_or(EntityReadError::InternalInvariant)?;
        let record_len = object_record_len(object.kind, stored_len)?;
        let inner_len = sized_len(stored_len)?;
        push_uvar(&mut body, record_len);
        push_uvar(&mut body, OBJECT_FIELDS);
        push_fixed_field(&mut body, 1, object.entity.as_bytes());
        push_uvar_field(&mut body, 2, object.kind);
        push_fixed_field(&mut body, 3, object.object_id.as_bytes());
        push_uvar(&mut body, 4);
        push_uvar(&mut body, inner_len);
        push_uvar(&mut body, stored_len);
        body.extend_from_slice(&object.stored);
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
    object: &EntityReadObject<'_>,
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
        || object.entity != *entity
        || object.object_id != bound_object
        || object.epoch != revision.epoch
    {
        return Err(EntityReadError::InternalInvariant);
    }
    Ok(())
}

fn stored_len(object: &EntityReadObject<'_>) -> Result<u64, EntityReadError> {
    u64::try_from(object.stored_bytes.len()).map_err(|_| EntityReadError::BudgetExceeded)
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

fn response_list_len(views: &[EntityReadObject<'_>]) -> Result<u64, EntityReadError> {
    check_record_ceilings()?;
    let mut list_content: u64 = 0;
    for object in views {
        let record_len = object_record_len(u64::from(object.kind), stored_len(object)?)?;
        list_content = list_content
            .checked_add(sized_len(record_len)?)
            .ok_or(EntityReadError::BudgetExceeded)?;
    }
    uvar_len(u64::try_from(views.len()).map_err(|_| EntityReadError::BudgetExceeded)?)
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
    use sley_scb1::{encode_record, encode_uvar};

    fn entity(byte: u8) -> EntityId {
        EntityId::from_bytes([byte; 32])
    }

    fn object(byte: u8) -> ObjectId {
        ObjectId::from_bytes([byte; 32])
    }

    fn epoch() -> SchemaEpochId {
        SchemaEpochId::from_bytes([9; 32])
    }

    /// Body descriptor for one fixture object.
    ///
    /// Function carries its ordered parameter identities and Parameter
    /// carries its back-link; every other kind is opaque with an explicit
    /// tag. Fixture views borrow the owned parameter lists, so the revision
    /// exercises the same narrow surface the repository adapter projects
    /// from real stored objects.
    #[derive(Clone, Debug)]
    enum FixtureBody {
        Function {
            parameters: Vec<EntityId>,
        },
        Parameter {
            owner: u8,
            ordinal: u32,
            role: ParameterRole,
        },
        Other,
    }

    struct Slot {
        entity: EntityId,
        object_id: ObjectId,
        kind: u16,
        body: FixtureBody,
        stored: Vec<u8>,
        epoch: SchemaEpochId,
    }

    struct Fixture {
        workspace: WorkspaceId,
        root: StateRoot,
        session: SessionId,
        bindings: Vec<(EntityId, ObjectId)>,
        slots: Vec<Slot>,
        tombstones: Vec<EntityId>,
    }

    impl Fixture {
        fn views(&self) -> Vec<EntityReadObject<'_>> {
            self.slots
                .iter()
                .map(|slot| {
                    let body = match &slot.body {
                        FixtureBody::Function { parameters } => {
                            EntityReadBody::Function { parameters }
                        }
                        FixtureBody::Parameter {
                            owner,
                            ordinal,
                            role,
                        } => EntityReadBody::Parameter {
                            owner: entity(*owner),
                            role: *role,
                            ordinal: *ordinal,
                        },
                        FixtureBody::Other => EntityReadBody::Other { kind: slot.kind },
                    };
                    EntityReadObject {
                        entity: slot.entity,
                        kind: slot.kind,
                        object_id: slot.object_id,
                        epoch: slot.epoch,
                        stored_bytes: &slot.stored,
                        body,
                    }
                })
                .collect()
        }

        fn revision(&self) -> EntityReadRevision<'_> {
            EntityReadRevision {
                workspace: self.workspace,
                root: self.root,
                epoch: epoch(),
                bindings: &self.bindings,
                tombstones: &self.tombstones,
            }
        }

        fn prepare(
            &self,
            method: EntityReadMethod,
            request: &EntityReadRequest,
            selected: &EntityReadCeilings,
        ) -> Result<EntityReadPlan, EntityReadError> {
            let views = self.views();
            let revision = self.revision();
            let selection = prepare_entity_read(
                method,
                &revision,
                request,
                selected,
                &views,
                fixture_view_at,
            )?;
            capture_entity_read_selection(selection)
        }

        fn roundtrip(
            &self,
            method: EntityReadMethod,
            request: &EntityReadRequest,
            selected: &EntityReadCeilings,
        ) -> ((u64, u64, u64), EntityReadOutcome, EntityReadResponse) {
            let plan = self.prepare(method, request, selected).unwrap();
            let scalars = (plan.work_units(), plan.body_len(), plan.object_count());
            let outcome = encode_entity_read_response(plan, self.session).unwrap();
            assert_eq!(u64::try_from(outcome.body.len()).unwrap(), scalars.1);
            assert_eq!(outcome.returned_bytes, scalars.1);
            assert_eq!(outcome.returned_entities, scalars.2);
            assert_eq!(outcome.work_units, scalars.0);
            let response = decode_entity_read_response(&outcome.body).unwrap();
            (scalars, outcome, response)
        }

        fn position_of(&self, byte: u8) -> usize {
            self.slots
                .iter()
                .position(|slot| slot.entity == entity(byte))
                .unwrap()
        }
    }

    /// Synthetic stored bytes for one fixture object: opaque to the owner,
    /// which copies but never interprets them. Lengths stay far below every
    /// byte ceiling; oversized cases set their own lengths explicitly.
    fn stored_bytes(byte: u8) -> Vec<u8> {
        vec![byte; 64 + usize::from(byte)]
    }

    /// Test lookup over one fixture's owned view vector, matching the
    /// production caller shape (context passed by value, views borrowed
    /// from the fixture rather than a function-local closure).
    fn fixture_view_at<'a>(
        views: &Vec<EntityReadObject<'a>>,
        index: usize,
    ) -> Option<EntityReadObject<'a>> {
        views.get(index).copied()
    }

    fn assemble(bindings: &[u8], kinds: &[u16], bodies: &[FixtureBody]) -> Fixture {
        assert_eq!(bindings.len(), kinds.len());
        assert_eq!(bindings.len(), bodies.len());
        let slots = bindings
            .iter()
            .enumerate()
            .map(|(position, byte)| Slot {
                entity: entity(*byte),
                object_id: object(*byte),
                kind: kinds[position],
                body: bodies[position].clone(),
                stored: stored_bytes(*byte),
                epoch: epoch(),
            })
            .collect::<Vec<_>>();
        let bindings = slots
            .iter()
            .map(|slot| (slot.entity, slot.object_id))
            .collect();
        Fixture {
            workspace: WorkspaceId::from_bytes([1; 32]),
            root: StateRoot::from_bytes([0x10; 32]),
            session: SessionId::from_bytes([0x20; 32]),
            bindings,
            slots,
            tombstones: Vec::new(),
        }
    }

    fn eighteen_kind_fixture() -> Fixture {
        let bytes: Vec<u8> = (1..=18_u8).collect();
        let kinds: Vec<u16> = (1..=18_u16).collect();
        let bodies: Vec<FixtureBody> = (1..=18_u8)
            .map(|byte| match byte {
                5 => FixtureBody::Function {
                    parameters: vec![entity(6)],
                },
                6 => FixtureBody::Parameter {
                    owner: 5,
                    ordinal: 0,
                    role: ParameterRole::Function,
                },
                _ => FixtureBody::Other,
            })
            .collect();
        assemble(&bytes, &kinds, &bodies)
    }

    fn signature_fixture() -> Fixture {
        assemble(
            &[40, 41, 42, 43, 44],
            &[5, 6, 6, 6, 3],
            &[
                FixtureBody::Function {
                    parameters: vec![entity(41), entity(42), entity(43)],
                },
                FixtureBody::Parameter {
                    owner: 40,
                    ordinal: 0,
                    role: ParameterRole::Function,
                },
                FixtureBody::Parameter {
                    owner: 40,
                    ordinal: 1,
                    role: ParameterRole::Function,
                },
                FixtureBody::Parameter {
                    owner: 40,
                    ordinal: 2,
                    role: ParameterRole::Function,
                },
                FixtureBody::Other,
            ],
        )
    }

    fn set_function(fixture: &mut Fixture, byte: u8, parameters: &[u8]) {
        let position = fixture.position_of(byte);
        fixture.slots[position].kind = 5;
        fixture.slots[position].body = FixtureBody::Function {
            parameters: parameters.iter().map(|member| entity(*member)).collect(),
        };
    }

    fn set_parameter(fixture: &mut Fixture, byte: u8, owner: u8, ordinal: u32) {
        let position = fixture.position_of(byte);
        fixture.slots[position].kind = 6;
        fixture.slots[position].body = FixtureBody::Parameter {
            owner,
            ordinal,
            role: ParameterRole::Function,
        };
    }

    fn set_other(fixture: &mut Fixture, byte: u8, kind: u16) {
        let position = fixture.position_of(byte);
        fixture.slots[position].kind = kind;
        fixture.slots[position].body = FixtureBody::Other;
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

    #[test]
    fn admit_orders_zero_ceiling_before_over_selected_ceiling() {
        let fixture = eighteen_kind_fixture();
        let selected = ceilings();
        // A zero object ceiling and an over-selected response ceiling
        // compete: the zero check runs first (NotCanonical, not BudgetExceeded).
        let mut zero = request(1, fixture.root);
        zero.max_objects = 0;
        zero.max_response_bytes = selected.max_response_bytes + 1;
        assert_eq!(
            fixture
                .prepare(EntityReadMethod::Version, &zero, &selected)
                .unwrap_err(),
            EntityReadError::NotCanonical
        );
        // An over-selected response ceiling alone is BudgetExceeded.
        let mut over = request(1, fixture.root);
        over.max_response_bytes = selected.max_response_bytes + 1;
        assert_eq!(
            fixture
                .prepare(EntityReadMethod::Version, &over, &selected)
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
    }

    #[test]
    fn version_returns_exact_stored_object_for_all_eighteen_kinds() {
        let fixture = eighteen_kind_fixture();
        let selected = ceilings();
        let views = fixture.views();
        for byte in 1..=18_u8 {
            let request = request(byte, fixture.root);
            let ((work, _, count), outcome, response) =
                fixture.roundtrip(EntityReadMethod::Version, &request, &selected);
            assert_eq!(count, 1);
            assert_eq!(outcome.returned_entities, 1);
            let object = &views[usize::from(byte - 1)];
            let stored_len = u64::try_from(object.stored_bytes.len()).unwrap();
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
            assert_eq!(returned.object_id, object.object_id);
            assert_eq!(returned.stored_bytes, object.stored_bytes);
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
            fixture.roundtrip(EntityReadMethod::Signature, &request, &selected);
        assert_eq!(count, 4);
        assert_eq!(response.objects.len(), 4);
        let expected = [40_u8, 41, 42, 43];
        let views = fixture.views();
        for (position, byte) in expected.iter().enumerate() {
            let object = &views[position];
            let returned = &response.objects[position];
            assert_eq!(returned.entity, entity(*byte));
            assert_eq!(returned.object_id, object.object_id);
            assert_eq!(returned.stored_bytes, object.stored_bytes);
        }
        assert_eq!(response.objects[0].kind, 5);
        assert!(response.objects[1..].iter().all(|object| object.kind == 6));
    }

    #[test]
    fn signature_with_zero_parameters_returns_function_only() {
        let mut fixture = eighteen_kind_fixture();
        set_function(&mut fixture, 5, &[]);
        let request = request(5, fixture.root);
        let ((_, _, count), _, response) =
            fixture.roundtrip(EntityReadMethod::Signature, &request, &ceilings());
        assert_eq!(count, 1);
        assert_eq!(response.objects.len(), 1);
        assert_eq!(response.objects[0].entity, entity(5));
    }

    #[test]
    fn signature_on_non_function_is_class_not_applicable() {
        let fixture = eighteen_kind_fixture();
        let request = request(4, fixture.root);
        let error = fixture
            .prepare(EntityReadMethod::Signature, &request, &ceilings())
            .unwrap_err();
        assert_eq!(error, EntityReadError::ClassNotApplicable);
        assert_eq!(error.owner_symbol(), Some("QUERY_CLASS_NOT_APPLICABLE"));
        assert_eq!(error.owner_numeric(), Some(31_010));
    }

    #[test]
    fn unknown_and_tombstoned_entities_are_unresolved() {
        let mut fixture = eighteen_kind_fixture();
        let missing = request(99, fixture.root);
        let error = fixture
            .prepare(EntityReadMethod::Version, &missing, &ceilings())
            .unwrap_err();
        assert_eq!(error, EntityReadError::UnresolvedEntity);
        assert_eq!(error.owner_symbol(), Some("QUERY_UNRESOLVED_ENTITY"));
        assert_eq!(error.owner_numeric(), Some(31_004));
        fixture.tombstones = vec![entity(2)];
        let tombstoned = request(2, fixture.root);
        assert_eq!(
            fixture
                .prepare(EntityReadMethod::Version, &tombstoned, &ceilings())
                .unwrap_err(),
            EntityReadError::UnresolvedEntity
        );
    }

    #[test]
    fn wrong_expected_root_is_root_mismatch() {
        let fixture = eighteen_kind_fixture();
        let request = request(1, StateRoot::from_bytes([0x77; 32]));
        let error = fixture
            .prepare(EntityReadMethod::Version, &request, &ceilings())
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
        let base = request(1, fixture.root);
        let mut zero_objects = base;
        zero_objects.max_objects = 0;
        assert_eq!(
            fixture
                .prepare(EntityReadMethod::Version, &zero_objects, &ceilings())
                .unwrap_err(),
            EntityReadError::NotCanonical
        );
        let mut over_selected = base;
        over_selected.max_objects = 65_536;
        assert_eq!(
            fixture
                .prepare(EntityReadMethod::Version, &over_selected, &ceilings())
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
        let mut zero_bytes = base;
        zero_bytes.max_response_bytes = 0;
        assert_eq!(
            fixture
                .prepare(EntityReadMethod::Version, &zero_bytes, &ceilings())
                .unwrap_err(),
            EntityReadError::NotCanonical
        );
        let mut zero_work = base;
        zero_work.max_work = 0;
        assert_eq!(
            fixture
                .prepare(EntityReadMethod::Version, &zero_work, &ceilings())
                .unwrap_err(),
            EntityReadError::NotCanonical
        );
        let mut over_work = base;
        over_work.max_work = 100_000_001;
        assert_eq!(
            fixture
                .prepare(EntityReadMethod::Version, &over_work, &ceilings())
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
        let signature = request(5, fixture.root);
        let mut one_object = signature;
        one_object.max_objects = 1;
        assert_eq!(
            fixture
                .prepare(EntityReadMethod::Signature, &one_object, &ceilings())
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
    }

    #[test]
    fn exact_and_one_below_response_and_work_limits() {
        let fixture = signature_fixture();
        let request = request(40, fixture.root);
        let plan = fixture
            .prepare(EntityReadMethod::Signature, &request, &ceilings())
            .unwrap();
        // The work bound charges the ceiling itself, so narrowing the byte
        // ceiling also shrinks the body: walk down to the minimal feasible
        // ceiling and prove the step below it refuses.
        let mut floor = plan.body_len();
        loop {
            let mut probe = request;
            probe.max_response_bytes = floor - 1;
            match fixture.prepare(EntityReadMethod::Signature, &probe, &ceilings()) {
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
        let exact = fixture
            .prepare(EntityReadMethod::Signature, &exact_bytes, &ceilings())
            .unwrap();
        assert!(exact.body_len() <= floor);
        let outcome = encode_entity_read_response(exact, fixture.session).unwrap();
        assert!(outcome.returned_bytes <= floor);
        let mut exact_work = request;
        exact_work.max_work = plan.work_units();
        fixture
            .prepare(EntityReadMethod::Signature, &exact_work, &ceilings())
            .unwrap();
        let mut below_work = request;
        below_work.max_work = plan.work_units() - 1;
        assert_eq!(
            fixture
                .prepare(EntityReadMethod::Signature, &below_work, &ceilings())
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
        let mut exact_budget = ceilings();
        exact_budget.budget_before_dispatch = plan.work_units();
        fixture
            .prepare(EntityReadMethod::Signature, &request, &exact_budget)
            .unwrap();
        let mut below_budget = ceilings();
        below_budget.budget_before_dispatch = plan.work_units() - 1;
        assert_eq!(
            fixture
                .prepare(EntityReadMethod::Signature, &request, &below_budget)
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
    }

    #[test]
    fn checked_overflow_is_budget_exceeded() {
        let fixture = eighteen_kind_fixture();
        let mut request = request(1, fixture.root);
        request.max_response_bytes = u64::MAX;
        request.max_work = u64::MAX;
        let mut selected = ceilings();
        selected.max_response_bytes = u64::MAX;
        selected.max_work = u64::MAX;
        selected.budget_before_dispatch = u64::MAX;
        assert_eq!(
            fixture
                .prepare(EntityReadMethod::Version, &request, &selected)
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
    }

    #[test]
    fn signature_relationship_defects_are_internal_invariant() {
        let mut fixture = signature_fixture();
        set_function(&mut fixture, 40, &[99]);
        assert_eq!(
            fixture
                .prepare(
                    EntityReadMethod::Signature,
                    &request(40, fixture.root),
                    &ceilings()
                )
                .unwrap_err(),
            EntityReadError::InternalInvariant
        );
    }

    #[test]
    fn parameter_owner_ordinal_and_role_defects_are_internal_invariant() {
        for variant in 0..3_u8 {
            let mut fixture = signature_fixture();
            match variant {
                0 => set_parameter(&mut fixture, 42, 44, 1),
                1 => set_parameter(&mut fixture, 42, 40, 7),
                _ => {
                    let position = fixture.position_of(42);
                    fixture.slots[position].body = FixtureBody::Parameter {
                        owner: 40,
                        ordinal: 1,
                        role: ParameterRole::Block,
                    };
                }
            }
            assert_eq!(
                fixture
                    .prepare(
                        EntityReadMethod::Signature,
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
        assert_eq!(
            fixture
                .prepare(
                    EntityReadMethod::Version,
                    &request(1, fixture.root),
                    &ceilings()
                )
                .unwrap_err(),
            EntityReadError::InternalInvariant
        );
        let mut fixture = eighteen_kind_fixture();
        fixture.slots[0].epoch = SchemaEpochId::from_bytes([8; 32]);
        assert_eq!(
            fixture
                .prepare(
                    EntityReadMethod::Version,
                    &request(1, fixture.root),
                    &ceilings()
                )
                .unwrap_err(),
            EntityReadError::InternalInvariant
        );
    }

    #[test]
    fn plan_captures_immutable_inputs() {
        let mut fixture = eighteen_kind_fixture();
        let mut tampered = request(1, fixture.root);
        let mut selected = ceilings();
        let plan = fixture
            .prepare(EntityReadMethod::Version, &tampered, &selected)
            .unwrap();
        let work = plan.work_units();
        let body_len = plan.body_len();
        let count = plan.object_count();
        tampered.expected_root = StateRoot::from_bytes([0x77; 32]);
        tampered.max_objects = 1;
        selected.max_work = 1;
        selected.budget_before_dispatch = 1;
        assert_eq!(
            fixture
                .prepare(EntityReadMethod::Version, &tampered, &selected)
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
        // The plan owns its captured objects: replacing the fixture's stored
        // bytes after preparation cannot reach encoding.
        let clean_body = encode_entity_read_response(plan, fixture.session).unwrap();
        let plan = fixture
            .prepare(
                EntityReadMethod::Version,
                &request(1, fixture.root),
                &ceilings(),
            )
            .unwrap();
        fixture.slots[0].stored = vec![0xFF; 999];
        fixture.slots[0].kind = 18;
        let outcome = encode_entity_read_response(plan, fixture.session).unwrap();
        assert_eq!(outcome.body, clean_body.body);
        assert_eq!(outcome.work_units, work);
        assert_eq!(outcome.returned_bytes, body_len);
        assert_eq!(outcome.returned_entities, count);
    }

    #[test]
    fn selection_borrows_selected_bytes_and_captures_only_at_capture() {
        let fixture = eighteen_kind_fixture();
        let views = fixture.views();
        let revision = fixture.revision();
        let request = request(1, fixture.root);
        let ceilings = ceilings();
        let selection = prepare_entity_read(
            EntityReadMethod::Version,
            &revision,
            &request,
            &ceilings,
            &views,
            fixture_view_at,
        )
        .unwrap();
        assert_eq!(selection.object_count(), 1);
        assert_eq!(selection.views().len(), 1);
        // No object-sized allocation at prepare: the selected view borrows
        // the fixture slot's bytes in place, so the frame preflight and the
        // reservation both precede any output allocation.
        let slot = fixture
            .slots
            .iter()
            .find(|slot| slot.entity == entity(1))
            .unwrap();
        assert!(core::ptr::eq(
            selection.views()[0].stored_bytes,
            slot.stored.as_slice()
        ));
        // Capture copies exactly those borrowed bytes into the owned plan.
        let plan = capture_entity_read_selection(selection).unwrap();
        let outcome = encode_entity_read_response(plan, fixture.session).unwrap();
        assert_eq!(outcome.returned_entities, 1);
        let response = decode_entity_read_response(&outcome.body).unwrap();
        assert_eq!(response.objects.len(), 1);
        assert_eq!(response.objects[0].stored_bytes, slot.stored);
    }

    /// Every accepted vector of the independent entity-read corpus is
    /// reproduced by the owner and the encoder: request bytes decode,
    /// selection resolves, capture copies, and the emitted body plus the
    /// work charge equal the recorded vector.
    ///
    /// The corpus is read-only test data (the same `include_str!` pattern
    /// the scb1 and mutation corpora use): no oracle code runs here, so
    /// S20-130 independence is untouched. Rejected vectors stay with the
    /// oracle checker plus the hand-built failure tests; only success
    /// vectors have owner-comparable bytes.
    #[test]
    fn accepted_corpus_vectors_match_owner_and_encoder() {
        let accepted: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/entity-read/v2/accepted.json"
        ))
        .unwrap();
        let inputs: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/entity-read/v2/inputs.json"
        ))
        .unwrap();
        let cases = accepted["cases"].as_object().unwrap();
        assert_eq!(cases.len(), 23, "accepted corpus case count");
        let (workspace, root, epoch, session, selected) = corpus_context(&inputs);
        let entities = inputs["entities"].as_object().unwrap();
        let authored = inputs["cases"].as_object().unwrap();
        for (id, case) in cases {
            let method = match authored[id]["method"].as_u64().unwrap() {
                306 => EntityReadMethod::Version,
                307 => EntityReadMethod::Signature,
                tag => panic!("{id}: unknown method tag {tag}"),
            };
            let request_bytes = hex_bytes(case["request_body_hex"].as_str().unwrap());
            let request = decode_entity_read_request(&request_bytes)
                .unwrap_or_else(|error| panic!("{id}: request decode: {error:?}"));
            let buffers = assemble_corpus_buffers(id, case, entities);
            let (views, mut bindings) = build_corpus_views(id, entities, &buffers, epoch);
            pad_corpus_bindings(id, &mut bindings);
            bindings.sort();
            let mut ordered: Vec<Option<EntityReadObject<'_>>> = vec![None; bindings.len()];
            for view in views {
                let position = bindings
                    .binary_search_by_key(&view.entity, |(entity, _)| *entity)
                    .unwrap_or_else(|_| panic!("{id}: view entity missing from bindings"));
                assert!(
                    ordered[position].is_none(),
                    "{id}: duplicate binding for view entity"
                );
                ordered[position] = Some(view);
            }
            let revision = EntityReadRevision {
                workspace,
                root,
                epoch,
                bindings: bindings.as_slice(),
                tombstones: &[],
            };
            let selection = prepare_entity_read(
                method,
                &revision,
                &request,
                &selected,
                &ordered,
                corpus_view_at,
            )
            .unwrap_or_else(|error| panic!("{id}: prepare: {error:?}"));
            let plan = capture_entity_read_selection(selection).unwrap();
            let outcome = encode_entity_read_response(plan, session).unwrap();
            let expected_body = hex_bytes(case["response_body_hex"].as_str().unwrap());
            assert_eq!(outcome.body, expected_body, "{id}: response bytes");
            assert_eq!(
                outcome.work_units,
                case["work"].as_u64().unwrap(),
                "{id}: work charge"
            );
            assert_eq!(
                outcome.returned_entities,
                case["count_k"].as_u64().unwrap(),
                "{id}: object count"
            );
        }
    }

    /// One corpus case's owned side buffers: stored bytes, parameter lists,
    /// and the raw row index. Views borrow these, so they are built before
    /// any view exists.
    struct CorpusBuffers {
        stored: Vec<Vec<u8>>,
        parameters: Vec<Vec<EntityId>>,
        raws: Vec<CorpusRaw>,
    }

    struct CorpusRaw {
        name: String,
        entity: EntityId,
        kind: u64,
        object_id: ObjectId,
        stored_index: usize,
        parameter_index: Option<usize>,
    }

    fn assemble_corpus_buffers(
        _id: &str,
        case: &serde_json::Value,
        entities: &serde_json::Map<String, serde_json::Value>,
    ) -> CorpusBuffers {
        let mut buffers = CorpusBuffers {
            stored: Vec::new(),
            parameters: Vec::new(),
            raws: Vec::new(),
        };
        for (name, object) in case["objects"].as_object().unwrap() {
            let entity = EntityId::from_bytes(hex32(entities[name]["id"].as_str().unwrap()));
            let object_id = ObjectId::from_bytes(hex32(object["object_id"].as_str().unwrap()));
            buffers
                .stored
                .push(hex_bytes(object["stored_hex"].as_str().unwrap()));
            let stored_index = buffers.stored.len() - 1;
            let kind = entities[name]["kind"].as_u64().unwrap();
            let parameter_index = if kind == 5 {
                let parameters: Vec<EntityId> = entities[name]["body"]["parameters"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| EntityId::from_bytes(hex32(value.as_str().unwrap())))
                    .collect();
                buffers.parameters.push(parameters);
                Some(buffers.parameters.len() - 1)
            } else {
                None
            };
            buffers.raws.push(CorpusRaw {
                name: name.clone(),
                entity,
                kind,
                object_id,
                stored_index,
                parameter_index,
            });
        }
        buffers
    }

    /// Borrowed narrow views plus their bindings for one corpus case.
    fn build_corpus_views<'b>(
        id: &str,
        entities: &serde_json::Map<String, serde_json::Value>,
        buffers: &'b CorpusBuffers,
        epoch: SchemaEpochId,
    ) -> (Vec<EntityReadObject<'b>>, Vec<(EntityId, ObjectId)>) {
        let mut views: Vec<EntityReadObject<'b>> = Vec::new();
        let mut bindings: Vec<(EntityId, ObjectId)> = Vec::new();
        for raw in &buffers.raws {
            let body = match raw.kind {
                5 => EntityReadBody::Function {
                    parameters: buffers.parameters[raw.parameter_index.unwrap()].as_slice(),
                },
                6 => {
                    let facts = &entities[raw.name.as_str()]["body"];
                    EntityReadBody::Parameter {
                        owner: EntityId::from_bytes(hex32(facts["owner"].as_str().unwrap())),
                        role: match facts["role"].as_str().unwrap() {
                            "Function" => ParameterRole::Function,
                            "Block" => ParameterRole::Block,
                            role => panic!("{id}: unknown parameter role {role}"),
                        },
                        ordinal: u32::try_from(facts["ordinal"].as_u64().unwrap()).unwrap(),
                    }
                }
                kind => EntityReadBody::Other {
                    kind: u16::try_from(kind).unwrap(),
                },
            };
            bindings.push((raw.entity, raw.object_id));
            views.push(EntityReadObject {
                entity: raw.entity,
                kind: u16::try_from(raw.kind).unwrap(),
                object_id: raw.object_id,
                epoch,
                stored_bytes: buffers.stored[raw.stored_index].as_slice(),
                body,
            });
        }
        (views, bindings)
    }

    /// Workspace, root, epoch, session, and ceilings from the corpus context.
    fn corpus_context(
        inputs: &serde_json::Value,
    ) -> (
        WorkspaceId,
        StateRoot,
        SchemaEpochId,
        SessionId,
        EntityReadCeilings,
    ) {
        let context = &inputs["context"];
        let limits = &inputs["selected_limits"];
        (
            WorkspaceId::from_bytes(hex32(context["workspace"].as_str().unwrap())),
            StateRoot::from_bytes(hex32(context["root"].as_str().unwrap())),
            SchemaEpochId::from_bytes(hex32(context["content_epoch"].as_str().unwrap())),
            SessionId::from_bytes(hex32(context["session"].as_str().unwrap())),
            EntityReadCeilings {
                max_entities: limits["max_entities"].as_u64().unwrap(),
                max_response_bytes: limits["max_response_bytes"].as_u64().unwrap(),
                max_work: limits["max_work"].as_u64().unwrap(),
                budget_before_dispatch: limits["max_work"].as_u64().unwrap(),
            },
        )
    }

    /// Pad case bindings to the corpus-declared synthetic root size so L
    /// matches the recorded work. Filler identities sort after every
    /// authored entity and are never selected: the owner touches selected
    /// indices only.
    fn pad_corpus_bindings(id: &str, bindings: &mut Vec<(EntityId, ObjectId)>) {
        let mut filler = bindings.len();
        while bindings.len() < 256 {
            let mut bytes = [0u8; 32];
            bytes[0] = 0x80;
            bytes[1] = u8::try_from(filler).unwrap();
            bindings.push((EntityId::from_bytes(bytes), ObjectId::from_bytes([0u8; 32])));
            filler += 1;
        }
        assert_eq!(bindings.len(), 256, "{id}: synthetic root size");
    }

    /// Test lookup over aligned optional views, matching the production
    /// caller shape (context passed by value, views borrowed from
    /// test-owned buffers, never re-resolved).
    fn corpus_view_at<'a>(
        ordered: &Vec<Option<EntityReadObject<'a>>>,
        index: usize,
    ) -> Option<EntityReadObject<'a>> {
        ordered.get(index).copied().flatten()
    }

    /// Corpus helper: exactly 32 raw bytes from 64 lowercase hex digits.
    fn hex32(text: &str) -> [u8; 32] {
        assert_eq!(text.len(), 64, "expected 32-byte hex");
        let raw = hex_bytes(text);
        raw.try_into().unwrap()
    }

    /// Corpus helper: raw bytes from even-length lowercase hex.
    fn hex_bytes(text: &str) -> Vec<u8> {
        assert!(text.len().is_multiple_of(2), "hex length must be even");
        let digits = text.as_bytes();
        digits
            .chunks_exact(2)
            .map(|pair| {
                let value = |character: u8| match character {
                    b'0'..=b'9' => u32::from(character - b'0'),
                    b'a'..=b'f' => u32::from(character - b'a') + 10,
                    other => panic!("non-hex digit {other}"),
                };
                u8::try_from(value(pair[0]) * 16 + value(pair[1])).unwrap()
            })
            .collect()
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

    fn manual_object_record(
        entity: EntityId,
        kind: u64,
        object_id: ObjectId,
        stored: &[u8],
    ) -> Vec<u8> {
        let mut inner = Vec::new();
        push_uvar(&mut inner, u64::try_from(stored.len()).unwrap());
        inner.extend_from_slice(stored);
        let mut record = Vec::new();
        push_uvar(&mut record, 4);
        push_field(&mut record, 1, entity.as_bytes());
        let kind_bytes = {
            let mut encoded = Vec::new();
            push_uvar(&mut encoded, kind);
            encoded
        };
        push_field(&mut record, 2, &kind_bytes);
        push_field(&mut record, 3, object_id.as_bytes());
        push_field(&mut record, 4, &inner);
        record
    }

    fn manual_response_body(
        fixture: &Fixture,
        session: SessionId,
        requested: EntityId,
        entries: &[(EntityId, u64, ObjectId, Vec<u8>)],
        work_units: u64,
    ) -> Vec<u8> {
        let mut list_content = Vec::new();
        push_uvar(&mut list_content, u64::try_from(entries.len()).unwrap());
        for (entity, kind, object_id, stored) in entries {
            push_sized(
                &mut list_content,
                &manual_object_record(*entity, *kind, *object_id, stored),
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
        let plan = fixture
            .prepare(EntityReadMethod::Version, &request, &selected)
            .unwrap();
        let work = plan.work_units();
        let outcome = encode_entity_read_response(plan, fixture.session).unwrap();
        let views = fixture.views();
        let object = &views[8];
        let expected = manual_response_body(
            &fixture,
            fixture.session,
            entity(9),
            &[(entity(9), 9, object.object_id, object.stored_bytes.to_vec())],
            work,
        );
        assert_eq!(outcome.body, expected);
        let response = decode_entity_read_response(&expected).unwrap();
        assert_eq!(response.objects[0].stored_bytes, object.stored_bytes);
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
        let plan = fixture
            .prepare(EntityReadMethod::Version, &request, &selected)
            .unwrap();
        let work = plan.work_units();
        let views = fixture.views();
        let object = &views[8];
        let valid = manual_response_body(
            &fixture,
            fixture.session,
            entity(9),
            &[(entity(9), 9, object.object_id, object.stored_bytes.to_vec())],
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
        let views = fixture.views();
        let object = &views[8];
        let valid_inner = {
            let mut inner = Vec::new();
            push_uvar(
                &mut inner,
                u64::try_from(object.stored_bytes.len()).unwrap(),
            );
            inner.extend_from_slice(object.stored_bytes);
            inner
        };
        let overlong_inner = {
            let mut inner = Vec::new();
            push_uvar(
                &mut inner,
                u64::try_from(object.stored_bytes.len()).unwrap() + 1,
            );
            inner.extend_from_slice(object.stored_bytes);
            inner
        };
        let nonminimal_inner = {
            let mut inner = vec![
                0x80 | u8::try_from(object.stored_bytes.len() & 0x7f).unwrap(),
                0x00,
            ];
            inner.extend_from_slice(object.stored_bytes);
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
            push_field(&mut record, 3, object.object_id.as_bytes());
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
        let base = request(1, fixture.root);
        for ceiling in 0..3_u8 {
            let mut defective = base;
            match ceiling {
                0 => defective.max_objects = 0,
                1 => defective.max_response_bytes = 0,
                _ => defective.max_work = 0,
            }
            assert_eq!(
                fixture
                    .prepare(EntityReadMethod::Version, &defective, &ceilings())
                    .unwrap_err(),
                EntityReadError::NotCanonical,
                "zero ceiling {ceiling} accepted"
            );
        }
        let mut competing = base;
        competing.max_objects = 0;
        competing.expected_root = StateRoot::from_bytes([0x77; 32]);
        assert_eq!(
            fixture
                .prepare(EntityReadMethod::Version, &competing, &ceilings())
                .unwrap_err(),
            EntityReadError::NotCanonical
        );
    }

    #[test]
    fn signature_preserves_nonmonotonic_declaration_order() {
        let mut fixture = signature_fixture();
        set_function(&mut fixture, 40, &[43, 41, 42]);
        for (byte, ordinal) in [(43_u8, 0_u32), (41, 1), (42, 2)] {
            set_parameter(&mut fixture, byte, 40, ordinal);
        }
        let request = request(40, fixture.root);
        let ((_, _, _), _, response) =
            fixture.roundtrip(EntityReadMethod::Signature, &request, &ceilings());
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
        assert_eq!(
            fixture
                .prepare(
                    EntityReadMethod::Signature,
                    &request(40, fixture.root),
                    &ceilings()
                )
                .unwrap_err(),
            EntityReadError::InternalInvariant
        );
        let mut fixture = signature_fixture();
        let position = fixture.position_of(42);
        fixture.slots[position].epoch = SchemaEpochId::from_bytes([8; 32]);
        assert_eq!(
            fixture
                .prepare(
                    EntityReadMethod::Signature,
                    &request(40, fixture.root),
                    &ceilings()
                )
                .unwrap_err(),
            EntityReadError::InternalInvariant
        );
    }

    #[test]
    fn oversized_stored_object_refused_before_copy() {
        let mut fixture = assemble(
            &[50],
            &[5],
            &[FixtureBody::Function {
                parameters: vec![entity(60); 1_000_000],
            }],
        );
        fixture.slots[0].stored = vec![0xABu8; 17_000_000];
        assert!(u64::try_from(fixture.slots[0].stored.len()).unwrap() > 16_777_216);
        let mut selected = ceilings();
        selected.max_work = u64::MAX;
        selected.max_response_bytes = u64::MAX;
        selected.budget_before_dispatch = u64::MAX;
        let mut request = request(50, fixture.root);
        request.max_response_bytes = 40_000_000;
        request.max_work = u64::MAX;
        assert_eq!(
            fixture
                .prepare(EntityReadMethod::Version, &request, &selected)
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
    }

    #[test]
    fn oversized_non_function_reports_applicability_before_resources() {
        let mut fixture = assemble(&[50], &[3], &[FixtureBody::Other]);
        fixture.slots[0].stored = vec![0xABu8; 17_000_000];
        assert!(u64::try_from(fixture.slots[0].stored.len()).unwrap() > 16_777_216);
        let mut selected = ceilings();
        selected.max_work = u64::MAX;
        selected.max_response_bytes = u64::MAX;
        selected.budget_before_dispatch = u64::MAX;
        let mut request = request(50, fixture.root);
        request.max_response_bytes = 40_000_000;
        request.max_work = u64::MAX;
        assert_eq!(
            fixture
                .prepare(EntityReadMethod::Signature, &request, &selected)
                .unwrap_err(),
            EntityReadError::ClassNotApplicable
        );
        assert_eq!(
            fixture
                .prepare(EntityReadMethod::Version, &request, &selected)
                .unwrap_err(),
            EntityReadError::BudgetExceeded
        );
    }

    #[test]
    fn response_body_has_exact_final_capacity_and_length() {
        let fixture = signature_fixture();
        let request = request(40, fixture.root);
        let plan = fixture
            .prepare(EntityReadMethod::Signature, &request, &ceilings())
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
            clean.roundtrip(EntityReadMethod::Signature, &target, &selected);
        assert_eq!(clean_scalars.2, 2);

        let mut poisoned = eighteen_kind_fixture();
        set_parameter(&mut poisoned, 9, 99, 7);
        set_function(&mut poisoned, 10, &[90, 91]);
        set_other(&mut poisoned, 11, 8);
        set_function(&mut poisoned, 12, &[]);
        let decoy_position = poisoned.position_of(12);
        poisoned.slots[decoy_position].stored = vec![0xDBu8; 20_000];
        let decoy_len = u64::try_from(poisoned.slots[decoy_position].stored.len()).unwrap();
        assert!(
            decoy_len > 10_000,
            "decoy must dwarf the answer; got {decoy_len}"
        );

        let (scalars, outcome, response) =
            poisoned.roundtrip(EntityReadMethod::Signature, &target, &selected);
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
        let again = poisoned
            .prepare(EntityReadMethod::Signature, &target, &selected)
            .unwrap();
        assert_eq!(
            (again.work_units(), again.body_len(), again.object_count()),
            scalars
        );
        // Sensitivity control: the poison is real; selecting a poisoned
        // Function with a dangling parameter fails loudly.
        assert_eq!(
            poisoned
                .prepare(
                    EntityReadMethod::Signature,
                    &request(10, poisoned.root),
                    &selected
                )
                .unwrap_err(),
            EntityReadError::InternalInvariant
        );
    }
}
