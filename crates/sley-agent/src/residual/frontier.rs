//! Bounded finite-description reconstruction, not semantic entitlement.
//!
//! A separating projection establishes unique reconstruction only within the
//! supplied rows. It establishes neither family completeness for a task nor
//! permission to omit semantic decisions. The CLI must retain its entitlement
//! checks before using this mathematical core to construct a program.

pub(crate) mod encoding;
pub mod factors;
mod family;
pub mod heap;
mod memory;
pub mod working_memory;

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::error::{AgentError, AgentErrorCode, Result};

/// Maximum explicit descriptions in a coupled component.
pub const MAX_DESCRIPTIONS: usize = 256;
/// Maximum decision fields across an invocation.
pub const MAX_FIELDS: usize = 64;
const MAX_WORK: u64 = 12_000_000;
const FAST_PATH_WALL: Duration = Duration::from_millis(250);
const MAX_EXACT_FIELDS: usize = 12;

#[derive(Clone, Debug)]
struct Field {
    name: String,
    cost: u32,
    eligible: bool,
}

/// A validated finite relation of typed JSON descriptions. Its rows are data,
/// not a claim that the author intended any of them or that a search is complete.
#[derive(Clone, Debug)]
pub struct Family {
    fields: Arc<Vec<Field>>,
    rows: Arc<Vec<Map<String, Value>>>,
    digest: String,
    _memory: Arc<memory::Reservation>,
}

impl Family {
    /// Reads `{fields:[{name,cost,eligible}],descriptions:[{field:value,...}]}`.
    /// Costs are positive u32 additive surrogate units, never billed tokens.
    /// All rows must provide exactly the declared fields, including ineligible
    /// fields. Row and field order do not affect identity or deterministic ties.
    ///
    /// # Errors
    /// Refuses duplicate JSON keys, floats, unknown members, empty families,
    /// incomplete/duplicate rows, zero/oversized costs, and structural bounds.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        Self::parse_with_budget(bytes, &mut Budget::default())
    }

    /// Parses and canonicalizes using the caller's invocation budget.
    /// Sorting keys, canonical traversal and retained vector/string capacity
    /// are charged. Clones share the payload and its reservation. Parser peak,
    /// map nodes and allocation overhead remain outside this accounting.
    /// Retained charges belong to this budget, even if later planning uses
    /// another budget; use one budget throughout an invocation.
    ///
    /// # Errors
    /// The same structural refusals as [`Self::parse`], plus shared-budget
    /// exhaustion. No partially validated family is returned.
    pub fn parse_with_budget(bytes: &[u8], budget: &mut Budget) -> Result<Self> {
        budget.checkpoint()?;
        let value = super::strict_json(bytes)?;
        budget.checkpoint()?;
        family::from_value(value, budget)
    }

    /// Complete supplied descriptions in deterministic order.
    #[must_use]
    pub fn descriptions(&self) -> &[Map<String, Value>] {
        &self.rows
    }
}

/// One aggregate budget shared across component plans and exact refinements.
/// Synchronous code creates no workers. Work units count bounded bitset steps;
/// wall time is checked during every charged batch. Linux additionally checks
/// this thread's CPU time; elsewhere the single-thread wall ceiling provides a
/// conservative CPU upper bound. Explicit scratch and retained family payload
/// reservations share one ceiling. The instrumented binary also observes
/// aggregate live-heap peaks at checkpoints; library-only callers must retain
/// the explicit uninstrumented status. The CLI additionally checks a complete
/// user-address-space working-memory upper bound at stage/CPU checkpoints.
/// These observations are not an instantaneous OS allocation/RSS cap.
pub struct Budget {
    start: Instant,
    wall: Duration,
    thread: std::thread::ThreadId,
    cpu_start: Option<u64>,
    cpu_limit: Duration,
    cpu_checked: Instant,
    remaining: u64,
    work_limit: u64,
    fields_used: usize,
    fast_path: bool,
    memory: memory::Memory,
    heap: heap::Monitor,
    heap_limit: usize,
    working_memory: working_memory::Monitor,
}

impl Default for Budget {
    fn default() -> Self {
        Self::limited(Duration::from_secs(2), MAX_WORK)
    }
}

impl Budget {
    /// Selects the explicit-authoring fast path without restarting the clock,
    /// resetting consumed work, or relaxing a caller's tighter wall ceiling.
    pub(crate) fn use_fast_path(&mut self) -> Result<()> {
        self.wall = self.wall.min(FAST_PATH_WALL);
        self.fast_path = true;
        self.checkpoint()
    }

    /// Checks the shared elapsed/work budget between bounded construction steps.
    ///
    /// # Errors
    /// Refuses an exhausted budget without resetting its start or counters.
    pub fn checkpoint(&mut self) -> Result<()> {
        self.charge(1)?;
        self.check_cpu()
    }

    /// Current resource observations and ceilings for inspection, not tokens.
    /// CPU is measured only when a thread clock is available; the wall ceiling
    /// bounds CPU conservatively for this synchronous, single-thread budget.
    #[must_use]
    pub fn usage(&self) -> Value {
        let cpu = self
            .cpu_start
            .zip(cpu_nanos())
            .and_then(|(start, now)| now.checked_sub(start));
        json!({
            "wall_micros":self.start.elapsed().as_micros(),
            "cpu_micros":cpu.map(|nanos| nanos / 1000),
            "cpu_accounting":if self.cpu_start.is_some() {"thread_schedstat"} else {"single_thread_wall_upper_bound"},
            "wall_limit_micros":self.wall.as_micros(),
            "cpu_limit_micros":self.cpu_limit.as_micros(),
            "charged_work":self.work_limit - self.remaining,
            "work_limit":self.work_limit,
            "planned_fields":self.fields_used,
            "planning_route":if self.fast_path {"explicit_fast_path"} else {"finite_frontier"},
            "workers":1,
            "memory_enforcement":"frontier_scratch_and_retained_payload_reservations",
            "aggregate_memory_enforcement":self.heap.status(),
            "allocator_heap":self.heap.usage(self.heap_limit),
            "complete_working_memory":self.working_memory.usage(self.heap_limit),
            "memory":self.memory.usage(),
            "scope":"after initial envelope parsing and startup hashing through preparation; excludes persistence and ordinary trial",
        })
    }
    /// Tightens the default two-second and work-unit ceilings; cannot raise them.
    #[must_use]
    pub fn limited(wall: Duration, work: u64) -> Self {
        Self::limited_with_memory(wall, work, memory::MAX_BYTES)
    }

    /// Tightens the wall, work and accounted memory ceilings. This does
    /// not impose a process-wide memory limit. The binary's additional heap
    /// observation and enabled complete working-memory bound use the same
    /// ceiling, with their explicit measurement scopes.
    #[must_use]
    pub fn limited_with_memory(wall: Duration, work: u64, bytes: usize) -> Self {
        Self {
            start: Instant::now(),
            wall: wall.min(Duration::from_secs(2)),
            thread: std::thread::current().id(),
            cpu_start: cpu_nanos(),
            cpu_limit: Duration::from_secs(2),
            cpu_checked: Instant::now(),
            remaining: work.min(MAX_WORK),
            work_limit: work.min(MAX_WORK),
            fields_used: 0,
            fast_path: false,
            memory: memory::Memory::new(bytes),
            heap: heap::Monitor::start(),
            heap_limit: bytes.min(memory::MAX_BYTES),
            working_memory: working_memory::Monitor::start(),
        }
    }

    fn reserve<T>(&mut self, count: usize) -> Result<memory::Reservation> {
        self.charge(0)?;
        self.memory
            .reserve(count.checked_mul(std::mem::size_of::<T>()))
    }

    fn check_cpu(&mut self) -> Result<()> {
        self.working_memory.check(self.heap_limit)?;
        if let Some(start) = self.cpu_start {
            let elapsed = cpu_nanos()
                .and_then(|now| now.checked_sub(start))
                .ok_or_else(|| {
                    limit("planner CPU observation became unavailable; use explicit authoring")
                })?;
            if Duration::from_nanos(elapsed) >= self.cpu_limit {
                return Err(limit(
                    "aggregate planner CPU budget exhausted; use explicit authoring",
                ));
            }
        }
        self.cpu_checked = Instant::now();
        Ok(())
    }

    pub(crate) fn charge(&mut self, work: usize) -> Result<()> {
        if std::thread::current().id() != self.thread {
            return Err(limit(
                "planning budget cannot move to another worker thread",
            ));
        }
        self.memory.check()?;
        self.heap.check(self.heap_limit)?;
        let work = u64::try_from(work).map_err(|_| limit("work counter overflow"))?;
        if self.start.elapsed() >= self.wall || work > self.remaining {
            return Err(limit(if self.fast_path {
                "aggregate explicit fast-path budget exhausted; use ordinary AF1-X"
            } else {
                "aggregate finite-frontier budget exhausted; use explicit authoring"
            }));
        }
        self.remaining -= work;
        // Avoid a procfs read for every pair comparison, while checking at
        // least every 5 ms of charged work and at every stage checkpoint.
        if self.cpu_checked.elapsed() >= Duration::from_millis(5) {
            self.check_cpu()?;
        }
        Ok(())
    }
}

fn cpu_nanos() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        // Same per-thread source used by the existing workbench search ledger.
        std::fs::read_to_string("/proc/thread-self/schedstat")
            .ok()?
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Optimization of the additive surrogate only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Method {
    /// Checked feasible greedy projection, with redundant fields removed.
    Greedy,
    /// Every eligible subset was inspected within the small-family ceiling.
    ExactAdditive,
    /// An exact refinement exhausted its budget; the best already checked
    /// feasible projection is retained without an optimality claim.
    BoundedBest,
}

/// Immutable, checked projection tied to one complete supplied relation.
/// No constructor or deserializer accepts unchecked fields or certificates.
/// Clones share retained storage and its construction-budget reservation.
#[derive(Clone, Debug)]
pub struct Frontier {
    family: Arc<String>,
    fields: Arc<Vec<String>>,
    cost: u64,
    method: Method,
    _memory: Arc<memory::Reservation>,
}

impl Frontier {
    /// Questions in lexical order, with no opaque description-index question.
    #[must_use]
    pub fn fields(&self) -> &[String] {
        &self.fields
    }
    /// Sum of the selected positive additive cost estimates.
    #[must_use]
    pub const fn cost(&self) -> u64 {
        self.cost
    }
    /// Whether optimization completed or remains heuristic.
    #[must_use]
    pub const fn method(&self) -> Method {
        self.method
    }
    /// Truthful reconstruction scope and explicit absence of semantic evidence.
    #[must_use]
    pub fn summary(&self) -> Value {
        json!({"family":*self.family,"fields":*self.fields,"additive_cost":self.cost,
            "method":match self.method {Method::Greedy=>"greedy",Method::ExactAdditive=>"exact_additive",Method::BoundedBest=>"bounded_best"},
            "guarantee":"unique reconstruction within the supplied descriptions",
            "semantic_entitlement":"not_established","family_completeness_for_task":"not_established",
            "billed_cost_optimality":"not_established"})
    }

    /// Projects a complete member, never an unseen/approximate description.
    ///
    /// # Errors
    /// Refuses changed family identity or a description outside the exact family.
    pub fn encode(
        &self,
        family: &Family,
        description: &Map<String, Value>,
    ) -> Result<Map<String, Value>> {
        self.bound(family)?;
        if !family.rows.contains(description) {
            return Err(failure(
                AgentErrorCode::ResidualChoiceUnknown,
                "description is outside the bound family",
            ));
        }
        Ok(self
            .fields
            .iter()
            .map(|field| (field.clone(), description[field].clone()))
            .collect())
    }

    /// Reconstructs exactly one supplied description from all frontier answers.
    /// Values retain their JSON types: true, 1, and "1" are different choices.
    ///
    /// # Errors
    /// Refuses changed identity, missing/unknown fields and answers with no
    /// matching complete description. Never returns a closest completion.
    pub fn decode(
        &self,
        family: &Family,
        answers: &Map<String, Value>,
    ) -> Result<Map<String, Value>> {
        self.bound(family)?;
        if answers.keys().any(|field| !self.fields.contains(field)) {
            return Err(failure(
                AgentErrorCode::ResidualChoiceUnknown,
                "answer names a non-frontier field",
            ));
        }
        if answers.len() != self.fields.len() {
            return Err(failure(
                AgentErrorCode::ResidualChoiceMissing,
                "answer every frontier field",
            ));
        }
        let mut matching = family.rows.iter().filter(|row| {
            answers
                .iter()
                .all(|(key, value)| row.get(key) == Some(value))
        });
        let row = matching.next().ok_or_else(|| {
            failure(
                AgentErrorCode::ResidualChoiceUnknown,
                "answers have no completion in the exact family",
            )
        })?;
        if matching.next().is_some() {
            return Err(failure(
                AgentErrorCode::ResidualInconclusive,
                "frontier failed unique reconstruction",
            ));
        }
        Ok(row.clone())
    }

    fn bound(&self, family: &Family) -> Result<()> {
        if *self.family != family.digest {
            return Err(failure(
                AgentErrorCode::ResidualBindingStale,
                "description family, vocabulary or cost profile changed",
            ));
        }
        Ok(())
    }
}

struct Coverage {
    columns: Vec<Vec<u64>>,
    full: Vec<u64>,
    // Declared last so its reservation outlives the owned vectors.
    _memory: memory::Reservation,
}

impl Coverage {
    fn new(family: &Family, budget: &mut Budget) -> Result<Self> {
        budget.charge(1)?;
        let pairs = family.rows.len() * (family.rows.len() - 1) / 2;
        let words = pairs.div_ceil(64);
        let storage = family.fields.len() * std::mem::size_of::<Vec<u64>>()
            + (family.fields.len() + 1) * words * std::mem::size_of::<u64>();
        let reservation = budget.reserve::<u8>(storage)?;
        // <= 64 * 510 words plus a 510-word target: under 266 KiB.
        let mut columns = vec![vec![0_u64; words]; family.fields.len()];
        let mut full = vec![u64::MAX; words];
        if let Some(last) = full.last_mut() {
            *last = u64::MAX >> ((64 - pairs % 64) % 64);
        }
        // Resolve field names once per row. Pair comparisons otherwise perform
        // millions of repeated BTreeMap lookups in a maximum-size component.
        // At most 256 * 64 borrowed value references; no domain values copied.
        let _borrowed = budget.reserve::<u8>(
            family.rows.len()
                * (std::mem::size_of::<Vec<&Value>>()
                    + family.fields.len() * std::mem::size_of::<&Value>()),
        )?;
        let mut values = Vec::with_capacity(family.rows.len());
        for row in family.rows.iter() {
            budget.charge(family.fields.len() + 1)?;
            values.push(
                family
                    .fields
                    .iter()
                    .map(|field| &row[&field.name])
                    .collect::<Vec<_>>(),
            );
        }
        let mut pair = 0;
        for left in 0..family.rows.len() {
            for right in left + 1..family.rows.len() {
                budget.charge(family.fields.len() + 1)?;
                let mut distinguished = false;
                for (index, field) in family.fields.iter().enumerate() {
                    if field.eligible && values[left][index] != values[right][index] {
                        columns[index][pair / 64] |= 1_u64 << (pair % 64);
                        distinguished = true;
                    }
                }
                if !distinguished {
                    return Err(failure(
                        AgentErrorCode::ResidualVocabularyIncomplete,
                        "eligible fields cannot distinguish a pair of complete descriptions",
                    ));
                }
                pair += 1;
            }
        }
        Ok(Self {
            columns,
            full,
            _memory: reservation,
        })
    }

    fn separates(&self, selected: u64, budget: &mut Budget) -> Result<bool> {
        budget.charge(self.full.len().max(1) * (selected.count_ones() as usize + 1))?;
        for (word, expected) in self.full.iter().enumerate() {
            let mut actual = 0_u64;
            for index in bits(selected) {
                actual |= self.columns[index][word];
            }
            if actual != *expected {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Plans a feasible weighted pair-separating frontier. Ties use lexical field
/// order and integer cross multiplication, with no floating point rounding.
/// Optional exact refinement enumerates all subsets only up to 12 eligible
/// fields; work/time exhaustion keeps the best previously checked feasible
/// projection. Accounted-memory exhaustion always refuses.
///
/// # Errors
/// Refuses inadequate vocabulary, accounted-memory exhaustion, or work/time
/// exhaustion before a feasible projection has been fully checked. It never
/// samples away descriptions.
pub fn plan(family: &Family, budget: &mut Budget, refine_exact: bool) -> Result<Frontier> {
    budget.fields_used = budget.fields_used.saturating_add(family.fields.len());
    if budget.fields_used > MAX_FIELDS {
        return Err(limit("aggregate invocation exceeds 64 planned fields"));
    }
    // Reserve output before refinement, which can legally exhaust work/time
    // while retaining a previously checked feasible answer. Final allocation
    // must not need a new budget or an unchecked memory allowance at that point.
    let vector_bytes = family.fields.len() * size_of::<String>();
    let mut retained = budget.reserve::<u8>(
        vector_bytes
            + family.digest.len()
            + family
                .fields
                .iter()
                .map(|field| field.name.len())
                .sum::<usize>(),
    )?;
    let coverage = Coverage::new(family, budget)?;
    let mut selected = greedy(family, &coverage, budget)?;
    budget.charge(0)?;
    let mut method = Method::Greedy;
    if refine_exact
        && family.fields.iter().filter(|field| field.eligible).count() <= MAX_EXACT_FIELDS
    {
        match refine(family, &coverage, budget, &mut selected) {
            Ok(()) => method = Method::ExactAdditive,
            Err(error) if error.code() == AgentErrorCode::ResidualLimit => {
                budget.memory.check()?;
                method = Method::BoundedBest;
            }
            Err(error) => return Err(error),
        }
    }
    let mut fields = Vec::with_capacity(family.fields.len());
    fields.extend(bits(selected).map(|index| family.fields[index].name.clone()));
    retained
        .shrink(vector_bytes + family.digest.len() + fields.iter().map(String::len).sum::<usize>());
    Ok(Frontier {
        family: Arc::new(family.digest.clone()),
        fields: Arc::new(fields),
        cost: cost(family, selected),
        method,
        _memory: Arc::new(retained),
    })
}

fn greedy(family: &Family, coverage: &Coverage, budget: &mut Budget) -> Result<u64> {
    let _memory = budget.reserve::<u8>(
        coverage.full.len() * std::mem::size_of::<u64>()
            + family.fields.len() * std::mem::size_of::<usize>(),
    )?;
    let mut uncovered = coverage.full.clone();
    let mut selected = 0_u64;
    while uncovered.iter().any(|word| *word != 0) {
        let mut best: Option<(usize, u64)> = None;
        for (index, field) in family.fields.iter().enumerate() {
            if !field.eligible || selected & (1_u64 << index) != 0 {
                continue;
            }
            budget.charge(uncovered.len().max(1))?;
            let gain: u64 = coverage.columns[index]
                .iter()
                .zip(&uncovered)
                .map(|(column, remaining)| u64::from((column & remaining).count_ones()))
                .sum();
            if gain > 0
                && best.is_none_or(|(previous, old_gain)| {
                    gain * u64::from(family.fields[previous].cost)
                        > old_gain * u64::from(field.cost)
                })
            {
                best = Some((index, gain));
            }
        }
        let (index, _) = best.ok_or_else(|| {
            failure(
                AgentErrorCode::ResidualVocabularyIncomplete,
                "no eligible separating field",
            )
        })?;
        selected |= 1_u64 << index;
        for (remaining, column) in uncovered.iter_mut().zip(&coverage.columns[index]) {
            *remaining &= !column;
        }
    }
    let mut removal = Vec::with_capacity(family.fields.len());
    removal.extend(bits(selected));
    // Total tie order makes stable sorting unnecessary; keep scratch bounded.
    removal.sort_unstable_by(|left, right| {
        family.fields[*right]
            .cost
            .cmp(&family.fields[*left].cost)
            .then_with(|| left.cmp(right))
    });
    for index in removal {
        let reduced = selected & !(1_u64 << index);
        if coverage.separates(reduced, budget)? {
            selected = reduced;
        }
    }
    // This check is mandatory even when no redundancy was removed.
    if !coverage.separates(selected, budget)? {
        return Err(failure(
            AgentErrorCode::ResidualInconclusive,
            "unchecked separating projection",
        ));
    }
    Ok(selected)
}

fn refine(family: &Family, coverage: &Coverage, budget: &mut Budget, best: &mut u64) -> Result<()> {
    let _memory = budget.reserve::<usize>(family.fields.len())?;
    let mut eligible = Vec::with_capacity(family.fields.len());
    eligible.extend(
        family
            .fields
            .iter()
            .enumerate()
            .filter(|(_, field)| field.eligible)
            .map(|(index, _)| index),
    );
    for subset in 0_u64..(1_u64 << eligible.len()) {
        budget.charge(eligible.len() + 1)?;
        let selected = bits(subset).fold(0_u64, |mask, index| mask | (1_u64 << eligible[index]));
        let candidate_cost = cost(family, selected);
        let best_cost = cost(family, *best);
        if candidate_cost > best_cost
            || (candidate_cost == best_cost && bits(selected).cmp(bits(*best)).is_ge())
        {
            continue;
        }
        if coverage.separates(selected, budget)? {
            *best = selected;
        }
    }
    budget.charge(0)
}

fn cost(family: &Family, mask: u64) -> u64 {
    bits(mask)
        .map(|index| u64::from(family.fields[index].cost))
        .sum()
}
fn bits(mut mask: u64) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        if mask == 0 {
            return None;
        }
        let index = mask.trailing_zeros() as usize;
        mask &= mask - 1;
        Some(index)
    })
}

fn read_field(value: &Value) -> Result<Field> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("field must be an object"))?;
    let expected = BTreeSet::from(["name", "cost", "eligible"]);
    if object.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected {
        return Err(invalid("field requires exactly name, cost and eligible"));
    }
    let name = value["name"]
        .as_str()
        .filter(|name| !name.is_empty() && name.len() <= 256)
        .ok_or_else(|| invalid("field name must contain 1..256 bytes"))?;
    let cost = value["cost"]
        .as_u64()
        .and_then(|cost| u32::try_from(cost).ok())
        .filter(|cost| *cost != 0)
        .ok_or_else(|| invalid("field cost must be a positive u32 integer"))?;
    let eligible = value["eligible"]
        .as_bool()
        .ok_or_else(|| invalid("eligible must be a boolean"))?;
    Ok(Field {
        name: name.to_owned(),
        cost,
        eligible,
    })
}

fn failure(code: AgentErrorCode, detail: &str) -> AgentError {
    AgentError::new(code, detail)
}
fn invalid(detail: &str) -> AgentError {
    failure(AgentErrorCode::ResidualParse, detail)
}
fn limit(detail: &str) -> AgentError {
    failure(AgentErrorCode::ResidualLimit, detail)
}

#[cfg(test)]
mod memory_tests;
#[cfg(test)]
mod tests;
