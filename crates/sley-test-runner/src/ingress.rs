//! Authenticated, bounded ingress for the native test supervisor socket.
//!
//! The kernel supplies the peer UID. An administrator-owned configuration
//! binds that UID to exactly one workspace and principal. Credentials are
//! checked before reading any request bytes; the complete frame is bounded
//! before allocation and must match that scope. This module admits a request
//! to the runner only. It does not launch a worker or sign a measurement.

use std::io::{self, Read};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use nix::sys::socket::{getsockopt, sockopt::PeerCredentials};
use sley_scb1::ScbErrorCode;

use crate::config::{MAX_REQUEST_BYTES, RunnerConfig};
use crate::protocol::{RUN_MAGIC, RunRequest};

/// Maximum time for one request frame, including a slow or stalled peer.
pub const MAX_INGRESS_TIMEOUT: Duration = Duration::from_secs(35);

/// Failure before a request reaches the worker or signing path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IngressError {
    /// Administrator configuration is malformed.
    InvalidConfiguration,
    /// Peer credentials could not be obtained from the kernel.
    CredentialsUnavailable,
    /// The peer UID has no administrator-approved caller binding.
    UnauthorizedPeer,
    /// The request names another workspace or principal.
    ScopeMismatch,
    /// The requested ingress deadline is zero or above the watchdog ceiling.
    InvalidDeadline,
    /// The peer stalled before a complete request arrived.
    DeadlineReached,
    /// Socket input ended early or failed.
    ReadFailure,
    /// The length prefix would exceed the closed request bound.
    FrameTooLarge,
    /// The complete frame failed its canonical protocol decoder.
    Malformed(ScbErrorCode),
}

/// A decoded request bound to the kernel-reported, approved peer UID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedRunRequest {
    caller_uid: u32,
    request: RunRequest,
}

impl AuthenticatedRunRequest {
    /// UID observed by the supervisor socket, never supplied in the frame.
    #[must_use]
    pub const fn caller_uid(&self) -> u32 {
        self.caller_uid
    }

    /// Exact canonically decoded and scope-matched request.
    #[must_use]
    pub const fn request(&self) -> &RunRequest {
        &self.request
    }
}

fn read_before(
    stream: &mut UnixStream,
    mut bytes: &mut [u8],
    deadline: Instant,
) -> Result<(), IngressError> {
    while !bytes.is_empty() {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or(IngressError::DeadlineReached)?;
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|_| IngressError::ReadFailure)?;
        match stream.read(bytes) {
            Ok(0) => return Err(IngressError::ReadFailure),
            Ok(count) => bytes = &mut bytes[count..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                return Err(IngressError::DeadlineReached);
            }
            Err(_) => return Err(IngressError::ReadFailure),
        }
    }
    Ok(())
}

/// Authenticates one Unix-socket peer and reads one bounded native run frame.
///
/// Credential and UID authorization precede all input reads. The fixed
/// deadline covers both header and body, so a slow peer cannot extend the
/// total budget by sending one byte at a time. The caller owns the socket
/// after this function and must close it after answering this one request.
///
/// # Errors
///
/// Refuses invalid configuration or deadlines, missing/unauthorized peer
/// credentials, oversized or malformed frames, and scope mismatches.
pub fn authenticate_request(
    stream: &mut UnixStream,
    config: &RunnerConfig,
    timeout: Duration,
) -> Result<AuthenticatedRunRequest, IngressError> {
    config
        .validate()
        .map_err(|_| IngressError::InvalidConfiguration)?;
    if timeout.is_zero() || timeout > MAX_INGRESS_TIMEOUT {
        return Err(IngressError::InvalidDeadline);
    }
    let uid = getsockopt(stream, PeerCredentials)
        .map_err(|_| IngressError::CredentialsUnavailable)?
        .uid();
    let caller = config
        .caller_for_uid(uid)
        .ok_or(IngressError::UnauthorizedPeer)?;
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(IngressError::InvalidDeadline)?;
    let mut header = [0_u8; 12];
    read_before(stream, &mut header, deadline)?;
    if header[..8] != *RUN_MAGIC {
        return Err(IngressError::Malformed(ScbErrorCode::MagicInvalid));
    }
    let body_len = usize::try_from(u32::from_be_bytes([
        header[8], header[9], header[10], header[11],
    ]))
    .map_err(|_| IngressError::FrameTooLarge)?;
    if body_len > MAX_REQUEST_BYTES - header.len() {
        return Err(IngressError::FrameTooLarge);
    }
    let mut frame = vec![0_u8; header.len() + body_len];
    frame[..header.len()].copy_from_slice(&header);
    read_before(stream, &mut frame[header.len()..], deadline)?;
    let request =
        RunRequest::decode_frame(&frame).map_err(|error| IngressError::Malformed(error.code()))?;
    if request.workspace != caller.workspace || request.principal != caller.principal {
        return Err(IngressError::ScopeMismatch);
    }
    Ok(AuthenticatedRunRequest {
        caller_uid: uid,
        request,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use sley_id::{CandidateId, EntityId, ObjectId, PolicyRootId, PrincipalId, WorkspaceId};
    use sley_vm::native_execution::NativeDeclaredLimits;

    use super::*;
    use crate::config::{AllowedCaller, default_config};
    use crate::worker::WorkerRequest;

    fn request() -> RunRequest {
        let declared_limits = NativeDeclaredLimits {
            fuel: 100,
            memory_bytes: 4_096,
            output_bytes: 64,
            effect_count: 0,
            call_depth: 8,
            wall_timeout_millis: 1_000,
        };
        RunRequest {
            workspace: WorkspaceId::from_bytes([1; 32]),
            principal: PrincipalId::from_bytes([2; 32]),
            candidate_id: Some(CandidateId::from_bytes([3; 32])),
            plan_id: sley_id::NativeTestPlanId::from_bytes([4; 32]),
            test_object: ObjectId::from_bytes([5; 32]),
            test_entity: EntityId::from_bytes([6; 32]),
            target_function: EntityId::from_bytes([7; 32]),
            policy_root: PolicyRootId::from_bytes([8; 32]),
            declared_limits,
            wall_ms: 1_000,
            nonce: [9; 32],
            worker_frame: WorkerRequest {
                program_bytes: vec![1, 2, 3],
                input_hashes: vec![[10; 32]],
                declared_limits,
                implementation_limits:
                    sley_vm::native_execution::NativeImplementationLimits::HARD_MAXIMA,
            }
            .encode_frame()
            .expect("worker frame"),
        }
    }

    fn config(uid: u32) -> RunnerConfig {
        default_config(
            "/run/sley-test-supervisor",
            "/usr/lib/sley/sley-native-test-worker",
            [7; 32],
            [8; 32],
            vec![AllowedCaller {
                uid,
                workspace: WorkspaceId::from_bytes([1; 32]),
                principal: PrincipalId::from_bytes([2; 32]),
            }],
            "/etc/sley-test-supervisor/measurement.key",
            "/etc/sley-test-supervisor/trust",
        )
        .expect("config")
    }

    fn pair() -> (UnixStream, UnixStream, u32) {
        let (server, client) = UnixStream::pair().expect("socket pair");
        let uid = getsockopt(&server, PeerCredentials)
            .expect("peer credentials")
            .uid();
        (server, client, uid)
    }

    #[test]
    fn real_socket_credentials_bind_a_complete_request() {
        let (mut server, mut client, uid) = pair();
        let expected = request();
        client
            .write_all(&expected.encode_frame().expect("frame"))
            .expect("write");
        let bound = authenticate_request(&mut server, &config(uid), Duration::from_secs(1))
            .expect("authenticated request");
        assert_eq!(bound.caller_uid(), uid);
        assert_eq!(bound.request(), &expected);
    }

    #[test]
    fn explicit_root_request_authenticates_without_candidate_identity() {
        let (mut server, mut client, uid) = pair();
        let expected = RunRequest {
            candidate_id: None,
            ..request()
        };
        client
            .write_all(&expected.encode_frame().expect("frame"))
            .expect("write");
        let bound = authenticate_request(&mut server, &config(uid), Duration::from_secs(1))
            .expect("authenticated explicit-root request");
        assert_eq!(bound.caller_uid(), uid);
        assert_eq!(bound.request(), &expected);
    }

    #[test]
    fn unauthorized_peer_refuses_without_reading() {
        let (mut server, _client, uid) = pair();
        assert_eq!(
            authenticate_request(
                &mut server,
                &config(uid.wrapping_add(1)),
                Duration::from_secs(1)
            ),
            Err(IngressError::UnauthorizedPeer)
        );
    }

    #[test]
    fn wrong_workspace_or_principal_refuses_after_canonical_decode() {
        for change_workspace in [true, false] {
            let (mut server, mut client, uid) = pair();
            let mut changed = request();
            if change_workspace {
                changed.workspace = WorkspaceId::from_bytes([10; 32]);
            } else {
                changed.principal = PrincipalId::from_bytes([11; 32]);
            }
            client
                .write_all(&changed.encode_frame().expect("frame"))
                .expect("write");
            assert_eq!(
                authenticate_request(&mut server, &config(uid), Duration::from_secs(1)),
                Err(IngressError::ScopeMismatch)
            );
        }
    }

    #[test]
    fn oversized_header_refuses_without_waiting_for_a_body() {
        let (mut server, mut client, uid) = pair();
        let mut header = RUN_MAGIC.to_vec();
        header.extend_from_slice(
            &u32::try_from(MAX_REQUEST_BYTES)
                .expect("request bound fits u32")
                .to_be_bytes(),
        );
        client.write_all(&header).expect("write header");
        assert_eq!(
            authenticate_request(&mut server, &config(uid), Duration::from_secs(1)),
            Err(IngressError::FrameTooLarge)
        );
    }

    #[test]
    fn truncated_frame_and_invalid_deadline_refuse() {
        let (mut server, mut client, uid) = pair();
        let frame = request().encode_frame().expect("frame");
        client
            .write_all(&frame[..frame.len() - 1])
            .expect("write partial frame");
        client
            .shutdown(std::net::Shutdown::Write)
            .expect("close write side");
        assert_eq!(
            authenticate_request(&mut server, &config(uid), Duration::from_secs(1)),
            Err(IngressError::ReadFailure)
        );
        assert_eq!(
            authenticate_request(&mut server, &config(uid), Duration::ZERO),
            Err(IngressError::InvalidDeadline)
        );
    }
}
