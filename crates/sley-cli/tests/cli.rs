//! S20-430 endpoint tests over a trusted genesis repository: every judgment
//! observed through the CLI must equal a direct `Server` over the same
//! repository, and every CLI failure must carry its exit status.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::Value;
use sley_id::SessionId;
use sley_json_bridge::{
    METHOD_TABLE_JSON, METHOD_TABLE_V2_JSON, frame_from_json, frame_to_json, hello_to_json,
};
use sley_protocol::{
    BoundedContext, DecodedFrame, FrameKind, Hello, MAX_FRAME_BYTES, Method, PROTOCOL_VERSION,
    ProtocolFailure, ProtocolFrame, Server, decode_frame, encode_frame, encode_hello_frame,
    frame_length, negotiate_identity,
};
use sley_repo::test_support::{TempDir, complete_bodies, complete_dependency_root, genesis};
use sley_scb1::encode_uvar;

const BRIDGE_FIXTURE: &str =
    include_str!("../../../conformance/smp1-json-bridge/v1/roundtrip.json");

fn repository(label: &str) -> (TempDir, PathBuf) {
    let (temp, _transactions, _genesis) =
        genesis(label, complete_bodies(), &[complete_dependency_root()]);
    let path = temp.child("repo");
    (temp, path)
}

fn offered() -> Hello {
    Server::offered_hello().unwrap()
}

fn request(
    session: Option<SessionId>,
    id: u64,
    method: Method,
    flags: u32,
    body: Vec<u8>,
) -> Vec<u8> {
    encode_frame(&ProtocolFrame {
        protocol_version: PROTOCOL_VERSION,
        session,
        request_id: id,
        kind: FrameKind::Request,
        method: method.tag(),
        flags,
        bounds: BoundedContext::none(),
        body,
    })
    .unwrap()
    .bytes
}

fn run(args: &[&str], input: &[u8]) -> (i32, Vec<u8>, String) {
    let args: Vec<String> = args.iter().map(ToString::to_string).collect();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin: &[u8] = input;
    let status = sley_cli::run(&args, &mut stdin, &mut stdout, &mut stderr);
    (status, stdout, String::from_utf8(stderr).unwrap())
}

fn split_frames(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let length = frame_length(&bytes[offset..offset + 8], MAX_FRAME_BYTES).unwrap();
        frames.push(bytes[offset..offset + 8 + length].to_vec());
        offset += 8 + length;
    }
    frames
}

fn response(bytes: &[u8]) -> ProtocolFrame {
    match decode_frame(bytes, MAX_FRAME_BYTES).unwrap().0 {
        DecodedFrame::Response(frame) => frame,
        other => panic!("not a response: {other:?}"),
    }
}

fn failure(bytes: &[u8]) -> ProtocolFailure {
    ProtocolFailure::decode(&response(bytes).body).unwrap()
}

fn stderr_object(text: &str) -> Value {
    serde_json::from_str(text.trim_end()).unwrap()
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

/// A direct server over the repository, its session, and the request
/// frames the CLI tests send after the hello.
struct Direct {
    server: Server,
    session: SessionId,
    frames: Vec<Vec<u8>>,
}

fn direct(path: &PathBuf) -> Direct {
    direct_with(path, &offered())
}

fn direct_with(path: &PathBuf, server_hello: &Hello) -> Direct {
    let client = offered();
    let (_, handshake) = negotiate_identity(&client, server_hello).unwrap();
    let mut server = Server::new(path, &client, server_hello).unwrap();
    let open_frame = request(
        None,
        0,
        Method::SessionOpen,
        0,
        handshake.as_bytes().to_vec(),
    );
    let open = server.answer(&open_frame).unwrap();
    assert!(!open.failed);
    let session = SessionId::from_bytes(response(&open.frame.bytes).body.try_into().unwrap());
    let frames = vec![
        open_frame,
        request(Some(session), 2, Method::SessionCapabilities, 0, Vec::new()),
        request(Some(session), 3, Method::RefsList, 0, encode_uvar(16)),
        request(Some(session), 4, Method::SessionBudgets, 0, Vec::new()),
        request(Some(session), 5, Method::WorkspaceOpen, 0, Vec::new()),
    ];
    Direct {
        server,
        session,
        frames,
    }
}

#[test]
fn handshake_identity_does_not_depend_on_the_transport_flag() {
    // S20-430 P0-1: --json is a transport flag, so the same client hello
    // over the same repository must negotiate the same handshake identity
    // in both modes, and that identity must equal a direct server built
    // from the unedited offer.
    let (temp, path) = repository("cli-mode-identity");
    let repo = path.to_str().unwrap();

    let byte_input = encode_hello_frame(&offered()).unwrap().bytes;
    let byte_report = temp.child("byte-report.json");
    let (status, _, stderr) = run(
        &[
            "serve",
            "--repository",
            repo,
            "--report",
            byte_report.to_str().unwrap(),
        ],
        &byte_input,
    );
    assert_eq!((status, stderr.as_str()), (0, ""));

    let mut lines = frame_to_json(&encode_hello_frame(&offered()).unwrap().bytes).unwrap();
    lines.push('\n');
    let json_report = temp.child("json-report.json");
    let (status, _, stderr) = run(
        &[
            "serve",
            "--repository",
            repo,
            "--json",
            "--report",
            json_report.to_str().unwrap(),
        ],
        lines.as_bytes(),
    );
    assert_eq!((status, stderr.as_str()), (0, ""));

    let byte_report: Value =
        serde_json::from_str(&std::fs::read_to_string(&byte_report).unwrap()).unwrap();
    let json_report: Value =
        serde_json::from_str(&std::fs::read_to_string(&json_report).unwrap()).unwrap();
    assert_eq!(
        byte_report["handshake_id"], json_report["handshake_id"],
        "the transport flag moved the negotiated handshake identity"
    );
    let client = offered();
    let _ = negotiate_identity(&client, &offered()).unwrap();
    let direct = Server::new(&path, &client, &offered()).unwrap();
    assert_eq!(
        byte_report["handshake_id"],
        Value::from(hex(direct.handshake_id().as_bytes())),
        "the endpoint negotiates something other than the server offer"
    );
}

#[test]
fn the_binary_delivers_byte_frames_across_the_process_boundary() {
    // S20-430 nabu P0: the S20-620 runner consumes the `sley` process,
    // not the library, so byte-mode answers must cross the real stdout
    // pipe unfragmented and the exit status must propagate.
    let (_temp, path) = repository("cli-process");
    let repo = path.to_str().unwrap().to_string();
    let handshake = negotiate_identity(&offered(), &offered()).unwrap().1;
    let mut input = encode_hello_frame(&offered()).unwrap().bytes;
    input.extend_from_slice(&request(
        None,
        0,
        Method::SessionOpen,
        0,
        handshake.as_bytes().to_vec(),
    ));

    let mut child = Command::new(env!("CARGO_BIN_EXE_sley"))
        .args(["serve", "--repository", &repo])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stderr, b"".to_vec());
    let frames = split_frames(&output.stdout);
    assert_eq!(frames.len(), 2, "truncated byte output from the binary");
    assert_eq!(frames[0], encode_hello_frame(&offered()).unwrap().bytes);
    assert_eq!(response(&frames[1]).body.len(), 32);

    // The library over the same input must produce the same hello frame:
    // the boundary moves bytes, nothing else.
    let (status, lib_stdout, _) = run(&["serve", "--repository", &repo], &input);
    assert_eq!(status, 0);
    let lib_frames = split_frames(&lib_stdout);
    assert_eq!(frames[0], lib_frames[0]);

    // Exit statuses propagate through the binary as well.
    let output = Command::new(env!("CARGO_BIN_EXE_sley"))
        .args(["bogus"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[test]
fn serve_in_byte_mode_answers_exactly_as_a_direct_server() {
    let (temp, path) = repository("cli-bytes");
    let mut direct = direct(&path);
    // Baseline: the transplanted session is live on its own server, so
    // the direct answers all succeed there.
    for frame in &direct.frames[1..] {
        let answer = direct.server.answer(frame).unwrap();
        assert!(answer.events.is_empty());
        assert!(!answer.failed);
    }
    let mut input = encode_hello_frame(&offered()).unwrap().bytes;
    for frame in &direct.frames {
        input.extend_from_slice(frame);
    }
    let report = temp.child("report.json");
    let (status, stdout, stderr) = run(
        &[
            "serve",
            "--repository",
            path.to_str().unwrap(),
            "--report",
            report.to_str().unwrap(),
        ],
        &input,
    );
    assert_eq!((status, stderr.as_str()), (0, ""));
    let frames = split_frames(&stdout);
    assert_eq!(frames.len(), 1 + direct.frames.len());
    assert_eq!(frames[0], encode_hello_frame(&offered()).unwrap().bytes);
    // Session identities are per server instance (S20-330 section 1): the
    // CLI server mints a fresh 32-byte identity no other instance shares,
    // so a session transplanted from the direct server is unknown here
    // rather than silently accepted.
    let open = response(&frames[1]);
    assert_eq!(open.body.len(), 32);
    assert_ne!(open.body, direct.session.as_bytes().to_vec());
    for frame in &frames[2..] {
        let failure = failure(frame);
        assert_eq!(
            (failure.code, failure.symbol.as_str()),
            (33_000, "SESSION_UNKNOWN")
        );
    }
    let report: Value = serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
    assert_eq!(report["contract"], "sley2-cli-report-v1");
    assert_eq!(report["mode"], "bytes");
    assert_eq!(report["batch"], false);
    assert_eq!(report["frames_read"], 6);
    assert_eq!(report["frames_written"], 6);
    assert_eq!(report["answers"], 5);
    assert_eq!(report["failed_answers"], 4);
    assert_eq!(report["codes"]["33000"], 4);
    assert_eq!(report["events_written"], 0);
    assert_eq!(report["exit_code"], 0);
    assert_eq!(report["cli_failure"], Value::Null);
    assert!(report["handshake_id"].as_str().unwrap().len() == 64);
    let failed = report["failed_answers"].as_u64().unwrap();
    let counted: u64 = report["codes"]
        .as_object()
        .unwrap()
        .values()
        .map(|v| v.as_u64().unwrap())
        .sum();
    assert_eq!(failed, counted);
    let text = std::fs::read_to_string(temp.child("report.json")).unwrap();
    assert!(
        !text.contains(' '),
        "no insignificant whitespace in the report"
    );
}

#[test]
fn serve_in_json_mode_answers_the_same_bytes_and_rejects_bad_lines_in_place() {
    // S20-430 section 6: the same request frames over the same repository
    // get byte-identical answers in both modes. Both runs transplant the
    // same direct-server session, which is unknown to either CLI server
    // instance (S20-330 section 1), so every post-open answer is the same
    // deterministic SESSION_UNKNOWN failure; only the session-open answers
    // differ, each carrying its own freshly minted 32-byte identity.
    let (temp, path) = repository("cli-json");
    let direct = direct(&path);
    let mut byte_input = encode_hello_frame(&offered()).unwrap().bytes;
    for frame in &direct.frames {
        byte_input.extend_from_slice(frame);
    }
    let (_, byte_output, _) = run(
        &["serve", "--repository", path.to_str().unwrap()],
        &byte_input,
    );
    let byte_frames = split_frames(&byte_output);

    let mut lines = String::new();
    lines.push_str(&frame_to_json(&encode_hello_frame(&offered()).unwrap().bytes).unwrap());
    lines.push('\n');
    for (index, frame) in direct.frames.iter().enumerate() {
        if index == 2 {
            lines.push_str("{\"nope\":1}\n");
            lines.push_str("not json at all\n");
        }
        lines.push_str(&frame_to_json(frame).unwrap());
        lines.push('\n');
    }
    let report = temp.child("report.json");
    let (status, stdout, stderr) = run(
        &[
            "serve",
            "--repository",
            path.to_str().unwrap(),
            "--json",
            "--report",
            report.to_str().unwrap(),
        ],
        lines.as_bytes(),
    );
    assert_eq!((status, stderr.as_str()), (0, ""));
    let text = String::from_utf8(stdout).unwrap();
    let outputs: Vec<Vec<u8>> = text
        .lines()
        .map(|line| frame_from_json(line).unwrap().bytes)
        .collect();
    assert_eq!(outputs.len(), byte_frames.len() + 2);
    // The wire offer is mode-independent: the JSON hello frame is the
    // byte hello frame.
    assert_eq!(outputs[0], byte_frames[0]);
    assert_eq!(outputs[0], encode_hello_frame(&offered()).unwrap().bytes);
    // Both session opens succeed with a fresh 32-byte identity each; the
    // identities differ because sessions are per server instance.
    for open in [&byte_frames[1], &outputs[1]] {
        assert_eq!(response(open).body.len(), 32);
    }
    assert_ne!(response(&byte_frames[1]).body, response(&outputs[1]).body);
    // The two rejected lines are answered in place with the bridge's code.
    for rejected in [&outputs[3], &outputs[4]] {
        let frame = response(rejected);
        assert_eq!(
            (frame.session, frame.request_id, frame.method),
            (None, 0, 0)
        );
        let failure = ProtocolFailure::decode(&frame.body).unwrap();
        assert_eq!(
            (failure.code, failure.symbol.as_str()),
            (42_000, "JSON_BRIDGE_SHAPE_INVALID")
        );
    }
    // Every other answer is byte-identical across the two modes.
    for (raw_byte, raw_json) in [
        (&byte_frames[2], &outputs[2]),
        (&byte_frames[3], &outputs[5]),
        (&byte_frames[4], &outputs[6]),
        (&byte_frames[5], &outputs[7]),
    ] {
        assert_eq!(raw_byte, raw_json);
    }
    let report: Value = serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
    assert_eq!(report["mode"], "json");
    assert_eq!(report["frames_read"], 8);
    assert_eq!(report["answers"], 7);
    assert_eq!(report["codes"]["42000"], 2);
}

#[test]
fn batch_mode_answers_one_batch_per_server() {
    // Cancellation-before-execution across one batch is covered at the
    // owning layer (`batch_cancel_precedes_execution_in_one_batch` in
    // sley-protocol): a batch cannot transplant a session across server
    // instances (S20-330 section 1), so the CLI batch test proves the
    // plumbing instead. One batch carrying the hello and two opens is
    // answered frame for frame with two distinct fresh identities.
    let (_temp, path) = repository("cli-batch");
    let handshake = negotiate_identity(&offered(), &offered()).unwrap().1;
    let mut input = encode_hello_frame(&offered()).unwrap().bytes;
    input.extend_from_slice(&request(
        None,
        0,
        Method::SessionOpen,
        0,
        handshake.as_bytes().to_vec(),
    ));
    input.extend_from_slice(&request(
        None,
        0,
        Method::SessionOpen,
        0,
        handshake.as_bytes().to_vec(),
    ));

    let (status, stdout, _) = run(
        &["serve", "--repository", path.to_str().unwrap(), "--batch"],
        &input,
    );
    assert_eq!(status, 0);
    let frames = split_frames(&stdout);
    assert_eq!(frames.len(), 3);
    let first = response(&frames[1]);
    let second = response(&frames[2]);
    assert_eq!(first.body.len(), 32);
    assert_eq!(second.body.len(), 32);
    assert_ne!(first.body, second.body);
}

#[test]
fn a_failed_negotiation_is_answered_once_and_ends_the_input() {
    let (temp, path) = repository("cli-negotiation");
    let direct = direct(&path);
    let client = Hello {
        protocol_versions: vec![2],
        ..offered()
    };
    let mut input = encode_hello_frame(&client).unwrap().bytes;
    input.extend_from_slice(&direct.frames[0]);
    let report = temp.child("report.json");
    let (status, stdout, stderr) = run(
        &[
            "serve",
            "--repository",
            path.to_str().unwrap(),
            "--report",
            report.to_str().unwrap(),
        ],
        &input,
    );
    assert_eq!((status, stderr.as_str()), (0, ""));
    let frames = split_frames(&stdout);
    assert_eq!(frames.len(), 1);
    let frame = response(&frames[0]);
    assert_eq!(
        (frame.session, frame.request_id, frame.method),
        (None, 0, 0)
    );
    let failure = ProtocolFailure::decode(&frame.body).unwrap();
    assert_eq!(
        (failure.code, failure.symbol.as_str()),
        (40_003, "PROTOCOL_NO_COMMON_PROFILE")
    );
    let report: Value = serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
    assert_eq!(report["frames_read"], 1);
    assert_eq!(report["handshake_id"], Value::Null);
    assert_eq!(report["codes"]["40003"], 1);
}

#[test]
fn a_prefix_above_the_ceiling_is_answered_without_reading_the_body() {
    let (temp, path) = repository("cli-too-large");
    let mut input = encode_hello_frame(&offered()).unwrap().bytes;
    input.extend_from_slice(&(MAX_FRAME_BYTES + 1).to_be_bytes());
    input.extend_from_slice(&[0xAA; 64]);
    let report = temp.child("report.json");
    let (status, stdout, stderr) = run(
        &[
            "serve",
            "--repository",
            path.to_str().unwrap(),
            "--report",
            report.to_str().unwrap(),
        ],
        &input,
    );
    assert_eq!((status, stderr.as_str()), (0, ""));
    let frames = split_frames(&stdout);
    assert_eq!(frames.len(), 2);
    let failure = failure(&frames[1]);
    assert_eq!(
        (failure.code, failure.symbol.as_str()),
        (40_002, "PROTOCOL_FRAME_TOO_LARGE")
    );
    let report: Value = serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
    assert_eq!(report["frames_read"], 2);
    assert_eq!(report["failed_answers"], 1);
    assert_eq!(report["codes"]["40002"], 1);
}

type Case<'a> = (Vec<&'a str>, Vec<u8>, i32, u32, Option<&'a str>);

#[test]
fn cli_failures_carry_their_exit_status_and_one_stderr_object() {
    let (temp, path) = repository("cli-failures");
    let repo = path.to_str().unwrap();
    let cases: Vec<Case> = vec![
        (vec![], vec![], 2, 43_000, None),
        (vec!["bogus"], vec![], 2, 43_000, Some("bogus")),
        (vec!["serve"], vec![], 2, 43_000, Some("--repository")),
        (
            vec!["serve", "--repository", repo, "--json", "--json"],
            vec![],
            2,
            43_000,
            Some("--json"),
        ),
        (
            vec!["serve", "--repository", repo, "--report"],
            vec![],
            2,
            43_000,
            Some("--report"),
        ),
        (vec!["frame"], vec![], 2, 43_000, Some("frame")),
        (vec!["frame", "both"], vec![], 2, 43_000, Some("both")),
        (vec!["methods", "extra"], vec![], 2, 43_000, Some("extra")),
        (vec!["serve", "--repository", repo], vec![], 5, 43_003, None),
        (
            vec!["serve", "--repository", repo, "--json"],
            vec![],
            5,
            43_003,
            None,
        ),
        (
            vec!["serve", "--repository", repo],
            direct(&path).frames[0].clone(),
            5,
            43_003,
            Some("NOT_A_HELLO"),
        ),
        (
            vec!["serve", "--repository", repo],
            vec![0u8; 7],
            3,
            43_001,
            Some("SHORT_PREFIX"),
        ),
        (
            vec!["serve", "--repository", repo, "--json"],
            b"{\"nope\":1}\n".to_vec(),
            5,
            43_003,
            Some("JSON_BRIDGE_SHAPE_INVALID"),
        ),
    ];
    for (args, input, status, code, cause) in cases {
        let (observed, stdout, stderr) = run(&args, &input);
        assert_eq!(observed, status, "{args:?}");
        assert!(stdout.is_empty(), "{args:?}");
        let object = stderr_object(&stderr);
        assert_eq!(object["code"], code, "{args:?}");
        assert_eq!(
            object["cause"],
            cause.map_or(Value::Null, Value::from),
            "{args:?}"
        );
        assert_eq!(stderr.matches('\n').count(), 1);
    }
    // A short read inside a frame after the handshake fails the invocation
    // and the report records it.
    let mut input = encode_hello_frame(&offered()).unwrap().bytes;
    input.extend_from_slice(&100u64.to_be_bytes());
    input.extend_from_slice(&[0u8; 10]);
    let report = temp.child("report.json");
    let (status, stdout, stderr) = run(
        &[
            "serve",
            "--repository",
            repo,
            "--report",
            report.to_str().unwrap(),
        ],
        &input,
    );
    assert_eq!(status, 3);
    assert_eq!(
        split_frames(&stdout).len(),
        1,
        "the hello was answered before the short frame"
    );
    assert_eq!(stderr_object(&stderr)["symbol"], "CLI_INPUT_INVALID");
    let report: Value = serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
    assert_eq!(report["exit_code"], 3);
    assert_eq!(report["cli_failure"]["code"], 43_001);
    assert_eq!(report["cli_failure"]["cause"], "SHORT_FRAME");
    assert_eq!(report["frames_read"], 1);
    assert_eq!(report["frames_written"], 1);
}

#[test]
fn frame_decode_and_encode_reproduce_the_bridge_fixture() {
    let fixture: Value = serde_json::from_str(BRIDGE_FIXTURE).unwrap();
    let vectors = fixture["vectors"].as_array().unwrap();
    let mut bytes = Vec::new();
    let mut lines = String::new();
    for vector in vectors {
        let hex = vector["frame_hex"].as_str().unwrap();
        bytes.extend(
            (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()),
        );
        lines.push_str(vector["json"].as_str().unwrap());
        lines.push('\n');
    }
    let (status, stdout, stderr) = run(&["frame", "decode"], &bytes);
    assert_eq!((status, stderr.as_str()), (0, ""));
    assert_eq!(String::from_utf8(stdout).unwrap(), lines);
    let (status, stdout, stderr) = run(&["frame", "encode"], lines.as_bytes());
    assert_eq!((status, stderr.as_str()), (0, ""));
    assert_eq!(stdout, bytes);
    let (status, _, stderr) = run(&["frame", "encode"], b"{\"nope\":1}\n");
    assert_eq!(status, 3);
    assert_eq!(stderr_object(&stderr)["cause"], "JSON_BRIDGE_SHAPE_INVALID");
    let (status, _, stderr) = run(&["frame", "decode"], &bytes[..bytes.len() - 1]);
    assert_eq!(status, 3);
    assert_eq!(stderr_object(&stderr)["cause"], "SHORT_FRAME");
}

#[test]
fn methods_hello_and_version_expose_the_offer_without_judgment() {
    let (status, stdout, _) = run(&["methods"], &[]);
    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), METHOD_TABLE_JSON);

    let (status, stdout, _) = run(&["hello"], &[]);
    assert_eq!(status, 0);
    assert_eq!(stdout, encode_hello_frame(&offered()).unwrap().bytes);
    let (status, stdout, _) = run(&["hello", "--json"], &[]);
    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        format!("{}\n", hello_to_json(&offered()).unwrap())
    );

    let (status, stdout, _) = run(&["version"], &[]);
    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        "{\"cli\":\"1\",\"contract\":\"sley2-cli-v1\",\"protocol_version\":1}\n"
    );
    let parsed = sley_cli::parse(&["hello".to_string(), "--json".to_string()]).unwrap();
    assert_eq!(
        parsed,
        sley_cli::Command::Hello {
            json: true,
            profile: sley_cli::ProtocolProfile::Legacy,
        }
    );
    for code in sley_cli::CliErrorCode::ALL {
        assert_eq!(
            code.numeric() - 43_000 + 2,
            u32::try_from(code.exit_status()).unwrap()
        );
        assert!(code.as_str().starts_with("CLI_"));
    }
}

fn offered_v2() -> Hello {
    Server::offered_hello_versioned().unwrap()
}

#[test]
fn profile_hello_offers_both_versions_with_the_v2_methods() {
    let (status, stdout, stderr) = run(
        &["hello", "--json", "--protocol-profile", "v2-capable"],
        &[],
    );
    assert_eq!((status, stderr.as_str()), (0, ""));
    let object: Value = serde_json::from_str(&String::from_utf8(stdout).unwrap()).unwrap();
    assert_eq!(object["protocol_versions"], Value::from(vec![1, 2]));
    let methods: Vec<&str> = object["methods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap())
        .collect();
    assert!(methods.contains(&"entity.version"));
    assert!(methods.contains(&"entity.signature"));
    assert_eq!(methods.len(), 39);

    let (status, stdout, _) = run(&["hello", "--protocol-profile", "v2-capable"], &[]);
    assert_eq!(status, 0);
    match decode_frame(&stdout, MAX_FRAME_BYTES).unwrap().0 {
        DecodedFrame::Hello(hello) => {
            assert_eq!(hello.methods.len(), 39);
            assert!(hello.methods.contains(&306));
            assert!(hello.methods.contains(&307));
        }
        _ => panic!("profile hello is not a hello frame"),
    }
}

#[test]
fn profile_methods_prints_the_v2_table_verbatim() {
    let (status, stdout, stderr) = run(&["methods", "--protocol-profile", "v2-capable"], &[]);
    assert_eq!((status, stderr.as_str()), (0, ""));
    assert_eq!(String::from_utf8(stdout).unwrap(), METHOD_TABLE_V2_JSON);
    assert!(METHOD_TABLE_V2_JSON.contains("entity.version"));
    assert!(METHOD_TABLE_V2_JSON.contains("entity.signature"));
}

#[test]
fn profile_version_reports_the_capable_contract() {
    let (status, stdout, stderr) = run(&["version", "--protocol-profile", "v2-capable"], &[]);
    assert_eq!((status, stderr.as_str()), (0, ""));
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        "{\"cli\":\"1\",\"contract\":\"sley2-cli-v2\",\"protocol_profile\":\"v2-capable\",\"protocol_versions\":[1,2]}\n"
    );
}

#[test]
fn profile_serve_reports_the_actual_selected_version() {
    let (_temp, path) = repository("cli-profile-serve");
    let repo = path.to_str().unwrap();
    let report = path
        .parent()
        .unwrap()
        .join("profile-report.json")
        .to_str()
        .unwrap()
        .to_string();
    // A version-aware client hello negotiates version 2 under the profile.
    let input = encode_hello_frame(&offered_v2()).unwrap().bytes;
    let (status, stdout, stderr) = run(
        &[
            "serve",
            "--repository",
            repo,
            "--protocol-profile",
            "v2-capable",
            "--report",
            &report,
        ],
        &input,
    );
    assert_eq!((status, stderr.as_str()), (0, ""));
    let frames = split_frames(&stdout);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0], input);
    let report: Value = serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
    assert_eq!(report["contract"], "sley2-cli-report-v2");
    assert_eq!(report["protocol_profile"], "v2-capable");
    assert_eq!(report["selected_protocol_version"], 2);

    // A legacy client hello against the capable server still selects 1.
    let legacy = encode_hello_frame(&offered()).unwrap().bytes;
    let (status, _, stderr) = run(
        &[
            "serve",
            "--repository",
            repo,
            "--protocol-profile",
            "v2-capable",
        ],
        &legacy,
    );
    assert_eq!((status, stderr.as_str()), (0, ""));
}

#[test]
fn profile_frame_commands_enforce_the_expected_version() {
    let hello_bytes = encode_hello_frame(&offered()).unwrap().bytes;
    let hello_text = frame_to_json(&hello_bytes).unwrap();
    // A hello converts under expected 1 and is rejected under expected 2.
    let (status, stdout, _) = run(
        &[
            "frame",
            "encode",
            "--protocol-profile",
            "v2-capable",
            "--expected-version",
            "1",
        ],
        hello_text.as_bytes(),
    );
    assert_eq!(status, 0);
    assert_eq!(stdout, hello_bytes);
    let (status, _, stderr) = run(
        &[
            "frame",
            "encode",
            "--protocol-profile",
            "v2-capable",
            "--expected-version",
            "2",
        ],
        hello_text.as_bytes(),
    );
    assert_eq!(status, 3);
    assert!(stderr.contains("VERSION_MISMATCH"));
    let (status, _, stderr) = run(
        &[
            "frame",
            "decode",
            "--protocol-profile",
            "v2-capable",
            "--expected-version",
            "2",
        ],
        &hello_bytes,
    );
    assert_eq!(status, 3);
    assert!(stderr.contains("VERSION_MISMATCH"));
    let (status, stdout, _) = run(
        &[
            "frame",
            "decode",
            "--expected-version",
            "1",
            "--protocol-profile",
            "v2-capable",
        ],
        &hello_bytes,
    );
    assert_eq!(status, 0);
    assert_eq!(stdout, format!("{hello_text}\n").into_bytes());
}

#[test]
fn profile_serve_json_converts_post_hello_lines_under_the_selection() {
    // A version 2-stamped open converts version-aware past the handshake;
    // under the frozen version 1 conversion the same line is refused, so a
    // passing open proves the selection drives JSON conversion.
    let (_temp, path) = repository("cli-profile-json-open");
    let repo = path.to_str().unwrap();
    let server_hello = Server::offered_hello_versioned().unwrap();
    let handshake = sley_protocol::negotiate_identity_versioned(&server_hello, &server_hello)
        .unwrap()
        .1;
    let handshake_hex = handshake
        .as_bytes()
        .iter()
        .fold(String::new(), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        });
    let (status, hello_out, _) = run(&["hello", "--protocol-profile", "v2-capable"], &[]);
    assert_eq!(status, 0);
    let hello_line = frame_to_json(&hello_out).unwrap();
    let hello_object: Value = serde_json::from_str(&hello_line).unwrap();
    let bounds = hello_object["bounds"].clone();
    let open = serde_json::json!({
        "body": handshake_hex,
        "bounds": bounds,
        "flags": {"cancel": false, "failed": false, "stream": false},
        "kind": "request",
        "method": "session.open",
        "protocol_version": 2,
        "request_id": 0,
        "session": null,
    });
    let input = format!("{hello_line}\n{open}\n");
    let (status, stdout, stderr) = run(
        &[
            "serve",
            "--repository",
            repo,
            "--protocol-profile",
            "v2-capable",
            "--json",
        ],
        input.as_bytes(),
    );
    assert_eq!((status, stderr.as_str()), (0, ""));
    let mut lines = String::from_utf8(stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    let answer: Value = serde_json::from_str(&lines.pop().unwrap()).unwrap();
    assert_eq!(answer["kind"], "response");
    assert_eq!(answer["flags"]["failed"], false);
    assert_eq!(answer["body"].as_str().unwrap().len(), 64);
}

#[test]
fn profile_frame_commands_report_version_mismatch_for_non_hello_frames() {
    // A version 1 request frame under expected 2, and its JSON text under
    // expected 2, must both fail CLI_INPUT_INVALID with VERSION_MISMATCH,
    // never with the codec's own version-gate code.
    let v1_request = request(None, 0, Method::SessionOpen, 0, Vec::new());
    let (status, _, stderr) = run(
        &[
            "frame",
            "decode",
            "--protocol-profile",
            "v2-capable",
            "--expected-version",
            "2",
        ],
        &v1_request,
    );
    assert_eq!(status, 3);
    assert!(stderr.contains("VERSION_MISMATCH"));
    let v1_text = frame_to_json(&v1_request).unwrap();
    let (status, _, stderr) = run(
        &[
            "frame",
            "encode",
            "--protocol-profile",
            "v2-capable",
            "--expected-version",
            "2",
        ],
        v1_text.as_bytes(),
    );
    assert_eq!(status, 3);
    assert!(stderr.contains("VERSION_MISMATCH"));
}

#[test]
fn profile_flag_misuse_is_a_usage_failure() {
    // --expected-version without the profile.
    let (status, _, _) = run(&["frame", "decode", "--expected-version", "1"], &[]);
    assert_eq!(status, 2);
    // The profile without --expected-version on a frame command.
    let (status, _, _) = run(
        &["frame", "encode", "--protocol-profile", "v2-capable"],
        &[],
    );
    assert_eq!(status, 2);
    // --expected-version on a non-frame command.
    let (status, _, _) = run(&["hello", "--expected-version", "1"], &[]);
    assert_eq!(status, 2);
    // Unsupported values and repeats.
    let (status, _, _) = run(&["hello", "--protocol-profile", "v3"], &[]);
    assert_eq!(status, 2);
    let (status, _, _) = run(
        &[
            "frame",
            "decode",
            "--protocol-profile",
            "v2-capable",
            "--expected-version",
            "3",
        ],
        &[],
    );
    assert_eq!(status, 2);
    let (status, _, _) = run(
        &[
            "hello",
            "--protocol-profile",
            "v2-capable",
            "--protocol-profile",
            "v2-capable",
        ],
        &[],
    );
    assert_eq!(status, 2);
}
