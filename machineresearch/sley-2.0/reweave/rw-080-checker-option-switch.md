# RW-080 §1.2 checker slice 2: Sley-owned Option-switch judgment

Status: PROVISIONAL C0 CONSTRUCTION. This is an executable Sley checker slice
under the operator development override. It is not RW-100 completion, C1,
self-hosting evidence, or runtime authority.

## Scope and behavior

`option_switch_cfg_checker` executes as an admitted Sley program. Runtime facts
describe the switch-specific projection of a three-block `Option<Bool>` CFG:
target resolution, selector type, case count, the two built-in case tags,
whether the payload-free `None` arm incorrectly consumes a payload, and target
argument type agreement. The image owns the native first-failure order and
returns the frozen S20-220 code or the exact compact `CfgReport` quantities.

This slice exercises:

- `CFG_TARGET_INVALID` (`22006`);
- `CFG_TARGET_ARGUMENTS` (`22007`);
- `CFG_SWITCH_TYPE` (`22010`);
- `CFG_SWITCH_CASES` (`22011`); and
- `CFG_SWITCH_PAYLOAD` (`22012`).

The target-inventory failure is checked first because native
`GraphIndex::build` resolves successor targets before value and terminator
judgment. The remaining checks follow `validate_switch`: selector type, exact
canonical cases, then each case's payload and argument typing. The accepted
projection returns three reachable blocks, two edges, and eight charged
dominator word operations.

## Native parity and negative corpus

`native_option_switch_cfg` independently constructs a typed `Option<Bool>`
function and invokes `sley_check::cfg::validate_function_graph`. The success
test compares all report fields and repeats Sley execution for determinism. The
negative matrix covers each single fault and adjacent multi-fault combinations
to pin target → selector → cases → payload → argument precedence. All six tests
in `rw080_checker_scaffold` pass, and focused Clippy with warnings denied is
clean.

## Explicit remainder

This is a compact admitted-fact projection. It does not decode a closure,
iterate arbitrary function or block inventories, validate named-variant member
sets, type definitions, general operation signatures, effects, contracts, or
construct the mandatory test plan. RW-100 still owns the complete type → CFG →
effect checker and semantic corpus. RW-080 and R2 stay provisional pending the
recorded independent acceptance debt.
