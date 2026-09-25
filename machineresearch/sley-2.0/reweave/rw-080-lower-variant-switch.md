# RW-080 §1.3 lowerer slice 4: Sley-owned built-in variant switches

Status: PROVISIONAL C0 CONSTRUCTION. This is an executable Sley lowering slice
under the operator development override. It is not RW-110 completion, C1,
self-hosting evidence, or runtime authority.

## Scope and behavior

`builtin_variant_switch_lowerer` consumes a selector register, a runtime vector
of built-in case facts, the dense-register count, and the block-slot count. A
case fact contains the frozen built-in case tag, target slot, and a runtime
vector of argument facts. Argument tag 1 carries an ordinary register; tag 2
marks the selected case payload. The returned typed model preserves selector,
case order, targets, argument order, and payload markers exactly.

The Sley entry walks every case using `VectorLen`, `VectorGet`, and a CFG
backedge. Each case invokes a second Sley function through `CallDirect`; that
helper independently walks the complete argument vector. The success path is
therefore unavailable until all nested runtime facts have been inspected.
Built-in case tags are restricted to the frozen `None`, `Some`, `Ok`, and `Err`
range. The program validates selector and ordinary argument registers against
the supplied register inventory and every target against the supplied block
inventory.

Malformed case or argument tags return `VM_LOWER_SIGNATURE_MISMATCH` (`26002`).
Invalid selector, target, or ordinary-register facts return
`VM_LOWER_LOCAL_REFERENCE_INVALID` (`26004`). Checked-index overflow returns
`VM_LOWER_RESOURCE_LIMIT` (`26006`). An absent value after a proven in-bounds
`VectorGet` traps `InternalInvariant` and is unreachable for a conforming VM.

Case exhaustiveness, canonical key order, payload typing, and target-parameter
typing remain checker judgments. The native lowerer has the same separation:
it copies an already checked case inventory and resolves dense references.

## Native parity and negative corpus

`native_builtin_switch_terminator` builds a valid `Option<Bool>` switch and
obtains its dense model from `sley_vm::lower_function`. The Sley execution is
compared field for field with that native result and repeated to demonstrate
determinism. The negative corpus covers an invalid selector, a late invalid
target, a late invalid ordinary argument, both boundaries around the built-in
case-tag range, and an unknown argument tag. The late failures demonstrate
that neither the outer nor nested loop accepts a checked prefix.

The complete `rw080_lower_scaffold` target has thirteen passing tests. Focused
Clippy with warnings denied, formatting, diff checks, and the anti-goal gate are
run at the landing checkpoint.

## Explicit remainder

Named-member case keys are not represented by this bounded built-in slice.
The program consumes admitted compact facts rather than decoded checked SSMC
closure objects. Complete block/function inventory traversal, register-type
construction, callee-table construction, `SLEYBC02` emission,
`EXEC_PACKAGE_V2` assembly, and `BuildError` remain RW-110 work. RW-080 and R2
stay provisional pending the recorded independent acceptance debt.
