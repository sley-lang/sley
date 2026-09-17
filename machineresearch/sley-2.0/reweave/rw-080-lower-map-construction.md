# RW-080 §1.3 lowerer slice 11: ordered-map construction

Status: PROVISIONAL C0 CONSTRUCTION. This is a real Sley lowering algorithm
under the operator development override. It is not RW-110 completion, C1,
self-hosting evidence, or runtime authority.

Successor note: `rw-080-lower-bootstrap-immediates.md` adds the frozen
bootstrap immediate-bearing opcode family and the admitted bridge operation.

## Scope and behavior

`variadic_operation_lowerer` now supports `MapNew`. Sley obtains the complete
runtime operand-vector length, computes its checked remainder modulo two, and
requires zero before invoking the complete register-vector validator. This
preserves the native key/value-pair shape rule for empty or arbitrary even
inventories and rejects an odd inventory as `VM_LOWER_SIGNATURE_MISMATCH`
(`26002`). The checked remainder's impossible arithmetic failure maps to
`VM_LOWER_RESOURCE_LIMIT` (`26006`).

After validation, the existing variadic emission path preserves every ordered
key/value register, derives one dense result register, and advances the
frontier. The Sley lowering paths now model forty-one distinct opcode tags.

## Native parity and negative corpus

The native fixture lowers two `Bool -> UInt(32)` key/value pairs and declares
the exact `Result<OrderedMap<Bool, UInt(32)>, DuplicateKey>` result. The Sley
model matches the native opcode, four operand registers, result register, and
frontier. A three-register MapNew case pins the even-arity rejection before
register validation.

All fifteen tests in `rw080_lower_scaffold` pass; focused Clippy with warnings
denied is clean.

## Explicit remainder

The row still relies on prior type judgment for alternating key/value types
and key admissibility. Projections and named record/variant immediates,
constant/global/function references, direct calls, contracts/effects/adapters/
capabilities, arbitrary decoded SSMC closures, SLEYBC02 emission, package
assembly, and the build driver remain RW-110 work. RW-080 and R2 stay
provisional pending the recorded independent acceptance debt.
