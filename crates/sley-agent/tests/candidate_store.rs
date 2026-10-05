//! Candidate-file recovery and allocation boundaries. Payload validity belongs
//! to the kernel; these tests exercise the store's opaque byte preservation.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;
use sley_agent::candidate::Store;
use sley_agent::error::AgentErrorCode;
use sley_agent::workspace::Workspace;

struct TempStore {
    path: PathBuf,
    store: Store,
}

impl TempStore {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "sley-candidate-store-{}-{stamp}",
            std::process::id()
        ));
        let store = Store::open(&Workspace::at(&path)).unwrap();
        Self { path, store }
    }

    fn file(&self, name: &str) -> PathBuf {
        self.path.join(".sley/candidates").join(name)
    }
}

impl Drop for TempStore {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

const FIRST: &[u8] = b"SLEYCAN1first-storage-payload";
const SECOND: &[u8] = b"SLEYCAN1second-storage-payload";

#[test]
fn metadata_publication_failure_preserves_bytes_and_reserves_the_handle() {
    let temp = TempStore::new();
    // A directory cannot be replaced by the metadata-file rename. This
    // deterministically fails after the complete candidate has been claimed.
    fs::create_dir(temp.file("c1.json")).unwrap();
    let error = temp
        .store
        .save(FIRST, &json!({"owner": "first"}))
        .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::Io);
    fs::remove_dir(temp.file("c1.json")).unwrap();

    // Reopen as a subsequent command would: no in-memory state is needed.
    let recovered = Store::open(&Workspace::at(&temp.path)).unwrap();
    assert_eq!(recovered.latest().unwrap().as_deref(), Some("c1"));
    assert_eq!(recovered.load("c1").unwrap(), FIRST);
    assert_eq!(recovered.meta("c1"), None);
    let meta = json!({"owner": "second"});
    assert_eq!(recovered.save(SECOND, &meta).unwrap(), "c2");
    assert_eq!(recovered.load("c1").unwrap(), FIRST);
    assert_eq!(recovered.meta("c1"), None);
    assert_eq!(recovered.load("c2").unwrap(), SECOND);
    assert_eq!(recovered.meta("c2"), Some(meta));
    assert_eq!(fs::read_dir(temp.file("")).unwrap().count(), 3);
}

#[test]
fn last_handle_is_allocated_once_then_exhaustion_preserves_existing_records() {
    let temp = TempStore::new();
    let first_meta = json!({"owner": "first"});
    assert_eq!(temp.store.save(FIRST, &first_meta).unwrap(), "c1");
    let previous = format!("c{}", u64::MAX - 1);
    for extension in ["hex", "json"] {
        fs::rename(
            temp.file(&format!("c1.{extension}")),
            temp.file(&format!("{previous}.{extension}")),
        )
        .unwrap();
    }
    let last = format!("c{}", u64::MAX);
    let last_meta = json!({"owner": "last"});
    assert_eq!(temp.store.save(SECOND, &last_meta).unwrap(), last);
    let error = temp
        .store
        .save(FIRST, &json!({"owner": "overflow"}))
        .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::Io);
    assert_eq!(temp.store.latest().unwrap(), Some(last.clone()));
    assert_eq!(temp.store.load(&previous).unwrap(), FIRST);
    assert_eq!(temp.store.meta(&previous), Some(first_meta));
    assert_eq!(temp.store.load(&last).unwrap(), SECOND);
    assert_eq!(temp.store.meta(&last), Some(last_meta));
    assert_eq!(fs::read_dir(temp.file("")).unwrap().count(), 4);
}
