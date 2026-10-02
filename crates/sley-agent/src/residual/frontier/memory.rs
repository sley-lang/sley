//! Shared reservations for explicitly accounted planner allocations.
//! This does not observe arbitrary allocations or replace a process memory cap.

use crate::error::{AgentError, Result};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

pub(super) const MAX_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug)]
struct State {
    limit: usize,
    used: AtomicUsize,
    peak: AtomicUsize,
    exhausted: AtomicBool,
}

pub(super) struct Memory {
    state: Arc<State>,
}

/// Owns a reservation independently of a mutable planning-budget borrow.
/// It is neither cloneable nor a way to raise the ceiling. Dropping it releases
/// capacity, including on every error path, but does not clear exhaustion.
#[derive(Debug)]
pub(super) struct Reservation {
    state: Arc<State>,
    bytes: usize,
}

impl Reservation {
    /// Release a conservative unused allowance once the retained shape is known.
    /// This cannot acquire capacity or clear a sticky exhaustion observation.
    pub fn shrink(&mut self, bytes: usize) {
        assert!(bytes <= self.bytes, "reservation cannot grow by shrinking");
        self.state
            .used
            .fetch_sub(self.bytes - bytes, Ordering::Relaxed);
        self.bytes = bytes;
    }
}

impl Memory {
    pub fn new(limit: usize) -> Self {
        Self {
            state: Arc::new(State {
                limit: limit.min(MAX_BYTES),
                used: AtomicUsize::new(0),
                peak: AtomicUsize::new(0),
                exhausted: AtomicBool::new(false),
            }),
        }
    }

    pub fn check(&self) -> Result<()> {
        if self.state.exhausted.load(Ordering::Relaxed) {
            return Err(Self::error());
        }
        Ok(())
    }

    fn error() -> AgentError {
        super::limit("aggregate planner memory budget exhausted; use explicit authoring")
    }

    fn exhaust(&self) -> AgentError {
        self.state.exhausted.store(true, Ordering::Relaxed);
        Self::error()
    }

    /// Reserve an exact requested capacity before allocating. Atomic accounting
    /// permits a reservation to be dropped on another thread; Budget still
    /// prohibits planning or acquiring reservations on a different worker.
    pub fn reserve(&self, bytes: Option<usize>) -> Result<Reservation> {
        self.check()?;
        let Some(bytes) = bytes else {
            return Err(self.exhaust());
        };
        let mut used = self.state.used.load(Ordering::Relaxed);
        loop {
            let Some(next) = used
                .checked_add(bytes)
                .filter(|next| *next <= self.state.limit)
            else {
                return Err(self.exhaust());
            };
            match self.state.used.compare_exchange_weak(
                used,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    self.state.peak.fetch_max(next, Ordering::Relaxed);
                    return Ok(Reservation {
                        state: Arc::clone(&self.state),
                        bytes,
                    });
                }
                Err(actual) => used = actual,
            }
        }
    }

    pub fn usage(&self) -> serde_json::Value {
        serde_json::json!({
            "reserved_bytes":self.state.used.load(Ordering::Relaxed),
            "peak_reserved_bytes":self.state.peak.load(Ordering::Relaxed),
            "limit_bytes":self.state.limit,
            "exhausted":self.state.exhausted.load(Ordering::Relaxed),
            "units":"requested_heap_capacity_bytes",
            "coverage":"pair coverage, borrowed-value indexes, greedy and exact-refinement scratch, borrowed join rows and size indexes, component and closed-relation encoding buffers, decision-domain encoding keys and borrowed-value vectors, family sort keys and canonical member indexes, retained family field/row vectors, copied field names and nested key/string/array capacity; retained ordinary/factored frontier field vectors, names, identity strings and component vectors (pre-reserved upper bounds tightened to final capacities)",
            "excludes":"parser peak, map nodes, family digest strings, other retained JSON trees, graph/compiler allocations, Arc storage and allocator overhead; family/frontier reservations stay with their construction budget",
        })
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        self.state.used.fetch_sub(self.bytes, Ordering::Relaxed);
    }
}
