# RW-040 loop-form mapping (slice 2, 2026-09-06)

Question (from slice 1): which CFG/backedge or recursion behavior expresses
the iteration the proposed bootstrap toolchain needs (codec traversal,
graph walks, lowering passes, build-driver closure resolution)?

Rule from the task: do not require both loops and recursion merely for
completeness. The bootstrap closure needs one supported bounded-iteration
form sufficient for bounded-structure walks. Both exist; either suffices,
and both are now demonstrated. No textual syntax was invented and no native
compiler helper was used: all workloads below run existing opcodes through
`lower_function` + `execute_function` under `CacheProfile::EXTENDED_V1`.

## Form A — CFG backedge loop (demonstrated)

- Legality: `docs/spec/CFG_VALIDATION_V1.md` — "Loops are legal backedges
  and are never rejected merely for being cycles"; "bounded handling of
  legal loops"; "cycles cannot recurse, hang, or become success after
  budget exhaustion."
- Mechanism: `Branch`/`CondBranch` terminators (`crates/sley-vm/src/execute.rs`
  `dispatch_terminator`); every terminator and instruction passes through
  `charge_action` fuel accounting, so a backedge iterates until its exit
  condition or deterministic `ResourceLimit` termination. No visited-set:
  nothing structural forbids the backedge.
- Workload: `cond-drain-loop` (`conformance/vm-extended/v1/accepted.json`,
  bytecode_sha256 `3e080650…`, 8 instructions). Four blocks: entry
  cell-threads the input map (`CellNew`), loop block probes one key
  (`CellGet` + `MapContains`), body block removes it (`MapRemove` +
  `CellSet`) and branches back, done block returns the drained map.
  Input `{7:"small"}`, key `7` → `Success({})`, backedge taken once.
- State threading without block parameters: `LocalCell` (E5) carries loop
  state across the backedge. Block parameters (`TargetEdge.arguments`)
  exist as a second mechanism, unexercised here and not needed for the map.

## Form B — bounded recursion (demonstrated, slice 1 retained)

- Mechanism: `call_direct` (opcode 112, E6) with an explicit call stack;
  1 fuel + 1 instruction per call.
- Bound (exact): 256 live frames including the entry; the frame that would
  make 257 live is refused (`saturating_add(1) > MAX_CALL_DEPTH` in
  `execute.rs`, pinned by `check_vm_extended_opcode_profile.py`).
- Workload: `call-direct-depth-ceiling` (255-link chain + entry = 256 live
  frames, `ResourceKind::CallDepth` on overflow).

## What the bootstrap closure needs (for RW-050)

- Iteration over bounded structures (maps/vectors/records): Form A
  (backedge + cell state) and Form B (recursion within the 256-frame
  ceiling) both suffice. RW-050 may freeze either as the canonical
  iteration form; requiring both is explicitly out of scope.
- Termination argument for either form: fuel bounds (`max_fuel` 10000 in
  the audit limits; profile limits frozen by RW-050), plus the call-depth
  ceiling for Form B.
- Deconstruction (consuming `Option`/`Result` lookups mid-walk): the
  designed form is CFG-level — `CondBranch` on `MapContains`/equality, or
  exhaustive `VariantSwitch` — not a straight-line unwrap op (none exists
  in EXTENDED_V1). `cond-drain-loop` exercises the `CondBranch` shape;
  `VariantSwitch` carries `CasePayload` for the selected variant
  (Ariadne round 1 INFO, confirmed against the contract), so payload
  delivery to case edges is specified; an accepted EXTENDED_V1
  `VariantSwitch` vector remains unexercised (recorded limitation, not a
  blocker: the loop workload proves the pattern with `CondBranch`).
- Profile limits RW-050 must freeze and carry: call-depth ceiling 256,
  fuel/instruction budgets, cell count cap 1,048,576, no E7 test/effect
  ops, no unwrap-op dependence.

## Codec note (Ariadne round 1 HIGH, see G-10)

Byte ORDERING (E1 order predicates over whole `Bytes` values) is
demonstrated by `bytes-less-than`/`text-less-than`, but ordering is not
access: no landed opcode takes apart or builds byte sequences, so an SCB
decode traversal or image-assembly emission has no executable workload yet.
That missing capability is gap G-10 (RW-050 §18 admission), not a defect in
any demonstrated form above.

## Not claimed

- No multi-key drain, no nested-loop, and no block-parameter threading
  workload was executed. Those are unexercised combinations of demonstrated
  forms, not missing capabilities; RW-050 may add workloads without new
  semantics if it needs them.
- No infinite loop was executed (constructing one would only demonstrate
  fuel exhaustion, a specified `ResourceLimit`, not a defect).
