//! Read-only snapshots for residual plans. Bindings are integrity records,
//! never authority, kernel judgments, or locks against later mutation.

mod source;
pub use source::DraftSource;

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use serde_json::{Value, json};

use super::{BaseRef, request_from_value, strict_json, strict_json_with_limit};
use crate::draft::{self, Drafts, State};
use crate::error::{AgentError, AgentErrorCode, Result, io};
use crate::hex;
use crate::names::NameMap;
use crate::workspace::{Head, NAMES_FILE, STATE_DIR, Workspace};

/// Maximum bytes read from any individual bound local artifact.
pub const MAX_BOUND_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;

/// Exact implementation inputs supplied by the workbench's fragment registry.
/// These values come from the running implementation, never the request.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeIdentity {
    value: Value,
}

impl RuntimeIdentity {
    /// Binds manifests, the actual expander build identity, and versioned
    /// semantic/cost/tokenizer profiles. Absent optional profiles are explicit.
    ///
    /// # Errors
    ///
    /// Refuses empty required material or empty optional version identifiers.
    pub fn new(
        fragment_manifest: &[u8],
        grammar_manifest: &[u8],
        expander_build: &[u8],
        semantic_profile: &str,
        cost_model: Option<&str>,
        tokenizer: Option<&str>,
    ) -> Result<Self> {
        if fragment_manifest.is_empty()
            || grammar_manifest.is_empty()
            || expander_build.is_empty()
            || semantic_profile.is_empty()
            || cost_model.is_some_and(str::is_empty)
            || tokenizer.is_some_and(str::is_empty)
        {
            return Err(super::parse_error("runtime identities must not be empty"));
        }
        Ok(Self {
            value: json!({
                "fragments": digest_bytes("fragment-manifest", fragment_manifest),
                "grammar": digest_bytes("grammar-manifest", grammar_manifest),
                "expander_build": digest_bytes("expander-build", expander_build),
                "semantic_profile": semantic_profile,
                "cost_model": cost_model,
                "tokenizer": tokenizer,
            }),
        })
    }
}

/// A captured request, workspace, and implementation snapshot.
///
/// All fields are obtained by capture; callers cannot turn arbitrary JSON into
/// a captured binding. Persisted plan loading must recapture and compare it.
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    snapshot: Value,
    digest: String,
}

impl Binding {
    /// Captures a strict request against an existing accepted head and explicit
    /// draft revision, without seeding or creating local files.
    ///
    /// Two observations must agree. This detects changes during capture; the
    /// existing kernel/revision checks still arbitrate changes after capture.
    ///
    /// # Errors
    ///
    /// Refuses malformed requests, missing state, stale roots/revisions, corrupt
    /// recorded draft data, or a snapshot that changes while being read.
    pub fn capture(
        workspace: &Workspace,
        request: &[u8],
        runtime: &RuntimeIdentity,
    ) -> Result<Self> {
        let value = strict_json(request)?;
        let parsed = request_from_value(value.clone())?;
        let first = observe(workspace, &parsed.base)?;
        let second = observe(workspace, &parsed.base)?;
        if first != second {
            return Err(stale(
                "workspace dependencies changed during capture; replan",
            ));
        }
        let snapshot = json!({
            "binding_version": 1,
            "request": canonical_digest("request", &value)?,
            "workspace": second,
            "runtime": runtime.value,
        });
        let digest = canonical_digest("plan-binding", &snapshot)?;
        Ok(Self { snapshot, digest })
    }

    /// The full domain-separated binding digest, never a capability.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// The complete inspectable snapshot used to derive the digest.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({"digest": self.digest, "snapshot": self.snapshot})
    }

    /// Re-observes all dependencies immediately before expansion or candidate
    /// creation. A changed request or implementation also requires replanning.
    ///
    /// # Errors
    ///
    /// `AGENT_RESIDUAL_BINDING_STALE` if the snapshot differs or its dependencies
    /// can no longer be read. No rebase or workspace write is performed.
    pub fn recheck(
        &self,
        workspace: &Workspace,
        request: &[u8],
        runtime: &RuntimeIdentity,
    ) -> Result<()> {
        let current = Self::capture(workspace, request, runtime)
            .map_err(|error| stale(format!("bound state unavailable: {error}")))?;
        if *self != current {
            return Err(stale(
                "request, workspace, draft, names, or implementation changed; replan",
            ));
        }
        Ok(())
    }
}

fn observe(workspace: &Workspace, base: &BaseRef) -> Result<Value> {
    let directory =
        fs::canonicalize(workspace.dir()).map_err(|error| io(workspace.dir(), &error))?;
    let head = workspace.read_head()?;
    if let BaseRef::AcceptedRoot(root) = base
        && root != head.state_root().root.as_bytes()
    {
        return Err(stale("/base is not the current accepted state root"));
    }
    let draft = match base {
        BaseRef::Draft(reference) => {
            let revision = reference
                .revision
                .ok_or_else(|| super::parse_error("/base requires an explicit draft revision"))?;
            let drafts = Drafts::read_only(workspace);
            if drafts.latest(&reference.handle)? != revision {
                return Err(stale(format!(
                    "{} is no longer the latest draft revision",
                    draft::spell(&reference.handle, revision)
                )));
            }
            let dir = drafts.revision_dir(&reference.handle, revision);
            Some(draft_snapshot(
                workspace,
                &head,
                &dir,
                &reference.handle,
                revision,
            )?)
        }
        BaseRef::AcceptedCurrent | BaseRef::AcceptedRoot(_) => None,
    };
    Ok(json!({
        // WorkspaceId binds semantic identity. The canonical local directory
        // additionally prevents replay of copied handles in another checkout.
        "directory": hex::encode(directory.as_os_str().as_encoded_bytes()),
        "workspace_id": hex::encode(head.workspace().as_bytes()),
        "accepted_head": hex::encode(head.transaction_id().as_bytes()),
        "accepted_root": hex::encode(head.state_root().root.as_bytes()),
        "policy_root": hex::encode(head.policy_root().root().as_bytes()),
        "schema_epoch": hex::encode(head.epoch().as_bytes()),
        "names": name_snapshot(workspace)?,
        "draft": draft,
    }))
}

fn name_snapshot(workspace: &Workspace) -> Result<Value> {
    Ok(read_names(workspace)?.1)
}

fn read_names(workspace: &Workspace) -> Result<(NameMap, Value)> {
    let paths = [
        workspace.dir().join(NAMES_FILE),
        workspace.dir().join(STATE_DIR).join(NAMES_FILE),
    ];
    let mut map = NameMap::default();
    let mut files = Vec::new();
    for path in &paths {
        match read_optional(path)? {
            Some(bytes) => {
                let value = strict_json_with_limit(&bytes, MAX_BOUND_ARTIFACT_BYTES)?;
                map.extend(&NameMap::from_value(&value, path)?);
                files.push(json!(digest_bytes("name-map-file", &bytes)));
            }
            None => files.push(Value::Null),
        }
    }
    let snapshot = json!({"effective": map.digest(), "files": files});
    Ok((map, snapshot))
}

fn draft_snapshot(
    workspace: &Workspace,
    head: &Head,
    directory: &Path,
    handle: &str,
    revision: u64,
) -> Result<Value> {
    let status_bytes = read_required(&directory.join("status.json"))?;
    let status = strict_json_with_limit(&status_bytes, MAX_BOUND_ARTIFACT_BYTES)?;
    if status["revision"].as_u64() != Some(revision) {
        return Err(stale("draft status does not name the selected revision"));
    }
    if status["base_head"].as_str() != Some(hex::encode(head.transaction_id().as_bytes()).as_str())
    {
        return Err(stale(
            "draft was authored against another accepted head; replan after an explicit rebase",
        ));
    }
    let state = status["state"].as_str().and_then(State::parse);
    if state.is_none() || state == Some(State::Text) || status["unlayered"] == true {
        return Err(AgentError::new(
            AgentErrorCode::DraftIncomplete,
            "the selected revision has no layered authoring frame",
        ));
    }
    let frame_bytes = read_required(&directory.join("frame.json"))?;
    let frame = strict_json_with_limit(&frame_bytes, MAX_BOUND_ARTIFACT_BYTES)?;
    if !frame.is_object() {
        return Err(AgentError::new(
            AgentErrorCode::DraftIncomplete,
            "the selected draft frame is not an authoring object",
        ));
    }
    // Incomplete frames remain usable for repair. A recorded candidate, when
    // present, must still be exactly the bytes the revision recorded.
    let candidate = match status.get("candidate") {
        None | Some(Value::Null) if state == Some(State::Incomplete) => Value::Null,
        Some(Value::String(candidate)) if crate::candidate::is_handle(candidate) => {
            let path = workspace
                .dir()
                .join(STATE_DIR)
                .join("candidates")
                .join(format!("{candidate}.hex"));
            let bytes = read_required(&path)?;
            let text = std::str::from_utf8(&bytes)
                .map_err(|_| stale("recorded candidate is not UTF-8 hex"))?;
            let stored = hex::decode(text.trim())
                .ok_or_else(|| stale("recorded candidate has malformed stored hex"))?;
            let digest = draft::sha256(&stored);
            if status["candidate_sha256"].as_str() != Some(digest.as_str()) {
                return Err(stale(
                    "recorded candidate bytes differ from the draft receipt",
                ));
            }
            json!({"handle": candidate, "sha256": digest})
        }
        _ => return Err(stale("draft candidate receipt is missing or malformed")),
    };
    let input = read_required(&directory.join("input.txt"))?;
    Ok(json!({
        "handle": handle,
        "revision": revision,
        "status": digest_bytes("draft-status", &status_bytes),
        "frame": digest_bytes("draft-frame", &frame_bytes),
        "input": digest_bytes("draft-input", &input),
        "candidate": candidate,
    }))
}

fn read_required(path: &Path) -> Result<Vec<u8>> {
    read_optional(path)?
        .ok_or_else(|| stale(format!("bound artifact is missing: {}", path.display())))
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io(path, &error)),
    };
    if !metadata.is_file() {
        return Err(stale(format!(
            "bound artifact is not a regular file: {}",
            path.display()
        )));
    }
    if metadata.len() > MAX_BOUND_ARTIFACT_BYTES as u64 {
        return Err(AgentError::new(
            AgentErrorCode::ResidualLimit,
            "bound artifact exceeds 16 MiB",
        ));
    }
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| {
            file.take(MAX_BOUND_ARTIFACT_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
        })
        .map_err(|error| io(path, &error))?;
    if bytes.len() > MAX_BOUND_ARTIFACT_BYTES {
        return Err(AgentError::new(
            AgentErrorCode::ResidualLimit,
            "bound artifact grew past 16 MiB",
        ));
    }
    Ok(Some(bytes))
}

fn stale(detail: impl Into<String>) -> AgentError {
    AgentError::new(AgentErrorCode::ResidualBindingStale, detail)
}

/// Domain and payload are individually length-prefixed before SHA-256.
pub(crate) fn digest_bytes(domain: &str, payload: &[u8]) -> String {
    let mut bytes = b"sley.ghostweave.integrity.v1\0".to_vec();
    put_bytes(&mut bytes, domain.as_bytes());
    put_bytes(&mut bytes, payload);
    draft::sha256(&bytes)
}

/// Typed canonical encoding: n/f/t for null and booleans; i/s for decimal
/// integers and UTF-8 strings; a/o plus u64 counts for arrays and objects.
/// Byte strings have u64 big-endian lengths. Object keys are sorted by UTF-8.
/// Integers use shortest decimal form. Floats have no encoding.
pub(crate) fn canonical_digest(domain: &str, value: &Value) -> Result<String> {
    let mut encoded = Vec::new();
    encode_value(&mut encoded, value)?;
    Ok(digest_bytes(domain, &encoded))
}

fn put_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    output.extend_from_slice(bytes);
}

fn encode_value(output: &mut Vec<u8>, value: &Value) -> Result<()> {
    match value {
        Value::Null => output.push(b'n'),
        Value::Bool(false) => output.push(b'f'),
        Value::Bool(true) => output.push(b't'),
        Value::Number(number) => {
            if !number.is_i64() && !number.is_u64() {
                return Err(super::parse_error(
                    "floats have no residual canonical encoding",
                ));
            }
            output.push(b'i');
            put_bytes(output, number.to_string().as_bytes());
        }
        Value::String(text) => {
            output.push(b's');
            put_bytes(output, text.as_bytes());
        }
        Value::Array(values) => {
            output.push(b'a');
            output.extend_from_slice(&(values.len() as u64).to_be_bytes());
            for item in values {
                encode_value(output, item)?;
            }
        }
        Value::Object(object) => {
            output.push(b'o');
            output.extend_from_slice(&(object.len() as u64).to_be_bytes());
            let mut members: Vec<_> = object.iter().collect();
            members.sort_unstable_by(|(a, _), (b, _)| a.as_bytes().cmp(b.as_bytes()));
            for (key, item) in members {
                put_bytes(output, key.as_bytes());
                encode_value(output, item)?;
            }
        }
    }
    Ok(())
}
