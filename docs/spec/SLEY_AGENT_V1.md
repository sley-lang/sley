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
sley-agent try <frame|ops> [--no-test] [--all-tests] [--public <cases.json>] [--raw]
sley-agent submit [<ref>]
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
reports running zero tests.

`submit` validates the referenced candidate against the current head again
and writes `final_candidate.hex` only when it is Valid. Submissions repeat:
the last Valid submission wins. `status` renders the submission's affected
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
scoped to their function. Block parameters and operations are scoped to
their block, and never shadow a function parameter. Qualified names join
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
One refusal lists every operation of a function that cannot resolve, and
then every terminator that cannot, one pointer per line, so a misplaced
name costs one round rather than one round per use. A name found only in
another block is reported as that block's parameter (pass it on as an edge
argument) or result (qualify it as `block.name`); a switch case key that is
not a case of the scrutinee's type names the expected keys; and an operand
that is a literal or a nested operation names the fix.

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
used as written, and a limit above the grant is the kernel's
`CANDIDATE_TEST_RESOURCE_LIMIT`.

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
runs only after a kernel refusal and only describes it.

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
| `AGENT_SUBMISSION_REFUSED` | the candidate is not Valid, or the transaction engine refused a commit |
| `AGENT_IO_FAILED` | a workspace file could not be read or written |

## 10. Execution (advisory)

`call`, `test`, `try` and `status` project the selected state with
`project_complete_entities`, build its type environment once, lower each
function once (`lower_function`, extended profile) and execute every input
with `execute_loaded_image`. `call` runs under generous fixed ceilings
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
JSON-pointer errors and one refusal per frame round; batch streaming;
name-matched redefinition; every guide example, every JSON example line of
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
