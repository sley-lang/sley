//! AF1: name-based JSON authoring frames.
//!
//! An AF1 frame is JSON data, not source syntax (ADR-0051). It names
//! entities by local name, derives every mechanical field (ordinals,
//! owner lists, parameter roles, reachability, empty sets, operation result
//! types), and compiles client-side, in one pass, into the existing mutation
//! candidate record. Created identities are derived under the record nonce
//! during compilation, so the author never handles a placeholder.
//!
//! Definitions are matched by name against the accepted state: a function,
//! block, parameter, or operation whose name already exists keeps its
//! identity and is replaced only when its body changes; old entities a new
//! definition no longer names are deleted. The kernel never sees AF1.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sley_id::{CandidateNonce, EntityId};
use sley_mutate::MutationPayload;
use sley_mutate::value::{
    BlockBody, ConstantBody, EntityBodyValue, EntityIdSet, FunctionBody, NamespaceBody,
    OperationBody, ParameterBody, TestCaseBody, TypeDefBody,
};
use sley_ssmc::{
    BranchTerminator, BuiltinCase, BuiltinFailureKind, CaseKey, CondBranchTerminator, ConstData,
    ConstValue, EffectEnvironment, ExpectedOutcome, FunctionRefValue, FunctionType, Immediate,
    IntegerWidth, MemberId, NamedType, OperationResultRef, ParameterRole, Reachability,
    RecordField, ResourceLimits, ReturnTerminator, SwitchArgument, SwitchCase, SwitchEdge,
    TargetEdge, Terminator, TrapCode, TrapTerminator, TypeDefForm, TypeExpr, ValueRef, VariantCase,
    VariantImmediate, VariantSwitchTerminator, Visibility,
};

use crate::candidate::PlannedOp;
use crate::error::{AgentError, AgentErrorCode, Result, frame};
use crate::names::{NAME_GRAMMAR, NameMap, Names, Scope, is_identifier};
use crate::opcodes::{self, ImmediateKind, OpcodeRow};
use crate::types::{self, TypeNames};
use crate::values::{self, TypeDefs};
use crate::workspace::Program;

/// Workspace `TestCase` defaults before clamping to the policy grant.
pub const DEFAULT_TEST_LIMITS: ResourceLimits = ResourceLimits {
    fuel: 1_000_000,
    memory_bytes: 16_777_216,
    output_bytes: 65_536,
    effect_count: 0,
    call_depth: 256,
    wall_timeout_millis: 10_000,
};

/// The limits an AF1 test takes when it states none: the defaults, each
/// clamped to the principal's grant.
#[must_use]
pub fn default_test_limits(ceilings: &sley_policy::PolicyResourceCeilings) -> ResourceLimits {
    ResourceLimits {
        fuel: DEFAULT_TEST_LIMITS.fuel.min(ceilings.max_fuel),
        memory_bytes: DEFAULT_TEST_LIMITS
            .memory_bytes
            .min(ceilings.max_memory_bytes),
        output_bytes: DEFAULT_TEST_LIMITS
            .output_bytes
            .min(ceilings.max_output_bytes),
        effect_count: DEFAULT_TEST_LIMITS.effect_count,
        call_depth: DEFAULT_TEST_LIMITS.call_depth,
        wall_timeout_millis: DEFAULT_TEST_LIMITS.wall_timeout_millis,
    }
}

/// The limit keys a `TestCase` may declare (`sley-agent help tests`).
pub const LIMIT_KEYS: &str =
    "fuel, memory_bytes, output_bytes, effect_count, call_depth, wall_timeout_millis";

/// The compiled frame.
#[derive(Clone, Debug, Default)]
pub struct Compiled {
    /// Planned operations: creates (in derivation order), replaces, deletes.
    pub ops: Vec<PlannedOp>,
    /// Names of created entities and members.
    pub names: NameMap,
    /// Created entity count.
    pub created: usize,
    /// Replaced entity count.
    pub replaced: usize,
    /// Deleted entity count.
    pub deleted: usize,
    /// Functions the frame defines or changes.
    pub functions: Vec<EntityId>,
    /// `TestCases` the frame declares.
    pub tests: Vec<EntityId>,
    /// Notes for the author (auto-deleted dependents, ...).
    pub notes: Vec<String>,
    /// Derived authoring artifacts kept with a draft revision, by file name
    /// (the expanded frame and its source map for an AF1-X frame).
    pub artifacts: Vec<(String, Value)>,
    /// Authoring statistics for the events ledger (feature use counts).
    pub stats: serde_json::Map<String, Value>,
}

/// Compiles an AF1 frame against the accepted program.
///
/// `random` supplies fresh member identities (SSMC1 section 4: member ids
/// are creator-supplied nonces, never derived from a name).
///
/// # Errors
///
/// `AGENT_FRAME_INVALID` with a JSON-pointer locator for a malformed frame
/// or an unresolvable name.
pub fn compile(
    program: &Program,
    names: &Names,
    ceilings: &sley_policy::PolicyResourceCeilings,
    frame_value: &Value,
    nonce: CandidateNonce,
    random: &mut dyn FnMut() -> Result<[u8; 32]>,
) -> Result<Compiled> {
    compile_inner(program, names, ceilings, frame_value, nonce, random, None)
}

pub(crate) mod constants;
pub(crate) mod replay;
pub(crate) mod signatures;

#[allow(clippy::too_many_arguments)]
fn compile_inner(
    program: &Program,
    names: &Names,
    ceilings: &sley_policy::PolicyResourceCeilings,
    frame_value: &Value,
    nonce: CandidateNonce,
    random: &mut dyn FnMut() -> Result<[u8; 32]>,
    identities: Option<replay::Identities<'_>>,
) -> Result<Compiled> {
    let object = frame_value
        .as_object()
        .ok_or_else(|| frame("", "an AF1 frame is a JSON object"))?;
    match object.get("af1") {
        Some(Value::Number(version)) if version.as_u64() == Some(1) => {}
        _ => return Err(frame("/af1", "declare \"af1\": 1")),
    }
    if let Some(afx) = object.get("afx") {
        if identities.is_some() {
            return Err(frame("/afx", "identity replay requires expanded AF1"));
        }
        if afx.as_u64() != Some(1) || !afx.is_number() {
            return Err(frame(
                "/afx",
                "declare \"afx\": 1 for the authoring dialect, or remove the key",
            ));
        }
        return compile_extended(program, names, ceilings, frame_value, nonce, random);
    }
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "af1"
                | "types"
                | "consts"
                | "fns"
                | "functions"
                | "patch"
                | "edit"
                | "tests"
                | "delete"
                | "namespace"
                | "comment"
        ) {
            if matches!(key.as_str(), "ripple" | "test_tables") {
                return Err(frame(
                    &format!("/{key}"),
                    format!(
                        "`{key}` belongs to the authoring dialect: add \"afx\": 1 to the frame (the AF1-X envelope) to use it"
                    ),
                ));
            }
            return Err(frame(&format!("/{key}"), "unknown frame key"));
        }
    }
    let mut compiler = Compiler {
        program,
        names,
        nonce,
        ordinal: 0,
        top: BTreeMap::new(),
        creates: Vec::new(),
        bodies: BTreeMap::new(),
        created_top: Vec::new(),
        deletes: BTreeSet::new(),
        new_names: NameMap::default(),
        forms: BTreeMap::new(),
        members: BTreeMap::new(),
        signatures: BTreeMap::new(),
        constants: BTreeMap::new(),
        notes: Vec::new(),
        functions: Vec::new(),
        tests: Vec::new(),
        ceilings: *ceilings,
        identities,
    };
    compiler.run(object, random)?;
    compiler.finish()
}

/// An AF1-X frame: expanded into plain AF1, compiled by the unchanged
/// path, and every compiler problem pointer mapped back to the authored
/// frame. The expansion and its source map become artifacts.
fn compile_extended(
    program: &Program,
    names: &Names,
    ceilings: &sley_policy::PolicyResourceCeilings,
    frame_value: &Value,
    nonce: CandidateNonce,
    random: &mut dyn FnMut() -> Result<[u8; 32]>,
) -> Result<Compiled> {
    let expansion = crate::afx::expand(program, names, frame_value)?;
    if !expansion.obligations.is_empty() {
        // The rest of the frame is compiled too, so one refusal lists its
        // problems beside the open decisions.
        let rest = if expansion.discloses_the_rest() {
            compile(program, names, ceilings, &expansion.frame, nonce, random)
                .err()
                .map(|error| expansion.map.rewrite(&error))
        } else {
            None
        };
        return Err(crate::afx::refusal_beside(
            &expansion.obligations,
            rest.as_ref(),
        ));
    }
    let mut compiled = compile(program, names, ceilings, &expansion.frame, nonce, random)
        .map_err(|error| expansion.map.rewrite(&error))?;
    compiled
        .artifacts
        .push(("expanded.json".to_owned(), expansion.frame));
    compiled
        .artifacts
        .push(("sourcemap.json".to_owned(), expansion.map.to_json()));
    if let Some(inventory) = expansion.ripple {
        compiled
            .artifacts
            .push(("ripple.json".to_owned(), inventory));
    }
    compiled.stats = expansion.stats.to_json();
    Ok(compiled)
}

/// The authoring statistics of an AF1-X frame's expansion, for the events
/// ledger of a frame refused before its statistics were compiled; `None`
/// for plain AF1 and for a frame that does not expand.
#[must_use]
pub fn expansion_stats(
    program: &Program,
    names: &Names,
    frame_value: &Value,
) -> Option<serde_json::Map<String, Value>> {
    frame_value.get("afx")?;
    crate::afx::expand(program, names, frame_value)
        .ok()
        .map(|expansion| expansion.stats.to_json())
}

#[derive(Clone, Copy, Debug)]
struct TopEntry {
    id: EntityId,
    kind: u16,
}

/// One value visible inside a function: identity, owning block (None for a
/// function parameter), and type once known.
#[derive(Clone, Debug)]
struct ValueSlot {
    reference: ValueRef,
    ty: Option<TypeExpr>,
}

struct Compiler<'a> {
    program: &'a Program,
    names: &'a Names,
    nonce: CandidateNonce,
    ordinal: u64,
    top: BTreeMap<String, TopEntry>,
    creates: Vec<(EntityId, u16)>,
    bodies: BTreeMap<EntityId, EntityBodyValue>,
    created_top: Vec<(EntityId, u16)>,
    deletes: BTreeSet<EntityId>,
    new_names: NameMap,
    forms: BTreeMap<EntityId, TypeDefForm>,
    members: BTreeMap<EntityId, Vec<(String, MemberId)>>,
    signatures: BTreeMap<EntityId, (Vec<TypeExpr>, TypeExpr)>,
    constants: BTreeMap<EntityId, ConstValue>,
    notes: Vec<String>,
    functions: Vec<EntityId>,
    tests: Vec<EntityId>,
    ceilings: sley_policy::PolicyResourceCeilings,
    identities: Option<replay::Identities<'a>>,
}

fn array<'v>(value: &'v Value, pointer: &str) -> Result<&'v Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| frame(pointer, "expected an array"))
}

fn string<'v>(value: &'v Value, pointer: &str) -> Result<&'v str> {
    value
        .as_str()
        .ok_or_else(|| frame(pointer, "expected a string"))
}

/// The target of a `br`: `["br", "b", arg...]`, or `["br", ["b", arg...]]`,
/// the bracketed form `cond` and `switch` also accept.
fn branch_target(items: &[Value]) -> Value {
    match items {
        [_, target @ Value::Array(_)] => target.clone(),
        _ => Value::Array(items[1..].to_vec()),
    }
}

/// An operand names a value; anything else is refused with the fix.
fn operand<'v>(value: &'v Value, pointer: &str) -> Result<&'v str> {
    value.as_str().ok_or_else(|| {
        let fix = match value {
            Value::Number(number) => format!(
                "the literal {number} is not a value name; add an operation such as [\"k\", \"const\", {number}] and use \"k\""
            ),
            Value::Array(_) => "operations do not nest; add the operation to the block's ops under a name and use that name".to_owned(),
            _ => "an operand names a value (a parameter or an operation result)".to_owned(),
        };
        frame(pointer, fix)
    })
}

fn name_of<'v>(
    object: &'v serde_json::Map<String, Value>,
    keys: &[&str],
    pointer: &str,
) -> Result<&'v str> {
    for key in keys {
        if let Some(value) = object.get(*key) {
            let name = string(value, &format!("{pointer}/{key}"))?;
            if !is_identifier(name) {
                return Err(frame(
                    &format!("{pointer}/{key}"),
                    format!("`{name}` is not a name ({NAME_GRAMMAR})"),
                ));
            }
            return Ok(name);
        }
    }
    Err(frame(pointer, format!("missing \"{}\"", keys[0])))
}

impl TypeNames for Compiler<'_> {
    fn type_definition(&self, name: &str) -> Option<EntityId> {
        if let Some(entry) = self.top.get(name) {
            return (entry.kind == 4).then_some(entry.id);
        }
        let id = self.names.resolve(name)?;
        matches!(self.program.body(&id), Some(EntityBodyValue::TypeDef(_))).then_some(id)
    }
}

impl TypeDefs for Compiler<'_> {
    fn form(&self, definition: &EntityId) -> Option<TypeDefForm> {
        self.forms
            .get(definition)
            .cloned()
            .or_else(|| match self.program.body(definition) {
                Some(EntityBodyValue::TypeDef(typedef)) => Some(typedef.form.clone()),
                _ => None,
            })
    }

    fn member(&self, definition: &EntityId, leaf: &str) -> Option<MemberId> {
        match self.members.get(definition) {
            Some(members) => members
                .iter()
                .find(|(name, _)| name == leaf)
                .map(|(_, id)| *id),
            None => self.names.resolve_member_leaf(definition, leaf),
        }
    }

    fn member_leaf(&self, definition: &EntityId, member: &MemberId) -> String {
        match self.members.get(definition) {
            Some(members) => members
                .iter()
                .find(|(_, id)| id == member)
                .map_or_else(|| "?".to_owned(), |(name, _)| name.clone()),
            None => self.names.member_leaf(definition, member),
        }
    }

    fn render(&self, ty: &TypeExpr) -> String {
        types::render(ty, self.names)
    }
}

impl Compiler<'_> {
    fn allocate(&mut self, kind: u16, leaf: &str, scope: Scope) -> Result<EntityId> {
        let id = if let Some(identities) = &self.identities {
            let id = identities.entity(kind, leaf, scope)?;
            if self.program.contains(&id) {
                return Err(frame("", "replay creation aliases an accepted identity"));
            }
            id
        } else {
            EntityId::derive(
                self.program.workspace(),
                self.nonce,
                u32::from(kind),
                self.ordinal,
            )
        };
        Ok(self.record_allocation(id, kind, leaf))
    }

    fn record_allocation(&mut self, id: EntityId, kind: u16, leaf: &str) -> EntityId {
        self.ordinal += 1;
        self.creates.push((id, kind));
        self.new_names.insert(*id.as_bytes(), leaf);
        id
    }

    fn is_new(&self, id: &EntityId) -> bool {
        !self.program.contains(id)
    }

    fn put(&mut self, id: EntityId, body: EntityBodyValue) {
        self.bodies.insert(id, body);
    }

    /// The live top-level entity of that name and kind, unless this frame
    /// deletes it: a frame that deletes and redefines a name creates a new
    /// entity rather than reviving the deleted one.
    fn existing_top(&self, name: &str, kind: u16) -> Option<EntityId> {
        let id = self.names.resolve(name)?;
        (self.names.scope(&id) == Scope::Top
            && !self.deletes.contains(&id)
            && self
                .program
                .body(&id)
                .is_some_and(|body| body.kind_tag() == kind))
        .then_some(id)
    }

    /// Declares a top-level name: reuse the live entity of that name and
    /// kind, or allocate a new one.
    fn declare(&mut self, name: &str, kind: u16, pointer: &str) -> Result<EntityId> {
        if self.top.contains_key(name) {
            return Err(frame(pointer, format!("`{name}` is declared twice")));
        }
        let id = if let Some(id) = self.existing_top(name, kind) {
            id
        } else {
            if let Some(other) = self.names.resolve(name)
                && self.names.scope(&other) == Scope::Top
                && !self.deletes.contains(&other)
            {
                return Err(frame(
                    pointer,
                    format!("`{name}` already names a different kind of entity"),
                ));
            }
            let id = self.allocate(kind, name, Scope::Top)?;
            self.created_top.push((id, kind));
            id
        };
        self.top.insert(name.to_owned(), TopEntry { id, kind });
        Ok(id)
    }

    fn resolve_top(&self, name: &str, kind: u16, pointer: &str) -> Result<EntityId> {
        if let Some(entry) = self.top.get(name) {
            if entry.kind == kind {
                return Ok(entry.id);
            }
        } else if let Some(id) = self.existing_top(name, kind)
            && !self.deletes.contains(&id)
        {
            return Ok(id);
        }
        Err(frame(
            pointer,
            format!("no {} named `{name}`", crate::names::kind_name(kind)),
        ))
    }

    fn read_type(&self, value: &Value, pointer: &str) -> Result<TypeExpr> {
        types::read(value, self, pointer)
    }

    fn signature(&self, function: &EntityId) -> Option<(Vec<TypeExpr>, TypeExpr)> {
        if let Some(signature) = self.signatures.get(function) {
            return Some(signature.clone());
        }
        let Some(EntityBodyValue::Function(body)) = self.program.body(function) else {
            return None;
        };
        let params = body
            .parameters
            .iter()
            .map(|param| match self.program.body(param) {
                Some(EntityBodyValue::Parameter(p)) => p.value_type.clone(),
                _ => TypeExpr::Unit,
            })
            .collect();
        Some((params, body.result_type.clone()))
    }

    fn constant_value(&self, id: &EntityId) -> Option<ConstValue> {
        self.constants
            .get(id)
            .cloned()
            .or_else(|| match self.program.body(id) {
                Some(EntityBodyValue::Constant(constant)) => Some(constant.value.clone()),
                _ => None,
            })
    }

    #[allow(clippy::too_many_lines)]
    fn run(
        &mut self,
        object: &serde_json::Map<String, Value>,
        random: &mut dyn FnMut() -> Result<[u8; 32]>,
    ) -> Result<()> {
        let empty = Vec::new();
        let list = |key: &str| -> Result<&Vec<Value>> {
            object
                .get(key)
                .map_or(Ok(&empty), |value| array(value, &format!("/{key}")))
        };
        let types_list = list("types")?;
        let consts_list = list("consts")?;
        let mut fns_list: Vec<(String, &Value)> = Vec::new();
        for key in ["fns", "functions"] {
            for (index, value) in list(key)?.iter().enumerate() {
                fns_list.push((format!("/{key}/{index}"), value));
            }
        }
        let patch_list = list("patch")?;
        let edit_list = list("edit")?;
        let tests_list = list("tests")?;
        let delete_list = list("delete")?;

        // Deletions first, so a frame can delete and redefine.
        for (index, value) in delete_list.iter().enumerate() {
            let pointer = format!("/delete/{index}");
            let name = string(value, &pointer)?;
            let id = self
                .names
                .resolve(name)
                .filter(|id| self.names.scope(id) == Scope::Top)
                .ok_or_else(|| frame(&pointer, format!("no top-level entity named `{name}`")))?;
            self.delete_tree(&id);
        }

        // Allocate every top-level name before compiling bodies, so bodies
        // may reference each other in any order.
        for (index, value) in types_list.iter().enumerate() {
            let pointer = format!("/types/{index}");
            let decl = value
                .as_object()
                .ok_or_else(|| frame(&pointer, "expected an object"))?;
            let name = name_of(decl, &["name"], &pointer)?;
            self.declare(name, 4, &pointer)?;
        }
        for (index, value) in consts_list.iter().enumerate() {
            let pointer = format!("/consts/{index}");
            let decl = value
                .as_object()
                .ok_or_else(|| frame(&pointer, "expected an object"))?;
            let name = name_of(decl, &["name"], &pointer)?;
            self.declare(name, 9, &pointer)?;
        }
        for (pointer, value) in &fns_list {
            let decl = value
                .as_object()
                .ok_or_else(|| frame(pointer, "expected an object"))?;
            let name = name_of(decl, &["fn", "name"], pointer)?;
            self.declare(name, 5, pointer)?;
        }
        // Types: member identities, then forms (fields may name each other).
        let mut type_decls = Vec::new();
        for (index, value) in types_list.iter().enumerate() {
            let pointer = format!("/types/{index}");
            let decl = value.as_object().expect("checked");
            let name = name_of(decl, &["name"], &pointer)?;
            let id = self.top[name].id;
            let (record, cases) = match (decl.get("variant"), decl.get("record")) {
                (Some(cases), None) => (false, array(cases, &format!("{pointer}/variant"))?),
                (None, Some(fields)) => (true, array(fields, &format!("{pointer}/record"))?),
                _ => {
                    return Err(frame(
                        &pointer,
                        "a type has exactly one of \"variant\" or \"record\"",
                    ));
                }
            };
            let mut members = Vec::new();
            for (position, case) in cases.iter().enumerate() {
                let case_pointer = format!(
                    "{pointer}/{}/{position}",
                    if record { "record" } else { "variant" }
                );
                let (leaf, payload) = match case {
                    Value::String(leaf) => (leaf.as_str(), None),
                    Value::Array(pair) if pair.len() == 2 => {
                        (string(&pair[0], &case_pointer)?, Some(&pair[1]))
                    }
                    _ => {
                        return Err(frame(
                            &case_pointer,
                            "expected \"Name\" or [\"Name\", type]",
                        ));
                    }
                };
                if record && payload.is_none() {
                    return Err(frame(&case_pointer, "a record field is [\"name\", type]"));
                }
                if !is_identifier(leaf)
                    || members.iter().any(
                        |(name, _, _, _): &(String, MemberId, Option<&Value>, String)| name == leaf,
                    )
                {
                    return Err(frame(
                        &case_pointer,
                        format!("`{leaf}` is not a fresh member name"),
                    ));
                }
                let member = match self.names.resolve_member_leaf(&id, leaf) {
                    Some(member) if !self.is_new(&id) => member,
                    _ => {
                        let member = if let Some(identities) = &self.identities {
                            identities.member(id, leaf)?
                        } else {
                            MemberId::from_bytes(random()?)
                        };
                        self.new_names.insert(*member.as_bytes(), leaf);
                        member
                    }
                };
                members.push((leaf.to_owned(), member, payload, case_pointer));
            }
            self.members.insert(
                id,
                members
                    .iter()
                    .map(|(leaf, member, _, _)| (leaf.clone(), *member))
                    .collect(),
            );
            type_decls.push((
                id,
                record,
                members,
                decl.get("visibility").cloned(),
                pointer,
            ));
        }
        for (id, record, members, visibility, pointer) in type_decls {
            // A restated live type keeps what AF1 cannot state or did not
            // restate: its visibility and its fields' (when omitted), its
            // type parameters and its invariants.
            let old = match self.program.body(&id) {
                Some(EntityBodyValue::TypeDef(old)) if !self.is_new(&id) => Some(old.clone()),
                _ => None,
            };
            let field_visibility = |member: &MemberId| {
                old.as_ref()
                    .and_then(|old| match &old.form {
                        TypeDefForm::Record(fields) => fields
                            .iter()
                            .find(|field| field.member_id == *member)
                            .map(|field| field.visibility),
                        TypeDefForm::Variant(_) => None,
                    })
                    .unwrap_or(Visibility::Exported)
            };
            let form = if record {
                TypeDefForm::Record(
                    members
                        .iter()
                        .map(|(_, member, payload, case_pointer)| {
                            Ok(RecordField {
                                member_id: *member,
                                value_type: self
                                    .read_type(payload.expect("checked"), case_pointer)?,
                                visibility: field_visibility(member),
                            })
                        })
                        .collect::<Result<_>>()?,
                )
            } else {
                TypeDefForm::Variant(
                    members
                        .iter()
                        .map(|(_, member, payload, case_pointer)| {
                            Ok(VariantCase {
                                member_id: *member,
                                payload_type: match payload {
                                    Some(Value::Null) | None => None,
                                    Some(ty) => Some(self.read_type(ty, case_pointer)?),
                                },
                            })
                        })
                        .collect::<Result<_>>()?,
                )
            };
            self.forms.insert(id, form.clone());
            let visibility = match (&visibility, &old) {
                (None, Some(old)) => old.visibility,
                _ => read_visibility(visibility.as_ref(), &pointer)?,
            };
            self.put(
                id,
                EntityBodyValue::TypeDef(TypeDefBody {
                    type_parameters: old
                        .as_ref()
                        .map_or_else(Vec::new, |old| old.type_parameters.clone()),
                    form,
                    invariants: old
                        .as_ref()
                        .map_or_else(|| set(Vec::new()), |old| old.invariants.clone()),
                    visibility,
                }),
            );
        }
        // Constants.
        for (index, value) in consts_list.iter().enumerate() {
            let pointer = format!("/consts/{index}");
            let decl = value.as_object().expect("checked");
            let name = name_of(decl, &["name"], &pointer)?;
            let id = self.top[name].id;
            let ty = self.read_type(
                decl.get("type").unwrap_or(&Value::from("i64")),
                &format!("{pointer}/type"),
            )?;
            let data = decl
                .get("value")
                .ok_or_else(|| frame(&pointer, "missing \"value\""))?;
            let value = values::read(data, &ty, self, &format!("{pointer}/value"))
                .map_err(|error| frame(&pointer, error.detail()))?;
            self.constants.insert(id, value.clone());
            self.put(id, EntityBodyValue::Constant(ConstantBody { value }));
        }
        // Function signatures (full definitions), before any body is built.
        let mut full = Vec::new();
        for (pointer, value) in &fns_list {
            let decl = value.as_object().expect("checked");
            let name = name_of(decl, &["fn", "name"], pointer)?;
            let id = self.top[name].id;
            let params = self.read_params(decl.get("params"), &format!("{pointer}/params"))?;
            let result =
                signatures::read_result(decl, pointer, &mut |value, at| self.read_type(value, at))?;
            self.signatures.insert(
                id,
                (
                    params.iter().map(|(_, ty)| ty.clone()).collect(),
                    result.clone(),
                ),
            );
            full.push((id, name.to_owned(), decl, params, result, pointer.clone()));
        }
        // Patched signatures (when a patch restates them).
        let mut patches = Vec::new();
        for (index, value) in patch_list.iter().enumerate() {
            let pointer = format!("/patch/{index}");
            let decl = value
                .as_object()
                .ok_or_else(|| frame(&pointer, "expected an object"))?;
            let name = name_of(decl, &["fn", "name"], &pointer)?;
            let id = self.resolve_top(name, 5, &format!("{pointer}/fn"))?;
            if self.is_new(&id) {
                return Err(frame(
                    &pointer,
                    format!(
                        "patch `{name}` names a function this frame creates; define it in \"fns\""
                    ),
                ));
            }
            let (old_params, old_result) = self.signature(&id).expect("live function");
            let params = match decl.get("params") {
                Some(value) => self.read_params(Some(value), &format!("{pointer}/params"))?,
                None => Vec::new(),
            };
            let result = match decl.get("returns") {
                Some(value) => self.read_type(value, &format!("{pointer}/returns"))?,
                None => old_result,
            };
            let param_types = if decl.contains_key("params") {
                params.iter().map(|(_, ty)| ty.clone()).collect()
            } else {
                old_params
            };
            self.signatures.insert(id, (param_types, result.clone()));
            patches.push((id, name.to_owned(), decl, params, result, pointer));
        }
        for (index, value) in edit_list.iter().enumerate() {
            let pointer = format!("/edit/{index}");
            let decl = value
                .as_object()
                .ok_or_else(|| frame(&pointer, "expected an object"))?;
            let name = name_of(decl, &["fn", "function", "name"], &pointer)?;
            let id = self.resolve_top(name, 5, &pointer)?;
            if !self.signatures.contains_key(&id) {
                let signature = self
                    .signature(&id)
                    .ok_or_else(|| frame(&pointer, "not a live function"))?;
                self.signatures.insert(id, signature);
            }
        }
        // Tests are allocated before function bodies so names resolve.
        let mut test_decls = Vec::new();
        for (index, value) in tests_list.iter().enumerate() {
            let pointer = format!("/tests/{index}");
            let decl = value
                .as_object()
                .ok_or_else(|| frame(&pointer, "expected an object"))?;
            let target_name = name_of(decl, &["fn", "target", "function"], &pointer)?;
            let key = ["fn", "target", "function"]
                .into_iter()
                .find(|key| decl.contains_key(*key))
                .unwrap_or("fn");
            let target = self.resolve_top(target_name, 5, &format!("{pointer}/{key}"))?;
            let name = match decl.get("name") {
                Some(_) => name_of(decl, &["name"], &pointer)?.to_owned(),
                None => self.fresh_test_name(target_name),
            };
            let id = self.declare(&name, 14, &pointer)?;
            test_decls.push((id, target, decl, pointer));
        }

        // Bodies. Every function, edit and test is compiled even after one
        // fails, so one refusal names every problem the frame has.
        let mut problems = Vec::new();
        for (id, name, decl, params, result, pointer) in full {
            let specs = decl
                .get("blocks")
                .ok_or_else(|| frame(&pointer, "missing \"blocks\""))
                .and_then(|blocks| array(blocks, &format!("{pointer}/blocks")))
                .and_then(|blocks| {
                    blocks
                        .iter()
                        .enumerate()
                        .map(|(index, block)| {
                            BlockSpec::from_frame(block, &format!("{pointer}/blocks/{index}"))
                        })
                        .collect::<Result<Vec<_>>>()
                });
            let defined = specs.and_then(|specs| {
                self.define_function(id, &name, Some(params), result, specs, decl, &pointer)
            });
            if let Err(error) = defined {
                problems.push(error);
            }
        }
        for (id, name, decl, params, result, pointer) in patches {
            let params = decl.contains_key("params").then_some(params);
            let defined = self.patch_specs(&id, decl, &pointer).and_then(|specs| {
                self.define_function(id, &name, params, result, specs, decl, &pointer)
            });
            if let Err(error) = defined {
                problems.push(error);
            }
        }
        if let Err(error) = self.edits(edit_list) {
            problems.push(error);
        }
        for (id, target, decl, pointer) in test_decls {
            if let Err(error) = self.test(id, target, decl, &pointer) {
                problems.push(error);
            }
        }
        if !problems.is_empty() {
            return Err(combined(problems));
        }
        self.namespaces(object.get("namespace"))?;
        Ok(())
    }

    fn fresh_test_name(&self, target: &str) -> String {
        self.fresh_name(&format!("t_{target}"), true)
    }

    /// The first unused top-level name `base_1`, `base_2`, ... (or `base`
    /// itself first when `numbered` is false).
    fn fresh_name(&self, base: &str, numbered: bool) -> String {
        let free =
            |name: &String| !self.top.contains_key(name) && self.names.resolve(name).is_none();
        if !numbered && free(&base.to_owned()) {
            return base.to_owned();
        }
        (1..=u32::MAX)
            .map(|n| format!("{base}_{n}"))
            .find(free)
            .unwrap_or_else(|| base.to_owned())
    }

    fn read_params(&self, value: Option<&Value>, pointer: &str) -> Result<Vec<(String, TypeExpr)>> {
        signatures::read_params(value, pointer, &mut || Ok(()), &mut |value, at| {
            self.read_type(value, at)
        })
        .map_err(signatures::ParameterError::into_error)
    }

    /// Deletes a top-level entity with everything it owns, `TestCases` that
    /// target it, and its namespace memberships (namespaces are rewritten
    /// in `namespaces`).
    fn delete_tree(&mut self, id: &EntityId) {
        self.deletes.insert(*id);
        if let Some(EntityBodyValue::Function(function)) = self.program.body(id) {
            for param in &function.parameters {
                self.deletes.insert(*param);
            }
            for block in &function.blocks {
                self.delete_block(block);
            }
            let dependents: Vec<EntityId> = self
                .program
                .objects()
                .iter()
                .filter_map(|object| match &object.record().body {
                    EntityBodyValue::TestCase(test) if test.target == *id => {
                        Some(object.record().entity_id)
                    }
                    _ => None,
                })
                .collect();
            for test in dependents {
                if self.deletes.insert(test) {
                    self.notes.push(format!(
                        "deleted TestCase {} with its target",
                        self.names.name(&test)
                    ));
                }
            }
        }
    }

    fn delete_block(&mut self, block: &EntityId) {
        self.deletes.insert(*block);
        if let Some(EntityBodyValue::Block(body)) = self.program.body(block) {
            for param in &body.parameters {
                self.deletes.insert(*param);
            }
            for operation in &body.operations {
                self.deletes.insert(*operation);
            }
        }
    }

    fn patch_specs(
        &self,
        id: &EntityId,
        decl: &serde_json::Map<String, Value>,
        pointer: &str,
    ) -> Result<Vec<BlockSpec>> {
        let Some(EntityBodyValue::Function(function)) = self.program.body(id) else {
            return Err(frame(pointer, "not a live function"));
        };
        let patched = match decl.get("blocks") {
            Some(Value::Object(blocks)) => blocks.clone(),
            None => serde_json::Map::new(),
            Some(_) => {
                return Err(frame(
                    &format!("{pointer}/blocks"),
                    "patch blocks are an object keyed by block name",
                ));
            }
        };
        let mut specs = Vec::new();
        let mut seen = BTreeSet::new();
        for block in &function.blocks {
            let leaf = self.names.leaf(block);
            seen.insert(leaf.clone());
            match patched.get(&leaf) {
                Some(Value::Null) => {}
                Some(spec) => specs.push(BlockSpec::from_patch(
                    &leaf,
                    spec,
                    &format!("{pointer}/blocks/{leaf}"),
                )?),
                None => specs.push(BlockSpec::Keep(*block)),
            }
        }
        for (leaf, spec) in &patched {
            if seen.contains(leaf) {
                continue;
            }
            if spec.is_null() {
                return Err(frame(
                    &format!("{pointer}/blocks/{leaf}"),
                    "no such block to delete",
                ));
            }
            specs.push(BlockSpec::from_patch(
                leaf,
                spec,
                &format!("{pointer}/blocks/{leaf}"),
            )?);
        }
        Ok(specs)
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn define_function(
        &mut self,
        id: EntityId,
        name: &str,
        params: Option<Vec<(String, TypeExpr)>>,
        result: TypeExpr,
        specs: Vec<BlockSpec>,
        decl: &serde_json::Map<String, Value>,
        pointer: &str,
    ) -> Result<()> {
        if self.functions.contains(&id) {
            return Err(frame(
                pointer,
                format!(
                    "`{name}` is restated more than once in this frame; \"fns\", \"patch\" and \"edit\" each restate a function, so change it in one of them"
                ),
            ));
        }
        self.functions.push(id);
        let old = match self.program.body(&id) {
            Some(EntityBodyValue::Function(function)) if !self.is_new(&id) => {
                Some(function.clone())
            }
            _ => None,
        };
        let child =
            |compiler: &Self, owner_children: &[EntityId], leaf: &str| -> Option<EntityId> {
                owner_children
                    .iter()
                    .find(|child| compiler.names.leaf(child) == leaf)
                    .copied()
            };
        // Function parameters.
        let mut kept: BTreeSet<EntityId> = BTreeSet::new();
        let old_params = old
            .as_ref()
            .map(|f| f.parameters.clone())
            .unwrap_or_default();
        let param_slots: Vec<(String, EntityId, TypeExpr)> = match params {
            Some(params) => params
                .into_iter()
                .map(|(leaf, ty)| {
                    let param = match child(self, &old_params, &leaf) {
                        Some(id) => id,
                        None => self.allocate(6, &leaf, Scope::Function(id))?,
                    };
                    kept.insert(param);
                    Ok((leaf, param, ty))
                })
                .collect::<Result<_>>()?,
            None => old_params
                .iter()
                .map(|param| {
                    kept.insert(*param);
                    let ty = match self.program.body(param) {
                        Some(EntityBodyValue::Parameter(p)) => p.value_type.clone(),
                        _ => TypeExpr::Unit,
                    };
                    (self.names.leaf(param), *param, ty)
                })
                .collect(),
        };
        for (ordinal, (_, param, ty)) in param_slots.iter().enumerate() {
            self.put(
                *param,
                EntityBodyValue::Parameter(ParameterBody {
                    owner: id,
                    role: ParameterRole::Function,
                    ordinal: u32::try_from(ordinal).unwrap_or(u32::MAX),
                    value_type: ty.clone(),
                }),
            );
        }
        // Blocks: identities first (terminators may target any block).
        let old_blocks = old.as_ref().map(|f| f.blocks.clone()).unwrap_or_default();
        let mut blocks: Vec<PlannedBlock> = Vec::new();
        for spec in specs {
            match spec {
                BlockSpec::Keep(block) => {
                    kept.insert(block);
                    let Some(EntityBodyValue::Block(body)) = self.program.body(&block) else {
                        continue;
                    };
                    for param in &body.parameters {
                        kept.insert(*param);
                    }
                    for operation in &body.operations {
                        kept.insert(*operation);
                    }
                    blocks.push(PlannedBlock {
                        leaf: self.names.leaf(&block),
                        id: block,
                        params: body
                            .parameters
                            .iter()
                            .map(|param| {
                                let ty = match self.program.body(param) {
                                    Some(EntityBodyValue::Parameter(p)) => p.value_type.clone(),
                                    _ => TypeExpr::Unit,
                                };
                                (self.names.leaf(param), *param, ty)
                            })
                            .collect(),
                        ops: body
                            .operations
                            .iter()
                            .map(|operation| PlannedOp1 {
                                leaf: self.names.leaf(operation),
                                id: *operation,
                                spec: None,
                                pointer: String::new(),
                            })
                            .collect(),
                        term: None,
                        unreachable: body.reachability == Reachability::ExplicitlyUnreachable,
                        pointer: String::new(),
                        keep: true,
                    });
                }
                BlockSpec::Frame {
                    leaf,
                    params,
                    ops,
                    term,
                    unreachable,
                    pointer,
                } => {
                    if blocks.iter().any(|b| b.leaf == leaf) {
                        return Err(frame(&pointer, format!("block `{leaf}` is declared twice")));
                    }
                    let block = match child(self, &old_blocks, &leaf) {
                        Some(id) => id,
                        None => self.allocate(7, &leaf, Scope::Function(id))?,
                    };
                    kept.insert(block);
                    let (old_block_params, old_ops) = match self.program.body(&block) {
                        Some(EntityBodyValue::Block(body)) if !self.is_new(&block) => {
                            (body.parameters.clone(), body.operations.clone())
                        }
                        _ => (Vec::new(), Vec::new()),
                    };
                    let mut block_params = Vec::new();
                    for (index, (param_leaf, ty)) in params.iter().enumerate() {
                        let ty = self.read_type(ty, &format!("{pointer}/params/{index}"))?;
                        let param = match child(self, &old_block_params, param_leaf) {
                            Some(id) => id,
                            None => self.allocate(6, param_leaf, Scope::Block(block))?,
                        };
                        kept.insert(param);
                        block_params.push((param_leaf.clone(), param, ty));
                    }
                    let mut planned_ops = Vec::new();
                    for (index, op) in ops.iter().enumerate() {
                        let op_pointer = format!("{pointer}/ops/{index}");
                        let (op_leaf, spec) = op_head(op, &op_pointer)?;
                        if planned_ops.iter().any(|o: &PlannedOp1| o.leaf == op_leaf)
                            || block_params.iter().any(|(p, _, _)| *p == op_leaf)
                        {
                            return Err(frame(
                                &op_pointer,
                                format!("value `{op_leaf}` is defined twice in block `{leaf}`"),
                            ));
                        }
                        let operation = match child(self, &old_ops, &op_leaf) {
                            Some(id) => id,
                            None => self.allocate(8, &op_leaf, Scope::Block(block))?,
                        };
                        kept.insert(operation);
                        planned_ops.push(PlannedOp1 {
                            leaf: op_leaf,
                            id: operation,
                            spec: Some(spec),
                            pointer: op_pointer,
                        });
                    }
                    blocks.push(PlannedBlock {
                        leaf,
                        id: block,
                        params: block_params,
                        ops: planned_ops,
                        term: Some(term),
                        unreachable,
                        pointer,
                        keep: false,
                    });
                }
            }
        }
        if blocks.is_empty() {
            return Err(frame(pointer, "a function needs at least one block"));
        }
        // Parameters and blocks share the function's name scope (`f.x`).
        for (leaf, _, _) in &param_slots {
            if let Some(block) = blocks.iter().find(|block| block.leaf == *leaf) {
                return Err(frame(
                    &block.pointer,
                    format!(
                        "block `{leaf}` has the name of a parameter of `{name}`; blocks and parameters share the function's names, so rename the block"
                    ),
                ));
            }
        }
        // Old entities no longer named are deleted.
        if let Some(old) = &old {
            let mut owned: Vec<EntityId> = old.parameters.clone();
            for block in &old.blocks {
                owned.push(*block);
                if let Some(EntityBodyValue::Block(body)) = self.program.body(block) {
                    owned.extend(body.parameters.iter().copied());
                    owned.extend(body.operations.iter().copied());
                }
            }
            for entity in owned {
                if !kept.contains(&entity) {
                    self.deletes.insert(entity);
                }
            }
        }
        // Values visible in this function.
        let mut scope = FunctionScope::default();
        for (leaf, param, ty) in &param_slots {
            scope.function_values.insert(
                leaf.clone(),
                ValueSlot {
                    reference: ValueRef::Parameter(*param),
                    ty: Some(ty.clone()),
                },
            );
        }
        for (index, block) in blocks.iter().enumerate() {
            scope.blocks.insert(block.leaf.clone(), index);
            let mut values = BTreeMap::new();
            for (leaf, param, ty) in &block.params {
                values.insert(
                    leaf.clone(),
                    ValueSlot {
                        reference: ValueRef::Parameter(*param),
                        ty: Some(ty.clone()),
                    },
                );
            }
            for op in &block.ops {
                let ty = match (&op.spec, self.program.body(&op.id)) {
                    (None, Some(EntityBodyValue::Operation(body))) => {
                        body.result_types.first().cloned()
                    }
                    _ => None,
                };
                values.insert(
                    op.leaf.clone(),
                    ValueSlot {
                        reference: ValueRef::OperationResult(OperationResultRef {
                            operation: op.id,
                            result_index: 0,
                        }),
                        ty,
                    },
                );
            }
            scope.block_values.push(values);
        }
        // Expected types from uses, for result types an operand cannot fix
        // (`ok`, `err`, `none`, empty `vec`).
        let hints = self.use_hints(&blocks, &scope, &result);
        // Infer result types to a fixed point, then build operations.
        let mut pending: Vec<(usize, usize)> = blocks
            .iter()
            .enumerate()
            .flat_map(|(b, block)| {
                block
                    .ops
                    .iter()
                    .enumerate()
                    .filter(|(_, op)| op.spec.is_some())
                    .map(move |(o, _)| (b, o))
            })
            .collect();
        let mut built: BTreeMap<EntityId, BuiltOp> = BTreeMap::new();
        loop {
            let before = pending.len();
            let mut next = Vec::new();
            let mut errors = Vec::new();
            for (b, o) in pending {
                let op = &blocks[b].ops[o];
                match self.build_op(op, b, &scope, &hints) {
                    Ok(Some((tag, operands, ty, immediate))) => {
                        scope.block_values[b]
                            .get_mut(&op.leaf)
                            .expect("declared")
                            .ty = Some(ty.clone());
                        built.insert(op.id, (tag, operands, ty, immediate));
                    }
                    Ok(None) => next.push((b, o)),
                    Err(error) => {
                        errors.push(error);
                        next.push((b, o));
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            if next.len() == before {
                // Every operation that cannot resolve, in one refusal: one
                // round per misplaced name is the costliest way to learn it.
                if !errors.is_empty() {
                    return Err(combined(errors));
                }
                let (b, o) = next[0];
                let op = &blocks[b].ops[o];
                // A malformed terminator removes the use that would type a
                // result: name it first, as the cause.
                let mut causes = malformed_terminators(&blocks);
                let consequence = if causes.is_empty() {
                    ""
                } else {
                    " (after the terminator problem above is fixed, its use may determine it)"
                };
                causes.push(frame(
                    &op.pointer,
                    format!(
                        "cannot infer the result type of `{}`; add \"type\" (object form) to the operation{consequence}",
                        op.leaf
                    ),
                ));
                return Err(combined(causes));
            }
            pending = next;
        }
        // Bodies.
        let mut terminator_errors = Vec::new();
        for (index, block) in blocks.iter().enumerate() {
            if block.keep {
                continue;
            }
            for (ordinal, (_, param, ty)) in block.params.iter().enumerate() {
                self.put(
                    *param,
                    EntityBodyValue::Parameter(ParameterBody {
                        owner: block.id,
                        role: ParameterRole::Block,
                        ordinal: u32::try_from(ordinal).unwrap_or(u32::MAX),
                        value_type: ty.clone(),
                    }),
                );
            }
            for (ordinal, op) in block.ops.iter().enumerate() {
                let (tag, operands, ty, immediate) =
                    built.remove(&op.id).expect("every frame op is built");
                self.put(
                    op.id,
                    EntityBodyValue::Operation(OperationBody {
                        block: block.id,
                        ordinal: u32::try_from(ordinal).unwrap_or(u32::MAX),
                        opcode: tag,
                        operands,
                        result_types: vec![ty],
                        immediate,
                    }),
                );
            }
            let terminator = match self.build_terminator(
                block.term.as_ref().expect("frame block"),
                index,
                &blocks,
                &scope,
                &format!("{}/term", block.pointer),
            ) {
                Ok(terminator) => terminator,
                Err(error) => {
                    terminator_errors.push(error);
                    continue;
                }
            };
            self.put(
                block.id,
                EntityBodyValue::Block(BlockBody {
                    function: id,
                    parameters: block.params.iter().map(|(_, param, _)| *param).collect(),
                    operations: block.ops.iter().map(|op| op.id).collect(),
                    terminator,
                    reachability: if block.unreachable {
                        Reachability::ExplicitlyUnreachable
                    } else {
                        Reachability::Required
                    },
                }),
            );
        }
        if !terminator_errors.is_empty() {
            return Err(combined(terminator_errors));
        }
        let entry = match decl.get("entry") {
            Some(value) => {
                let leaf = string(value, &format!("{pointer}/entry"))?;
                blocks
                    .iter()
                    .find(|block| block.leaf == leaf)
                    .map(|block| block.id)
                    .ok_or_else(|| {
                        frame(&format!("{pointer}/entry"), format!("no block `{leaf}`"))
                    })?
            }
            None => match &old {
                Some(old) if blocks.iter().any(|block| block.id == old.entry_block) => {
                    old.entry_block
                }
                _ => blocks[0].id,
            },
        };
        let visibility = match decl.get("visibility") {
            Some(value) => read_visibility(Some(value), pointer)?,
            None => old
                .as_ref()
                .map_or(Visibility::Exported, |old| old.visibility),
        };
        self.put(
            id,
            EntityBodyValue::Function(FunctionBody {
                type_parameters: Vec::new(),
                parameters: param_slots.iter().map(|(_, param, _)| *param).collect(),
                result_type: result,
                effects: set(Vec::new()),
                entry_block: entry,
                blocks: blocks.iter().map(|block| block.id).collect(),
                contracts: old
                    .as_ref()
                    .map_or_else(|| set(Vec::new()), |old| old.contracts.clone()),
                visibility,
            }),
        );
        Ok(())
    }

    #[allow(clippy::unused_self)]
    fn resolve_value(
        &self,
        text: &str,
        block: usize,
        scope: &FunctionScope,
        pointer: &str,
    ) -> Result<ValueSlot> {
        let (base, index) = match text.split_once('#') {
            Some((base, index)) => (
                base,
                index
                    .parse::<u32>()
                    .map_err(|_| frame(pointer, format!("bad result index in `{text}`")))?,
            ),
            None => (text, 0),
        };
        let slot = if let Some((block_name, leaf)) = base.split_once('.') {
            let target = scope
                .blocks
                .get(block_name)
                .ok_or_else(|| frame(pointer, format!("no block `{block_name}`")))?;
            let slot = scope.block_values[*target].get(leaf).cloned();
            // `b.x` names results of block b. A block parameter is visible
            // only inside its own block: pass it on as an edge argument.
            if let Some(ValueSlot {
                reference: ValueRef::Parameter(_),
                ..
            }) = &slot
                && *target != block
            {
                return Err(frame(
                    pointer,
                    format!(
                        "`{text}` is a parameter of block `{block_name}`; block parameters are visible only in their own block, so pass it to this block as an edge argument"
                    ),
                ));
            }
            slot
        } else {
            scope.block_values[block]
                .get(base)
                .or_else(|| scope.function_values.get(base))
                .cloned()
        };
        let mut slot = slot.ok_or_else(|| frame(pointer, unresolved(text, base, block, scope)))?;
        if index != 0
            && let ValueRef::OperationResult(result) = &mut slot.reference
        {
            result.result_index = index;
            slot.ty = None;
        }
        Ok(slot)
    }

    #[allow(clippy::unused_self)]
    fn block_target(
        &self,
        value: &Value,
        scope: &FunctionScope,
        pointer: &str,
    ) -> Result<(usize, Vec<String>)> {
        match value {
            Value::String(name) => scope
                .blocks
                .get(name)
                .map(|index| (*index, Vec::new()))
                .ok_or_else(|| frame(pointer, format!("no block `{name}`"))),
            Value::Array(items) if !items.is_empty() => {
                let name = string(&items[0], pointer)?;
                let index = scope
                    .blocks
                    .get(name)
                    .ok_or_else(|| frame(pointer, format!("no block `{name}`")))?;
                let args = items[1..]
                    .iter()
                    .map(|arg| {
                        if arg.is_array() {
                            // A list-wrapped argument is a shape mistake, not a
                            // nested operation.
                            return Err(frame(
                                pointer,
                                format!(
                                    "edge arguments are value names: write [\"{name}\", \"x\", \"y\"], not [\"{name}\", [\"x\", \"y\"]]"
                                ),
                            ));
                        }
                        operand(arg, pointer).map(str::to_owned)
                    })
                    .collect::<Result<_>>()?;
                Ok((*index, args))
            }
            _ => Err(frame(
                pointer,
                "a target is \"block\" or [\"block\", args...]",
            )),
        }
    }

    /// Expected types of values from their uses: returned values take the
    /// result type, edge arguments the target parameter type, call
    /// arguments the callee parameter type.
    fn use_hints(
        &self,
        blocks: &[PlannedBlock],
        scope: &FunctionScope,
        result: &TypeExpr,
    ) -> BTreeMap<EntityId, TypeExpr> {
        let mut hints = BTreeMap::new();
        let mut hint = |slot: &ValueSlot, ty: &TypeExpr| {
            if let ValueRef::OperationResult(result) = slot.reference
                && result.result_index == 0
            {
                constants::hint(&mut hints, result.operation, ty);
            }
        };
        for (index, block) in blocks.iter().enumerate() {
            let Some(term) = &block.term else { continue };
            let Some(items) = term.as_array() else {
                continue;
            };
            let word = items.first().and_then(Value::as_str).unwrap_or("");
            let edge_hint =
                |target: &Value, hint: &mut dyn FnMut(&ValueSlot, &TypeExpr)| -> Result<()> {
                    let (target, args) = self.block_target(target, scope, &block.pointer)?;
                    for (arg, (_, _, ty)) in args.iter().zip(&blocks[target].params) {
                        if let Ok(slot) = self.resolve_value(arg, index, scope, &block.pointer) {
                            hint(&slot, ty);
                        }
                    }
                    Ok(())
                };
            match word {
                "return" => {
                    if let Some(Value::String(value)) = items.get(1)
                        && let Ok(slot) = self.resolve_value(value, index, scope, &block.pointer)
                    {
                        hint(&slot, result);
                    }
                }
                "br" | "jump" => {
                    if items.len() >= 2 {
                        let _ = edge_hint(&branch_target(items), &mut hint);
                    }
                }
                "cond" if items.len() == 4 => {
                    let _ = edge_hint(&items[2], &mut hint);
                    let _ = edge_hint(&items[3], &mut hint);
                }
                "switch" if items.len() >= 3 => {
                    // [key, "b", args...] or [key, ["b", args...]]; the `$`
                    // payload resolves to no value and hints nothing.
                    for case in &items[2..] {
                        let Some(case) = case.as_array().filter(|case| case.len() >= 2) else {
                            continue;
                        };
                        let target = match &case[1] {
                            Value::Array(target) if case.len() == 2 && !target.is_empty() => {
                                Value::Array(target.clone())
                            }
                            other => {
                                let mut target = vec![other.clone()];
                                target.extend(case[2..].iter().cloned());
                                Value::Array(target)
                            }
                        };
                        let _ = edge_hint(&target, &mut hint);
                    }
                }
                _ => {}
            }
            for op in &block.ops {
                let Some(spec) = &op.spec else { continue };
                if spec.opcode.tag != 112 {
                    continue;
                }
                let Some(Value::String(callee)) = spec.args.first() else {
                    continue;
                };
                let Ok(callee) = self.resolve_top(callee, 5, &op.pointer) else {
                    continue;
                };
                let Some((params, _)) = self.signature(&callee) else {
                    continue;
                };
                for (arg, ty) in spec.args[1..].iter().zip(&params) {
                    if let Value::String(arg) = arg
                        && let Ok(slot) = self.resolve_value(arg, index, scope, &op.pointer)
                    {
                        hint(&slot, ty);
                    }
                }
            }
        }
        hints
    }

    /// Builds one operation, or `None` while an operand type is unknown.
    #[allow(clippy::too_many_lines)]
    fn build_op(
        &mut self,
        op: &PlannedOp1,
        block: usize,
        scope: &FunctionScope,
        hints: &BTreeMap<EntityId, TypeExpr>,
    ) -> Result<Option<BuiltOp>> {
        let spec = op.spec.as_ref().expect("frame op");
        let row = spec.opcode;
        let pointer = &op.pointer;
        let explicit = match &spec.ty {
            Some(ty) => Some(self.read_type(ty, &format!("{pointer}/type"))?),
            None => None,
        };
        let hint = explicit.clone().or_else(|| hints.get(&op.id).cloned());
        let mut args = spec.args.iter();
        let mut immediate = Immediate::None;
        let mut immediate_type: Option<TypeExpr> = None;
        let mut variant_payload: Option<Option<TypeExpr>> = None;
        let mut callee_result: Option<TypeExpr> = None;
        let mut field_type: Option<TypeExpr> = None;
        match row.immediate {
            ImmediateKind::None => {}
            ImmediateKind::Entity => {
                let arg = args.next().ok_or_else(|| {
                    frame(
                        pointer,
                        format!("`{}` needs an entity argument", row.mnemonic),
                    )
                })?;
                if row.tag == 1 {
                    let (constant, value) = self.constant_arg(arg, hint.as_ref(), pointer)?;
                    immediate_type = Some(value.value_type);
                    immediate = Immediate::Entity(constant);
                } else {
                    let name = string(arg, pointer)?;
                    let kind = match row.tag {
                        18 => 4,
                        144 => 13,
                        160 => 11,
                        161 => 15,
                        162 => 12,
                        193 => 10,
                        _ => 0,
                    };
                    let id = if kind == 4 {
                        self.type_definition(name)
                            .ok_or_else(|| frame(pointer, format!("no type `{name}`")))?
                    } else {
                        self.names
                            .resolve(name)
                            .filter(|id| {
                                self.program.body(id).is_some_and(|b| b.kind_tag() == kind)
                            })
                            .ok_or_else(|| {
                                frame(
                                    pointer,
                                    format!("no {} named `{name}`", crate::names::kind_name(kind)),
                                )
                            })?
                    };
                    if row.tag == 18 {
                        immediate_type = Some(TypeExpr::Named(NamedType {
                            definition: id,
                            arguments: Vec::new(),
                        }));
                    }
                    if row.tag == 193
                        && let Some(EntityBodyValue::GlobalValue(global)) = self.program.body(&id)
                    {
                        immediate_type = Some(global.value_type.clone());
                    }
                    immediate = Immediate::Entity(id);
                }
            }
            ImmediateKind::Index => {
                let index = args
                    .next()
                    .and_then(Value::as_u64)
                    .and_then(|index| u32::try_from(index).ok())
                    .ok_or_else(|| frame(pointer, "`tuple_get` needs an index"))?;
                immediate = Immediate::Index(index);
            }
            ImmediateKind::Field => {
                let name = string(
                    args.next()
                        .ok_or_else(|| frame(pointer, "`field` needs \"Type.field\""))?,
                    pointer,
                )?;
                let (definition, member) = self.member_path(name, pointer)?;
                if let Some(TypeDefForm::Record(fields)) = self.form(&definition) {
                    field_type = fields
                        .iter()
                        .find(|f| f.member_id == member)
                        .map(|f| f.value_type.clone());
                }
                immediate = Immediate::Field(member);
            }
            ImmediateKind::Variant => {
                let name = string(
                    args.next()
                        .ok_or_else(|| frame(pointer, "`variant` needs \"Type.Case\""))?,
                    pointer,
                )?;
                let (definition, member) = if name.split_once('.').is_some() {
                    self.member_path(name, pointer)?
                } else {
                    let definition = match &hint {
                        Some(TypeExpr::Named(named)) => named.definition,
                        _ => {
                            return Err(frame(
                                pointer,
                                format!("qualify the case: \"Type.{name}\""),
                            ));
                        }
                    };
                    let member = self
                        .member(&definition, name)
                        .ok_or_else(|| frame(pointer, format!("no case `{name}`")))?;
                    (definition, member)
                };
                if let Some(TypeDefForm::Variant(cases)) = self.form(&definition) {
                    variant_payload = cases
                        .iter()
                        .find(|c| c.member_id == member)
                        .map(|c| c.payload_type.clone());
                }
                immediate_type = Some(TypeExpr::Named(NamedType {
                    definition,
                    arguments: Vec::new(),
                }));
                immediate = Immediate::Variant(VariantImmediate {
                    definition,
                    member_id: member,
                });
            }
            ImmediateKind::Observation => {
                let text = string(
                    args.next()
                        .ok_or_else(|| frame(pointer, "`observe` needs an id"))?,
                    pointer,
                )?;
                let bytes = crate::hex::decode32(text)
                    .ok_or_else(|| frame(pointer, "observation ids are 64 hex"))?;
                immediate = Immediate::Observation(bytes);
            }
            ImmediateKind::Function => {
                let name = string(
                    args.next()
                        .ok_or_else(|| frame(pointer, "`call` needs a function name"))?,
                    pointer,
                )?;
                let function = self.resolve_top(name, 5, pointer)?;
                let (params, result) = self
                    .signature(&function)
                    .ok_or_else(|| frame(pointer, format!("`{name}` has no signature")))?;
                if row.tag == 194 {
                    immediate_type = Some(TypeExpr::FunctionRef(FunctionType {
                        parameters: params,
                        result: Box::new(result),
                        effects: Vec::new(),
                    }));
                } else {
                    callee_result = Some(result);
                }
                immediate = Immediate::Function(FunctionRefValue {
                    function,
                    type_arguments: Vec::new(),
                });
            }
        }
        let mut operands = Vec::new();
        let mut operand_types = Vec::new();
        let mut operand_names = Vec::new();
        for arg in args {
            let text = operand(arg, pointer)?;
            let slot = self.resolve_value(text, block, scope, pointer)?;
            operands.push(slot.reference);
            operand_types.push(slot.ty);
            operand_names.push(text.to_owned());
        }
        if operand_types.iter().any(Option::is_none) && explicit.is_none() {
            return Ok(None);
        }
        if operand_types.iter().all(Option::is_some) {
            let typed: Vec<TypeExpr> = operand_types.iter().flatten().cloned().collect();
            self.check_operands(row, &operand_names, &typed, pointer)?;
        }
        let known: Vec<TypeExpr> = operand_types.into_iter().flatten().collect();
        // `tuple_get` reads its element type from the tuple operand.
        if let (Immediate::Index(index), Some(TypeExpr::Tuple(items))) = (&immediate, known.first())
        {
            field_type = items.get(*index as usize).cloned();
        }
        // `ok`/`err` whose operand disagrees with the expected Result arm:
        // say so plainly instead of failing inference.
        if explicit.is_none()
            && matches!(row.tag, 130 | 131)
            && let (Some(TypeExpr::Result { ok, error }), Some(operand)) = (&hint, known.first())
        {
            let (arm, want) = if row.tag == 130 {
                ("Ok", ok)
            } else {
                ("Err", error)
            };
            if **want != *operand {
                return Err(frame(
                    pointer,
                    format!(
                        "`{}` passes a {} where the result's {arm} type is {}",
                        op.leaf,
                        types::render(operand, self.names),
                        types::render(want, self.names)
                    ),
                ));
            }
        }
        let ty = if let Some(ty) = explicit {
            ty
        } else {
            let inferred = infer(
                row,
                &known,
                hint.as_ref(),
                immediate_type.as_ref(),
                variant_payload.as_ref(),
                callee_result.as_ref(),
                field_type.as_ref(),
            );
            match inferred {
                Some(ty) => ty,
                None => return Ok(None),
            }
        };
        Ok(Some((row.tag, operands, ty, immediate)))
    }

    /// Operand types the kernel would refuse later with only a function-level
    /// locator (`VM_LOWER_SIGNATURE_MISMATCH`), named here at the operation.
    fn check_operands(
        &self,
        row: &OpcodeRow,
        names: &[String],
        types: &[TypeExpr],
        pointer: &str,
    ) -> Result<()> {
        let render = |ty: &TypeExpr| types::render(ty, self.names);
        let wrapped = |ty: &TypeExpr| matches!(ty, TypeExpr::Result { .. } | TypeExpr::Option(_));
        let scalar = matches!(row.tag, 64..=71 | 80..=85 | 98..=101);
        let checked = if matches!(row.tag, 70 | 71) {
            1
        } else {
            types.len()
        };
        if scalar {
            for (name, ty) in names.iter().zip(types).take(checked) {
                if wrapped(ty) {
                    return Err(frame(
                        pointer,
                        format!(
                            "`{name}` is a {}, not a value `{}` can use: switch on it first (the \"Ok\" or \"Some\" case's `$` is the value)",
                            render(ty),
                            row.mnemonic
                        ),
                    ));
                }
            }
        }
        if matches!(row.tag, 102..=104) {
            for (name, ty) in names.iter().zip(types) {
                if *ty != TypeExpr::Bool {
                    return Err(frame(
                        pointer,
                        format!(
                            "`{}` takes bool operands; `{name}` is {}",
                            row.mnemonic,
                            render(ty)
                        ),
                    ));
                }
            }
        }
        let same = scalar && !matches!(row.tag, 70 | 71) || matches!(row.tag, 96 | 97);
        if same && let (Some(first), Some(first_name)) = (types.first(), names.first()) {
            for (name, ty) in names.iter().zip(types).skip(1) {
                if ty != first {
                    return Err(frame(
                        pointer,
                        format!(
                            "the operands of `{}` must have one type: `{first_name}` is {}, `{name}` is {}",
                            row.mnemonic,
                            render(first),
                            render(ty)
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    fn member_path(&self, path: &str, pointer: &str) -> Result<(EntityId, MemberId)> {
        let (type_name, leaf) = path
            .rsplit_once('.')
            .ok_or_else(|| frame(pointer, format!("expected \"Type.member\", got `{path}`")))?;
        let definition = self
            .type_definition(type_name)
            .ok_or_else(|| frame(pointer, format!("no type `{type_name}`")))?;
        let member = self.member(&definition, leaf).ok_or_else(|| {
            frame(
                pointer,
                format!("type `{type_name}` has no member `{leaf}`"),
            )
        })?;
        Ok((definition, member))
    }

    /// Resolves a `const` argument: a constant name, or a literal that
    /// reuses an equal constant or creates one.
    fn constant_arg(
        &mut self,
        arg: &Value,
        hint: Option<&TypeExpr>,
        pointer: &str,
    ) -> Result<(EntityId, ConstValue)> {
        if let Value::String(name) = arg {
            let id = self.resolve_top(name, 9, pointer)?;
            let value = self
                .constant_value(&id)
                .ok_or_else(|| frame(pointer, format!("`{name}` is not a constant")))?;
            return Ok((id, value));
        }
        let ty = constants::immediate_type(arg, hint, self, pointer)?;
        let data = match arg {
            Value::Object(object) if object.contains_key("value") => &object["value"],
            _ => arg,
        };
        let value = values::read(data, &ty, self, pointer)
            .map_err(|error| frame(pointer, error.detail()))?;
        // Reuse an equal frame constant, or an equal live constant that this
        // frame neither deletes nor gives a new value.
        for (id, existing) in &self.constants {
            if *existing == value {
                return Ok((*id, value));
            }
        }
        for object in self.program.objects() {
            let id = object.record().entity_id;
            if let EntityBodyValue::Constant(constant) = &object.record().body
                && constant.value == value
                && !self.deletes.contains(&id)
                && !self.constants.contains_key(&id)
            {
                return Ok((id, value));
            }
        }
        let base = match &value.data {
            ConstData::SInt(n) if *n < 0 => format!("k_neg{}", n.unsigned_abs()),
            ConstData::SInt(n) => format!("k_{n}"),
            ConstData::UInt(n) => format!("k_{n}u"),
            ConstData::Bool(flag) => format!("k_{flag}"),
            _ => "k_const".to_owned(),
        };
        let leaf = self.fresh_name(&base, false);
        let id = if let Some(identities) = &self.identities {
            let id = identities.constant(&value, self.program)?;
            self.record_allocation(id, 9, &leaf)
        } else {
            self.allocate(9, &leaf, Scope::Top)?
        };
        self.created_top.push((id, 9));
        self.top.insert(leaf, TopEntry { id, kind: 9 });
        self.constants.insert(id, value.clone());
        self.put(
            id,
            EntityBodyValue::Constant(ConstantBody {
                value: value.clone(),
            }),
        );
        Ok((id, value))
    }

    fn edge(
        &self,
        target: &Value,
        block: usize,
        blocks: &[PlannedBlock],
        scope: &FunctionScope,
        pointer: &str,
    ) -> Result<TargetEdge> {
        let (target, args) = self.block_target(target, scope, pointer)?;
        let params = &blocks[target].params;
        let leaf = &blocks[target].leaf;
        if args.len() != params.len() {
            return Err(frame(
                pointer,
                format!(
                    "block `{leaf}` takes {} argument(s) ({}); this edge passes {}",
                    params.len(),
                    params
                        .iter()
                        .map(|(name, _, ty)| format!("{name}: {}", types::render(ty, self.names)))
                        .collect::<Vec<_>>()
                        .join(", "),
                    args.len()
                ),
            ));
        }
        let mut arguments = Vec::new();
        for (arg, (param, _, ty)) in args.iter().zip(params) {
            let slot = self.resolve_value(arg, block, scope, pointer)?;
            if let Some(actual) = &slot.ty
                && actual != ty
            {
                return Err(frame(
                    pointer,
                    format!(
                        "`{arg}` is {} but parameter `{param}` of block `{leaf}` is {}{}",
                        types::render(actual, self.names),
                        types::render(ty, self.names),
                        if matches!(actual, TypeExpr::Result { .. } | TypeExpr::Option(_)) {
                            "; switch on it and pass the case's `$`"
                        } else {
                            ""
                        }
                    ),
                ));
            }
            arguments.push(slot.reference);
        }
        Ok(TargetEdge {
            target: blocks[target].id,
            arguments,
        })
    }

    #[allow(clippy::too_many_lines)]
    fn build_terminator(
        &self,
        term: &Value,
        block: usize,
        blocks: &[PlannedBlock],
        scope: &FunctionScope,
        pointer: &str,
    ) -> Result<Terminator> {
        let items = array(term, pointer)?;
        let word = items
            .first()
            .and_then(Value::as_str)
            .ok_or_else(|| frame(pointer, "a terminator starts with its word"))?;
        let value_at = |index: usize| -> Result<ValueSlot> {
            let text = operand(
                items
                    .get(index)
                    .ok_or_else(|| frame(pointer, "missing operand"))?,
                &format!("{pointer}/{index}"),
            )?;
            self.resolve_value(text, block, scope, pointer)
        };
        Ok(match word {
            "return" if items.len() == 2 => Terminator::Return(ReturnTerminator {
                value: value_at(1)?.reference,
            }),
            "br" | "jump" if items.len() >= 2 => Terminator::Branch(BranchTerminator {
                edge: self.edge(&branch_target(items), block, blocks, scope, pointer)?,
            }),
            "cond" if items.len() == 4 => Terminator::CondBranch(CondBranchTerminator {
                condition: value_at(1)?.reference,
                if_true: self.edge(&items[2], block, blocks, scope, &format!("{pointer}/2"))?,
                if_false: self.edge(&items[3], block, blocks, scope, &format!("{pointer}/3"))?,
            }),
            "switch" if items.len() >= 3 => {
                let scrutinee = value_at(1)?;
                let definition = match &scrutinee.ty {
                    Some(TypeExpr::Named(named)) => Some(named.definition),
                    _ => None,
                };
                let mut cases = Vec::new();
                for (index, case) in items[2..].iter().enumerate() {
                    let case_pointer = format!("{pointer}/{}", index + 2);
                    let case = case
                        .as_array()
                        .filter(|case| case.len() >= 2)
                        .ok_or_else(|| frame(&case_pointer, "a case is [key, block, args...]"))?;
                    let key = string(&case[0], &case_pointer)?;
                    let case_key = match key {
                        "Ok" => CaseKey::Builtin(BuiltinCase::Ok),
                        "Err" => CaseKey::Builtin(BuiltinCase::Err),
                        "Some" => CaseKey::Builtin(BuiltinCase::Some),
                        "None" => CaseKey::Builtin(BuiltinCase::None),
                        member => {
                            let member = member.rsplit('.').next().unwrap_or(member);
                            let definition = definition.ok_or_else(|| {
                                frame(&case_pointer, not_a_case(member, scrutinee.ty.as_ref()))
                            })?;
                            CaseKey::Member(self.member(&definition, member).ok_or_else(|| {
                                frame(&case_pointer, format!("no case `{member}`"))
                            })?)
                        }
                    };
                    // Past the target, an array is never valid; one that starts
                    // with a case key is a case left inside this one.
                    if let Some(inner) = case[2..].iter().find_map(|item| {
                        let inner = item.as_array()?;
                        let word = inner.first()?.as_str()?;
                        let is_case = matches!(word, "Ok" | "Err" | "Some" | "None")
                            || definition.is_some_and(|definition| {
                                self.member(&definition, word.rsplit('.').next().unwrap_or(word))
                                    .is_some()
                            });
                        is_case.then_some(word)
                    }) {
                        return Err(frame(
                            &case_pointer,
                            format!(
                                "the `{key}` case contains the `{inner}` case: close [\"{key}\", ...] before [\"{inner}\", ...]"
                            ),
                        ));
                    }
                    if case[1].is_array() && case.len() > 2 {
                        return Err(frame(
                            &case_pointer,
                            format!(
                                "a bracketed target stands alone: write [\"{key}\", [\"block\", args...]] or [\"{key}\", \"block\", args...]"
                            ),
                        ));
                    }
                    // `[key, "b", args...]`, or `[key, ["b", args...]]` as in `cond`.
                    let (target_value, rest) = match &case[1] {
                        Value::Array(target) if case.len() == 2 && !target.is_empty() => {
                            (&target[0], &target[1..])
                        }
                        other => (other, &case[2..]),
                    };
                    let target_name = string(target_value, &case_pointer)?;
                    let target = scope
                        .blocks
                        .get(target_name)
                        .ok_or_else(|| frame(&case_pointer, format!("no block `{target_name}`")))?;
                    let arguments = rest
                        .iter()
                        .map(|arg| {
                            let text = operand(arg, &case_pointer)?;
                            if text == "$" {
                                Ok(SwitchArgument::CasePayload)
                            } else {
                                self.resolve_value(text, block, scope, &case_pointer)
                                    .map(|slot| SwitchArgument::Value(slot.reference))
                            }
                        })
                        .collect::<Result<_>>()?;
                    cases.push(SwitchCase {
                        case_key,
                        edge: SwitchEdge {
                            target: blocks[*target].id,
                            arguments,
                        },
                    });
                }
                cases.sort_by(|a, b| a.case_key.cmp(&b.case_key));
                Terminator::VariantSwitch(VariantSwitchTerminator {
                    value: scrutinee.reference,
                    cases,
                })
            }
            "trap" => {
                let code = match items
                    .get(1)
                    .and_then(Value::as_str)
                    .unwrap_or("unreachable")
                {
                    "unreachable" => TrapCode::Unreachable,
                    "resource_exhausted" => TrapCode::ResourceExhausted,
                    "adapter_contract_violation" => TrapCode::AdapterContractViolation,
                    "internal_invariant" => TrapCode::InternalInvariant,
                    other => return Err(frame(pointer, format!("unknown trap code `{other}`"))),
                };
                let payload = match items.get(2) {
                    Some(_) => Some(value_at(2)?.reference),
                    None => None,
                };
                Terminator::Trap(TrapTerminator { code, payload })
            }
            other => return Err(frame(pointer, terminator_shape(other, items))),
        })
    }

    #[allow(clippy::too_many_lines)]
    /// Where one edit applies: the function, the block and operation it
    /// replaces, and the replacement spec (named like the operation).
    fn edit_target(
        &self,
        decl: &serde_json::Map<String, Value>,
        pointer: &str,
    ) -> Result<(String, EntityId, String, EntityId, EntityId, Value)> {
        let function_name = name_of(decl, &["fn", "function", "name"], pointer)?;
        let function = self.resolve_top(function_name, 5, pointer)?;
        let replace_path = string(
            decl.get("replace_op").ok_or_else(|| {
                frame(
                    pointer,
                    "an edit is {\"fn\", \"replace_op\": \"block.op\", \"with\": op}",
                )
            })?,
            &format!("{pointer}/replace_op"),
        )?;
        let (block_leaf, op_leaf) = replace_path
            .split_once('.')
            .ok_or_else(|| frame(&format!("{pointer}/replace_op"), "expected \"block.op\""))?;
        let with = decl
            .get("with")
            .ok_or_else(|| frame(pointer, "missing \"with\""))?;
        let Some(EntityBodyValue::Function(body)) = self.program.body(&function) else {
            return Err(frame(pointer, "edits apply to live functions"));
        };
        let block = body
            .blocks
            .iter()
            .find(|block| self.names.leaf(block) == block_leaf)
            .copied()
            .ok_or_else(|| {
                frame(
                    pointer,
                    format!("no block `{block_leaf}` in `{function_name}`"),
                )
            })?;
        let Some(EntityBodyValue::Block(block_body)) = self.program.body(&block) else {
            return Err(frame(pointer, "not a live block"));
        };
        let operation = block_body
            .operations
            .iter()
            .find(|op| self.names.leaf(op) == op_leaf)
            .copied()
            .ok_or_else(|| {
                frame(
                    pointer,
                    format!(
                        "no operation `{op_leaf}` in `{block_leaf}`{}",
                        self.expanded_home(&body.blocks, block_leaf, op_leaf, function_name)
                    ),
                )
            })?;
        let replacement = match with {
            Value::Array(items) => {
                let mut items = items.clone();
                items.insert(0, Value::from(op_leaf));
                Value::Array(items)
            }
            Value::Object(object) => {
                let mut object = object.clone();
                object.insert("name".to_owned(), Value::from(op_leaf));
                Value::Object(object)
            }
            _ => return Err(frame(&format!("{pointer}/with"), "expected an operation")),
        };
        Ok((
            function_name.to_owned(),
            function,
            block_leaf.to_owned(),
            block,
            operation,
            replacement,
        ))
    }

    /// Where an authored operation of a block the authoring dialect split
    /// now lives (a generated block `block__...` holding `op`, or `op__r`
    /// for a checked operation), with the fix; empty when nowhere.
    fn expanded_home(
        &self,
        blocks: &[EntityId],
        block_leaf: &str,
        op_leaf: &str,
        function_name: &str,
    ) -> String {
        let piece_prefix = format!("{block_leaf}__");
        let checked = format!("{op_leaf}__r");
        for block in blocks {
            let piece = self.names.leaf(block);
            if !piece.starts_with(&piece_prefix) {
                continue;
            }
            let Some(EntityBodyValue::Block(body)) = self.program.body(block) else {
                continue;
            };
            let Some(found) = body
                .operations
                .iter()
                .map(|op| self.names.leaf(op))
                .find(|leaf| *leaf == op_leaf || *leaf == checked)
            else {
                continue;
            };
            return format!(
                "; the authoring dialect's expansion holds it as `{piece}.{found}` (block `{block_leaf}` was split into generated blocks): to change it, restate block `{block_leaf}` with patch: {{\"af1\": 1, \"afx\": 1, \"patch\": [{{\"fn\": \"{function_name}\", \"blocks\": {{\"{block_leaf}\": {{...}}}}}}]}}"
            );
        }
        String::new()
    }

    /// Applies every edit of the frame. Edits are grouped by function: each
    /// edited block is restated once with all of its replaced operations,
    /// and each function is restated once, so no edit overwrites another.
    #[allow(clippy::type_complexity)]
    fn edits(&mut self, edit_list: &[Value]) -> Result<()> {
        let mut groups: Vec<(
            String,
            EntityId,
            String,
            Vec<(String, EntityId, Vec<(EntityId, Value)>)>,
        )> = Vec::new();
        for (index, value) in edit_list.iter().enumerate() {
            let pointer = format!("/edit/{index}");
            let decl = value.as_object().expect("checked");
            let (function_name, function, block_leaf, block, operation, replacement) =
                self.edit_target(decl, &pointer)?;
            let position = groups
                .iter()
                .position(|(_, id, _, _)| *id == function)
                .unwrap_or_else(|| {
                    groups.push((function_name, function, pointer.clone(), Vec::new()));
                    groups.len() - 1
                });
            let group = &mut groups[position];
            let blocks = &mut group.3;
            let position = blocks
                .iter()
                .position(|(_, id, _)| *id == block)
                .unwrap_or_else(|| {
                    blocks.push((block_leaf, block, Vec::new()));
                    blocks.len() - 1
                });
            let entry = &mut blocks[position];
            if entry.2.iter().any(|(op, _)| *op == operation) {
                return Err(frame(
                    &pointer,
                    "this operation is edited twice in the frame; keep one edit",
                ));
            }
            entry.2.push((operation, replacement));
        }
        for (function_name, function, pointer, blocks) in groups {
            let mut restated = serde_json::Map::new();
            for (block_leaf, block, replacements) in blocks {
                restated.insert(block_leaf, self.restate_block(&block, &replacements)?);
            }
            let mut patch = serde_json::Map::new();
            patch.insert("fn".to_owned(), Value::from(function_name.as_str()));
            patch.insert("blocks".to_owned(), Value::Object(restated));
            let (_, result) = self.signature(&function).expect("live function");
            let specs = self.patch_specs(&function, &patch, &pointer)?;
            self.define_function(
                function,
                &function_name,
                None,
                result,
                specs,
                &patch,
                &pointer,
            )?;
        }
        Ok(())
    }

    /// A live block as a patch spec, with some operations replaced.
    fn restate_block(&self, block: &EntityId, replacements: &[(EntityId, Value)]) -> Result<Value> {
        let Some(EntityBodyValue::Block(block_body)) = self.program.body(block) else {
            return Err(AgentError::new(
                AgentErrorCode::FrameInvalid,
                "not a live block",
            ));
        };
        let mut ops = Vec::new();
        for op in &block_body.operations {
            match replacements.iter().find(|(id, _)| id == op) {
                Some((_, replacement)) => ops.push(replacement.clone()),
                None => ops.push(self.existing_op_spec(op, block)?),
            }
        }
        let mut spec = serde_json::Map::new();
        spec.insert("ops".to_owned(), Value::Array(ops));
        spec.insert(
            "params".to_owned(),
            Value::Array(
                block_body
                    .parameters
                    .iter()
                    .map(|param| {
                        let ty = match self.program.body(param) {
                            Some(EntityBodyValue::Parameter(p)) => {
                                types::render(&p.value_type, self.names)
                            }
                            _ => "unit".to_owned(),
                        };
                        Value::Array(vec![Value::from(self.names.leaf(param)), Value::from(ty)])
                    })
                    .collect(),
            ),
        );
        spec.insert(
            "term".to_owned(),
            self.existing_term_spec(&block_body.terminator, block),
        );
        Ok(Value::Object(spec))
    }

    /// Restates a live operation as AF1 (used by `edit`).
    fn existing_op_spec(&self, op: &EntityId, block: &EntityId) -> Result<Value> {
        let Some(EntityBodyValue::Operation(body)) = self.program.body(op) else {
            return Err(frame("", "not a live operation"));
        };
        let row = opcodes::by_tag(body.opcode).ok_or_else(|| frame("", "unknown opcode"))?;
        let mut object = serde_json::Map::new();
        object.insert("name".to_owned(), Value::from(self.names.leaf(op)));
        object.insert("op".to_owned(), Value::from(row.mnemonic));
        let mut args = Vec::new();
        match &body.immediate {
            Immediate::None => {}
            Immediate::Entity(id) => args.push(Value::from(self.names.name(id))),
            Immediate::Index(index) => args.push(Value::from(*index)),
            Immediate::Field(member) => args.push(Value::from(self.names.member_any(member))),
            Immediate::Variant(variant) => {
                args.push(Value::from(
                    self.names.member(&variant.definition, &variant.member_id),
                ));
            }
            Immediate::Observation(bytes) => args.push(Value::from(crate::hex::encode(bytes))),
            Immediate::Function(function) => {
                args.push(Value::from(self.names.name(&function.function)));
            }
        }
        for operand in &body.operands {
            args.push(Value::from(self.names.value(operand, Some(block))));
        }
        object.insert("args".to_owned(), Value::Array(args));
        if let Some(ty) = body.result_types.first() {
            object.insert(
                "type".to_owned(),
                Value::from(types::render(ty, self.names)),
            );
        }
        Ok(Value::Object(object))
    }

    /// Restates a live terminator as AF1 (used by `edit`).
    fn existing_term_spec(&self, terminator: &Terminator, block: &EntityId) -> Value {
        let value = |v: &ValueRef| Value::from(self.names.value(v, Some(block)));
        let edge = |edge: &TargetEdge| {
            let mut items = vec![Value::from(self.names.leaf(&edge.target))];
            items.extend(edge.arguments.iter().map(value));
            Value::Array(items)
        };
        match terminator {
            Terminator::Return(ret) => Value::Array(vec![Value::from("return"), value(&ret.value)]),
            Terminator::Branch(branch) => {
                let mut items = vec![Value::from("br")];
                if let Value::Array(rest) = edge(&branch.edge) {
                    items.extend(rest);
                }
                Value::Array(items)
            }
            Terminator::CondBranch(cond) => Value::Array(vec![
                Value::from("cond"),
                value(&cond.condition),
                edge(&cond.if_true),
                edge(&cond.if_false),
            ]),
            Terminator::VariantSwitch(switch) => {
                let mut items = vec![Value::from("switch"), value(&switch.value)];
                for case in &switch.cases {
                    let key = match case.case_key {
                        CaseKey::Builtin(builtin) => format!("{builtin:?}"),
                        CaseKey::Member(member) => self.names.member_any(&member),
                    };
                    let mut case_items = vec![
                        Value::from(key),
                        Value::from(self.names.leaf(&case.edge.target)),
                    ];
                    for argument in &case.edge.arguments {
                        case_items.push(match argument {
                            SwitchArgument::Value(v) => value(v),
                            SwitchArgument::CasePayload => Value::from("$"),
                        });
                    }
                    items.push(Value::Array(case_items));
                }
                Value::Array(items)
            }
            Terminator::Trap(trap) => {
                let code = match trap.code {
                    TrapCode::Unreachable => "unreachable",
                    TrapCode::ResourceExhausted => "resource_exhausted",
                    TrapCode::AdapterContractViolation => "adapter_contract_violation",
                    TrapCode::InternalInvariant => "internal_invariant",
                };
                let mut items = vec![Value::from("trap"), Value::from(code)];
                if let Some(payload) = &trap.payload {
                    items.push(value(payload));
                }
                Value::Array(items)
            }
        }
    }

    fn test(
        &mut self,
        id: EntityId,
        target: EntityId,
        decl: &serde_json::Map<String, Value>,
        pointer: &str,
    ) -> Result<()> {
        let (params, result) = self
            .signature(&target)
            .ok_or_else(|| frame(pointer, "the target has no signature"))?;
        let inputs_value = decl
            .get("args")
            .or_else(|| decl.get("inputs"))
            .cloned()
            .unwrap_or(Value::Array(Vec::new()));
        let inputs_list = array(&inputs_value, &format!("{pointer}/args"))?;
        if inputs_list.len() != params.len() {
            return Err(frame(
                &format!("{pointer}/args"),
                format!(
                    "the target takes {} argument(s), got {}",
                    params.len(),
                    inputs_list.len()
                ),
            ));
        }
        let inputs = inputs_list
            .iter()
            .zip(&params)
            .enumerate()
            .map(|(index, (value, ty))| {
                values::read(value, ty, self, &format!("{pointer}/args/{index}"))
                    .map_err(|error| frame(&format!("{pointer}/args/{index}"), error.detail()))
            })
            .collect::<Result<Vec<_>>>()?;
        let expect = decl
            .get("expect")
            .ok_or_else(|| frame(pointer, "missing \"expect\""))?;
        let expected = match expect.as_object().and_then(|object| object.get("trap")) {
            Some(code) => ExpectedOutcome::FailureCode(match code {
                Value::Number(number) => number
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or_else(|| frame(pointer, "bad trap code"))?,
                Value::String(text) => crate::exec::trap_code(text)
                    .ok_or_else(|| frame(pointer, format!("unknown trap `{text}`")))?,
                _ => return Err(frame(pointer, "bad trap code")),
            }),
            None => ExpectedOutcome::Value(
                values::read(expect, &result, self, &format!("{pointer}/expect"))
                    .map_err(|error| frame(&format!("{pointer}/expect"), error.detail()))?,
            ),
        };
        let mut limits = default_test_limits(&self.ceilings);
        let object = match decl.get("limits") {
            None => None,
            Some(Value::Object(object)) => Some(object),
            Some(_) => {
                return Err(frame(
                    &format!("{pointer}/limits"),
                    format!("limits are an object of integers, keyed by {LIMIT_KEYS}"),
                ));
            }
        };
        if let Some(object) = object {
            for (key, value) in object {
                let number = value.as_u64().ok_or_else(|| {
                    frame(&format!("{pointer}/limits/{key}"), "expected an integer")
                })?;
                match key.as_str() {
                    "fuel" => limits.fuel = number,
                    "memory_bytes" => limits.memory_bytes = number,
                    "output_bytes" => limits.output_bytes = number,
                    "effect_count" => limits.effect_count = number,
                    "call_depth" => limits.call_depth = number,
                    "wall_timeout_millis" => limits.wall_timeout_millis = number,
                    _ => {
                        return Err(frame(
                            &format!("{pointer}/limits/{key}"),
                            format!("unknown limit `{key}`: the limits are {LIMIT_KEYS}"),
                        ));
                    }
                }
            }
        }
        self.tests.push(id);
        self.put(
            id,
            EntityBodyValue::TestCase(TestCaseBody {
                target,
                inputs,
                effect_environment: EffectEnvironment::Replay(Vec::new()),
                expected,
                observations: Vec::new(),
                resource_limits: limits,
            }),
        );
        Ok(())
    }

    /// Keeps namespace membership consistent: deleted members leave every
    /// namespace; created top-level entities (not tests) join the named
    /// namespace, or the only namespace when there is exactly one. With
    /// `"namespace": null` they join none; existing members stay and deleted
    /// members still leave.
    fn namespaces(&mut self, requested: Option<&Value>) -> Result<()> {
        let namespaces: Vec<(EntityId, NamespaceBody)> = self
            .program
            .objects()
            .iter()
            .filter_map(|object| match &object.record().body {
                EntityBodyValue::Namespace(namespace) => {
                    Some((object.record().entity_id, namespace.clone()))
                }
                _ => None,
            })
            .collect();
        let target = match requested {
            Some(Value::Null) => None,
            Some(value) => {
                let name = value
                    .as_str()
                    .ok_or_else(|| frame("/namespace", "a namespace name, or null to join none"))?;
                Some(self.resolve_top(name, 3, "/namespace")?)
            }
            None if namespaces.len() == 1 => Some(namespaces[0].0),
            None => None,
        };
        for (id, namespace) in namespaces {
            if self.deletes.contains(&id) {
                continue;
            }
            let mut members: Vec<EntityId> = namespace
                .members
                .as_slice()
                .iter()
                .filter(|member| !self.deletes.contains(member))
                .copied()
                .collect();
            if Some(id) == target {
                for (created, kind) in &self.created_top {
                    if *kind != 14 && !members.contains(created) {
                        members.push(*created);
                    }
                }
            }
            let members = set(members);
            if members != namespace.members {
                self.put(
                    id,
                    EntityBodyValue::Namespace(NamespaceBody {
                        parent: namespace.parent,
                        members,
                    }),
                );
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Result<Compiled> {
        let mut compiled = Compiled {
            names: self.new_names,
            notes: self.notes,
            functions: self.functions,
            tests: self.tests,
            ..Compiled::default()
        };
        for (id, kind) in &self.creates {
            if self.deletes.contains(id) {
                continue;
            }
            let body = self.bodies.remove(id).ok_or_else(|| {
                AgentError::new(
                    AgentErrorCode::FrameInvalid,
                    format!(
                        "internal: no body for created {}",
                        crate::hex::short(id.as_bytes())
                    ),
                )
            })?;
            compiled.ops.push(PlannedOp {
                kind: *kind,
                target: *id,
                payload: MutationPayload::CreateEntity(body),
                field_tag: None,
            });
            compiled.created += 1;
        }
        for (id, body) in self.bodies {
            if self.deletes.contains(&id) || self.program.body(&id) == Some(&body) {
                continue;
            }
            compiled.ops.push(PlannedOp {
                kind: body.kind_tag(),
                target: id,
                payload: MutationPayload::ReplaceEntityVersion(body),
                field_tag: None,
            });
            compiled.replaced += 1;
        }
        for id in &self.deletes {
            let Some(body) = self.program.body(id) else {
                continue;
            };
            compiled.ops.push(PlannedOp {
                kind: body.kind_tag(),
                target: *id,
                payload: MutationPayload::DeleteEntityBinding,
                field_tag: None,
            });
            compiled.deleted += 1;
        }
        Ok(compiled)
    }
}

fn set(values: Vec<EntityId>) -> EntityIdSet {
    let mut values = values;
    values.sort_unstable();
    values.dedup();
    EntityIdSet::from_unsorted(values).expect("deduplicated")
}

fn read_visibility(value: Option<&Value>, pointer: &str) -> Result<Visibility> {
    Ok(match value.and_then(Value::as_str) {
        None | Some("exported") => Visibility::Exported,
        Some("private") => Visibility::Private,
        Some("package") => Visibility::Package,
        Some("workspace") => Visibility::Workspace,
        Some(other) => return Err(frame(pointer, format!("unknown visibility `{other}`"))),
    })
}

/// Infers an operation's result type from its opcode, operand types, and
/// context; `None` when the context does not determine it.
pub(crate) fn infer(
    row: &OpcodeRow,
    operands: &[TypeExpr],
    hint: Option<&TypeExpr>,
    immediate: Option<&TypeExpr>,
    variant_payload: Option<&Option<TypeExpr>>,
    callee_result: Option<&TypeExpr>,
    field_type: Option<&TypeExpr>,
) -> Option<TypeExpr> {
    let first = operands.first();
    let arithmetic = |ty: &TypeExpr| TypeExpr::Result {
        ok: Box::new(ty.clone()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)),
    };
    Some(match row.tag {
        1 | 18 | 20 | 193 | 194 => immediate?.clone(),
        16 => TypeExpr::Tuple(operands.to_vec()),
        17 | 19 => field_type?.clone(),
        21 => match variant_payload {
            Some(Some(payload)) => TypeExpr::Option(Box::new(payload.clone())),
            _ => TypeExpr::Option(Box::new(hint.and_then(option_item)?.clone())),
        },
        32 => match first {
            Some(ty) => TypeExpr::Vector(Box::new(ty.clone())),
            None => hint?.clone(),
        },
        33 => TypeExpr::UInt(IntegerWidth::from_bits(64)),
        34 => match first? {
            TypeExpr::Vector(item) => TypeExpr::Option(item.clone()),
            _ => return None,
        },
        35 => TypeExpr::Result {
            ok: Box::new(first?.clone()),
            error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)),
        },
        37 => match first? {
            TypeExpr::OrderedMap { value, .. } => TypeExpr::Option(value.clone()),
            _ => return None,
        },
        38 | 96..=104 => TypeExpr::Bool,
        39 | 40 | 80..=85 => first?.clone(),
        64..=71 => arithmetic(first?),
        112 => callee_result?.clone(),
        128 => TypeExpr::Option(Box::new(first?.clone())),
        130 => match hint {
            Some(TypeExpr::Result { ok, error }) if **ok == *first? => TypeExpr::Result {
                ok: ok.clone(),
                error: error.clone(),
            },
            _ => return None,
        },
        131 => match hint {
            Some(TypeExpr::Result { ok, error }) if **error == *first? => TypeExpr::Result {
                ok: ok.clone(),
                error: error.clone(),
            },
            _ => return None,
        },
        144 => TypeExpr::Result {
            ok: Box::new(TypeExpr::Unit),
            error: Box::new(TypeExpr::BuiltinFailure(
                BuiltinFailureKind::ContractViolation,
            )),
        },
        145 | 178 => TypeExpr::Unit,
        176 => TypeExpr::LocalCell(Box::new(first?.clone())),
        177 => match first? {
            TypeExpr::LocalCell(item) => (**item).clone(),
            _ => return None,
        },
        192 => TypeExpr::Bytes,
        _ => hint?.clone(),
    })
}

fn option_item(ty: &TypeExpr) -> Option<&TypeExpr> {
    match ty {
        TypeExpr::Option(item) => Some(item),
        _ => None,
    }
}

/// A block as the frame states it, or an existing block kept unchanged.
enum BlockSpec {
    Keep(EntityId),
    Frame {
        leaf: String,
        params: Vec<(String, Value)>,
        ops: Vec<Value>,
        term: Value,
        unreachable: bool,
        pointer: String,
    },
}

impl BlockSpec {
    fn from_frame(value: &Value, pointer: &str) -> Result<Self> {
        let object = value
            .as_object()
            .ok_or_else(|| frame(pointer, "a block is an object"))?;
        let leaf = name_of(object, &["name"], pointer)?.to_owned();
        Self::from_object(&leaf, object, pointer)
    }

    fn from_patch(leaf: &str, value: &Value, pointer: &str) -> Result<Self> {
        if !is_identifier(leaf) {
            return Err(frame(
                pointer,
                format!("`{leaf}` is not a block name ({NAME_GRAMMAR})"),
            ));
        }
        let object = value
            .as_object()
            .ok_or_else(|| frame(pointer, "a block is an object"))?;
        Self::from_object(leaf, object, pointer)
    }

    fn from_object(
        leaf: &str,
        object: &serde_json::Map<String, Value>,
        pointer: &str,
    ) -> Result<Self> {
        for key in object.keys() {
            if !matches!(
                key.as_str(),
                "name" | "params" | "ops" | "term" | "unreachable" | "comment"
            ) {
                return Err(frame(&format!("{pointer}/{key}"), "unknown block key"));
            }
        }
        let params =
            match object.get("params") {
                Some(value) => {
                    array(value, &format!("{pointer}/params"))?
                        .iter()
                        .enumerate()
                        .map(|(index, param)| {
                            let param_pointer = format!("{pointer}/params/{index}");
                            let pair = param.as_array().filter(|pair| pair.len() == 2).ok_or_else(
                                || frame(&param_pointer, "a block parameter is [\"name\", type]"),
                            )?;
                            let name = string(&pair[0], &param_pointer)?;
                            if !is_identifier(name) {
                                return Err(frame(
                                    &param_pointer,
                                    format!("`{name}` is not a name ({NAME_GRAMMAR})"),
                                ));
                            }
                            Ok((name.to_owned(), pair[1].clone()))
                        })
                        .collect::<Result<_>>()?
                }
                None => Vec::new(),
            };
        let ops = match object.get("ops") {
            Some(value) => array(value, &format!("{pointer}/ops"))?.clone(),
            None => Vec::new(),
        };
        let term = object
            .get("term")
            .cloned()
            .ok_or_else(|| frame(pointer, "missing \"term\""))?;
        Ok(Self::Frame {
            leaf: leaf.to_owned(),
            params,
            ops,
            term,
            unreachable: object
                .get("unreachable")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            pointer: pointer.to_owned(),
        })
    }
}

/// A parsed operation statement.
struct OpSpec {
    opcode: &'static OpcodeRow,
    args: Vec<Value>,
    ty: Option<Value>,
}

/// Reads `["name", "opcode", args...]` or `{"name", "op", "args", "type"}`.
fn op_head(value: &Value, pointer: &str) -> Result<(String, OpSpec)> {
    let (name, opcode, args, ty) = match value {
        Value::Array(items) if items.len() >= 2 => (
            string(&items[0], pointer)?,
            string(&items[1], pointer)?,
            items[2..].to_vec(),
            None,
        ),
        Value::Object(object) => (
            name_of(object, &["name"], pointer)?,
            string(
                object
                    .get("op")
                    .or_else(|| object.get("opcode"))
                    .ok_or_else(|| frame(pointer, "missing \"op\""))?,
                pointer,
            )?,
            object
                .get("args")
                .or_else(|| object.get("operands"))
                .map_or(Ok(Vec::new()), |args| array(args, pointer).cloned())?,
            object.get("type").cloned(),
        ),
        _ => {
            return Err(frame(
                pointer,
                "an operation is [\"name\", \"opcode\", args...]",
            ));
        }
    };
    if !is_identifier(name) {
        return Err(frame(
            pointer,
            format!("`{name}` is not a value name ({NAME_GRAMMAR})"),
        ));
    }
    let opcode = opcodes::by_word(opcode).ok_or_else(|| {
        if matches!(opcode, "return" | "br" | "jump" | "cond" | "switch" | "trap") {
            frame(
                pointer,
                format!("`{opcode}` is a terminator, not an operation: it goes in the block's \"term\", as [\"{opcode}\", ...]"),
            )
        } else {
            frame(
                pointer,
                format!("unknown opcode `{opcode}` (see `sley-agent help opcodes`)"),
            )
        }
    })?;
    Ok((name.to_owned(), OpSpec { opcode, args, ty }))
}

/// One built operation: opcode tag, operands, result type, immediate.
type BuiltOp = (u32, Vec<ValueRef>, TypeExpr, Immediate);

struct PlannedOp1 {
    leaf: String,
    id: EntityId,
    spec: Option<OpSpec>,
    pointer: String,
}

struct PlannedBlock {
    leaf: String,
    id: EntityId,
    params: Vec<(String, EntityId, TypeExpr)>,
    ops: Vec<PlannedOp1>,
    term: Option<Value>,
    unreachable: bool,
    pointer: String,
    keep: bool,
}

/// The terminators of frame blocks whose word or item count no terminator
/// form accepts, as frame problems at their pointers.
fn malformed_terminators(blocks: &[PlannedBlock]) -> Vec<AgentError> {
    let mut problems = Vec::new();
    for block in blocks {
        let Some(term) = &block.term else { continue };
        let pointer = format!("{}/term", block.pointer);
        let Some(items) = term.as_array() else {
            problems.push(frame(&pointer, "expected an array"));
            continue;
        };
        let Some(word) = items.first().and_then(Value::as_str) else {
            problems.push(frame(&pointer, "a terminator starts with its word"));
            continue;
        };
        let shaped = match word {
            "return" => items.len() == 2,
            "br" | "jump" | "switch" => items.len() >= 2 + usize::from(word == "switch"),
            "cond" => items.len() == 4,
            "trap" => true,
            _ => false,
        };
        if !shaped {
            problems.push(frame(&pointer, terminator_shape(word, items)));
        }
    }
    problems
}

/// Why a terminator with a known or unknown word has the wrong shape, with
/// the fix.
fn terminator_shape(word: &str, items: &[Value]) -> String {
    let count = items.len();
    match word {
        "return" => format!("`return` takes one value: [\"return\", v], not {count} items"),
        "br" | "jump" => {
            "`br` names its target: [\"br\", \"b\", args...] or [\"br\", [\"b\", args...]]"
                .to_owned()
        }
        "cond" => {
            let mut fix = String::new();
            if count > 4 && items[2].is_string() && items[3].is_string() {
                let mut target = vec![items[3].clone()];
                target.extend(items[4..].iter().cloned());
                let suggestion = Value::Array(vec![
                    items[0].clone(),
                    items[1].clone(),
                    items[2].clone(),
                    Value::Array(target),
                ]);
                fix = format!(", e.g. {suggestion}");
            }
            format!(
                "`cond` takes [\"cond\", c, then, else], not {count} items; a target with arguments is bracketed{fix}"
            )
        }
        "switch" => {
            "`switch` takes a value and its cases: [\"switch\", v, [key, block, args...], ...]"
                .to_owned()
        }
        other => format!("bad terminator `{other}`: use return, br, cond, switch, or trap"),
    }
}

/// The problem lines of a refusal: one line, or a combined refusal's
/// headline problem and its indented continuation lines.
pub(crate) fn problem_lines(error: &AgentError) -> Vec<String> {
    let mut lines: Vec<String> = error
        .detail()
        .lines()
        .map(|line| line.trim().to_owned())
        .collect();
    if let Some(first) = lines.first_mut()
        && let Some(cut) = first.rfind(" (1 of ")
        && first.ends_with(" problems)")
    {
        first.truncate(cut);
    }
    lines.retain(|line| !line.is_empty());
    lines
}

/// One refusal for several frame problems. The headline is the first
/// problem, pointer included, so the refusal line itself says where; the
/// others follow one per line.
pub(crate) fn combined(mut errors: Vec<AgentError>) -> AgentError {
    if errors.len() == 1 {
        return errors.remove(0);
    }
    let mut lines: Vec<String> = Vec::new();
    for error in &errors {
        for line in problem_lines(error) {
            if !lines.contains(&line) {
                lines.push(line);
            }
        }
    }
    match lines.len() {
        0 => AgentError::new(AgentErrorCode::FrameInvalid, "the frame is invalid"),
        1 => AgentError::new(AgentErrorCode::FrameInvalid, lines.remove(0)),
        count => {
            let first = lines.remove(0);
            AgentError::new(
                AgentErrorCode::FrameInvalid,
                format!("{first} (1 of {count} problems)\n  {}", lines.join("\n  ")),
            )
        }
    }
}

/// Why a plain name resolves nowhere in `block`, with the fix when the
/// name lives in another block.
fn unresolved(text: &str, base: &str, block: usize, scope: &FunctionScope) -> String {
    let owner = scope
        .blocks
        .iter()
        .filter(|(_, index)| **index != block)
        .find_map(|(name, index)| {
            scope.block_values[*index]
                .get(base)
                .map(|slot| (name, slot))
        });
    match owner {
        Some((
            name,
            ValueSlot {
                reference: ValueRef::Parameter(_),
                ..
            },
        )) => format!(
            "`{text}` is a parameter of block `{name}`; block parameters are visible only in their own block, so add a parameter to this block and pass `{base}` to it as an edge argument"
        ),
        Some((name, _)) => format!(
            "`{text}` is a result of block `{name}`; write `{name}.{base}` (a result is visible in the blocks its block dominates)"
        ),
        None => format!("no value `{text}` in scope"),
    }
}

/// Why a switch case key is not a case of the scrutinee's type.
pub(crate) fn not_a_case(key: &str, scrutinee: Option<&TypeExpr>) -> String {
    match scrutinee {
        Some(TypeExpr::Result { .. }) => format!(
            "`{key}` is not a case of a Result; a Result switch lists [\"Ok\", block, args...] and [\"Err\", block, args...], and `$` passes the payload"
        ),
        Some(TypeExpr::Option(_)) => format!(
            "`{key}` is not a case of an Option; an Option switch lists [\"Some\", block, args...] and [\"None\", block, args...], and `$` passes the payload"
        ),
        _ => format!(
            "`{key}` needs a variant scrutinee; a case is [key, block, args...] with key Ok/Err (Result), Some/None (Option) or a case of the variant type"
        ),
    }
}

#[derive(Default)]
struct FunctionScope {
    function_values: BTreeMap<String, ValueSlot>,
    blocks: BTreeMap<String, usize>,
    block_values: Vec<BTreeMap<String, ValueSlot>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_clamp_to_the_grant() {
        let ceilings = sley_policy::PolicyResourceCeilings::new(1_000, 1_000, 1_000, 100, 100, 100);
        let limits = default_test_limits(&ceilings);
        assert_eq!(limits.fuel, 1_000);
        assert_eq!(limits.memory_bytes, 1_000);
        assert_eq!(limits.output_bytes, 1_000);
        assert_eq!(limits.effect_count, 0);
    }
}
