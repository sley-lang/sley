# RW-080 §1.3 lowerer slice 14: unified immediate-free operations

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley lowering
algorithm under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

`immediate_free_operation_lowerer` replaces the separate variadic entry with
one execution-time operand-vector surface for every immediate-free target
operation modeled by the construction corpus. It covers all 35
immediate-free opcodes admitted by `BOOTSTRAP_PROFILE_2` plus the six floating
target opcodes retained for full-profile native parity: 41 target tags total.

Sley recognizes the opcode before inspecting arity. It then enforces exact
zero-, one-, two-, or three-operand shapes where required; permits arbitrary
arity for tuple and vector construction; and proves even arity for ordered-map
construction. A reusable Sley callee walks every operand and requires a prior
dense register. The caller emits the compact instruction model, allocates one
result register, and advances the frontier with checked arithmetic.

The implementation closure itself remains inside `BOOTSTRAP_PROFILE_2`; the
six floating tags are data interpreted by the lowerer and do not add floating
instructions to the bootstrap closure.

## Native parity and negative corpus

Native reference fixtures cover Boolean logic, equality and ordering, all
checked integer operations, all six floating operations, option/result
construction, the complete local-cell chain, value hashing, tuple/vector
construction and access, and ordered-map construction/access/update. The Sley
result matches the native opcode, complete operand vector, dense result
register, and next frontier for all 41 tags. Repeated executions are
identical.

Negative cases pin opcode-before-signature behavior, fixed arity, MapNew
pair arity, late invalid register detection, and frontier overflow. All
nineteen tests in `rw080_lower_scaffold` pass; focused Clippy with warnings
denied is clean.

## Explicit remainder

Immediate-bearing operations still use their dedicated entry point. The next
composition step must dispatch both entry points from one ordered mixed
inventory and then combine operations with register types, blocks,
terminators, and the callee table. Full identity payloads, SLEYBC02 emission,
package assembly, and the driver remain RW-110/RW-120 work. RW-080 and R2 stay
provisional pending the recorded independent acceptance debt.
