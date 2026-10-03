//! Allocator-observed heap peaks for the single-thread workbench binary.
//!
//! The binary's isolated allocator reports live allocation extents, including
//! size-class rounding. Idle cached blocks, allocator metadata, stacks and OS
//! mappings are outside this measurement. Reallocation provisionally observes
//! both source and destination even when the system can resize in place.
//! This observes bounded operations; it does not turn infallible Rust allocation
//! into a fallible operation or impose an OS address-space/RSS limit.

use std::cell::{Cell, RefCell};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use serde_json::{Value, json};

use crate::error::Result;

const SCOPES: usize = 32;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static INVALID: AtomicBool = AtomicBool::new(false);

struct State {
    baseline: usize,
    peak: AtomicUsize,
    active: AtomicBool,
}

struct Watches {
    slots: [Option<Arc<State>>; SCOPES],
    bound: usize,
}

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static WATCHING: Cell<bool> = const { Cell::new(false) };
    static WATCHES: RefCell<Watches> = const {
        RefCell::new(Watches { slots: [const { None }; SCOPES], bound: 0 })
    };
}

/// Enables observations on this planning thread after installing the binary's
/// instrumented global allocator. This must never be called with an allocator
/// that omits the reporting calls below. Library-only callers remain explicitly
/// uninstrumented; no allocator is installed or replaced by this function.
pub fn enable_current_thread() {
    ENABLED.with(|enabled| enabled.set(true));
}

#[inline]
fn observe(peak: usize) {
    if !WATCHING.try_with(Cell::get).unwrap_or(false) {
        return;
    }
    observe_active(peak);
}

// Keep the inactive allocator path small across the library/binary boundary.
// Planning still traverses every active observation without changing its peak.
#[inline(never)]
fn observe_active(peak: usize) {
    // No allocation, panic on recursive borrowing, or TLS initialization with
    // heap storage is permitted on the allocator path. Inactive watches are
    // reclaimed by the next scope creation, outside allocator callbacks.
    let _ = WATCHES.try_with(|watches| {
        if let Ok(watches) = watches.try_borrow() {
            let mut active = false;
            for state in watches.slots[..watches.bound].iter().flatten() {
                if state.active.load(Ordering::Relaxed) {
                    active = true;
                    state.peak.fetch_max(peak, Ordering::Relaxed);
                }
            }
            if !active {
                let _ = WATCHING.try_with(|watching| watching.set(false));
            }
        }
    });
}

/// Reports one successful allocation's full live extent, including class
/// rounding. Called by the binary allocator; does not allocate or unwind.
#[inline]
pub fn allocated(bytes: usize) {
    // A wrapping counter is unusable, but INVALID is permanent and every
    // instrumented budget then refuses. One atomic addition avoids a CAS retry
    // loop on every ordinary workbench allocation.
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed);
    if let Some(peak) = live.checked_add(bytes) {
        observe(peak);
    } else {
        INVALID.store(true, Ordering::Relaxed);
    }
}

/// Reports release of a live allocation, including when its block becomes idle
/// in the allocator cache. Cross-thread releases use the same atomic counter.
#[inline]
pub fn deallocated(bytes: usize) {
    if LIVE.fetch_sub(bytes, Ordering::Relaxed) < bytes {
        INVALID.store(true, Ordering::Relaxed);
    }
}

/// Observes conservative source-plus-destination scratch for system reallocation
/// before it begins. It does not change the live counter or allocate storage.
#[inline]
pub fn reallocating(destination: usize) {
    if let Some(peak) = LIVE.load(Ordering::Relaxed).checked_add(destination) {
        observe(peak);
    } else {
        INVALID.store(true, Ordering::Relaxed);
    }
}

pub(super) enum Monitor {
    Disabled,
    Unavailable,
    Active(Observation),
}

pub(super) struct Observation {
    state: Arc<State>,
}

impl Drop for Observation {
    fn drop(&mut self) {
        // Safe even if the Budget is dropped on another thread. The original
        // thread's fixed watch slot remains inactive until its next scope.
        self.state.active.store(false, Ordering::Relaxed);
    }
}

impl Monitor {
    pub(super) fn start() -> Self {
        if !ENABLED.try_with(Cell::get).unwrap_or(false) {
            return Self::Disabled;
        }
        let baseline = LIVE.load(Ordering::Relaxed);
        let state = Arc::new(State {
            baseline,
            peak: AtomicUsize::new(baseline),
            active: AtomicBool::new(true),
        });
        let installed = WATCHES
            .try_with(|watches| {
                let Ok(mut watches) = watches.try_borrow_mut() else {
                    return false;
                };
                let Some(slot) = watches.slots.iter_mut().find(|slot| {
                    slot.as_ref()
                        .is_none_or(|state| !state.active.load(Ordering::Relaxed))
                }) else {
                    return false;
                };
                *slot = Some(Arc::clone(&state));
                watches.bound = watches
                    .slots
                    .iter()
                    .rposition(|slot| {
                        slot.as_ref()
                            .is_some_and(|state| state.active.load(Ordering::Relaxed))
                    })
                    .map_or(0, |index| index + 1);
                true
            })
            .unwrap_or(false);
        if installed {
            WATCHING.with(|watching| watching.set(true));
            Self::Active(Observation { state })
        } else {
            Self::Unavailable
        }
    }

    pub(super) fn check(&self, limit: usize) -> Result<()> {
        match self {
            Self::Disabled => Ok(()),
            Self::Unavailable => Err(super::limit(
                "allocator heap observation unavailable; use explicit authoring",
            )),
            Self::Active(observation) => {
                observation
                    .state
                    .peak
                    .fetch_max(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
                if INVALID.load(Ordering::Relaxed) {
                    return Err(super::limit(
                        "allocator heap accounting became invalid; use explicit authoring",
                    ));
                }
                let additional = observation
                    .state
                    .peak
                    .load(Ordering::Relaxed)
                    .saturating_sub(observation.state.baseline);
                if additional > limit {
                    return Err(super::limit(
                        "aggregate observed planner heap budget exhausted; use explicit authoring",
                    ));
                }
                Ok(())
            }
        }
    }

    pub(super) fn status(&self) -> &'static str {
        match self {
            Self::Disabled => "not_implemented",
            Self::Unavailable => "observer_unavailable",
            Self::Active(_) => "allocator_peak_checked_at_budget_checkpoints",
        }
    }

    pub(super) fn usage(&self, limit: usize) -> Value {
        let mut value = json!({"status":self.status(),
            "limit_additional_bytes":limit,
            "units":"live_allocation_extent_bytes_including_size_class_rounding",
            "reallocation":"conservative_simultaneous_source_and_destination",
            "scope":"single_planning_thread; process_live_heap_delta_since_budget_creation",
            "excludes":"idle allocator cache, system allocator metadata, stacks, OS mappings; pre-budget allocations are baseline",
            "enforcement":"at checkpoints; individual allocations can overshoot; not an OS memory cap"});
        if let Self::Active(observation) = self {
            let current = LIVE.load(Ordering::Relaxed);
            observation.state.peak.fetch_max(current, Ordering::Relaxed);
            let peak = observation.state.peak.load(Ordering::Relaxed);
            value["baseline_bytes"] = json!(observation.state.baseline);
            value["current_bytes"] = json!(current);
            value["peak_bytes"] = json!(peak);
            value["peak_additional_bytes"] = json!(peak.saturating_sub(observation.state.baseline));
            value["exhausted"] = json!(peak.saturating_sub(observation.state.baseline) > limit);
            value["accounting_valid"] = json!(!INVALID.load(Ordering::Relaxed));
        }
        value
    }
}
