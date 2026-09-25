//! The `BOOTSTRAP_PROFILE_1` admission gate (RW-050 slice 2, §§3–4).
//!
//! The frozen bootstrap profile is a static subset of `EXTENDED_V1`: the
//! exact opcodes, type forms, call discipline, and native imports the
//! self-hosted toolchain closure of REWEAVE §10.2 may depend on. This module
//! judges one entry function plus its transitive call closure against that
//! frozen subset and refuses anything outside it with the shared lowering
//! refusal vocabulary (`LowerErrorCode`), so callers map gate refusals onto
//! the same decisions as lowering refusals without inventing a code.
//!
//! What the gate judges (profile membership only):
//!
//! - every reached operation's opcode is a frozen bootstrap opcode
//!   (`PERMITTED_BOOTSTRAP_OPCODES`); excluded families refuse with
//!   `OpcodeUnsupported`;
//! - every `adapter_invoke` names a carried import — unknown identities
//!   refuse with `OpcodeUnsupported`, exactly as lowering judges them;
//! - every carried `AdapterImport` is referenced by a reached
//!   `adapter_invoke`, carries a globally distinct identity, and is itself
//!   a bootstrap-schema row (frozen fields plus conversion schemas or the
//!   push relationship pin over bootstrap types) — unreferenced,
//!   duplicated, or non-bootstrap rows refuse with `OpcodeUnsupported`, so
//!   no unused import rides an admission (RW-070 closure repair);
//! - every reached function declares no effects, no contracts, and no type
//!   parameters (bootstrap functions are pure closed value functions);
//! - the call graph is acyclic: recursive cycles refuse with
//!   `ProfileUnsupported` (canonical iteration is the CFG backedge loop,
//!   Form A; the 256-frame VM ceiling stays as defense in depth);
//! - every declared type form is a bootstrap type (no floats, no handles,
//!   tokens, function references, or type parameters; cells only inside
//!   executions, never at function boundaries).
//!
//! What the gate deliberately does NOT re-judge (single-authority rule):
//! reference integrity, operand agreement, CFG validity, canonical forms,
//! and resource accounting stay with S20-210/S20-220/lowering/execution.
//! The gate needs callee and import resolution only to walk the closure and
//! to check registry membership — the same codes lowering reports for the
//! same conditions.
//!
//! The gate is a static admission authority, not an execution path:
//! `EXTENDED_V1` lowering and execution are unchanged, and no execution
//! entry calls this gate. Bootstrap-closure programs pass the gate as
//! evidence (every frozen closure vector is gate-admitted before emission);
//! future toolchain admissions route through it.

use std::collections::{BTreeMap, BTreeSet};

use sley_check::TypeEnvironment;
use sley_id::SemanticFingerprint;
use sley_ssmc::{
    AdapterImport, Block, ConstData, ConstantDefinition, FunctionGraph, Immediate, Opcode,
    Operation, Parameter, TypeExpr,
    fingerprint::{FingerprintErrorCode, FunctionFingerprintInput},
};

use super::extended::{BridgeKind, resolve_bridge_entry};
use super::{LowerError, LowerErrorCode, LoweringError};

/// Frozen bootstrap opcodes: E1 data (with the base Boolean operations),
/// E2 checked integers, E4 records/variants/maps, E5 cells and value
/// hashing, E6 direct calls. Excluded: E3 floats (80–85), E7a contract
/// assertions (144), E7 tests/effects/adapters/capabilities (145, 160,
/// 162), E5 globals and function references (193, 194). Opcode 161
/// (`adapter_invoke`) is admitted only through genuine bridge resolution,
/// never by tag membership — it is deliberately absent here.
const PERMITTED_BOOTSTRAP_OPCODES: &[u32] = &[
    1, 16, 17, 18, 19, 20, 21, 32, 33, 34, 35, 36, 37, 38, 39, 40, 64, 65, 66, 67, 68, 69, 70, 71,
    96, 97, 98, 99, 100, 101, 102, 103, 104, 112, 128, 129, 130, 131, 176, 177, 178, 192,
];

/// Bootstrap profile version selecting the gate's bridge allowlist.
///
/// V1 is the frozen `BOOTSTRAP_PROFILE_1` subset (conversions plus push).
/// V2 is the successor `BOOTSTRAP_PROFILE_2` subset, which additionally
/// admits the raw-hash row (`RHW1`) for Sley-owned digest assembly. The
/// version travels from admission request into the admission report, so
/// package approval can pin evidence to authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapProfileVersion {
    /// Frozen v1 subset: no raw-hash row.
    V1,
    /// Successor v2 subset: raw-hash row admitted.
    V2,
}

/// One gate admission request: the entry function plus the root-wide
/// inventories the closure walks, mirroring the lowering input shape.
pub struct BootstrapProfileInput<'a> {
    /// Selected type environment (named type definitions are inspected
    /// through it: a `Named` type admits only when its definition's
    /// fields/payloads admit).
    pub types: &'a TypeEnvironment,
    /// Bootstrap profile version selecting the admitted bridge allowlist:
    /// v1 admits exactly the conversions plus push (`B2V1`, `V2B1`,
    /// `PSH1`); v2 additionally admits the raw-hash row (`RHW1`). The
    /// version is recorded in the admission report, and package approval
    /// pins the report version to the receipt profile — so a v2-judged
    /// report (the only kind that can name `RHW1`) can never approve a
    /// v1 package, and v1 authority stays isolated from successor
    /// imports.
    pub profile_version: BootstrapProfileVersion,
    /// Exact schema epoch the closure is admitted under. Semantic
    /// fingerprints carried in the admission report are epoch-bound, so
    /// the authority and the package builder must agree on this epoch
    /// for their fingerprints to match byte-for-byte.
    pub schema_epoch: sley_id::SchemaEpochId,
    /// Image bytes presented with the closure for binding. The gate
    /// records their SHA-256 digest into the report as a commitment
    /// channel — a structural byte hash with no semantic judgment, no
    /// lowering, and no correspondence verification. Correspondence
    /// between these bytes and the judged graphs (builder faithfulness)
    /// is verified by the admission authority's reference re-lowering
    /// comparison (production: the Sley build driver per the RW-080
    /// contract; RW-075: modeled in tests), never by the host path.
    pub presented_image_bytes: &'a [u8],
    /// Entry function of the bootstrap closure.
    pub entry: &'a FunctionGraph,
    /// Complete function inventory (entry plus callees).
    pub functions: &'a [FunctionGraph],
    /// Complete parameter inventory (function and block parameters).
    pub parameters: &'a [Parameter],
    /// Complete block inventory.
    pub blocks: &'a [Block],
    /// Complete operation inventory.
    pub operations: &'a [Operation],
    /// Complete `AdapterImport` inventory (bridge rows resolve here).
    pub adapters: &'a [AdapterImport],
    /// Complete constant inventory (`constant_ref` names enter here).
    pub constants: &'a [ConstantDefinition],
}

/// Successful gate admission evidence.
///
/// Sealed two ways: `#[non_exhaustive]` prevents downstream struct-literal
/// construction, and private fields prevent downstream mutation of a
/// genuine report. The only way to obtain a report is to run
/// `judge_bootstrap_profile` over the closure's full inventories — which
/// refuses out-of-profile closures instead of reporting them. Reads go
/// through the `functions`, `imports`, `operation_count`,
/// `bridge_uses`, and `profile_version` accessors and work unchanged
/// everywhere.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapProfileReport {
    /// Reached functions in traversal order, entry first.
    functions: Vec<sley_id::EntityId>,
    /// Admitted import identities in inventory order: exactly the invoked
    /// set, so the report names which imports the admission covers.
    imports: Vec<sley_id::EntityId>,
    /// Count of judged operations across the closure.
    operation_count: u32,
    /// Count of `adapter_invoke` operations admitted through the bridge.
    bridge_uses: u32,
    /// Canonical semantic fingerprints of the judged closure, one per
    /// reached function in traversal order (entry first). Computed from
    /// the judged graphs after every membership check passes, so the
    /// report is bound to the exact judged closure bytes — not only to
    /// entry/import/count summaries. Two closures sharing summaries but
    /// differing in any judged operation have different fingerprints
    /// (BLAKE3 second-preimage resistance), and no report exists for a
    /// gate-refused closure.
    closure_fingerprints: Vec<SemanticFingerprint>,
    /// SHA-256 digest of the image bytes presented with the closure.
    admitted_image_digest: [u8; 32],
    /// Profile version this report was judged under. Package approval
    /// requires the report version matching the receipt profile (v1
    /// reports approve v1 packages, v2 reports v2 packages), closing
    /// cross-version report replay: a v2 report naming `RHW1` can never
    /// back a v1 approval.
    profile_version: BootstrapProfileVersion,
}

impl BootstrapProfileReport {
    /// Reached functions in traversal order, entry first.
    #[must_use]
    pub fn functions(&self) -> &[sley_id::EntityId] {
        &self.functions
    }

    /// Admitted import identities in inventory order.
    #[must_use]
    pub fn imports(&self) -> &[sley_id::EntityId] {
        &self.imports
    }

    /// Count of judged operations across the closure.
    #[must_use]
    pub const fn operation_count(&self) -> u32 {
        self.operation_count
    }

    /// Count of admitted `adapter_invoke` operations.
    #[must_use]
    pub const fn bridge_uses(&self) -> u32 {
        self.bridge_uses
    }

    /// Canonical semantic fingerprints of the judged closure, entry first.
    #[must_use]
    pub fn closure_fingerprints(&self) -> &[SemanticFingerprint] {
        &self.closure_fingerprints
    }

    /// SHA-256 digest of the image bytes presented with the closure.
    ///
    /// Commitment channel binding this sealed report to exact executable
    /// bytes: approval and execution re-derive the digest from the
    /// package image and refuse on mismatch, so a report cannot be
    /// replayed against different executable content — including
    /// same-opcode rewiring, which changes bytes and therefore the
    /// digest. Recorded, never verified for graphs correspondence here;
    /// correspondence is the admission authority's reference re-lowering
    /// comparison (RW-080 contract), not host work.
    #[must_use]
    pub fn admitted_image_digest(&self) -> &[u8; 32] {
        &self.admitted_image_digest
    }

    /// Profile version this report was judged under.
    #[must_use]
    pub const fn profile_version(&self) -> BootstrapProfileVersion {
        self.profile_version
    }
}

fn gate_fail(code: LowerErrorCode) -> Result<BootstrapProfileReport, LoweringError> {
    Err(LoweringError::Lower(LowerError::new(code)))
}

/// Whether a declared type form belongs to the frozen bootstrap subset.
/// Cells are execution-local: permitted inside executions (operation result
/// types) but never at function boundaries (parameter and result types) or
/// in constants, which have no cell form to check against. Named types are
/// inspected through their definitions (fields and variant payloads), not
/// just their arguments: a definition hiding an excluded form fails the
/// whole use. `visiting` guards definition cycles (coinductive accept on
/// revisit: a cycle with no excluded form anywhere in it admits).
fn bootstrap_type_ok(
    types: &TypeEnvironment,
    value_type: &TypeExpr,
    at_boundary: bool,
    visiting: &mut BTreeSet<sley_id::EntityId>,
) -> bool {
    match value_type {
        TypeExpr::Unit
        | TypeExpr::Bool
        | TypeExpr::SInt(_)
        | TypeExpr::UInt(_)
        | TypeExpr::Bytes
        | TypeExpr::Text
        | TypeExpr::BuiltinFailure(_) => true,
        // No floats (the toolchain closure is integer/byte/map-indexed;
        // excluding them also keeps host float-environment dependence out
        // of the bootstrap determinism claim), no handles, tokens,
        // function references, or type parameters (no generic
        // specialization in the bootstrap subset).
        TypeExpr::F32
        | TypeExpr::F64
        | TypeExpr::AdapterHandle(_)
        | TypeExpr::CapabilityToken(_)
        | TypeExpr::FunctionRef(_)
        | TypeExpr::TypeParameter(_) => false,
        TypeExpr::LocalCell(inner) => {
            !at_boundary && bootstrap_type_ok(types, inner, false, visiting)
        }
        TypeExpr::Vector(inner) | TypeExpr::Option(inner) => {
            bootstrap_type_ok(types, inner, at_boundary, visiting)
        }
        TypeExpr::Tuple(items) => items
            .iter()
            .all(|item| bootstrap_type_ok(types, item, at_boundary, visiting)),
        TypeExpr::Result { ok, error } => {
            bootstrap_type_ok(types, ok, at_boundary, visiting)
                && bootstrap_type_ok(types, error, at_boundary, visiting)
        }
        TypeExpr::OrderedMap { key, value } => {
            bootstrap_type_ok(types, key, at_boundary, visiting)
                && bootstrap_type_ok(types, value, at_boundary, visiting)
        }
        TypeExpr::Named(named) => {
            if !named
                .arguments
                .iter()
                .all(|argument| bootstrap_type_ok(types, argument, at_boundary, visiting))
            {
                return false;
            }
            // A definition already under inspection admits coinductively;
            // an unresolvable definition fails closed (the gate cannot
            // verify what it cannot see).
            if !visiting.insert(named.definition) {
                return true;
            }
            let admitted =
                types
                    .definition(named.definition)
                    .is_ok_and(|definition| match &definition.form {
                        sley_ssmc::TypeDefForm::Record(fields) => fields.iter().all(|field| {
                            bootstrap_type_ok(types, &field.value_type, at_boundary, visiting)
                        }),
                        sley_ssmc::TypeDefForm::Variant(cases) => cases.iter().all(|case| {
                            case.payload_type.as_ref().is_none_or(|payload| {
                                bootstrap_type_ok(types, payload, at_boundary, visiting)
                            })
                        }),
                    });
            visiting.remove(&named.definition);
            admitted
        }
    }
}

/// Judges one entry function plus its transitive call closure against the
/// frozen bootstrap subset.
///
/// # Errors
///
/// Returns the exact `LowerErrorCode` refusal for the first
/// out-of-profile condition, using the shared lowering vocabulary.
pub fn judge_bootstrap_profile(
    input: &BootstrapProfileInput<'_>,
) -> Result<BootstrapProfileReport, LoweringError> {
    let functions: BTreeMap<sley_id::EntityId, &FunctionGraph> = input
        .functions
        .iter()
        .map(|function| (function.entity_id, function))
        .collect();
    let parameters: BTreeMap<sley_id::EntityId, &Parameter> = input
        .parameters
        .iter()
        .map(|parameter| (parameter.entity_id, parameter))
        .collect();
    // The entry resolves in the supplied inventory, like every callee.
    if !functions.contains_key(&input.entry.entity_id) {
        return gate_fail(LowerErrorCode::ImmediateMismatch);
    }
    let mut reached = Vec::new();
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    walk_function(
        input,
        &functions,
        input.entry.entity_id,
        &mut reached,
        &mut visiting,
        &mut visited,
    )?;
    // The admission covers exactly the reached closure: inventory
    // functions no call reaches are not judged, so they cannot ride an
    // admission — callers narrow inventories first, as lowering does.
    if reached.len() != functions.len() {
        return gate_fail(LowerErrorCode::ProfileUnsupported);
    }
    // The admission covers exactly the reached imports: every carried
    // `AdapterImport` must be referenced by a reached `adapter_invoke`
    // operation and must itself be a bootstrap-schema row, and no two rows
    // share one identity. An unused row — registered or not, shape-valid
    // or not — cannot ride an admission, and neither can a duplicated
    // identity (RW-070 closure repair: the positive import manifest is
    // closed at the inventory level, mirroring the function rule above).
    // Per-use operand binding stays with lowering judgment, which selects
    // among monomorphized push rows by exact schemas.
    let mut referenced = BTreeSet::new();
    for operation in input.operations.iter().filter(|operation| {
        operation.opcode == Opcode::AdapterInvoke
            && input.blocks.iter().any(|block| {
                block.entity_id == operation.block && reached.contains(&block.function)
            })
    }) {
        if let Immediate::Entity(entry) = &operation.immediate {
            referenced.insert(*entry);
        }
    }
    // Import identities are globally distinct (S20-230 §2): two rows sharing
    // one identity make the registry ambiguous, so a duplicated identity
    // refuses even when each row alone is valid. In particular a closed
    // inventory carries at most one push row; a closure needing two element
    // types needs a new admission, not a second row (RW-070 freeze rule).
    let mut seen = BTreeSet::new();
    let mut imports = Vec::new();
    for import in input.adapters {
        if !seen.insert(import.entity_id) {
            return gate_fail(LowerErrorCode::OpcodeUnsupported);
        }
        if !referenced.contains(&import.entity_id) {
            return gate_fail(LowerErrorCode::OpcodeUnsupported);
        }
        if !bootstrap_row_ok(input.types, input.adapters, import, input.profile_version) {
            return gate_fail(LowerErrorCode::OpcodeUnsupported);
        }
        imports.push(import.entity_id);
    }
    // Constant inventory membership: every carried constant's value type
    // faces boundary-strict membership, referenced or not — the admitted
    // inventory is exactly what the closure declares. (Constants have no
    // cell form, so cells refuse here as at every other boundary.)
    for constant in input.constants {
        if !bootstrap_type_ok(
            input.types,
            &constant.value.value_type,
            true,
            &mut BTreeSet::new(),
        ) {
            return gate_fail(LowerErrorCode::SignatureMismatch);
        }
        // Successor preimage bound (AR-02): under v2 a carried `Bytes`
        // payload longer than the raw-hash ceiling refuses at admission
        // with `ResourceLimit`. Every digest the closure can produce
        // through `RHW1` then equals canonical single-shot BLAKE3 over
        // the full preimage — no carried preimage can reach the
        // execution-time refusal, and computed over-bound values keep
        // that typed refusal as backstop. V1 needs no bound (no `RHW1`;
        // `ValueHash` stays canonical at any size under its own
        // encoder/work limits).
        if input.profile_version == BootstrapProfileVersion::V2
            && matches!(&constant.value.data, ConstData::Bytes(bytes) if bytes.len() > crate::raw_hash::RAW_HASH_MAX_BYTES)
        {
            return gate_fail(LowerErrorCode::ResourceLimit);
        }
    }
    // One resolution per function: the walk's map entry is the judged
    // graph (duplicate inventory identities collapse here, exactly once,
    // rather than resolving two ways in two places).
    let mut operation_count = 0_u32;
    let mut bridge_uses = 0_u32;
    for function in &reached {
        let Some(graph) = functions.get(function) else {
            return gate_fail(LowerErrorCode::ImmediateMismatch);
        };
        let (operations, bridges) = judge_reached_function(input, &parameters, function, graph)?;
        operation_count = operation_count.saturating_add(operations);
        bridge_uses = bridge_uses.saturating_add(bridges);
    }
    let closure_fingerprints = judged_closure_fingerprints(input, &reached, input.schema_epoch)?;
    // Commitment digest over the presented image bytes: a structural
    // SHA-256 with no semantic judgment, no lowering, and no
    // correspondence verification. It binds this sealed report to exact
    // executable bytes downstream.
    let admitted_image_digest = {
        use sha2::{Digest as _, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(input.presented_image_bytes);
        hasher.finalize().into()
    };
    Ok(BootstrapProfileReport {
        functions: reached,
        operation_count,
        bridge_uses,
        imports,
        closure_fingerprints,
        admitted_image_digest,
        profile_version: input.profile_version,
    })
}

/// Computes the canonical semantic fingerprint of every reached function
/// in traversal order (entry first).
///
/// Narrowing rule (canonical): each function is fingerprinted over exactly
/// its owned inventories — function parameters owned by the function,
/// block parameters of the function's blocks (identified through the
/// blocks' own parameter lists, since block parameters are owned by their
/// block), blocks whose function is the function, and operations whose
/// block belongs to one of those blocks — in inventory order. The builder
/// of an execution package applies this same rule when it carries the
/// gate's quantitative claims, so authority and builder fingerprints agree
/// byte-for-byte.
///
/// This runs after every membership check passes and performs no new
/// admission judgment of its own: it encodes the already-judged graphs.
/// A fingerprint failure refuses admission fail-closed (`ResourceLimit`
/// for bounds; `LocalReferenceInvalid` for fingerprint-unencodable
/// reference shapes, which the profile gate deliberately leaves to
/// lowering/CFG judgment — the gate refuses to admit what it cannot
/// fingerprint).
///
/// # Errors
///
/// Returns the exact refusal for an unfingerprintable judged closure.
fn judged_closure_fingerprints(
    input: &BootstrapProfileInput<'_>,
    reached: &[sley_id::EntityId],
    schema_epoch: sley_id::SchemaEpochId,
) -> Result<Vec<SemanticFingerprint>, LoweringError> {
    let mut fingerprints = Vec::with_capacity(reached.len());
    for function in reached {
        let blocks: Vec<sley_ssmc::Block> = input
            .blocks
            .iter()
            .filter(|block| &block.function == function)
            .cloned()
            .collect();
        let operations: Vec<sley_ssmc::Operation> = input
            .operations
            .iter()
            .filter(|operation| {
                blocks
                    .iter()
                    .any(|block| block.entity_id == operation.block)
            })
            .cloned()
            .collect();
        let parameters: Vec<sley_ssmc::Parameter> = input
            .parameters
            .iter()
            .filter(|parameter| {
                &parameter.owner == function
                    || blocks.iter().any(|block| {
                        block.entity_id == parameter.owner
                            && block.parameters.contains(&parameter.entity_id)
                    })
            })
            .cloned()
            .collect();
        let Some(graph) = input
            .functions
            .iter()
            .find(|graph| &graph.entity_id == function)
        else {
            return Err(LoweringError::Lower(LowerError::new(
                LowerErrorCode::ImmediateMismatch,
            )));
        };
        let fingerprint = sley_ssmc::fingerprint::fingerprint_function(
            schema_epoch,
            FunctionFingerprintInput {
                function: graph,
                parameters: &parameters,
                blocks: &blocks,
                operations: &operations,
            },
        )
        .map_err(|error| {
            LoweringError::Lower(LowerError::new(
                if error.code() == FingerprintErrorCode::ResourceLimit {
                    LowerErrorCode::ResourceLimit
                } else {
                    LowerErrorCode::LocalReferenceInvalid
                },
            ))
        })?;
        fingerprints.push(fingerprint);
    }
    Ok(fingerprints)
}

/// Depth-first call-closure walk from one function: judges the per-function
/// profile rules on entry to each function and refuses recursive cycles.
/// `visiting` holds the current stack (a revisit is a cycle); `visited`
/// holds completed functions. `visited` is inserted post-order only, so it
/// never holds a stack member — never insert pre-order, or a cycle reads
/// as done (the self-recursion repair this comment guards).
fn walk_function(
    input: &BootstrapProfileInput<'_>,
    functions: &BTreeMap<sley_id::EntityId, &FunctionGraph>,
    function: sley_id::EntityId,
    reached: &mut Vec<sley_id::EntityId>,
    visiting: &mut BTreeSet<sley_id::EntityId>,
    visited: &mut BTreeSet<sley_id::EntityId>,
) -> Result<(), LoweringError> {
    if visited.contains(&function) {
        return Ok(());
    }
    if !visiting.insert(function) {
        // A call-graph cycle: recursion is excluded from the bootstrap
        // subset (canonical iteration is the backedge loop). The VM's
        // 256-frame ceiling stays as defense in depth, never as the
        // admitted mechanism.
        return Err(LoweringError::Lower(LowerError::new(
            LowerErrorCode::ProfileUnsupported,
        )));
    }
    let Some(graph) = functions.get(&function) else {
        return Err(LoweringError::Lower(LowerError::new(
            LowerErrorCode::ImmediateMismatch,
        )));
    };
    // Bootstrap functions are pure closed value functions: no effects, no
    // contracts, no type parameters (no generic specialization, §6).
    if !graph.effects.is_empty() || !graph.contracts.is_empty() || !graph.type_parameters.is_empty()
    {
        return Err(LoweringError::Lower(LowerError::new(
            LowerErrorCode::ProfileUnsupported,
        )));
    }
    // Pre-order: the report lists the entry first, then callees.
    reached.push(function);
    let callees: Vec<sley_id::EntityId> = input
        .operations
        .iter()
        .filter(|operation| {
            input
                .blocks
                .iter()
                .any(|block| block.entity_id == operation.block && block.function == function)
        })
        .filter_map(
            |operation| match (&operation.opcode, &operation.immediate) {
                (Opcode::CallDirect, Immediate::Function(reference)) => {
                    if reference.type_arguments.is_empty() {
                        Some(reference.function)
                    } else {
                        None
                    }
                }
                _ => None,
            },
        )
        .collect();
    // A generic call needs specialization, which the subset excludes.
    let generic = input.operations.iter().any(|operation| {
        matches!(
            (&operation.opcode, &operation.immediate),
            (Opcode::CallDirect, Immediate::Function(reference))
            if !reference.type_arguments.is_empty()
                && input.blocks.iter().any(|block| {
                    block.entity_id == operation.block && block.function == function
                })
        )
    });
    if generic {
        return Err(LoweringError::Lower(LowerError::new(
            LowerErrorCode::ProfileUnsupported,
        )));
    }
    for callee in callees {
        walk_function(input, functions, callee, reached, visiting, visited)?;
    }
    visiting.remove(&function);
    visited.insert(function);
    Ok(())
}

/// Whether one carried import row is a bootstrap-schema bridge row, judged
/// through the one shared resolution authority plus the gate's membership
/// rule: the row must resolve as a landed bridge entry (frozen identity
/// fields, conversion schemas or the push relationship pin — the same
/// checks lowering, execution, and surcharge apply), and its request and
/// response types must themselves be bootstrap types. The relationship pin
/// accepts any structurally related pair at lowering under `EXTENDED_V1`,
/// so the bootstrap membership check here is what keeps float or handle
/// instantiations out of the subset. Callers enforce reference and
/// distinctness separately; with distinct identities the resolved row is
/// always the carried row itself.
fn bootstrap_row_ok(
    types: &TypeEnvironment,
    adapters: &[AdapterImport],
    carried: &AdapterImport,
    profile_version: BootstrapProfileVersion,
) -> bool {
    let Some((kind, resolved)) = resolve_bridge_entry(adapters, &carried.entity_id) else {
        return false;
    };
    if resolved.entity_id != carried.entity_id {
        return false;
    }
    // Version-specific allowlist: the raw-hash row admits only under the
    // successor profile. V1 authority therefore cannot cover a closure
    // naming `RHW1` — no v1-judged report names it, and approval pins
    // the report version to the receipt profile.
    if kind == BridgeKind::RawHash && profile_version != BootstrapProfileVersion::V2 {
        return false;
    }
    bootstrap_type_ok(types, &resolved.request_type, true, &mut BTreeSet::new())
        && bootstrap_type_ok(types, &resolved.response_type, true, &mut BTreeSet::new())
}

/// Admits one `adapter_invoke` operation: its immediate must name a carried
/// import. Opcode 161 is admitted only this way — never by tag membership.
/// Every carried row already faces full registry validation in
/// `judge_bootstrap_profile`, so naming a carried row is the whole per-use
/// check; per-use operand binding stays with lowering judgment.
fn admit_bridge_use(
    input: &BootstrapProfileInput<'_>,
    operation: &Operation,
) -> Result<(), LoweringError> {
    let Immediate::Entity(entry) = &operation.immediate else {
        return Err(LoweringError::Lower(LowerError::new(
            LowerErrorCode::ImmediateMismatch,
        )));
    };
    if input
        .adapters
        .iter()
        .any(|import| import.entity_id == *entry)
    {
        return Ok(());
    }
    Err(LoweringError::Lower(LowerError::new(
        LowerErrorCode::OpcodeUnsupported,
    )))
}

/// Judges one reached function's operations and boundary types. Returns
/// the operation count and the admitted bridge-use count. The graph is the
/// walk's own resolution, so duplicate inventory identities cannot split
/// the judgment between two lookups.
fn judge_reached_function(
    input: &BootstrapProfileInput<'_>,
    parameters: &BTreeMap<sley_id::EntityId, &Parameter>,
    function: &sley_id::EntityId,
    graph: &FunctionGraph,
) -> Result<(u32, u32), LoweringError> {
    let mut operation_count = 0_u32;
    let mut bridge_uses = 0_u32;
    let blocks: Vec<&Block> = input
        .blocks
        .iter()
        .filter(|block| &block.function == function)
        .collect();
    for block in &blocks {
        for parameter_id in &block.parameters {
            let Some(parameter) = parameters.get(parameter_id) else {
                continue;
            };
            // Block parameters thread loop state inside the execution, so
            // cells are permitted here (unlike function boundaries).
            if !bootstrap_type_ok(
                input.types,
                &parameter.value_type,
                false,
                &mut BTreeSet::new(),
            ) {
                return Err(LoweringError::Lower(LowerError::new(
                    LowerErrorCode::SignatureMismatch,
                )));
            }
        }
    }
    for operation in input.operations.iter().filter(|operation| {
        blocks
            .iter()
            .any(|block| block.entity_id == operation.block)
    }) {
        operation_count = operation_count.saturating_add(1);
        if operation.opcode == Opcode::AdapterInvoke {
            admit_bridge_use(input, operation)?;
            bridge_uses = bridge_uses.saturating_add(1);
        } else if PERMITTED_BOOTSTRAP_OPCODES
            .binary_search(&operation.opcode.tag())
            .is_err()
        {
            return Err(LoweringError::Lower(LowerError::new(
                LowerErrorCode::OpcodeUnsupported,
            )));
        }
        // Declared result types face membership in every case, bridge
        // operations included: a resolved import serving a non-bootstrap
        // result is not a profile member.
        for result in &operation.result_types {
            if !bootstrap_type_ok(input.types, result, false, &mut BTreeSet::new()) {
                return Err(LoweringError::Lower(LowerError::new(
                    LowerErrorCode::SignatureMismatch,
                )));
            }
        }
    }
    // Function boundary types admit no cells and no other excluded form.
    // (The graph is the walk's resolution, not a second lookup.)
    for parameter_id in &graph.parameters {
        if let Some(parameter) = parameters.get(parameter_id)
            && !bootstrap_type_ok(
                input.types,
                &parameter.value_type,
                true,
                &mut BTreeSet::new(),
            )
        {
            return Err(LoweringError::Lower(LowerError::new(
                LowerErrorCode::SignatureMismatch,
            )));
        }
    }
    if !bootstrap_type_ok(input.types, &graph.result_type, true, &mut BTreeSet::new()) {
        return Err(LoweringError::Lower(LowerError::new(
            LowerErrorCode::SignatureMismatch,
        )));
    }
    Ok((operation_count, bridge_uses))
}

#[cfg(test)]
mod tests {
    use sley_id::EntityId;
    use sley_ssmc::{
        Block, ConstantDefinition, FunctionGraph, Immediate, IntegerWidth, Opcode, Operation,
        OperationResultRef, Parameter, ParameterRole, Reachability, ReturnTerminator, Terminator,
        TypeExpr, ValueRef, Visibility,
    };

    use super::*;
    use crate::extended::{bridge_entry_id, bridge_test_imports};

    fn id(byte: u8) -> EntityId {
        EntityId::from_bytes([byte; 32])
    }

    fn u8_type() -> TypeExpr {
        TypeExpr::UInt(IntegerWidth::from_bits(8))
    }

    struct Program {
        types: TypeEnvironment,
        entry: FunctionGraph,
        functions: Vec<FunctionGraph>,
        parameters: Vec<Parameter>,
        blocks: Vec<Block>,
        operations: Vec<Operation>,
        adapters: Vec<AdapterImport>,
        constants: Vec<ConstantDefinition>,
    }

    impl Program {
        fn input(&self) -> BootstrapProfileInput<'_> {
            BootstrapProfileInput {
                types: &self.types,
                schema_epoch: sley_id::SchemaEpochId::from_bytes([8; 32]),
                entry: &self.entry,
                presented_image_bytes: &[],
                functions: &self.functions,
                parameters: &self.parameters,
                blocks: &self.blocks,
                operations: &self.operations,
                adapters: &self.adapters,
                constants: &self.constants,
                // In-crate profile tests judge the frozen v1 subset;
                // successor coverage passes V2 explicitly per case.
                profile_version: BootstrapProfileVersion::V1,
            }
        }
    }

    /// One single-block function running one operation over two unit
    /// parameters and returning the first parameter.
    fn single_op(
        opcode: Opcode,
        immediate: Immediate,
        scope_type: TypeExpr,
        request_type: TypeExpr,
        result_type: TypeExpr,
    ) -> Program {
        let function = id(1);
        let block = id(2);
        let scope_param = id(10);
        let request_param = id(11);
        let operation = id(100);
        Program {
            entry: FunctionGraph {
                entity_id: function,
                type_parameters: Vec::new(),
                parameters: vec![scope_param, request_param],
                result_type: scope_type.clone(),
                effects: Vec::new(),
                entry_block: block,
                blocks: vec![block],
                contracts: Vec::new(),
                visibility: Visibility::Private,
            },
            functions: vec![FunctionGraph {
                entity_id: function,
                type_parameters: Vec::new(),
                parameters: vec![scope_param, request_param],
                result_type: scope_type.clone(),
                effects: Vec::new(),
                entry_block: block,
                blocks: vec![block],
                contracts: Vec::new(),
                visibility: Visibility::Private,
            }],
            parameters: vec![
                Parameter {
                    entity_id: scope_param,
                    owner: function,
                    role: ParameterRole::Function,
                    ordinal: 0,
                    value_type: scope_type,
                },
                Parameter {
                    entity_id: request_param,
                    owner: function,
                    role: ParameterRole::Function,
                    ordinal: 1,
                    value_type: request_type,
                },
            ],
            blocks: vec![Block {
                entity_id: block,
                function,
                parameters: Vec::new(),
                operations: vec![operation],
                terminator: Terminator::Return(ReturnTerminator {
                    value: ValueRef::Parameter(scope_param),
                }),
                reachability: Reachability::Required,
            }],
            operations: vec![Operation {
                entity_id: operation,
                block,
                ordinal: 0,
                opcode,
                operands: vec![
                    ValueRef::Parameter(scope_param),
                    ValueRef::Parameter(request_param),
                ],
                result_types: vec![result_type],
                immediate,
            }],
            adapters: Vec::new(),
            constants: Vec::new(),
            types: TypeEnvironment::new(Vec::new()).unwrap(),
        }
    }

    fn gate_code(program: &Program) -> LowerErrorCode {
        match judge_bootstrap_profile(&program.input()) {
            Ok(_) => panic!("gate admitted an out-of-profile program"),
            Err(LoweringError::Lower(error)) => error.code(),
            Err(LoweringError::Cfg(failure)) => panic!("no CFG judgment in the gate: {failure}"),
        }
    }

    #[test]
    fn bootstrap_gate_admits_every_permitted_family() {
        // E1 data: checked by opcode tag alone (semantic agreement stays
        // with lowering); one representative per family group suffices for
        // the membership table, with the full tag list pinned below.
        let data = single_op(
            Opcode::VectorLen,
            Immediate::None,
            TypeExpr::Vector(Box::new(TypeExpr::Bool)),
            TypeExpr::Unit,
            TypeExpr::UInt(IntegerWidth::from_bits(64)),
        );
        let report = judge_bootstrap_profile(&data.input()).expect("E1 data admits");
        assert_eq!(report.functions, vec![id(1)]);
        assert_eq!(report.operation_count, 1);
        assert_eq!(report.bridge_uses, 0);
        // E2 checked integers.
        let ints = single_op(
            Opcode::IntAddChecked,
            Immediate::None,
            TypeExpr::SInt(IntegerWidth::from_bits(32)),
            TypeExpr::SInt(IntegerWidth::from_bits(32)),
            TypeExpr::Result {
                ok: Box::new(TypeExpr::SInt(IntegerWidth::from_bits(32))),
                error: Box::new(TypeExpr::BuiltinFailure(
                    sley_ssmc::BuiltinFailureKind::Arithmetic,
                )),
            },
        );
        judge_bootstrap_profile(&ints.input()).expect("E2 admits");
        // E4 maps.
        let maps = single_op(
            Opcode::MapContains,
            Immediate::None,
            TypeExpr::OrderedMap {
                key: Box::new(TypeExpr::UInt(IntegerWidth::from_bits(64))),
                value: Box::new(TypeExpr::Text),
            },
            TypeExpr::UInt(IntegerWidth::from_bits(64)),
            TypeExpr::Bool,
        );
        judge_bootstrap_profile(&maps.input()).expect("E4 admits");
        // E5 cells and hashing.
        let cells = single_op(
            Opcode::CellNew,
            Immediate::None,
            TypeExpr::Bool,
            TypeExpr::Unit,
            TypeExpr::LocalCell(Box::new(TypeExpr::Bool)),
        );
        judge_bootstrap_profile(&cells.input()).expect("E5 cells admit");
        let hashing = single_op(
            Opcode::ValueHash,
            Immediate::None,
            TypeExpr::Text,
            TypeExpr::Unit,
            TypeExpr::Bytes,
        );
        judge_bootstrap_profile(&hashing.input()).expect("E5 hashing admits");
        // E8 bridge through genuine resolution.
        let mut bridge = single_op(
            Opcode::AdapterInvoke,
            Immediate::Entity(bridge_entry_id(*b"B2V1")),
            TypeExpr::Unit,
            TypeExpr::Bytes,
            TypeExpr::Result {
                ok: Box::new(TypeExpr::Vector(Box::new(u8_type()))),
                error: Box::new(TypeExpr::BuiltinFailure(
                    sley_ssmc::BuiltinFailureKind::Index,
                )),
            },
        );
        bridge.adapters = vec![bridge_test_imports()[0].clone()];
        let report = judge_bootstrap_profile(&bridge.input()).expect("E8 bridge admits");
        assert_eq!(report.bridge_uses, 1);
    }

    #[test]
    fn bootstrap_gate_permits_exactly_the_frozen_opcode_table() {
        // Every tag the frozen table names admits; spot-check the table
        // edges (first, last, and family boundaries).
        for tag in [
            1, 17, 21, 32, 40, 64, 71, 96, 101, 102, 104, 112, 128, 131, 176, 192,
        ] {
            assert!(
                PERMITTED_BOOTSTRAP_OPCODES.binary_search(&tag).is_ok(),
                "frozen tag {tag} missing from the gate table"
            );
        }
        assert_eq!(
            PERMITTED_BOOTSTRAP_OPCODES.len(),
            42,
            "the frozen table holds 42 opcodes"
        );
        assert!(
            PERMITTED_BOOTSTRAP_OPCODES
                .windows(2)
                .all(|pair| pair[0] < pair[1]),
            "the frozen table stays sorted for binary search"
        );
        // Every excluded opcode refuses with the opcode code: E3 floats,
        // E7a assertions, E7 tests/effects/capabilities, E5 globals and
        // function references.
        for opcode in [
            Opcode::FloatAdd,
            Opcode::FloatSub,
            Opcode::FloatMul,
            Opcode::FloatDiv,
            Opcode::FloatNeg,
            Opcode::FloatFma,
            Opcode::ContractAssert,
            Opcode::TestObserve,
            Opcode::EffectRequest,
            Opcode::CapabilityNarrow,
            Opcode::GlobalGet,
            Opcode::FunctionRef,
        ] {
            let program = single_op(
                opcode,
                Immediate::None,
                TypeExpr::Unit,
                TypeExpr::Unit,
                TypeExpr::Unit,
            );
            assert_eq!(
                gate_code(&program),
                LowerErrorCode::OpcodeUnsupported,
                "excluded opcode {opcode:?} must refuse"
            );
        }
    }

    #[test]
    fn bootstrap_gate_refuses_excluded_type_forms() {
        // Floats, handles, tokens, function references, and type
        // parameters refuse wherever they are declared.
        for forbidden in [
            TypeExpr::F32,
            TypeExpr::F64,
            TypeExpr::AdapterHandle(id(9)),
            TypeExpr::CapabilityToken(id(9)),
            TypeExpr::FunctionRef(sley_ssmc::FunctionType {
                parameters: Vec::new(),
                result: Box::new(TypeExpr::Unit),
                effects: Vec::new(),
            }),
            TypeExpr::TypeParameter(0),
            TypeExpr::Vector(Box::new(TypeExpr::F64)),
            TypeExpr::Result {
                ok: Box::new(TypeExpr::Unit),
                error: Box::new(TypeExpr::F32),
            },
        ] {
            let program = single_op(
                Opcode::BoolNot,
                Immediate::None,
                forbidden.clone(),
                TypeExpr::Unit,
                TypeExpr::Bool,
            );
            assert_eq!(
                gate_code(&program),
                LowerErrorCode::SignatureMismatch,
                "excluded type {forbidden:?} must refuse"
            );
        }
        // Cells are execution-local: permitted in operation results, never
        // at function boundaries.
        let cell_result = single_op(
            Opcode::CellNew,
            Immediate::None,
            TypeExpr::Bool,
            TypeExpr::Unit,
            TypeExpr::LocalCell(Box::new(TypeExpr::Bool)),
        );
        judge_bootstrap_profile(&cell_result.input()).expect("cell results admit");
        let cell_param = single_op(
            Opcode::CellGet,
            Immediate::None,
            TypeExpr::LocalCell(Box::new(TypeExpr::Bool)),
            TypeExpr::Unit,
            TypeExpr::Bool,
        );
        assert_eq!(
            gate_code(&cell_param),
            LowerErrorCode::SignatureMismatch,
            "cell parameter types must refuse"
        );
    }

    #[test]
    fn bootstrap_gate_requires_pure_closed_functions_and_acyclic_calls() {
        // Effectful, contracted, and generic functions refuse.
        let mut effectful = single_op(
            Opcode::BoolNot,
            Immediate::None,
            TypeExpr::Bool,
            TypeExpr::Unit,
            TypeExpr::Bool,
        );
        effectful.entry.effects = vec![id(20)];
        effectful.functions[0].effects = vec![id(20)];
        assert_eq!(
            gate_code(&effectful),
            LowerErrorCode::ProfileUnsupported,
            "effectful functions must refuse"
        );
        let mut contracted = single_op(
            Opcode::BoolNot,
            Immediate::None,
            TypeExpr::Bool,
            TypeExpr::Unit,
            TypeExpr::Bool,
        );
        contracted.entry.contracts = vec![id(21)];
        contracted.functions[0].contracts = vec![id(21)];
        assert_eq!(
            gate_code(&contracted),
            LowerErrorCode::ProfileUnsupported,
            "contracted functions must refuse"
        );
        let mut generic = single_op(
            Opcode::BoolNot,
            Immediate::None,
            TypeExpr::Bool,
            TypeExpr::Unit,
            TypeExpr::Bool,
        );
        generic.entry.type_parameters = vec![sley_ssmc::TypeParameterDef { ordinal: 0 }];
        generic.functions[0].type_parameters = vec![sley_ssmc::TypeParameterDef { ordinal: 0 }];
        assert_eq!(
            gate_code(&generic),
            LowerErrorCode::ProfileUnsupported,
            "generic functions must refuse"
        );
        // A two-function acyclic call admits, entry first.
        let acyclic = call_program(false);
        let report = judge_bootstrap_profile(&acyclic.input()).expect("acyclic calls admit");
        assert_eq!(report.functions.len(), 2);
        assert_eq!(report.functions[0], id(1));
        assert_eq!(report.operation_count, 2);
        // A self-recursive call refuses.
        assert_eq!(
            gate_code(&call_program(true)),
            LowerErrorCode::ProfileUnsupported,
            "recursive cycles must refuse"
        );
        // An unknown callee refuses with the immediate code.
        let mut unknown = call_program(false);
        let Operation { immediate, .. } = &mut unknown.operations[0];
        *immediate = Immediate::Function(sley_ssmc::FunctionRefValue {
            function: id(77),
            type_arguments: Vec::new(),
        });
        assert_eq!(
            gate_code(&unknown),
            LowerErrorCode::ImmediateMismatch,
            "unknown callees must refuse"
        );
    }

    /// Caller (id 1) calling a helper (id 6) with one Bool operand;
    /// `recursive` turns the helper's body into a self-call.
    fn call_program(recursive: bool) -> Program {
        let main = id(1);
        let sub = id(6);
        let main_block = id(2);
        let operand = id(10);
        let call = id(100);
        let (sub_graph, sub_block_value, sub_operation) = sub_side(recursive);
        let entry = FunctionGraph {
            entity_id: main,
            type_parameters: Vec::new(),
            parameters: vec![operand],
            result_type: TypeExpr::Bool,
            effects: Vec::new(),
            entry_block: main_block,
            blocks: vec![main_block],
            contracts: Vec::new(),
            visibility: Visibility::Private,
        };
        Program {
            entry: entry.clone(),
            functions: vec![entry, sub_graph],
            parameters: vec![
                Parameter {
                    entity_id: operand,
                    owner: main,
                    role: ParameterRole::Function,
                    ordinal: 0,
                    value_type: TypeExpr::Bool,
                },
                Parameter {
                    entity_id: operand,
                    owner: sub,
                    role: ParameterRole::Function,
                    ordinal: 0,
                    value_type: TypeExpr::Bool,
                },
            ],
            blocks: vec![
                Block {
                    entity_id: main_block,
                    function: main,
                    parameters: Vec::new(),
                    operations: vec![call],
                    terminator: Terminator::Return(ReturnTerminator {
                        value: ValueRef::OperationResult(OperationResultRef {
                            operation: call,
                            result_index: 0,
                        }),
                    }),
                    reachability: Reachability::Required,
                },
                sub_block_value,
            ],
            operations: vec![
                Operation {
                    entity_id: call,
                    block: main_block,
                    ordinal: 0,
                    opcode: Opcode::CallDirect,
                    operands: vec![ValueRef::Parameter(operand)],
                    result_types: vec![TypeExpr::Bool],
                    immediate: Immediate::Function(sley_ssmc::FunctionRefValue {
                        function: sub,
                        type_arguments: Vec::new(),
                    }),
                },
                sub_operation,
            ],
            adapters: Vec::new(),
            constants: Vec::new(),
            types: TypeEnvironment::new(Vec::new()).unwrap(),
        }
    }

    /// The helper side of `call_program`: graph, block, and operation with
    /// entity ids recomputed from the same constants, so no id table
    /// crosses the split.
    fn sub_side(recursive: bool) -> (FunctionGraph, Block, Operation) {
        let sub = id(6);
        let sub_block = id(7);
        let operand = id(10);
        let answer = id(101);
        let graph = FunctionGraph {
            entity_id: sub,
            type_parameters: Vec::new(),
            parameters: vec![operand],
            result_type: TypeExpr::Bool,
            effects: Vec::new(),
            entry_block: sub_block,
            blocks: vec![sub_block],
            contracts: Vec::new(),
            visibility: Visibility::Private,
        };
        let block = Block {
            entity_id: sub_block,
            function: sub,
            parameters: Vec::new(),
            operations: vec![answer],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: answer,
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        };
        let operation = if recursive {
            Operation {
                entity_id: answer,
                block: sub_block,
                ordinal: 0,
                opcode: Opcode::CallDirect,
                operands: vec![ValueRef::Parameter(operand)],
                result_types: vec![TypeExpr::Bool],
                immediate: Immediate::Function(sley_ssmc::FunctionRefValue {
                    function: sub,
                    type_arguments: Vec::new(),
                }),
            }
        } else {
            Operation {
                entity_id: answer,
                block: sub_block,
                ordinal: 0,
                opcode: Opcode::BoolNot,
                operands: vec![ValueRef::Parameter(operand)],
                result_types: vec![TypeExpr::Bool],
                immediate: Immediate::None,
            }
        };
        (graph, block, operation)
    }

    /// Inventory functions no call reaches are not judged, so they
    /// cannot ride an admission — even clean ones. Callers narrow
    /// inventories first, as lowering does.
    #[test]
    fn bootstrap_gate_refuses_unreached_inventory() {
        let mut program = call_program(false);
        // A clean unreached function still refuses: the admission covers
        // exactly the reached closure.
        let idle = id(50);
        program.functions.push(FunctionGraph {
            entity_id: idle,
            type_parameters: Vec::new(),
            parameters: Vec::new(),
            result_type: TypeExpr::Unit,
            effects: Vec::new(),
            entry_block: id(51),
            blocks: Vec::new(),
            contracts: Vec::new(),
            visibility: Visibility::Private,
        });
        assert_eq!(
            gate_code(&program),
            LowerErrorCode::ProfileUnsupported,
            "unreached inventory must refuse"
        );
    }

    #[test]
    fn bootstrap_gate_refuses_mutual_recursion() {
        // A calls B calls A: the visiting-stack check fires on the second
        // arrival at A, the same mechanism as self-recursion.
        let first = id(1);
        let second = id(6);
        let first_block = id(2);
        let second_block = id(7);
        let operand = id(10);
        let first_call = id(100);
        let second_call = id(101);
        let call = |entity: EntityId, block: EntityId, target: EntityId| Operation {
            entity_id: entity,
            block,
            ordinal: 0,
            opcode: Opcode::CallDirect,
            operands: vec![ValueRef::Parameter(operand)],
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::Function(sley_ssmc::FunctionRefValue {
                function: target,
                type_arguments: Vec::new(),
            }),
        };
        let graph = |entity: EntityId, block: EntityId| FunctionGraph {
            entity_id: entity,
            type_parameters: Vec::new(),
            parameters: vec![operand],
            result_type: TypeExpr::Bool,
            effects: Vec::new(),
            entry_block: block,
            blocks: vec![block],
            contracts: Vec::new(),
            visibility: Visibility::Private,
        };
        let block = |entity: EntityId, owner: EntityId, operation: EntityId| Block {
            entity_id: entity,
            function: owner,
            parameters: Vec::new(),
            operations: vec![operation],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation,
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        };
        let entry = graph(first, first_block);
        let program = Program {
            entry: entry.clone(),
            functions: vec![entry, graph(second, second_block)],
            parameters: vec![Parameter {
                entity_id: operand,
                owner: first,
                role: ParameterRole::Function,
                ordinal: 0,
                value_type: TypeExpr::Bool,
            }],
            blocks: vec![
                block(first_block, first, first_call),
                block(second_block, second, second_call),
            ],
            operations: vec![
                call(first_call, first_block, second),
                call(second_call, second_block, first),
            ],
            adapters: Vec::new(),
            constants: Vec::new(),
            types: TypeEnvironment::new(Vec::new()).unwrap(),
        };
        assert_eq!(
            gate_code(&program),
            LowerErrorCode::ProfileUnsupported,
            "mutual recursion must refuse"
        );
    }

    /// A record definition hiding an excluded form fails the whole use:
    /// named types are inspected through their definitions, not just
    /// their arguments (Ariadne slice-2 review).
    #[test]
    fn bootstrap_gate_inspects_named_definitions() {
        let tainted = id(60);
        let clean = id(61);
        let definitions = vec![
            sley_ssmc::TypeDefinition {
                entity_id: tainted,
                type_parameters: Vec::new(),
                form: sley_ssmc::TypeDefForm::Record(vec![sley_ssmc::RecordField {
                    member_id: sley_ssmc::MemberId::from_bytes([0xA1; 32]),
                    value_type: TypeExpr::F64,
                    visibility: Visibility::Private,
                }]),
                invariants: Vec::new(),
                visibility: Visibility::Private,
            },
            sley_ssmc::TypeDefinition {
                entity_id: clean,
                type_parameters: Vec::new(),
                form: sley_ssmc::TypeDefForm::Variant(vec![
                    sley_ssmc::VariantCase {
                        member_id: sley_ssmc::MemberId::from_bytes([0xC1; 32]),
                        payload_type: Some(TypeExpr::Bool),
                    },
                    sley_ssmc::VariantCase {
                        member_id: sley_ssmc::MemberId::from_bytes([0xC2; 32]),
                        payload_type: None,
                    },
                ]),
                invariants: Vec::new(),
                visibility: Visibility::Private,
            },
        ];
        let named = |definition| {
            TypeExpr::Named(sley_ssmc::NamedType {
                definition,
                arguments: Vec::new(),
            })
        };
        // The tainted record refuses as a parameter type even though the
        // `Named` form itself names no float.
        let mut program = single_op(
            Opcode::BoolNot,
            Immediate::None,
            named(tainted),
            TypeExpr::Unit,
            TypeExpr::Bool,
        );
        program.types = TypeEnvironment::new(definitions).unwrap();
        assert_eq!(
            gate_code(&program),
            LowerErrorCode::SignatureMismatch,
            "named definitions hiding floats must refuse"
        );
        // The clean variant admits: definition inspection is a walk, not a
        // blanket refusal of named types.
        let mut admits = single_op(
            Opcode::BoolNot,
            Immediate::None,
            named(clean),
            TypeExpr::Unit,
            TypeExpr::Bool,
        );
        admits.types =
            TypeEnvironment::new(vec![program.types.definition(clean).unwrap().clone()]).unwrap();
        judge_bootstrap_profile(&admits.input()).expect("clean named types admit");
        // An unresolvable definition fails closed: the gate cannot verify
        // what it cannot see.
        let mut unknown = single_op(
            Opcode::BoolNot,
            Immediate::None,
            named(id(62)),
            TypeExpr::Unit,
            TypeExpr::Bool,
        );
        unknown.types = TypeEnvironment::new(Vec::new()).unwrap();
        assert_eq!(
            gate_code(&unknown),
            LowerErrorCode::SignatureMismatch,
            "unresolvable definitions must refuse"
        );
    }

    /// Cells admit only with bootstrap element types: `LocalCell<F64>`
    /// is out at any position (Ariadne slice-2 review).
    #[test]
    fn bootstrap_gate_checks_cell_element_types() {
        let program = single_op(
            Opcode::CellNew,
            Immediate::None,
            TypeExpr::Bool,
            TypeExpr::Unit,
            TypeExpr::LocalCell(Box::new(TypeExpr::F64)),
        );
        assert_eq!(
            gate_code(&program),
            LowerErrorCode::SignatureMismatch,
            "cells over excluded types must refuse"
        );
    }

    /// Constant inventory membership: an unreferenced constant carrying
    /// an excluded type still refuses — the admitted inventory is exactly
    /// what the closure declares (Nabu slice-2 review, C3).
    #[test]
    fn bootstrap_gate_checks_constant_value_types() {
        let mut program = single_op(
            Opcode::BoolNot,
            Immediate::None,
            TypeExpr::Bool,
            TypeExpr::Unit,
            TypeExpr::Bool,
        );
        program.constants = vec![sley_ssmc::ConstantDefinition {
            entity_id: id(200),
            value: sley_ssmc::ConstValue {
                value_type: TypeExpr::F64,
                data: sley_ssmc::ConstData::F64Bits(0),
            },
        }];
        assert_eq!(
            gate_code(&program),
            LowerErrorCode::SignatureMismatch,
            "constants over excluded types must refuse"
        );
    }

    /// A relationship-holding push row over a non-bootstrap element type
    /// resolves under `EXTENDED_V1` but is not a permitted bootstrap
    /// import; a frozen row serving a non-bootstrap result is not a
    /// profile member either (Ariadne slice-2 review).
    #[test]
    fn bootstrap_gate_checks_bridge_row_and_result_types() {
        use crate::extended::{BridgeEntry, bridge_entry_id, bridge_import};
        let float_push = bridge_import(
            BridgeEntry::VectorPush,
            TypeExpr::F64,
            TypeExpr::Vector(Box::new(TypeExpr::F64)),
        );
        let mut program = single_op(
            Opcode::AdapterInvoke,
            Immediate::Entity(bridge_entry_id(*b"PSH1")),
            TypeExpr::Vector(Box::new(TypeExpr::F64)),
            TypeExpr::F64,
            TypeExpr::Result {
                ok: Box::new(TypeExpr::Vector(Box::new(TypeExpr::F64))),
                error: Box::new(TypeExpr::BuiltinFailure(
                    sley_ssmc::BuiltinFailureKind::Index,
                )),
            },
        );
        program.adapters = vec![float_push];
        assert_eq!(
            gate_code(&program),
            LowerErrorCode::OpcodeUnsupported,
            "non-bootstrap row schemas are not permitted imports"
        );
        // Frozen row, correct operands, non-bootstrap declared result.
        let mut mistyped = single_op(
            Opcode::AdapterInvoke,
            Immediate::Entity(bridge_entry_id(*b"B2V1")),
            TypeExpr::Unit,
            TypeExpr::Bytes,
            TypeExpr::Result {
                ok: Box::new(TypeExpr::Vector(Box::new(TypeExpr::F64))),
                error: Box::new(TypeExpr::BuiltinFailure(
                    sley_ssmc::BuiltinFailureKind::Index,
                )),
            },
        );
        mistyped.adapters = vec![bridge_test_imports()[0].clone()];
        assert_eq!(
            gate_code(&mistyped),
            LowerErrorCode::SignatureMismatch,
            "non-bootstrap bridge results must refuse"
        );
    }

    #[test]
    fn bootstrap_gate_admits_only_registered_bridge_rows() {
        let mut frozen = single_op(
            Opcode::AdapterInvoke,
            Immediate::Entity(bridge_entry_id(*b"V2B1")),
            TypeExpr::Unit,
            TypeExpr::Vector(Box::new(u8_type())),
            TypeExpr::Result {
                ok: Box::new(TypeExpr::Bytes),
                error: Box::new(TypeExpr::BuiltinFailure(
                    sley_ssmc::BuiltinFailureKind::Index,
                )),
            },
        );
        frozen.adapters = vec![bridge_test_imports()[1].clone()];
        judge_bootstrap_profile(&frozen.input()).expect("frozen rows admit");
        // Unknown identity, tampered ABI, and effectful rows refuse with
        // the opcode code — the same genuine resolution lowering uses.
        let mut unknown = single_op(
            Opcode::AdapterInvoke,
            Immediate::Entity(id(77)),
            TypeExpr::Unit,
            TypeExpr::Bytes,
            TypeExpr::Result {
                ok: Box::new(TypeExpr::Vector(Box::new(u8_type()))),
                error: Box::new(TypeExpr::BuiltinFailure(
                    sley_ssmc::BuiltinFailureKind::Index,
                )),
            },
        );
        unknown.adapters = Vec::new();
        assert_eq!(
            gate_code(&unknown),
            LowerErrorCode::OpcodeUnsupported,
            "unknown identities must refuse"
        );
        let mut tampered = frozen;
        tampered.adapters[0].abi_version = 2;
        assert_eq!(
            gate_code(&tampered),
            LowerErrorCode::OpcodeUnsupported,
            "tampered rows must refuse"
        );
    }

    /// Unreferenced import rows cannot ride an admission, however
    /// shape-valid and registered they are (RW-070 closure repair: the
    /// positive manifest is closed at the inventory level, mirroring the
    /// unreached-function rule).
    #[test]
    fn bootstrap_gate_refuses_unreferenced_import_rows() {
        let mut program = single_op(
            Opcode::AdapterInvoke,
            Immediate::Entity(bridge_entry_id(*b"B2V1")),
            TypeExpr::Unit,
            TypeExpr::Bytes,
            TypeExpr::Result {
                ok: Box::new(TypeExpr::Vector(Box::new(u8_type()))),
                error: Box::new(TypeExpr::BuiltinFailure(
                    sley_ssmc::BuiltinFailureKind::Index,
                )),
            },
        );
        // The invoked row alone admits.
        program.adapters = vec![bridge_test_imports()[0].clone()];
        judge_bootstrap_profile(&program.input()).expect("exact inventory admits");
        // A registered but uninvoked row refuses.
        program.adapters = bridge_test_imports().to_vec();
        assert_eq!(
            gate_code(&program),
            LowerErrorCode::OpcodeUnsupported,
            "unreferenced rows must refuse"
        );
        // An effectful uninvoked row refuses the same way.
        let mut effectful = bridge_test_imports()[2].clone();
        effectful.effects = vec![id(20)];
        program.adapters = vec![bridge_test_imports()[0].clone(), effectful];
        assert_eq!(
            gate_code(&program),
            LowerErrorCode::OpcodeUnsupported,
            "unreferenced effectful rows must refuse"
        );
    }

    /// Two rows sharing one identity refuse even when each row alone is
    /// valid: import identities are globally distinct (S20-230 §2), so a
    /// closed inventory carries at most one push row.
    #[test]
    fn bootstrap_gate_refuses_duplicated_import_identities() {
        let mut program = single_op(
            Opcode::AdapterInvoke,
            Immediate::Entity(bridge_entry_id(*b"B2V1")),
            TypeExpr::Unit,
            TypeExpr::Bytes,
            TypeExpr::Result {
                ok: Box::new(TypeExpr::Vector(Box::new(u8_type()))),
                error: Box::new(TypeExpr::BuiltinFailure(
                    sley_ssmc::BuiltinFailureKind::Index,
                )),
            },
        );
        program.adapters = vec![
            bridge_test_imports()[0].clone(),
            bridge_test_imports()[0].clone(),
        ];
        assert_eq!(
            gate_code(&program),
            LowerErrorCode::OpcodeUnsupported,
            "duplicated identities must refuse"
        );
    }
}
