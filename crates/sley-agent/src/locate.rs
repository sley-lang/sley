//! Authored frame positions for function-wide kernel refusals.
//!
//! A phase 7 refusal names a whole function: the kernel checks a function's
//! graph and operations as one unit. For a candidate made from an AF1 frame,
//! this module maps that function, and the blocks, operations, terminators
//! and signatures the advisory analysis (`explain`) ties to the kernel's
//! symbol, to JSON pointers into the frame. It never changes the kernel's
//! judgment. When nothing narrower is identifiable it says so and points at
//! the whole function; it never guesses a single operation.
//!
//! For a frame an authoring dialect expanded, a site is first mapped
//! through the source map's entries (`{"entries": [{"expanded",
//! "authored", "role", "name"}]}`): a generated block's terminator maps to
//! the authored terminator, checked operation or exit that produced it; an
//! operation in a generated block to its authored operation; a shared exit
//! to the whole function. Names a frame does not spell are otherwise looked
//! up in the source map's names tables (`blocks`, `values`, then `names`:
//! `{"<function>": {"<generated name>": "<authored pointer>"}}`).

use serde_json::{Map, Value, json};
use sley_id::EntityId;
use sley_mutate::value::EntityBodyValue;
use sley_policy::RefusalLocator;

use crate::explain::{self, Site};
use crate::names::Names;
use crate::workspace::Program;

/// Refusal symbols that concern the refused function's signature: with a
/// function-wide locator its authored parameters and result are reported.
const SIGNATURE_SYMBOLS: [&str; 2] = ["VM_LOWER_SIGNATURE_MISMATCH", "CFG_RETURN_TYPE"];

/// What a function pointer means when nothing narrower is identifiable.
pub const FUNCTION_WIDE: &str = "function-wide; the kernel names no smaller location";

/// Where a candidate's authored coordinates come from: the frame it was made
/// from and, when present, that frame's source-map names table.
#[derive(Clone, Copy, Debug, Default)]
pub struct Source<'a> {
    /// The frame (the layered frame for `try --on`); `None` for raw operations.
    pub frame: Option<&'a Value>,
    /// The source-map names table, when the frame produced one.
    pub sourcemap: Option<&'a Value>,
}

impl<'a> Source<'a> {
    /// The frame `try` compiled, with its `sourcemap.json` artifact if any.
    #[must_use]
    pub fn of_try(frame: &'a Value, artifacts: &'a [(String, Value)]) -> Self {
        Self {
            frame: frame.is_object().then_some(frame),
            sourcemap: artifacts
                .iter()
                .find(|(file, _)| file == "sourcemap.json")
                .map(|(_, map)| map),
        }
    }

    /// The frame (and `sourcemap`) kept in a candidate's metadata.
    #[must_use]
    pub fn of_meta(meta: &'a Value) -> Self {
        Self {
            frame: meta.get("frame").filter(|frame| frame.is_object()),
            sourcemap: meta.get("sourcemap"),
        }
    }
}

/// The authored positions of one refusal.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Authored {
    /// JSON pointers into the frame, each with what it designates.
    pub items: Vec<(String, String)>,
    /// Why there is no pointer, when there is none.
    pub none: Option<String>,
    /// The frame the pointers index, when it is not the frame as given (a
    /// layered frame: a draft revision's `frame.json`).
    pub frame: Option<String>,
}

impl Authored {
    /// The text of the `authored:` line.
    #[must_use]
    pub fn text(&self) -> String {
        if let Some(reason) = &self.none {
            return format!("none ({reason})");
        }
        let text = self
            .items
            .iter()
            .map(|(at, what)| format!("{at} ({what})"))
            .collect::<Vec<_>>()
            .join(", ");
        match &self.frame {
            Some(frame) => format!("{text}; pointers refer to {frame}"),
            None => text,
        }
    }

    fn push(&mut self, at: String, what: String) {
        if !self.items.iter().any(|(existing, _)| *existing == at) {
            self.items.push((at, what));
        }
    }

    /// `[{"at": pointer, "what": ...}]`; `at` is null when there is none.
    #[must_use]
    pub fn to_json(&self) -> Value {
        if let Some(reason) = &self.none {
            return json!([{"at": Value::Null, "what": reason}]);
        }
        Value::Array(
            self.items
                .iter()
                .map(|(at, what)| json!({"at": at, "what": what}))
                .collect(),
        )
    }
}

/// The authored positions of a refusal whose locator names only a function,
/// for a candidate made from `frame`. `None` when the locator is narrower,
/// the refusal is not at phase 7, or `frame` is not a frame object.
#[must_use]
pub fn authored(
    symbol: &str,
    phase: Option<u32>,
    locator: &RefusalLocator,
    program: &Program,
    names: &Names,
    source: &Source<'_>,
    analysis: Option<&explain::Analysis>,
) -> Option<Authored> {
    let (frame, sourcemap) = (source.frame?, source.sourcemap);
    let function = locator.subject?;
    let narrower =
        locator.operation.is_some() || locator.related.is_some() || locator.field.is_some();
    if phase != Some(7) || narrower {
        return None;
    }
    let Some(EntityBodyValue::Function(_)) = program.body(&function) else {
        return None;
    };
    let index = FrameIndex {
        frame: frame.as_object()?,
        sourcemap,
    };
    let mut out = Authored::default();
    let analysis = match analysis {
        Some(analysis) if analysis.function() == function => analysis.clone(),
        _ => explain::Analysis::of(locator, program, names)?,
    };
    for site in analysis.sites(symbol) {
        for (at, what) in index.site(site, program, names) {
            if what == FUNCTION_WIDE {
                for (at, _) in index.entries(&names.leaf(&function)) {
                    out.push(at, FUNCTION_WIDE.to_owned());
                }
            } else {
                out.push(at, what);
            }
        }
    }
    if !out.items.is_empty() {
        return Some(out);
    }
    let leaf = names.leaf(&function);
    let entries = index.entries(&leaf);
    if entries.is_empty() {
        out.none = Some(format!(
            "{} is not in this frame; the kernel names the whole function",
            names.name(&function)
        ));
        return Some(out);
    }
    for (at, _) in entries {
        out.push(at, FUNCTION_WIDE.to_owned());
    }
    if SIGNATURE_SYMBOLS.contains(&symbol) {
        for (at, what) in index.signature(&leaf, function, names) {
            out.push(at, what);
        }
    }
    Some(out)
}

/// Pointer lookups in one frame (and its optional source-map names table).
struct FrameIndex<'a> {
    frame: &'a Map<String, Value>,
    sourcemap: Option<&'a Value>,
}

impl FrameIndex<'_> {
    /// The pointers of a finding's site, each with what it designates.
    fn site(&self, site: Site, program: &Program, names: &Names) -> Vec<(String, String)> {
        let owner = |block: &EntityId| match program.body(block) {
            Some(EntityBodyValue::Block(body)) => Some(names.leaf(&body.function)),
            _ => None,
        };
        let one = |at: Option<String>, what: String| at.map(|at| (at, what)).into_iter().collect();
        if let Some(found) = self.through_map(site, program, names) {
            return found;
        }
        match site {
            Site::Block(block) => {
                let Some(function) = owner(&block) else {
                    return Vec::new();
                };
                let leaf = names.leaf(&block);
                let at = self.block(&function, &leaf).map(|(at, _)| at);
                one(
                    at.or_else(|| self.generated(&function, &leaf, "blocks")),
                    names.name(&block),
                )
            }
            Site::Terminator(block) => {
                let Some(function) = owner(&block) else {
                    return Vec::new();
                };
                let leaf = names.leaf(&block);
                let at = self
                    .block(&function, &leaf)
                    .filter(|(_, body)| body.get("term").is_some())
                    .map(|(at, _)| format!("{at}/term"));
                one(
                    at.or_else(|| self.generated(&function, &leaf, "blocks")),
                    format!("terminator of {}", names.name(&block)),
                )
            }
            Site::Operation(operation) => {
                let Some(EntityBodyValue::Operation(body)) = program.body(&operation) else {
                    return Vec::new();
                };
                let Some(function) = owner(&body.block) else {
                    return Vec::new();
                };
                let (block, leaf) = (names.leaf(&body.block), names.leaf(&operation));
                one(
                    self.operation(&function, &block, &leaf)
                        .or_else(|| self.generated(&function, &leaf, "values")),
                    names.name(&operation),
                )
            }
            Site::Constant(constant) => {
                let leaf = names.leaf(&constant);
                let at = self.list("consts").find_map(|(at, entry)| {
                    (entry.get("name").and_then(Value::as_str) == Some(leaf.as_str())).then(|| {
                        if entry.get("type").is_some() {
                            format!("{at}/type")
                        } else {
                            at
                        }
                    })
                });
                one(at, format!("type of constant {}", names.name(&constant)))
            }
            Site::Params(function) | Site::Returns(function) => {
                let key = if matches!(site, Site::Params(_)) {
                    "params"
                } else {
                    "returns"
                };
                self.signature(&names.leaf(&function), function, names)
                    .into_iter()
                    .filter(|(at, _)| at.ends_with(key))
                    .collect()
            }
        }
    }

    /// A block, terminator or operation site in a frame an authoring
    /// dialect expanded, through the source map's entries; `None` when the
    /// site's block is not in them. A shared exit, or a construct the map
    /// does not place, is disclosed as function-wide.
    fn through_map(
        &self,
        site: Site,
        program: &Program,
        names: &Names,
    ) -> Option<Vec<(String, String)>> {
        let wide = || vec![(String::new(), FUNCTION_WIDE.to_owned())];
        let (block, operation) = match site {
            Site::Block(block) | Site::Terminator(block) => (block, None),
            Site::Operation(operation) => match program.body(&operation) {
                Some(EntityBodyValue::Operation(body)) => (body.block, Some(operation)),
                _ => return None,
            },
            _ => return None,
        };
        let Some(EntityBodyValue::Block(body)) = program.body(&block) else {
            return None;
        };
        let mapped = self.mapped(&names.leaf(&body.function), &names.leaf(&block))?;
        let name = names.name(&block);
        Some(match (site, mapped.role.as_str()) {
            (Site::Block(_), "block") => vec![(mapped.authored, name)],
            // A continuation is the rest of an authored block.
            (Site::Block(_), "continuation") => vec![(
                mapped
                    .authored
                    .split("/ops/")
                    .next()
                    .unwrap_or_default()
                    .to_owned(),
                format!("{name} (part of this block)"),
            )],
            (Site::Terminator(_), "block" | "continuation") => {
                match self.mapped_at(&mapped.expanded, "/term", None) {
                    Some((at, role)) if role == "term" => {
                        vec![(at, format!("terminator of {name}"))]
                    }
                    Some((at, role)) if role == "switch" => {
                        vec![(at, format!("the checked operation that ends {name}"))]
                    }
                    Some((at, role)) if role == "exit" => {
                        vec![(at, format!("the exit that ends {name}"))]
                    }
                    _ => wide(),
                }
            }
            (Site::Operation(_), "block" | "continuation") => {
                let operation = operation?;
                match self.mapped_at(&mapped.expanded, "/ops/", Some(&names.leaf(&operation))) {
                    Some((at, role)) if role != "shared-exit" => vec![(at, names.name(&operation))],
                    _ => wide(),
                }
            }
            _ => wide(),
        })
    }

    /// The entries of one top-level frame list, with their pointers.
    fn list(&self, key: &str) -> impl Iterator<Item = (String, &Value)> {
        let entries = match self.frame.get(key) {
            Some(Value::Array(list)) => list.as_slice(),
            _ => &[],
        };
        entries
            .iter()
            .enumerate()
            .map(move |(index, entry)| (format!("/{key}/{index}"), entry))
    }

    /// Every `fns`, `functions`, `patch` or `edit` entry stating `function`.
    fn entries(&self, function: &str) -> Vec<(String, &Value)> {
        ["fns", "functions", "patch", "edit"]
            .into_iter()
            .flat_map(|key| self.list(key))
            .filter(|(_, entry)| entry.get("fn").and_then(Value::as_str) == Some(function))
            .collect()
    }

    /// The authored `params` and `returns` of `function`.
    fn signature(&self, leaf: &str, function: EntityId, names: &Names) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (key, what) in [("params", "parameters"), ("returns", "result")] {
            for (at, entry) in self.entries(leaf) {
                if entry.get(key).is_some() {
                    out.push((
                        format!("{at}/{key}"),
                        format!("{what} of {}", names.name(&function)),
                    ));
                }
            }
        }
        out
    }

    /// A block restated in a definition (`blocks` array) or patch (`blocks`
    /// object), with its authored value.
    fn block(&self, function: &str, block: &str) -> Option<(String, &Value)> {
        self.entries(function)
            .into_iter()
            .find_map(|(at, entry)| match entry.get("blocks")? {
                Value::Array(blocks) => blocks.iter().enumerate().find_map(|(index, body)| {
                    (body.get("name").and_then(Value::as_str) == Some(block))
                        .then(|| (format!("{at}/blocks/{index}"), body))
                }),
                Value::Object(blocks) => blocks
                    .get(block)
                    .filter(|body| !body.is_null())
                    .map(|body| (format!("{at}/blocks/{}", escape(block)), body)),
                _ => None,
            })
    }

    /// An operation: an `edit` that replaces it, or its entry in a restated
    /// block's `ops`.
    fn operation(&self, function: &str, block: &str, operation: &str) -> Option<String> {
        let path = format!("{block}.{operation}");
        if let Some((at, _)) = self.entries(function).into_iter().find(|(at, entry)| {
            at.starts_with("/edit/")
                && entry.get("replace_op").and_then(Value::as_str) == Some(path.as_str())
        }) {
            return Some(format!("{at}/with"));
        }
        let (at, body) = self.block(function, block)?;
        let ops = body.get("ops")?.as_array()?;
        ops.iter()
            .position(|op| {
                let name = match op {
                    Value::Array(items) => items.first(),
                    Value::Object(object) => object.get("name"),
                    _ => None,
                };
                name.and_then(Value::as_str) == Some(operation)
            })
            .map(|index| format!("{at}/ops/{index}"))
    }

    /// The source-map pointer of a name the frame does not spell: its
    /// table of one kind (`blocks` or `values`), else the merged `names`.
    fn generated(&self, function: &str, name: &str, kind: &str) -> Option<String> {
        let map = self.sourcemap?;
        [kind, "names"].into_iter().find_map(|table| {
            map.get(table)?
                .get(function)?
                .get(name)?
                .as_str()
                .map(str::to_owned)
        })
    }

    /// The source-map entry of expanded block `block` of `function`: its
    /// expanded pointer, authored pointer and role.
    fn mapped(&self, function: &str, block: &str) -> Option<Mapped> {
        let entries = self.sourcemap?.get("entries")?.as_array()?;
        let prefixes: Vec<String> = self
            .entries(function)
            .into_iter()
            .map(|(at, _)| format!("{at}/blocks/"))
            .collect();
        entries.iter().find_map(|entry| {
            let expanded = entry.get("expanded")?.as_str()?;
            let role = entry.get("role")?.as_str()?;
            let in_function = prefixes.iter().any(|prefix| {
                expanded
                    .strip_prefix(prefix.as_str())
                    .is_some_and(|rest| !rest.is_empty() && !rest.contains('/'))
            });
            (in_function
                && entry.get("name")?.as_str()? == block
                && matches!(role, "block" | "continuation" | "shared-exit"))
            .then(|| Mapped {
                expanded: expanded.to_owned(),
                authored: entry
                    .get("authored")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                role: role.to_owned(),
            })
        })
    }

    /// The authored pointer and role of an entry under expanded block
    /// `block`: its terminator (`/term`), or its operation named `name`
    /// (`/ops/`).
    fn mapped_at(&self, block: &str, part: &str, name: Option<&str>) -> Option<(String, String)> {
        let entries = self.sourcemap?.get("entries")?.as_array()?;
        let wanted = format!("{block}{part}");
        entries.iter().find_map(|entry| {
            let expanded = entry.get("expanded")?.as_str()?;
            let here = match name {
                None => expanded == wanted,
                Some(name) => {
                    expanded
                        .strip_prefix(wanted.as_str())
                        .is_some_and(|rest| !rest.contains('/'))
                        && entry.get("name")?.as_str()? == name
                }
            };
            here.then(|| {
                Some((
                    entry.get("authored")?.as_str()?.to_owned(),
                    entry.get("role")?.as_str()?.to_owned(),
                ))
            })
            .flatten()
        })
    }
}

/// A generated or authored block in an expanded frame, per the source map.
struct Mapped {
    expanded: String,
    authored: String,
    role: String,
}

/// Escapes one JSON-pointer path segment (RFC 6901).
fn escape(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}
