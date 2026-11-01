//! `sley-agent bindgen <file | -> --rust | --typescript`: typed bindings for
//! a familiar program ([`crate::embed::bindgen`]), compiled as
//! [`crate::embed::compile_python`] compiles it. It uses no workspace and
//! writes nothing but its output: the text, or with `--json`
//! `{"language", "text"}`.

use std::io::Write;

use serde_json::json;

use super::{EXIT_OK, Global, read_familiar, words, write_json, write_text};
use crate::error::{Result, usage};

const USAGE: &str = "bindgen <file | -> --rust | --typescript";

pub(super) fn command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = words(args, &[], &["--rust", "--typescript"])?;
    let ([file], rust, typescript) = (
        words.positional.as_slice(),
        words.has("--rust"),
        words.has("--typescript"),
    ) else {
        return Err(usage(USAGE));
    };
    if rust == typescript {
        return Err(usage(format!(
            "give one of --rust and --typescript: {USAGE}"
        )));
    }
    if file != "-" && !std::path::Path::new(file).is_file() {
        return Err(usage(format!(
            "bindgen reads a file or - (standard input); {file} is neither"
        )));
    }
    let bytes = read_familiar(file)?;
    let text = generate(file, &bytes, rust)?;
    if global.json {
        let language = if rust { "rust" } else { "typescript" };
        write_json(out, &json!({"language": language, "text": text}))?;
    } else {
        write_text(out, &text)?;
    }
    Ok(EXIT_OK)
}

#[cfg(feature = "familiar")]
fn generate(file: &str, bytes: &[u8], rust: bool) -> Result<String> {
    use crate::embed::{EmbedOptions, bindgen, compile_python};
    use crate::error::{AgentError, AgentErrorCode};
    let source =
        std::str::from_utf8(bytes).map_err(|_| usage(format!("{file} is not UTF-8 text")))?;
    let module = compile_python(source, &EmbedOptions::default()).map_err(|error| {
        let code = if error.code == AgentErrorCode::FrameInvalid.symbol() {
            AgentErrorCode::FrameInvalid
        } else {
            AgentErrorCode::CandidateInvalid
        };
        let at = match (error.line, error.column) {
            (Some(line), Some(column)) => format!("{file}:{line}:{column}: "),
            _ => format!("{file}: "),
        };
        AgentError::new(code, format!("{at}{}", error.message))
    })?;
    Ok(if rust {
        bindgen::rust(&module)
    } else {
        bindgen::typescript(&module)
    })
}

#[cfg(not(feature = "familiar"))]
fn generate(_file: &str, _bytes: &[u8], _rust: bool) -> Result<String> {
    Err(usage(
        "this sley-agent was built without the optional familiar frontend (cargo feature `familiar`), which bindgen compiles with",
    ))
}
