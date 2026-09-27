# Sley Agent Workbench v1

Status: revision 2 (2026-09-27). Revision 1 (2026-09-25) was the Sley
2.0.2 contract implementing `SLEY-2.0.2-BR` Track A (BR-01 through BR-12).
Revision 2 adds decision-only authoring: the AF1-X dialect and test tables
(sections 5.1 and 5.2), drafts and delta repair (section 12), focused and
AV1-X views (sections 4.1 and 4.2), the namespace opt-out, and authored
locators. ADR-0051 records the boundary decisions and ADR-0053 the
revision-2 decisions. The implementation is `crates/sley-agent`, binary
`sley-agent`.

The workbench is the interface Sley offers to the agents that author
programs. It is a separate binary over the kernel libraries. The thin
transport CLI (`SLEY_CLI_V1.md`, ADR-0035) is unchanged and remains a
transport endpoint. The workbench adds four things:

1. **AV1**: derived, output-only views of program entities under local names.
2. **AF1**: name-based JSON authoring frames, compiled client-side into the
   existing mutation candidate record (`CANDIDATE_RECORD_V1.md`).
3. **Decoded refusals**: the candidate result's decision, phase, source
   symbol and retryability, a checked-in remediation hint, and an
   explain-only locator.
4. **An advisory dev loop**: in-process execution of functions and TestCases
   through `sley_vm`, lowering each function once per program state.

The kernel alone judges candidates. The workbench adds no validation rule.
Its derived outputs (views, hints, locators, execution results) are never
admission evidence, never signed, never committed, and never parsed back by
any product component.

## 1. Workspace

A workspace is a directory holding `repo/` (a repository) or, before first
use, `base.pack` (an exchange pack). The workbench locates the workspace by
walking up from the current directory, or takes `--workspace <dir>`. On
first use it imports `base.pack` into an empty `repo/`. This is the only
write the workbench makes to `repo/` outside `commit`.

| Path | Owner | Content |
|---|---|---|
| `repo/` | kernel | the repository |
| `base.pack` | author | seed exchange pack |
| `names.json` | author | optional starter name map (section 3) |
| `.sley/names.json` | workbench | names of entities and members the workbench created |
| `.sley/candidates/cN.hex`, `cN.json` | workbench | stored candidate bytes and their summary, by handle |
| `.sley/submission` | workbench | the handle last submitted |
| `final_candidate.hex` | workbench | the submission: stored candidate bytes as lowercase hex |

The authoring principal is the policy root's only grant, or else the first
grant that allows `CreateEntity` and `ReplaceEntityVersion`.

## 2. Commands

```text
sley-agent view [name...] [--package] [--after <ref>] [--x] [--ids] [--types] [--limits]
sley-agent view --focus <name> [--after <ref>] [--x] [--ids] [--types] [--limits]
sley-agent find [text] [--kind fn|type|const|test|ns] [--after <ref>]
sley-agent try <frame|ops> [--on <ref>] [--no-test] [--all-tests] [--public <cases.json>] [--raw]
sley-agent submit [<ref>] [--untested]
sley-agent try <frame|ops> --on <draft>[@r<N>] [--rebase] [--verbose]
sley-agent fill <draft> <delta.json> --revision <N> [--rebase] [--no-test] [--all-tests] [--public <cases.json>] [--raw] [--verbose]
sley-agent import <cases.json> [--on <draft>[@r<N>]] [--only <name,...>] [--rebase] [--no-test] [--all-tests] [--public <cases.json>] [--raw] [--verbose]
sley-agent draft [<draft>[@r<N>]] [--obligations | --expanded | --input | --frame]
sley-agent submit <draft>[@r<N>] [--untested]
sley-agent status [--raw]
sley-agent call <fn> <arg-json>... [--on <ref>] [--batch <file|->] [--stats]
sley-agent test [<ref>] [--public <cases.json>]
sley-agent explain [<ref>]
sley-agent search <fn> --public <cases.json> [--from <handle|draft>[@r<N>]] [--max-neighbors <n>] [--max-millis <ms>]
sley-agent init [<dir>] [--seed <64-hex>]
sley-agent commit [<ref>]
sley-agent export <file.pack>
sley-agent help [guide|af1|afx|drafts|types|tests|opcodes|search|refusals]
```

A `<ref>` is a handle (`c3`), `latest`, a file holding stored candidate
hex, raw stored candidate hex, or a bare candidate record (which is framed
through the kernel). `--json` makes every command print one JSON object.
A flag given twice (`--json`, `--workspace`, or any flag of a command) is
refused with `AGENT_USAGE_INVALID`, never resolved by position.
Exit status 0 is success. Exit status 1 is a negative outcome: a refused
candidate, a failing test, or no submission. Exit status 2 is a workbench
refusal (section 9). No response prints record, stored or body hex: a
candidate is its handle, and its bytes stay under `.sley/`. `--raw` on
`try` and `status` adds the stored candidate hex (`stored:` in text,
`stored_hex` in JSON).

`try` compiles a frame (a JSON object with `"af1": 1`) or a raw operation
list (a JSON array), assembles one candidate over the accepted head, and
validates it in process with `validate_candidate_bytes`. It stores the
candidate under the next handle. When the candidate is Valid, `try` executes
the TestCases the kernel selected for it, plus the TestCases the frame
declares, plus the listed public cases. It prints one compact result: the
handle, the decision or the decoded refusal, and per-test results. A Valid
candidate that runs no TestCase says so (`tests: 0 ran`), as a test runner
reports running zero tests, and its `next:` line suggests adding tests
before submitting.

A handle made from an AF1 frame keeps that frame in its metadata.
`try --on <ref>` layers the new frame on the frame of `<ref>`, then compiles
the result against the head like any other frame. The layered frame is
the `frame.json` of the draft revision the `try` records (section 12);
refusal pointers refer to it, and `explain` names the same file later.
When `<ref>` is a candidate handle it is also written to
`.sley/layered.json`, which the next such `try` rewrites. The layering
rules:
- A `types`, `consts`, `fns` or named `tests` entry replaces the base entry
  of the same name; other entries are appended.
- A `patch` of a function the base defines applies to that definition: a
  block replaces the block of its name, `null` deletes it, and `params` and
  `returns` replace. A `patch` of a function the base patches merges with
  that patch. Otherwise it is appended.
- An `edit` replaces the operation in the base's definition or patch of
  that function, or else replaces a base edit of the same operation, or
  else is appended.
- `delete` entries are added, and `namespace` replaces.

A handle made from raw operations has no frame, and `try --on` refuses it
with `AGENT_USAGE_INVALID`; a reference that names no candidate is
`AGENT_HANDLE_UNKNOWN`.

The `next:` line of a Valid candidate proposes a layered follow-up. With no
TestCase, it proposes the tests; with a failing TestCase, the fix. A
kernel-refused candidate made from a frame gets a `fix:` line that proposes
the same. When the frame came from a file, a frame refusal ends by asking
for that file to be edited in place at the pointers rather than rewritten.

`submit` validates the referenced candidate against the current head again
and writes `final_candidate.hex` only when it is Valid. A Valid candidate
that creates, replaces or deletes part of a function while no TestCase in it
targets a function it changes (the kernel selects none) is refused with
`AGENT_SUBMISSION_REFUSED`, unless `--untested` is given; such a submission
then notes that no TestCase targets a function it changes. Without a
reference (or with `latest`), `submit` takes the draft revision recorded
last (section 12.6). Submissions
repeat: the last Valid submission wins. `status` renders the submission's affected
functions and TestCases in AV1 and runs its selected tests.

`commit` passes the candidate to the transaction engine
(`TransactionRepository::commit`), which validates it again. Candidates that
select TestCases are refused there until native test evidence (N5) exists.

`init` writes one trusted genesis into an empty directory: a workspace
identity derived from a genesis nonce (random, or `--seed`), the principal
of that identity, and a policy that grants it the workbench ceilings
(section 7). It then exports `base.pack`.

## 3. Local names

Every entity and member gets a local name. Names are non-canonical, never
identity inputs, and never enter canonical bytes. In order of preference:

1. the object's `label`, when it is a name (`[A-Za-z_][A-Za-z0-9_-]*`, at
   most 64 bytes; a hyphen is allowed because `.`, `#` and `$` are the only
   characters with a meaning inside a name reference);
2. the workspace name map (`names.json`, then `.sley/names.json`);
3. a positional name: `<kind>_<first 8 hex>` for top-level entities, `entry`
   or `b<i>` for blocks, `p<i>` for parameters, `v<i>` for block parameters,
   `op<i>` for operations, and `m<i>` for members.

Leaf names are unique within their scope. Function parameters and blocks are
scoped to their function and share it: a block cannot take a parameter's
name. Block parameters and operations are scoped to their block; inside a
block, its own parameters and results are found before the function's
parameters. Qualified names join
leaves with dots: `f.block.op`, `Type.Case`. Collisions append a short
identity suffix. A name map is a JSON object from 64-hex identity bytes to
leaf names. Entries that are not names are ignored.

## 4. AV1 (output only)

An AV1 rendering begins with the header line
`# sley view (AV1, non-canonical) root=<8 hex>[ after=<handle>]`. A
function renders as its signature, then one block per listed block (in
function order) with its parameters, one line per operation
(`name = mnemonic immediate operands`), and the terminator:

```text
fn f(a: i64, b: i64) -> Result<i64,E>   [1a2b3c4d]
  entry:
    c = lt a, b
    cond c -> small, large
  large(v: i64):
    r = ok v
    return r
```

Terminators render as `return v`, `br b(args)`, `cond c -> t(args), f(args)`,
`switch v: Case -> b(args), ...` (where `$` is the case payload), and
`trap code [payload]`. A value defined in another block renders as
`block.leaf`. Unlisted blocks and parameters that claim a function are
listed after it. Types, constants, TestCases and namespaces render as one
line each. `--package` renders every top-level entity in name order within
kind groups.

AV1 is derived debug notation under ADR-0001. No product component reads it.
Authoring input is AF1 or raw operations (JSON). AV1 text given to `try` is
refused as not JSON.

### 4.1 Focused views

`view --focus <name>` prints one entity and the context it directly
depends on, in at most 4,000 bytes. A block, operation or parameter name
focuses on its function. After the header, the line
`# focus <name>: context, not a completeness certificate` says what the
view is. The target renders exactly as `view <name>` renders it. For a
function, these sections follow:

- `types: N`, then the type definitions its signature and body name, one
  AV1 line each, in order of first appearance;
- `consts: N`, then the constants and globals it reads;
- `calls: N`, then the signature line of every function it calls or
  references;
- `callers: N: a, b`, the functions that call or reference it;
- `tests: N: t1, t2`, the TestCases that target it;
- `boundary:` its visibility, effects and namespaces, the number of other
  functions that call it (`callers outside it`), and its entry points.

A type, constant or global lists `types:` (the types it names) and
`used by:` (the functions, types, constants and globals that name it). Its
boundary gives its visibility, namespaces and number of users. A TestCase
lists its target's signature under `calls:`. An empty section prints
`none`.

When the text would exceed 4,000 bytes, parts are omitted from the least
relevant end. The caller and test name lists shrink to their counts first.
Then rendered context goes entity by entity: calls, then consts, then
types, each from its end. If that is still too long, the target's body is
replaced by its signature line and `# body omitted (N lines): sley-agent
view <name>`, and the context refills from the most relevant end. Each
omission prints its count and the exact command that shows what it left
out: `# 3 more calls: sley-agent view c d e` for rendered context, and
`tests: 60 (names: sley-agent --json view --focus f)` for a name list.
Expansion commands repeat `--after`, `--x` and the rendering switches. When
even the omitted names do not fit, the commands point to the JSON lists
instead. The header, the focus line, the target's signature and the
boundary line always print.

Under `--json`, the output is `{"view": text, "focus": {...}}`. `focus`
holds `target`, `kind`, and the complete name list of every section
(`types`, `consts`, `calls`, `callers` and `tests`, or `types` and
`used_by`). It also holds `boundary` (`visibility`, `exported`, `effects`,
`namespaces`, `callers_outside` and `entrypoints`, or `used_outside`),
`omitted` (each part with `section`, `count`, `names` and `expand`),
`bytes` and `bound`. The name lists are never bounded.

Every line of the target section equals the corresponding line of
`view <name>`. Every rendered context line equals the first line of that
entity's own view.

### 4.2 AV1-X (`--x`)

`view --x` renders functions in AV1-X. AV1-X is AV1 in which each region
that matches the plain-AF1 shape of an AF1-X expansion is shown in its
compact authored form. Names generated by expansion contain a double
underscore. The header is
`# sley view (AV1-X, non-canonical, output only) root=<8 hex>[ after=<handle>]`.
A region is sugared only when it matches one of these shapes exactly:

- A checked operation. The block's last operation `x__r` is used only by
  the block's `switch x__r: Ok -> B__x($, t...), Err -> exit` (or
  `Some`/`None`). `B__x` takes `x`, then parameters named like the threaded
  values `t...`, and has no other predecessor. `exit` is a shared exit
  block. The region renders as `x = op?Case operands` (`op?` for `__err`
  and `__none`), and the lines of `B__x` follow in place.
- A conditional exit. `cond c -> __fail_Case[(p)], B__if<i>(t...)` renders
  as `!Case if c[, p]`, and `B__if<i>` follows in place.
- A failure or success exit. `br __fail_Case[(p)]` renders as
  `fail Case [p]`, and `br __none` as `fail`. A block ending
  `B__ok = ok v; return B__ok` renders as `ok v`.
- An inline operand. A generated operand (`n__a<k>`, `B__t<k>` or
  `B__if<i>`) is used exactly once, and the hoisted operations sit
  immediately before their use in depth-first, left-to-right order. A
  constant renders as its literal (`3`), and any other operation as
  `(op a, b)`.

A shared exit block is one of three shapes: `__fail_Case` (`variant
E.Case`, with the payload parameter when the case has one, then `err` and
`return`), `__err` (`err e`, `return`), or `__none` (`none`, `return`). It
is hidden once every edge into it is sugared. Inside a merged region,
values of the region print by leaf. A continuation that would bring in a
second value under a leaf the region already has is not merged.

Every sugared line ends with a comment listing the expanded entities it
stands for, relative to its function:
`# entry.total__r, entry__total, __fail_Overflow`. `view <fn>.<name>`, or
`view <fn>` without `--x`, shows them in AV1. Everything else renders
exactly as AV1, so a program without generated names renders identically
apart from the header. `--x` combines with names, `--package`, `--after`
and `--focus`.

AV1-X is output only, like AV1. Nothing reads it back, and `try` refuses it
as not JSON.

## 5. AF1 (structured authoring data)

An AF1 frame is one JSON object with the keys `af1` (the number 1),
`types`, `consts`, `fns`, `patch`, `edit`, `tests`, `delete` and
`namespace`. The keys and their shapes are closed. `sley-agent help af1` is
the normative reference text and `data/help-af1.md` its source. AF1 has no
expression grammar. Every operation is a JSON array or object that names an
opcode from the closed table (section 6). Types are shorthand strings drawn
from a fixed constructor table (`i64`, `Result<T,E>`, `Vec<T>`,
`Map<K,V>`, `Option<T>`, `Cell<T>`, tuples, and named definitions), or the
equivalent one-key JSON objects.

Compilation, in one pass:

1. Deletions are resolved first. Deleting a function deletes the entities it
   owns and the TestCases that target it.
2. Every top-level name is resolved or allocated. A name that is live with
   the same kind keeps its identity. A new name allocates
   `EntityId::derive(workspace, nonce, kind, create ordinal)`, in allocation
   order.
3. Type members keep the identities they have by name. New members take
   fresh random identities (SSMC1: member identities are creator-supplied
   nonces).
4. Functions are defined whole (`fns`) or patched by block (`patch`,
   `edit`). Parameters, blocks, block parameters and operations are matched
   by name against the live function. Matched entities keep their
   identities. Unmatched live entities of the function are deleted.
5. Operation result types, ordinals, owner lists, parameter roles and
   ordinals, reachability, empty sets and visibility are derived. A result
   type that neither the operands nor a use (return, edge argument, call
   argument) determines must be given explicitly.
6. Literal constants reuse an equal live or frame constant, or create one
   named `k_<value>`.
7. Namespace membership stays consistent. Deleted members leave every
   namespace, and created top-level entities (not tests) join the named
   namespace, or the only namespace when there is exactly one. With
   `"namespace": null` they join no namespace; existing members stay, and
   deleted members still leave.
8. The planned operations are emitted as creates (in derivation order), then
   replaces (only for changed bodies), then deletes. Every precondition is
   bound in operation order: `ExpectedIdentityAbsent` for creates, and the
   exact live object version for everything else.

Malformed frames are refused with `AGENT_FRAME_INVALID` and a JSON pointer.
One refusal lists every problem of every function, patch, edit and test in
the frame: within a function, every operation that cannot resolve, then
every terminator that cannot. The first line carries the first problem with
its pointer and `(1 of N problems)`; each other problem follows on its own
indented line with its own pointer. A misplaced name therefore costs one
round rather than one round per use. A name found only in another block is
reported as that block's parameter (pass it on as an edge argument) or
result (qualify it as `block.name`); a switch case key that is not a case of
the scrutinee's type names the expected keys; an operand that is a literal
or a nested operation names the fix; and a list-wrapped edge argument
(`["b", ["x"]]`) names the flat form.

These mistakes each get a named fix:
- A terminator with the wrong shape names its correct shape. A `cond` with
  more than four items is shown with its trailing arguments bracketed onto
  the else target.
- A switch case that contains another case, or a bracketed target followed
  by more items, says how to close the case.
- A terminator word used as an opcode says it belongs in the block's
  `"term"`.

A frame or a block may carry a `"comment"` string, which is ignored.

The frame compiler also names, at the operation or terminator, the operand
and edge type errors that lowering would otherwise report only per function
(`VM_LOWER_SIGNATURE_MISMATCH`, `CFG_TARGET_ARGUMENTS`): an edge whose
argument count or types differ from the target block's parameters; a
`Result` or `Option` value given to an arithmetic, float or ordering
operation (switch on it first); a non-`bool` operand of `not`, `and` or
`or`; and operands of different types where the operation requires one
type. These checks refuse only frames the kernel refuses: they apply the
lowering rules of `sley-vm` to types the frame determines, and when a type
is not yet known they defer to the kernel. The kernel still validates every
candidate.

AF1 terminators accept `["br", "b", arg...]` and `["br", ["b", arg...]]`,
the bracketed target form `cond` and `switch` also accept.

The raw operation path accepts the 2.0.0 trial-tool JSON form for classes
`CreateEntity`, `ReplaceEntityVersion` and `DeleteEntityBinding` over kinds
3 through 9 and 14. Identities may be 64 hex, local names, or `@key` for a
create in the same list that carries `"key"`.

### 5.1 AF1-X (`"afx": 1`)

A frame with `"afx": 1` is an AF1-X frame. The workbench expands it,
client-side and deterministically, into an ordinary AF1 frame, which the
AF1 compiler of section 5 compiles unchanged. A frame without `"afx"` is
plain AF1 and refuses every form below exactly as before; a frame whose
`afx` is not the number 1 is refused at `/afx`. `sley-agent help
afx` is the normative reference text and `data/help-afx.md` its source.
AF1-X adds, inside the blocks of `fns` and `patch` (`edit.with` stays plain
AF1):

1. **Operands (X1).** After an operation's immediate, and in terminator
   value positions, an operand may be a literal (`3`, `true`,
   `{"type": "u8", "value": 3}`) or a nested operation
   (`["add", "a", 1]`). Strings are always names. A bare literal takes its
   type from its context (the other operand of a same-type operation, a
   callee or target parameter, the function result, a variant payload or
   record field); with none, the literal is refused, never defaulted.
   Nested operations evaluate left to right, depth first, before the
   operation that uses them. In `cond` and `switch` target arguments and in
   an exit's payload only names and literals are allowed, so that nothing
   runs on a path that is not taken.
2. **Checked propagation (X2).** `["x", "op?C", ...]` names the Ok (or
   Some) value of an operation that returns `Result` or `Option`. On
   failure control goes to `C`: a case of the function's error variant
   (built with `variant`, `err`, `return` in a shared exit), or a handler
   block. A name that is both is refused (`AGENT_X_PROPAGATION`). A payload
   moves only to a case or handler parameter of exactly its type; naming a
   case or handler without a payload is the explicit choice to drop it.
   Bare `op?` returns the failure unchanged when the function's error type
   equals the operation's, or `None` from an Option function.
3. **Exits (X3).** `["!C", "if", cond]` (optionally with a payload) leaves
   through `C` when `cond` is true. The terminators `["ok", v]`,
   `["fail", "Case"]`, `["fail", "Case", p]` and `["fail"]` (Option
   functions) return through shared exits.
4. **Names (X4).** A plain name resolves to an earlier value of its own
   block, else a function parameter, else the nearest definition of that
   name among the blocks every path to the use passes through. When that
   definition is an operation result and no other block redefines the name
   on a path from it to the use, the expansion qualifies it; when it is
   another block's parameter or checked value, or the name is redefined on
   the way, the name is refused (`AGENT_X_SCOPE`) with the fix (declare a
   parameter, or write `block.name`). An explicit `b.x` means block `b`'s
   own `x`, exactly as in plain AF1. An edge that passes fewer arguments
   than its target takes is completed, trailing parameter by trailing
   parameter, with the value of the parameter's name visible at the edge
   when its type fits; otherwise the author is asked (`AGENT_X_SCOPE`).
   Two edges take no argument by name: an edge back into a loop (into a
   block that dominates the edge's block), and a switch case whose value
   carries a payload unless the author wrote `$`. Explicit arguments are
   never changed and no value is chosen by type alone.

Expansion splits a block at each `?` and exit. The continuation takes the
unwrapped value and the block parameters still in use; generated names use
`__` (`x__r`, `<block>__<x>`, `<block>__if<i>`, `n__a<k>`, `__fail_<Case>`,
`__err`, `__none`), which authored names may not contain. Checked
arithmetic stays checked, failures leave at their written position, and
nothing is reassociated, speculated, retried or duplicated. Depth 32,
4,096 expanded operations and 1,024 generated blocks per function bound the
expansion (`AGENT_X_LIMIT`); a bound refuses, never truncates. In a
`patch`, restating a block deletes the generated blocks of its previous
expansion and shared exits no longer used.

Every generated entity maps to the authored JSON pointer it came from.
A problem found in the expanded frame is reported at the authored pointer,
followed by `[expanded <pointer>]`. The expanded frame and the source map
are kept with the draft revision (`sley-agent draft <d> --expanded`).
`ripple` intents (typed graph transformations) are described in section
5.3.

### 5.2 Test tables

`"test_tables"` (AF1-X frames) states a target function and defaults once,
then one row per case:
`{"name": "t_f", "fn": "f", "defaults": {"limits": {...}}, "cases":
[{"args": [...], "expect": ..., "name": "optional", "limits": {...}}]}`.
Each row lowers to one AF1 test named by its `name`, else `<table>_<i>`;
row limits override the table defaults, and the rules of section 7 then
apply unchanged. A malformed row, two rows with the same arguments, or a
derived name that collides with another test is refused with
`AGENT_TEST_TABLE_INVALID` at the row's pointer. `try --on` replaces a table
by name.

A row's test name (given or derived) may name a TestCase live at the head
only when that very test, as it is now, was made by the same table in the
same draft lineage: the same entity, at the object version the table made
(a replacement by name keeps the entity, so a test that another change has
replaced since is no longer the table's); the row then updates it. Otherwise the row is refused
with `AGENT_TEST_TABLE_INVALID` at the row (at its `name` when given), and
the refusal names the draft whose table made the test when one did. The
author gives the row another `name`, deletes the live test explicitly
(`"delete": ["t_0"]`, which makes a new test), or updates the table on the
draft that made it (`try --on <draft>`, with `--rebase` after a commit).
A draft lineage records, per table and test name, the TestCase entities
and object versions its candidates made (`tables` in `status.json`,
`{"id", "object"}` records); a lineage continues
through `try --on`, `fill` and `import --on`, and through `try --on
<handle>` to the draft that made the candidate. When a table is restated
without a row it once made, the live test of that row is deleted in the
same candidate (a `note:` says so), so no stale row survives beside the
restated ones. A test the table made that another change has replaced is
never deleted this way: a `note:` says it stays as it is. A table removed
from the frame altogether deletes nothing; its live tests are deleted only
explicitly.

### 5.3 Ripple

An AF1 frame with `"afx": 1` may carry `"ripple": [intent, ...]`: typed
changes stated once. `sley-agent help afx` is the reference text. Intents
apply in written order after the frame's own definitions are expanded, and
each derives ordinary AF1 edits into the same frame: patches of the live
functions it rewrites (only the blocks that change, restated with every
other operation as it is), rewritten calls in the frame's own functions,
and restated TestCases. The unchanged compiler and kernel then judge the
whole candidate; a derivation is never admission evidence. A decision a
derivation cannot make mechanically is an obligation whose pointer leads
into the intent (`/ripple/<i>`, `/ripple/<i>/value`, `/ripple/<i>/in/<j>`),
and the frame is then not compiled. Derivation is deterministic. The derived
edits are part of the draft's expanded frame, and `ripple.json` in the
draft revision lists, per intent, the sites and tests rewritten or left as
written, the boundary met and the changed entities. The events ledger
counts `ripple_intents`, `ripple_edits` and `ripple_holes`.

An intent the head already reflects (a committed revision derived it)
derives only what differs from what that derivation left, so a revision
layered on the committed one, such as a tests-only `try --on dN --rebase`,
gives what the draft gave before the commit. For an `arity` intent whose
restated parameters `f` already has at the head: every live call's value
fill (the constant load `n__v<k>` that passed `"value"` as argument `k` of
call `n`) that loads another value than the intent's `"value"` is
rewritten to load it; each call or test of `f` the frame itself writes is
read as the head has it: one whose argument count is not that of the
parameters, or that `"frame_calls": "old"` covers, takes the arguments of
the committed call of the same name (with the value fills that call
loads), or of the committed TestCase of the same name, and is an
obligation when there is none; one with the fitting count that differs
from the committed one is an obligation unless `"frame_calls": "new"`
covers it (one stated after the intent with no committed form is new code
and kept). Live tests keep their arguments. A `guard`
is applied when its function already has the exact shape its mode derives
(entry: a guard block in the chain of guards the function starts with,
calling `g` on the value the guards before it leave for `p`, its checked
value held by a `tuple`/`tuple_get` block, its error passed to the block
that returns it or to the named handler; preserve: a block ends with that
call, continuing without the checked value). An intent with nothing to
derive is listed as `already applied`.

In `try --on`, a follow-up's intent replaces the draft's intent with the
same identity where it stands: `arity` of the same function, or `guard`
with the same checker and `arg`; other intents are appended. An `arity`
intent the follow-up does not restate records the follow-up's provenance
in `"after"`: the functions, tests and test tables it states. The intent's
`"frame_calls"` covers only what the intent's own revision states, so a
call or test of `f` in content listed in `"after"` is read by its argument
count alone, and one whose count fits both the old and the new parameters
is an obligation (a later revision may copy a call written for either). A
restating intent keeps the list, unless it restates the function's
parameters (a new change) or states `"frame_calls"` itself, which then
covers everything the frame states. The author may state or edit the
list. Several entry guards on one function run in written
order: each goes after the guards the function starts with (including
committed ones) and checks the value they leave for its `arg`. A block
restated after a committed preserve guard replaces the pieces its
expansion made, through the guard's call as well.

Two intents are enabled:

- `{"arity": f}`, `{"arity": f, "value": v}`, optionally with
  `"frame_calls": "old"|"new"`. The frame restates the parameters of the
  live function `f`. Every call of `f` in live code the frame does not
  restate, and every TestCase of `f`, gets its arguments by parameter name:
  a kept parameter (same name and type) keeps its argument, a removed one
  drops it while its computation still runs, and a new one takes `v`, a
  literal of its type, when exactly one parameter is new. Anything else is
  `AGENT_RIPPLE_HOLE_UNFILLED` naming the site, the parameter and its type;
  a parameter kept by name with another type is never coerced. A call or
  test the frame itself writes (including those carried from earlier
  revisions of the draft, which the layered frame cannot tell apart) is
  read by its argument count: kept as written when it has the new count,
  rewritten by name when it has the old one. When the old and new counts
  are equal (a reorder or a rename), the count does not tell, so each such
  call or test is a hole unless the intent says `"frame_calls": "old"`
  (rewrite them all by name; one with another count is then a hole) or
  `"new"` (keep them all as written). The exported boundary is
  `AGENT_RIPPLE_EXPORTED_BOUNDARY`: `f` is the function of an entry point,
  a global's initializer, named by a contract or policy binding, or in a
  package's exports, directly or through a namespace that holds it at any
  depth (code outside the program's calls uses its parameters); or a call
  ripple would rewrite belongs to a function in other namespaces than `f`,
  whether that function is live or restated by the frame (a new function
  joins the frame's namespace). Visibility alone is not the boundary. A
  use of `f` as a value (`fnref`, or a constant or TestCase that holds a
  reference to `f`) is unresolved dispatch and a hole.
- `{"guard": g, "arg": p, "in": [f, ...], "mode": m}`, in entry mode
  optionally with `"handler": b`. `g` is a checker `P -> Result<P,E>`
  (another shape is `AGENT_RIPPLE_GUARD_SHAPE`; no error case is inferred
  for `Option`), defined in the same frame or live; each `f` is a live
  function the frame does not restate, with parameter `p` of type `P`. The
  guard reads the program as the frame leaves it: `g`'s body, constants and
  the call graph are those the frame defines (the frame's definitions
  compiled over the program, with the edits of earlier intents), so a
  frame that does not compile is refused as it stands. `"preserve"` (the
  default) replaces an inline check at its own position when it is
  structurally the same as `g`'s body up to names: the same pure operations
  on `p`, in the same order, ending a block, with constants equal in the
  candidate and the same functions called, the same error results, and one
  success continuation that reads nothing the check defines. The checked
  value, the order of evaluation, the errors and the continuation are then
  those of the original. A match is never inferred from tests; anything
  else is `AGENT_RIPPLE_GUARD_ORDER`. `"entry"` evaluates `g(p)` once when
  `f` starts, before anything else `f` does. When `g` fails, `f` fails with
  `g`'s error on every path: this changes not only which error wins when
  several inputs are invalid, but also turns paths that returned early,
  trapped, or failed another way before reaching a check of `p` into `g`'s
  error. When `g` succeeds, `f` runs as before, and every use of `p` that
  the `Ok` payload dominates reads it; a block the error route can also
  reach, or an explicitly unreachable one, keeps `p`. The error is
  returned unchanged when `f` returns `Result<_,E>`; otherwise it goes to
  the block the author names with `"handler"`, which must take exactly one
  `E`, and without one it is a hole. A block is never chosen as a route by
  its parameter type alone. Entry is refused (`AGENT_RIPPLE_GUARD_ORDER`)
  when `f` performs effects, when `g` calls `f` in the candidate's call
  graph, and when `f` already evaluates `g`, on any value, directly or
  through the functions it calls, since a second evaluation could not be
  excluded; it never deletes an existing check.

`effect`, `member`, `retype`, `move` and `prune` are not enabled in this
build: they are refused with `AGENT_RIPPLE_INTENT_UNKNOWN`, as is any
unknown intent. The bounds are 32 intents per frame, 256 call sites and
tests per `arity`, 64 functions per `guard`, 64 blocks per checker (in
either mode), and 1024 structural comparisons per function a preserve guard
searches; reaching one is `AGENT_RIPPLE_LIMIT`, never truncation or a
reported mismatch. A function with type parameters or declared effects is
never patched, since AF1 cannot restate either. A frame refused before it
compiles still records its expansion counts (the ripple counts among them)
in the events ledger.

## 6. Opcodes

The mnemonic table maps every epoch-1 opcode one to one:
`const tuple tuple_get record field variant variant_get vec vec_len vec_get
vec_set map map_get map_has map_insert map_remove add sub mul div rem neg shl
shr fadd fsub fmul fdiv fneg fma eq ne lt le gt ge not and or call some none
ok err assert observe effect adapter narrow cell cell_get cell_set hash global
fnref`. AF1 also accepts the SSMC1 names and the numeric tags.

## 7. TestCase defaults and the workbench policy

An AF1 test (or a test table's `defaults` or row) declares its limits as
`"limits"`, an object of non-negative integers whose keys are exactly
`fuel`, `memory_bytes`, `output_bytes`, `effect_count`, `call_depth` and
`wall_timeout_millis`. Another key, a value that is not an integer, or a
`limits` that is not an object is refused with `AGENT_FRAME_INVALID` at its
pointer, and the refusal names these keys. A limit a test does not declare
takes the smaller of the workbench default and the grant: `fuel` 1,000,000;
`memory_bytes` 16,777,216; `output_bytes` 65,536; `effect_count` 0.
`call_depth` is 256 and `wall_timeout_millis` 10,000 (these are context
limits, not grant limits). A declared limit is used as written. For the TestCases a candidate selects (those that target
a function it changes), a limit above the grant is the kernel's
`CANDIDATE_TEST_RESOURCE_LIMIT`; phase 12 does not check the limits of a
TestCase it does not select.

`init` grants its principal fuel 1,000,000, memory 16,777,216 bytes, output
65,536 bytes, 0 effects, 10,000 mutations per candidate and 0 adapter calls,
for the mutation classes 1 through 9, 12 and 13. Benchmark starters built
with `init` therefore meet `SLEY-2.0.2-BR` BR-03(b).

## 8. Refusals, locators, explain

A refusal reports:

- the decision name;
- the failed phase and its name (`CANDIDATE_RESULT_V1.md` section 6);
- the diagnostic's source symbol and numeric code;
- the retryability;
- the hint from `data/hints.json`, keyed by symbol and falling back to the
  decision. An unknown symbol reports `no hint`.

The locator comes from `CandidateValidationOutput::refusal_locator`: a
non-encoded side channel that the refusing phase fills and that never
enters the result record or its digest (ADR-0051). Phase 2 names the
exceeded profile limit. Phase 3 names the stale operation and entity. Phase 4
names the colliding operation. Phase 5 names the entity whose reference
failed, the dependency and the relationship. Phase 7 names the function
whose graph failed. Phase 9 names the ungranted operation or the mutation
budget. Phase 12 names the TestCase, the limit, the requested value and the
effective ceiling. For phase 7, the workbench also reports the offending
block, operation or operand, found by an advisory analysis of that one
function (dominance, reachability, owner lists, edge arity). That analysis
runs only after a kernel refusal and only describes it. After a phase-7
refusal, `try` also runs the analysis over every function of the proposed
program and prints each further finding as an `also:` line (`"also"` in
JSON), so one round discloses every structural problem the analysis sees
rather than the kernel's first.

A phase 7 locator names a whole function. When the refused candidate was
made from a frame, `try` and `explain` add an `authored:` line (`"authored"`
in the JSON verdict, a list of `{"at", "what"}`): JSON pointers into that
frame (for `try --on`, the layered frame: the `frame.json` of the draft
revision that made the candidate, for a draft or a candidate handle) of the
blocks, operations, terminators, signatures (`params`, `returns`) and
constant types the analysis ties to the kernel's symbol. Besides the
structural checks, the analysis compares declared types exactly as the
kernel does for a returned value against the function's result
(`CFG_RETURN_TYPE`), and for a `call` or `const` against the callee's
signature or the constant's type (`VM_LOWER_SIGNATURE_MISMATCH`), so the
refusal of a caller the frame does not contain points at the signature or
constant the frame changed. When the analysis ties nothing to the symbol,
the line gives the function's own entry marked `(function-wide; the kernel
names no smaller location)`, with its authored parameters and result for a
signature refusal, or says that the function is not in the frame. Names the
frame does not spell are looked up in the candidate's source-map names table
when it has one. The line never changes the kernel's judgment and never
narrows a function-wide cause to one operation.

## 9. Workbench refusal symbols

Symbol-only (numeric `0`, the SMP1 section 8 convention):

| Symbol | Meaning |
|---|---|
| `AGENT_USAGE_INVALID` | the command line names no known command or arguments |
| `AGENT_WORKSPACE_NOT_FOUND` | no directory at or above the start holds `repo/` or `base.pack` |
| `AGENT_WORKSPACE_INVALID` | the repository, seed, policy grant or genesis could not be loaded or written |
| `AGENT_NAME_UNKNOWN` | a local name does not resolve in the selected state |
| `AGENT_FRAME_INVALID` | an AF1 frame or raw operation list is malformed (with a JSON pointer) |
| `AGENT_HANDLE_UNKNOWN` | a candidate reference does not resolve |
| `AGENT_CANDIDATE_INVALID` | the kernel refused to construct or validate the candidate record |
| `AGENT_INPUT_INVALID` | a call or test input does not fit the declared type |
| `AGENT_EXECUTION_REFUSED` | the dev loop cannot execute the function or state |
| `AGENT_SUBMISSION_REFUSED` | the candidate is not Valid, changes a function no TestCase in it targets (without `--untested`), or the transaction engine refused a commit |
| `AGENT_IO_FAILED` | a workspace file could not be read or written |
| `AGENT_DRAFT_STALE` | the revision a `fill` names is not the draft's latest revision, or another command recorded a newer revision while this one ran, holds a claim of one, or left a claim that cannot be checked |
| `AGENT_DRAFT_HEAD_CHANGED` | the accepted head changed since the draft revision; build on the new head explicitly with `--rebase` |
| `AGENT_DRAFT_INCOMPLETE` | the draft revision has no complete, Valid candidate for the request: a text revision or a follow-up not yet layered cannot be layered on, and only a `valid` revision is submitted |
| `AGENT_DELTA_INVALID` | a delta has another shape, or a target that is malformed, missing, given twice or overlapping another |
| `AGENT_X_PROPAGATION` | an AF1-X `?` or exit has no single, type-correct failure route |
| `AGENT_X_SCOPE` | an AF1-X name is ambiguous, not available where it is used, or an omitted edge argument cannot be derived |
| `AGENT_X_LIMIT` | an AF1-X expansion bound (depth, operations, generated blocks) was reached |
| `AGENT_X_EFFECT_ORDER` | reserved and never emitted: AF1-X expansion never reorders evaluation; a form that would run an operation on a path not taken (a nested operation in a `cond` or `switch` target argument, or in an exit payload) is a grammar refusal, `AGENT_FRAME_INVALID`, and a guard that cannot keep the written order is `AGENT_RIPPLE_GUARD_ORDER` |
| `AGENT_TEST_TABLE_INVALID` | a test table or row is malformed, duplicated, collides with another test, or names a live test its table did not make |
| `AGENT_RIPPLE_INTENT_UNKNOWN` | a `ripple` intent is unknown, or not enabled in this build |
| `AGENT_RIPPLE_TARGET_KIND` | an intent names something other than a live function it can change |
| `AGENT_RIPPLE_HOLE_UNFILLED` | a derivation needs an argument, route or decision it cannot derive |
| `AGENT_RIPPLE_EXPORTED_BOUNDARY` | a derivation reached a use outside the program's calls or in another namespace |
| `AGENT_RIPPLE_LIMIT` | a ripple bound was reached |
| `AGENT_RIPPLE_GUARD_SHAPE` | the checker is not `P -> Result<P,E>` for the guarded parameter |
| `AGENT_RIPPLE_GUARD_ORDER` | no identical check to replace, or an evaluation order a guard cannot keep |
| `AGENT_SEARCH_NO_ORACLE` | `search` has no permitted public case for the function: the case file is unreadable, holds no case, none for the function, or none whose arguments fit the function |
| `AGENT_SEARCH_SEED_INVALID` | the `search` seed is unusable (refused, incomplete, a text draft, not made from a frame), the name is not one of its functions or cannot run, or the attempt has used its searches |

## 10. Execution (advisory)

`call`, `test`, `try` and `status` project the selected state with
`project_complete_entities`, build its type environment once, lower each
function once (`lower_function`, extended profile), load and digest-verify
its image once (`VerifiedImage::load`) and execute every input with
`VerifiedImage::execute`, which answers exactly what `execute_loaded_image`
answers for the same request. `call` runs under generous fixed ceilings
(`sley_agent::exec::call_limits`). A TestCase runs under its declared fuel.
Expected and observed results are compared by the kernel's rule: value hashes
(`hash_validated_value`) or trap codes through
`sley_tests::compare_expected_evidence`. Functions that declare type
parameters, effects or contracts are refused by lowering. That is the
profile's limit, not the workbench's.

## 11. Evidence

`crates/sley-agent/tests/workbench.rs` executes every acceptance item this
contract states. The items are: AV1 size and byte stability; refusal of AV1
as input; AF1 edits with tests; a failing expectation showing both values;
the TestCase-limit, orphaned-block, dominance, unresolved-reference and
mutation-budget locators; repeatable submission; a 300-operation candidate;
JSON-pointer errors and one refusal per frame round, across functions,
with the first pointer in the headline; the edge count, edge type and
operand type checks; `also:` findings; terminator and case shape fixes;
`try --on` layering, its pointers and its refusal of raw handles; the
edit-in-place hint; the untested-submission refusal and
`--untested`; batch streaming;
name-matched redefinition; every guide example (fenced frames and the
inline `br`, `cond` and `switch` terminators), every JSON example line of
the `af1` and `tests` help topics, and every value form the `types` topic
documents (read and rendered back); the `init` ceilings, pinned; `call`
and `test` leaving the repository byte-identical; hex only under `--raw`; and every workbench
refusal symbol. The candidate-result
conformance vectors and the `sley-policy` suite pin that the locator channel
leaves result bytes unchanged.

`crates/sley-agent/tests/views.rs` executes the view items of section 4:
focused and full view parity, byte stability, the 4,000-byte bound and
every expansion command it prints, the JSON shapes, AV1-X on each sugared
shape with its routes, the fallback to AV1 when a shape is not exact,
identity with AV1 for programs without generated names, and the refusal of
AV1-X as input.

`crates/sley-agent/tests/afx.rs` executes the AF1-X items of section 5:
each form against a hand-written plain AF1 equivalent on the same inputs
(overflow, division by zero, simultaneous failures, payloads, calls,
Option operations, shadowing, joins, loops), generated expression trees
against a reference evaluator, determinism, source-map pointers, every
refusal named in section 5.1, test tables, and the unchanged compilation of
plain AF1 frames. `tests/drafts.rs` executes section 12, `tests/locators.rs`
and `tests/namespace.rs` the authored locators and the namespace opt-out.
The guide stays at or under 3,500 bytes, and its examples and commands and
those of the `drafts` topic run in `tests/workbench.rs`.

Four refusals agents meet in practice (`CANDIDATE_TEST_RESOURCE_LIMIT`
with memory 1,000,000 over a ceiling of 1,000, `GRAPH_UNRESOLVED_REFERENCE`,
and the phase-7 orphaned-block and dominance refusals) are regression tests
that rebuild each scenario through AF1 or the raw path. Each test asserts
the symbol and the locator the spec names.

## 12. Drafts, delta repair and test import

Drafts are advisory, local authoring state. They never enter a candidate,
are never admission evidence, and change nothing the kernel judges.

### 12.1 Draft revisions

Every `try`, `fill` and `import` records one draft revision under
`.sley/drafts/dN/`. A revision is identified by its draft handle, its
revision number and its base head (the accepted transaction it was made
on), and is spelled `d1@r3`; `d1` alone means the latest revision.
`draft.json` names the latest revision and the base head of `r1`; the
latest revision is the highest recorded `rN`. Each revision directory
`rN/` holds:

| File | Content |
|---|---|
| `input.txt` | the exact input bytes (the frame, delta or case file given) |
| `frame.json` | the complete authored frame of the revision (layered, or with the delta applied); absent when the input is not JSON |
| other `*.json` | derived authoring artifacts of the compiled frame |
| `status.json` | `revision`, `base_head`, `parent`, `made_by` (`try`, `try-on`, `fill`, `import` or `rebase`), `on`, `unlayered`, `delta`, `whole_frame`, `state`, `candidate`, `candidate_sha256`, `verdict`, `obligations`, `tests`, `sources`, `tables`, `results` when tests ran, and `skipped` when claims of stopped commands were skipped |

A revision's `state` is one of:

- `text`: the input is not JSON. It is kept with the parser's line, column
  and byte offset; no structure is guessed.
- `incomplete`: the frame parsed but made no candidate (a frame refusal, or
  the kernel could not build the record). No candidate handle is allocated.
- `refused`: the kernel refused the candidate made from this revision.
- `valid`: the candidate made from this revision is Valid.

A `refused` or `valid` revision names its candidate handle and the SHA-256
of that candidate's stored bytes, and the candidate's metadata names the
revision. Candidate handles keep their meaning and stay monotonic.

Commands may run concurrently in one workspace. A new draft claims its
directory, a revision its `.rN.partial` directory, and a candidate its
`cN.hex` file atomically (a create or link that fails when the name
exists, then the next number), so two commands never report the same
handle or revision, and a recorded revision always holds its own command's
frame and candidate. Shared files (`names.json`, `layered.json`,
`draft.json`, candidate metadata) are replaced whole, and `names.json` is
read, merged and replaced under an exclusive lock on `.sley/names.lock`,
so concurrent commands keep each other's names. A revision claim holds
`.owner`, which its command keeps locked until it records the revision or
drops the claim; the claims of a draft are made one at a time under
`dN/.claims.lock`. A claim whose owner lock is free belongs to a command
that stopped (killed, or out of time) before recording it: later commands
skip its number and never reuse it, because a candidate that command
stored may name that revision. A draft's revision numbers may therefore
have gaps; the revision recorded next lists the skipped numbers
(`skipped`) and its result says so in a `note:`. Every recorded
revision also claims the next entry of `.sley/drafts/.order/`, the
workspace's recording order over all drafts.

`try` of a frame starts a new draft (`dN@r1`). `try --on <draft>` layers
the frame on the frame of the draft's latest revision, or of the revision
`dN@rK` names, with the layering rules of section 2, and records the next
revision of that draft; its `parent` is the revision layered on. A
follow-up therefore states only what it adds or changes, and the tests of
earlier revisions stay in the frame until they are replaced or removed.
`try --on <handle>` keeps its section 2 meaning and starts a new draft. A
text revision cannot be layered on (`AGENT_DRAFT_INCOMPLETE`, with the
`fill` that replaces it whole).

A follow-up given to `try --on` or `import --on` is kept even when it
cannot be layered: when it is not JSON (state `text`), or when layering
refuses it (a frame without `"af1": 1`, or raw operations; state
`incomplete`). Such a revision records
`unlayered: true` and its base in `on`; its `frame.json` holds the
follow-up as given. Nothing is layered on it (`AGENT_DRAFT_INCOMPLETE`,
naming the repair), and `fill` repairs the follow-up and layers the result
on `on` again, so the base's definitions and tests stay in the draft. `try
--on <base>` with the corrected follow-up does the same in one step.

A follow-up refused before layering is not recorded: when the revision it
names is a text revision or an `unlayered` one (`AGENT_DRAFT_INCOMPLETE`),
when that revision was made from raw operations (`AGENT_USAGE_INVALID`,
as `try --on` refuses a raw handle in section 2: no repair of the
follow-up could fix its base, so the refusal points to a standalone
`try`) or holds JSON that is not a frame object (`AGENT_DRAFT_INCOMPLETE`,
with the `fill` that replaces it whole), or when the head changed since
that revision (`AGENT_DRAFT_HEAD_CHANGED`, section 12.5). Recording it would either build on a revision without a
complete frame, hiding that revision's repair behind a newer one, or build
on a head the author did not choose. The refusal says the follow-up was
not recorded and how to send it again: after the repair, with `--on` the
revision the refusal names, or with `--rebase`. `import --on` is refused
the same way.

### 12.2 Trial output

A frame refusal keeps exit status 2, its `error AGENT_*:` line and its
problem lines. It then says where the pointers point (the frame file to
edit in place, or the revision's `frame.json` for a layered frame), and
adds the draft line (`draft d1@r3: incomplete, 2 obligation(s)
(AGENT_FRAME_INVALID 2); list: sley-agent draft d1 --obligations`) and a
`next:` line with the `fill` that repairs it in place, at the first
obligation's pointer; when that pointer had to move to an existing
ancestor (section 12.3), the line says which member is missing and that
the ancestor is replaced whole. Input that is not JSON gets the draft line
with the parser location. In JSON the refusal object adds `draft`,
`state`, `obligations` and `next`.

The first line of a candidate's result ends with its draft revision
(`c4: Valid (+3 created, 1 replaced, 0 deleted) draft d1@r2`). The result
then lists:

- `changed:` the functions, types, constants and TestCases the candidate
  creates (`+`), replaces or changes a part of (`~`), or deletes (`-`), by
  kind and name (`changed: fn ~bound +area; type +Shape; test +t1`);
- `exported:` the exported functions that existed before the candidate and
  are changed or deleted by it;
- `tests: X/Y passed [authored A, imported I, provided P]` (counts that are
  zero are left out; `; replaces provided t_1` names the live tests the
  candidate replaces) and one line per failing test; passing tests are
  listed only with `--verbose`;
- `tests: 0 ran` when no TestCase ran, as section 2 describes;
- the `next:` step, which layers on the draft (`try --on d1`), repairs it
  (`fill`) or submits it (`submit d1`).

A `--public` case file is read and checked before anything is recorded or
stored: a file that cannot be read (`AGENT_IO_FAILED`), is not a JSON
array, or holds a case without a `function` or with `args` that are not
an array (`AGENT_INPUT_INVALID`) is refused on its own. A case that cannot
run against the candidate (its function does not resolve, or its
arguments do not fit) is known only once the candidate exists: the
revision, its candidate and its own test results are recorded
(`results.public_refusal` in `status.json`), the result is printed as
above without a `next:` line, and its last line is the refusal, `error
AGENT_...: <file>: case <i> (<name>): <detail>` (in JSON, `error` and
`detail` beside the result), with exit status 2.

A kernel refusal about a TestCase, when the verdict names no authored
positions itself (section 8), gets them from the test: `authored:` lists,
as bare pointers with what each designates, the refused limit (when the
refusal names one) and the test's entry in the frame, for example
`authored: /tests/0/limits/fuel (fuel limit of test t_lim), /tests/0
(test t_lim)`. For a test made from a table row these are the row's
`limits` or the table's `defaults` the limit comes from, and the row. In
JSON they are the kernel obligation's `at` and `also_at`; the verdict's
own `authored` list stays the phase 7 analysis of section 8.
Unchanged program bodies are not reprinted. JSON output keeps every earlier
key and adds `draft`, `state`, `changed`, `obligations` and `provenance`.

### 12.3 Obligations

An obligation is an unresolved problem of a revision: `{"id", "symbol",
"at", "expected", "available", "decision", "kernel", "count"}`. The problem
lines of a frame refusal become obligations; a line prefixed
`[AGENT_...] ` carries that symbol, and every other line carries the
refusal's symbol. `at` is the JSON pointer into the revision's
`frame.json` (`""` for the whole frame, `null` when the problem names no
pointer). An `at` always exists in that frame: a problem about a missing
member (a test without `args`) points at the nearest existing ancestor,
and the pointer the problem names is kept as `missing`, so the `fill` an
obligation suggests is one `fill` accepts. `expected` and `available` are set only when the problem states
an expected type or shape or the values available, and are `null`
otherwise. Problems with the same symbol and decision form one obligation
with their `count`; `also_at` lists the pointers after the first. A kernel
refusal is one obligation whose `kernel` holds the kernel's symbol, phase
and locator unchanged; its `at` is the first authored position of the
verdict (for a TestCase limit, the limit), when there is one, and
`also_at` holds the others. Obligations report what the author must decide;
they complete nothing.

`draft` lists the drafts: the latest revision of each, its state, its
candidate, and its open obligations. `draft <draft>` prints a revision's
state, origin, test counts and at most 8 obligation lines, then
`N more: sley-agent draft <draft> --obligations`. `--obligations` prints
every obligation and pointer, `--input` the exact input, `--frame` the
frame, and `--expanded` the derived artifacts. With `--json`, `draft
<draft>` prints `status.json` with `draft` and `latest` added.

### 12.4 Delta repair

`fill <draft> <delta> --revision <N>` repairs the latest revision of a
draft. The delta (a file, `-`, or inline JSON) is the closed object
`{"set": [{"at": "<pointer>", "value": <replacement>}, ...]}` with exactly
these keys. Each `at` is an RFC 6901 pointer that exists in the revision's
`frame.json`, and `value` is the complete subtree that replaces it. `""`
replaces the whole frame and is recorded as `whole_frame`. Every target
resolves against revision `N`, and the replacements apply together. An
array grows or shrinks only by replacing the array. No target is ever
chosen by similarity.

`fill` is refused, and writes no revision and no candidate, when:

- `N` is not the latest revision (`AGENT_DRAFT_STALE`);
- the delta has another shape or an empty `set`, or a pointer is
  malformed, does not exist, is given twice, or contains another pointer
  of the delta (`AGENT_DELTA_INVALID`);
- the revision is a text revision and a target is not `""`
  (`AGENT_DELTA_INVALID`).

A successful `fill` records revision `N+1` (`made_by: "fill"`, with its
targets and size under `delta`) and runs it exactly as `try` runs a frame.
Filling an `unlayered` revision replaces parts of its follow-up (or, for a
text revision, the whole follow-up) and layers the result on its `on`
base again. When another command records a revision of the same draft
while `fill` runs, the fill is refused with `AGENT_DRAFT_STALE` and writes
nothing; `try --on <draft>` and `import --on <draft>` without an explicit
revision are refused the same way. They are also refused while another
command holds a claim of the next revision (the refusal names the revision
it is recording), and when a claim cannot be checked (a claim directory
without a lockable owner file, which the refusal names, to be removed when
no other command runs on the draft). A claim whose command stopped never
refuses them: its number is skipped (section 12.1).

### 12.5 Head changes

A revision records the accepted head it was made on. When the head has
changed since, `fill`, `try --on <draft>` and `import --on <draft>` refuse
with `AGENT_DRAFT_HEAD_CHANGED` unless `--rebase` is given. A rebased
revision records `made_by: "rebase"` and `rebase: {"from_head",
"to_head", "via"}`. Nothing is rebased implicitly, and a command refused
for a changed head records nothing: the refusal says to send it again
with `--rebase`.

### 12.6 Submission

`submit <draft>` submits the candidate of the draft's latest revision, and
`submit <draft>@r<N>` that of revision `N`. The revision must be `valid`,
and the stored bytes of its candidate must match the recorded SHA-256;
otherwise the submission is refused with `AGENT_DRAFT_INCOMPLETE`, which
names the revision's state. The candidate of an earlier revision is never
used in place of a later one. The candidate then takes the unchanged
submit path, including the refusal of an untested function change.
`submit <handle>` is unchanged.

A bare `submit` (or `submit latest`) takes the draft revision recorded
last in the workspace, over every draft (the last entry of the recording
order; `draft` marks it `recorded last`). It submits that revision's
candidate when the revision is `valid`; any other state is refused with
`AGENT_DRAFT_INCOMPLETE`, naming the revision, its state and the latest
valid revision of its draft, if any. An earlier revision or candidate is
submitted only when named (`submit d1@r2`, `submit c4`). In a workspace
without draft revisions, a bare `submit` takes the latest candidate
handle.

### 12.7 Test import and provenance

`import <cases.json> [--on <draft>] [--only a,b]` reads public cases
(`[{"name", "function", "args", "expect"}]`; a case without a name is
`case<i>`) and records a revision whose frame adds them as AF1 `tests`
named by their cases: layered on the draft with `--on`, or as a new draft.
`--only` selects cases by name. With `--on`, a case replaces a draft test
of the same name only when that test is an earlier import unchanged since,
or already equals the case; a test the author wrote (a `tests` entry, or
the test a `test_tables` row makes under its given or derived name), or
changed after its import, is never replaced: the import is refused with
`AGENT_INPUT_INVALID`, naming those cases, and records nothing (`--only`
leaves them out). A case without `expect` is refused with
`AGENT_INPUT_INVALID`: expected values come from the case file, never from
running a candidate. `status.json` records the file's SHA-256 and case
count (`import`) and, per imported test, its case, the file's SHA-256 and
a digest of the test entry (`sources`). The frame carries no provenance.

Tests are counted by origin (`tests` in `status.json`, `provenance` in
trial JSON):

- `provided`: the TestCases live at the base head that the candidate
  keeps and does not replace. A frame test (or table row) that restates a
  live test unchanged, so that its compiled TestCase is the live one,
  counts here once and not as imported or authored. A frame test that
  changes a live test replaces it: it counts where its entry comes from,
  and `replaced` lists it. A live test that a ripple `arity` intent
  restates (its arguments rewritten for the changed parameters, its
  expectation kept) is still provided, counted once, and not listed as
  replaced;
- `imported`: the other frame tests whose entry is unchanged since their
  import;
- `authored`: the remaining frame tests and table rows.

Every TestCase of the candidate that the frame states or the head
provides is therefore counted once. Before a candidate exists (an
incomplete revision), a frame test that names a live test counts where its
entry comes from, and that live test is not counted as provided.

An imported test that the author restates counts as authored from then on.
Imported tests never satisfy a requirement to author a test.

### 12.8 Events ledger

Every command that uses a workspace appends one JSON line to
`.sley/events.jsonl`, also when it is refused before it opens the
workspace (an unreadable frame or delta, a malformed case file), with
exactly these keys: `seq`, `cmd`, `draft`,
`candidate`, `input_bytes` (the frame, delta or case file read, otherwise
the command line), `output_bytes` (what the command printed),
`whole_frame`, `rewrite` (a `try` of a whole new frame while drafts
exist), `delta_targets`, `delta_bytes`, `afx` (the authoring
feature counters of the compiled frame), `table_rows`, `tests` (the
provenance counts), `refusal` (the workbench or kernel symbol),
`obligations` and `valid`. A line holds counts, handles and symbols only:
no clock, no names and no program content. `seq` is the line's position in
the file: each command takes it and appends its line (one write) under an
exclusive lock on the ledger, so concurrent commands get unique numbers in
file order. A failure to append never fails the command, and `help` and
`version` append nothing.

`crates/sley-agent/tests/drafts.rs` executes the contract of this section:
stale revisions, changed heads and explicit rebases, missing, repeated and
overlapping delta targets, recorded whole-frame replacement, text drafts
repaired whole, monotonic candidate handles bound by digest, tests
inherited across one-operation corrections, name collisions across
revisions, the refusal to submit after a newer incomplete revision
(named or bare), import provenance and digests, grouped and bounded
obligations, ledger lines without content, table rows that would take over
live tests, restated tables, existing obligation pointers, concurrent
commands, repaired and refused follow-ups, replaced provided tests,
identical restatements counted once, the authored pointers of table-test
refusals, ledger sequence numbers under concurrent commands, and
follow-ups refused without being recorded.

## 13. Verified search

`search <fn> --public <cases.json> [--from <seed>]` proposes small, typed
repairs of one function and checks each against the public cases the
author passes. It prints ranked edit frames and the evidence for each. It
never submits, commits or stores a candidate; the author applies a neighbor
with `try --on <seed> <frame>`, the command its `next:` line prints.

### 13.1 Seed and cases

The seed is `--from`: a Valid candidate made from an AF1 frame, or a draft
revision in state `valid` (`d1` is the latest revision). Without `--from`
it is the head. The candidate is validated against the current head again;
a refused, stale, incomplete or text seed, a candidate made from raw
operations, a name that is not a function of the seed, and a function the
dev loop cannot run (lowering refuses it) are refused with
`AGENT_SEARCH_SEED_INVALID`. A reference that does not exist is
`AGENT_HANDLE_UNKNOWN`.

The case file has the `try --public` format
(`[{"name", "function", "args", "expect"}]`). It needs at least one usable
case (a `function`, an `expect` and an `args` array) for the function
whose arguments read against the function's parameter types, or `search`
refuses with `AGENT_SEARCH_NO_ORACLE` and names why each case cannot run.
Every usable case in the file runs, in file order. Expected values come
only from the file: no expectation is derived from the seed or a neighbor.

The `TestCases` of the function in the seed and in each Valid neighbor
(the frame's own, imported and provided ones, found by their target's
name) run after the public cases, under their declared limits with fuel
at most 10,000,000. They are author-written expectations, reported beside
the public cases and never ranked.

### 13.2 Generators and neighbor frames

Six typed generators run over the function's reachable blocks. Generation
is local and deterministic.

| Generator | Proposal | Edit size |
|---|---|---|
| opcode swap | another opcode of the same-signature family: `add sub mul div rem`, `shl shr`, `fadd fsub fmul fdiv`, `eq ne`, `lt le gt ge`, `and or` | 1 |
| operand permutation | the two operands of `sub div rem shl shr fsub fdiv lt le gt ge` exchanged, when they differ and have one type | 2 |
| operand substitution | an operand, a returned value or a `br` argument replaced by another visible value of the same type | 1, plus each nested operation and literal it removes |
| constant nudge | an integer constant plus one, minus one, or negated, in its type's range, as a new literal | 1 |
| edge swap | the two targets of a `cond`; the targets of two `switch` cases whose arguments fit the other target's parameters | 2 |
| negation | a `cond` condition (or an AF1-X exit condition) `c` becomes `not c`; a condition `not x` becomes `x` | 1 |

A visible value is a function parameter, a result of a block that
dominates the block, or a parameter or earlier result of the same block.
A value with no same-typed substitute gets no substitution. Neighbors are
generated one generator at a time (opcode swap, permutation, nudge, edge
swap, negation, then substitution), each in program order; the position in
that sequence is the neighbor's generation index.

Each neighbor is an ordinary `edit` of one operation or `patch` of one
block, written so that `try --on <seed>` layers it on the seed's frame.
When the seed's frame states the function, the neighbor restates what the
author wrote, in the frame's dialect; for an AF1-X frame the source map
leads each operation back to its authored statement, nested operation or
literal. Otherwise it restates the live function. When a substitution
removes the use that typed a value (a returned value, a `br` argument or a
call argument, from which the frame compiler infers the type of `ok`,
`err`, `none` or an empty `vec`), the neighbor gives the authored
statement of that value's operation its `"type"` (the object form, with
the type the seed compiled); when that statement is in another block, the
neighbor is a `patch` of both blocks.

Layering records the functions a follow-up states in the `"after"` of each
`arity` intent it does not restate (section 5.3). When the seed's frame has
an `arity` intent with `"frame_calls"`, both parameter counts are equal,
and the neighbor states a function the intent reads (one that calls the
intent's target), the neighbor restates the intent exactly as the seed's
frame states it: the intent then reads the neighbor as it reads the seed's
own revision.

A change no such frame can state is skipped and counted before it can take
one of the neighbor slots:

- a change to code the tool generated (AF1-X continuations, checked
  switches, shared exits), and any neighbor of an AF1-X seed whose frame
  would name a generated (`__`) entity;
- a change to a block the seed's frame does not state whose code differs
  from the head's, except where an `arity` intent made the difference
  (below);
- any change to a function a `guard` intent of the seed's frame names: the
  intent refuses every frame that restates that function, so no neighbor
  layered on the seed can change it (the `next:` line says so);
- a permutation of two nested operations (it would reorder their
  evaluation), and a block change beside the seed's own `edit`;
- a substitution inside an authored call that an `arity` intent rewrites
  by name (`"frame_calls": "old"` with equal counts, or two intents on
  one target): the compiled arguments are not in the order the author
  wrote them, so the change has no place in the authored call;
- a change to code restated from the program (a block as the head states
  it, or an operation beside the seed's edits) that calls the target of
  an `arity` intent the head does not yet reflect, with equal parameter
  counts, unless the intent's `"frame_calls"` reads that call as written
  (`"old"` for a call as the head has it, `"new"` for one the seed
  compiled) and its `"after"` does not list the function: the intent
  could not tell which parameters the call is written for;
- a substitution that would replace a nested operation carrying a failure
  route (`op?`), a call, an effect, a contract check, an observation or a
  cell operation, anywhere inside it, by a name;
- `not x -> x` where `x` is an unnamed nested operation (writing it would
  evaluate it twice);
- a change whose layered frame, compiled as `try --on <seed>` compiles it,
  states exactly the head: it reverts the seed's change (counted apart as
  a revert: `skipped_reverts`, beside `skipped_unstated`).

A function that the seed frame's `ripple` intents rewrite while the frame
does not define, patch or edit it (a caller that an `arity` intent
rewrites) is searched as the head states it: the generators run over the
head's function, every change is written as a `patch` of the block, and
the neighbor is layered on the seed's authored frame with its intents, so
the derivation runs again on the patch exactly as `try --on <seed>` runs
it (a patch holding a call of the target of an intent with equal counts
restates the intent, as above). A value an intent derives (the `value` of
`arity`) is not a search target. The output marks such a search `derived`
and says how neighbors are written.

Search then compiles exactly the layered frame, assembles the record and
validates it with `validate_candidate_bytes` (the layered frame is
compiled while neighbors are generated, so a revert never takes a slot).
Only kernel-Valid neighbors run the cases.

### 13.3 Ranking, bounds and output

Neighbors rank by public cases passed (descending), then edit size
(ascending), then generator order (the table), then generation index. The
rule is printed with every result.

| Bound | Default |
|---|---|
| neighbors generated per command (`--max-neighbors`, at most 4096) | 64 |
| wall time per command (`--max-millis`, at most 3,600,000) | 10,000 ms |
| searches per attempt (per accepted head of the workspace) | 2 |
| fuel of a seed's case | 10,000,000 |
| fuel of a neighbor's case | ten times the seed's on that case, within 1,000,000 and 10,000,000; 1,000,000 when the seed ran out |
| fuel of a `TestCase` run | its declared fuel, at most 10,000,000 |

Cases run under the `call` limits otherwise. The wall limit is checked
before each case run, `TestCase` run and neighbor, and generation stops at
it; a run in progress finishes. The granularity is therefore one run: a
command can exceed the limit by at most the run in progress (its fuel
cap) and one neighbor's compilation. The output reports the bound and the
time actually taken.

An attempt is one accepted head of a workspace: every search on it counts,
whatever its seed, function or draft, so trying the same frame again or
chaining `try --on` gives no more searches, and only a commit (a new head)
starts a new attempt. Each search claims one slot file,
`.sley/search/<head transaction>-<n>.json` with `n` from 1 to 2, by
linking a complete file into place, which fails when the slot exists;
concurrent searches therefore never share a slot or exceed the bound. A
third search is refused with `AGENT_SEARCH_SEED_INVALID`. The use is
claimed only after every other check passes: a refused search uses
nothing.

The output states the seed's own case results, the neighbor counts
(generated, kernel-valid, refused, evaluated, partial, not evaluated,
skipped), the rule, each neighbor's generator, place, change, edit size,
frame, verdict and per-case results (`pass`, `fail`, `resource limit`, or
`unknown` when a case cannot run), its `TestCase` results (or that no
`TestCase` targets the function), the limits reached (the neighbor limit
when more neighbors exist, the wall limit with what it stopped), local wall
time, the thread's CPU time and the process's peak resident memory where
the platform reports them (else `not measured`), and this sentence:
verified means kernel-valid and evaluated against the stated public cases,
not proof of correctness for all inputs. Text lists the top five ranked
neighbors; `--json` lists all. For the same seed and cases the output is
identical except for the resource measurements and the use slot. The
`next:` line names the `TestCases` of the function the proposed neighbor
fails, after a `# caution:` on the same line. Exit status is 0 when the
top-ranked neighbor passes every public case and every `TestCase` of the
function, else 1.

The events ledger line of a search carries `search_neighbors`,
`search_valid`, `search_evaluated` and `search_exhausted` among its `afx`
counters. Its `input_bytes` is the size of the public case file whenever
that is a readable file, whether the search runs or is refused; otherwise
it is the command line, as for every command (section 12.8).

`crates/sley-agent/tests/search.rs` executes this section: each generator
on a small program, a value without a same-typed substitute, the neighbor
limit and the wall limit, a looping neighbor stopped at its fuel cap, AF1-X
neighbors that layer on their draft, the head seed, the seed and case-file
refusals, runnable-case checks, determinism, the ranking rule, the
per-attempt use limit (repeated frames, long `try --on` chains, concurrent
searches), repeated flags, unstateable and derived code skipped before the
budget, nested failure routes kept, negations that would duplicate
evaluation skipped, the wall limit's granularity, `TestCase` evidence,
callers an `arity` intent rewrites searched and repaired through the
intent, callers of an equal-count `arity` intent read as the seed reads
them (and skipped when the intent cannot read them), substitutions that
remove the use typing a value, functions a `guard` intent rewrites
skipped, the ledger counters and `input_bytes`, the `search` help example,
and wrong opcode, constant, return value and switch edge repairs applied
with `try --on`.
