//! One-shot socket service boundaries for the native test supervisor.
//!
//! Both paths authenticate kernel peer credentials, scope, framing, and the
//! ingress deadline before responding. The legacy one-shot handler explicitly
//! refuses execution. The root handler requires startup-provisioned authority,
//! stages one checked worker input, owns the system unit through exit/reap,
//! and signs complete measurements or an authenticated prelaunch staging
//! refusal. Postlaunch failures yield no result; unconfirmed cleanup has a
//! distinct error for daemon degradation.
//! The root daemon binds the production listener and calls this boundary.
//! Installation and privileged qualification remain separate work.

use std::io::{self, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sley_tests::ROLE_MEASUREMENT;

use crate::attest::{
    AttestationError, sign_complete_attempt, sign_measured_timeout, sign_prelaunch_refusal,
};
use crate::config::RunnerConfig;
use crate::enforce::EnforceError;
use crate::ingress::{IngressError, authenticate_request};
use crate::owner::{OwnerError, run_owned_system_unit};
use crate::protocol::{RunResponse, RunStatus};
use crate::stage::{StageError, stage_worker_input};
use crate::trust_store::ProvisionedMeasurementAuthority;
use crate::unit::expected_supervisor_config;

/// Stable refusal code while the selected-program handoff is absent.
pub const RUN_REFUSAL_EXECUTION_NOT_WIRED: u32 = 1;
/// Stable refusal for an authenticated request with invalid program bindings.
pub const RUN_REFUSAL_PROGRAM_INVALID: u32 = 2;
/// Stable prelaunch refusal when the signer lacks this run's exact scope.
pub const RUN_REFUSAL_TRUST_NOT_GRANTED: u32 = 3;
/// Stable signed refusal when worker input cannot be staged before launch.
pub const RUN_REFUSAL_STAGE_FAILED: u32 = 4;
/// Stable signed refusal for a wall budget below this systemd profile's
/// currently supported minimum.
pub const RUN_REFUSAL_WALL_UNSUPPORTED: u32 = 5;
/// This initial systemd profile has only been observed to complete native
/// tests at a one-second wall budget; shorter requests refuse before launch.
pub const MIN_SUPPORTED_WALL_MILLIS: u64 = 1_000;
/// Stable failed-run code for a sampled wall deadline with confirmed teardown.
pub const RUN_FAILURE_WALL_TIMEOUT: u32 = 1;
/// Hard deadline for writing one complete bounded supervisor response.
pub const RESPONSE_WRITE_TIMEOUT: Duration = Duration::from_secs(2);

/// A socket service failure; none confers a test result or attestation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceError {
    /// A peer failed authentication, scope, framing, or the read deadline.
    Ingress(IngressError),
    /// The listener could not accept a connection.
    AcceptFailure,
    /// The checked worker frame could not be staged or removed.
    Stage(StageError),
    /// The exact transient unit could not be rendered.
    Unit(EnforceError),
    /// The staged path could not be represented as a launch argument.
    InputPath,
    /// Host time could not be used for trust-interval preflight.
    ClockUnavailable,
    /// The owned worker failed, with cleanup confirmed.
    Owner(OwnerError),
    /// A launched worker could not be proven reaped; the daemon must degrade.
    CleanupUnconfirmed,
    /// A completed worker attempt could not be safely signed.
    Attestation(AttestationError),
    /// The closed refusal frame could not be encoded.
    ResponseEncoding,
    /// The peer disconnected or its response could not be written.
    WriteFailure,
}

impl core::fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Ingress(_) => "NATIVE_SERVICE_INGRESS_REFUSED",
            Self::AcceptFailure => "NATIVE_SERVICE_ACCEPT_FAILED",
            Self::Stage(_) => "NATIVE_SERVICE_STAGE_FAILED",
            Self::Unit(_) => "NATIVE_SERVICE_UNIT_FAILED",
            Self::InputPath => "NATIVE_SERVICE_INPUT_PATH_INVALID",
            Self::ClockUnavailable => "NATIVE_SERVICE_CLOCK_UNAVAILABLE",
            Self::Owner(_) => "NATIVE_SERVICE_OWNER_FAILED",
            Self::CleanupUnconfirmed => "NATIVE_SERVICE_CLEANUP_UNCONFIRMED",
            Self::Attestation(_) => "NATIVE_SERVICE_ATTESTATION_FAILED",
            Self::ResponseEncoding => "NATIVE_SERVICE_RESPONSE_ENCODING_FAILED",
            Self::WriteFailure => "NATIVE_SERVICE_WRITE_FAILED",
        })
    }
}

impl std::error::Error for ServiceError {}

fn write_response(stream: &mut UnixStream, response: &RunResponse) -> Result<(), ServiceError> {
    let frame = response
        .encode_frame()
        .map_err(|_| ServiceError::ResponseEncoding)?;
    let deadline = Instant::now()
        .checked_add(RESPONSE_WRITE_TIMEOUT)
        .ok_or(ServiceError::WriteFailure)?;
    let mut remaining = frame.as_slice();
    while !remaining.is_empty() {
        let budget = deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or(ServiceError::WriteFailure)?;
        stream
            .set_write_timeout(Some(budget))
            .map_err(|_| ServiceError::WriteFailure)?;
        match stream.write(remaining) {
            Ok(0) => return Err(ServiceError::WriteFailure),
            Ok(count) => remaining = &remaining[count..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return Err(ServiceError::WriteFailure),
        }
    }
    Ok(())
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
        no_result: None,
    };
    write_response(stream, &response)
}

/// Handles one socket run using authority loaded before accepting work.
///
/// Invalid selected programs receive a closed prelaunch refusal. Unsupported
/// short wall budgets and staging failures under provisioned trust receive a
/// signed no-result refusal. A sampled report deadline with confirmed kill
/// and reap receives a signed no-result failure. Other postlaunch failures
/// close the connection without a signed result. `CleanupUnconfirmed` requires
/// the outer daemon to stop accepting work until orphan reconciliation succeeds.
///
/// # Errors
///
/// Returns the first ingress, stage, launch, cleanup, signing, or transport
/// failure. No error grants a native test result.
pub fn handle_root_connection(
    stream: &mut UnixStream,
    authority: &ProvisionedMeasurementAuthority,
    timeout: Duration,
) -> Result<(), ServiceError> {
    let config = authority.config();
    let authenticated =
        authenticate_request(stream, config, timeout).map_err(ServiceError::Ingress)?;
    if authenticated.request().verified_program().is_err() {
        return write_response(
            stream,
            &RunResponse {
                status: RunStatus::Refused,
                code: RUN_REFUSAL_PROGRAM_INVALID,
                evidence: None,
                no_result: None,
            },
        );
    }
    let expected =
        expected_supervisor_config(config, authenticated.request(), authenticated.caller_uid())
            .map_err(|error| ServiceError::Attestation(AttestationError::Binding(error.code())))?;
    let now_millis = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ServiceError::ClockUnavailable)?
            .as_millis(),
    )
    .map_err(|_| ServiceError::ClockUnavailable)?;
    if !authority.trust().grants(
        &authority.signer().public_key(),
        ROLE_MEASUREMENT,
        authenticated.request().workspace.as_bytes(),
        expected.id().as_bytes(),
        now_millis,
    ) {
        return write_response(
            stream,
            &RunResponse {
                status: RunStatus::Refused,
                code: RUN_REFUSAL_TRUST_NOT_GRANTED,
                evidence: None,
                no_result: None,
            },
        );
    }
    if authenticated.request().wall_ms < MIN_SUPPORTED_WALL_MILLIS {
        let response = sign_prelaunch_refusal(
            config,
            &authenticated,
            authority.trust(),
            authority.signer(),
            RUN_REFUSAL_WALL_UNSUPPORTED,
        )
        .map_err(ServiceError::Attestation)?;
        return write_response(stream, &response);
    }
    let Ok(staged) = stage_worker_input(config, authenticated.request()) else {
        let response = sign_prelaunch_refusal(
            config,
            &authenticated,
            authority.trust(),
            authority.signer(),
            RUN_REFUSAL_STAGE_FAILED,
        )
        .map_err(ServiceError::Attestation)?;
        return write_response(stream, &response);
    };
    let unit = staged.render_unit().map_err(ServiceError::Unit)?;
    let input_path = staged.path().to_str().ok_or(ServiceError::InputPath)?;
    let result = run_owned_system_unit(
        &unit,
        config,
        input_path,
        stream,
        authenticated.request().wall_ms,
    );
    let removal = staged.remove();
    if matches!(&result, Err(OwnerError::CleanupUnconfirmed)) {
        return Err(ServiceError::CleanupUnconfirmed);
    }
    removal.map_err(ServiceError::Stage)?;
    let response = match result {
        Ok(result) => sign_complete_attempt(
            config,
            &authenticated,
            result,
            authority.trust(),
            authority.signer(),
        )
        .map_err(ServiceError::Attestation)?,
        Err(OwnerError::MeasuredTimeout(facts)) => sign_measured_timeout(
            config,
            &authenticated,
            facts,
            authority.trust(),
            authority.signer(),
            RUN_FAILURE_WALL_TIMEOUT,
        )
        .map_err(ServiceError::Attestation)?,
        Err(error) => return Err(ServiceError::Owner(error)),
    };
    write_response(stream, &response)
}

/// Accepts one connection for the provisioned root service.
///
/// # Errors
///
/// Returns the first accept or handled-connection failure.
pub fn serve_one_root(
    listener: &UnixListener,
    authority: &ProvisionedMeasurementAuthority,
    timeout: Duration,
) -> Result<(), ServiceError> {
    let (mut stream, _) = listener.accept().map_err(|_| ServiceError::AcceptFailure)?;
    handle_root_connection(&mut stream, authority, timeout)
}

/// Accepts and handles exactly one connection from a trusted listener.
///
/// This legacy refusal path does not bind a path, install a service, or
/// claim native execution readiness.
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
    use crate::outcome::{Ed25519MeasurementSigner, Signer};
    use crate::program::PortableTestProgram;
    use crate::protocol::RunRequest;
    use crate::worker::WorkerRequest;
    use sley_tests::{
        HistoricalTrustPolicyParts, HistoricalTrustPolicyV1, ROLE_MEASUREMENT, TrustEntry,
    };

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

    fn test_authority(uid: u32) -> ProvisionedMeasurementAuthority {
        let config = config(uid);
        let signer = Ed25519MeasurementSigner::from_secret_bytes([3; 32]);
        let trust = HistoricalTrustPolicyV1::build(HistoricalTrustPolicyParts {
            policy_nonce: [5; 32],
            entries: vec![TrustEntry {
                key_id: signer.public_key(),
                role: ROLE_MEASUREMENT,
                workspaces: vec![*config.allowed_callers[0].workspace.as_bytes()],
                profiles: vec![[4; 32]],
                valid_from_unix_millis: 0,
                valid_until_unix_millis: u64::MAX,
            }],
        })
        .expect("test trust");
        ProvisionedMeasurementAuthority::for_test(config, trust, signer)
    }

    fn granted_authority(
        config: RunnerConfig,
        request: &RunRequest,
        uid: u32,
    ) -> ProvisionedMeasurementAuthority {
        let profile = *expected_supervisor_config(&config, request, uid)
            .expect("exact profile")
            .id()
            .as_bytes();
        let signer = Ed25519MeasurementSigner::from_secret_bytes([3; 32]);
        let trust = HistoricalTrustPolicyV1::build(HistoricalTrustPolicyParts {
            policy_nonce: [6; 32],
            entries: vec![TrustEntry {
                key_id: signer.public_key(),
                role: ROLE_MEASUREMENT,
                workspaces: vec![*request.workspace.as_bytes()],
                profiles: vec![profile],
                valid_from_unix_millis: 0,
                valid_until_unix_millis: u64::MAX,
            }],
        })
        .expect("exact trust");
        ProvisionedMeasurementAuthority::for_test(config, trust, signer)
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
                no_result: None,
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
                no_result: None,
            }
        );
        std::fs::remove_file(path).expect("remove socket");
    }

    #[test]
    fn root_handler_refuses_invalid_program_before_staging_or_launch() {
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
            serve_one_root(&listener, &test_authority(uid), Duration::from_secs(1)),
            Ok(())
        );
        assert_eq!(
            RunResponse::decode_frame(&client.join().expect("client thread"))
                .expect("closed response"),
            RunResponse {
                status: RunStatus::Refused,
                code: RUN_REFUSAL_PROGRAM_INVALID,
                evidence: None,
                no_result: None,
            }
        );
        std::fs::remove_file(path).expect("remove socket");
    }

    #[test]
    fn root_handler_refuses_ungranted_profile_before_staging() {
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
                stream.read_to_end(&mut bytes).expect("read EOF");
                bytes
            }
        });
        assert_eq!(
            serve_one_root(&listener, &test_authority(uid), Duration::from_secs(1)),
            Ok(())
        );
        assert_eq!(
            RunResponse::decode_frame(&client.join().expect("client thread"))
                .expect("closed response"),
            RunResponse {
                status: RunStatus::Refused,
                code: RUN_REFUSAL_TRUST_NOT_GRANTED,
                evidence: None,
                no_result: None,
            }
        );
        std::fs::remove_file(path).expect("remove socket");
    }

    #[test]
    fn root_handler_signs_a_staging_failure_before_launch() {
        let (listener, path) = test_listener();
        let uid = std::fs::symlink_metadata(&path)
            .expect("socket metadata")
            .uid();
        let request = request();
        let mut config = config(uid);
        config.runtime_dir = path
            .with_extension("missing-runtime")
            .to_string_lossy()
            .into_owned();
        let authority = granted_authority(config, &request, uid);
        let client = std::thread::spawn({
            let path = path.clone();
            let request = request.clone();
            move || {
                let mut stream = UnixStream::connect(path).expect("connect");
                stream
                    .write_all(&request.encode_frame().expect("frame"))
                    .expect("write request");
                let mut bytes = Vec::new();
                stream.read_to_end(&mut bytes).expect("read refusal");
                bytes
            }
        });
        assert_eq!(
            serve_one_root(&listener, &authority, Duration::from_secs(1)),
            Ok(())
        );
        let response = RunResponse::decode_frame(&client.join().expect("client thread"))
            .expect("signed refusal frame");
        assert_eq!(response.status, RunStatus::Refused);
        assert_eq!(response.code, RUN_REFUSAL_STAGE_FAILED);
        let evidence = request
            .verified_no_result_evidence(&response, uid)
            .expect("bound refusal");
        assert_eq!(evidence.attestation().execution_report_id(), None);
        assert_eq!(evidence.attestation().installed_memory_cap(), 0);
        std::fs::remove_file(path).expect("remove socket");
    }

    #[test]
    fn root_handler_signs_an_unsupported_short_wall_refusal_before_launch() {
        let (listener, path) = test_listener();
        let uid = std::fs::symlink_metadata(&path)
            .expect("socket metadata")
            .uid();
        let mut request = request();
        request.wall_ms = MIN_SUPPORTED_WALL_MILLIS - 1;
        let authority = granted_authority(config(uid), &request, uid);
        let client = std::thread::spawn({
            let path = path.clone();
            let request = request.clone();
            move || {
                let mut stream = UnixStream::connect(path).expect("connect");
                stream
                    .write_all(&request.encode_frame().expect("frame"))
                    .expect("write request");
                let mut bytes = Vec::new();
                stream.read_to_end(&mut bytes).expect("read refusal");
                bytes
            }
        });
        assert_eq!(
            serve_one_root(&listener, &authority, Duration::from_secs(1)),
            Ok(())
        );
        let response = RunResponse::decode_frame(&client.join().expect("client thread"))
            .expect("signed refusal frame");
        assert_eq!(response.status, RunStatus::Refused);
        assert_eq!(response.code, RUN_REFUSAL_WALL_UNSUPPORTED);
        let evidence = request
            .verified_no_result_evidence(&response, uid)
            .expect("bound prelaunch refusal");
        assert_eq!(evidence.attestation().execution_report_id(), None);
        assert_eq!(evidence.attestation().elapsed_ns(), 0);
        std::fs::remove_file(path).expect("remove socket");
    }
}
