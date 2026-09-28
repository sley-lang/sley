//! Bounded private socket client for one native test attempt.
//!
//! The client checks the root peer before sending a program, enforces one
//! deadline across connect, write, and response, and returns exactly one
//! canonical supervisor frame. It does not grant measurement trust or native
//! admission; the transaction owner verifies the response against the
//! request and its receiver-provisioned measurement manifest.

use std::io::{self, Read, Write};
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use nix::sys::socket::{
    AddressFamily, SockFlag, SockType, UnixAddr, connect, getsockopt, socket,
    sockopt::{PeerCredentials, SocketError},
};
use sley_scb1::ScbErrorCode;

use crate::ingress::MAX_INGRESS_TIMEOUT;
use crate::protocol::{RUN_MAGIC, RunRequest, RunResponse};
use crate::response::MAX_RESPONSE_FRAME_BYTES;

/// Failure before the transaction owner receives a verified measurement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientError {
    /// Zero or excessive end-to-end deadline.
    InvalidDeadline,
    /// The proposed request is not a canonical selected Sley program.
    InvalidRequest(ScbErrorCode),
    /// The fixed local endpoint could not be connected before the deadline.
    ConnectFailure,
    /// The connect, write, or response deadline expired.
    DeadlineReached,
    /// The connected service is not the expected root-owned peer.
    UnauthenticatedServer,
    /// Request write or half-close failed.
    WriteFailure,
    /// Response read failed or ended before its declared length.
    ReadFailure,
    /// The response declared more than the closed frame bound.
    FrameTooLarge,
    /// The one-shot service wrote bytes after its declared frame.
    TrailingResponse,
    /// The complete response frame failed the canonical decoder.
    MalformedResponse(ScbErrorCode),
}

fn remaining(deadline: Instant) -> Result<Duration, ClientError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or(ClientError::DeadlineReached)
}

fn connect_before(path: &Path, deadline: Instant) -> Result<UnixStream, ClientError> {
    let address = UnixAddr::new(path).map_err(|_| ClientError::ConnectFailure)?;
    let fd = socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
        None,
    )
    .map_err(|_| ClientError::ConnectFailure)?;
    match connect(fd.as_raw_fd(), &address) {
        Ok(()) => {}
        Err(Errno::EINPROGRESS | Errno::EAGAIN) => loop {
            let milliseconds = remaining(deadline)?.as_millis().max(1);
            let milliseconds =
                PollTimeout::try_from(milliseconds).map_err(|_| ClientError::InvalidDeadline)?;
            let mut fds = [PollFd::new(fd.as_fd(), PollFlags::POLLOUT)];
            match poll(&mut fds, milliseconds) {
                Ok(0) | Err(Errno::EINTR) => {}
                Ok(_) => {
                    let pending =
                        getsockopt(&fd, SocketError).map_err(|_| ClientError::ConnectFailure)?;
                    if pending != 0 {
                        return Err(ClientError::ConnectFailure);
                    }
                    break;
                }
                Err(_) => return Err(ClientError::ConnectFailure),
            }
        },
        Err(_) => return Err(ClientError::ConnectFailure),
    }
    remaining(deadline)?;
    let stream = UnixStream::from(fd);
    stream
        .set_nonblocking(false)
        .map_err(|_| ClientError::ConnectFailure)?;
    Ok(stream)
}

fn write_before(
    stream: &mut UnixStream,
    mut bytes: &[u8],
    deadline: Instant,
) -> Result<(), ClientError> {
    while !bytes.is_empty() {
        stream
            .set_write_timeout(Some(remaining(deadline)?))
            .map_err(|_| ClientError::WriteFailure)?;
        match stream.write(bytes) {
            Ok(0) => return Err(ClientError::WriteFailure),
            Ok(count) => bytes = &bytes[count..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                return Err(ClientError::DeadlineReached);
            }
            Err(_) => return Err(ClientError::WriteFailure),
        }
    }
    Ok(())
}

fn read_before(
    stream: &mut UnixStream,
    mut bytes: &mut [u8],
    deadline: Instant,
) -> Result<(), ClientError> {
    while !bytes.is_empty() {
        stream
            .set_read_timeout(Some(remaining(deadline)?))
            .map_err(|_| ClientError::ReadFailure)?;
        match stream.read(bytes) {
            Ok(0) => return Err(ClientError::ReadFailure),
            Ok(count) => bytes = &mut bytes[count..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                return Err(ClientError::DeadlineReached);
            }
            Err(_) => return Err(ClientError::ReadFailure),
        }
    }
    Ok(())
}

fn exchange_for_uid(
    path: &Path,
    request: &RunRequest,
    timeout: Duration,
    expected_uid: u32,
) -> Result<Vec<u8>, ClientError> {
    if timeout.is_zero() || timeout > MAX_INGRESS_TIMEOUT {
        return Err(ClientError::InvalidDeadline);
    }
    let frame = request
        .encode_frame()
        .map_err(|error| ClientError::InvalidRequest(error.code()))?;
    request
        .verified_program()
        .map_err(|error| ClientError::InvalidRequest(error.code()))?;
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(ClientError::InvalidDeadline)?;
    let mut stream = connect_before(path, deadline)?;
    let peer_uid = getsockopt(&stream, PeerCredentials)
        .map_err(|_| ClientError::UnauthenticatedServer)?
        .uid();
    if peer_uid != expected_uid {
        return Err(ClientError::UnauthenticatedServer);
    }
    write_before(&mut stream, &frame, deadline)?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|_| ClientError::WriteFailure)?;
    let mut header = [0_u8; 12];
    read_before(&mut stream, &mut header, deadline)?;
    if header[..8] != *RUN_MAGIC {
        return Err(ClientError::MalformedResponse(ScbErrorCode::MagicInvalid));
    }
    let body_len = usize::try_from(u32::from_be_bytes(
        header[8..12]
            .try_into()
            .map_err(|_| ClientError::ReadFailure)?,
    ))
    .map_err(|_| ClientError::FrameTooLarge)?;
    if body_len > MAX_RESPONSE_FRAME_BYTES - header.len() {
        return Err(ClientError::FrameTooLarge);
    }
    let mut response = vec![0_u8; header.len() + body_len];
    response[..header.len()].copy_from_slice(&header);
    read_before(&mut stream, &mut response[header.len()..], deadline)?;
    let mut trailing = [0_u8; 1];
    loop {
        stream
            .set_read_timeout(Some(remaining(deadline)?))
            .map_err(|_| ClientError::ReadFailure)?;
        match stream.read(&mut trailing) {
            Ok(0) => break,
            Ok(_) => return Err(ClientError::TrailingResponse),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                return Err(ClientError::DeadlineReached);
            }
            Err(_) => return Err(ClientError::ReadFailure),
        }
    }
    RunResponse::decode_frame(&response)
        .map_err(|error| ClientError::MalformedResponse(error.code()))?;
    Ok(response)
}

/// Exchanges one selected Sley run with the root-owned local supervisor.
///
/// The endpoint path is administrator configured. A non-root peer is refused
/// before request bytes are sent, and the returned frame still needs the
/// transaction owner's request-binding and measurement-trust checks.
///
/// # Errors
///
/// Returns the first request, connection, identity, deadline, or response
/// failure. No failure grants a native test result.
pub fn run_native_test(
    socket_path: &Path,
    request: &RunRequest,
    timeout: Duration,
) -> Result<Vec<u8>, ClientError> {
    exchange_for_uid(socket_path, request, timeout, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::config::{AllowedCaller, default_config};
    use crate::program::PortableTestProgram;
    use crate::protocol::{RunResponse, RunStatus};
    use crate::service::{RUN_REFUSAL_EXECUTION_NOT_WIRED, serve_one};
    use crate::worker::WorkerRequest;

    static NEXT_SOCKET: AtomicU64 = AtomicU64::new(0);

    fn listener() -> (UnixListener, std::path::PathBuf, u32) {
        let path = std::env::temp_dir().join(format!(
            "sley-native-client-{}-{}",
            std::process::id(),
            NEXT_SOCKET.fetch_add(1, Ordering::Relaxed)
        ));
        let listener = UnixListener::bind(&path).expect("bind local test socket");
        let uid = std::fs::symlink_metadata(&path)
            .expect("socket owner")
            .uid();
        (listener, path, uid)
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
    fn client_exchanges_one_canonical_refusal_with_authenticated_service() {
        let (listener, path, uid) = listener();
        let request = request();
        let config = default_config(
            "/run/sley-test-supervisor",
            "/usr/lib/sley/sley-native-test-worker",
            [7; 32],
            [8; 32],
            vec![AllowedCaller {
                uid,
                workspace: request.workspace,
                principal: request.principal,
            }],
            "/etc/sley-test-supervisor/measurement.key",
            "/etc/sley-test-supervisor/trust",
        )
        .expect("service config");
        let service = std::thread::spawn(move || {
            serve_one(&listener, &config, Duration::from_secs(1)).expect("serve refusal");
        });
        let frame = exchange_for_uid(&path, &request, Duration::from_secs(1), uid)
            .expect("closed response");
        assert_eq!(
            RunResponse::decode_frame(&frame).expect("canonical frame"),
            RunResponse {
                status: RunStatus::Refused,
                code: RUN_REFUSAL_EXECUTION_NOT_WIRED,
                evidence: None,
                no_result: None,
            }
        );
        service.join().expect("service thread");
        std::fs::remove_file(path).expect("remove socket");
    }

    #[test]
    fn client_refuses_wrong_peer_before_sending_program() {
        let (listener, path, uid) = listener();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept peer");
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).expect("read peer bytes");
            bytes
        });
        assert_eq!(
            exchange_for_uid(
                &path,
                &request(),
                Duration::from_secs(1),
                uid.wrapping_add(1)
            ),
            Err(ClientError::UnauthenticatedServer)
        );
        assert!(server.join().expect("server thread").is_empty());
        std::fs::remove_file(path).expect("remove socket");
    }

    #[test]
    fn client_rejects_oversized_or_trailing_responses() {
        for oversized in [true, false] {
            let (listener, path, uid) = listener();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("accept peer");
                let mut request_bytes = Vec::new();
                stream
                    .read_to_end(&mut request_bytes)
                    .expect("read full request");
                RunRequest::decode_frame(&request_bytes).expect("valid request frame");
                let response = if oversized {
                    let mut header = RUN_MAGIC.to_vec();
                    header.extend_from_slice(&u32::MAX.to_be_bytes());
                    header
                } else {
                    let mut frame = RunResponse {
                        status: RunStatus::Refused,
                        code: 1,
                        evidence: None,
                        no_result: None,
                    }
                    .encode_frame()
                    .expect("response frame");
                    frame.push(0xff);
                    frame
                };
                stream.write_all(&response).expect("write response");
            });
            let error = exchange_for_uid(&path, &request(), Duration::from_secs(1), uid)
                .expect_err("bad response refuses");
            assert_eq!(
                error,
                if oversized {
                    ClientError::FrameTooLarge
                } else {
                    ClientError::TrailingResponse
                }
            );
            server.join().expect("server thread");
            std::fs::remove_file(path).expect("remove socket");
        }
    }

    #[test]
    fn client_deadline_covers_stalled_response() {
        let (listener, path, uid) = listener();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept peer");
            let mut request_bytes = Vec::new();
            stream
                .read_to_end(&mut request_bytes)
                .expect("read full request");
            RunRequest::decode_frame(&request_bytes).expect("valid request frame");
            std::thread::sleep(Duration::from_millis(100));
        });
        assert_eq!(
            exchange_for_uid(&path, &request(), Duration::from_millis(20), uid),
            Err(ClientError::DeadlineReached)
        );
        server.join().expect("server thread");
        std::fs::remove_file(path).expect("remove socket");
    }
}
