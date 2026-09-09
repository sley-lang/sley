//! Thin machine-oriented CLI (S20-430, contract `docs/spec/SLEY_CLI_V1.md`,
//! ADR-0035).
//!
//! A transport endpoint: SMP1 frames move between standard input, standard
//! output, and the deterministic S20-410 server in byte or JSON form. The
//! endpoint builds only its own hello and one failure frame through the
//! frozen codec, counts what it moved into a report, and judges nothing.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;

use serde_json::{Map, Value};
use sley_json_bridge::{
    BridgeError, JsonBridgeErrorCode, MAX_JSON_TEXT_BYTES, METHOD_TABLE_JSON, METHOD_TABLE_V2_JSON,
    frame_from_json, frame_to_json, hello_to_json, hello_to_json_versioned,
};
use sley_protocol::{
    Answer, BoundedContext, DecodedFrame, EncodedFrame, FrameKind, Hello, MAX_FRAME_BYTES,
    PROTOCOL_VERSION, PROTOCOL_VERSION_V2, ProtocolError, ProtocolErrorCode, ProtocolFailure,
    ProtocolFrame, Server, decode_frame, encode_frame, encode_hello_frame, frame_length, negotiate,
    negotiate_versioned,
};

/// The CLI contract name written by `sley version`.
pub const CLI_CONTRACT: &str = "sley2-cli-v1";
/// The CLI version written by `sley version`.
pub const CLI_VERSION: &str = "1";
/// The capable CLI contract name written by `sley version` under the
/// version-aware profile (contract section 9).
pub const CLI_CONTRACT_V2: &str = "sley2-cli-v2";
/// The only protocol profile the endpoint accepts (contract section 9).
pub const PROFILE_V2_CAPABLE: &str = "v2-capable";
/// The report contract name (contract section 3).
pub const REPORT_CONTRACT: &str = "sley2-cli-report-v1";
/// The capable report contract name (contract section 9).
pub const REPORT_CONTRACT_V2: &str = "sley2-cli-report-v2";
const LENGTH_PREFIX: usize = 8;

// ---------------------------------------------------------------------------
// Failures
// ---------------------------------------------------------------------------

/// The endpoint's own failures (contract section 4).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CliErrorCode {
    /// `CLI_USAGE_INVALID`: an unknown command, option, repeat, or missing value.
    UsageInvalid,
    /// `CLI_INPUT_INVALID`: a short read inside a frame or an unreadable text.
    InputInvalid,
    /// `CLI_IO_FAILURE`: a stream, report, or endpoint failure.
    IoFailure,
    /// `CLI_HANDSHAKE_REQUIRED`: the first frame is not a client hello.
    HandshakeRequired,
}

impl CliErrorCode {
    /// Every code in numeric order.
    pub const ALL: [Self; 4] = [
        Self::UsageInvalid,
        Self::InputInvalid,
        Self::IoFailure,
        Self::HandshakeRequired,
    ];

    /// The frozen symbol.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UsageInvalid => "CLI_USAGE_INVALID",
            Self::InputInvalid => "CLI_INPUT_INVALID",
            Self::IoFailure => "CLI_IO_FAILURE",
            Self::HandshakeRequired => "CLI_HANDSHAKE_REQUIRED",
        }
    }

    /// The frozen numeric code.
    #[must_use]
    pub const fn numeric(self) -> u32 {
        match self {
            Self::UsageInvalid => 43_000,
            Self::InputInvalid => 43_001,
            Self::IoFailure => 43_002,
            Self::HandshakeRequired => 43_003,
        }
    }

    /// The process exit status.
    #[must_use]
    pub const fn exit_status(self) -> i32 {
        match self {
            Self::UsageInvalid => 2,
            Self::InputInvalid => 3,
            Self::IoFailure => 4,
            Self::HandshakeRequired => 5,
        }
    }
}

/// One CLI failure and the codec or bridge symbol it wraps, if any.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CliFailure {
    /// The endpoint's code.
    pub code: CliErrorCode,
    /// The codec or bridge symbol, or the offending word, this failure wraps.
    pub cause: Option<String>,
}

impl CliFailure {
    fn new(code: CliErrorCode) -> Self {
        Self { code, cause: None }
    }

    fn with_cause(code: CliErrorCode, cause: impl Into<String>) -> Self {
        Self {
            code,
            cause: Some(cause.into()),
        }
    }

    /// The failure object written to standard error and the report.
    #[must_use]
    pub fn value(&self) -> Value {
        let mut map = Map::new();
        map.insert("code".into(), Value::from(self.code.numeric()));
        map.insert("symbol".into(), Value::from(self.code.as_str()));
        map.insert(
            "cause".into(),
            self.cause.as_deref().map_or(Value::Null, Value::from),
        );
        Value::Object(map)
    }
}

type Result<T> = core::result::Result<T, CliFailure>;

fn stream_failure(_: std::io::Error) -> CliFailure {
    CliFailure::with_cause(CliErrorCode::IoFailure, "STREAM")
}

// Used as a `map_err` function; the codec error is not `Copy`.
#[allow(clippy::needless_pass_by_value)]
fn endpoint_failure(error: ProtocolError) -> CliFailure {
    CliFailure::with_cause(CliErrorCode::IoFailure, error.code().as_str())
}

fn render_failure(error: &BridgeError) -> CliFailure {
    CliFailure::with_cause(CliErrorCode::IoFailure, error.symbol())
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Options of `sley serve` (contract section 2).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServeOptions {
    /// The repository path handed to the server.
    pub repository: PathBuf,
    /// JSON lines instead of byte frames on both streams.
    pub json: bool,
    /// Read to end of input before answering together.
    pub batch: bool,
    /// Where to write the report at exit.
    pub report: Option<PathBuf>,
}

/// The protocol profile selected on the command line (contract section 9).
/// Legacy is the frozen version 1 behavior; the capable profile offers
/// protocol versions 1 and 2 without forcing selection 2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolProfile {
    /// Frozen version 1 behavior (the default).
    Legacy,
    /// `--protocol-profile v2-capable`.
    V2Capable,
}

/// Resolves a `--protocol-profile` value; anything but the one capable
/// profile is a usage failure naming the offending value.
fn profile_value(value: &str) -> Result<ProtocolProfile> {
    match value {
        PROFILE_V2_CAPABLE => Ok(ProtocolProfile::V2Capable),
        _ => Err(CliFailure::with_cause(CliErrorCode::UsageInvalid, value)),
    }
}

/// Resolves an `--expected-version` value; only versions 1 and 2 exist.
fn expected_value(value: &str) -> Result<u32> {
    match value {
        "1" => Ok(PROTOCOL_VERSION),
        "2" => Ok(PROTOCOL_VERSION_V2),
        _ => Err(CliFailure::with_cause(CliErrorCode::UsageInvalid, value)),
    }
}

/// Consumes the value of a flag that takes one; a missing value is a usage
/// failure naming the flag.
fn flag_value<'a>(words: &'a [String], index: &mut usize, flag: &str) -> Result<&'a str> {
    *index += 1;
    words
        .get(*index)
        .map(String::as_str)
        .ok_or_else(|| CliFailure::with_cause(CliErrorCode::UsageInvalid, flag))
}

/// One parsed command line (contract section 1).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    /// `sley serve ...`.
    Serve {
        /// The unchanged four-field serve options.
        options: ServeOptions,
        /// The protocol profile (legacy default).
        profile: ProtocolProfile,
    },
    /// `sley frame decode`.
    FrameDecode {
        /// The protocol profile (legacy default).
        profile: ProtocolProfile,
        /// The required wire version under the capable profile.
        expected_version: Option<u32>,
    },
    /// `sley frame encode`.
    FrameEncode {
        /// The protocol profile (legacy default).
        profile: ProtocolProfile,
        /// The required wire version under the capable profile.
        expected_version: Option<u32>,
    },
    /// `sley methods`.
    Methods {
        /// The protocol profile (legacy default).
        profile: ProtocolProfile,
    },
    /// `sley hello [--json]`.
    Hello {
        /// Render the `Hello` object instead of the hello frame.
        json: bool,
        /// The protocol profile (legacy default).
        profile: ProtocolProfile,
    },
    /// `sley version`.
    Version {
        /// The protocol profile (legacy default).
        profile: ProtocolProfile,
    },
}

/// Parses the flag tail of `frame decode` / `frame encode`: at most one
/// `--protocol-profile v2-capable` and at most one `--expected-version
/// 1|2`, in any order. The expected version requires the profile and the
/// profile requires it; anything else is a usage failure.
fn parse_frame_flags(words: &[String]) -> Result<(ProtocolProfile, Option<u32>)> {
    let failure = |word: &str| CliFailure::with_cause(CliErrorCode::UsageInvalid, word);
    let mut profile = ProtocolProfile::Legacy;
    let mut expected_version = None;
    let mut index = 0;
    while index < words.len() {
        match words[index].as_str() {
            "--protocol-profile" if profile == ProtocolProfile::Legacy => {
                let value = flag_value(words, &mut index, "--protocol-profile")?;
                profile = profile_value(value)?;
                index += 1;
            }
            "--expected-version" if expected_version.is_none() => {
                let value = flag_value(words, &mut index, "--expected-version")?;
                expected_version = Some(expected_value(value)?);
                index += 1;
            }
            word => return Err(failure(word)),
        }
    }
    match (profile, expected_version) {
        (ProtocolProfile::Legacy, None) | (ProtocolProfile::V2Capable, Some(_)) => {
            Ok((profile, expected_version))
        }
        (ProtocolProfile::Legacy, Some(_)) => Err(failure("--expected-version")),
        (ProtocolProfile::V2Capable, None) => Err(failure("--protocol-profile")),
    }
}

/// Parses the flag tail of `methods` / `version`: at most one
/// `--protocol-profile v2-capable` and nothing else.
fn parse_profile_only(words: &[String]) -> Result<ProtocolProfile> {
    let failure = |word: &str| CliFailure::with_cause(CliErrorCode::UsageInvalid, word);
    let mut profile = ProtocolProfile::Legacy;
    let mut index = 0;
    while index < words.len() {
        match words[index].as_str() {
            "--protocol-profile" if profile == ProtocolProfile::Legacy => {
                let value = flag_value(words, &mut index, "--protocol-profile")?;
                profile = profile_value(value)?;
                index += 1;
            }
            word => return Err(failure(word)),
        }
    }
    Ok(profile)
}

/// Parses the exact command line; every deviation is `CLI_USAGE_INVALID`.
///
/// # Errors
///
/// Returns `CLI_USAGE_INVALID` with the offending word as the cause.
pub fn parse(args: &[String]) -> Result<Command> {
    let usage = |word: &str| CliFailure::with_cause(CliErrorCode::UsageInvalid, word);
    let Some((command, rest)) = args.split_first() else {
        return Err(CliFailure::new(CliErrorCode::UsageInvalid));
    };
    match command.as_str() {
        "serve" => {
            let mut repository = None;
            let mut json = false;
            let mut batch = false;
            let mut report = None;
            let mut profile = ProtocolProfile::Legacy;
            let mut words = rest.iter();
            while let Some(word) = words.next() {
                match word.as_str() {
                    "--repository" if repository.is_none() => {
                        repository = Some(PathBuf::from(words.next().ok_or_else(|| usage(word))?));
                    }
                    "--json" if !json => json = true,
                    "--batch" if !batch => batch = true,
                    "--report" if report.is_none() => {
                        report = Some(PathBuf::from(words.next().ok_or_else(|| usage(word))?));
                    }
                    "--protocol-profile" if profile == ProtocolProfile::Legacy => {
                        let value = words.next().ok_or_else(|| usage(word))?;
                        profile = profile_value(value)?;
                    }
                    _ => return Err(usage(word)),
                }
            }
            Ok(Command::Serve {
                options: ServeOptions {
                    repository: repository.ok_or_else(|| usage("--repository"))?,
                    json,
                    batch,
                    report,
                },
                profile,
            })
        }
        "frame" => match rest.first().map(String::as_str) {
            Some("decode") => {
                let (profile, expected_version) = parse_frame_flags(&rest[1..])?;
                Ok(Command::FrameDecode {
                    profile,
                    expected_version,
                })
            }
            Some("encode") => {
                let (profile, expected_version) = parse_frame_flags(&rest[1..])?;
                Ok(Command::FrameEncode {
                    profile,
                    expected_version,
                })
            }
            _ => Err(usage(rest.first().map_or("frame", String::as_str))),
        },
        "methods" => Ok(Command::Methods {
            profile: parse_profile_only(rest)?,
        }),
        "version" => Ok(Command::Version {
            profile: parse_profile_only(rest)?,
        }),
        "hello" => {
            let mut json = false;
            let mut profile = ProtocolProfile::Legacy;
            let mut index = 0;
            while index < rest.len() {
                match rest[index].as_str() {
                    "--json" if !json => {
                        json = true;
                        index += 1;
                    }
                    "--protocol-profile" if profile == ProtocolProfile::Legacy => {
                        let value = flag_value(rest, &mut index, "--protocol-profile")?;
                        profile = profile_value(value)?;
                        index += 1;
                    }
                    word => return Err(usage(word)),
                }
            }
            Ok(Command::Hello { json, profile })
        }
        other => Err(usage(other)),
    }
}

/// Runs one command line over the given streams and returns the exit
/// status; a CLI failure is written to standard error as one JSON object.
pub fn run(
    args: &[String],
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let outcome = parse(args).and_then(|command| match command {
        Command::Serve { options, profile } => serve_profile(&options, profile, stdin, stdout),
        Command::FrameDecode {
            expected_version, ..
        } => frame_decode(expected_version, stdin, stdout),
        Command::FrameEncode {
            expected_version, ..
        } => frame_encode(expected_version, stdin, stdout),
        Command::Methods { profile } => stdout
            .write_all(method_table(profile).as_bytes())
            .map_err(stream_failure),
        Command::Hello { json, profile } => hello(json, profile, stdout),
        Command::Version { profile } => version(profile, stdout),
    });
    match outcome {
        Ok(()) => {
            // Every write path flushes (contract section 2), and this
            // final flush covers the commands that write directly, so
            // `main`'s `process::exit` never drops buffered bytes.
            if let Err(error) = stdout.flush() {
                let failure = stream_failure(error);
                let _ = writeln!(stderr, "{}", failure.value());
                return failure.code.exit_status();
            }
            0
        }
        Err(failure) => {
            // Standard error is best effort; the exit status carries the code.
            let _ = writeln!(stderr, "{}", failure.value());
            failure.code.exit_status()
        }
    }
}

/// The method table the endpoint prints: the frozen version 1 table by
/// default, the additive version 2 table under the capable profile
/// (contract section 9).
fn method_table(profile: ProtocolProfile) -> &'static str {
    match profile {
        ProtocolProfile::Legacy => METHOD_TABLE_JSON,
        ProtocolProfile::V2Capable => METHOD_TABLE_V2_JSON,
    }
}

/// The hello the endpoint offers: the server's own hello, unedited
/// (contract section 2). The wire form is a transport choice, never a
/// negotiated feature, so the offer never carries a transport feature
/// and the handshake identity does not depend on `--json`.
fn offered_hello(profile: ProtocolProfile) -> Result<Hello> {
    match profile {
        ProtocolProfile::Legacy => Server::offered_hello(),
        ProtocolProfile::V2Capable => Server::offered_hello_versioned(),
    }
    .map_err(endpoint_failure)
}

fn hello(json: bool, profile: ProtocolProfile, stdout: &mut dyn Write) -> Result<()> {
    let offered = offered_hello(profile)?;
    if json {
        let text = match profile {
            ProtocolProfile::Legacy => {
                hello_to_json(&offered).map_err(|error| render_failure(&error))
            }
            ProtocolProfile::V2Capable => {
                hello_to_json_versioned(&offered).map_err(|error| render_failure(&error))
            }
        }?;
        writeln!(stdout, "{text}").map_err(stream_failure)
    } else {
        let frame = encode_hello_frame(&offered).map_err(endpoint_failure)?;
        stdout.write_all(&frame.bytes).map_err(stream_failure)
    }
}

fn version(profile: ProtocolProfile, stdout: &mut dyn Write) -> Result<()> {
    let mut map = Map::new();
    map.insert("cli".into(), Value::from(CLI_VERSION));
    match profile {
        ProtocolProfile::Legacy => {
            map.insert("contract".into(), Value::from(CLI_CONTRACT));
            map.insert("protocol_version".into(), Value::from(PROTOCOL_VERSION));
        }
        ProtocolProfile::V2Capable => {
            map.insert("contract".into(), Value::from(CLI_CONTRACT_V2));
            map.insert("protocol_profile".into(), Value::from(PROFILE_V2_CAPABLE));
            map.insert(
                "protocol_versions".into(),
                Value::from(vec![
                    Value::from(PROTOCOL_VERSION),
                    Value::from(PROTOCOL_VERSION_V2),
                ]),
            );
        }
    }
    writeln!(stdout, "{}", Value::Object(map)).map_err(stream_failure)
}

fn frame_decode(
    expected_version: Option<u32>,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
) -> Result<()> {
    loop {
        match read_frame(stdin, MAX_FRAME_BYTES)? {
            Next::End => return Ok(()),
            Next::TooLarge(_) => {
                return Err(CliFailure::with_cause(
                    CliErrorCode::InputInvalid,
                    ProtocolErrorCode::FrameTooLarge.as_str(),
                ));
            }
            Next::Frame(bytes) => {
                let text = frame_to_json(&bytes).map_err(|error| {
                    CliFailure::with_cause(CliErrorCode::InputInvalid, error.symbol())
                })?;
                if let Some(want) = expected_version {
                    let (is_hello, version) = converted_kind_and_version(&bytes)?;
                    enforce_expected_version(is_hello, version, want)?;
                }
                writeln!(stdout, "{text}").map_err(stream_failure)?;
            }
            Next::Rejected(_) => unreachable!("byte frames are never bridge-rejected"),
        }
    }
}

fn frame_encode(
    expected_version: Option<u32>,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
) -> Result<()> {
    let mut reader = BufReader::new(stdin);
    loop {
        match read_line(&mut reader)? {
            Next::End => return Ok(()),
            Next::Rejected(error) => {
                return Err(CliFailure::with_cause(
                    CliErrorCode::InputInvalid,
                    error.symbol(),
                ));
            }
            Next::Frame(bytes) => {
                if let Some(want) = expected_version {
                    let (is_hello, version) = converted_kind_and_version(&bytes)?;
                    enforce_expected_version(is_hello, version, want)?;
                }
                stdout.write_all(&bytes).map_err(stream_failure)?;
            }
            Next::TooLarge(_) => unreachable!("text lines are never prefix-rejected"),
        }
    }
}

/// The kind and wire version of converted frame bytes: a hello travels at
/// frame version 1; any other frame carries its own selected version.
fn converted_kind_and_version(bytes: &[u8]) -> Result<(bool, u32)> {
    match decode_frame(bytes, MAX_FRAME_BYTES).map_err(endpoint_failure)? {
        (DecodedFrame::Hello(_), _) => Ok((true, PROTOCOL_VERSION)),
        (DecodedFrame::Request(frame) | DecodedFrame::Response(frame), _) => {
            Ok((false, frame.protocol_version))
        }
    }
}

/// The stateless version rule (contract section 9): a hello is accepted
/// only under expected version 1, and every other frame must carry exactly
/// the expected version. A version mismatch is an unreadable conversion.
fn enforce_expected_version(is_hello: bool, version: u32, expected: u32) -> Result<()> {
    let accepted = if is_hello {
        expected == PROTOCOL_VERSION
    } else {
        version == expected
    };
    if accepted {
        Ok(())
    } else {
        Err(CliFailure::with_cause(
            CliErrorCode::InputInvalid,
            "VERSION_MISMATCH",
        ))
    }
}

// ---------------------------------------------------------------------------
// Frame sources
// ---------------------------------------------------------------------------

/// One step of a frame source.
enum Next {
    /// Clean end of input.
    End,
    /// One complete frame (already re-encoded by the bridge in JSON mode).
    Frame(Vec<u8>),
    /// A byte prefix whose length exceeds the ceiling; the body was not read.
    TooLarge(Vec<u8>),
    /// A text line the bridge rejected.
    Rejected(BridgeError),
}

fn read_exact_or_end(input: &mut dyn Read, buffer: &mut [u8]) -> Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match input.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(count) => filled += count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(stream_failure(error)),
        }
    }
    Ok(filled)
}

fn read_frame(input: &mut dyn Read, ceiling: u64) -> Result<Next> {
    let mut prefix = [0u8; LENGTH_PREFIX];
    match read_exact_or_end(input, &mut prefix)? {
        0 => return Ok(Next::End),
        LENGTH_PREFIX => {}
        _ => {
            return Err(CliFailure::with_cause(
                CliErrorCode::InputInvalid,
                "SHORT_PREFIX",
            ));
        }
    }
    let length = match frame_length(&prefix, ceiling) {
        Ok(length) => length,
        Err(error) if error.code() == ProtocolErrorCode::FrameTooLarge => {
            return Ok(Next::TooLarge(prefix.to_vec()));
        }
        Err(error) => {
            return Err(CliFailure::with_cause(
                CliErrorCode::InputInvalid,
                error.code().as_str(),
            ));
        }
    };
    let mut bytes = vec![0u8; LENGTH_PREFIX + length];
    bytes[..LENGTH_PREFIX].copy_from_slice(&prefix);
    if read_exact_or_end(input, &mut bytes[LENGTH_PREFIX..])? != length {
        return Err(CliFailure::with_cause(
            CliErrorCode::InputInvalid,
            "SHORT_FRAME",
        ));
    }
    Ok(Next::Frame(bytes))
}

fn read_line(reader: &mut BufReader<&mut dyn Read>) -> Result<Next> {
    let mut line = Vec::new();
    let limit = u64::try_from(MAX_JSON_TEXT_BYTES + 2).unwrap_or(u64::MAX);
    let count = reader
        .by_ref()
        .take(limit)
        .read_until(b'\n', &mut line)
        .map_err(stream_failure)?;
    if count == 0 {
        return Ok(Next::End);
    }
    if line.last() == Some(&b'\n') {
        line.pop();
    }
    let Ok(text) = String::from_utf8(line) else {
        return Ok(Next::Rejected(BridgeError::Bridge(
            JsonBridgeErrorCode::ShapeInvalid,
        )));
    };
    Ok(match frame_from_json(&text) {
        Ok(encoded) => Next::Frame(encoded.bytes),
        Err(error) => Next::Rejected(error),
    })
}

enum Source<'a> {
    Bytes(&'a mut dyn Read),
    Text(BufReader<&'a mut dyn Read>),
}

impl Source<'_> {
    fn next(&mut self, ceiling: u64) -> Result<Next> {
        match self {
            Source::Bytes(input) => read_frame(*input, ceiling),
            Source::Text(reader) => read_line(reader),
        }
    }
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// The counting report of one `serve` invocation (contract section 3).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Report {
    /// Whether the invocation ran in JSON mode.
    pub mode_json: bool,
    /// Whether the invocation answered in batch.
    pub batch: bool,
    /// The derived handshake identity once negotiated.
    pub handshake_id: Option<String>,
    /// Frames read, the client hello included.
    pub frames_read: u64,
    /// Frames written, the endpoint's hello included.
    pub frames_written: u64,
    /// Event frames among the frames written.
    pub events_written: u64,
    /// Response frames written, the endpoint's own failure responses included.
    pub answers: u64,
    /// Response frames carrying a failure body.
    pub failed_answers: u64,
    /// Failure codes copied from the failure bodies, by numeric code.
    pub codes: BTreeMap<u32, u64>,
    /// The endpoint's own failure, if the invocation failed.
    pub cli_failure: Option<CliFailure>,
    /// The exit status.
    pub exit_code: i32,
    /// Whether the invocation ran under the version-aware profile.
    pub capable: bool,
    /// The negotiated protocol version under the capable profile: null
    /// before a successful negotiation, otherwise the actual 1 or 2.
    pub selected_protocol_version: Option<u32>,
}

impl Report {
    /// Renders the report with the bridge's declared encodings.
    #[must_use]
    pub fn value(&self) -> Value {
        let mut map = Map::new();
        map.insert(
            "contract".into(),
            Value::from(if self.capable {
                REPORT_CONTRACT_V2
            } else {
                REPORT_CONTRACT
            }),
        );
        map.insert("command".into(), Value::from("serve"));
        map.insert(
            "mode".into(),
            Value::from(if self.mode_json { "json" } else { "bytes" }),
        );
        map.insert("batch".into(), Value::Bool(self.batch));
        map.insert(
            "handshake_id".into(),
            self.handshake_id
                .as_deref()
                .map_or(Value::Null, Value::from),
        );
        map.insert("frames_read".into(), Value::from(self.frames_read));
        map.insert("frames_written".into(), Value::from(self.frames_written));
        map.insert("events_written".into(), Value::from(self.events_written));
        map.insert("answers".into(), Value::from(self.answers));
        map.insert("failed_answers".into(), Value::from(self.failed_answers));
        let mut codes = Map::new();
        for (code, count) in &self.codes {
            codes.insert(code.to_string(), Value::from(*count));
        }
        map.insert("codes".into(), Value::Object(codes));
        map.insert(
            "cli_failure".into(),
            self.cli_failure
                .as_ref()
                .map_or(Value::Null, CliFailure::value),
        );
        map.insert("exit_code".into(), Value::from(self.exit_code));
        if self.capable {
            map.insert("protocol_profile".into(), Value::from(PROFILE_V2_CAPABLE));
            map.insert(
                "selected_protocol_version".into(),
                self.selected_protocol_version
                    .map_or(Value::Null, Value::from),
            );
        }
        Value::Object(map)
    }
}

// ---------------------------------------------------------------------------
// serve
// ---------------------------------------------------------------------------

/// Serves frames from `stdin` to `stdout` over the repository (contract
/// section 2), writing the report at exit when one was requested.
/// The legacy wrapper: frozen version 1 behavior.
///
/// # Errors
///
/// Returns the endpoint's own failure; a failed answer is not a failure.
pub fn serve(options: &ServeOptions, stdin: &mut dyn Read, stdout: &mut dyn Write) -> Result<()> {
    serve_profile(options, ProtocolProfile::Legacy, stdin, stdout)
}

/// The profile-aware serve entrypoint (contract section 9): the legacy
/// options keep their shape and `serve` above stays the legacy wrapper.
/// Under the capable profile the endpoint offers, negotiates, and serves
/// version-aware; by default everything is frozen version 1.
///
/// # Errors
///
/// Returns the endpoint's own failure; a failed answer is not a failure.
pub fn serve_profile(
    options: &ServeOptions,
    profile: ProtocolProfile,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
) -> Result<()> {
    let mut report = Report {
        mode_json: options.json,
        batch: options.batch,
        capable: profile == ProtocolProfile::V2Capable,
        ..Report::default()
    };
    let outcome = serve_frames(options, profile, stdin, stdout, &mut report);
    if let Err(failure) = &outcome {
        report.cli_failure = Some(failure.clone());
        report.exit_code = failure.code.exit_status();
    }
    if let Some(path) = &options.report {
        let text = format!("{}\n", report.value());
        if let Err(error) = std::fs::write(path, text) {
            return outcome.and(Err(stream_failure(error)));
        }
    }
    outcome
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

/// The one frame the endpoint builds itself: a failure response with no
/// session and request identifier zero (contract section 2).
fn failure_frame(failure: &ProtocolFailure) -> Result<EncodedFrame> {
    let frame = ProtocolFrame {
        protocol_version: PROTOCOL_VERSION,
        session: None,
        request_id: 0,
        kind: FrameKind::Response,
        method: 0,
        flags: 0,
        bounds: BoundedContext::none(),
        body: failure.encode().map_err(endpoint_failure)?,
    };
    encode_frame(&frame).map_err(endpoint_failure)
}

fn write_frame(
    stdout: &mut dyn Write,
    json: bool,
    frame: &EncodedFrame,
    report: &mut Report,
) -> Result<()> {
    if json {
        let text = frame_to_json(&frame.bytes).map_err(|error| render_failure(&error))?;
        writeln!(stdout, "{text}").map_err(stream_failure)?;
    } else {
        stdout.write_all(&frame.bytes).map_err(stream_failure)?;
    }
    // Flush per answer (contract section 2): byte-mode frames carry no
    // reliable trailing newline, so a pipe consumer must never wait on a
    // response sitting in the endpoint's buffer.
    stdout.flush().map_err(stream_failure)?;
    report.frames_written += 1;
    Ok(())
}

/// Writes one of the endpoint's own failure responses and counts it as a
/// failed answer.
fn write_rejection(
    stdout: &mut dyn Write,
    json: bool,
    failure: &ProtocolFailure,
    report: &mut Report,
) -> Result<()> {
    write_frame(stdout, json, &failure_frame(failure)?, report)?;
    report.answers += 1;
    report.failed_answers += 1;
    *report.codes.entry(failure.code).or_default() += 1;
    Ok(())
}

fn write_answer(
    stdout: &mut dyn Write,
    json: bool,
    answer: &Answer,
    report: &mut Report,
) -> Result<()> {
    for event in &answer.events {
        write_frame(stdout, json, event, report)?;
        report.events_written += 1;
    }
    write_frame(stdout, json, &answer.frame, report)?;
    report.answers += 1;
    if answer.failed {
        report.failed_answers += 1;
        if let Ok((DecodedFrame::Response(frame), _)) =
            decode_frame(&answer.frame.bytes, MAX_FRAME_BYTES)
            && let Ok(failure) = ProtocolFailure::decode(&frame.body)
        {
            *report.codes.entry(failure.code).or_default() += 1;
        }
    }
    Ok(())
}

fn serve_frames(
    options: &ServeOptions,
    profile: ProtocolProfile,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    report: &mut Report,
) -> Result<()> {
    let json = options.json;
    let capable = profile == ProtocolProfile::V2Capable;
    let offered = offered_hello(profile)?;
    let mut source = if json {
        Source::Text(BufReader::new(stdin))
    } else {
        Source::Bytes(stdin)
    };
    let handshake = |cause: &str| CliFailure::with_cause(CliErrorCode::HandshakeRequired, cause);
    let client = match source.next(MAX_FRAME_BYTES)? {
        Next::End => return Err(CliFailure::new(CliErrorCode::HandshakeRequired)),
        Next::TooLarge(_) => return Err(handshake(ProtocolErrorCode::FrameTooLarge.as_str())),
        Next::Rejected(error) => return Err(handshake(error.symbol())),
        Next::Frame(bytes) => {
            report.frames_read += 1;
            match decode_frame(&bytes, MAX_FRAME_BYTES) {
                Ok((DecodedFrame::Hello(client), _)) => client,
                Ok(_) => return Err(handshake("NOT_A_HELLO")),
                Err(error) => return Err(handshake(error.code().as_str())),
            }
        }
    };
    // The offer is checked here so a negotiation failure is answered as
    // a protocol rejection frame; the server below re-derives the same
    // selection and the transcript-bound identity from the observed
    // hellos (contract section 2), never from this asserted value.
    // Rejection frames travel at frame version 1: without a selection no
    // version is negotiated, and a version 1 peer reads them.
    match if capable {
        negotiate_versioned(&client, &offered)
    } else {
        negotiate(&client, &offered)
    } {
        Ok(_) => {}
        Err(error) => {
            return write_rejection(
                stdout,
                json,
                &ProtocolFailure::protocol(error.code()),
                report,
            );
        }
    }
    let mut server = if capable {
        Server::new_versioned(&options.repository, &client, &offered)
    } else {
        Server::new(&options.repository, &client, &offered)
    }
    .map_err(endpoint_failure)?;
    report.handshake_id = Some(hex(server.handshake_id().as_bytes()));
    if capable {
        report.selected_protocol_version = Some(server.profile().protocol_version);
    }
    write_frame(
        stdout,
        json,
        &encode_hello_frame(&offered).map_err(endpoint_failure)?,
        report,
    )?;
    let ceiling = server.profile().limits.max_frame_bytes;

    let mut pending: Vec<Vec<u8>> = Vec::new();
    loop {
        let bytes = match source.next(ceiling)? {
            Next::End => break,
            Next::Frame(bytes) => {
                report.frames_read += 1;
                bytes
            }
            Next::TooLarge(prefix) => {
                // The body is never read; the prefix alone is answered and
                // the stream cannot be resynchronised, so input ends here.
                report.frames_read += 1;
                pending.push(prefix);
                break;
            }
            Next::Rejected(error) => {
                report.frames_read += 1;
                write_rejection(stdout, json, &error.envelope(), report)?;
                if error == BridgeError::Bridge(JsonBridgeErrorCode::ResourceLimit) {
                    break;
                }
                continue;
            }
        };
        if options.batch {
            pending.push(bytes);
        } else {
            let answer = server.answer(&bytes).map_err(endpoint_failure)?;
            write_answer(stdout, json, &answer, report)?;
        }
    }
    if !pending.is_empty() {
        let requests: Vec<&[u8]> = pending.iter().map(Vec::as_slice).collect();
        for answer in server.answer_batch(&requests).map_err(endpoint_failure)? {
            write_answer(stdout, json, &answer, report)?;
        }
    }
    Ok(())
}
