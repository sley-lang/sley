//! Deterministic structural fragments, lowered exclusively to existing AF1-X.
//! The caller must bind state and pass the result through the ordinary compiler
//! and kernel. An expansion is never a validity or admission claim.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use super::frontier::{Budget, encoding};
use super::{FragmentRef, Operation, Request};
use crate::error::{AgentError, AgentErrorCode, Result};
use crate::names::is_identifier;

/// Maximum nested fragment applications in one construction.
pub const MAX_FRAGMENT_DEPTH: usize = 8;
/// Maximum guards or arithmetic steps in one fragment application.
pub const MAX_FRAGMENT_ITEMS: usize = 64;
/// Aggregate authored operations/guards/steps before AF1-X's existing ceilings.
pub const MAX_CONSTRUCTION_ITEMS: usize = 1024;

/// Resolve the closed integer pipeline vocabulary through AF1-X's opcode table.
/// Checked suffixes stay excluded: the fragment's explicit policy supplies them.
pub(super) fn pipeline_opcode(word: &str) -> Option<&'static crate::opcodes::OpcodeRow> {
    crate::opcodes::by_word(word).filter(|row| (64..=71).contains(&row.tag))
}

/// The shipped fragment contracts. Only implemented families appear here.
#[must_use]
pub fn manifest() -> Value {
    json!({
        "manifest": 1,
        "families": [
            {
                "id": "ordered_guard_chain", "version": 1,
                "target": "function", "parameters": ["guards", "success"],
                "guard": {"when": "AF1-X predicate", "fail": "explicit fail terminator"},
                "terminal_region": "Explicit return, ok, fail or trap; no authored block targets. Trap takes an optional code (default unreachable) and optional persistable payload. No extra items are ignored.",
                "policy": "Evaluate guards in written order; true selects that guard's failure. Otherwise enter success.",
                "effects": "No added effects; explicitly authored terminal regions use ordinary AF1-X checks.",
                "rule": "ordered-guard-blocks-v1"
            },
            {
                "id": "checked_pipeline", "version": 1,
                "target": "function", "parameters": ["steps", "arithmetic_failure", "rounding", "result"],
                "step": "[name, AF1-X arithmetic expression]",
                "opcode_spelling": "AF1-X mnemonic, SSMC1 name or decimal tag for integer opcodes 64–71; the fragment adds the explicit checked route",
                "rounding": ["toward_zero"],
                "arithmetic_failure": "Error variant case name, or {propagate:true} to preserve the native error payload",
                "policy": "Evaluate steps and nested operands in written order; every arithmetic failure takes the explicit route; wrap the final value with AF1-X ok.",
                "effects": "Arithmetic only; no calls, I/O, or implicit conversions.",
                "rule": "checked-steps-v1",
                "edit": {
                    "lens":"integer_literal",
                    "targets":"1..64 exact accepted or kernel-valid draft integer-load names feeding checked arithmetic",
                    "bindings":["lens","value","overrides?"],
                    "per_target_bindings":["lens","values"],
                    "values":"Alternative to value/overrides: one integer per exact scope name. Missing entries are separate author decisions; independence is not implied.",
                    "preserve":{"outside_scope":true,"boundaries":true},
                    "policy":"Inherit width and all unchanged fields. Replace only selected loads, with copy-on-write constants. Equal typed values change nothing. A new value affects every use of that load; unchanged caller bytes do not imply unchanged caller behavior."
                }
            },
            {
                "id": "typed_branch_result", "version": 1,
                "target": "function", "parameters": ["input", "cases", "join"],
                "case": {"case":"variant case name", "payload":"null or [name, AF1 type]",
                    "ops":"AF1-X operation list", "values":"ordered join arguments"},
                "join": {"params":"AF1 parameter pairs", "ops":"AF1-X operation list",
                    "term":"explicit AF1-X result terminator"},
                "policy": "Switch on input once; enter only its explicit case, bind its payload, evaluate its operations in order, and pass values to the shared join. No fallback or missing case is invented. Ordinary compiler and kernel checks enforce exhaustiveness and types.",
                "effects": "No added effects; authored case and join operations use ordinary AF1-X checks.",
                "rule": "typed-switch-join-v1"
            }
        ],
        "signature": {"params": "AF1 parameter pairs", "returns": "AF1 type"},
        "interface_preflight": {
            "rule":"declared-fragment-interfaces-v1",
            "context":"accepted types or the exact bound source draft declarations",
            "connections":"nested guards share outer inputs/result; pipeline requires Result<integer,error>; bare propagation preserves ArithmeticError; explicit Option/Result/named-variant cases must be exhaustive with matching payloads; guard failures and arithmetic mappings must match the return interface; scoped authored operation results, branch arguments and terminal payloads must match their known declared types",
            "evidence":"residual-interfaces.json; composition remains partial",
            "failure_routes":"Named checked and conditional exits target cases of the bound return error variant; no authored handler blocks are exposed. Missing cases refuse; incomplete error definitions remain unresolved.",
            "callee_effect_interfaces":"Unchanged callees retain accepted effect identities; AF1 definitions, patches and the residual target emit empty effect declarations. Body conformance remains an ordinary compiler/kernel obligation.",
            "deferred":["expression_types","ownership","effects","full_control_flow","kernel"]
        },
        "scope": "Derive: one exact function name. Edit: the selected lens's exact target names.",
        "choices": {"version":1,"contract":"author_closed_relation",
            "rows":"1..256 exact complete bindings for every missing semantic field, keyed by decision path",
            "policy":"The author declares the entire permitted relation. No sampled candidate family or test filtering establishes this contract. Questions use actual missing-field paths; omitted values follow from the exact declared rows and supplied answers.",
            "rule":"closed-author-relation-v1","cost_model":"disclosed-field-json-bytes-v1"},
        "factored_choices": {"version":2,"contract":"author_closed_constraints",
            "scope":"accepted or kernel-valid draft graph integer_literal per-target values only",
            "constraints":"1..256 complete finite tables with exact missing-field paths",
            "dependencies":"explicit additional coupling; mandatory typed graph edges are always retained",
            "rule":"closed-author-constraints-v1","cost_model":"disclosed-field-json-bytes-v1",
            "policy":"Conjoin all author tables, check every inherited width, retain graph coupling, and reconstruct without materializing independent products. No test filtering or task-correctness claim."},
        "limits": {
            "nesting": MAX_FRAGMENT_DEPTH, "items_per_application": MAX_FRAGMENT_ITEMS,
            "aggregate_items": MAX_CONSTRUCTION_ITEMS
        }
    })
}

/// An AF1-X construction plus origins for every generated region.
#[derive(Clone, Debug)]
pub struct Expansion {
    /// Ordinary AF1-X, ready for the unchanged compiler.
    pub frame: Value,
    /// Expanded JSON pointers to authored decisions or disclosed fragment rules.
    pub provenance: BTreeMap<String, Value>,
}

impl Expansion {
    /// Resolves an AF1-X pointer to its most specific residual origin.
    /// Directly copied subtrees retain the suffix below their authored pointer.
    /// A derived value keeps all of its inputs rather than selecting one as
    /// though it were the only author decision.
    #[must_use]
    pub fn origin(&self, pointer: &str) -> Option<Value> {
        let (prefix, source) = self
            .provenance
            .iter()
            .filter(|(prefix, _)| {
                pointer == prefix.as_str()
                    || pointer
                        .strip_prefix(prefix.as_str())
                        .is_some_and(|tail| tail.starts_with('/'))
            })
            .max_by_key(|(prefix, _)| prefix.len())?;
        let mut source = source.clone();
        if source["class"] == "AUTHORED" {
            let authored = source["pointer"].as_str()?;
            source["pointer"] = json!(format!("{authored}{}", &pointer[prefix.len()..]));
        }
        Some(source)
    }

    /// Composes the unchanged AF1-X source map with residual provenance for a
    /// pointer in final, plain AF1. A missing origin stays missing; it is never
    /// relabelled as a justified derivation.
    #[must_use]
    pub fn lowered_origin(&self, map: &crate::afx::SourceMap, pointer: &str) -> Option<Value> {
        let entry = map
            .entries
            .iter()
            .filter(|entry| {
                pointer == entry.expanded
                    || pointer
                        .strip_prefix(&entry.expanded)
                        .is_some_and(|tail| tail.starts_with('/'))
            })
            .max_by_key(|entry| entry.expanded.len());
        let Some(entry) = entry else {
            return self.origin(pointer);
        };
        self.lowering_origin(&entry.authored)
    }

    pub(crate) fn lowering_origin(&self, intermediate: &str) -> Option<Value> {
        // AF1-X maps constructs, not necessarily shape-identical leaves:
        // an ok terminator can become a variant op with more operands.
        // Bind that checked lowering to the source construct and every
        // more-specific decision under it, rather than inventing leaf offsets.
        self.frame.pointer(intermediate)?;
        let mut origins = vec![self.origin(intermediate)?];
        origins.extend(
            self.provenance
                .iter()
                .filter(|(at, _)| {
                    at.strip_prefix(intermediate)
                        .is_some_and(|tail| tail.starts_with('/'))
                })
                .map(|(_, origin)| origin.clone()),
        );
        Some(json!({"class":"DERIVED", "rule":"af1-x-lowering-v1",
            "intermediate":intermediate, "origins":origins}))
    }
}

/// Expands a fully specified derive request without reading files or graphs.
///
/// Signature type resolution, CFG validity and candidate authority are checked
/// by the existing compiler and kernel. Missing semantic parameters are refused;
/// no singleton or public-test result supplies them.
///
/// # Errors
///
/// Refuses unknown fragments, missing decisions, unsupported shapes, edit
/// requests without an implemented lens, or an exceeded aggregate budget.
pub fn expand(request: &Request) -> Result<Expansion> {
    expand_with_budget(request, &mut Budget::default())
}

/// Expands under the caller's aggregate work and clock budget.
///
/// # Errors
/// The same refusals as [`expand`], including prior shared-budget exhaustion.
pub fn expand_with_budget(request: &Request, budget: &mut Budget) -> Result<Expansion> {
    budget.checkpoint()?;
    encoding::size(&request.bindings, budget, super::MAX_REQUEST_BYTES)?;
    if request.operation != Operation::Derive {
        return Err(failure(
            AgentErrorCode::ResidualFragmentShape,
            "/operation",
            "this fragment constructs a function; its edit lens is not enabled",
        ));
    }
    let scope = request
        .scope
        .as_array()
        .filter(|scope| scope.len() == 1)
        .ok_or_else(|| {
            failure(
                AgentErrorCode::ResidualScope,
                "/scope",
                "name exactly one target function",
            )
        })?;
    let name = scope[0]
        .as_str()
        .filter(|name| is_identifier(name) && !name.contains("__"))
        .ok_or_else(|| {
            failure(
                AgentErrorCode::ResidualScope,
                "/scope/0",
                "give an AF1-X function name",
            )
        })?;
    let params = required(&request.bindings, "params", "/bindings")?;
    let returns = required(&request.bindings, "returns", "/bindings")?;
    if !params.is_array() || (!returns.is_string() && !returns.is_object()) {
        return Err(shape(
            "/bindings",
            "params is an AF1 parameter list; returns is an explicit AF1 type",
        ));
    }
    let mut body = request.bindings.clone();
    body.remove("params");
    body.remove("returns");
    let mut builder = Builder {
        budget,
        blocks: Vec::new(),
        provenance: BTreeMap::new(),
        items: 0,
        prefix: "gw".into(),
    };
    // Generated block names must not capture an authored name or reference.
    while bindings_mention_prefix(&request.bindings, &builder.prefix, builder.budget)? {
        builder.prefix.push('g');
        if builder.prefix.len() > 32 {
            return Err(failure(
                AgentErrorCode::ResidualLimit,
                "/bindings",
                "cannot allocate uncaptured block names",
            ));
        }
    }
    let entry = builder.fragment(&request.fragment, &body, "/bindings", "/fragment", 1)?;
    builder
        .provenance
        .insert("/af1".into(), rule("/fragment", "af1-x-envelope-v1"));
    builder
        .provenance
        .insert("/afx".into(), rule("/fragment", "af1-x-envelope-v1"));
    builder
        .provenance
        .insert("/fns/0".into(), rule("/fragment", "function-shell-v1"));
    builder
        .provenance
        .insert("/fns/0/fn".into(), authored_origin("/scope/0"));
    builder
        .provenance
        .insert("/fns/0/params".into(), authored_origin("/bindings/params"));
    builder.provenance.insert(
        "/fns/0/returns".into(),
        authored_origin("/bindings/returns"),
    );
    let expansion = Expansion {
        frame: json!({"af1":1, "afx":1, "fns":[{
            "fn":name, "params":params, "returns":returns,
            "entry":entry, "blocks":builder.blocks
        }]}),
        provenance: builder.provenance,
    };
    builder.budget.checkpoint()?;
    Ok(expansion)
}

struct Builder<'a> {
    budget: &'a mut Budget,
    blocks: Vec<Value>,
    provenance: BTreeMap<String, Value>,
    items: usize,
    prefix: String,
}

impl Builder<'_> {
    fn charge(&mut self, count: usize, at: &str) -> Result<()> {
        self.budget.charge(count.max(1))?;
        self.items = self.items.checked_add(count).ok_or_else(|| limit(at))?;
        if self.items > MAX_CONSTRUCTION_ITEMS {
            return Err(limit(at));
        }
        Ok(())
    }

    fn block(&mut self, ops: Value, term: Value, origin: Value) -> Result<(String, String)> {
        self.budget.checkpoint()?;
        if self.blocks.len() >= crate::afx::MAX_GENERATED_BLOCKS_PER_FUNCTION {
            return Err(limit("/bindings"));
        }
        let index = self.blocks.len();
        let name = format!("{}_b{index}", self.prefix);
        let pointer = format!("/fns/0/blocks/{index}");
        self.blocks.push(Value::Object(Map::from_iter([
            ("name".into(), json!(name)),
            ("ops".into(), ops),
            ("term".into(), term),
        ])));
        self.provenance.insert(pointer.clone(), origin);
        Ok((name, pointer))
    }

    fn fragment(
        &mut self,
        fragment: &FragmentRef,
        bindings: &Map<String, Value>,
        at: &str,
        selected: &str,
        depth: usize,
    ) -> Result<String> {
        self.budget.checkpoint()?;
        if depth > MAX_FRAGMENT_DEPTH {
            return Err(limit(at));
        }
        if fragment.version != 1 {
            return Err(failure(
                AgentErrorCode::ResidualFragmentUnknown,
                selected,
                "unsupported fragment version",
            ));
        }
        match fragment.id.as_str() {
            "ordered_guard_chain" => self.guards(bindings, at, selected, depth),
            "checked_pipeline" => self.pipeline(bindings, at, selected),
            "typed_branch_result" => self.branch(bindings, at, selected),
            _ => Err(failure(
                AgentErrorCode::ResidualFragmentUnknown,
                selected,
                "unknown fragment family",
            )),
        }
    }

    fn guards(
        &mut self,
        bindings: &Map<String, Value>,
        at: &str,
        selected: &str,
        depth: usize,
    ) -> Result<String> {
        self.budget.checkpoint()?;
        closed(bindings, &["guards", "success"], at)?;
        let guards = list(required(bindings, "guards", at)?, &format!("{at}/guards"))?;
        let mut entries = Vec::new();
        // Blocks are emitted in written order; edges are connected after the
        // terminal region is known. No guard is sorted, duplicated, or hoisted.
        for (index, guard) in guards.iter().enumerate() {
            self.budget.charge(1)?;
            let pointer = format!("{at}/guards/{index}");
            let object = guard
                .as_object()
                .ok_or_else(|| shape(&pointer, "guard is an object"))?;
            closed(object, &["when", "fail"], &pointer)?;
            let predicate = required(object, "when", &pointer)?;
            let exit = required(object, "fail", &pointer)?;
            let exit_array = exit
                .as_array()
                .filter(|items| (1..=3).contains(&items.len()) && items[0] == "fail")
                .ok_or_else(|| {
                    shape(
                        &format!("{pointer}/fail"),
                        "write an explicit AF1-X fail terminator",
                    )
                })?;
            if exit_array.len() > 1 && !exit_array[1].is_string() {
                return Err(shape(
                    &format!("{pointer}/fail/1"),
                    "failure case must be a name",
                ));
            }
            self.charge(1, &pointer)?;
            let check_index = self.blocks.len();
            let (check, check_at) = self.block(
                json!([]),
                Value::Null,
                rule(selected, "ordered-guard-blocks-v1"),
            )?;
            let (failure_name, failure_at) = self.block(
                json!([]),
                exit.clone(),
                rule(selected, "ordered-guard-blocks-v1"),
            )?;
            self.provenance.insert(
                format!("{failure_at}/term"),
                authored_origin(&format!("{pointer}/fail")),
            );
            self.provenance.insert(
                format!("{check_at}/term/1"),
                authored_origin(&format!("{pointer}/when")),
            );
            entries.push((check, check_index, failure_name, predicate.clone()));
        }
        let success = self.region(
            required(bindings, "success", at)?,
            &format!("{at}/success"),
            depth + 1,
        )?;
        for (index, (_, block_index, failure_name, predicate)) in entries.iter().enumerate() {
            self.budget.charge(1)?;
            let next = entries
                .get(index + 1)
                .map_or(success.as_str(), |entry| entry.0.as_str());
            self.blocks[*block_index]["term"] = json!(["cond", predicate, failure_name, next]);
        }
        Ok(entries.first().map_or(success, |entry| entry.0.clone()))
    }

    fn region(&mut self, value: &Value, at: &str, depth: usize) -> Result<String> {
        self.budget.checkpoint()?;
        let object = value
            .as_object()
            .ok_or_else(|| shape(at, "success region is an object"))?;
        if let Some(fragment) = object.get("fragment") {
            closed(object, &["fragment", "bindings"], at)?;
            let fragment = super::parse_fragment(fragment.clone())?;
            let bindings = required(object, "bindings", at)?
                .as_object()
                .ok_or_else(|| shape(at, "nested bindings must be an object"))?;
            self.fragment(
                &fragment,
                bindings,
                &format!("{at}/bindings"),
                &format!("{at}/fragment"),
                depth,
            )
        } else {
            closed(object, &["ops", "term"], at)?;
            let ops = required(object, "ops", at)?
                .as_array()
                .ok_or_else(|| shape(at, "ops must be an AF1-X operation list"))?;
            let term = required(object, "term", at)?;
            if !term.is_array() {
                return Err(shape(at, "term must be an AF1-X terminator"));
            }
            self.charge(ops.len(), at)?;
            let (name, pointer) =
                self.block(json!(ops), term.clone(), rule(at, "terminal-region-v1"))?;
            self.provenance.insert(
                format!("{pointer}/ops"),
                authored_origin(&format!("{at}/ops")),
            );
            self.provenance.insert(
                format!("{pointer}/term"),
                authored_origin(&format!("{at}/term")),
            );
            Ok(name)
        }
    }

    fn branch(
        &mut self,
        bindings: &Map<String, Value>,
        at: &str,
        selected: &str,
    ) -> Result<String> {
        self.budget.checkpoint()?;
        closed(bindings, &["input", "cases", "join"], at)?;
        let cases = list(required(bindings, "cases", at)?, &format!("{at}/cases"))?;
        if cases.is_empty() {
            return Err(failure(
                AgentErrorCode::ResidualChoiceMissing,
                &format!("{at}/cases"),
                "supply explicit exhaustive cases; an empty family does not define a result",
            ));
        }
        let join_at = format!("{at}/join");
        let join = required(bindings, "join", at)?
            .as_object()
            .ok_or_else(|| shape(&join_at, "join is an object"))?;
        closed(join, &["params", "ops", "term"], &join_at)?;
        let join_params = list(
            required(join, "params", &join_at)?,
            &format!("{join_at}/params"),
        )?;
        let join_ops = required(join, "ops", &join_at)?
            .as_array()
            .ok_or_else(|| shape(&join_at, "join ops must be an AF1-X operation list"))?;
        let join_term = required(join, "term", &join_at)?;
        let term = join_term
            .as_array()
            .ok_or_else(|| shape(&join_at, "join needs an explicit result terminator"))?;
        if !term
            .first()
            .and_then(Value::as_str)
            .is_some_and(|op| matches!(op, "ok" | "fail" | "return"))
        {
            return Err(shape(
                &format!("{join_at}/term"),
                "join must explicitly return, ok, or fail",
            ));
        }
        self.charge(cases.len() + join_ops.len() + join_params.len(), at)?;
        let dispatch_index = self.blocks.len();
        let (entry, entry_at) = self.block(
            json!([]),
            Value::Null,
            rule(selected, "typed-switch-join-v1"),
        )?;
        let join_index = self.blocks.len();
        let (join_name, join_pointer) = self.block(
            json!(join_ops),
            join_term.clone(),
            rule(selected, "typed-switch-join-v1"),
        )?;
        self.blocks[join_index]["params"] = json!(join_params);
        for field in ["params", "ops", "term"] {
            self.provenance.insert(
                format!("{join_pointer}/{field}"),
                authored_origin(&format!("{join_at}/{field}")),
            );
        }
        let mut switch = vec![json!("switch"), required(bindings, "input", at)?.clone()];
        self.provenance.insert(
            format!("{entry_at}/term/1"),
            authored_origin(&format!("{at}/input")),
        );
        let mut seen = std::collections::BTreeSet::new();
        for (index, case) in cases.iter().enumerate() {
            self.budget.charge(1)?;
            let case_at = format!("{at}/cases/{index}");
            let case = case
                .as_object()
                .ok_or_else(|| shape(&case_at, "case is an object"))?;
            closed(case, &["case", "payload", "ops", "values"], &case_at)?;
            let tag = required(case, "case", &case_at)?
                .as_str()
                .filter(|name| is_identifier(name))
                .ok_or_else(|| {
                    shape(
                        &format!("{case_at}/case"),
                        "case must be a variant case name",
                    )
                })?;
            if !seen.insert(tag) {
                return Err(shape(&format!("{case_at}/case"), "duplicate case"));
            }
            let arm = self.branch_arm(case, &case_at, selected, (&join_name, join_params.len()))?;
            self.provenance.insert(
                format!("{entry_at}/term/{}/0", index + 2),
                authored_origin(&format!("{case_at}/case")),
            );
            switch.push(json!(arm));
        }
        self.blocks[dispatch_index]["term"] = json!(switch);
        Ok(entry)
    }

    fn branch_arm(
        &mut self,
        case: &Map<String, Value>,
        case_at: &str,
        selected: &str,
        join: (&str, usize),
    ) -> Result<Vec<Value>> {
        self.budget.checkpoint()?;
        let payload = required(case, "payload", case_at)?;
        if !payload.is_null() {
            let pair = payload
                .as_array()
                .filter(|pair| pair.len() == 2)
                .ok_or_else(|| {
                    shape(
                        &format!("{case_at}/payload"),
                        "payload is null or [name, AF1 type]",
                    )
                })?;
            if !pair[0]
                .as_str()
                .is_some_and(|name| is_identifier(name) && !name.contains("__"))
                || (!pair[1].is_string() && !pair[1].is_object())
            {
                return Err(shape(
                    &format!("{case_at}/payload"),
                    "payload needs an explicit name and type",
                ));
            }
        }
        let ops = required(case, "ops", case_at)?
            .as_array()
            .ok_or_else(|| shape(case_at, "case ops must be an AF1-X operation list"))?;
        let values = list(
            required(case, "values", case_at)?,
            &format!("{case_at}/values"),
        )?;
        if values.len() != join.1 {
            return Err(shape(
                &format!("{case_at}/values"),
                "supply one value per explicit join parameter",
            ));
        }
        self.charge(ops.len() + values.len(), case_at)?;
        let mut jump = vec![json!("br"), json!(join.0)];
        jump.extend(values.iter().cloned());
        let block_index = self.blocks.len();
        let (name, pointer) = self.block(
            json!(ops),
            json!(jump),
            rule(selected, "typed-switch-join-v1"),
        )?;
        let mut arm = vec![case["case"].clone(), json!(name)];
        if !payload.is_null() {
            self.blocks[block_index]["params"] = json!([payload]);
            arm.push(json!("$"));
            self.provenance.insert(
                format!("{pointer}/params/0"),
                authored_origin(&format!("{case_at}/payload")),
            );
        }
        self.provenance.insert(
            format!("{pointer}/ops"),
            authored_origin(&format!("{case_at}/ops")),
        );
        for value_index in 0..values.len() {
            self.budget.charge(1)?;
            self.provenance.insert(
                format!("{pointer}/term/{}", value_index + 2),
                authored_origin(&format!("{case_at}/values/{value_index}")),
            );
        }
        Ok(arm)
    }

    fn pipeline(
        &mut self,
        bindings: &Map<String, Value>,
        at: &str,
        selected: &str,
    ) -> Result<String> {
        self.budget.checkpoint()?;
        closed(
            bindings,
            &["steps", "arithmetic_failure", "rounding", "result"],
            at,
        )?;
        let policy = required(bindings, "arithmetic_failure", at)?;
        let route = if policy.as_object().is_some_and(|object| {
            object.len() == 1 && object.get("propagate").and_then(Value::as_bool) == Some(true)
        }) {
            ""
        } else {
            policy
                .as_str()
                .filter(|name| is_identifier(name) && !name.contains("__"))
                .ok_or_else(|| {
                    shape(
                        at,
                        "arithmetic_failure is a error variant case name or {propagate:true}",
                    )
                })?
        };
        if required(bindings, "rounding", at)? != "toward_zero" {
            return Err(failure(
                AgentErrorCode::ResidualConstraintConflict,
                &format!("{at}/rounding"),
                "this version supports the existing VM's toward_zero division policy",
            ));
        }
        let steps = list(required(bindings, "steps", at)?, &format!("{at}/steps"))?;
        let mut ops = Vec::new();
        let mut origins = Vec::new();
        for (index, step) in steps.iter().enumerate() {
            self.budget.charge(1)?;
            let pointer = format!("{at}/steps/{index}");
            let pair = step
                .as_array()
                .filter(|pair| pair.len() == 2)
                .ok_or_else(|| shape(&pointer, "step is [name, arithmetic expression]"))?;
            let name = pair[0]
                .as_str()
                .filter(|name| is_identifier(name) && !name.contains("__"))
                .ok_or_else(|| shape(&pointer, "step needs an AF1-X result name"))?;
            let expression = self.arithmetic(&pair[1], route, &format!("{pointer}/1"), 0)?;
            let expression = expression
                .as_array()
                .ok_or_else(|| shape(&pointer, "a step needs an arithmetic operation"))?;
            let mut op = vec![json!(name)];
            op.extend(expression.iter().cloned());
            ops.push(json!(op));
            origins.push(pointer);
        }
        let result = required(bindings, "result", at)?;
        if result.is_array() || result.is_null() || result.is_boolean() {
            return Err(shape(
                &format!("{at}/result"),
                "result is an integer name or literal; put arithmetic in a named step",
            ));
        }
        let (name, pointer) = self.block(
            json!(ops),
            json!(["ok", result]),
            rule(selected, "checked-steps-v1"),
        )?;
        for (index, authored) in origins.iter().enumerate() {
            self.budget.charge(1)?;
            self.provenance.insert(
                format!("{pointer}/ops/{index}/0"),
                authored_origin(&format!("{authored}/0")),
            );
            self.provenance.insert(format!("{pointer}/ops/{index}"), json!({
                "class":"DERIVED", "rule":"checked-route-v1",
                "inputs":[format!("{authored}/1"), format!("{at}/arithmetic_failure"), format!("{at}/rounding")]
            }));
        }
        self.provenance.insert(
            format!("{pointer}/term/1"),
            authored_origin(&format!("{at}/result")),
        );
        Ok(name)
    }

    fn arithmetic(
        &mut self,
        expression: &Value,
        route: &str,
        at: &str,
        depth: usize,
    ) -> Result<Value> {
        self.budget.charge(1)?;
        if depth > crate::afx::MAX_EXPR_DEPTH {
            return Err(limit(at));
        }
        match expression {
            Value::String(_) | Value::Number(_) => Ok(expression.clone()),
            Value::Object(object) => {
                closed(object, &["type", "value"], at)?;
                if !object["value"].is_number() {
                    return Err(shape(at, "arithmetic literal must be an integer"));
                }
                Ok(expression.clone())
            }
            Value::Array(items) => {
                let Some(op) = items.first().and_then(Value::as_str) else {
                    return Err(shape(at, "arithmetic expression needs an opcode"));
                };
                let row = pipeline_opcode(op).ok_or_else(|| {
                    shape(
                        at,
                        "unsupported arithmetic opcode; use ordinary AF1-X for this operation",
                    )
                })?;
                let operands = if row.tag == 69 { 1 } else { 2 };
                if items.len() != operands + 1 {
                    return Err(shape(at, "wrong arithmetic operand count"));
                }
                self.charge(1, at)?;
                let mut expanded = vec![json!(format!("{op}?{route}"))];
                for (index, value) in items.iter().enumerate().skip(1) {
                    expanded.push(self.arithmetic(
                        value,
                        route,
                        &format!("{at}/{index}"),
                        depth + 1,
                    )?);
                }
                Ok(json!(expanded))
            }
            _ => Err(shape(at, "arithmetic operands must be integer expressions")),
        }
    }
}

fn required<'a>(object: &'a Map<String, Value>, field: &str, at: &str) -> Result<&'a Value> {
    object.get(field).ok_or_else(|| {
        failure(
            AgentErrorCode::ResidualChoiceMissing,
            &format!("{at}/{field}"),
            "supply this semantic decision explicitly",
        )
    })
}

fn closed(object: &Map<String, Value>, fields: &[&str], at: &str) -> Result<()> {
    for key in object.keys() {
        if !fields.contains(&key.as_str()) {
            return Err(shape(at, &format!("unknown field {key}")));
        }
    }
    for key in fields {
        required(object, key, at)?;
    }
    Ok(())
}

fn list<'a>(value: &'a Value, at: &str) -> Result<&'a Vec<Value>> {
    let list = value
        .as_array()
        .ok_or_else(|| shape(at, "expected an ordered list"))?;
    if list.len() > MAX_FRAGMENT_ITEMS {
        return Err(limit(at));
    }
    Ok(list)
}

fn bindings_mention_prefix(
    bindings: &Map<String, Value>,
    prefix: &str,
    budget: &mut Budget,
) -> Result<bool> {
    for value in bindings.values() {
        if mentions_prefix(value, prefix, budget)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn mentions_prefix(value: &Value, prefix: &str, budget: &mut Budget) -> Result<bool> {
    budget.charge(1)?;
    match value {
        Value::String(text) => Ok(text.starts_with(prefix)),
        Value::Array(values) => {
            for value in values {
                if mentions_prefix(value, prefix, budget)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Value::Object(object) => bindings_mention_prefix(object, prefix, budget),
        _ => Ok(false),
    }
}

fn authored_origin(pointer: &str) -> Value {
    json!({"class":"AUTHORED", "pointer":pointer})
}
fn rule(selection: &str, rule: &str) -> Value {
    json!({"class":"FRAGMENT_DEFINED", "selection":selection, "rule":rule})
}
fn shape(at: &str, detail: &str) -> AgentError {
    failure(AgentErrorCode::ResidualFragmentShape, at, detail)
}
fn limit(at: &str) -> AgentError {
    failure(
        AgentErrorCode::ResidualLimit,
        at,
        "fragment construction budget exceeded; use ordinary AF1-X",
    )
}
fn failure(code: AgentErrorCode, at: &str, detail: &str) -> AgentError {
    AgentError::new(code, format!("{at}: {detail}"))
}

#[cfg(test)]
mod tests;
