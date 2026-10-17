//! Bounded report channel for the gated native-test worker.
//!
//! The worker writes a raw canonical `SLEYNEX1` report, flushes it, and
//! waits for the daemon's release byte. Waiting for stdout EOF before reading
//! telemetry would deadlock. This reader obtains the exact envelope length,
//! parses one complete report, and leaves the pipe open until the daemon has
//! sampled the live cgroup. A second read then requires EOF and no trailers.

use std::io::{self, Read};
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::time::Instant;

use nix::errno::Errno;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use sley_tests::NativeExecutionReportV1;

use crate::config::MAX_WORKER_OUTPUT_BYTES;

const REPORT_MAGIC: &[u8; 8] = b"SLEYNEX1";
const REPORT_TRAILER_BYTES: usize = 32;

/// Refusal to trust a worker's stdout or the requesting peer's lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelError {
    /// Daemon's strict monotonic deadline elapsed.
    Deadline,
    /// Caller closed the socket while the worker was active.
    PeerLost,
    /// Output pipe closed before a complete report or failed unexpectedly.
    OutputLost,
    /// Worker supplied bytes other than one canonical `SLEYNEX1` report.
    MalformedReport,
    /// Declared or actual report length exceeded the closed output limit.
    OversizedReport,
    /// Worker wrote bytes after its complete report.
    TrailingOutput,
}

impl core::fmt::Display for ChannelError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Deadline => "NATIVE_CHANNEL_DEADLINE",
            Self::PeerLost => "NATIVE_CHANNEL_PEER_LOST",
            Self::OutputLost => "NATIVE_CHANNEL_OUTPUT_LOST",
            Self::MalformedReport => "NATIVE_CHANNEL_REPORT_MALFORMED",
            Self::OversizedReport => "NATIVE_CHANNEL_REPORT_OVERSIZED",
            Self::TrailingOutput => "NATIVE_CHANNEL_TRAILING_OUTPUT",
        })
    }
}

impl std::error::Error for ChannelError {}

fn wait_readable<Output: AsFd>(
    output: &Output,
    peer: &UnixStream,
    deadline: Instant,
) -> Result<(), ChannelError> {
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or(ChannelError::Deadline)?;
        let milliseconds = remaining.as_millis().clamp(1, 1_000);
        let timeout = PollTimeout::try_from(milliseconds).map_err(|_| ChannelError::Deadline)?;
        let (output_events, peer_events) = {
            let mut fds = [
                PollFd::new(
                    output.as_fd(),
                    PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR,
                ),
                PollFd::new(peer.as_fd(), PollFlags::POLLHUP | PollFlags::POLLERR),
            ];
            match poll(&mut fds, timeout) {
                Ok(0) | Err(Errno::EINTR) => continue,
                Err(_) => return Err(ChannelError::OutputLost),
                Ok(_) => (fds[0].revents(), fds[1].revents()),
            }
        };
        if peer_events.is_some_and(|events| {
            events.intersects(PollFlags::POLLHUP | PollFlags::POLLERR | PollFlags::POLLNVAL)
        }) {
            return Err(ChannelError::PeerLost);
        }
        if output_events.is_some_and(|events| {
            events.intersects(PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR)
        }) {
            return Ok(());
        }
        if output_events.is_some_and(|events| events.contains(PollFlags::POLLNVAL)) {
            return Err(ChannelError::OutputLost);
        }
    }
}

/// Checks that the request socket has not been fully closed before release.
///
/// A client's write half may be closed after its request; that does not mean
/// the response socket or client lifetime has been lost.
///
/// # Errors
///
/// Refuses a disconnected peer or an unavailable socket poll.
pub fn check_peer_connected(peer: &UnixStream) -> Result<(), ChannelError> {
    loop {
        let mut fds = [PollFd::new(
            peer.as_fd(),
            PollFlags::POLLHUP | PollFlags::POLLERR,
        )];
        match poll(&mut fds, PollTimeout::ZERO) {
            Ok(_) => {}
            Err(Errno::EINTR) => continue,
            Err(_) => return Err(ChannelError::PeerLost),
        }
        if fds[0].revents().is_some_and(|events| {
            events.intersects(PollFlags::POLLHUP | PollFlags::POLLERR | PollFlags::POLLNVAL)
        }) {
            return Err(ChannelError::PeerLost);
        }
        return Ok(());
    }
}

fn envelope_total_len(bytes: &[u8]) -> Result<Option<usize>, ChannelError> {
    if bytes.len() < REPORT_MAGIC.len() {
        if !REPORT_MAGIC.starts_with(bytes) {
            return Err(ChannelError::MalformedReport);
        }
        return Ok(None);
    }
    if &bytes[..REPORT_MAGIC.len()] != REPORT_MAGIC {
        return Err(ChannelError::MalformedReport);
    }
    let Some(version) = bytes.get(REPORT_MAGIC.len()) else {
        return Ok(None);
    };
    if *version != 1 {
        return Err(ChannelError::MalformedReport);
    }
    let mut record_len = 0_usize;
    for index in 0..4 {
        let Some(byte) = bytes.get(REPORT_MAGIC.len() + 1 + index).copied() else {
            return Ok(None);
        };
        let digit = usize::from(byte & 0x7f);
        record_len |= digit << (index * 7);
        if byte & 0x80 == 0 {
            if (index > 0 && digit == 0) || record_len == 0 {
                return Err(ChannelError::MalformedReport);
            }
            let total = REPORT_MAGIC.len() + 1 + index + 1 + record_len + REPORT_TRAILER_BYTES;
            if total > MAX_WORKER_OUTPUT_BYTES {
                return Err(ChannelError::OversizedReport);
            }
            return Ok(Some(total));
        }
    }
    Err(ChannelError::OversizedReport)
}

/// Reads one exact canonical execution report before worker exit.
///
/// A half-closed request socket remains valid: clients close only their write
/// side after sending the request. Full socket loss or a deadline refuses.
/// The caller must capture live cgroup telemetry before releasing the worker.
///
/// # Errors
///
/// Refuses lost peers, deadlines, missing output, bad envelopes, and overflow.
pub fn read_report_before<Output: AsFd + Read>(
    output: &mut Output,
    peer: &UnixStream,
    deadline: Instant,
) -> Result<NativeExecutionReportV1, ChannelError> {
    let mut bytes = Vec::new();
    let mut scratch = [0_u8; 4_096];
    loop {
        wait_readable(output, peer, deadline)?;
        let count = match output.read(&mut scratch) {
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(ChannelError::OutputLost),
        };
        if count == 0 {
            return Err(ChannelError::OutputLost);
        }
        if bytes.len() + count > MAX_WORKER_OUTPUT_BYTES {
            return Err(ChannelError::OversizedReport);
        }
        bytes.extend_from_slice(&scratch[..count]);
        if let Some(total) = envelope_total_len(&bytes)? {
            if bytes.len() > total {
                return Err(ChannelError::TrailingOutput);
            }
            if bytes.len() == total {
                return NativeExecutionReportV1::parse(&bytes)
                    .map_err(|_| ChannelError::MalformedReport);
            }
        }
    }
}

/// Requires EOF after the daemon releases the worker, with no extra bytes.
///
/// # Errors
///
/// Refuses a lost peer, deadline, stream failure, or trailing worker output.
pub fn require_output_eof_before<Output: AsFd + Read>(
    output: &mut Output,
    peer: &UnixStream,
    deadline: Instant,
) -> Result<(), ChannelError> {
    let mut byte = [0_u8; 1];
    loop {
        wait_readable(output, peer, deadline)?;
        match output.read(&mut byte) {
            Ok(0) => return Ok(()),
            Ok(_) => return Err(ChannelError::TrailingOutput),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return Err(ChannelError::OutputLost),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::net::Shutdown;
    use std::time::Duration;

    use super::*;

    const REPORT: &[u8] =
        include_bytes!("../../../conformance/native-worker/v1/observed-report.bin");

    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(1)
    }

    #[test]
    fn reads_a_complete_report_while_writer_remains_alive() {
        let (mut output, mut writer) = UnixStream::pair().unwrap();
        let (peer, client) = UnixStream::pair().unwrap();
        client.shutdown(Shutdown::Write).unwrap();
        writer.write_all(&REPORT[..11]).unwrap();
        writer.write_all(&REPORT[11..]).unwrap();
        let report = read_report_before(&mut output, &peer, deadline()).unwrap();
        assert_eq!(report.stored_bytes(), REPORT);
        drop(writer);
        assert_eq!(
            require_output_eof_before(&mut output, &peer, deadline()),
            Ok(())
        );
    }

    #[test]
    fn rejects_trailing_output_and_oversized_declaration() {
        let (mut output, mut writer) = UnixStream::pair().unwrap();
        let (peer, _client) = UnixStream::pair().unwrap();
        writer.write_all(REPORT).unwrap();
        read_report_before(&mut output, &peer, deadline()).unwrap();
        writer.write_all(b"x").unwrap();
        assert_eq!(
            require_output_eof_before(&mut output, &peer, deadline()),
            Err(ChannelError::TrailingOutput)
        );
        let (mut output, mut writer) = UnixStream::pair().unwrap();
        writer.write_all(b"SLEYNEX1\x01\x80\x80\x10").unwrap();
        assert_eq!(
            read_report_before(&mut output, &peer, deadline()),
            Err(ChannelError::OversizedReport)
        );
    }

    #[test]
    fn refuses_peer_loss_and_deadline_without_waiting_for_eof() {
        let (mut output, _writer) = UnixStream::pair().unwrap();
        let (peer, client) = UnixStream::pair().unwrap();
        drop(client);
        assert_eq!(check_peer_connected(&peer), Err(ChannelError::PeerLost));
        assert_eq!(
            read_report_before(&mut output, &peer, deadline()),
            Err(ChannelError::PeerLost)
        );
        let (_peer, client) = UnixStream::pair().unwrap();
        assert_eq!(
            read_report_before(&mut output, &client, Instant::now()),
            Err(ChannelError::Deadline)
        );
    }
}
