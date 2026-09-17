# RW-080 §1.3 lowerer slice 9: value constructors, local cells, and hashing

Status: PROVISIONAL C0 CONSTRUCTION. This is a real Sley lowering algorithm
under the operator development override. It is not RW-110 completion, C1,
self-hosting evidence, or runtime authority.

## Scope and behavior

`ordered_scalar_inventory_lowerer` now handles eight additional
immediate-free operations:

- `OptionSome` and zero-operand `OptionNone`;
- `ResultOk` and `ResultErr`;
- `CellNew`, `CellGet`, and `CellSet`; and
- `ValueHash`.

The Sley graph adds a real zero-arity signature and emission path for
`OptionNone`; it constructs an empty operand vector while still assigning one
dense result register. Unary constructors, cell construction/read, and hashing
use the existing complete one-reference path. `CellSet` uses the two-reference
path. Every operation appends an exact instruction model through `PSH1` and
advances the frontier with checked arithmetic. The loop covers thirty opcode
tags.

## Native parity

The native scalar fixture covers option/result constructors and hashing with
valid declared types. A separate three-instruction native cell fixture lowers
`CellNew(parameter 0)`, `CellSet(cell, parameter 1)`, and `CellGet(cell)` so
the local cell never crosses the function boundary. The Sley inventory starts
at the two-parameter frontier and reproduces native operands and result
registers `2`, `3`, and `4`, ending at frontier `5`. The zero-operand Option
case independently proves frontier `0` produces result register `0`.

All thirteen tests in `rw080_lower_scaffold` pass; focused Clippy with warnings
denied is clean.

## Explicit remainder

These compact rows still omit type and immediate objects and rely on prior
checker evidence. Aggregate/container operations, `FloatFma`, direct calls,
named record/variant immediates, contracts/effects/adapters/capabilities,
globals, function references, arbitrary decoded SSMC closures, SLEYBC02
emission, package assembly, and the build driver remain RW-110 work. RW-080
and R2 stay provisional pending the recorded independent acceptance debt.
