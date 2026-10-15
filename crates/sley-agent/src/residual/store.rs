//! Immutable local residual records. Digests are integrity checks, not authority.

use std::fs::{self, File};
use std::io::Read;

use serde_json::{Value, json};

use super::binding::{MAX_BOUND_ARTIFACT_BYTES, canonical_digest};
use crate::error::{AgentError, AgentErrorCode, Result, io};
use crate::workspace::Workspace;

const MAX_RECORDS: u64 = 10_000;

/// Publishes a complete immutable record through the existing atomic allocator.
pub(crate) fn save(workspace: &Workspace, record: &Value) -> Result<String> {
    let digest = canonical_digest("residual-record-v1", record)?;
    let bytes = serde_json::to_vec(&json!({"record":record,"digest":digest})).expect("JSON record");
    // Apply the same bounds before publishing that inspection will enforce.
    super::strict_json_with_limit(&bytes, MAX_BOUND_ARTIFACT_BYTES)?;
    let dir = workspace.state_dir()?.join("residual");
    fs::create_dir_all(&dir).map_err(|error| io(&dir, &error))?;
    let last = fs::read_dir(&dir)
        .map_err(|error| io(&dir, &error))?
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| {
            entry.file_name().to_str().and_then(|name| {
                name.strip_prefix('r')?
                    .strip_suffix(".json")?
                    .parse::<u64>()
                    .ok()
            })
        })
        .max()
        .unwrap_or(0);
    if last >= MAX_RECORDS {
        return Err(AgentError::new(
            AgentErrorCode::ResidualLimit,
            "workspace residual record bound reached",
        ));
    }
    let number = crate::candidate::claim_file_bounded(
        &dir,
        last + 1,
        MAX_RECORDS,
        |number| format!("r{number}.json"),
        &bytes,
    )?;
    Ok(format!("r{number}@1"))
}

pub(crate) fn is_handle(handle: &str) -> bool {
    number(handle).is_some()
}

/// Atomically allows one trial per immutable plan. A terminal or interrupted
/// attempt requires explicit replanning, never implicit lock reclamation.
pub(crate) fn claim_fill(workspace: &Workspace, handle: &str, input: &[u8]) -> Result<()> {
    let number = number(handle)
        .ok_or_else(|| AgentError::new(AgentErrorCode::ResidualParse, "invalid plan handle"))?;
    let dir = workspace.state_dir()?.join("residual/fills");
    fs::create_dir_all(&dir).map_err(|error| io(&dir, &error))?;
    let name = format!("r{number}.json");
    let bytes = serde_json::to_vec(
        &json!({"plan":handle,"fill":super::binding::digest_bytes("residual-fill-v1",input)}),
    )
    .expect("fill record");
    crate::candidate::claim_file_bounded(&dir, 1, 1, |_| name.clone(), &bytes).map_err(
        |error| {
            if dir.join(&name).exists() {
                AgentError::new(
                    AgentErrorCode::ResidualBindingStale,
                    "plan fill already started; inspect its drafts or explicitly replan",
                )
            } else {
                error
            }
        },
    )?;
    Ok(())
}

fn number(handle: &str) -> Option<u64> {
    let digits = handle.strip_prefix('r')?.strip_suffix("@1")?;
    if digits.starts_with('0')
        || digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    digits.parse().ok()
}

pub(crate) fn load(workspace: &Workspace, handle: &str) -> Result<Value> {
    let number = number(handle).ok_or_else(|| {
        AgentError::new(
            AgentErrorCode::ResidualParse,
            "residual record reference must be rN@1",
        )
    })?;
    let path = workspace
        .dir()
        .join(".sley/residual")
        .join(format!("r{number}.json"));
    let file = File::open(&path).map_err(|error| io(&path, &error))?;
    let mut bytes = Vec::new();
    file.take(MAX_BOUND_ARTIFACT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io(&path, &error))?;
    let envelope = super::strict_json_with_limit(&bytes, MAX_BOUND_ARTIFACT_BYTES)?;
    if envelope.as_object().is_none_or(|object| {
        object.len() != 2 || !object.contains_key("record") || !object.contains_key("digest")
    }) || envelope["digest"].as_str()
        != Some(canonical_digest("residual-record-v1", &envelope["record"])?.as_str())
    {
        return Err(AgentError::new(
            AgentErrorCode::ResidualBindingStale,
            "local residual record integrity differs",
        ));
    }
    Ok(envelope["record"].clone())
}
