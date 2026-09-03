# S20-700 extended VM fuzz lane campaign (2026-09-03)

Package: adversarial coverage of the S20-260/S20-270 extended opcode families,
dependency the landed E1 through E6 slices, phase M5. Owner of record Vulcan;
executed by the integrator with every Council lane unavailable (ADR-0026). No
new surface is claimed: the target still refuses a raw bytecode entry point and
the slice stays scoped to typed VM inputs.

## What was missing

`fuzz/targets/vm_canonical_inputs.rs` exercised the extended profile only
through the Boolean fixtures of slice E1: the cross-profile lane asserted that
`RESTRICTED_V1` and `EXTENDED_V1` terminate identically on programs whose
opcodes both profiles implement. Families E2 through E6, which carry the
checked arithmetic, float canonicalization, aggregate, cell, and call-stack
semantics, had no fuzz coverage at all.

## Mechanics

A second lane runs when the selector byte is a multiple of three. It builds one
of seven extended fixtures, one per family plus a constant reference, and
asserts three properties for each:

1. execution under `EXTENDED_V1` is deterministic across two runs with the same
   fuzzer-derived inputs and limits;
2. the same fixture is refused by `RESTRICTED_V1`, so no extended opcode leaks
   into the frozen profile;
3. a completed outcome re-derives its observation identity exactly.

| Fixture | Family | Surface |
|---|---|---|
| `IntAddChecked` over `SInt32` | E2 | overflow as a value failure |
| `IntDivChecked` over `SInt64` | E2 | divide-by-zero and signed-minimum division |
| `FloatAdd` over `F64` | E3 | rounding and canonical results |
| `MapNew` over two pairs | E4 | canonical key order and duplicate-key failure values |
| `CellNew` then `CellGet` | E5 | per-execution cells that never escape |
| `ConstantRef` | E1 | constant projection under the extended profile |
| `CallDirect` to a zero-parameter callee | E6 | the explicit call stack with a real callee inventory |

The seed corpus gained one seed per restricted fixture and family pair in both
input lanes, growing from 625 to 751 seeds; the smoke now executes 752 runs.

## Result

`make vm-persistent-fuzz-smoke` passes with no finding. The slice checker,
runner, and machine summary pin the seven-fixture count, the family list, the
new seed count, and the three lane assertions, so removing a lane fails
`make quick`.

## Open questions for the Council

1. Vulcan: the lane asserts cross-profile refusal by executing under
   `RESTRICTED_V1` and requiring an error. Should it also pin the exact
   `VM_LOWER_OPCODE_UNSUPPORTED` code, or is that too tight for a fuzz lane?
2. Nabu: the E6 fixture keeps its callee's blocks and operations in the same
   inventories as the entry, which the lowerer narrows per function. Should the
   fixture instead carry a separate callee inventory to exercise the narrowing
   itself?
3. Ariadne: E7 opcodes have no lane because no owner implements them; is a
   negative lane asserting their refusal worth adding before their owners land?

## Validation

Landed at `9acf87d` (E2, E3, E5 lanes) and `2d1b805` (E4 map and E6 call).
Tier 1 `make quick` passed at each commit. Tier 2 at `9acf87d`: `make core`
exit 0 (14 s), `make conformance` exit 0 (11 s), `make adversarial` exit 0
(8 s), `make fuzz-smoke` exit 0, `make vm-persistent-fuzz-smoke` exit 0. The
final smoke at `2d1b805` ran 752 executions with 751 seeds and no finding.
