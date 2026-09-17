# RW-080 §1.3 lowerer slice 16: simple bytecode-block composition

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley lowering
algorithm under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

`simple_block_lowerer` composes the ordered mixed-operation entry with the
return/branch/conditional/trap terminator entry. Its success value is the
first Sley-produced model aligned with the native `BytecodeBlock` boundary:

```text
(slot,
 parameter_registers,
 ordered_instructions,
 lowered_terminator,
 reachability)
```

The result also carries the final dense register frontier. Sley lowers every
operation first, forwards its first rejection unchanged, then passes the
derived frontier into terminator validation. A valid terminator is attached
only after its register and block targets pass. The admitted closure contains
the mixed dispatcher, both operation-family lowerers and validators, the
simple terminator lowerer and its validators, and the block composer.

## Native parity and negative corpus

The native reference supplies one Boolean instruction plus return, branch,
conditional branch, and trap terminators. For every terminator kind the Sley
block matches native slot 0, ordered instruction fields, terminator fields,
required reachability, and frontier 3. Repeated executions are identical. An
empty explicitly-unreachable block preserves parameter registers `[0, 1]`, a
return of register 1, and frontier 2.

An invalid operation register and invalid return register both map to
`VM_LOWER_LOCAL_REFERENCE_INVALID`; the invalid operation wins when both are
present, proving operation-before-terminator composition. All twenty-three
tests in `rw080_lower_scaffold` pass; focused Clippy with warnings denied is
clean.

## Explicit remainder

This block model accepts already-assigned parameter registers and covers the
four simple terminators. It does not allocate register types or block
parameters, compose multiple blocks, attach the built-in variant-switch
model, form the function/callee table, emit SLEYBC02, or assemble an execution
package. Those remain RW-110 construction layers. The driver remains RW-120
work. RW-080 and R2 stay provisional pending the recorded independent
acceptance debt.
