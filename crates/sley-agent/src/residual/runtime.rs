//! Actual running-build and authoring-contract identity for residual bindings.

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::OnceLock;

use sha2::{Digest, Sha256};

use super::binding::RuntimeIdentity;
use crate::error::{AgentError, AgentErrorCode, Result, io};

/// Cache only the immutable identity of this process, never workspace state.
pub(crate) fn identity() -> Result<RuntimeIdentity> {
    static IDENTITY: OnceLock<Result<RuntimeIdentity>> = OnceLock::new();
    IDENTITY.get_or_init(capture).clone()
}

fn capture() -> Result<RuntimeIdentity> {
    // Linux's process handle continues to identify the loaded executable when
    // its installation path is replaced while this process is alive.
    let executable = if cfg!(target_os = "linux") {
        PathBuf::from("/proc/self/exe")
    } else {
        std::env::current_exe().map_err(|error| io(std::path::Path::new("<executable>"), &error))?
    };
    let mut file = File::open(&executable).map_err(|error| io(&executable, &error))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    let mut bytes = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| io(&executable, &error))?;
        if read == 0 {
            break;
        }
        bytes += read as u64;
        if bytes > 512 * 1024 * 1024 {
            return Err(AgentError::new(
                AgentErrorCode::ResidualLimit,
                "running executable exceeds the 512 MiB identity-read bound",
            ));
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    // These checked-in contracts are compiled into this executable. The build
    // digest additionally binds all implementation/dependency changes.
    let grammar = [
        include_bytes!("../../data/help-af1.md").as_slice(),
        include_bytes!("../../data/help-afx.md").as_slice(),
        include_bytes!("../../data/help-types.md").as_slice(),
        include_bytes!("../residual.rs").as_slice(),
    ]
    .into_iter()
    .map(crate::draft::sha256)
    .collect::<Vec<_>>()
    .join(":");
    RuntimeIdentity::new(
        &serde_json::to_vec(&super::fragments::manifest()).expect("JSON manifest"),
        grammar.as_bytes(),
        &digest,
        "sley-epoch-1/af1-1/afx-1/residual-1",
        Some("disclosed-field-json-bytes-v1"),
        None,
    )
}
