//! Test-only exact path-bound recovery-read failure injection.

use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};

/// Closed artifact kind accepted by the recovery-read test seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryPathReadKind {
    /// A canonical transaction receipt read.
    Receipt,
    /// An immutable entity-object read.
    Object,
}

struct RecoveryPathReadPlan {
    kind: RecoveryPathReadKind,
    path: PathBuf,
    remaining_failures: u32,
}

std::thread_local! {
    static SELECTED_PATH_READ: RefCell<Option<RecoveryPathReadPlan>> =
        const { RefCell::new(None) };
}

/// Installs one exact path-bound failure plan on the current test thread.
///
/// The caller supplies the exact number of matching reads that must fail.
/// Installing over an undrained plan is rejected so tests cannot silently
/// inherit or replace fault state.
pub fn install(kind: RecoveryPathReadKind, path: &Path, failure_count: u32) {
    assert!(
        path.is_absolute(),
        "recovery path-read plan requires an absolute path"
    );
    assert!(
        failure_count > 0,
        "recovery path-read plan requires one or more failures"
    );
    SELECTED_PATH_READ.with(|selected| {
        let mut selected = selected.borrow_mut();
        assert!(
            selected.is_none(),
            "recovery path-read plan is already installed"
        );
        *selected = Some(RecoveryPathReadPlan {
            kind,
            path: path.to_path_buf(),
            remaining_failures: failure_count,
        });
    });
}

/// Fails one matching selected read with exact host `ErrorKind::Other`.
///
/// Nonmatching artifact kinds and paths pass through without consuming the
/// selected plan. The final matching failure clears the thread-local plan.
pub fn inject(kind: RecoveryPathReadKind, path: &Path) -> io::Result<()> {
    let should_fail = SELECTED_PATH_READ.with(|selected| {
        let mut selected = selected.borrow_mut();
        let Some(plan) = selected.as_mut() else {
            return false;
        };
        if plan.kind != kind || plan.path != path {
            return false;
        }
        plan.remaining_failures = plan.remaining_failures.checked_sub(1).unwrap();
        let exhausted = plan.remaining_failures == 0;
        if exhausted {
            *selected = None;
        }
        true
    });
    if should_fail {
        Err(io::Error::other(
            "s20-530 path-bound recovery read injection",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_path_and_kind_fail_twice_then_clear() {
        let selected = Path::new("/sley-test/selected");
        install(RecoveryPathReadKind::Receipt, selected, 2);
        assert!(inject(RecoveryPathReadKind::Object, selected).is_ok());
        assert!(
            inject(
                RecoveryPathReadKind::Receipt,
                Path::new("/sley-test/control")
            )
            .is_ok()
        );
        for _ in 0..2 {
            let error = inject(RecoveryPathReadKind::Receipt, selected).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::Other);
        }
        assert!(inject(RecoveryPathReadKind::Receipt, selected).is_ok());
    }
}
