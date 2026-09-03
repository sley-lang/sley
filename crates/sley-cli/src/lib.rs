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
    BridgeError, JsonBridgeErrorCode, MAX_JSON_TEXT_BYTES, METHOD_TABLE_JSON, frame_from_json,
    frame_to_json, hello_to_json,
};
use sley_protocol::{
    Answer, BoundedContext, DecodedFrame, EncodedFrame, FEATURE_JSON_BRIDGE, FrameKind, Hello,
    MAX_FRAME_BYTES, PROTOCOL_VERSION, ProtocolError, ProtocolErrorCode, ProtocolFailure,
    ProtocolFrame, Server, decode_frame, encode_frame, encode_hello_frame, frame_length, negotiate,
};

/// The CLI contract name written by `sley version`.
pub const CLI_CONTRACT: &str = "sley2-cli-v1";
/// The CLI version written by `sley version`.
pub const CLI_VERSION: &str = "1";
/// The report contract name (contract section 3).
pub const REPORT_CONTRACT: &str = "sley2-cli-report-v1";
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

/// One parsed command line (contract section 1).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    /// `sley serve ...`.
    Serve(ServeOptions),
    /// `sley frame decode`.
    FrameDecode,
    /// `sley frame encode`.
    FrameEncode,
    /// `sley methods`.
    Methods,
    /// `sley hello [--json]`.
    Hello {
        /// Render the `Hello` object instead of the hello frame.
        json: bool,
    },
    /// `sley version`.
    Version,
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
            let mut words = rest.iter();
            while let Some(word) = words.next() {
                match word.as_str() {
                    "--repository" if repository.is_none() => {
                        repository = Some(PathBuf::from(words.next().ok_or_else(|| usage(word))?));
                    }
                    "--report" if report.is_none() => {
                        report = Some(PathBuf::from(words.next().ok_or_else(|| usage(word))?));
                    }
                    "--json" if !json => json = true,
                    "--batch" if !batch => batch = true,
                    _ => return Err(usage(word)),
                }
            }
            Ok(Command::Serve(ServeOptions {
                repository: repository.ok_or_else(|| usage("--repository"))?,
                json,
                batch,
                report,
            }))
        }
        "frame" => match rest {
            [word] if word == "decode" => Ok(Command::FrameDecode),
            [word] if word == "encode" => Ok(Command::FrameEncode),
            _ => Err(usage(rest.first().map_or("frame", String::as_str))),
        },
        "methods" => rest
            .first()
            .map_or(Ok(Command::Methods), |word| Err(usage(word))),
        "version" => rest
            .first()
            .map_or(Ok(Command::Version), |word| Err(usage(word))),
        "hello" => match rest {
            [] => Ok(Command::Hello { json: false }),
            [word] if word == "--json" => Ok(Command::Hello { json: true }),
            _ => Err(usage(rest.first().map_or("hello", String::as_str))),
        },
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
        Command::Serve(options) => serve(&options, stdin, stdout),
        Command::FrameDecode => frame_decode(stdin, stdout),
        Command::FrameEncode => frame_encode(stdin, stdout),
        Command::Methods => stdout
            .write_all(METHOD_TABLE_JSON.as_bytes())
            .map_err(stream_failure),
        Command::Hello { json } => hello(json, stdout),
        Command::Version => version(stdout),
    });
    match outcome {
        Ok(()) => 0,
        Err(failure) => {
            // Standard error is best effort; the exit status carries the code.
            let _ = writeln!(stderr, "{}", failure.value());
            failure.code.exit_status()
        }
    }
}

fn offered_hello(json: bool) -> Result<Hello> {
    let mut offered = Server::offered_hello().map_err(endpoint_failure)?;
    if json {
        offered.features |= FEATURE_JSON_BRIDGE;
    }
    Ok(offered)
}

fn hello(json: bool, stdout: &mut dyn Write) -> Result<()> {
    let offered = offered_hello(json)?;
    if json {
        let text = hello_to_json(&offered).map_err(|error| render_failure(&error))?;
        writeln!(stdout, "{text}").map_err(stream_failure)
    } else {
        let frame = encode_hello_frame(&offered).map_err(endpoint_failure)?;
        stdout.write_all(&frame.bytes).map_err(stream_failure)
    }
}

fn version(stdout: &mut dyn Write) -> Result<()> {
    let mut map = Map::new();
    map.insert("cli".into(), Value::from(CLI_VERSION));
    map.insert("contract".into(), Value::from(CLI_CONTRACT));
    map.insert("protocol_version".into(), Value::from(PROTOCOL_VERSION));
    writeln!(stdout, "{}", Value::Object(map)).map_err(stream_failure)
}

fn frame_decode(stdin: &mut dyn Read, stdout: &mut dyn Write) -> Result<()> {
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
                writeln!(stdout, "{text}").map_err(stream_failure)?;
            }
            Next::Rejected(_) => unreachable!("byte frames are never bridge-rejected"),
        }
    }
}

fn frame_encode(stdin: &mut dyn Read, stdout: &mut dyn Write) -> Result<()> {
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
            Next::Frame(bytes) => stdout.write_all(&bytes).map_err(stream_failure)?,
            Next::TooLarge(_) => unreachable!("text lines are never prefix-rejected"),
        }
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
}

impl Report {
    /// Renders the report with the bridge's declared encodings.
    #[must_use]
    pub fn value(&self) -> Value {
        let mut map = Map::new();
        map.insert("contract".into(), Value::from(REPORT_CONTRACT));
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
        Value::Object(map)
    }
}

// ---------------------------------------------------------------------------
// serve
// ---------------------------------------------------------------------------

/// Serves frames from `stdin` to `stdout` over the repository (contract
/// section 2), writing the report at exit when one was requested.
///
/// # Errors
///
/// Returns the endpoint's own failure; a failed answer is not a failure.
pub fn serve(options: &ServeOptions, stdin: &mut dyn Read, stdout: &mut dyn Write) -> Result<()> {
    let mut report = Report {
        mode_json: options.json,
        batch: options.batch,
        ..Report::default()
    };
    let outcome = serve_frames(options, stdin, stdout, &mut report);
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
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    report: &mut Report,
) -> Result<()> {
    let json = options.json;
    let offered = offered_hello(json)?;
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
    let selected = match negotiate(&client, &offered) {
        Ok(selected) => selected,
        Err(error) => {
            return write_rejection(
                stdout,
                json,
                &ProtocolFailure::protocol(error.code()),
                report,
            );
        }
    };
    let mut server = Server::new(&options.repository, selected).map_err(endpoint_failure)?;
    report.handshake_id = Some(hex(server.handshake_id().as_bytes()));
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
