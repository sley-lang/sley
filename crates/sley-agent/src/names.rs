//! Local names: short, deterministic, non-canonical handles for entities.
//!
//! Names never carry identity and never enter canonical bytes. Each entity
//! is named, in order of preference, by its object label, by the workspace
//! name map (`names.json` from the starter, `.sley/names.json` written by
//! AF1 compilation), or positionally. Function-scoped entities are
//! qualified by their owners: `clamp.value` (parameter), `clamp.entry`
//! (block), `clamp.entry.inverted` (operation), `clamp.ok.v` (block
//! parameter). Variant cases and record fields are `Type.Member`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sley_id::EntityId;
use sley_mutate::value::EntityBodyValue;
use sley_ssmc::{MemberId, TypeDefForm, ValueRef};

use crate::error::{AgentError, AgentErrorCode, Result, io};
use crate::hex;
use crate::workspace::Program;

/// Where a function-scoped entity lives.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scope {
    /// A top-level entity.
    Top,
    /// A function parameter or block (owner: the function).
    Function(EntityId),
    /// A block parameter or operation (owner: the block).
    Block(EntityId),
}

/// A name map: identity bytes (entity or member) to preferred leaf name.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NameMap {
    entries: BTreeMap<[u8; 32], String>,
}

impl NameMap {
    /// Reads a `{"<64-hex>": "name"}` map; an absent file is empty.
    ///
    /// # Errors
    ///
    /// `AGENT_WORKSPACE_INVALID` for a malformed map, `AGENT_IO_FAILED` for
    /// an unreadable one.
    pub fn read(path: &Path) -> Result<Self> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(io(path, &error)),
        };
        let invalid = |detail: &str| {
            AgentError::new(
                AgentErrorCode::WorkspaceInvalid,
                format!("{}: {detail}", path.display()),
            )
        };
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|error| invalid(&error.to_string()))?;
        let object = value
            .as_object()
            .ok_or_else(|| invalid("a name map is one JSON object"))?;
        let mut map = Self::default();
        for (key, name) in object {
            let id = hex::decode32(key).ok_or_else(|| invalid("keys are 64 lowercase hex"))?;
            let name = name.as_str().ok_or_else(|| invalid("values are strings"))?;
            if is_identifier(name) {
                map.entries.insert(id, name.to_owned());
            }
        }
        Ok(map)
    }

    /// Writes the map as sorted JSON.
    ///
    /// # Errors
    ///
    /// `AGENT_IO_FAILED` when the file cannot be written.
    pub fn write(&self, path: &Path) -> Result<()> {
        let object: serde_json::Map<String, serde_json::Value> = self
            .entries
            .iter()
            .map(|(id, name)| (hex::encode(id), serde_json::Value::from(name.as_str())))
            .collect();
        let mut text =
            serde_json::to_string_pretty(&serde_json::Value::Object(object)).unwrap_or_default();
        text.push('\n');
        crate::candidate::replace_file(path, text.as_bytes())
    }

    /// Adds or replaces one preferred name.
    pub fn insert(&mut self, id: [u8; 32], name: impl Into<String>) {
        self.entries.insert(id, name.into());
    }

    /// Merges `other` over `self`.
    pub fn extend(&mut self, other: &Self) {
        for (id, name) in &other.entries {
            self.entries.insert(*id, name.clone());
        }
    }

    /// Returns the preferred name for identity bytes.
    #[must_use]
    pub fn get(&self, id: &[u8; 32]) -> Option<&str> {
        self.entries.get(id).map(String::as_str)
    }

    /// Returns whether the map is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The leaf-name grammar, as refusals state it.
pub const NAME_GRAMMAR: &str = "[A-Za-z_][A-Za-z0-9_-]*, at most 64 bytes";

/// Whether `text` is a usable leaf name (`NAME_GRAMMAR`). A hyphen is
/// allowed after the first character: `.`, `#` and `$` are the only
/// characters with a meaning inside a name reference.
#[must_use]
pub fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && text.len() <= 64
}

/// Short kind prefix for positional top-level names.
#[must_use]
pub const fn kind_prefix(kind: u16) -> &'static str {
    match kind {
        1 => "ws",
        2 => "pkg",
        3 => "ns",
        4 => "type",
        5 => "fn",
        6 => "param",
        7 => "blk",
        8 => "op",
        9 => "const",
        10 => "global",
        11 => "effect",
        12 => "capreq",
        13 => "contract",
        14 => "test",
        15 => "adapter",
        16 => "entry",
        17 => "policy",
        18 => "dep",
        _ => "entity",
    }
}

/// The SSMC1 name of an entity kind.
#[must_use]
pub const fn kind_name(kind: u16) -> &'static str {
    match kind {
        1 => "Workspace",
        2 => "Package",
        3 => "Namespace",
        4 => "TypeDef",
        5 => "Function",
        6 => "Parameter",
        7 => "Block",
        8 => "Operation",
        9 => "Constant",
        10 => "GlobalValue",
        11 => "EffectDef",
        12 => "CapabilityRequirement",
        13 => "Contract",
        14 => "TestCase",
        15 => "AdapterImport",
        16 => "EntryPoint",
        17 => "PolicyBinding",
        18 => "DependencyBinding",
        _ => "Entity",
    }
}

/// One named entity: its qualified name, leaf name, and scope (always set
/// together).
#[derive(Clone, Debug)]
struct Named {
    qualified: String,
    leaf: String,
    scope: Scope,
}

/// The resolved naming of one program state.
#[derive(Clone, Debug, Default)]
pub struct Names {
    named: BTreeMap<EntityId, Named>,
    by_name: BTreeMap<String, EntityId>,
    members: BTreeMap<(EntityId, MemberId), String>,
    member_by_name: BTreeMap<String, (EntityId, MemberId)>,
}

impl Names {
    /// Names every entity in `program` using `map` for preferences.
    #[must_use]
    pub fn build(program: &Program, map: &NameMap) -> Self {
        let mut names = Self::default();
        let preferred = |object: &sley_mutate::EntityObject| -> Option<String> {
            let record = object.record();
            record
                .label
                .as_deref()
                .filter(|label| is_identifier(label))
                .map(str::to_owned)
                .or_else(|| map.get(record.entity_id.as_bytes()).map(str::to_owned))
        };
        // Top-level entities first, in id order.
        let mut taken: BTreeSet<String> = BTreeSet::new();
        for object in program.objects() {
            let record = object.record();
            let kind = record.body.kind_tag();
            if matches!(kind, 6..=8) {
                continue;
            }
            let base = preferred(object).unwrap_or_else(|| {
                format!(
                    "{}_{}",
                    kind_prefix(kind),
                    hex::short(record.entity_id.as_bytes())
                )
            });
            let name = unique(&base, record.entity_id.as_bytes(), &taken);
            taken.insert(name.clone());
            names.set(record.entity_id, name.clone(), name, Scope::Top);
        }
        // Function-owned entities, reached through the function lists.
        for object in program.objects() {
            let record = object.record();
            if let EntityBodyValue::Function(function) = &record.body {
                names.name_function(program, map, record.entity_id, function);
            }
        }
        // Anything still unnamed is an orphan: name it through its owner,
        // blocks before the parameters and operations they may own.
        for kind in [7, 6, 8] {
            for object in program.objects() {
                let record = object.record();
                if record.body.kind_tag() != kind || names.named.contains_key(&record.entity_id) {
                    continue;
                }
                names.name_orphan(program, map, record.entity_id);
            }
        }
        // Record fields and variant cases.
        for object in program.objects() {
            let record = object.record();
            if let EntityBodyValue::TypeDef(typedef) = &record.body {
                let members: Vec<MemberId> = match &typedef.form {
                    TypeDefForm::Record(fields) => fields.iter().map(|f| f.member_id).collect(),
                    TypeDefForm::Variant(cases) => cases.iter().map(|c| c.member_id).collect(),
                };
                let type_name = names.name(&record.entity_id);
                let mut used = BTreeSet::new();
                for (position, member) in members.iter().enumerate() {
                    let base = map
                        .get(member.as_bytes())
                        .map_or_else(|| format!("m{position}"), str::to_owned);
                    let leaf = unique(&base, member.as_bytes(), &used);
                    used.insert(leaf.clone());
                    names
                        .member_by_name
                        .insert(format!("{type_name}.{leaf}"), (record.entity_id, *member));
                    names.members.insert((record.entity_id, *member), leaf);
                }
            }
        }
        names
    }

    fn set(&mut self, id: EntityId, qualified: String, leaf: String, scope: Scope) {
        self.by_name.insert(qualified.clone(), id);
        self.named.insert(
            id,
            Named {
                qualified,
                leaf,
                scope,
            },
        );
    }

    fn name_function(
        &mut self,
        program: &Program,
        map: &NameMap,
        function_id: EntityId,
        function: &sley_mutate::value::FunctionBody,
    ) {
        let function_name = self.name(&function_id);
        let pick = |id: &EntityId, fallback: &dyn Fn() -> String| -> String {
            program
                .object(id)
                .and_then(|object| object.record().label.as_deref())
                .filter(|label| is_identifier(label))
                .or_else(|| map.get(id.as_bytes()))
                .map_or_else(fallback, str::to_owned)
        };
        let mut params = BTreeSet::new();
        for (position, param) in function.parameters.iter().enumerate() {
            if self.named.contains_key(param) || !program.contains(param) {
                continue;
            }
            let leaf = unique_among(
                pick(param, &|| format!("p{position}")),
                param.as_bytes(),
                |name| params.contains(name),
            );
            params.insert(leaf.clone());
            self.set(
                *param,
                format!("{function_name}.{leaf}"),
                leaf,
                Scope::Function(function_id),
            );
        }
        let mut blocks = BTreeSet::new();
        for (position, block) in function.blocks.iter().enumerate() {
            if self.named.contains_key(block) || !program.contains(block) {
                continue;
            }
            let fallback = || {
                if *block == function.entry_block {
                    "entry".to_owned()
                } else {
                    format!("b{position}")
                }
            };
            // Blocks share the function's scope with its parameters (`f.x`).
            let leaf = unique_among(pick(block, &fallback), block.as_bytes(), |name| {
                blocks.contains(name) || params.contains(name)
            });
            blocks.insert(leaf.clone());
            let block_name = format!("{function_name}.{leaf}");
            self.set(
                *block,
                block_name.clone(),
                leaf,
                Scope::Function(function_id),
            );
            if let Some(EntityBodyValue::Block(body)) = program.body(block) {
                // Block-local values share one namespace, and never shadow a
                // function parameter (the renderer prints both unqualified).
                let mut values = BTreeSet::new();
                let taken = |values: &BTreeSet<String>, name: &str| {
                    values.contains(name) || params.contains(name)
                };
                for (position, param) in body.parameters.iter().enumerate() {
                    if self.named.contains_key(param) || !program.contains(param) {
                        continue;
                    }
                    let leaf = unique_among(
                        pick(param, &|| format!("v{position}")),
                        param.as_bytes(),
                        |name| taken(&values, name),
                    );
                    values.insert(leaf.clone());
                    self.set(
                        *param,
                        format!("{block_name}.{leaf}"),
                        leaf,
                        Scope::Block(*block),
                    );
                }
                for (position, operation) in body.operations.iter().enumerate() {
                    if self.named.contains_key(operation) || !program.contains(operation) {
                        continue;
                    }
                    let leaf = unique_among(
                        pick(operation, &|| format!("op{position}")),
                        operation.as_bytes(),
                        |name| taken(&values, name),
                    );
                    values.insert(leaf.clone());
                    self.set(
                        *operation,
                        format!("{block_name}.{leaf}"),
                        leaf,
                        Scope::Block(*block),
                    );
                }
            }
        }
    }

    fn name_orphan(&mut self, program: &Program, map: &NameMap, id: EntityId) {
        let Some(body) = program.body(&id) else {
            return;
        };
        let kind = body.kind_tag();
        let (owner, scope) = match body {
            EntityBodyValue::Parameter(parameter) => (
                Some(parameter.owner),
                match parameter.role {
                    sley_ssmc::ParameterRole::Function => Scope::Function(parameter.owner),
                    sley_ssmc::ParameterRole::Block => Scope::Block(parameter.owner),
                },
            ),
            EntityBodyValue::Block(block) => {
                (Some(block.function), Scope::Function(block.function))
            }
            EntityBodyValue::Operation(operation) => {
                (Some(operation.block), Scope::Block(operation.block))
            }
            _ => (None, Scope::Top),
        };
        let leaf = program
            .object(&id)
            .and_then(|object| object.record().label.clone())
            .filter(|label| is_identifier(label))
            .or_else(|| map.get(id.as_bytes()).map(str::to_owned))
            .unwrap_or_else(|| format!("{}_{}", kind_prefix(kind), hex::short(id.as_bytes())));
        let prefix: Option<String> = owner
            .filter(|owner: &EntityId| self.named.contains_key(owner))
            .map(|owner| self.name(&owner));
        let mut qualified =
            prefix.map_or_else(|| leaf.clone(), |prefix| format!("{prefix}.{leaf}"));
        if self.by_name.contains_key(&qualified) {
            qualified = format!("{qualified}_{}", hex::encode(&id.as_bytes()[..2]));
        }
        let scope = if prefix_known(&scope, &self.named) {
            scope
        } else {
            Scope::Top
        };
        self.set(id, qualified, leaf, scope);
    }

    /// The qualified name of an entity (a positional name when unnamed).
    #[must_use]
    pub fn name(&self, id: &EntityId) -> String {
        self.named.get(id).map_or_else(
            || format!("#{}", hex::short(id.as_bytes())),
            |named| named.qualified.clone(),
        )
    }

    /// The leaf (unqualified) name of an entity.
    #[must_use]
    pub fn leaf(&self, id: &EntityId) -> String {
        self.named.get(id).map_or_else(
            || format!("#{}", hex::short(id.as_bytes())),
            |named| named.leaf.clone(),
        )
    }

    /// The scope an entity was named in.
    #[must_use]
    pub fn scope(&self, id: &EntityId) -> Scope {
        self.named.get(id).map_or(Scope::Top, |named| named.scope)
    }

    /// Renders a member as `Type.Member`.
    #[must_use]
    pub fn member(&self, definition: &EntityId, member: &MemberId) -> String {
        self.members.get(&(*definition, *member)).map_or_else(
            || {
                format!(
                    "{}.#{}",
                    self.name(definition),
                    hex::short(member.as_bytes())
                )
            },
            |leaf| format!("{}.{leaf}", self.name(definition)),
        )
    }

    /// Renders a member's leaf name.
    #[must_use]
    pub fn member_leaf(&self, definition: &EntityId, member: &MemberId) -> String {
        self.members
            .get(&(*definition, *member))
            .cloned()
            .unwrap_or_else(|| format!("#{}", hex::short(member.as_bytes())))
    }

    /// Renders a member whose definition is not at hand (`record_get`
    /// immediates, switch keys) by searching every definition.
    #[must_use]
    pub fn member_any(&self, member: &MemberId) -> String {
        self.members
            .iter()
            .find(|((_, candidate), _)| candidate == member)
            .map_or_else(
                || format!("#{}", hex::short(member.as_bytes())),
                |((definition, _), leaf)| format!("{}.{leaf}", self.name(definition)),
            )
    }

    /// Resolves `Type.Member`.
    #[must_use]
    pub fn resolve_member(&self, name: &str) -> Option<(EntityId, MemberId)> {
        self.member_by_name.get(name).copied()
    }

    /// Resolves a member of a known definition by leaf name.
    #[must_use]
    pub fn resolve_member_leaf(&self, definition: &EntityId, leaf: &str) -> Option<MemberId> {
        self.members
            .iter()
            .find(|((owner, _), name)| owner == definition && name.as_str() == leaf)
            .map(|((_, member), _)| *member)
    }

    /// Resolves a qualified name, a `#`-prefixed id prefix (eight or more
    /// hex digits), or a full 64-hex id.
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<EntityId> {
        if let Some(id) = self.by_name.get(name) {
            return Some(*id);
        }
        if let Some(bytes) = hex::decode32(name) {
            let id = EntityId::from_bytes(bytes);
            return self.named.contains_key(&id).then_some(id);
        }
        let prefix = name.strip_prefix('#')?;
        if prefix.len() < 8 {
            return None;
        }
        let mut found = self
            .named
            .keys()
            .filter(|id| hex::encode(id.as_bytes()).starts_with(prefix));
        let first = found.next()?;
        found.next().is_none().then_some(*first)
    }

    /// Every named entity with its qualified name, in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &EntityId)> {
        self.by_name.iter()
    }

    /// Renders a value reference as seen from `current` (a block): the leaf
    /// for function parameters and values of the same block, `block.leaf`
    /// otherwise; `#n` selects a result index other than zero.
    #[must_use]
    pub fn value(&self, value: &ValueRef, current: Option<&EntityId>) -> String {
        let (id, index) = match value {
            ValueRef::Parameter(id) => (*id, 0),
            ValueRef::OperationResult(result) => (result.operation, result.result_index),
        };
        let mut text = match self.scope(&id) {
            Scope::Block(block) if Some(&block) != current => {
                format!("{}.{}", self.leaf(&block), self.leaf(&id))
            }
            Scope::Top if !self.named.contains_key(&id) => {
                format!("#{}", hex::short(id.as_bytes()))
            }
            _ => self.leaf(&id),
        };
        if index != 0 {
            let _ = write!(text, "#{index}");
        }
        text
    }
}

fn prefix_known(scope: &Scope, named: &BTreeMap<EntityId, Named>) -> bool {
    match scope {
        Scope::Top => true,
        Scope::Function(owner) | Scope::Block(owner) => named.contains_key(owner),
    }
}

fn unique(base: &str, id: &[u8; 32], taken: &BTreeSet<String>) -> String {
    unique_among(base.to_owned(), id, |name| taken.contains(name))
}

/// `unique` over any taken-name predicate, keeping `base` when it is free.
fn unique_among(base: String, id: &[u8; 32], taken: impl Fn(&str) -> bool) -> String {
    if !taken(&base) {
        return base;
    }
    for width in [2, 4, 32] {
        let candidate = format!("{base}_{}", hex::encode(&id[..width]));
        if !taken(&candidate) {
            return candidate;
        }
    }
    format!("{base}_{}", hex::encode(id))
}
