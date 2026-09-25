# RW-080 §1.3 lowerer slice 18: ordered function-block composition

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley lowering
algorithm under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

Successor note: `rw-080-lower-function-metadata.md` attaches lossless function
identity, register-type, and result-type encodings to this model.

## Scope and behavior

`complete_function_lowerer` walks an arbitrary runtime vector of compact block
facts and calls `complete_block_lowerer` for every row. It reproduces the
native allocator's semantic order: function parameters first, followed for
each block by that block's parameters and then its operation results. A Sley
dense-range validator checks every supplied parameter register against the
current frontier and returns the advanced frontier with checked arithmetic.

Each block slot must equal its vector ordinal. Accepted block models are
inserted into an `OrderedMap<UInt32, CompleteBlockModel>` keyed by slot. This
retains canonical semantic order without admitting a second monomorphized
PSH1 row: the closure continues to carry exactly the one instruction-vector
push import already needed by its operation lowerer. The final model contains
function parameter registers, entry slot, the canonical block map, and total
register count. The declared block count must equal the traversed inventory,
and the entry slot must be in range.

The program preserves first-failure order across function parameters, block
slots, block parameters, operations, and terminators. Missing values after a
proven vector bound and failure of empty-map construction trap as internal
invariants. Dense-register overflow returns `VM_LOWER_RESOURCE_LIMIT`; all
ordering/range drift returns `VM_LOWER_LOCAL_REFERENCE_INVALID`.

## Native parity and negative corpus

The native reference is a three-block `Option<Bool>` function. It allocates
function parameters 0 and 1, an option result at 2, a `None` block parameter
and result at 3 and 4, and two `Some` block parameters plus a Boolean result
at 5 through 7. The entry uses a two-case built-in variant switch; both target
blocks return locally computed values. Sley matches every native block model,
slot, parameter register, instruction, terminator, reachability tag, entry
slot, function parameter vector, and final frontier 8. Repeated execution is
identical.

Negative cases cover an out-of-order slot, a gap in a later parameter range,
a declared block-count mismatch, and an out-of-range entry. A deliberately
wrong first-block immediate wins over the later parameter defect, proving
ordered failure propagation. All twenty-seven tests in
`rw080_lower_scaffold` pass; focused Clippy with warnings denied is clean.

## Explicit remainder

The successor attaches exact function identity, register types, and result
type. Operation immediates still use the bounded two-word projection recorded
by earlier slices. The Sley closure has not yet built the transitive callee
table, emitted SLEYBC02, or assembled an execution package. Those remain
RW-110 construction layers. The driver remains RW-120 work. RW-080 and R2
stay provisional pending the recorded independent acceptance debt.
