# RW-080 §1.3 lowerer slice 17: complete bytecode-block composition

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley lowering
algorithm under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

`complete_block_lowerer` composes the ordered mixed-operation entry with all
five frozen bytecode terminator families: return, branch, conditional branch,
built-in variant switch, and trap. Its success value is a closed block model:

```text
(slot,
 parameter_registers,
 ordered_instructions,
 (terminator_kind, simple_terminator, builtin_switch),
 reachability)
```

The result also carries the final dense-register frontier. Sley lowers the
complete instruction inventory before dispatching the terminator kind, so an
instruction rejection always precedes a terminator rejection. The simple and
switch paths call their existing Sley validators and normalize into the same
tuple without discarding case order, edge targets, edge arguments, or payload
markers.

The seed assembler rebases the independently built variant-switch graph into
a disjoint artifact and function namespace. It removes the superseded simple
block wrapper and all of that wrapper's owned graph artifacts before adding
the new entry. The resulting inventory is therefore exactly its transitive
call closure, as required by V2 bootstrap admission; shared operation and
terminator lowerers remain intact.

## Native parity and negative corpus

Native reference blocks cover Boolean instructions with return, branch,
conditional branch, and trap terminators, plus an `OptionSome` instruction
with the complete `Option` built-in variant switch. Sley matches the native
slot, parameter registers, ordered instruction fields, terminator fields,
reachability, and final frontier for every family. Repeated variant-switch
execution is identical.

The negative corpus combines an invalid operation register with an invalid
late switch-edge register and proves that the operation failure wins. With a
valid operation inventory, the late edge fails with the same
`VM_LOWER_LOCAL_REFERENCE_INVALID` code. All twenty-five tests in
`rw080_lower_scaffold` pass; focused Clippy with warnings denied is clean.

## Explicit remainder

The block model consumes already-assigned parameter registers and compact
decoded operation/terminator facts. It does not allocate register types or
block parameters, compose an ordered multi-block function, build the callee
table, emit SLEYBC02, or assemble an execution package. Those remain RW-110
construction layers. The driver remains RW-120 work. RW-080 and R2 stay
provisional pending the recorded independent acceptance debt.
