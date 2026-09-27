//! Fail-closed socket service boundary before worker execution is wired.
//!
//! This handles one connection on a caller-supplied Unix listener. The
//! existing ingress authenticates kernel peer credentials, scope, framing,
//! and deadline before a response is written. The authenticated request's
//! portable program and input hashes are checked before the current explicit
//! execution refusal. Invalid or unauthorized ingress peers receive no response.
//! No worker is launched, no measurement is signed, and no production daemon
//! entry or systemd unit is supplied by this module.

use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::Duration;

use crate::config::RunnerConfig;
use crate::ingress::{IngressError, authenticate_request};
use crate::protocol::{RunResponse, RunStatus};

/// Stable refusal code while the selected-program handoff is absent.
pub const RUN_REFUSAL_EXECUTION_NOT_WIRED: u32 = 1;
/// Stable refusal for an authenticated request with invalid program bindings.
pub const RUN_REFUSAL_PROGRAM_INVALID: u32 = 2;

/// A socket service failure; none confers a test result or attestation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceError {
    /// A peer failed authentication, scope, framing, or the read deadline.
    Ingress(IngressError),
    /// The listener could not accept a connection.
    AcceptFailure,
    /// The closed refusal frame could not be encoded.
    ResponseEncoding,
    /// The peer disconnected or its response could not be written.
    WriteFailure,
}

/// Handles one already accepted socket connection, always refusing execution.
///
/// # Errors
///
/// Returns an ingress or transport failure without signing or spawning.
pub fn handle_connection(
    stream: &mut UnixStream,
    config: &RunnerConfig,
    timeout: Duration,
) -> Result<(), ServiceError> {
    let authenticated =
        authenticate_request(stream, config, timeout).map_err(ServiceError::Ingress)?;
    let code = if authenticated.request().verified_program().is_ok() {
        RUN_REFUSAL_EXECUTION_NOT_WIRED
    } else {
        RUN_REFUSAL_PROGRAM_INVALID
    };
    let response = RunResponse {
        status: RunStatus::Refused,
        code,
        evidence: None,
    };
    let frame = response
        .encode_frame()
        .map_err(|_| ServiceError::ResponseEncoding)?;
    stream
        .write_all(&frame)
        .map_err(|_| ServiceError::WriteFailure)
}

/// Accepts and handles exactly one connection from a trusted listener.
///
/// This is a composable boundary for the eventual root loop; it does not
/// bind a path, install a service, or claim execution readiness.
///
/// # Errors
///
/// Returns the first accept, ingress, or response error.
pub fn serve_one(
    listener: &UnixListener,
    config: &RunnerConfig,
    timeout: Duration,
) -> Result<(), ServiceError> {
    let (mut stream, _) = listener.accept().map_err(|_| ServiceError::AcceptFailure)?;
    handle_connection(&mut stream, config, timeout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::os::unix::fs::MetadataExt;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::config::{AllowedCaller, default_config};
    use crate::program::PortableTestProgram;
    use crate::protocol::RunRequest;
    use crate::worker::WorkerRequest;

    static NEXT_SOCKET: AtomicU64 = AtomicU64::new(0);

    fn test_listener() -> (UnixListener, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "sley-native-service-{}-{}",
            std::process::id(),
            NEXT_SOCKET.fetch_add(1, Ordering::Relaxed)
        ));
        (UnixListener::bind(&path).expect("bind test socket"), path)
    }

    fn config(uid: u32) -> RunnerConfig {
        let scope = request();
        default_config(
            "/run/sley-test-supervisor",
            "/usr/lib/sley/sley-native-test-worker",
            [7; 32],
            [8; 32],
            vec![AllowedCaller {
                uid,
                workspace: scope.workspace,
                principal: scope.principal,
            }],
            "/etc/sley-test-supervisor/measurement.key",
            "/etc/sley-test-supervisor/trust",
        )
        .expect("test config")
    }

    fn request() -> RunRequest {
        let worker = WorkerRequest::decode_frame(include_bytes!(
            "../../../conformance/native-worker/v1/observed-input.bin"
        ))
        .expect("canonical worker vector");
        let program = PortableTestProgram::parse(&worker.program_bytes).expect("portable program");
        RunRequest::from_portable_program(&program, 1_000, [9; 32]).expect("supervisor request")
    }

    #[test]
    fn authenticated_socket_request_receives_only_not_wired_refusal() {
        let (listener, path) = test_listener();
        let uid = std::fs::symlink_metadata(&path)
            .expect("socket metadata")
            .uid();
        let client = std::thread::spawn({
            let path = path.clone();
            move || {
                let mut stream = UnixStream::connect(path).expect("connect");
                stream
                    .write_all(&request().encode_frame().expect("frame"))
                    .expect("write request");
                let mut bytes = Vec::new();
                stream.read_to_end(&mut bytes).expect("read refusal");
                bytes
            }
        });
        assert_eq!(
            serve_one(&listener, &config(uid), Duration::from_secs(1)),
            Ok(())
        );
        let reply = client.join().expect("client thread");
        assert_eq!(
            RunResponse::decode_frame(&reply).expect("closed response"),
            RunResponse {
                status: RunStatus::Refused,
                code: RUN_REFUSAL_EXECUTION_NOT_WIRED,
                evidence: None,
            }
        );
        std::fs::remove_file(path).expect("remove socket");
    }

    #[test]
    fn unauthorized_peer_receives_no_response() {
        let (listener, path) = test_listener();
        let uid = std::fs::symlink_metadata(&path)
            .expect("socket metadata")
            .uid();
        let client = std::thread::spawn({
            let path = path.clone();
            move || {
                let mut stream = UnixStream::connect(path).expect("connect");
                let mut bytes = Vec::new();
                stream.read_to_end(&mut bytes).expect("read EOF");
                bytes
            }
        });
        assert_eq!(
            serve_one(
                &listener,
                &config(uid.wrapping_add(1)),
                Duration::from_secs(1)
            ),
            Err(ServiceError::Ingress(IngressError::UnauthorizedPeer))
        );
        assert!(client.join().expect("client thread").is_empty());
        std::fs::remove_file(path).expect("remove socket");
    }

    #[test]
    fn authenticated_program_substitution_is_refused_before_execution() {
        let (listener, path) = test_listener();
        let uid = std::fs::symlink_metadata(&path)
            .expect("socket metadata")
            .uid();
        let client = std::thread::spawn({
            let path = path.clone();
            move || {
                let mut changed = request();
                let mut worker = WorkerRequest::decode_frame(&changed.worker_frame)
                    .expect("valid worker envelope");
                worker.input_hashes[0] = [99; 32];
                changed.worker_frame = worker.encode_frame().expect("substituted worker");
                let mut stream = UnixStream::connect(path).expect("connect");
                stream
                    .write_all(&changed.encode_frame().expect("outer frame"))
                    .expect("write request");
                let mut bytes = Vec::new();
                stream.read_to_end(&mut bytes).expect("read refusal");
                bytes
            }
        });
        assert_eq!(
            serve_one(&listener, &config(uid), Duration::from_secs(1)),
            Ok(())
        );
        let reply = client.join().expect("client thread");
        assert_eq!(
            RunResponse::decode_frame(&reply).expect("closed response"),
            RunResponse {
                status: RunStatus::Refused,
                code: RUN_REFUSAL_PROGRAM_INVALID,
                evidence: None,
            }
        );
        std::fs::remove_file(path).expect("remove socket");
    }
}
