//! Test-only deterministic recovery-ancestry cycle injection.

use std::borrow::Cow;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use sley_id::TransactionId;

use crate::{RepositoryMaintenanceGuard, TransactionRepository, VerifiedRevision};

const PLAN_DIGEST_DOMAIN: &[u8] = b"sley2.s20-530.recovery-ancestry-test-plan.v1";
const OPERATION_COUNT: usize = 2;

/// Closed count of synthetic cycle epochs supplied by one test plan.
pub enum RecoveryAncestryTestEpochs {
    /// Supply one synthetic cycle epoch.
    One,
    /// Supply two synthetic cycle epochs.
    Two,
}

impl RecoveryAncestryTestEpochs {
    fn count(&self) -> u64 {
        match self {
            Self::One => 1,
            Self::Two => 2,
        }
    }
}

/// Non-cloneable identity for one installed test plan.
pub struct RecoveryAncestryTestPlanIdentity {
    generation: u64,
    owner_root: PathBuf,
    digest: [u8; 32],
    armed: bool,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl RecoveryAncestryTestPlanIdentity {
    /// Returns the canonical repository root bound to this plan.
    #[must_use]
    pub fn owner_root(&self) -> &Path {
        &self.owner_root
    }

    /// Returns the deterministic plan digest.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

impl Drop for RecoveryAncestryTestPlanIdentity {
    fn drop(&mut self) {
        if self.armed {
            clear_matching_plan(self.generation, &self.owner_root, self.digest);
            self.armed = false;
        }
    }
}

/// RAII boundary for one owning recovery call or joined branch verifier.
pub struct RecoveryAncestryTestOperationGuard {
    generation: Option<u64>,
    owner_root: Option<PathBuf>,
    digest: Option<[u8; 32]>,
    ordinal: Option<usize>,
    role: GuardRole,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl RecoveryAncestryTestOperationGuard {
    fn inert() -> Self {
        Self {
            generation: None,
            owner_root: None,
            digest: None,
            ordinal: None,
            role: GuardRole::Inert,
            _not_send_or_sync: PhantomData,
        }
    }
}

impl Drop for RecoveryAncestryTestOperationGuard {
    fn drop(&mut self) {
        let (Some(generation), Some(owner_root), Some(digest), Some(ordinal)) = (
            self.generation,
            self.owner_root.as_ref(),
            self.digest,
            self.ordinal,
        ) else {
            return;
        };
        HOOK_STATE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let Some(plan) = slot.plan.as_mut() else {
                return;
            };
            if plan.generation != generation
                || plan.owner_root != *owner_root
                || plan.digest != digest
            {
                return;
            }
            if std::thread::panicking() {
                slot.plan = None;
                return;
            }
            match self.role {
                GuardRole::Inert => {}
                GuardRole::Join => {
                    let Some(active) = plan.active.as_mut() else {
                        slot.plan = None;
                        return;
                    };
                    if active.ordinal != ordinal || active.join_depth != 1 {
                        slot.plan = None;
                        return;
                    }
                    active.join_depth = 0;
                }
                GuardRole::Owner => {
                    let Some(active) = plan.active.take() else {
                        slot.plan = None;
                        return;
                    };
                    if active.ordinal != ordinal
                        || active.join_depth != 0
                        || ordinal >= OPERATION_COUNT
                        || plan.completed[ordinal].is_some()
                    {
                        slot.plan = None;
                        return;
                    }
                    plan.completed[ordinal] = Some(RecoveryAncestryTestOperationObservation {
                        claims: active.claims,
                        left_edges: active.left_edges,
                        right_edges: active.right_edges,
                    });
                }
            }
        });
    }
}

/// Finalized observation for one owning recovery operation.
pub struct RecoveryAncestryTestOperationObservation {
    claims: u64,
    left_edges: u64,
    right_edges: u64,
}

impl RecoveryAncestryTestOperationObservation {
    /// Returns the number of synthetic epochs claimed by this operation.
    #[must_use]
    pub const fn claims(&self) -> u64 {
        self.claims
    }

    /// Returns the observed `(left-to-right, right-to-left)` edge counts.
    #[must_use]
    pub const fn edge_counts(&self) -> (u64, u64) {
        (self.left_edges, self.right_edges)
    }
}

/// Consumed observations from one exact two-operation test plan.
pub struct RecoveryAncestryTestObservations {
    epoch_budget: u64,
    operation_1: RecoveryAncestryTestOperationObservation,
    operation_2: RecoveryAncestryTestOperationObservation,
    remaining_epochs: u64,
}

impl RecoveryAncestryTestObservations {
    /// Returns the immutable synthetic epoch budget.
    #[must_use]
    pub const fn epoch_budget(&self) -> u64 {
        self.epoch_budget
    }

    /// Returns the first owning operation observation.
    #[must_use]
    pub const fn operation_1(&self) -> &RecoveryAncestryTestOperationObservation {
        &self.operation_1
    }

    /// Returns the second owning operation observation.
    #[must_use]
    pub const fn operation_2(&self) -> &RecoveryAncestryTestOperationObservation {
        &self.operation_2
    }

    /// Returns the number of unclaimed synthetic epochs.
    #[must_use]
    pub const fn remaining_epochs(&self) -> u64 {
        self.remaining_epochs
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransactionOperationKind {
    /// Transaction-owned accepted-head recovery.
    AcceptedRecovery,
    /// Transaction-owned branch ancestry verification.
    BranchAncestry,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OwnerKind {
    AcceptedRecovery,
    BranchAncestry,
    RefRecovery,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum GuardRole {
    Inert,
    Owner,
    Join,
}

struct ActiveOperation {
    kind: OwnerKind,
    ordinal: usize,
    activated: bool,
    claims: u64,
    left_edges: u64,
    right_edges: u64,
    join_depth: u64,
}

struct Plan {
    generation: u64,
    owner_root: PathBuf,
    digest: [u8; 32],
    left: TransactionId,
    right: TransactionId,
    epoch_budget: RecoveryAncestryTestEpochs,
    consumed_epochs: u64,
    next_operation: usize,
    active: Option<ActiveOperation>,
    completed: [Option<RecoveryAncestryTestOperationObservation>; OPERATION_COUNT],
}

struct HookState {
    next_generation: u64,
    plan: Option<Plan>,
}

std::thread_local! {
    static HOOK_STATE: RefCell<HookState> = const {
        RefCell::new(HookState {
            next_generation: 0,
            plan: None,
        })
    };
}

/// Installs one exact same-thread synthetic ancestry plan.
#[must_use]
pub fn install(
    repository: &TransactionRepository,
    maintenance: &RepositoryMaintenanceGuard,
    epochs: RecoveryAncestryTestEpochs,
    left: TransactionId,
    right: TransactionId,
) -> Option<RecoveryAncestryTestPlanIdentity> {
    if !maintenance.is_exclusive() || !maintenance.covers(repository.root()) || left == right {
        return None;
    }
    let owner_root = maintenance.repository_root().to_path_buf();
    install_at_owner_root(owner_root, epochs, left, right)
}

/// Installs one exact same-thread ref-owner plan when the caller already owns
/// the enclosing repository maintenance guard.
#[doc(hidden)]
#[must_use]
pub fn install_for_ref_owner_test(
    repository: &TransactionRepository,
    epochs: RecoveryAncestryTestEpochs,
    left: TransactionId,
    right: TransactionId,
) -> Option<RecoveryAncestryTestPlanIdentity> {
    if left == right {
        return None;
    }
    let owner_root = ::std::fs::canonicalize(repository.root()).ok()?;
    install_at_owner_root(owner_root, epochs, left, right)
}

fn install_at_owner_root(
    owner_root: PathBuf,
    epochs: RecoveryAncestryTestEpochs,
    left: TransactionId,
    right: TransactionId,
) -> Option<RecoveryAncestryTestPlanIdentity> {
    let epoch_budget = epochs.count();
    let digest = plan_digest(&owner_root, epoch_budget, left, right);
    HOOK_STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.plan.is_some() {
            return None;
        }
        let generation = slot.next_generation.checked_add(1)?;
        slot.next_generation = generation;
        slot.plan = Some(Plan {
            generation,
            owner_root: owner_root.clone(),
            digest,
            left,
            right,
            epoch_budget: epochs,
            consumed_epochs: 0,
            next_operation: 0,
            active: None,
            completed: [None, None],
        });
        Some(RecoveryAncestryTestPlanIdentity {
            generation,
            owner_root,
            digest,
            armed: true,
            _not_send_or_sync: PhantomData,
        })
    })
}

/// Begins one ref-recovery owner operation.
#[must_use]
pub fn begin_ref_operation(
    repository: &TransactionRepository,
    maintenance: &RepositoryMaintenanceGuard,
) -> RecoveryAncestryTestOperationGuard {
    begin_operation(repository, maintenance, OwnerKind::RefRecovery)
}

/// Consumes one completed exact plan and returns both operation observations.
#[must_use]
pub fn take(
    repository: &TransactionRepository,
    maintenance: &RepositoryMaintenanceGuard,
    mut identity: RecoveryAncestryTestPlanIdentity,
) -> Option<RecoveryAncestryTestObservations> {
    let observations = HOOK_STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let plan = slot.plan.as_ref()?;
        if !maintenance.is_exclusive()
            || !maintenance.covers(repository.root())
            || maintenance.repository_root() != plan.owner_root
            || identity.generation != plan.generation
            || identity.owner_root != plan.owner_root
            || identity.digest != plan.digest
            || plan.active.is_some()
            || plan.next_operation != OPERATION_COUNT
            || plan.consumed_epochs != plan.epoch_budget.count()
        {
            return None;
        }
        let plan = slot.plan.take()?;
        let [Some(operation_1), Some(operation_2)] = plan.completed else {
            return None;
        };
        Some(RecoveryAncestryTestObservations {
            epoch_budget: plan.epoch_budget.count(),
            operation_1,
            operation_2,
            remaining_epochs: plan.epoch_budget.count() - plan.consumed_epochs,
        })
    });
    if observations.is_some() {
        identity.armed = false;
    }
    observations
}

pub(crate) fn begin_transaction_operation(
    repository: &TransactionRepository,
    maintenance: &RepositoryMaintenanceGuard,
    kind: TransactionOperationKind,
) -> RecoveryAncestryTestOperationGuard {
    let owner_kind = match kind {
        TransactionOperationKind::AcceptedRecovery => OwnerKind::AcceptedRecovery,
        TransactionOperationKind::BranchAncestry => OwnerKind::BranchAncestry,
    };
    begin_operation(repository, maintenance, owner_kind)
}

pub(crate) fn activate_ancestry_epoch(
    repository: &TransactionRepository,
    maintenance: &RepositoryMaintenanceGuard,
) {
    HOOK_STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(plan) = slot.plan.as_mut() else {
            return;
        };
        assert_operation_owner(plan, repository, maintenance);
        let active = plan.active.as_mut().expect("active recovery operation");
        assert!(matches!(
            (active.kind, active.join_depth),
            (OwnerKind::AcceptedRecovery | OwnerKind::BranchAncestry, 0)
                | (OwnerKind::RefRecovery, 1)
        ));
        assert!(!active.activated);
        assert!(plan.consumed_epochs < plan.epoch_budget.count());
        plan.consumed_epochs += 1;
        active.activated = true;
        active.claims = 1;
    });
}

pub(crate) fn substitute_verified_parents<'a>(
    root: &Path,
    revision: &'a VerifiedRevision,
) -> Cow<'a, [TransactionId]> {
    substitute_verified_parent_slice(
        root,
        revision.transaction_id(),
        revision
            .receipt()
            .transaction
            .record
            .parent_transaction_ids
            .as_slice(),
    )
}

fn substitute_verified_parent_slice<'a>(
    root: &Path,
    transaction_id: TransactionId,
    durable: &'a [TransactionId],
) -> Cow<'a, [TransactionId]> {
    HOOK_STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(plan) = slot.plan.as_mut() else {
            return Cow::Borrowed(durable);
        };
        let canonical_root = std::fs::canonicalize(root).expect("canonical recovery test root");
        assert_eq!(canonical_root, plan.owner_root);
        let Some(active) = plan.active.as_mut() else {
            return Cow::Borrowed(durable);
        };
        if !active.activated {
            return Cow::Borrowed(durable);
        }
        assert!(matches!(
            (active.kind, active.join_depth),
            (OwnerKind::AcceptedRecovery | OwnerKind::BranchAncestry, 0)
                | (OwnerKind::RefRecovery, 1)
        ));
        if transaction_id == plan.left {
            assert_eq!(durable, [plan.right]);
            active.left_edges = active.left_edges.checked_add(1).expect("left edge count");
            Cow::Owned(vec![plan.right])
        } else if transaction_id == plan.right {
            assert_eq!(durable.len(), 1);
            assert_ne!(durable[0], plan.left);
            assert_ne!(durable[0], plan.right);
            active.right_edges = active.right_edges.checked_add(1).expect("right edge count");
            Cow::Owned(vec![plan.left])
        } else {
            Cow::Borrowed(durable)
        }
    })
}

fn begin_operation(
    repository: &TransactionRepository,
    maintenance: &RepositoryMaintenanceGuard,
    requested_kind: OwnerKind,
) -> RecoveryAncestryTestOperationGuard {
    HOOK_STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(plan) = slot.plan.as_mut() else {
            return RecoveryAncestryTestOperationGuard::inert();
        };
        assert_operation_owner(plan, repository, maintenance);
        if let Some(active) = plan.active.as_mut() {
            assert_eq!(active.kind, OwnerKind::RefRecovery);
            assert_eq!(requested_kind, OwnerKind::BranchAncestry);
            assert_eq!(active.join_depth, 0);
            active.join_depth = 1;
            return RecoveryAncestryTestOperationGuard {
                generation: Some(plan.generation),
                owner_root: Some(plan.owner_root.clone()),
                digest: Some(plan.digest),
                ordinal: Some(active.ordinal),
                role: GuardRole::Join,
                _not_send_or_sync: PhantomData,
            };
        }
        assert!(plan.next_operation < OPERATION_COUNT);
        let ordinal = plan.next_operation;
        plan.next_operation += 1;
        plan.active = Some(ActiveOperation {
            kind: requested_kind,
            ordinal,
            activated: false,
            claims: 0,
            left_edges: 0,
            right_edges: 0,
            join_depth: 0,
        });
        RecoveryAncestryTestOperationGuard {
            generation: Some(plan.generation),
            owner_root: Some(plan.owner_root.clone()),
            digest: Some(plan.digest),
            ordinal: Some(ordinal),
            role: GuardRole::Owner,
            _not_send_or_sync: PhantomData,
        }
    })
}

fn assert_operation_owner(
    plan: &Plan,
    repository: &TransactionRepository,
    maintenance: &RepositoryMaintenanceGuard,
) {
    assert!(maintenance.is_exclusive());
    assert!(maintenance.covers(repository.root()));
    assert_eq!(maintenance.repository_root(), plan.owner_root);
}

fn clear_matching_plan(generation: u64, owner_root: &Path, digest: [u8; 32]) {
    HOOK_STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.plan.as_ref().is_some_and(|plan| {
            plan.generation == generation && plan.owner_root == owner_root && plan.digest == digest
        }) {
            slot.plan = None;
        }
    });
}

fn plan_digest(
    owner_root: &Path,
    epoch_budget: u64,
    left: TransactionId,
    right: TransactionId,
) -> [u8; 32] {
    let root_bytes = owner_root.as_os_str().as_encoded_bytes();
    let root_len = u64::try_from(root_bytes.len()).expect("test root length");
    let mut hasher = blake3::Hasher::new();
    hasher.update(PLAN_DIGEST_DOMAIN);
    hasher.update(&root_len.to_le_bytes());
    hasher.update(root_bytes);
    hasher.update(&epoch_budget.to_le_bytes());
    hasher.update(left.as_bytes());
    hasher.update(right.as_bytes());
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new(label: &str) -> Self {
            let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "sley-recovery-ancestry-hook-{label}-{}-{sequence:016x}",
                std::process::id()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn transaction(byte: u8) -> TransactionId {
        TransactionId::from_bytes([byte; 32])
    }

    #[test]
    fn two_epoch_plan_records_owner_and_joined_operations() {
        let root = TempRoot::new("two-epoch");
        let repository = TransactionRepository::new(&root.0);
        let maintenance = repository.acquire_exclusive_maintenance().unwrap();
        let left = transaction(1);
        let right = transaction(2);
        let genesis = transaction(3);
        let identity = install(
            &repository,
            &maintenance,
            RecoveryAncestryTestEpochs::Two,
            left,
            right,
        )
        .unwrap();
        assert_eq!(identity.owner_root(), maintenance.repository_root());

        {
            let _operation = begin_transaction_operation(
                &repository,
                &maintenance,
                TransactionOperationKind::AcceptedRecovery,
            );
            activate_ancestry_epoch(&repository, &maintenance);
            assert_eq!(
                substitute_verified_parent_slice(repository.root(), left, &[right]).as_ref(),
                &[right]
            );
        }
        {
            let _operation = begin_ref_operation(&repository, &maintenance);
            let _joined = begin_transaction_operation(
                &repository,
                &maintenance,
                TransactionOperationKind::BranchAncestry,
            );
            activate_ancestry_epoch(&repository, &maintenance);
            assert_eq!(
                substitute_verified_parent_slice(repository.root(), left, &[right]).as_ref(),
                &[right]
            );
            assert_eq!(
                substitute_verified_parent_slice(repository.root(), right, &[genesis]).as_ref(),
                &[left]
            );
        }

        let observations = take(&repository, &maintenance, identity).unwrap();
        assert_eq!(observations.epoch_budget(), 2);
        assert_eq!(observations.operation_1().claims(), 1);
        assert_eq!(observations.operation_1().edge_counts(), (1, 0));
        assert_eq!(observations.operation_2().claims(), 1);
        assert_eq!(observations.operation_2().edge_counts(), (1, 1));
        assert_eq!(observations.remaining_epochs(), 0);
    }

    #[test]
    fn one_epoch_plan_preserves_pre_ancestry_first_operation() {
        let root = TempRoot::new("one-epoch");
        let repository = TransactionRepository::new(&root.0);
        let maintenance = repository.acquire_exclusive_maintenance().unwrap();
        let left = transaction(4);
        let right = transaction(5);
        let genesis = transaction(6);
        let identity = install(
            &repository,
            &maintenance,
            RecoveryAncestryTestEpochs::One,
            left,
            right,
        )
        .unwrap();

        drop(begin_ref_operation(&repository, &maintenance));
        {
            let _operation = begin_ref_operation(&repository, &maintenance);
            let _joined = begin_transaction_operation(
                &repository,
                &maintenance,
                TransactionOperationKind::BranchAncestry,
            );
            activate_ancestry_epoch(&repository, &maintenance);
            assert_eq!(
                substitute_verified_parent_slice(repository.root(), left, &[right]).as_ref(),
                &[right]
            );
            assert_eq!(
                substitute_verified_parent_slice(repository.root(), right, &[genesis]).as_ref(),
                &[left]
            );
        }

        let observations = take(&repository, &maintenance, identity).unwrap();
        assert_eq!(observations.epoch_budget(), 1);
        assert_eq!(observations.operation_1().claims(), 0);
        assert_eq!(observations.operation_1().edge_counts(), (0, 0));
        assert_eq!(observations.operation_2().claims(), 1);
        assert_eq!(observations.operation_2().edge_counts(), (1, 1));
        assert_eq!(observations.remaining_epochs(), 0);
    }

    #[test]
    fn unwind_and_completed_take_both_allow_fresh_install() {
        let root = TempRoot::new("unwind-and-take");
        let repository = TransactionRepository::new(&root.0);
        let maintenance = repository.acquire_exclusive_maintenance().unwrap();
        let left = transaction(7);
        let right = transaction(8);
        let identity = install(
            &repository,
            &maintenance,
            RecoveryAncestryTestEpochs::Two,
            left,
            right,
        )
        .unwrap();

        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _operation = begin_transaction_operation(
                &repository,
                &maintenance,
                TransactionOperationKind::AcceptedRecovery,
            );
            activate_ancestry_epoch(&repository, &maintenance);
            panic!("test unwind");
        }));
        assert!(unwind.is_err());
        assert!(take(&repository, &maintenance, identity).is_none());

        let replacement = install(
            &repository,
            &maintenance,
            RecoveryAncestryTestEpochs::One,
            left,
            right,
        )
        .unwrap();
        drop(begin_ref_operation(&repository, &maintenance));
        {
            let _operation = begin_ref_operation(&repository, &maintenance);
            let _joined = begin_transaction_operation(
                &repository,
                &maintenance,
                TransactionOperationKind::BranchAncestry,
            );
            activate_ancestry_epoch(&repository, &maintenance);
        }
        assert!(take(&repository, &maintenance, replacement).is_some());
        assert!(
            install(
                &repository,
                &maintenance,
                RecoveryAncestryTestEpochs::One,
                left,
                right,
            )
            .is_some()
        );
    }

    #[test]
    fn stale_guard_cannot_clear_a_replacement_generation() {
        let root = TempRoot::new("stale-guard");
        let repository = TransactionRepository::new(&root.0);
        let maintenance = repository.acquire_exclusive_maintenance().unwrap();
        let left = transaction(9);
        let right = transaction(10);

        let stale_identity = install(
            &repository,
            &maintenance,
            RecoveryAncestryTestEpochs::Two,
            left,
            right,
        )
        .unwrap();
        let stale_guard = begin_transaction_operation(
            &repository,
            &maintenance,
            TransactionOperationKind::AcceptedRecovery,
        );
        drop(stale_identity);

        let replacement = install(
            &repository,
            &maintenance,
            RecoveryAncestryTestEpochs::One,
            left,
            right,
        )
        .unwrap();
        drop(stale_guard);
        drop(begin_ref_operation(&repository, &maintenance));
        {
            let _operation = begin_ref_operation(&repository, &maintenance);
            let _joined = begin_transaction_operation(
                &repository,
                &maintenance,
                TransactionOperationKind::BranchAncestry,
            );
            activate_ancestry_epoch(&repository, &maintenance);
        }
        assert!(take(&repository, &maintenance, replacement).is_some());
    }

    #[test]
    fn failed_take_consumes_identity_and_clears_its_matching_plan() {
        let root = TempRoot::new("failed-take-owner");
        let other_root = TempRoot::new("failed-take-other");
        let repository = TransactionRepository::new(&root.0);
        let other_repository = TransactionRepository::new(&other_root.0);
        let maintenance = repository.acquire_exclusive_maintenance().unwrap();
        let other_maintenance = other_repository.acquire_exclusive_maintenance().unwrap();
        let left = transaction(11);
        let right = transaction(12);
        let identity = install(
            &repository,
            &maintenance,
            RecoveryAncestryTestEpochs::One,
            left,
            right,
        )
        .unwrap();

        assert!(take(&other_repository, &other_maintenance, identity).is_none());
        assert!(
            install(
                &repository,
                &maintenance,
                RecoveryAncestryTestEpochs::One,
                left,
                right,
            )
            .is_some()
        );
    }

    #[test]
    fn ref_owner_cannot_activate_before_branch_verifier_join() {
        let root = TempRoot::new("activation-before-join");
        let repository = TransactionRepository::new(&root.0);
        let maintenance = repository.acquire_exclusive_maintenance().unwrap();
        let left = transaction(13);
        let right = transaction(14);
        let identity = install(
            &repository,
            &maintenance,
            RecoveryAncestryTestEpochs::One,
            left,
            right,
        )
        .unwrap();

        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _owner = begin_ref_operation(&repository, &maintenance);
            activate_ancestry_epoch(&repository, &maintenance);
        }));
        assert!(unwind.is_err());
        assert!(take(&repository, &maintenance, identity).is_none());
        assert!(
            install(
                &repository,
                &maintenance,
                RecoveryAncestryTestEpochs::One,
                left,
                right,
            )
            .is_some()
        );
    }

    #[test]
    fn ref_owner_cannot_substitute_after_branch_verifier_join_drops() {
        let root = TempRoot::new("substitution-after-join");
        let repository = TransactionRepository::new(&root.0);
        let maintenance = repository.acquire_exclusive_maintenance().unwrap();
        let left = transaction(15);
        let right = transaction(16);
        let identity = install(
            &repository,
            &maintenance,
            RecoveryAncestryTestEpochs::One,
            left,
            right,
        )
        .unwrap();

        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _owner = begin_ref_operation(&repository, &maintenance);
            {
                let _joined = begin_transaction_operation(
                    &repository,
                    &maintenance,
                    TransactionOperationKind::BranchAncestry,
                );
                activate_ancestry_epoch(&repository, &maintenance);
            }
            let _parents = substitute_verified_parent_slice(repository.root(), left, &[right]);
        }));
        assert!(unwind.is_err());
        assert!(take(&repository, &maintenance, identity).is_none());
        assert!(
            install(
                &repository,
                &maintenance,
                RecoveryAncestryTestEpochs::One,
                left,
                right,
            )
            .is_some()
        );
    }
}
