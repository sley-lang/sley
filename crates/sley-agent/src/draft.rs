//! Persistent drafts: advisory authoring state kept across `try`, `fill`
//! and `import`, under `.sley/drafts/dN/`.
//!
//! A draft revision is identified by its handle, revision number and base
//! head, and spelled `d1@r3`; `d1` alone means the latest revision. Every
//! revision keeps the exact input (`input.txt`), the complete authored frame
//! when the input parsed (`frame.json`), the derived authoring artifacts,
//! and `status.json`: its state (`text`, `incomplete`, `refused`, `valid`),
//! the candidate made from exactly this revision with the digest of its
//! stored bytes, its obligations and its test provenance.
//!
//! Drafts, obligations and deltas are workbench data. They are never
//! admission evidence: the kernel alone judges the candidates.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::catalog::Verdict;
use crate::error::{AgentError, AgentErrorCode, Result, io};
use crate::workspace::Workspace;

/// The drafts directory inside the workbench state directory.
pub const DRAFTS_DIR: &str = "drafts";
/// A draft's summary file.
pub const DRAFT_FILE: &str = "draft.json";
/// The workspace's recording order: `.order/<n>` names the n-th revision
/// recorded, over every draft.
pub const ORDER_DIR: &str = ".order";
/// Obligation lines shown by default; the rest are counted.
pub const SHOWN_OBLIGATIONS: usize = 8;

/// The files of a revision that artifacts may not replace.
const RESERVED: [&str; 3] = ["input.txt", "frame.json", "status.json"];

/// The state of one draft revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    /// The input is not JSON; only its text is kept.
    Text,
    /// The frame parsed but produced no candidate (frame problems, or the
    /// kernel could not build the record).
    Incomplete,
    /// The kernel refused the candidate made from this revision.
    Refused,
    /// The candidate made from this revision is Valid.
    Valid,
}

impl State {
    /// The state as `status.json` spells it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Incomplete => "incomplete",
            Self::Refused => "refused",
            Self::Valid => "valid",
        }
    }

    /// Reads a spelled state.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "text" => Self::Text,
            "incomplete" => Self::Incomplete,
            "refused" => Self::Refused,
            "valid" => Self::Valid,
            _ => return None,
        })
    }
}

/// A draft reference: `d3` (the latest revision) or `d3@r2`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftRef {
    /// The draft handle (`d3`).
    pub handle: String,
    /// The revision, when spelled.
    pub revision: Option<u64>,
}

impl DraftRef {
    /// Parses `dN` or `dN@rK` (`N`, `K` positive).
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (handle, revision) = match text.split_once("@r") {
            Some((handle, revision)) => (handle, Some(positive(revision)?)),
            None => (text, None),
        };
        positive(handle.strip_prefix('d')?)?;
        Some(Self {
            handle: handle.to_owned(),
            revision,
        })
    }
}

fn positive(digits: &str) -> Option<u64> {
    if digits.is_empty() || digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Whether a reference names a draft (`d3`, `d3@r2`).
#[must_use]
pub fn is_draft_ref(text: &str) -> bool {
    DraftRef::parse(text).is_some()
}

/// Spells a revision: `d1@r3`.
#[must_use]
pub fn spell(handle: &str, revision: u64) -> String {
    format!("{handle}@r{revision}")
}

/// Lowercase hex SHA-256 of exact bytes.
#[must_use]
pub fn sha256(bytes: &[u8]) -> String {
    crate::hex::encode(&sley_vm::host_abi::image_digest(bytes))
}

/// One revision to record.
#[derive(Clone, Copy, Debug)]
pub struct Revision<'a> {
    /// The exact input bytes.
    pub input: &'a [u8],
    /// The complete authored frame, when the input parsed.
    pub frame: Option<&'a Value>,
    /// Derived artifacts by file name.
    pub artifacts: &'a [(String, Value)],
    /// `status.json`; its `revision` and `base_head` fields are required.
    pub status: &'a Value,
}

/// The draft store of a workspace.
#[derive(Clone, Debug)]
pub struct Drafts {
    dir: PathBuf,
}

impl Drafts {
    /// Opens (creating) the draft store.
    ///
    /// # Errors
    ///
    /// `AGENT_IO_FAILED` when the directory cannot be created.
    pub fn open(workspace: &Workspace) -> Result<Self> {
        let dir = workspace.state_dir()?.join(DRAFTS_DIR);
        fs::create_dir_all(&dir).map_err(|error| io(&dir, &error))?;
        Ok(Self { dir })
    }

    fn numbers(&self) -> Result<Vec<u64>> {
        let entries = fs::read_dir(&self.dir).map_err(|error| io(&self.dir, &error))?;
        let mut numbers: Vec<u64> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name();
                name.to_str()
                    .and_then(|name| name.strip_prefix('d'))
                    .and_then(positive)
            })
            .collect();
        numbers.sort_unstable();
        Ok(numbers)
    }

    /// Every draft handle with a recorded revision, oldest first.
    ///
    /// # Errors
    ///
    /// `AGENT_IO_FAILED` when the store cannot be listed.
    pub fn handles(&self) -> Result<Vec<String>> {
        Ok(self
            .numbers()?
            .into_iter()
            .map(|number| format!("d{number}"))
            .filter(|handle| self.latest(handle).is_ok())
            .collect())
    }

    /// Claims a new draft and its first revision. The draft directory is
    /// created atomically, so concurrent commands never share a handle.
    ///
    /// # Errors
    ///
    /// `AGENT_IO_FAILED` when the store cannot be written.
    pub fn claim_new(&self) -> Result<Claim> {
        let mut number = self.numbers()?.last().copied().unwrap_or(0) + 1;
        loop {
            let handle = format!("d{number}");
            let dir = self.dir.join(&handle);
            match fs::create_dir(&dir) {
                Ok(()) => {
                    let mut claim = self.claim_at(&handle, 1)?;
                    claim.new_draft = true;
                    return Ok(claim);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => number += 1,
                Err(error) => return Err(io(&dir, &error)),
            }
        }
    }

    /// Claims the next revision of a draft: the first number after its
    /// latest revision that no other command holds.
    ///
    /// # Errors
    ///
    /// `AGENT_HANDLE_UNKNOWN` for an unknown draft, `AGENT_IO_FAILED` when
    /// the store cannot be written.
    pub fn claim_next(&self, handle: &str) -> Result<Claim> {
        let latest = self.latest(handle)?;
        self.claim_at(handle, latest + 1)
    }

    fn claim_at(&self, handle: &str, first: u64) -> Result<Claim> {
        let draft_dir = self.dir.join(handle);
        let mut number = first;
        loop {
            let scratch = draft_dir.join(format!(".r{number}.partial"));
            match fs::create_dir(&scratch) {
                // A number another command already recorded is taken too.
                Ok(()) if self.revision_dir(handle, number).exists() => {
                    let _ = fs::remove_dir(&scratch);
                    number += 1;
                }
                Ok(()) => {
                    return Ok(Claim {
                        draft_dir,
                        handle: handle.to_owned(),
                        number,
                        scratch,
                        new_draft: false,
                        recorded: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => number += 1,
                Err(error) => return Err(io(&scratch, &error)),
            }
        }
    }

    /// The directory of one revision.
    #[must_use]
    pub fn revision_dir(&self, handle: &str, revision: u64) -> PathBuf {
        self.dir.join(handle).join(format!("r{revision}"))
    }

    fn recorded(&self, handle: &str) -> Vec<u64> {
        let mut numbers: Vec<u64> = fs::read_dir(self.dir.join(handle))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name();
                name.to_str()
                    .and_then(|name| name.strip_prefix('r'))
                    .and_then(positive)
            })
            .filter(|number| {
                self.revision_dir(handle, *number)
                    .join("status.json")
                    .is_file()
            })
            .collect();
        numbers.sort_unstable();
        numbers
    }

    /// The latest recorded revision of a draft.
    ///
    /// # Errors
    ///
    /// `AGENT_HANDLE_UNKNOWN` when there is no such draft.
    pub fn latest(&self, handle: &str) -> Result<u64> {
        let known = DraftRef::parse(handle).is_some_and(|reference| reference.revision.is_none());
        self.recorded(handle)
            .last()
            .copied()
            .filter(|_| known)
            .ok_or_else(|| {
                AgentError::new(
                    AgentErrorCode::HandleUnknown,
                    format!("no draft {handle} (sley-agent draft lists them)"),
                )
            })
    }

    /// The revision recorded last in this workspace, over every draft.
    ///
    /// # Errors
    ///
    /// `AGENT_IO_FAILED` when the store cannot be read.
    pub fn last_recorded(&self) -> Result<Option<(String, u64)>> {
        let dir = self.dir.join(ORDER_DIR);
        let last = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| entry.file_name().to_str().and_then(positive))
            .max();
        let Some(last) = last else {
            return Ok(None);
        };
        let path = dir.join(last.to_string());
        let text = fs::read_to_string(&path).map_err(|error| io(&path, &error))?;
        Ok(DraftRef::parse(text.trim())
            .and_then(|reference| Some((reference.handle, reference.revision?))))
    }

    /// The handle and revision a reference names (`d1` is the latest).
    ///
    /// # Errors
    ///
    /// `AGENT_HANDLE_UNKNOWN` when the draft or revision does not exist.
    pub fn resolve(&self, reference: &DraftRef) -> Result<(String, u64)> {
        let latest = self.latest(&reference.handle)?;
        match reference.revision {
            None => Ok((reference.handle.clone(), latest)),
            Some(revision) if self.recorded(&reference.handle).contains(&revision) => {
                Ok((reference.handle.clone(), revision))
            }
            Some(revision) => Err(AgentError::new(
                AgentErrorCode::HandleUnknown,
                format!(
                    "{} has no revision r{revision} (its latest is r{latest})",
                    reference.handle
                ),
            )),
        }
    }

    /// A revision's `status.json`.
    ///
    /// # Errors
    ///
    /// `AGENT_IO_FAILED` when it cannot be read.
    pub fn status(&self, handle: &str, revision: u64) -> Result<Value> {
        read_json(&self.revision_dir(handle, revision).join("status.json"))?.ok_or_else(|| {
            AgentError::new(
                AgentErrorCode::Io,
                format!("{} has no status", spell(handle, revision)),
            )
        })
    }

    /// A revision's authored frame (`None` for a text revision).
    ///
    /// # Errors
    ///
    /// `AGENT_IO_FAILED` when it cannot be read.
    pub fn frame(&self, handle: &str, revision: u64) -> Result<Option<Value>> {
        read_json(&self.revision_dir(handle, revision).join("frame.json"))
    }

    /// A revision's exact input bytes.
    ///
    /// # Errors
    ///
    /// `AGENT_IO_FAILED` when they cannot be read.
    pub fn input(&self, handle: &str, revision: u64) -> Result<Vec<u8>> {
        let path = self.revision_dir(handle, revision).join("input.txt");
        fs::read(&path).map_err(|error| io(&path, &error))
    }

    /// A revision's derived artifacts, by file name.
    ///
    /// # Errors
    ///
    /// `AGENT_IO_FAILED` when the revision cannot be listed.
    pub fn artifacts(&self, handle: &str, revision: u64) -> Result<Vec<(String, Value)>> {
        let dir = self.revision_dir(handle, revision);
        let entries = fs::read_dir(&dir).map_err(|error| io(&dir, &error))?;
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
            .filter(|name| {
                !RESERVED.contains(&name.as_str())
                    && Path::new(name)
                        .extension()
                        .is_some_and(|extension| extension == "json")
            })
            .collect();
        names.sort();
        let mut artifacts = Vec::new();
        for name in names {
            if let Some(value) = read_json(&dir.join(&name))? {
                artifacts.push((name, value));
            }
        }
        Ok(artifacts)
    }

    /// Records a claimed revision: its files are written into the claimed
    /// scratch directory, which is renamed into place; then the draft
    /// summary and the workspace's recording order name it.
    ///
    /// # Errors
    ///
    /// `AGENT_IO_FAILED` when the files cannot be written.
    pub fn record(&self, claim: &mut Claim, revision: &Revision<'_>) -> Result<()> {
        let scratch = &claim.scratch;
        write_file(&scratch.join("input.txt"), revision.input)?;
        if let Some(frame) = revision.frame {
            write_json(&scratch.join("frame.json"), frame)?;
        }
        for (name, value) in revision.artifacts {
            // Artifacts are plain file names beside the fixed files.
            let plain = Path::new(name).file_name().and_then(|n| n.to_str()) == Some(name);
            if plain && !name.starts_with('.') && !RESERVED.contains(&name.as_str()) {
                write_json(&scratch.join(name), value)?;
            }
        }
        write_json(&scratch.join("status.json"), revision.status)?;
        let target = self.revision_dir(&claim.handle, claim.number);
        fs::rename(scratch, &target).map_err(|error| io(&target, &error))?;
        claim.recorded = true;
        let base_head = read_json(&self.revision_dir(&claim.handle, 1).join("status.json"))?
            .and_then(|status| status.get("base_head").cloned())
            .unwrap_or(Value::Null);
        let latest = self.latest(&claim.handle)?;
        let summary = json!({"handle": claim.handle, "latest": latest, "base_head": base_head});
        let mut text = serde_json::to_string_pretty(&summary).unwrap_or_default();
        text.push('\n');
        crate::candidate::replace_file(&claim.draft_dir.join(DRAFT_FILE), text.as_bytes())?;
        let order = self.dir.join(ORDER_DIR);
        fs::create_dir_all(&order).map_err(|error| io(&order, &error))?;
        let first = fs::read_dir(&order)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| entry.file_name().to_str().and_then(positive))
            .max()
            .unwrap_or(0)
            + 1;
        crate::candidate::claim_file(
            &order,
            first,
            |number| number.to_string(),
            spell(&claim.handle, claim.number).as_bytes(),
        )?;
        Ok(())
    }
}

/// A claimed revision slot (`.rN.partial`): recording renames it into place;
/// dropped unrecorded, it is removed with the draft directory it created.
#[derive(Debug)]
pub struct Claim {
    draft_dir: PathBuf,
    handle: String,
    number: u64,
    scratch: PathBuf,
    new_draft: bool,
    recorded: bool,
}

impl Claim {
    /// The draft handle.
    #[must_use]
    pub fn handle(&self) -> &str {
        &self.handle
    }

    /// The claimed revision number.
    #[must_use]
    pub const fn number(&self) -> u64 {
        self.number
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        if !self.recorded {
            let _ = fs::remove_dir_all(&self.scratch);
            if self.new_draft {
                let _ = fs::remove_dir(&self.draft_dir);
            }
        }
    }
}

fn read_json(path: &Path) -> Result<Option<Value>> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map(Some).map_err(|error| {
            AgentError::new(AgentErrorCode::Io, format!("{}: {error}", path.display()))
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io(path, &error)),
    }
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::write(path, bytes).map_err(|error| io(path, &error))
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    let mut text = serde_json::to_string_pretty(value).unwrap_or_default();
    text.push('\n');
    write_file(path, text.as_bytes())
}

/// Where a JSON parse failed: 1-based line and column, 0-based byte.
#[must_use]
pub fn parse_location(bytes: &[u8], error: &serde_json::Error) -> (usize, usize, usize) {
    let (line, column) = (error.line(), error.column());
    let start = if line <= 1 {
        0
    } else {
        bytes
            .iter()
            .enumerate()
            .filter(|(_, byte)| **byte == b'\n')
            .nth(line - 2)
            .map_or(bytes.len(), |(at, _)| at + 1)
    };
    let byte = (start + column.saturating_sub(1)).min(bytes.len());
    (line, column, byte)
}

// ---------------------------------------------------------------------------
// Deltas

/// One validated delta: `{"set": [{"at": pointer, "value": subtree}, ...]}`.
#[derive(Clone, Debug)]
pub struct Delta {
    /// The replacements, in the order given.
    pub set: Vec<(String, Value)>,
}

impl Delta {
    /// Whether a replacement targets the whole frame (`""`).
    #[must_use]
    pub fn whole_frame(&self) -> bool {
        self.set.iter().any(|(at, _)| at.is_empty())
    }

    /// The target pointers.
    #[must_use]
    pub fn targets(&self) -> Vec<String> {
        self.set.iter().map(|(at, _)| at.clone()).collect()
    }
}

fn delta_invalid(detail: impl Into<String>) -> AgentError {
    AgentError::new(AgentErrorCode::DeltaInvalid, detail)
}

const DELTA_SHAPE: &str =
    "a delta is {\"set\": [{\"at\": \"<pointer>\", \"value\": <replacement>}, ...]}";

/// Reads a delta's closed shape.
///
/// # Errors
///
/// `AGENT_DELTA_INVALID` for any other shape or a malformed pointer.
pub fn parse_delta(value: &Value) -> Result<Delta> {
    let object = value
        .as_object()
        .ok_or_else(|| delta_invalid(DELTA_SHAPE))?;
    if let Some(key) = object.keys().find(|key| key.as_str() != "set") {
        return Err(delta_invalid(format!(
            "unknown delta key `{key}`: {DELTA_SHAPE}"
        )));
    }
    let items = object
        .get("set")
        .and_then(Value::as_array)
        .ok_or_else(|| delta_invalid(DELTA_SHAPE))?;
    if items.is_empty() {
        return Err(delta_invalid("the delta sets nothing: \"set\" is empty"));
    }
    let mut set = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let entry = item
            .as_object()
            .ok_or_else(|| delta_invalid(format!("/set/{index}: {DELTA_SHAPE}")))?;
        if let Some(key) = entry
            .keys()
            .find(|key| !matches!(key.as_str(), "at" | "value"))
        {
            return Err(delta_invalid(format!(
                "/set/{index}: unknown key `{key}`; a replacement has exactly \"at\" and \"value\""
            )));
        }
        let at = entry.get("at").and_then(Value::as_str).ok_or_else(|| {
            delta_invalid(format!("/set/{index}: \"at\" is a JSON pointer string"))
        })?;
        let value = entry.get("value").ok_or_else(|| {
            delta_invalid(format!(
                "/set/{index}: \"value\" is missing (the complete replacement subtree)"
            ))
        })?;
        tokens(at).map_err(|detail| delta_invalid(format!("/set/{index}: {detail}")))?;
        set.push((at.to_owned(), value.clone()));
    }
    Ok(Delta { set })
}

/// The reference tokens of an RFC 6901 pointer.
fn tokens(pointer: &str) -> std::result::Result<Vec<String>, String> {
    if pointer.is_empty() {
        return Ok(Vec::new());
    }
    let Some(rest) = pointer.strip_prefix('/') else {
        return Err(format!(
            "`{pointer}` is not a JSON pointer: it is \"\" or starts with /"
        ));
    };
    rest.split('/')
        .map(|token| {
            let mut out = String::with_capacity(token.len());
            let mut chars = token.chars();
            while let Some(c) = chars.next() {
                if c == '~' {
                    match chars.next() {
                        Some('0') => out.push('~'),
                        Some('1') => out.push('/'),
                        _ => return Err(format!("`{pointer}`: `~` is written ~0, and `/` ~1")),
                    }
                } else {
                    out.push(c);
                }
            }
            Ok(out)
        })
        .collect()
}

/// Applies a delta to a revision's frame (`None`: a text revision), all
/// targets resolved against that pre-edit frame.
///
/// # Errors
///
/// `AGENT_DELTA_INVALID` for a missing, repeated or overlapping target, or
/// a target other than `""` on a text revision.
pub fn apply_delta(frame: Option<&Value>, delta: &Delta, revision: &str) -> Result<Value> {
    let parsed: Vec<Vec<String>> = delta
        .set
        .iter()
        .map(|(at, _)| tokens(at).map_err(delta_invalid))
        .collect::<Result<_>>()?;
    for (i, left) in parsed.iter().enumerate() {
        for (j, right) in parsed.iter().enumerate().skip(i + 1) {
            let (a, b) = (&delta.set[i].0, &delta.set[j].0);
            if left == right {
                return Err(delta_invalid(format!(
                    "/set/{j}: `{b}` is replaced twice (also /set/{i}); keep one replacement"
                )));
            }
            if right.starts_with(left) || left.starts_with(right) {
                let (outer, inner) = if left.len() < right.len() {
                    (a, b)
                } else {
                    (b, a)
                };
                return Err(delta_invalid(format!(
                    "/set/{i} `{a}` and /set/{j} `{b}` overlap: `{outer}` contains `{inner}`; replace `{outer}` once with the change included"
                )));
            }
        }
    }
    let Some(frame) = frame else {
        return match delta.set.as_slice() {
            [(at, value)] if at.is_empty() => Ok(value.clone()),
            _ => Err(delta_invalid(format!(
                "{revision} is a text draft (its input is not JSON): replace it whole, {{\"set\": [{{\"at\": \"\", \"value\": <the frame>}}]}}"
            ))),
        };
    };
    let mut next = frame.clone();
    for (index, (at, value)) in delta.set.iter().enumerate() {
        if at.is_empty() {
            next = value.clone();
            continue;
        }
        let slot = next.pointer_mut(at).ok_or_else(|| {
            delta_invalid(format!(
                "/set/{index}: `{at}` does not exist in {revision} (sley-agent draft {} --frame shows it); replace an existing parent to add or remove entries",
                revision.split('@').next().unwrap_or(revision)
            ))
        })?;
        *slot = value.clone();
    }
    Ok(next)
}

// ---------------------------------------------------------------------------
// Obligations

/// One refusal problem line split into symbol, pointer and decision.
struct Problem {
    symbol: String,
    at: Option<String>,
    decision: String,
    expanded: Option<String>,
}

fn problem(line: &str, default_symbol: &str) -> Problem {
    let (symbol, rest) = match line
        .strip_prefix('[')
        .and_then(|rest| rest.split_once("] "))
    {
        Some((symbol, rest)) if symbol.starts_with("AGENT_") => (symbol, rest),
        _ => (default_symbol, line),
    };
    let (at, decision) = if let Some(detail) = rest.strip_prefix(": ") {
        (Some(String::new()), detail)
    } else if rest.starts_with('/') {
        match rest.split_once(": ") {
            Some((pointer, detail)) if !pointer.contains(' ') => (Some(pointer.to_owned()), detail),
            _ => (None, rest),
        }
    } else {
        (None, rest)
    };
    let (decision, expanded) = match decision
        .strip_suffix(']')
        .and_then(|head| head.rsplit_once(" [expanded "))
    {
        Some((decision, pointer)) => (decision, Some(pointer.to_owned())),
        None => (decision, None),
    };
    Problem {
        symbol: symbol.to_owned(),
        at,
        decision: decision.to_owned(),
        expanded,
    }
}

/// The expected type or shape a problem states, when it states one.
fn expected_of(decision: &str) -> Option<String> {
    let cut = |text: &str| {
        let end = text
            .find([';', ',', ')'])
            .into_iter()
            .chain(text.find(" but "))
            .chain(text.find(", got"))
            .min()
            .unwrap_or(text.len());
        let found = text[..end].trim();
        let found = found
            .strip_prefix("an ")
            .or_else(|| found.strip_prefix("a "))
            .unwrap_or(found);
        (!found.is_empty()).then(|| found.to_owned())
    };
    if let Some(at) = decision.find("expected ") {
        return cut(&decision[at + "expected ".len()..]);
    }
    // "`x` is A but parameter `p` of block `b` is T"
    if let Some((_, tail)) = decision.split_once(" but parameter ")
        && let Some((_, ty)) = tail.rsplit_once(" is ")
    {
        return cut(ty);
    }
    // "block `b` takes `c: i64`, and no value named `c` is available here"
    if let Some((_, tail)) = decision.split_once(" takes `")
        && let Some((param, _)) = tail.split_once('`')
        && let Some((_, ty)) = param.split_once(": ")
    {
        return Some(ty.trim().to_owned());
    }
    // "block `b` takes 1 argument(s) (j: i64); this edge passes 0"
    if let Some((_, tail)) = decision.split_once(" argument(s) (")
        && let Some((params, _)) = tail.split_once(')')
    {
        return Some(format!("({params})"));
    }
    None
}

/// The typed values a problem lists as available, when it lists them.
fn available_of(decision: &str) -> Option<Vec<String>> {
    let tail = [
        "available: ",
        "visible: ",
        "values of that type here: ",
        "values of that type: ",
    ]
    .iter()
    .find_map(|marker| {
        decision
            .find(marker)
            .map(|at| &decision[at + marker.len()..])
    })?;
    let end = tail.find(';').unwrap_or(tail.len());
    // A list closing a parenthesized remark ends with that remark's `)`;
    // types may hold parentheses of their own, so only the last one goes.
    let listed = tail[..end].trim_end();
    let listed = if tail[..end].len() == tail.len() && decision.trim_end().ends_with(')') {
        listed.strip_suffix(')').unwrap_or(listed)
    } else {
        listed
    };
    let values: Vec<String> = listed
        .split(", ")
        .map(|value| value.trim().trim_end_matches('.').to_owned())
        .filter(|value| !value.is_empty())
        .collect();
    (!values.is_empty()).then_some(values)
}

/// The obligations of a refusal before any candidate existed: one record
/// per distinct decision, with its count and every pointer.
#[must_use]
pub fn obligations_of(error: &AgentError) -> Vec<Value> {
    let lines = crate::frame::problem_lines(error);
    let mut groups: Vec<(Problem, Vec<Option<String>>)> = Vec::new();
    for line in &lines {
        let problem = problem(line, error.code().symbol());
        if let Some((_, ats)) = groups
            .iter_mut()
            .find(|(seen, _)| seen.symbol == problem.symbol && seen.decision == problem.decision)
        {
            ats.push(problem.at);
            continue;
        }
        let at = problem.at.clone();
        groups.push((problem, vec![at]));
    }
    groups
        .into_iter()
        .enumerate()
        .map(|(index, (problem, ats))| {
            let mut record = json!({
                "id": format!("o{}", index + 1),
                "symbol": problem.symbol,
                "at": ats[0],
                "expected": expected_of(&problem.decision),
                "available": available_of(&problem.decision),
                "decision": problem.decision,
                "kernel": null,
                "count": ats.len(),
            });
            if ats.len() > 1 {
                record["also_at"] = json!(ats[1..]);
            }
            if let Some(expanded) = problem.expanded {
                record["expanded"] = json!(expanded);
            }
            record
        })
        .collect()
}

/// The obligation of a text revision: the parser's location.
#[must_use]
pub fn text_obligation(detail: &str, line: usize, column: usize, byte: usize) -> Value {
    json!({
        "id": "o1",
        "symbol": AgentErrorCode::FrameInvalid.symbol(),
        "at": "",
        "expected": "JSON",
        "available": null,
        "decision": format!("the input is not JSON: {detail} (line {line}, column {column}, byte {byte}); fix the JSON"),
        "kernel": null,
        "count": 1,
    })
}

/// The obligation of a kernel refusal: the kernel's symbol, phase and
/// locator, never reinterpreted. The verdict's authored frame positions, or
/// else `positions` (those of a refused `TestCase`), give `at` (the first)
/// and `also_at` (the others).
#[must_use]
pub fn kernel_obligation(verdict: &Verdict, positions: &[String]) -> Value {
    let symbol = verdict
        .symbol
        .clone()
        .unwrap_or_else(|| verdict.decision.clone());
    let authored: Vec<Value> = verdict
        .to_json()
        .get("authored")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("at").filter(|at| at.is_string()).cloned())
        .collect();
    let authored = if authored.is_empty() {
        positions.iter().map(|at| json!(at)).collect()
    } else {
        authored
    };
    let at = authored.first().cloned();
    let mut decision = format!(
        "{}: {}",
        verdict.headline(),
        verdict.hint.as_deref().unwrap_or("no hint")
    );
    if at
        .as_ref()
        .and_then(Value::as_str)
        .is_some_and(|at| at.starts_with("/test_tables/"))
    {
        decision.push_str(
            "; this test comes from a table row: change the limit in the row's \"limits\" or the table's \"defaults\", or leave it out",
        );
    }
    let mut record = json!({
        "id": "o1",
        "symbol": symbol,
        "at": at,
        "expected": null,
        "available": null,
        "decision": decision,
        "kernel": {
            "symbol": verdict.symbol,
            "phase": verdict.phase,
            "phase_name": verdict.phase.map(crate::catalog::phase_name),
            "where": verdict.location,
        },
        "count": 1,
    });
    if authored.len() > 1 {
        record["also_at"] = json!(authored[1..]);
    }
    record
}

/// The nearest pointer at or above `pointer` that exists in `frame` (a text
/// revision, `None`, has only `""`).
#[must_use]
pub fn existing_pointer(frame: Option<&Value>, pointer: &str) -> String {
    let Some(frame) = frame else {
        return String::new();
    };
    let mut current = pointer;
    while !current.is_empty() && frame.pointer(current).is_none() {
        current = current.rfind('/').map_or("", |cut| &current[..cut]);
    }
    current.to_owned()
}

/// Points every obligation at a pointer that exists in the revision's
/// frame: a missing one moves to its nearest existing ancestor, and the
/// original is kept as `missing`, so the fill an obligation suggests is
/// one `fill` accepts.
pub fn anchor(obligations: &mut [Value], frame: Option<&Value>) {
    for record in obligations {
        if let Some(at) = record["at"].as_str().map(str::to_owned) {
            let existing = existing_pointer(frame, &at);
            if existing != at {
                record["at"] = json!(existing);
                record["missing"] = json!(at);
            }
        }
        if let Some(also) = record.get_mut("also_at").and_then(Value::as_array_mut) {
            for at in also.iter_mut() {
                if let Some(pointer) = at.as_str() {
                    *at = json!(existing_pointer(frame, pointer));
                }
            }
        }
    }
}

/// The authored pointer of an expanded one, by a `sourcemap.json`: the
/// entry with the longest expanded prefix; below it, the same path when the
/// entry keeps the authored shape.
#[must_use]
pub fn authored_pointer(sourcemap: &Value, pointer: &str) -> Option<String> {
    const CARRIES: [&str; 5] = ["block", "op", "term", "checked", "table-row"];
    let mut best: Option<(&str, &str, &str)> = None;
    for entry in sourcemap.get("entries")?.as_array()? {
        let (Some(expanded), Some(authored)) =
            (entry["expanded"].as_str(), entry["authored"].as_str())
        else {
            continue;
        };
        let covers = pointer == expanded
            || (pointer.starts_with(expanded) && pointer[expanded.len()..].starts_with('/'));
        if covers && best.is_none_or(|(known, _, _)| expanded.len() > known.len()) {
            best = Some((expanded, authored, entry["role"].as_str().unwrap_or("")));
        }
    }
    let (expanded, authored, role) = best?;
    Some(if CARRIES.contains(&role) {
        format!("{authored}{}", &pointer[expanded.len()..])
    } else {
        authored.to_owned()
    })
}

/// The number of unresolved problems obligations stand for.
#[must_use]
pub fn obligation_count(obligations: &[Value]) -> u64 {
    obligations
        .iter()
        .map(|record| record["count"].as_u64().unwrap_or(1))
        .sum()
}

/// Obligations per symbol, for a one-line summary: `AGENT_FRAME_INVALID 2`.
#[must_use]
pub fn obligation_symbols(obligations: &[Value]) -> String {
    let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
    for record in obligations {
        *counts
            .entry(record["symbol"].as_str().unwrap_or("?"))
            .or_default() += record["count"].as_u64().unwrap_or(1);
    }
    counts
        .iter()
        .map(|(symbol, count)| format!("{symbol} {count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Renders obligations: at most [`SHOWN_OBLIGATIONS`] lines (each distinct
/// decision once, with its count) and the exact command for the rest, or
/// every obligation and pointer with `all`.
#[must_use]
pub fn obligations_text(obligations: &[Value], handle: &str, all: bool) -> String {
    let total = obligation_count(obligations);
    let mut text = format!("obligations: {total}\n");
    let shown = if all {
        obligations.len()
    } else {
        obligations.len().min(SHOWN_OBLIGATIONS)
    };
    for record in &obligations[..shown] {
        let id = record["id"].as_str().unwrap_or("o?");
        let symbol = record["symbol"].as_str().unwrap_or("?");
        let decision = record["decision"].as_str().unwrap_or("");
        let count = record["count"].as_u64().unwrap_or(1);
        let mut pointers: Vec<String> = std::iter::once(&record["at"])
            .chain(record["also_at"].as_array().into_iter().flatten())
            .filter_map(Value::as_str)
            .map(|at| {
                if at.is_empty() {
                    "\"\"".to_owned()
                } else {
                    at.to_owned()
                }
            })
            .collect();
        let hidden = if all || pointers.len() <= 4 {
            0
        } else {
            let hidden = pointers.len() - 3;
            pointers.truncate(3);
            hidden
        };
        let _ = write!(text, "  {id} {symbol}");
        if count > 1 {
            let _ = write!(text, " ({count}x)");
        }
        if !pointers.is_empty() {
            let _ = write!(text, " at {}", pointers.join(", "));
            if hidden > 0 {
                let _ = write!(text, " and {hidden} more");
            }
        }
        let _ = write!(text, ": {decision}");
        if let Some(expected) = record["expected"].as_str() {
            let _ = write!(text, " [expected {expected}]");
        }
        if let Some(available) = record["available"].as_array() {
            let values: Vec<&str> = available.iter().filter_map(Value::as_str).collect();
            let _ = write!(text, " [available: {}]", values.join(", "));
        }
        if let Some(location) = record["kernel"]["where"].as_str() {
            let _ = write!(text, " [where: {location}]");
        }
        text.push('\n');
    }
    let rest: u64 = obligations[shown..]
        .iter()
        .map(|record| record["count"].as_u64().unwrap_or(1))
        .sum();
    if rest > 0 {
        let _ = writeln!(text, "{rest} more: sley-agent draft {handle} --obligations");
    }
    text
}

// ---------------------------------------------------------------------------
// Test provenance

/// A test entry's content digest, binding an imported test to its source.
#[must_use]
pub fn entry_digest(entry: &Value) -> String {
    sha256(entry.to_string().as_bytes())
}

/// Frame tests by origin: `(imported, authored)`. A frame test (or table
/// row) named in `kept` restates a live test unchanged and counts as
/// provided, not here. Of the others, a test is imported when a source
/// names it and its entry is unchanged since the import; the rest, and
/// every other table row, are authored.
#[must_use]
pub fn frame_tests(frame: &Value, sources: &[Value], kept: &[String]) -> (u64, u64) {
    let is_kept = |name: Option<&str>| name.is_some_and(|name| kept.iter().any(|k| k == name));
    let entries: Vec<&Value> = frame
        .get("tests")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|entry| !is_kept(entry.get("name").and_then(Value::as_str)))
        .collect();
    let imported = entries
        .iter()
        .filter(|entry| is_imported(entry, sources))
        .count() as u64;
    let kept_rows = crate::tables::row_tests(frame)
        .1
        .iter()
        .filter(|row| is_kept(Some(&row.name)))
        .count() as u64;
    (
        imported,
        entries.len() as u64 - imported + table_rows(frame).saturating_sub(kept_rows),
    )
}

/// The rows of a frame's `test_tables`.
#[must_use]
pub fn table_rows(frame: &Value) -> u64 {
    frame
        .get("test_tables")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|table| {
            table
                .get("cases")
                .and_then(Value::as_array)
                .map_or(0, Vec::len) as u64
        })
        .sum()
}

fn is_imported(entry: &Value, sources: &[Value]) -> bool {
    let Some(name) = entry.get("name").and_then(Value::as_str) else {
        return false;
    };
    let digest = entry_digest(entry);
    sources.iter().any(|source| {
        source["test"].as_str() == Some(name) && source["entry"].as_str() == Some(&digest)
    })
}

/// The sources still standing for tests of `frame` (replaced or removed
/// imported tests drop out).
#[must_use]
pub fn live_sources(frame: &Value, sources: &[Value]) -> Vec<Value> {
    let entries = frame
        .get("tests")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    sources
        .iter()
        .filter(|source| {
            entries
                .iter()
                .any(|entry| is_imported(entry, std::slice::from_ref(source)))
        })
        .cloned()
        .collect()
}

/// A JSON object holding only the given keys of `status` (for listings).
#[must_use]
pub fn pick(status: &Value, keys: &[&str]) -> Value {
    let mut out = Map::new();
    for key in keys {
        if let Some(value) = status.get(*key) {
            out.insert((*key).to_owned(), value.clone());
        }
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::{DraftRef, apply_delta, obligations_of, parse_delta};
    use crate::error::{AgentError, AgentErrorCode};
    use serde_json::json;

    #[test]
    fn dialect_scope_problems_carry_their_type_and_values() {
        let decision = "block `next` takes `c: (i64,i64)`, and no value named `c` is available here: pass it explicitly (values of that type here: b: (i64,i64), a: (i64,i64))";
        assert_eq!(
            crate::draft::expected_of(decision).as_deref(),
            Some("(i64,i64)")
        );
        assert_eq!(
            crate::draft::available_of(decision),
            Some(vec!["b: (i64,i64)".to_owned(), "a: (i64,i64)".to_owned()])
        );
    }

    #[test]
    fn references_parse() {
        assert_eq!(
            DraftRef::parse("d12@r3"),
            Some(DraftRef {
                handle: "d12".into(),
                revision: Some(3)
            })
        );
        assert_eq!(DraftRef::parse("d1").unwrap().revision, None);
        for bad in ["d0", "d", "c1", "d1@r0", "d1@", "d01", "d1@r"] {
            assert!(DraftRef::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn deltas_replace_existing_disjoint_targets_only() {
        let frame = json!({"af1": 1, "fns": [{"fn": "f", "blocks": [{"term": ["return", "a"]}]}]});
        let delta = parse_delta(
            &json!({"set": [{"at": "/fns/0/blocks/0/term", "value": ["return", "b"]}]}),
        )
        .unwrap();
        let next = apply_delta(Some(&frame), &delta, "d1@r1").unwrap();
        assert_eq!(next["fns"][0]["blocks"][0]["term"], json!(["return", "b"]));
        let code = |delta| {
            parse_delta(&delta)
                .and_then(|delta| apply_delta(Some(&frame), &delta, "d1@r1"))
                .unwrap_err()
                .code()
        };
        for delta in [
            json!({"set": [{"at": "/fns/1", "value": 1}]}),
            json!({"set": [{"at": "/fns/0", "value": 1}, {"at": "/fns/0/fn", "value": "g"}]}),
            json!({"set": [{"at": "/af1", "value": 1}, {"at": "/af1", "value": 1}]}),
            json!({"set": [{"at": "", "value": {}}, {"at": "/af1", "value": 1}]}),
            json!({"set": [{"at": "/af1", "value": 1, "why": "x"}]}),
            json!({"set": [], "more": 1}),
            json!({"set": [{"at": "af1", "value": 1}]}),
            json!({"set": [{"at": "/a~2", "value": 1}]}),
        ] {
            assert_eq!(code(delta), AgentErrorCode::DeltaInvalid);
        }
        // `/fns/1` and `/fns/10` do not overlap; a missing one is refused.
        let delta = parse_delta(
            &json!({"set": [{"at": "/fns/1", "value": 1}, {"at": "/fns/10", "value": 2}]}),
        )
        .unwrap();
        assert!(apply_delta(Some(&frame), &delta, "d1@r1").is_err());
        let text = parse_delta(&json!({"set": [{"at": "", "value": {"af1": 1}}]})).unwrap();
        assert_eq!(
            apply_delta(None, &text, "d1@r1").unwrap(),
            json!({"af1": 1})
        );
        let partial = parse_delta(&json!({"set": [{"at": "/af1", "value": 1}]})).unwrap();
        assert!(apply_delta(None, &partial, "d1@r1").is_err());
    }

    #[test]
    fn problem_lines_become_grouped_obligations() {
        let error = AgentError::new(
            AgentErrorCode::FrameInvalid,
            "/fns/0/blocks/0/ops/1: `d` is odd (1 of 4 problems)\n  /fns/1/blocks/0/ops/1: `d` is odd\n  [AGENT_X_SCOPE] /fns/2: no value `total`; available: a: i64, b: i64\n  /tests/0/args: expected an array",
        );
        let obligations = obligations_of(&error);
        assert_eq!(obligations.len(), 3);
        assert_eq!(obligations[0]["count"], 2);
        assert_eq!(obligations[0]["also_at"], json!(["/fns/1/blocks/0/ops/1"]));
        assert_eq!(obligations[1]["symbol"], "AGENT_X_SCOPE");
        assert_eq!(obligations[1]["available"], json!(["a: i64", "b: i64"]));
        assert_eq!(obligations[2]["expected"], "array");
        assert_eq!(obligations[2]["id"], "o3");
    }
}
