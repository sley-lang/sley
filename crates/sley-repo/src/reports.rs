//! S20-560 execution report store: create-once, fsynced, identity-verified
//! records of S20-290 execution report preimages under
//! `reports/execution/<id hex>` (SMP1 appendix C, methods 600 and 604).

use core::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use sley_id::ExecutionReportId;

const REPORT_DIRECTORY: &str = "reports/execution";
/// Largest stored report preimage accepted by the store.
pub const MAX_STORED_REPORT_BYTES: usize = 16_777_216;

/// Stable failures of the report store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportStoreErrorCode {
    /// The identity names no stored report.
    Unknown,
    /// The stored bytes do not re-derive the identity or exceed the bound.
    Invalid,
    /// A host read, write, or sync failed.
    Io,
}

impl ReportStoreErrorCode {
    /// The frozen symbol.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "REPORT_STORE_UNKNOWN",
            Self::Invalid => "REPORT_STORE_INVALID",
            Self::Io => "REPORT_STORE_IO",
        }
    }

    /// The frozen numeric code.
    #[must_use]
    pub const fn numeric(self) -> u32 {
        match self {
            Self::Unknown => 56_000,
            Self::Invalid => 56_001,
            Self::Io => 56_002,
        }
    }
}

/// A report store failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportStoreError(ReportStoreErrorCode);

impl ReportStoreError {
    /// The code.
    #[must_use]
    pub const fn code(&self) -> ReportStoreErrorCode {
        self.0
    }
}

impl fmt::Display for ReportStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for ReportStoreError {}

fn hex(bytes: &[u8]) -> String {
    use core::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(64), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

fn report_path(repository: &Path, id: ExecutionReportId) -> PathBuf {
    repository.join(REPORT_DIRECTORY).join(hex(id.as_bytes()))
}

fn sync_directory(path: &Path) -> Result<(), ReportStoreError> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| ReportStoreError(ReportStoreErrorCode::Io))
}

/// Stores one execution report preimage under its identity, create-once.
///
/// Storing bytes that are already stored byte-identically succeeds without
/// writing; the identity must re-derive from the bytes.
///
/// # Errors
///
/// Returns `REPORT_STORE_INVALID` for a preimage that does not derive the
/// identity, is empty, or exceeds the bound, and `REPORT_STORE_IO` for a host
/// failure or a differing existing entry.
pub fn store_execution_report(
    repository: &Path,
    id: ExecutionReportId,
    preimage: &[u8],
) -> Result<(), ReportStoreError> {
    if preimage.is_empty()
        || preimage.len() > MAX_STORED_REPORT_BYTES
        || ExecutionReportId::derive(preimage) != id
    {
        return Err(ReportStoreError(ReportStoreErrorCode::Invalid));
    }
    let path = report_path(repository, id);
    if let Ok(existing) = fs::read(&path) {
        return if existing == preimage {
            Ok(())
        } else {
            Err(ReportStoreError(ReportStoreErrorCode::Io))
        };
    }
    let directory = repository.join(REPORT_DIRECTORY);
    fs::create_dir_all(&directory).map_err(|_| ReportStoreError(ReportStoreErrorCode::Io))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| ReportStoreError(ReportStoreErrorCode::Io))?;
    file.write_all(preimage)
        .and_then(|()| file.sync_all())
        .map_err(|_| ReportStoreError(ReportStoreErrorCode::Io))?;
    sync_directory(&directory)
}

/// Reads one stored execution report preimage, verifying its identity.
///
/// # Errors
///
/// Returns `REPORT_STORE_UNKNOWN` when nothing is stored under the identity,
/// `REPORT_STORE_INVALID` when the stored bytes do not derive it, and
/// `REPORT_STORE_IO` for a host failure.
pub fn read_execution_report(
    repository: &Path,
    id: ExecutionReportId,
) -> Result<Vec<u8>, ReportStoreError> {
    let path = report_path(repository, id);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ReportStoreError(ReportStoreErrorCode::Unknown));
        }
        Err(_) => return Err(ReportStoreError(ReportStoreErrorCode::Io)),
    };
    if bytes.is_empty()
        || bytes.len() > MAX_STORED_REPORT_BYTES
        || ExecutionReportId::derive(&bytes) != id
    {
        return Err(ReportStoreError(ReportStoreErrorCode::Invalid));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_are_stored_once_read_back_and_verified() {
        let temp = crate::test_support::TempDir::new("report-store");
        let repository = temp.child("repo");
        fs::create_dir(&repository).unwrap();
        let preimage = b"SLEYEXR1 preimage bytes".to_vec();
        let id = ExecutionReportId::derive(&preimage);
        store_execution_report(&repository, id, &preimage).unwrap();
        store_execution_report(&repository, id, &preimage).unwrap();
        assert_eq!(read_execution_report(&repository, id).unwrap(), preimage);
        let other = ExecutionReportId::derive(b"other");
        assert_eq!(
            read_execution_report(&repository, other)
                .unwrap_err()
                .code(),
            ReportStoreErrorCode::Unknown
        );
        assert_eq!(
            store_execution_report(&repository, other, &preimage)
                .unwrap_err()
                .code(),
            ReportStoreErrorCode::Invalid
        );
        fs::write(report_path(&repository, id), b"tampered").unwrap();
        assert_eq!(
            read_execution_report(&repository, id).unwrap_err().code(),
            ReportStoreErrorCode::Invalid
        );
        assert_eq!(
            store_execution_report(&repository, id, &preimage)
                .unwrap_err()
                .code(),
            ReportStoreErrorCode::Io
        );
    }
}
