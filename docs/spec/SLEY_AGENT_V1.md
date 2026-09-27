# Sley Agent Workbench v1

Status: Sley 2.0.2 contract, revision 1 (2026-09-25), implementing
`SLEY-2.0.2-BR` Track A (BR-01 through BR-12). ADR-0051 records the
boundary decisions. The implementation is `crates/sley-agent`, binary
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
sley-agent view [name...] [--package] [--after <ref>] [--ids] [--types] [--limits]
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
sley-agent init [<dir>] [--seed <64-hex>]
sley-agent commit [<ref>]
sley-agent export <file.pack>
sley-agent help [guide|af1|types|tests|opcodes|refusals]
```

A `<ref>` is a handle (`c3`), `latest`, a file holding stored candidate
hex, raw stored candidate hex, or a bare candidate record (which is framed
through the kernel). `--json` makes every command print one JSON object.
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
written to `.sley/layered.json`, so refusal pointers refer to it. The
layering rules:
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
with `AGENT_USAGE_INVALID`.

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
then notes that no TestCase targets a function it changes. Submissions
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
   namespace, or the only namespace when there is exactly one.
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

## 6. Opcodes

The mnemonic table maps every epoch-1 opcode one to one:
`const tuple tuple_get record field variant variant_get vec vec_len vec_get
vec_set map map_get map_has map_insert map_remove add sub mul div rem neg shl
shr fadd fsub fmul fdiv fneg fma eq ne lt le gt ge not and or call some none
ok err assert observe effect adapter narrow cell cell_get cell_set hash global
fnref`. AF1 also accepts the SSMC1 names and the numeric tags.

## 7. TestCase defaults and the workbench policy

An AF1 test that declares no limits takes, per limit, the smaller of the
workbench default and the grant: fuel 1,000,000; memory 16,777,216 bytes;
output 65,536 bytes; effect count 0. Call depth is 256 and wall time is
10,000 ms (these are context limits, not grant limits). A declared limit is
used as written. For the TestCases a candidate selects (those that target
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
| `AGENT_DRAFT_STALE` | the revision a `fill` names is not the draft's latest revision |
| `AGENT_DRAFT_HEAD_CHANGED` | the accepted head changed since the draft revision; build on the new head explicitly with `--rebase` |
| `AGENT_DRAFT_INCOMPLETE` | the draft revision has no complete, Valid candidate for the request: a text revision cannot be layered on, and only a `valid` revision is submitted |
| `AGENT_DELTA_INVALID` | a delta has another shape, or a target that is malformed, missing, given twice or overlapping another |

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
`draft.json` names the latest revision and the base head of `r1`. Each
revision directory `rN/` holds:

| File | Content |
|---|---|
| `input.txt` | the exact input bytes (the frame, delta or case file given) |
| `frame.json` | the complete authored frame of the revision (layered, or with the delta applied); absent when the input is not JSON |
| other `*.json` | derived authoring artifacts of the compiled frame |
| `status.json` | `revision`, `base_head`, `parent`, `made_by` (`try`, `try-on`, `fill`, `import` or `rebase`), `on`, `delta`, `whole_frame`, `state`, `candidate`, `candidate_sha256`, `verdict`, `obligations`, `tests`, `sources`, and `results` when tests ran |

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

`try` of a frame starts a new draft (`dN@r1`). `try --on <draft>` layers
the frame on the frame of the draft's latest revision, or of the revision
`dN@rK` names, with the layering rules of section 2, and records the next
revision of that draft; its `parent` is the revision layered on. A
follow-up therefore states only what it adds or changes, and the tests of
earlier revisions stay in the frame until they are replaced or removed.
`try --on <handle>` keeps its section 2 meaning and starts a new draft. A
text revision cannot be layered on (`AGENT_DRAFT_INCOMPLETE`, with the
`fill` that replaces it whole).

### 12.2 Trial output

A frame refusal keeps exit status 2, its `error AGENT_*:` line and its
problem lines. It then says where the pointers point (the frame file to
edit in place, `.sley/layered.json`, or the revision's `frame.json`), and
adds the draft line (`draft d1@r3: incomplete, 2 obligation(s)
(AGENT_FRAME_INVALID 2); list: sley-agent draft d1 --obligations`) and a
`next:` line with the `fill` that repairs it in place. Input that is not
JSON gets the draft line with the parser location. In JSON the refusal
object adds `draft`, `state`, `obligations` and `next`.

The first line of a candidate's result ends with its draft revision
(`c4: Valid (+3 created, 1 replaced, 0 deleted) draft d1@r2`). The result
then lists:

- `changed:` the functions, types, constants and TestCases the candidate
  creates (`+`), replaces or changes a part of (`~`), or deletes (`-`), by
  kind and name (`changed: fn ~bound +area; type +Shape; test +t1`);
- `exported:` the exported functions that existed before the candidate and
  are changed or deleted by it;
- `tests: X/Y passed [authored A, imported I, provided P]` (counts that are
  zero are left out) and one line per failing test; passing tests are
  listed only with `--verbose`;
- `tests: 0 ran` when no TestCase ran, as section 2 describes;
- the `next:` step, which layers on the draft (`try --on d1`), repairs it
  (`fill`) or submits it (`submit d1`).

Unchanged program bodies are not reprinted. JSON output keeps every earlier
key and adds `draft`, `state`, `changed`, `obligations` and `provenance`.

### 12.3 Obligations

An obligation is an unresolved problem of a revision: `{"id", "symbol",
"at", "expected", "available", "decision", "kernel", "count"}`. The problem
lines of a frame refusal become obligations; a line prefixed
`[AGENT_...] ` carries that symbol, and every other line carries the
refusal's symbol. `at` is the JSON pointer into the revision's
`frame.json` (`""` for the whole frame, `null` when the problem names no
pointer). `expected` and `available` are set only when the problem states
an expected type or shape or the values available, and are `null`
otherwise. Problems with the same symbol and decision form one obligation
with their `count`; `also_at` lists the pointers after the first. A kernel
refusal is one obligation whose `kernel` holds the kernel's symbol, phase
and locator unchanged. Obligations report what the author must decide;
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

### 12.5 Head changes

A revision records the accepted head it was made on. When the head has
changed since, `fill`, `try --on <draft>` and `import --on <draft>` refuse
with `AGENT_DRAFT_HEAD_CHANGED` unless `--rebase` is given. A rebased
revision records `made_by: "rebase"` and `rebase: {"from_head",
"to_head", "via"}`. Nothing is rebased implicitly.

### 12.6 Submission

`submit <draft>` submits the candidate of the draft's latest revision, and
`submit <draft>@r<N>` that of revision `N`. The revision must be `valid`,
and the stored bytes of its candidate must match the recorded SHA-256;
otherwise the submission is refused with `AGENT_DRAFT_INCOMPLETE`, which
names the revision's state. The candidate of an earlier revision is never
used in place of a later one. The candidate then takes the unchanged
submit path, including the refusal of an untested function change.
`submit <handle>` is unchanged.

### 12.7 Test import and provenance

`import <cases.json> [--on <draft>] [--only a,b]` reads public cases
(`[{"name", "function", "args", "expect"}]`; a case without a name is
`case<i>`) and records a revision whose frame adds them as AF1 `tests`
named by their cases: layered on the draft with `--on`, or as a new draft.
`--only` selects cases by name. A case without `expect` is refused with
`AGENT_INPUT_INVALID`: expected values come from the case file, never from
running a candidate. `status.json` records the file's SHA-256 and case
count (`import`) and, per imported test, its case, the file's SHA-256 and
a digest of the test entry (`sources`). The frame carries no provenance.

Tests are counted by origin (`tests` in `status.json`, `provenance` in
trial JSON):

- `provided`: the TestCases live at the base head that the candidate
  keeps;
- `imported`: frame tests whose entry is unchanged since their import;
- `authored`: the other frame tests, plus the rows of `test_tables` when
  the frame has them.

An imported test that the author restates counts as authored from then on.
Imported tests never satisfy a requirement to author a test.

### 12.8 Events ledger

Every command that uses a workspace appends one JSON line to
`.sley/events.jsonl`, with exactly these keys: `seq`, `cmd`, `draft`,
`candidate`, `input_bytes` (the frame, delta or case file read, otherwise
the command line), `output_bytes` (what the command printed),
`whole_frame`, `delta_targets`, `delta_bytes`, `afx` (the authoring
feature counters of the compiled frame), `table_rows`, `tests` (the
provenance counts), `refusal` (the workbench or kernel symbol),
`obligations` and `valid`. A line holds counts, handles and symbols only:
no clock, no names and no program content. A failure to append never
fails the command, and `help` and `version` append nothing.

`crates/sley-agent/tests/drafts.rs` executes the contract of this section:
stale revisions, changed heads and explicit rebases, missing, repeated and
overlapping delta targets, recorded whole-frame replacement, text drafts
repaired whole, monotonic candidate handles bound by digest, tests
inherited across one-operation corrections, name collisions across
revisions, the refusal to submit after a newer incomplete revision, import
provenance and digests, grouped and bounded obligations, and ledger lines
without content.
