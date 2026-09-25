# RW-080 §1.2 checker slice 1: Sley-owned Boolean CFG judgment

Status: PROVISIONAL C0 CONSTRUCTION. This advances the retained checker
scaffold with one real bounded CFG algorithm under the existing operator
development override. It is not RW-100 completion, C1, a self-hosting claim,
or runtime authority.

## Scope and behavior

`single_bool_cfg_checker` executes as an admitted Sley program. Runtime inputs
describe a compact projection of a two-block, one-operation Boolean function:
the declared block count, whether the entry resolves, operation count and
ordinal, declared reachability, operand resolution, return result index, and
return-type agreement. These facts arrive after image admission, so the image
cannot contain the selected verdict.

Sley checks those facts in the native S20-220 first-failure order and returns
`Result<Tuple<UInt32, UInt32, UInt64>, UInt32>`. Success contains the exact
native `CfgReport` quantities for the fixture: one reachable block, zero edges,
and zero dominator word operations. Failure contains the frozen native code.
This slice exercises:

- `GRAPH_INVENTORY_MISMATCH` (`22001`),
- `GRAPH_ORDINAL_MISMATCH` (`22003`),
- `CFG_ENTRY_INVALID` (`22005`),
- `CFG_RETURN_TYPE` (`22008`),
- `CFG_VALUE_UNRESOLVED` (`22013`),
- `CFG_RESULT_INDEX` (`22014`), and
- `CFG_REACHABILITY` (`22017`).

All paths return typed values. No native adapter import, callback, semantic
host service, or prebaked verdict is used.

## Reference comparison

The test independently constructs the corresponding native `FunctionGraph`,
parameter, blocks, and operation from the same runtime facts and invokes
`sley_check::cfg::validate_function_graph`. The accepted case compares all
three report quantities and runs twice for determinism. The rejection matrix
compares every single fault plus adjacent multi-fault cases, preserving the
native precedence from inventory through return type.

Source and gate: `crates/sley-vm/tests/rw080_checker_scaffold.rs`, tests
`checker_single_boolean_cfg_matches_native_report` and
`checker_single_boolean_cfg_preserves_native_first_failure_codes`. The two
retained scaffold tests remain green.

## Explicit remainder

This slice does not decode a complete closure, iterate arbitrary function and
block inventories, validate type definitions or opcode signatures, compute
general successor/dominator sets, judge effects, construct the selected test
plan, or render diagnostics. RW-100 still requires the complete type → CFG →
effect checker and its semantic corpus. RW-080 and R2 statuses remain unchanged
pending the recorded independent acceptance debt.
