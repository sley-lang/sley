# RW-080 §1.2 checker slice 3: Sley-owned operation inventory

Status: PROVISIONAL C0 CONSTRUCTION. This is an executable Sley checker slice
under the operator development override. It is not RW-100 completion, C1,
self-hosting evidence, or runtime authority.

## Scope and behavior

`ordered_operation_inventory_checker` accepts an arbitrary runtime vector of
compact operation facts plus a function-parameter count. Each row contains the
declared operation ordinal, reference kind, and referenced inventory index.
Reference kind 1 names a function parameter; kind 2 names an operation result.

The Sley program makes two complete CFG-backedge passes over the runtime
vector. The first pass validates every ordinal against its semantic position.
Only after the whole ordinal inventory succeeds does the second pass resolve
uses in operation order. Parameter indexes must fall below the supplied
parameter count. Operation indexes must exist and must precede the current
operation. This ordering preserves native graph-construction precedence: a
late ordinal fault wins over an earlier unresolved use.

The slice returns the exact compact `CfgReport` for the represented one-block
function or one of:

- `GRAPH_ORDINAL_MISMATCH` (`22003`);
- `CFG_VALUE_UNRESOLVED` (`22013`);
- `CFG_USE_BEFORE_DEFINITION` (`22015`); or
- `CFG_RESOURCE_LIMIT` (`22020`) for checked loop-index overflow.

An absent row after a proven in-bounds `VectorGet` traps `InternalInvariant`
and is unreachable for a conforming VM.

## Native parity and negative corpus

`native_operation_inventory_cfg` builds the same runtime-selected parameter
and operation inventories and invokes
`sley_check::cfg::validate_function_graph`. Successful non-empty and empty
inventories match the native report, and repeated execution is deterministic.
The negative corpus covers unknown parameter and operation identities, unknown
reference kinds, forward and self references, late failures, and the key
cross-pass case: an unresolved first row plus a bad later ordinal must return
the ordinal error. All eight tests in `rw080_checker_scaffold` pass; focused
Clippy with warnings denied is clean.

## Explicit remainder

Rows are a compact admitted projection rather than decoded `Operation`
objects. They represent one operand and one Boolean result per operation; full
opcode signatures, multiple operands/results, cross-block dominance,
unreachable uses, arbitrary block/function inventories, type definitions,
effects, contracts, and mandatory test planning remain RW-100 work. RW-080 and
R2 stay provisional pending the recorded independent acceptance debt.
