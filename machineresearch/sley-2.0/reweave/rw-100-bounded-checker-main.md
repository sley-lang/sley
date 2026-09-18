# RW-100 bounded checker composition — provisional record

Status: PROVISIONAL C0 CONSTRUCTION (2026-09-18, operator development
override `rw075_correction.operator_override_2026_09_07`). This slice composes
the seven existing executable checker algorithms into one admitted Sley
closure. It is a bounded checker profile, not the complete
`check_program(program_closure)` algorithm, a complete TestPlan, canonical
state root `S`, or an R2 readiness claim.

## Composition

`crates/sley-vm/tests/rw080_checker_program/composed.rs` constructs the child
programs for:

1. the two-block Boolean CFG projection;
2. the `Option<Bool>` switch projection;
3. ordered operation-inventory validation;
4. unary type-chain validation;
5. one-definition effect closure;
6. sorted effect-set inventory validation; and
7. two-function direct-call effect propagation.

Each child begins with fixture-local identities. The composition rewrites all
function, parameter, block, operation, and constant identities into a
collision-free child namespace and rewrites every ownership, graph, operand,
terminator, type, constant, and immediate reference through the same map.
The wrapper then calls the selected child with Sley `CallDirect` and switches
on its typed Result.

The bounded entry takes a selector plus the complete typed input tuple for
all seven projections. Only the selected tuple is passed to its child. Its
result is normalized as:

`Result<(selector: UInt8, metric64: UInt64, metric32a: UInt32,
metric32b: UInt32, work64: UInt64), UInt32>`.

CFG reports populate the two UInt32 metrics and final UInt64 work field; the
type-chain report populates `metric64`; effect reports populate all four
metric fields. Child errors retain their exact frozen S20-210, S20-220, or
S20-230 numeric value. Selector values outside `0..=6` return the bounded
profile code `UInt32::MAX`; that code is not presented as a language semantic
error.

## Evidence

The merged admitted closure contains:

- functions: 8 (one wrapper plus seven algorithms);
- parameters: 209;
- blocks: 181;
- operations: 346;
- constants: 85.

The positive test executes all seven selectors and compares the normalized
wrapper result against direct execution of the selected rebased child. The
negative test repeats that comparison for representative CFG, type, and
effect failures, proving exact error forwarding, and separately checks the
unknown-selector refusal. Admission uses Bootstrap Profile V2 and the normal
admit, approve, execute boundary.

Validation commands:

- `cargo test -p sley-vm --test rw080_checker_scaffold bounded_checker_main -- --nocapture`
- `cargo test -p sley-vm --test rw080_checker_scaffold`
- `cargo clippy -p sley-vm --test rw080_checker_scaffold -- -D warnings`
- `cargo fmt --all -- --check`

## Open RW-100 surface

The inputs remain admitted fact projections. This composition does not decode
the canonical object closure, traverse arbitrary function/block/type
inventories, validate the full opcode signature table, compute general CFG
dominance, close arbitrary cyclic multi-effect call graphs, validate adapters,
capabilities, contracts, or resource ceilings, or construct the mandatory
test plan. Those surfaces must be implemented and compared with the native
checker corpus before RW-100 can pass. Canonical object materialization and a
retained checker component root also remain open.
