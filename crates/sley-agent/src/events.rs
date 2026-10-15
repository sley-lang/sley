//! The events ledger: one JSON line per workbench command, appended to
//! `.sley/events.jsonl`, for attributing authoring effort (input and output
//! sizes, whole-frame rewrites, delta sizes, authoring feature use, test
//! provenance, refusals, draft completion).
//!
//! A line carries counts, handles and symbols only: no clock, no names, no
//! frame or program content. Each line takes the next `seq` under an
//! exclusive lock on the ledger, so numbers are unique and follow file
//! order even for concurrent commands. Appending is best effort; it never
//! fails or changes the command.

use std::fs::{self, OpenOptions};
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::path::PathBuf;

use serde_json::{Map, Value, json};

use crate::workspace::{REPO_DIR, STATE_DIR};

/// The ledger file in the workbench state directory.
pub const EVENTS_FILE: &str = "events.jsonl";

/// Most authoring counters one line carries.
const MAX_STATS: usize = 32;
/// Longest symbol a line carries.
const MAX_SYMBOL: usize = 64;

/// The counters one command reports, filled while it runs.
#[derive(Clone, Debug, Default)]
pub struct Event {
    workspace: Option<PathBuf>,
    fields: Map<String, Value>,
}

impl Event {
    /// Records the workspace the command used (the ledger's home).
    pub fn at(&mut self, dir: PathBuf) {
        self.workspace = Some(dir);
    }

    /// Sets one counter.
    pub fn set(&mut self, key: &str, value: impl Into<Value>) {
        self.fields.insert(key.to_owned(), value.into());
    }

    /// The ledger line: a fixed key set, with defaults for the counters the
    /// command did not report. Unknown keys never reach the line.
    #[must_use]
    pub fn line(&self, seq: u64, command: &str, output_bytes: usize) -> Value {
        let field = |key: &str, default: Value| self.fields.get(key).cloned().unwrap_or(default);
        let stats: Map<String, Value> = self
            .fields
            .get("afx")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter(|(_, value)| value.is_number() || value.is_boolean())
            .take(MAX_STATS)
            .map(|(key, value)| (key.chars().take(MAX_SYMBOL).collect(), value.clone()))
            .collect();
        let refusal = self
            .fields
            .get("refusal")
            .and_then(Value::as_str)
            .map(|symbol| symbol.chars().take(MAX_SYMBOL).collect::<String>());
        let tests = self
            .fields
            .get("tests")
            .filter(|value| value.is_object())
            .map(|tests| {
                json!({
                    "provided": tests["provided"].as_u64().unwrap_or(0),
                    "imported": tests["imported"].as_u64().unwrap_or(0),
                    "authored": tests["authored"].as_u64().unwrap_or(0),
                })
            });
        let mut line = json!({
            "seq": seq,
            "cmd": command.chars().take(MAX_SYMBOL).collect::<String>(),
            "draft": field("draft", Value::Null),
            "candidate": field("candidate", Value::Null),
            "input_bytes": field("input_bytes", json!(0)),
            "output_bytes": output_bytes,
            "whole_frame": field("whole_frame", json!(false)),
            "rewrite": field("rewrite", json!(false)),
            "delta_targets": field("delta_targets", json!(0)),
            "delta_bytes": field("delta_bytes", json!(0)),
            "afx": stats,
            "table_rows": field("table_rows", json!(0)),
            "tests": tests,
            "refusal": refusal,
            "obligations": field("obligations", json!(0)),
            "valid": field("valid", Value::Null),
        });
        if let Some(stats) = self.fields.get("residual").and_then(Value::as_object) {
            let stats: Map<String, Value> = stats
                .iter()
                .filter(|(_, value)| value.is_number() || value.is_boolean())
                .take(MAX_STATS)
                .map(|(key, value)| (key.chars().take(MAX_SYMBOL).collect(), value.clone()))
                .collect();
            line["residual"] = Value::Object(stats);
        }
        line
    }
}

/// Appends the command's line to its workspace's ledger. Commands that
/// used no workspace (help, version) and directories that are not
/// workspaces get no line; a failure to append is ignored.
pub fn append(event: &Event, command: &str, output_bytes: usize) {
    let Some(dir) = &event.workspace else {
        return;
    };
    if !dir.join(REPO_DIR).is_dir() && !dir.join(STATE_DIR).is_dir() {
        return;
    }
    let state = dir.join(STATE_DIR);
    if fs::create_dir_all(&state).is_err() {
        return;
    }
    let path = state.join(EVENTS_FILE);
    let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .read(true)
        .open(&path)
    else {
        return;
    };
    // The sequence number is read and the line appended under an exclusive
    // lock on the ledger, so concurrent commands number their lines once
    // each, in file order. Without the lock (unsupported), the append goes
    // ahead unlocked.
    let locked = file.lock().is_ok();
    let seq = count_lines(&mut file) + 1;
    let mut text = event.line(seq, command, output_bytes).to_string();
    text.push('\n');
    let _ = file.write_all(text.as_bytes());
    if locked {
        let _ = file.unlock();
    }
}

/// The number of complete lines in the ledger.
fn count_lines(file: &mut fs::File) -> u64 {
    let mut bytes = Vec::new();
    if file.seek(SeekFrom::Start(0)).is_err() || file.read_to_end(&mut bytes).is_err() {
        return 0;
    }
    bytes.split(|byte| *byte == b'\n').count() as u64 - 1
}
