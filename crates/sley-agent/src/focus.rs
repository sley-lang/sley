//! Focused views (`view --focus <name>`): one entity and its directly
//! relevant context, bounded in size.
//!
//! The target renders as `view <name>` renders it (AV1, or AV1-X with
//! `--x`). Then, for a function: `types:` the type definitions its
//! signature and body name, `consts:` the constants and globals it reads,
//! `calls:` the signatures of the functions it calls, `callers:` the
//! functions that call it, `tests:` the `TestCases` that target it, and
//! `boundary:` its visibility, effects, namespaces, whether other functions
//! call it, and entry points. Types, constants and globals list `used by:`
//! instead of calls and callers.
//!
//! Output stays within `FOCUS_BOUND` bytes: rendered context is omitted
//! from the end (calls, then consts, then types), then the target's body;
//! name lists shrink to their counts last. Every omission prints its count
//! and the exact command that shows what it left out. A focused view is
//! context for reading, not a completeness certificate.

use std::fmt::Write as _;

use serde_json::{Value, json};
use sley_id::EntityId;
use sley_mutate::value::{EntityBodyValue, FunctionBody};
use sley_ssmc::{Immediate, TypeDefForm, TypeExpr, Visibility};

use crate::names::{Names, kind_prefix};
use crate::view::{self, ViewOptions};
use crate::workspace::Program;
use crate::xview;

/// The byte bound of a focused view's text.
pub const FOCUS_BOUND: usize = 4000;

/// How a focused view renders, and how its expansion commands read.
#[derive(Clone, Debug)]
pub struct FocusRequest {
    /// The view header line (with its newline).
    pub header: String,
    /// Rendering switches.
    pub options: ViewOptions,
    /// Render functions in AV1-X.
    pub x: bool,
    /// The `--after` reference as commands repeat it.
    pub after: Option<String>,
}

impl FocusRequest {
    /// `sley-agent view` with this request's flags.
    fn view_command(&self) -> String {
        let mut command = String::from("sley-agent view");
        for (flag, set) in [
            ("--x", self.x),
            ("--ids", self.options.ids),
            ("--types", self.options.types),
            ("--limits", self.options.limits),
        ] {
            if set {
                let _ = write!(command, " {flag}");
            }
        }
        if let Some(after) = &self.after {
            let _ = write!(command, " --after {after}");
        }
        command
    }

    /// The command that lists every name of the focused view in JSON.
    fn json_command(&self, target: &str) -> String {
        let mut command = format!("sley-agent --json view --focus {target}");
        if let Some(after) = &self.after {
            let _ = write!(command, " --after {after}");
        }
        command
    }
}

/// A rendered focused view.
#[derive(Clone, Debug)]
pub struct Focused {
    /// The bounded text (header included).
    pub text: String,
    /// The structured lists (complete, never bounded) and what the text
    /// omitted.
    pub json: Value,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Style {
    /// Entities rendered one or more lines each.
    Rendered,
    /// Names on one line.
    Names,
}

struct Section {
    key: &'static str,
    title: &'static str,
    style: Style,
    entries: Vec<(String, String)>,
    kept: usize,
    compact: bool,
}

impl Section {
    fn new(key: &'static str, title: &'static str, style: Style) -> Self {
        Self {
            key,
            title,
            style,
            entries: Vec::new(),
            kept: 0,
            compact: false,
        }
    }

    fn names(&self) -> Vec<&str> {
        self.entries.iter().map(|(name, _)| name.as_str()).collect()
    }
}

/// Renders the focused view of `target` (a function-scoped name focuses on
/// its function).
#[must_use]
pub fn render(
    program: &Program,
    names: &Names,
    target: &EntityId,
    request: &FocusRequest,
) -> Focused {
    let top = view::owner_function(program, target).unwrap_or(*target);
    let name = names.name(&top);
    let target_text = if request.x {
        xview::entity(program, names, &top, request.options)
    } else {
        view::entity(program, names, &top, request.options)
    };
    let body = program.body(&top);
    let context = Context {
        program,
        names,
        index: Index::of(program),
        options: request.options,
        top,
    };
    let namespaces = context.index.namespaces_of(&top);
    let mut boundary = Boundary {
        text: String::from("boundary:"),
        json: serde_json::Map::new(),
        namespace: list_or_none(names, &namespaces),
    };
    boundary
        .json
        .insert("namespaces".into(), json!(name_list(names, &namespaces)));
    let sections = match body {
        Some(EntityBodyValue::Function(function)) => context.function(function, &mut boundary),
        Some(EntityBodyValue::TestCase(test)) => {
            let mut calls = Section::new("calls", "calls", Style::Rendered);
            if program.contains(&test.target) {
                calls
                    .entries
                    .push((names.name(&test.target), context.signature(&test.target)));
            }
            let _ = write!(boundary.text, " namespace {}", boundary.namespace);
            vec![calls]
        }
        Some(other) => context.other(other, &mut boundary),
        None => {
            let _ = write!(boundary.text, " namespace {}", boundary.namespace);
            Vec::new()
        }
    };
    boundary.text.push('\n');
    let mut layout = Layout {
        request,
        name: &name,
        target_text: &target_text,
        collapsed: false,
        long_commands: true,
        sections,
        boundary: &boundary.text,
    };
    let (text, omitted) = layout.bounded();
    let mut focus = serde_json::Map::new();
    focus.insert("target".into(), json!(name));
    focus.insert(
        "kind".into(),
        json!(body.map_or("entity", |body| kind_prefix(body.kind_tag()))),
    );
    for section in &layout.sections {
        focus.insert(section.key.into(), json!(section.names()));
    }
    focus.insert("boundary".into(), Value::Object(boundary.json));
    focus.insert("omitted".into(), Value::Array(omitted));
    focus.insert("bytes".into(), json!(text.len()));
    focus.insert("bound".into(), json!(FOCUS_BOUND));
    Focused {
        text,
        json: Value::Object(focus),
    }
}

/// The `boundary:` line and its JSON form.
struct Boundary {
    text: String,
    json: serde_json::Map<String, Value>,
    namespace: String,
}

/// What a focused view reads its context from.
struct Context<'a> {
    program: &'a Program,
    names: &'a Names,
    index: Index,
    options: ViewOptions,
    top: EntityId,
}

impl Context<'_> {
    /// An entity's own AV1 rendering.
    fn full_view(&self, id: &EntityId) -> String {
        view::entity(self.program, self.names, id, self.options)
    }

    /// A function's signature line (other entities render whole).
    fn signature(&self, id: &EntityId) -> String {
        match self.program.body(id) {
            Some(EntityBodyValue::Function(function)) => {
                view::signature(self.program, self.names, id, function, self.options)
            }
            _ => self.full_view(id),
        }
    }

    fn rendered(
        &self,
        section: &mut Section,
        ids: &[EntityId],
        render: impl Fn(&EntityId) -> String,
    ) {
        section.entries = ids
            .iter()
            .map(|id| (self.names.name(id), render(id)))
            .collect();
    }

    fn sorted(&self, section: &mut Section, ids: &[EntityId]) {
        section.entries = ids
            .iter()
            .map(|id| (self.names.name(id), self.names.name(id)))
            .collect();
        section.entries.sort();
    }

    /// A function's context: types, consts, calls, callers, tests.
    fn function(&self, function: &FunctionBody, boundary: &mut Boundary) -> Vec<Section> {
        let names = self.names;
        let refs = Refs::of(self.program, &self.index, &self.top, function);
        let callers = self.index.callers_of(&self.top);
        let tests = self.index.tests_of(&self.top);
        let entrypoints = self.index.entrypoints_of(&self.top);
        let mut types = Section::new("types", "types", Style::Rendered);
        self.rendered(&mut types, &refs.types, |id| self.full_view(id));
        let mut consts = Section::new("consts", "consts", Style::Rendered);
        self.rendered(&mut consts, &refs.reads, |id| self.full_view(id));
        let mut calls = Section::new("calls", "calls", Style::Rendered);
        self.rendered(&mut calls, &refs.calls, |id| self.signature(id));
        let mut caller_list = Section::new("callers", "callers", Style::Names);
        self.sorted(&mut caller_list, &callers);
        let mut test_list = Section::new("tests", "tests", Style::Names);
        self.sorted(&mut test_list, &tests);
        let effects = function.effects.as_slice();
        let visibility = view::visibility(function.visibility);
        let _ = write!(
            boundary.text,
            " {visibility}; effects {}; namespace {}; callers outside it: {}",
            list_or_none(names, effects),
            boundary.namespace,
            count_or_none(callers.len())
        );
        if !entrypoints.is_empty() {
            let _ = write!(
                boundary.text,
                "; entrypoint {}",
                list_or_none(names, &entrypoints)
            );
        }
        let json = &mut boundary.json;
        json.insert("visibility".into(), json!(visibility));
        json.insert(
            "exported".into(),
            json!(function.visibility == Visibility::Exported),
        );
        json.insert("effects".into(), json!(name_list(names, effects)));
        json.insert("callers_outside".into(), json!(callers.len()));
        json.insert("entrypoints".into(), json!(name_list(names, &entrypoints)));
        vec![types, consts, calls, caller_list, test_list]
    }

    /// A type's, constant's, global's or namespace's context: the types it
    /// names and what uses it.
    fn other(&self, body: &EntityBodyValue, boundary: &mut Boundary) -> Vec<Section> {
        let mut mentioned = Vec::new();
        let visibility = match body {
            EntityBodyValue::TypeDef(typedef) => {
                for ty in member_types(&typedef.form) {
                    named_types(ty, &mut mentioned);
                }
                Some(typedef.visibility)
            }
            EntityBodyValue::Constant(constant) => {
                named_types(&constant.value.value_type, &mut mentioned);
                None
            }
            EntityBodyValue::GlobalValue(global) => {
                named_types(&global.value_type, &mut mentioned);
                Some(global.visibility)
            }
            _ => None,
        };
        mentioned.retain(|id| *id != self.top && self.index.is_type(self.program, id));
        if let Some(visibility) = visibility {
            let _ = write!(boundary.text, " {};", view::visibility(visibility));
            boundary
                .json
                .insert("visibility".into(), json!(view::visibility(visibility)));
            boundary
                .json
                .insert("exported".into(), json!(visibility == Visibility::Exported));
        }
        let _ = write!(boundary.text, " namespace {}", boundary.namespace);
        if matches!(body, EntityBodyValue::Namespace(_)) {
            return Vec::new();
        }
        let users = self.index.users_of(self.program, &self.top);
        let _ = write!(
            boundary.text,
            "; used outside it: {}",
            count_or_none(users.len())
        );
        boundary
            .json
            .insert("used_outside".into(), json!(users.len()));
        let mut types = Section::new("types", "types", Style::Rendered);
        self.rendered(&mut types, &mentioned, |id| self.full_view(id));
        let mut used = Section::new("used_by", "used by", Style::Names);
        self.sorted(&mut used, &users);
        vec![types, used]
    }
}

/// The parts of a focused view and how much of each is shown.
struct Layout<'a> {
    request: &'a FocusRequest,
    name: &'a str,
    target_text: &'a str,
    collapsed: bool,
    long_commands: bool,
    sections: Vec<Section>,
    boundary: &'a str,
}

impl Layout<'_> {
    /// Fits the text to the bound. Parts go from the least relevant end:
    /// the test and caller name lists shrink to counts, then rendered
    /// context goes entity by entity (calls, consts, types, each from its
    /// end). When even that does not fit, the target's body is replaced by
    /// its signature and the context refills from the most relevant end;
    /// as a last resort, expansion commands point at the JSON name lists
    /// instead of listing names. The fixed lines always stay.
    fn bounded(&mut self) -> (String, Vec<Value>) {
        let configurations: &[(bool, bool)] = if self.target_text.lines().count() > 1 {
            &[(false, true), (true, true), (true, false)]
        } else {
            &[(false, true), (false, false)]
        };
        for &(collapsed, long_commands) in configurations {
            self.collapsed = collapsed;
            self.long_commands = long_commands;
            if self.fit() {
                break;
            }
        }
        self.compose()
    }

    fn fits(&self) -> bool {
        self.compose().0.len() <= FOCUS_BOUND
    }

    /// Shows everything, then omits from the least relevant end until the
    /// text fits; `false` when it never does.
    fn fit(&mut self) -> bool {
        for section in &mut self.sections {
            section.kept = section.entries.len();
            section.compact = false;
        }
        if self.fits() {
            return true;
        }
        for index in (0..self.sections.len()).rev() {
            if self.sections[index].style == Style::Names
                && !self.sections[index].entries.is_empty()
            {
                self.sections[index].compact = true;
                if self.fits() {
                    return true;
                }
            }
        }
        for index in (0..self.sections.len()).rev() {
            while self.sections[index].style == Style::Rendered && self.sections[index].kept > 0 {
                self.sections[index].kept -= 1;
                if self.fits() {
                    return true;
                }
            }
        }
        false
    }

    fn compose(&self) -> (String, Vec<Value>) {
        let mut text = self.request.header.clone();
        let mut omitted = Vec::new();
        let _ = writeln!(
            text,
            "# focus {}: context, not a completeness certificate",
            self.name
        );
        let view_command = self.request.view_command();
        let json_command = self.request.json_command(self.name);
        if self.collapsed {
            let mut lines = self.target_text.lines();
            let first = lines.next().unwrap_or_default();
            let rest = lines.count();
            let expand = format!("{view_command} {}", self.name);
            let _ = writeln!(text, "{first}\n# body omitted ({rest} lines): {expand}");
            omitted.push(
                json!({"section": "target", "count": rest, "names": [self.name], "expand": expand}),
            );
        } else {
            text.push_str(self.target_text);
        }
        for section in &self.sections {
            let total = section.entries.len();
            if total == 0 {
                let _ = writeln!(text, "{}: none", section.title);
                continue;
            }
            match section.style {
                Style::Rendered => {
                    let _ = writeln!(text, "{}: {total}", section.title);
                    for (_, entry) in &section.entries[..section.kept] {
                        text.push_str(entry);
                    }
                    let rest: Vec<&str> = section.entries[section.kept..]
                        .iter()
                        .map(|(name, _)| name.as_str())
                        .collect();
                    if !rest.is_empty() {
                        let expand = if self.long_commands {
                            format!("{view_command} {}", rest.join(" "))
                        } else {
                            json_command.clone()
                        };
                        let _ = writeln!(text, "# {} more {}: {expand}", rest.len(), section.title);
                        omitted.push(json!({"section": section.key, "count": rest.len(),
                            "names": rest, "expand": expand}));
                    }
                }
                Style::Names if section.compact => {
                    let _ = writeln!(text, "{}: {total} (names: {json_command})", section.title);
                    omitted.push(json!({"section": section.key, "count": total,
                        "names": section.names(), "expand": json_command}));
                }
                Style::Names => {
                    let _ = writeln!(
                        text,
                        "{}: {total}: {}",
                        section.title,
                        section.names().join(", ")
                    );
                }
            }
        }
        text.push_str(self.boundary);
        (text, omitted)
    }
}

/// Program-wide relations a focused view reads.
struct Index {
    functions: Vec<(EntityId, Vec<EntityId>, Vec<EntityId>)>,
    tests: Vec<(EntityId, EntityId)>,
    namespaces: Vec<(EntityId, Vec<EntityId>)>,
    entrypoints: Vec<(EntityId, EntityId)>,
    members: Vec<(sley_ssmc::MemberId, EntityId)>,
}

impl Index {
    fn of(program: &Program) -> Self {
        let mut index = Self {
            functions: Vec::new(),
            tests: Vec::new(),
            namespaces: Vec::new(),
            entrypoints: Vec::new(),
            members: Vec::new(),
        };
        for object in program.objects() {
            let record = object.record();
            let id = record.entity_id;
            match &record.body {
                EntityBodyValue::TestCase(test) => index.tests.push((id, test.target)),
                EntityBodyValue::Namespace(namespace) => {
                    index
                        .namespaces
                        .push((id, namespace.members.as_slice().to_vec()));
                }
                EntityBodyValue::EntryPoint(entry) => index.entrypoints.push((id, entry.function)),
                EntityBodyValue::TypeDef(typedef) => match &typedef.form {
                    TypeDefForm::Record(fields) => index
                        .members
                        .extend(fields.iter().map(|field| (field.member_id, id))),
                    TypeDefForm::Variant(cases) => index
                        .members
                        .extend(cases.iter().map(|case| (case.member_id, id))),
                },
                _ => {}
            }
        }
        for object in program.objects() {
            let record = object.record();
            if let EntityBodyValue::Function(function) = &record.body {
                let refs = Refs::of(program, &index, &record.entity_id, function);
                let mut named = refs.types;
                named.extend(refs.reads);
                index.functions.push((record.entity_id, refs.calls, named));
            }
        }
        index
    }

    fn is_type(&self, program: &Program, id: &EntityId) -> bool {
        let _ = self;
        matches!(program.body(id), Some(EntityBodyValue::TypeDef(_)))
    }

    fn callers_of(&self, target: &EntityId) -> Vec<EntityId> {
        self.functions
            .iter()
            .filter(|(id, calls, _)| id != target && calls.contains(target))
            .map(|(id, _, _)| *id)
            .collect()
    }

    fn tests_of(&self, target: &EntityId) -> Vec<EntityId> {
        self.tests
            .iter()
            .filter(|(_, of)| of == target)
            .map(|(id, _)| *id)
            .collect()
    }

    fn namespaces_of(&self, target: &EntityId) -> Vec<EntityId> {
        self.namespaces
            .iter()
            .filter(|(_, members)| members.contains(target))
            .map(|(id, _)| *id)
            .collect()
    }

    fn entrypoints_of(&self, target: &EntityId) -> Vec<EntityId> {
        self.entrypoints
            .iter()
            .filter(|(_, function)| function == target)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Functions, types, constants and globals that name `target` (a type,
    /// constant or global).
    fn users_of(&self, program: &Program, target: &EntityId) -> Vec<EntityId> {
        let mut users: Vec<EntityId> = self
            .functions
            .iter()
            .filter(|(_, _, named)| named.contains(target))
            .map(|(id, _, _)| *id)
            .collect();
        for object in program.objects() {
            let record = object.record();
            let mut mentioned = Vec::new();
            match &record.body {
                EntityBodyValue::TypeDef(typedef) => {
                    for ty in member_types(&typedef.form) {
                        named_types(ty, &mut mentioned);
                    }
                }
                EntityBodyValue::Constant(constant) => {
                    named_types(&constant.value.value_type, &mut mentioned);
                }
                EntityBodyValue::GlobalValue(global) => {
                    named_types(&global.value_type, &mut mentioned);
                    mentioned.push(global.initializer);
                }
                _ => continue,
            }
            if record.entity_id != *target && mentioned.contains(target) {
                users.push(record.entity_id);
            }
        }
        users
    }
}

/// What one function's signature and body name.
struct Refs {
    types: Vec<EntityId>,
    reads: Vec<EntityId>,
    calls: Vec<EntityId>,
}

impl Refs {
    fn of(program: &Program, index: &Index, id: &EntityId, function: &FunctionBody) -> Self {
        let mut types = Vec::new();
        let mut reads = Vec::new();
        let mut calls = Vec::new();
        let push = |list: &mut Vec<EntityId>, id: EntityId| {
            if !list.contains(&id) {
                list.push(id);
            }
        };
        let param_types = |params: &[EntityId], types: &mut Vec<EntityId>| {
            for param in params {
                if let Some(EntityBodyValue::Parameter(param)) = program.body(param) {
                    named_types(&param.value_type, types);
                }
            }
        };
        param_types(&function.parameters, &mut types);
        named_types(&function.result_type, &mut types);
        for block in &function.blocks {
            let Some(EntityBodyValue::Block(block)) = program.body(block) else {
                continue;
            };
            param_types(&block.parameters, &mut types);
            for operation in &block.operations {
                let Some(EntityBodyValue::Operation(operation)) = program.body(operation) else {
                    continue;
                };
                for ty in &operation.result_types {
                    named_types(ty, &mut types);
                }
                match &operation.immediate {
                    Immediate::Variant(variant) => push(&mut types, variant.definition),
                    Immediate::Field(member) => {
                        if let Some((_, definition)) = index
                            .members
                            .iter()
                            .find(|(candidate, _)| candidate == member)
                        {
                            push(&mut types, *definition);
                        }
                    }
                    Immediate::Entity(entity) => match program.body(entity) {
                        Some(EntityBodyValue::TypeDef(_)) => push(&mut types, *entity),
                        Some(EntityBodyValue::Constant(_) | EntityBodyValue::GlobalValue(_)) => {
                            push(&mut reads, *entity);
                        }
                        _ => {}
                    },
                    Immediate::Function(reference) => {
                        if reference.function != *id
                            && matches!(
                                program.body(&reference.function),
                                Some(EntityBodyValue::Function(_))
                            )
                        {
                            push(&mut calls, reference.function);
                        }
                    }
                    _ => {}
                }
            }
        }
        types.retain(|ty| matches!(program.body(ty), Some(EntityBodyValue::TypeDef(_))));
        Self {
            types,
            reads,
            calls,
        }
    }
}

/// The payload and field types of a type definition.
fn member_types(form: &TypeDefForm) -> Vec<&TypeExpr> {
    match form {
        TypeDefForm::Record(fields) => fields.iter().map(|field| &field.value_type).collect(),
        TypeDefForm::Variant(cases) => cases
            .iter()
            .filter_map(|case| case.payload_type.as_ref())
            .collect(),
    }
}

/// Every `TypeDef` a type expression names, in order of appearance.
fn named_types(ty: &TypeExpr, out: &mut Vec<EntityId>) {
    match ty {
        TypeExpr::Named(instance) => {
            if !out.contains(&instance.definition) {
                out.push(instance.definition);
            }
            for argument in &instance.arguments {
                named_types(argument, out);
            }
        }
        TypeExpr::Tuple(items) => items.iter().for_each(|item| named_types(item, out)),
        TypeExpr::Vector(item) | TypeExpr::Option(item) | TypeExpr::LocalCell(item) => {
            named_types(item, out);
        }
        TypeExpr::OrderedMap { key, value } => {
            named_types(key, out);
            named_types(value, out);
        }
        TypeExpr::Result { ok, error } => {
            named_types(ok, out);
            named_types(error, out);
        }
        TypeExpr::FunctionRef(function) => {
            function
                .parameters
                .iter()
                .for_each(|item| named_types(item, out));
            named_types(&function.result, out);
        }
        _ => {}
    }
}

fn name_list(names: &Names, ids: &[EntityId]) -> Vec<String> {
    ids.iter().map(|id| names.name(id)).collect()
}

fn list_or_none(names: &Names, ids: &[EntityId]) -> String {
    if ids.is_empty() {
        "none".into()
    } else {
        name_list(names, ids).join(", ")
    }
}

fn count_or_none(count: usize) -> String {
    if count == 0 {
        "none".into()
    } else {
        count.to_string()
    }
}
