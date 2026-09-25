# AF1 frames (reference)

An AF1 frame is one JSON object. It is data, not source text: the tool
compiles it into one mutation candidate and the kernel never sees AF1.

    {"af1": 1,
     "types":  [type...],        create or update type definitions
     "consts": [const...],       create or update named constants
     "fns":    [function...],    define whole functions (create or replace)
     "patch":  [patch...],       restate some blocks of existing functions
     "edit":   [edit...],        replace single operations
     "tests":  [test...],        create or update TestCases
     "delete": ["name", ...],    delete top-level entities
     "namespace": "name"}        namespace for new entities (default: the only one)

Every key is optional except "af1". Names are `[A-Za-z_][A-Za-z0-9_-]*`.
Errors name the frame position as a JSON pointer, e.g. `/fns/0/blocks/2/ops/1`.

## types

    {"name": "Shape", "variant": ["Empty", ["Circle", "i64"], ["Rect", "(i64,i64)"]]}
    {"name": "Point", "record": [["x", "i64"], ["y", "i64"]]}

A variant case is "Name" or ["Name", payload type]. Record fields are
[name, type]. Existing members keep their identities by name.

## consts

    {"name": "limit", "type": "i64", "value": 10000}

## fns

    {"fn": "name", "params": [["a", "i64"], ...], "returns": "type",
     "blocks": [block, ...], "entry": "block name (default: the first)",
     "visibility": "exported|private|package|workspace"}

    block: {"name": "b", "params": [["x", "i64"]], "ops": [op, ...],
            "term": terminator, "unreachable": false}

Redefining an existing function matches parameters, blocks, block
parameters and operations by name. Matched entities keep their identities.
The ones you no longer name are deleted.

## patch and edit

    {"fn": "name", "blocks": {"b": block-without-name, "old": null}, "params": [...], "returns": "type"}
    {"fn": "name", "replace_op": "block.op", "with": ["opcode", args...]}

Unmentioned blocks are kept. A new block name adds a block, and `null`
deletes one. `edit` restates one operation and keeps its name and position.

## operations

    ["name", "opcode", immediate?, operand, ...]
    {"name": "n", "op": "opcode", "args": [immediate?, operand, ...], "type": "T"}

The immediate comes first for these opcodes:

    const <constant name | literal>     call <function>     fnref <function>
    variant <Type.Case> [payload]       variant_get <Type.Case> v
    record <Type> fields...             field <Type.field> v
    tuple_get <index> t                 global <name>

Result types are derived from operands and uses. The object form's "type"
is needed only when nothing determines the type (for example `none` or an
empty `vec` that is never returned or passed on).

Operands: `x` (a parameter or a value of this block), `b.x` (value x of
block b, which must dominate the use), `x#1` (result 1 of x).

## terminators

    ["return", v]
    ["br", target, arg...]
    ["cond", c, target, target]          target: "b" or ["b", arg...]
    ["switch", v, [key, block, arg...], ...]
    ["trap"] / ["trap", "unreachable"]

Switch keys are Ok, Err, Some, None, or a case name. Cases may be listed in
any order and must cover every case. `$` is the case payload. The target
block binds it as a parameter. A case may also be written
`[key, ["block", arg...]]`, the target form `cond` uses.

Operands name function parameters, this block's parameters, and results.
`b.x` is result x of block b, which must dominate the use. Block parameters
are visible only in their own block: pass them on as edge arguments.

## raw operations

`try` also accepts a JSON array of raw mutation operations
(`{"class": "CreateEntity"|"ReplaceEntityVersion"|"DeleteEntityBinding",
"kind": n, "target": id, "key": "k", "payload": body}`). Identities may be
64 hex, local names, or "@k" for a create in the same list.
