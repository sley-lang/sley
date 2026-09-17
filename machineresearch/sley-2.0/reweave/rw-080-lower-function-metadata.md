# RW-080 §1.3 lowerer slice 19: lossless function metadata

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley lowering
algorithm under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

`complete_function_lowerer` now carries the three native function-body fields
that cannot be reconstructed from dense block facts: the exact 32-byte
function identity, the canonical encoded type of every dense register, and
the canonical encoded result type. Its model is:

```text
(function_identity_bytes,
 function_parameter_registers,
 register_type_encodings,
 result_type_encoding,
 entry_slot,
 canonical_block_map,
 register_count)
```

The identity crosses the frozen B2V1 bridge and must contain exactly 32
octets. A second Sley helper counts the register-type vector with checked
UInt64 and UInt32 loop state; its count must equal the final frontier derived
by function-parameter, block-parameter, and operation-result allocation. Too
few or too many type rows therefore fail closed. The model preserves each
type byte string and the result-type byte string without host interpretation.
Canonical type formation and semantic validity remain codec/checker duties;
this lowering boundary owns lossless carriage and register cardinality.

The test-side independent projection covers all frozen `TypeExpr` forms and
reproduces the SLEYBC02 type-body framing: big-endian tags and widths,
length-prefixed children, entity framing, function effects, type parameters,
and built-in failure tags. The current native multi-block fixture exercises
Boolean and `Option<Bool>` register types and a Boolean result.

## Native parity and negative corpus

The enriched Sley model matches the native three-block function's identity,
eight ordered register-type encodings, result type, parameter registers,
entry slot, complete blocks, and frontier. A 31-byte identity refuses with
`VM_LOWER_LOCAL_REFERENCE_INVALID`. Removing one register type or appending an
extra type returns the same typed refusal after block construction. All
twenty-eight tests in `rw080_lower_scaffold` pass; focused Clippy with
warnings denied is clean.

## Explicit remainder

Operation immediates still carry their bounded projection rather than exact
identity/member/type-argument payload bytes. The Sley closure has not yet
emitted function-body bytes, built the transitive callee table, wrapped the
SLEYBC02 header, or assembled an execution package. Those remain RW-110
construction layers. The driver remains RW-120 work. RW-080 and R2 stay
provisional pending the recorded independent acceptance debt.
