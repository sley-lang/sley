//! Protected local listener for the root native test supervisor.
//!
//! The service manager creates the runtime directory. The daemon verifies
//! its ownership and permissions through a symlink-free open, then binds the
//! single configured socket name. Socket access is open to local UIDs because
//! the ingress authenticates kernel peer credentials before reading bytes.
//! Only a socket inode created by this instance is removed on orderly exit.

use std::fs::{self, File};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use nix::fcntl::{OFlag, OpenHow, ResolveFlag, openat2};

use crate::config::{RunnerConfig, valid_admin_path};

/// Failure before the daemon exposes a local listener.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SocketBindError {
    /// Only the root daemon may bind the configured endpoint.
    Unprivileged,
    /// The configured path or runtime directory is unsafe.
    UnsafeDirectory,
    /// A socket or other file already occupies the exact path.
    PathOccupied,
    /// The bind, permission setup, or socket metadata check failed.
    BindFailure,
}

impl core::fmt::Display for SocketBindError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Unprivileged => "NATIVE_SOCKET_BIND_UNPRIVILEGED",
            Self::UnsafeDirectory => "NATIVE_SOCKET_BIND_UNSAFE_DIRECTORY",
            Self::PathOccupied => "NATIVE_SOCKET_BIND_PATH_OCCUPIED",
            Self::BindFailure => "NATIVE_SOCKET_BIND_FAILED",
        })
    }
}

impl std::error::Error for SocketBindError {}

/// One socket inode owned by this supervisor instance.
pub struct BoundSupervisorSocket {
    listener: UnixListener,
    path: PathBuf,
    device: u64,
    inode: u64,
}

impl BoundSupervisorSocket {
    /// The already bound local listener.
    #[must_use]
    pub const fn listener(&self) -> &UnixListener {
        &self.listener
    }
}

impl Drop for BoundSupervisorSocket {
    fn drop(&mut self) {
        if let Ok(metadata) = fs::symlink_metadata(&self.path)
            && metadata.file_type().is_socket()
            && metadata.dev() == self.device
            && metadata.ino() == self.inode
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn bind_for_owner(
    config: &RunnerConfig,
    expected_uid: u32,
) -> Result<BoundSupervisorSocket, SocketBindError> {
    config
        .validate()
        .map_err(|_| SocketBindError::UnsafeDirectory)?;
    if !valid_admin_path(&config.runtime_dir) {
        return Err(SocketBindError::UnsafeDirectory);
    }
    let runtime = Path::new(&config.runtime_dir);
    let relative = runtime
        .strip_prefix("/")
        .map_err(|_| SocketBindError::UnsafeDirectory)?;
    let root = File::open("/").map_err(|_| SocketBindError::UnsafeDirectory)?;
    let directory = File::from(
        openat2(
            &root,
            relative,
            OpenHow::new()
                .flags(OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC)
                .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS),
        )
        .map_err(|_| SocketBindError::UnsafeDirectory)?,
    );
    let metadata = directory
        .metadata()
        .map_err(|_| SocketBindError::UnsafeDirectory)?;
    if !metadata.is_dir()
        || metadata.uid() != expected_uid
        || metadata.permissions().mode() & 0o022 != 0
    {
        return Err(SocketBindError::UnsafeDirectory);
    }
    let path = PathBuf::from(config.socket_path());
    if fs::symlink_metadata(&path).is_ok() {
        return Err(SocketBindError::PathOccupied);
    }
    let listener = UnixListener::bind(&path).map_err(|_| SocketBindError::BindFailure)?;
    let setup = || {
        fs::set_permissions(&path, fs::Permissions::from_mode(0o666))
            .map_err(|_| SocketBindError::BindFailure)?;
        let socket = fs::symlink_metadata(&path).map_err(|_| SocketBindError::BindFailure)?;
        if !socket.file_type().is_socket()
            || socket.uid() != expected_uid
            || socket.permissions().mode() & 0o7777 != 0o666
        {
            return Err(SocketBindError::BindFailure);
        }
        Ok(socket)
    };
    let socket = match setup() {
        Ok(socket) => socket,
        Err(error) => {
            let _ = fs::remove_file(&path);
            return Err(error);
        }
    };
    Ok(BoundSupervisorSocket {
        listener,
        path,
        device: socket.dev(),
        inode: socket.ino(),
    })
}

/// Binds the root service's one configured Unix socket.
///
/// The runtime directory must already exist with root ownership and no
/// group/world write. An occupied path is never removed or replaced.
///
/// # Errors
///
/// Refuses nonroot use, unsafe storage, occupied path, or failed binding.
pub fn bind_supervisor_socket(
    config: &RunnerConfig,
) -> Result<BoundSupervisorSocket, SocketBindError> {
    if !nix::unistd::geteuid().is_root() {
        return Err(SocketBindError::Unprivileged);
    }
    bind_for_owner(config, 0)
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixStream;
    use std::sync::atomic::{AtomicU64, Ordering};

    use sley_id::{PrincipalId, WorkspaceId};

    use super::*;
    use crate::config::{AllowedCaller, default_config};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn bound_socket_accepts_and_removes_only_its_own_inode() {
        let runtime = std::env::temp_dir().join(format!(
            "sley-supervisor-socket-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&runtime).expect("create runtime");
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o755)).expect("safe runtime");
        let config = default_config(
            runtime.to_str().expect("path"),
            "/usr/lib/sley/sley-native-test-worker",
            [7; 32],
            [8; 32],
            vec![AllowedCaller {
                uid: 1_000,
                workspace: WorkspaceId::from_bytes([1; 32]),
                principal: PrincipalId::from_bytes([2; 32]),
            }],
            "/etc/sley-test-supervisor/measurement.key",
            "/etc/sley-test-supervisor/trust",
        )
        .expect("config");
        let uid = nix::unistd::getuid().as_raw();
        let bound = bind_for_owner(&config, uid).expect("bind");
        let connection = UnixStream::connect(config.socket_path()).expect("connect");
        let (_, _) = bound.listener().accept().expect("accept");
        drop(connection);
        assert_eq!(
            bind_for_owner(&config, uid).err(),
            Some(SocketBindError::PathOccupied)
        );
        drop(bound);
        assert!(!Path::new(&config.socket_path()).exists());
        fs::remove_dir_all(runtime).expect("cleanup fixture");
    }
}
