# Sley Agent Workbench v1

Status: revision 3 (2026-09-28, implementation in progress). Revision 1 (2026-09-25) was the Sley
2.0.2 contract implementing `SLEY-2.0.2-BR` Track A (BR-01 through BR-12).
Revision 2 adds decision-only authoring: the AF1-X dialect and test tables
(sections 5.1 and 5.2), drafts and delta repair (section 12), focused and
AV1-X views (sections 4.1 and 4.2), the namespace opt-out, and authored
locators. ADR-0051 records the boundary decisions and ADR-0053 the
revision-2 decisions. Revision 3 registers the GHOSTWEAVE residual request
envelope, strict parser, read-only state binding, and deterministic fragment
expansion APIs, with `residual try/plan/fill/show` using ordinary trials and
an accepted/draft-graph integer-literal edit lens. Further sparse edit lenses,
independent factoring, aggregate resource enforcement, and full cost accounting
remain in progress. Explicit author-declared finite relations now use a
checked greedy frontier. The implementation is
`crates/sley-agent`, binary `sley-agent`.

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

Candidate handles are reserved by publishing the complete `.hex` bytes before
writing the optional `.json` summary. A metadata-write failure can therefore
return `AGENT_IO_FAILED` while leaving a complete candidate readable under its
handle. A later command skips that reserved handle and allocates a new one;
missing metadata does not stand in for candidate validity. This is not atomic
two-file publication or a power-loss durability guarantee. If the final `u64`
handle is occupied, allocation returns `AGENT_IO_FAILED` without wrapping or
changing existing records.

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
sley-agent residual try <request.json> [--no-test] [--all-tests] [--public <cases.json>] [--raw] [--verbose]
sley-agent residual plan <request.json>
sley-agent residual fill <rN@1> <decisions.json>
sley-agent residual show <dN@rK | rN@1> [--expanded | --decisions | --provenance | --targets]
sley-agent init [<dir>] [--seed <64-hex>]
sley-agent commit [<ref>]
sley-agent export <file.pack>
sley-agent help [guide|af1|afx|afx-quick|afx-reference|drafts|types|tests|opcodes|search|residual|residual-quick|residual-reference|refusals]
```

`help residual` and `help residual-quick` return the compact fragment guide.
`help residual-reference` loads the full planning, repair and provenance contract
on demand. The compact guide is capped at 3,500 UTF-8 bytes; its examples run
through the ordinary workbench and kernel in the help tests. This byte bound
does not establish a model-token or cost reduction.

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

### 2.1 GHOSTWEAVE residual request parser

The Rust workbench API `sley_agent::residual::parse_request` accepts a UTF-8
JSON object with exactly these required members: `residual` (integer `1`),
`base`, `operation` (`derive` or `edit`), `fragment`, `bindings`, and `scope`.
`preserve` is forbidden for `derive` and required for `edit`. Optional
`choices` declares a versioned, closed relation over missing fields (section 2.5);
it is not a sampled candidate list.

`base` is `current`, a 64-digit lowercase accepted-state root, or an explicit
draft revision such as `d1@r2`; a latest-only draft reference is refused.
`fragment` is exactly `{"id": <lowercase-id>, "version": <positive-u32>}`.
`bindings` is an object. `scope` is an exact-target array or a typed-selector
object. `preserve`, when present, is an array or object. Fragment versions
must validate the nested binding, scope, and preservation schemas before any
expansion or state write.

The parser rejects duplicate object members at every depth, unknown envelope
and fragment members, floating-point values, invalid UTF-8, and requests over
1 MiB, 32 container levels, or 100,000 JSON values. It does not bind a plan,
expand a fragment, write workspace state, create a candidate, or add a CLI
command. Those paths remain subject to the GHOSTWEAVE feature specification
and must reuse the existing AF1-X, draft, and kernel flows.

### 2.2 GHOSTWEAVE state binding API

`residual::binding::Binding::capture` snapshots an already initialized
workspace without seeding it or creating local directories. It binds the
strict request, canonical workspace path and identity, accepted transaction,
state and policy roots, epoch, both name-map files and their merged mapping,
and an explicitly selected draft revision. A draft must still be latest,
be based on the accepted head, and contain a layered frame. Incomplete and
refused drafts remain usable for repair; text and unlayered drafts do not.
Draft input, frame, status, and any recorded candidate bytes are bound.

The caller supplies a `RuntimeIdentity` from the actual running implementation:
fragment and grammar manifests, expander build identity, semantic profile,
and optional cost-model/tokenizer versions. Request data cannot substitute
for these values. The CLI constructs this identity from the shipped fragment
manifest, compiled-in grammar contracts, and a streamed SHA-256 digest of
the actual running executable (cached within that process). Linux uses
`/proc/self/exe` so replacement of the installation path does not substitute
another build. The cost profile is `disclosed-field-json-bytes-v1`; the
tokenizer version is null because no provider token estimate is made.

Capture requires two matching observations. `Binding::recheck` captures
again and refuses changed or unavailable dependencies with
`AGENT_RESIDUAL_BINDING_STALE`. Hashes use domain-separated SHA-256 and
length-delimited, typed canonical encoding; booleans and integers stay
distinct. Per-artifact reads are capped at 16 MiB (`MAX_BOUND_ARTIFACT_BYTES`,
16,777,216 bytes). Raw artifact hashes
conservatively invalidate formatting-only changes.

A binding is an integrity record, not a lock or authority. The CLI rechecks
before expansion, before claiming a draft, and after ordinary compilation
immediately before candidate assembly. It also checks that the trial's head
is the captured head. Existing draft revision and kernel mutation checks
remain authoritative. Both explicit-decision and closed-relation plans persist
this binding, including the complete original relation and its constraints.

### 2.3 GHOSTWEAVE fragment expansion API

`residual::fragments::expand` deterministically lowers a derive request to
ordinary AF1-X. It returns the entire frame and a JSON-pointer provenance
map; it reads no files and does not validate, store, submit, or commit a
candidate. Callers must use the existing compiler and kernel. Full expression typing,
exhaustiveness, effects, and control-flow compatibility remain compiler/kernel
checks. The CLI now performs the initial interface preflight below before
calling the structural expander; direct users of this low-level expansion API
must arrange their own bound interface check.

All three version-1 families require exactly one function name in `scope`,
explicit AF1 `params` and `returns` in `bindings`, and the following
family parameters. Nested applications carry `fragment` and `bindings`
without another function signature.

| Family | Parameters and disclosed behavior |
|---|---|
| `ordered_guard_chain` | Ordered `guards`, each with `when` and an explicit `fail` terminator; `success` is a nested fragment or an explicit `{"ops": [...], "term": [...]}` region. Evaluate guards in written order; the first true predicate selects its failure exit. |
| `checked_pipeline` | Ordered `steps`, each `[name, expression]`; explicit `arithmetic_failure` (error variant case name or `{"propagate":true}`), `rounding`, and `result`. Check each arithmetic node, preserve nesting and step order, map failures to the supplied route, and package the final value with `ok`. Version 1 accepts only `toward_zero` rounding and integer `neg/add/sub/mul/div/rem/shl/shr`. Widths come from explicit typed operands/signature under ordinary AF1-X inference. |
| `typed_branch_result` | `input`, ordered `cases`, and `join`. Each case supplies `case`, `payload` (null or `[name, type]`), `ops`, and `values`. The join supplies `params`, `ops`, and an explicit `return`, `ok`, or `fail` terminator. Evaluate the switch once, execute only the chosen case, and pass its values to the shared join. No fallback case is invented; inputs with a resolved parameter or direct-call result type require exact case coverage before expansion; other computed inputs retain compiler/kernel obligations. |

`residual::interfaces::check` resolves declared function parameters/results,
branch payload annotations and join parameter types through the existing
AF1-X `Context` and type reader, using accepted declarations or the exact source
draft frame. Duplicate parameters report both authored locators. Nested guards
share the outer signature; a pipeline requires `Result<integer,error>` and bare
arithmetic propagation must preserve `ArithmeticError`. For Option/Result/named-variant
inputs whose parameter or direct-call result type is known, branch cases must
cover exactly the bound variant and payload declarations must match the selected case. Duplicate,
unknown or missing cases refuse with authored locators. Record and scalar
inputs are not switch variants. Source-draft member type errors are checked
using the existing AF1-X declaration obligations before member inspection.
Branch values with resolved types must match join types and arity even when
the case contains authored operations. Types are compared as resolved
`TypeExpr` values, not text spellings.

Before expansion, the complete inventory of function parameter/result, branch
payload and join parameter types also undergoes the canonical closed-type
judgment, including unused bindings. Reachable accepted/draft definitions are
hydrated using the shared invocation budget. Free type parameters, incorrect
named argument counts, definition cycles and invalid canonical map-key types
refuse at the declared type's locator. Resource/depth failures remain
`AGENT_RESIDUAL_LIMIT`. An incomplete or unavailable definition used by a
known interface refuses with `AGENT_RESIDUAL_CONSTRAINT_CONFLICT` at the bound
type's locator before fragment expansion. The diagnostic identifies the type
and asks for ordinary AF1-X declaration repair; it never invents a shape or
payload. Independent known-invalid declarations are still checked rather than
hidden by an unavailable definition. Successful reports have checked declared
types and an empty `deferred_interface_types` list. This inventory does not
establish body validity or replace operation-specific VM restrictions.

Checked pipeline preflight separately queries the ordinary checked arithmetic
AST with original `/bindings/steps/<index>/1/...` and `/bindings/result`
locators. Pipeline type equalities do not supply literal contexts that ordinary
AF1-X lacks. `ordinary_literal_contexts`, `literal_contexts` and
`untyped_literal_contexts` disclose that distinction. Library analysis retains
missing contexts; the CLI refuses them before expansion or draft creation.
An explicitly typed operand, an actual parameter context, or a direct `ok`
endpoint can supply a valid ordinary context. No literal is annotated or
rewritten, and no fragment blocks or graph entities are generated by this query.

Explicit authored operation result annotations, bound direct-call parameter and
result types, and constant/global/function-reference types also receive canonical
closed-type checks. Unused results and unresolved argument expressions do not
bypass these checks. `closed_types` expression evidence records the use location,
type source and `checked` status. Incomplete definitions refuse at the authored
use or annotation locator even when a result type is known. Annotation
checks use the original `/type` locator and apply to the wrapped result of a
checked operation. A valid annotation does not establish the validity of an
unknown operation or a declared callee body. These checks use the invocation
budget and preserve canonical depth/resource refusals.

Before generation, parsed value operands in authored definitions and patched
blocks also check local declaration order and qualified parameter ownership.
Self/forward reads, including indexed and nested uses, retain original use and
declaration locators; local names shadow function parameters. Another block's
parameter or AF1-X checked continuation requires an explicit edge argument.
Literal contents, opcode immediates and type annotations are not scanned as reads.
`declared_body_control_flow.value_availability_scope` and per-function `value_reads`
record this partial scope. Complete current authored/retained block-start graphs
now project the entry, ordinary successors, checked handlers and early exits.
Qualified operation uses check dominance at symbolic continuation slots for
checked/nested operations and early exits, including definitions after a split.
Exact retained canonical operation and terminator operands consult current
entry/types without rebinding accepted IDs/indexes. Per-function
`control_projection` records named starts, symbolic sites and checked authored/
retained reachability declarations. Missing/non-bool flags keep ordinary AF1's
required interpretation. Canonical shape recognition excludes unreferenced old
generated shared-exit bodies from unchanged-consumer checks; future allocation/
reuse stays deferred. Generated-looking user blocks keep their flag obligation.
Known bare AF1-X names select the nearest dominating operation at symbolic sites;
nearer parameters hide outer operations and intervening same-name declarations
refuse path rebinding. Known foreign bare names in plain AF1 require qualification.
Result-index spelling follows the ordinary unsigned-u32 parser; known authored
and retained operation cardinalities are checked. Recognized fragment-region
operations retain their definition and result kind even when their value type
is unresolved: nonzero ordinary-operation indexes refuse before expansion;
checked continuation parameters retain the ordinary parameter-reference rule.
Unknown operation syntax and unresolved value types remain deferred in library
analysis. CLI fragment expressions must resolve before expansion; an unresolved
connection requires complete supported authoring or the ordinary AF1-X path.
Source-helper graph and generated-identity limits remain separately reported.
Valid numeric parameter
suffixes preserve ordinary parameter-reference identity. Source condition,
selector, edge and function-exit checks consume resolved read-site types; nested
result inference uses the same read-only type map. Retained operations read the
exact accepted result slot. Unknown references retain deferred types. Ordinary
inference parses the same numeric selectors and keeps the declared type of
parameters, including checked continuation parameters. Operation references
retain result bounds; an unavailable local result cannot fall through to a
same-named function parameter.
No hint or authored reference is rewritten. Missing/ambiguous graph views,
unknown availability, generated shared-terminal identities/
flags and full expanded inventory remain deferred; this is not a full expansion/
body-validity proof.

Source-definition and retained-patch exits also connect known return values to
the current function result before generation. Trap payloads receive the canonical
closed-type and persistability judgments through the same budgeted reachable
accepted/draft definition environment used by fragment checks. Retained consumers
keep accepted value IDs/result indexes and use the current declaration overlay.
AF1-X `ok` requires Result; bare `fail` keeps Option None behavior, and nongeneric
named Result failures check exact ordinary case leaves and payload presence/types.
Generic failure construction and incomplete/unknown type views remain deferred.
Plain-AF1 sugary terms and lenient trap shapes retain ordinary compiler obligations;
trap codes are not rewritten. `declared_body_control_flow.function_exit_scope` and
per-function `terminator_inputs` record this partial scope. Known types do not
prove literal admission, value order/availability, dominance or generated CFG.

Guard exits check the return interface: bare `fail` requires Option; named
failures require a case of the Result's named error variant, with the correct
payload presence. Parameter, primitive literal and direct-call payload
connections must match its type; other computed payload values stay deferred.
Named arithmetic mappings either drop
the native error into an explicitly selected unit case or preserve it in a
case carrying exactly `ArithmeticError`. A checked-pipeline fragment creates
no authored handler blocks; use ordinary AF1-X to author such handlers.
`connections` evidence records checked branch coverage and error routing,
and marks coverage of branch inputs with unresolved types as deferred. The branch's
64-case ceiling applies to exhaustive branching, not to looking up one error
case in a larger variant. All member/case checks share the invocation budget.

Named branch coverage, guard/terminal failures and arithmetic mappings require
a complete bound variant definition before claiming a checked destination.
Malformed or duplicate draft members never establish an exhaustive member list
or a unit payload. A partial replacement masks the accepted definition and a
known fragment interface using that incomplete type refuses before expansion.
Authored join values and failure payload expressions are still checked
independently; no destination or payload is invented. Nested fragments retain
their authored locators. Repair the bound declaration through ordinary AF1-X,
then retry composition against the resulting exact draft revision. Unknown
expression types remain deferred and cannot establish validity.

Guard predicate preflight checks direct parameter reads, boolean literals,
typed primitive literals, boolean operators and comparison type connections.
Boolean operators require boolean operands; comparisons require matching operand
types. Ordering admits only bool, integers, bytes, text, f32 and f64, matching
the VM scalar comparison profile. Equality checks canonical hashability and the
VM's comparison judgment, including its composite-float exclusions. Reachable
accepted/draft named definitions use the same bounded hydration as map-key
checks; known interfaces with malformed or incomplete shapes refuse. Literal values
are checked with the existing typed-value reader when a type is explicit or
supplied by a known comparison partner. Both operand orders receive that context.
Unchecked integer arithmetic cannot directly supply a boolean condition.
Unknown names and conflicting connections refuse at the authored expression use.

Reference operations check arity and resolve constants, globals and functions
through the bound interface. Named constants use accepted/draft types;
typed `const` immediates use the existing value reader. Bare numeric `const`
immediates use the shared ordinary immediate-type rule and read-only AF1-X
use-site inventory: direct returns, target-edge arguments and direct call
arguments supply hints; comparison partners and constructor payload contexts
do not. Within an emitted piece, terminal hints precede calls. Earlier pieces
retain priority across checked operations or conditional exits; the first hint
wins, and every edge argument is lowered before its final terminal hints.
The same ordinary first-use inventory covers unannotated Option/Result
constructors. Named `none`, `ok`, and `err` results retain their emitted type
at later uses: conflicting return/call widths refuse before expansion. Terminal
uses precede calls within a continuation piece; earlier pieces precede later
ones. Checked outputs remain continuation parameters rather than direct results.
Empty vectors and maps use the same direct emitted hints when no operand fixes
their types. Short variant constructor cases resolve through their hinted named
result. Nonempty containers and qualified variant members keep their operand or
definition types, so use hints cannot silently retag them.
Explicit annotations override use hints. Without an integer hint, an untyped
numeric immediate is i64. The read-only inventory creates no operations/blocks
and shares the invocation budget. This resolves constant connections, not all
ordinary AF1-X body inference: internal inference and emitted-use hints remain
distinct, and unchanged ordinary typing refusals still apply.
Literals with complete type definitions validate nested
contents through the ordinary typed-value reader and canonical constant checker.
This includes tuples, vectors, maps, options, results and named records/variants.
A read-only definition adapter binds accepted or complete draft members; draft
member identities are never published. Partial replacements mask accepted
definitions; unavailable definitions for typed contents refuse before expansion.
Each recursive value read
charges the shared work budget, with checkpoints around canonical checking.
This does not establish aggregate allocation accounting or evaluate expressions.
Explicit integer context checks primitive range constraints.

Terminal regions close with `return`, `ok`, `fail`, or `trap`. Authored `br`,
`jump`, `cond`, and `switch` terminators refuse before expansion: these regions
expose no named block targets. Use fragment composition or ordinary AF1-X for
named control flow. Typed branch joins retain their stricter `return`/`ok`/`fail`
contract. Unknown or malformed terminal forms also refuse at their authored
location.

A trap has an optional code (default `unreachable`) and optional payload; extra
items are refused. The existing trap-code vocabulary is reused. Payload
expressions receive the same scoped type checks as other expressions, followed
by the canonical closed-type and persistability judgments using bound named
definitions. `payload_eligibility` distinguishes `absent`, `persistable_checked`,
`definition_deferred`, and `unresolved`; `payload_interface` retains independent
expression obligations even when the result type is known. Terminal evidence
describes the absence of successor edges, not whole-function CFG validity.
Checked failures inside a payload retain their written evaluation order and
failure route. Preflight does not execute the trap or claim kernel validity.

Global reads check the initializer's constant identity and exact type, including
draft replacement types. Deleting and recreating an initializer's name does not
retarget an existing global. Function references preserve accepted parameter,
result and effect types; accepted generic functions refuse. Declared/redeclared
function references use AF1's emitted empty effect declaration, including
recursive references to the residual target. Their parameter/result types must
still resolve; an incomplete signature stays deferred.
Per-location `references` evidence records source, target, result type and known
function effect identities. These checks create no constants or runtime values.

Value hashing checks one operand and its canonical hashability through the
existing type checker, using reachable accepted/draft named definitions. The
VM's structural exclusion of local cells also applies, including cells inside
function-reference signatures. Hashing permits hashable float composites; the
equality operation's separate float-composite restriction does not apply.
Its result is always bytes, even when operand typing remains unresolved. That
known result neither supplies an input integer width nor hides deferred operands
or incomplete definitions. Checked propagation on a hash refuses because its
result is not Option/Result. Per-location `hashes` evidence records operand type,
eligibility and result type with `evaluated: false`; preflight computes no digest.

Direct calls in guard predicates, branch inputs, failure payloads and authored
regions resolve through the same AF1-X context. Preflight checks argument count, known parameter
types, typed primitive literal ranges and return connections. Nested calls share
the invocation budget and expression depth bound. Accepted signatures, draft
declarations and patches are distinguished; deleted or shadowed names cannot
fall back to an accepted function signature. Calls to the residual target use
its authored parameter order/result, including when replacing an accepted
function. No argument is converted, reordered or supplied implicitly.

Accepted callee effect sets must be empty, matching the generated function
interface. AF1 definitions, patches and operation edits that restate their owner
emit an empty effect declaration, so draft
and recursive callee interfaces report `empty_declared_effect_set`; unchanged
accepted callees report `empty_accepted_effect_set`. This rule follows emitted
compiler metadata rather than inheriting an old function's effects. It does not
establish that a draft body obeys the declaration: `effect_body_validation`
retains that ordinary compiler/kernel obligation. The call
report records callee, authored location, signature source, result and effect
evidence. A known return type does not hide deferred arguments. Scoped authored
regions use the same call checks.

Bound callee signatures must be complete before connecting fragment calls or
function references. The read-only AF1-X context retains declaration errors
from the ordinary compiler's shared parameter/result readers. Malformed
parameter lists, duplicate or invalid parameter names, unresolved parameter or
result types, and missing full-definition `returns` refuse at the authored use
with the canonical source declaration locator. Missing full-definition `params`
still means zero parameters; omitted patch signature fields still inherit the
accepted interface. The residual target's replacement interface supersedes its
old declaration. Calls in surviving source helper bodies also require complete
bound callee signatures. Refusal leaves source draft/candidate/name state intact
and retains the ordinary refusal event; repair the source using ordinary AF1-X.
This signature check does not prove unknown body expressions; constant immediate
widths use the separate ordinary use-site inventory described above.

Surviving authored source definitions and patch blocks also expose
`operation_signatures` in the control-flow report. A read-only type view reuses
ordinary literal/nested contexts, direct emitted first-use hints, and resolved
operation ownership. Complete immediate-free and tuple-index operations use the
canonical VM signature judge. Direct calls and function references use that same
judge with bound callee parameter/result/generic/effect headers; this projection creates
no candidate graph or allocated identity. Known operand, arity or result-type
conflicts refuse at the operation locator before residual revision publication.
Bound record, field and variant operations use definition-specific member query
keys; valid restatements preserve existing member identities. Named constants
and globals use real constant data and initializer identity. Known literal and
declared constant data use the ordinary value reader and canonical constant
checker, including range and endpoint contexts, before expansion. These checks
allocate no published identities. Retained accepted operations and unchanged callers are also checked against
current parameter, callee, nominal member, constant and global bindings. Their
actual operand/result/immediate identities are kept; deleting/recreating a name
does not retarget its old consumers. Authored replacements expose their emitted
value types to retained reads, preserving ordinary constant-use hints and split
identity rules. Other immediates and unknown authored types/literal contexts
remain explicitly deferred.
This does not establish complete body conformance or alter ordinary authoring.

`declared_body_effects` separately checks complete `fns`/`functions` definitions
in a bound source draft before fragment expansion. It reuses the AF1-X parser
to inspect named and nested operations in statements, conditional exits and
terminator operands, without interpreting literals or opcode immediates as
code. Every complete definition's calls must connect to an empty bound effect
interface, including helper chains and mutually recursive declarations. Known
excluded opcodes refuse at the original source locator. Each body is scanned
once per preflight with the shared budget; recursion does not expand callees.
The residual target's replaced old definition is not checked as if it survived.

`declared_body_control_flow` separately checks explicit authored successor and
entry targets in complete source definitions and patches. It reuses the ordinary
AF1-X terminator parser without generating blocks. Duplicate block declarations
and function-parameter/block namespace collisions refuse at the original source
locator. Explicit targets must belong to the surviving source function. Patches
check retained accepted terminator targets after authored replacement/deletion
and reuse the existing generated-piece removal recognizer. The inventory and
edge traversal charge the same invocation budget.

Deleting an entry block can be valid ordinary AF1: explicit replacement is
checked, while implicit replacement by the first remaining block stays deferred
to the ordinary builder. Targets containing `__` in AF1-X remain deferred because
actual expansion can regenerate, remove or collide with those names. Later
Ripple/edit transformations, incomplete bodies and unknown terminators retain
explicit deferred evidence. The replaced residual target's old body is skipped.
`explicit_targets_checked` covers this inventory only; `composition` remains
`partial`. Generated edges, dominance, reachability and full kernel
control-flow/ownership judgment are not established by this pass.

For complete parsed source definitions, `edge_arguments` connects explicit
branch/conditional/switch arguments to destination block parameter count and
known types before generation. The view reuses AF1-X's preparatory declaration
and worklist inference, nested expression contexts and switch payload rules;
no lowering or generated block is invoked. Both passes share the invocation
budget through fallible checkpoints; ordinary expansion uses infallible wrappers.
Plain AF1 requires exact argument count, while AF1-X rejects surplus arguments
and leaves omitted trailing arguments to ordinary X4 derivation. Conflicts name
the authored argument locator and destination parameter's declared type.

Unknown references/types, nonzero result suffixes, literal admission, parser
failures, implicit fills and unresolved generated value identities remain deferred.
Shared far-result inference without a dominator tree establishes a common type
only across agreeing candidates, not dominance or value availability. Known
argument types do not establish expression correctness or literal bounds.
`edge_argument_scope` records these limits; the report retains `authority: none`
and `composition: partial`.

Accepted patches also connect explicit authored edges and retained canonical
edges to the post-patch destination arity/known types. A preparatory declaration
view overlays function/block parameters and ordinary operation result types;
retained values keep their accepted entity IDs and exact result indexes. The
ordinary builder can reuse an identity by its existing owner/leaf and role.
Deleting a parameter/operation, replacing its role or moving an operation into
a generated continuation does not move or silently rebind the old identity.
Conflicts name the retained consuming block and original value. Potential
regeneration/collisions at generated-looking names remain deferred.

Retained canonical edges require exact argument count in both AF1 and AF1-X;
X4 fills only authored trailing arguments. Option, Result and nominal switch
payloads use the current type/member declarations and existing case rules.
Nominal case payloads substitute the scrutinee's ordered type arguments before
comparison, including nested containers and function-reference parameters/results;
function-reference effects remain unchanged. This instantiation does not add
VM support for generic construction operations.
Renaming destination parameters and reordering function parameters preserve
ordinary identity rules; known compatible restatements are admitted. This
establishes edge argument connections. A separate preparatory terminator-input
pass requires known branch conditions to be bool and known switch selectors to
be Option, Result or nominal variants. Complete recognizable case sets reject
missing, duplicate and unexpected keys. Current declaration overlays participate;
retained cases preserve exact member identity under the selector definition,
including nominal cases whose leaves match builtin names. Unknown selectors,
incomplete declarations and unresolved case keys remain explicitly deferred.
These passes do not establish return/trap type connections, literal admission,
value availability/dominance or generated CFG.
Later Ripple/edit transforms and incomplete parser views remain deferred.

Patches of accepted functions check both authored replacement blocks and
surviving accepted operations. AF1-X's existing `LiveGraph` recognizer identifies
generated continuation pieces removed by restating/deleting their source block;
plain AF1 retains those pieces unless explicitly replaced/deleted. Generated
shared exits recognized as pure none/err/variant-return plumbing require no
effect connection. Retained calls keep their original callee entity identity:
deleting/recreating its name cannot retarget the old call. Reports include the
retained operation's identity/name; conflicts identify it alongside the patch
and callee. Signature-only patches check the retained body too.

Patch targets are checked before body inventory. Unavailable accepted owners,
invalid block maps/names, non-object block bodies and deletion of missing blocks
refuse with escaped source locations. New object blocks are allowed. AF1-X can
emit a generated block over a null/raw entry: possible collisions at names
containing `__` remain deferred until actual expansion, with `pending_expansion`
target evidence. Reports include `patch_targets` with source owner/block IDs
(null for absent blocks) and the authored create/restate/delete action. These
source references confer no authority and do not establish the final expanded
block set, typing, ownership or control flow. Signature-only patches have an
empty target list. Target traversal charges the shared invocation budget.

Standalone ordinary `edit` groups on accepted functions are also checked.
Edits of distinct operations are combined per function; old replaced operations
are removed before checking their replacements and surviving calls. The pass
uses the existing AF1-X plain-edit eligibility rule and parser. Replacement
diagnostics point to `/edit/<index>/with`.

Edit targets are checked before replacement inventory. Missing/deleted owners,
missing blocks or operations, malformed `block.operation` paths, missing or
non-operation replacement containers, and repeated targets refuse with authored
locations. Duplicate-target diagnostics identify both edits. A valid target does
not resolve an unknown replacement: its body checks remain deferred. Reports
include `edit_targets` with the accepted owner/block/operation IDs and source
locations; these are references to the source graph, not authority or claims
that replacement operations retain those identities. Target traversal charges
the shared invocation budget.

Ordinary edits restate edited blocks through AF1 authoring. Existing calls in
those blocks resolve their callee names again, while calls in untouched blocks
retain entity bindings. Reports distinguish `restated_operation` and
`callee_binding` (`authoring_name` or `retained_entity`). A `retained_operation`
identity in such a report identifies the source operation; it does not assert
that an operation in a restated block keeps its identity. The edited owner's
signature remains inherited, and its empty emitted effect declaration is
checked against actual ordinary compiler output.

Repeated function definitions/patches and combinations of definitions, patches
and edit groups on one function refuse before expansion, matching the ordinary
compiler's one-restatement-per-function rule. Diagnostics identify both source
locations. Multiple ordinary edits to distinct operations form one edit group,
and transformations on different function owners remain compatible. The source
body being replaced by the residual target retains its existing exception.

Unknown syntax, missing callee metadata, unresolved edit shapes or extended edits,
edits on the residual target itself, and Ripple transformations remain
explicitly deferred. A checked effect
connection is not proof of body typing, ownership, control flow, admission or
execution. The ordinary compiler/kernel obligations remain in force.

The current candidate-analysis profile excludes `contract_assert` (144),
`test_observe` (145), `effect_request` (160), `adapter_invoke` (161) and
`capability_narrow` (162). Residual preflight refuses these with
`AGENT_RESIDUAL_FRAGMENT_SHAPE` at the authored operation before expansion or
publication, including nested uses, aliases and checked forms. The diagnostic
identifies the candidate profile's `CANDIDATE_OPERATION_ANALYSIS_UNSUPPORTED`
exclusion; it does not claim the kernel ran. Some malformed operations and test
observations receive an earlier owner-phase refusal in ordinary authoring. VM
execution support alone does not establish candidate admissibility. The ordinary
authoring path and kernel phases are unchanged.

The opcode table supplies mnemonic, schema-name and numeric-word aliases.
This check does not evaluate predicates or change guard order. Other AF1-X operations,
literal values with incomplete definitions and comparisons with unanchored literals retain exact
`deferred_expressions` locators. A comparison can have a checked boolean result
while its operands remain partially checked. `guard_result`, `expression_types`
and deferred locators distinguish those cases. Per-location `comparisons`
evidence records operand type, eligibility (`checked`, `definition_deferred`, or
`unresolved`) and whether both operand types resolved. Eligibility alone does
not establish that unresolved expressions produce the required operands.
Remaining effects and control flow still require ordinary compiler checks. No expression is
rewritten, no type is guessed and no unknown operation is silently forbidden.

Success regions, branch cases and joins now track locally authored operation
results. Known arithmetic/boolean/comparison/call results and explicit unchecked result
annotations connect to later operands, branch arguments and returns. Array and
object operation syntax retain their actual authored operand pointers. All local
names mask outer bindings throughout the region; an unresolved operation result
never borrows the type of an outer parameter with the same name. Duplicate local
names refuse. Case payloads/results do not leak into sibling cases or the join;
join parameters have their own scope. A separate operand walk checks local
use-before-definition even when the operation's type is unresolved. Forward and
self-references identify both the use and definition; shadowing cannot fall back
to a function parameter. The walk follows the existing opcode immediate schema,
including aliases and checked suffixes. Callee/type/constant/field immediates and
typed literal payloads are not local reads. Conditional exit conditions/payloads
are walked too. Earlier results with unknown types remain available.

The region report records `local_definition_order`, resolved reads and definition
locators, and `deferred_operands`. Unrecognized syntax retains explicit partial
evidence. Local order does not prove
operation validity, handler edges, reachability or cross-block dominance. Those
remaining control-flow checks are still ordinary compiler obligations.

Value operands also receive a region-scope check. A fragment creates private
block names chosen to avoid all authored references, so qualified block operands
and special references cannot reach another region. Such operands refuse before
expansion, with the exact authored locator. Function parameters, current payload/
join parameters and available operation results are the local value vocabulary.
`value_bindings` records the binding and its definition when known. Typed literal
contents and opcode immediates (callee, constant, type and qualified member names)
are not value-reference reads and retain their existing interpretation.

Zero result indexes, including `#0` and `#00`, preserve the local binding type.
Malformed or overflowing indexes refuse. Parameter bindings retain their declared
type for every valid u32 suffix, matching the ordinary compiler's parameter
reference resolution. Evidence records the binding kind and parsed index.
Known checked payloads, including nested operands and terminal/branch/failure
values, also use the existing AF1-X preparatory inference before expansion. If
the authored payload conflicts with the ordinary continuation parameter type,
preflight refuses at the expression locator without creating a draft. Untyped
numeric literals use the actual ordinary operand context. An ordinary literal
with no type context is recorded in the library analysis. The CLI refuses it
before expansion, after other known interface diagnostics, rather than creating
a draft with a speculative width. Explicit literal
and constant types remain authoritative. Failure payloads retain the full
region inventory rather than discarding prior definitions. These read-only
checks neither lower blocks nor change ordinary inference. Broader unresolved
body typing remains separately deferred; this is not whole body conformance.

Checked operation outputs are unwrapped continuation parameters: their known
payload types also survive every valid u32 suffix, reported with binding kind
`continuation_parameter`. An unknown checked payload stays unresolved and masks
any outer binding with the same name; its wrapper annotation cannot supply the
payload type. Shared ordinary AF1-X inference applies the same binding-kind
rule to numeric selectors without rewriting authored references or adding
speculative types. Malformed or overflowing selectors remain unresolved there
and are refused by canonical reference resolution. Other unresolved expressions
can still produce an incomplete ordinary trial.
Recognized ordinary operations reject nonzero result indexes even when their
value types remain unresolved; checked continuation parameters retain the
ordinary parameter-reference rule. Unresolved values keep their local scope
without inventing a type. Unknown operation syntax remains deferred. These checks do not establish the generated graph's complete
ownership, reachability or dominance invariants.

`return` values must match the function result, and `ok` terminators require
Result with the matching successful payload. Named `fail` payloads use the
current region's bindings and distinguish known operation results, parameters and
unwrapped continuation parameters. Option/Result constructors check arity, wrapper type and known payload
connections, including primitive literal ranges. Missing type context remains
deferred in library analysis. The CLI checks unresolved expression locators after
known conflicts and missing literal contexts, then refuses before draft creation
with an ordinary AF1-X escape. Unsupported operations retain their deferred
locators in library reports; an annotation cannot make unknown syntax supported.
A checked
operation's explicit wrapped-result annotation is never mistaken for its
unwrapped continuation value. Unsupported named terminal control flow refuses.
These checks produce no new blocks, operations, inferred annotations or edits.

Tuple construction checks the expected shape and each known element connection,
including nested tuples, empty tuples and contextual primitive literal ranges.
Without a result context, all element types must be known to infer the tuple
type. `tuple_get` checks its operand count, unsigned 32-bit index, tuple operand
type and element bounds. The selected type connects to later operations,
predicates, branch interfaces and returns, or to checked propagation when it is
an Option/Result. Explicit annotations still describe the raw wrapped result.
Mnemonic, schema-name and numeric aliases use the same checks. Every element
is inspected in authored order; selecting one element does not discard checks
or failure evidence from other elements. Unknown elements and unresolved result
indexes retain deferred locators. Result context never erases those remaining
obligations, and no tuple or extraction is evaluated or rewritten.

Named record/variant operations resolve definitions and members through the
bound accepted/draft interface. Record construction checks field count and types
in declaration order; field access requires a qualified member and an operand
of its owning record type. Variant construction checks case payload presence and
type; a short case name needs a named result context. Variant extraction requires
a payload-bearing case and its owning variant, yielding `Option<payload>`.
Checked extraction preserves the authored Option failure route. Record fields
that themselves contain Option/Result can use ordinary checked propagation too.
Generic definitions are outside the VM's supported named-operation profile.

The shared bounded definition walk checks reachable types before using the
member view. Known incomplete/malformed shapes refuse at the immediate's authored
locator. A short variant case without any named context remains unresolved rather
than acquiring a guessed definition. `named_values` evidence
records the immediate, member-check status and raw result type; a known result
does not erase unknown operand locators. These checks neither construct runtime
values nor select a variant case, reorder operands, or rewrite the source.

Vector construction checks homogeneous element types, with context from an
explicit result or any known sibling element. Later typed elements can constrain
earlier literals and nested expressions; unresolved elements retain their own
deferred locators. An empty vector needs an element type from context. Vector
length requires a vector and yields `u64`; reads require a `u64` index and yield
`Option<element>`. Writes also require a replacement of the exact element type
and yield `Result<Vec<element>,IndexError>`. Known replacements can constrain
an unresolved source vector. Raw annotations and checked continuation types
remain distinct. Checked reads preserve None or map it to a unit error case;
checked writes preserve IndexError or use a compatible named case. These native
failure routes can be checked even when the element type remains unresolved.

Vector checks share the invocation budget and expression-depth limit. Index
values are never used to evaluate or fold the access: missing reads and failed
writes remain runtime Option/Result outcomes. All operands and failure evidence
retain authored order, including replacement expressions evaluated before the
write's own bounds result. Construction and length do not produce a checked
Result/Option. Type consistency does not guarantee ordinary AF1-X can infer
every unannotated expression, and other deferred obligations remain explicit.

Ordered-map construction checks alternating key/value pairs and independent
homogeneity constraints for keys and values. Expected results and later typed
pairs can constrain earlier literals and nested expressions. The raw result is
`Result<Map<K,V>,DuplicateKeyError>`, including for empty constructors with a
supplied type. Lookup requires the exact key type and returns `Option<V>`;
membership returns bool; insertion requires the exact key/value types and
returns the map type; removal requires the exact key type and returns the map
type. Insertion and removal are not checked Result/Option operations. Native
DuplicateKey/None propagation is checked even with unresolved payload types.

The existing canonical type checker validates map-key admissibility, including
the reachable named record/variant definitions from accepted and draft state.
Draft members override accepted members; restated accepted definitions retain
generic parameter metadata and invariants. The trait-only view uses local member
identities for draft definitions and never emits or changes a graph entity.
Malformed, ambiguous or shadowed declarations remain `definition_deferred`;
they are not treated as empty definitions with admissible traits. Canonical
checks reject inadmissible key traits, invalid generic arity and definition
cycles, while depth/work limits use the invocation budget. `maps` records
key/value types, key-trait status,
`entries: not_evaluated` and `rewritten: false` per authored operation. Keys are
not compared, sorted or deduplicated by preflight. Duplicate-key failure,
insertion/replacement, missing lookups/removals, operand order and runtime map
ordering remain ordinary compiler/VM behavior. Inference revisits update the
same evidence entry while preserving unresolved operands and definition checks.

Local-cell construction, reads and writes check operand counts and exact element
types before expansion. Construction uses an explicit `Cell<T>` result context
when available and checks persistability through the canonical type environment,
including reachable accepted/draft named definitions. It also uses the VM's
structural exclusion of local cells inside initializer types. Incomplete named
definitions stay deferred; bare literals without a type anchor do not acquire a
default width from preflight. Reads return the stored element type; writes require
that same type and return `unit`. A known unit result does not hide unresolved
operands. Cell creation cannot use checked propagation, while reads of stored
Option/Result values use their actual failure route. Evidence under `cells`
records element/result types, storage eligibility and unchanged authored
locations; no runtime cell is allocated, read or written by these checks.

Every supported operation also applies the VM's structural operand rule:
operands containing local cells are refused except for `cell_get` and `cell_set`.
This includes cells nested in container types and function signatures, using
the VM's existing `contains_cell` judgment. Named definition bodies are not
newly unfolded by this structural rule. Nested expressions restore the enclosing
operation's rule before checking their result as an operand. `cell_operands`
evidence records the operand and operation locations, with `absent_checked`,
`cell_operation_exempt`, or `type_deferred`; unresolved operands cannot establish
cell absence. These checks share the invocation budget and do not evaluate cells.

The declared function result is also checked by the existing VM
`check_result_type` boundary before expansion, rejecting structurally contained
execution-local cells and host handles. The report records
`execution_local_return: vm_boundary_checked`. This boundary and cell storage
checks do not establish full ownership, lifetime, aliasing or control-flow
compatibility; those remain ordinary compiler/VM obligations.

Integer arithmetic in general AF1-X expressions now checks arity, equal
integer widths/signedness, primitive literal ranges and `u32` shift counts.
Known negation operands must be signed integers. An operation without `?`
produces `Result<integer,ArithmeticError>`; a checked operation exposes its
successful payload. Nested arithmetic, comparison/call operand context and
explicit wrapped-result annotations retain this distinction. An unresolved
integer width stays deferred, even when its failure type is already known.

Floating-point add/subtract/multiply/divide, negation and fused multiply-add
check operand counts and require exactly matching f32 or f64 types. Result
context or a known sibling operand can constrain earlier literals and nested
expressions. Unanchored widths and unsupported operands stay deferred; no
default width or implicit integer/float conversion is introduced. These
operations return the float directly and cannot use checked propagation, even
when their width is unresolved. Aliases and explicit result annotations use the
same checks. No expression is calculated, rounded, reassociated or rewritten;
the existing VM profile retains IEEE arithmetic with canonical NaN and positive
zero results, plus distinct fused-operation semantics.

The existing request parser still rejects decimal JSON numbers. Float parameters,
contextual integer literals and explicitly typed NaN/inf/-inf literals use the
ordinary AF1-X/value-reader rules. Supporting these operations does not widen the
residual envelope grammar.

Checked calls and other supported Result/Option expressions also check their
failure connection. Bare `?` preserves the failure unchanged and must match the
enclosing function's Result error or Option return. Named error cases can drop
the failure into a unit case or carry the exact Result failure payload. None
cannot supply a payload. A named route absent from the bound error variant
refuses at its authored location, including when the source expression's result
type is unresolved. These fragment interfaces expose no authored handler blocks;
use ordinary AF1-X for such handlers. Incomplete error definitions retain
`error_definition_deferred` evidence instead of inventing a missing or unit case.
Reports
record failure route, source failure type, output locator and known unwrapped
type. Inference revisits update one evidence entry per authored location,
preserving first-visit order. No expression is evaluated or reordered, and this
report does not establish complete control flow or rule out runtime overflow.

Conditional exits check their authored shape, Boolean condition and failure
payload before expansion. Bare `!` requires an Option return and no payload.
Resolved named error cases require exactly their declared payload presence and
type, including primitive literal ranges. Conditional-exit payloads cannot be
nested operations, matching ordinary AF1-X. Conditions can contain supported
nested and checked operations; preflight neither evaluates nor reorders them.
Reports record the condition check, failure route and authored payload locator.
Unknown condition types and incomplete error definitions remain deferred. A
provably missing named case refuses even when its condition is false; preflight
does not prune authored paths. Complete control flow and generated edge threading
still require the ordinary compiler. Exits define no local operation result.

Checked-pipeline preflight also checks integer type equalities across nested
arithmetic operands and named intermediate results. Add/subtract/multiply/
divide/remainder operands have the same integer type; shifts require a `u32`
count independent of the shifted integer's width. Negation requires a signed
integer, including when a later constraint establishes its type. The final value must match
the Result's successful type. Typed literals and referenced parameters provide
explicit type anchors; parameter/step reads, forward references and duplicate
step definitions follow the authored block's naming constraints. Literal values
whose types are resolved are checked with the existing typed-value reader.
Conflicts identify the authored use and the binding/type sources involved.
Parameter suffixes use the same u32 parsing and declared types described above;
step names still shadow parameters and nonzero step result indexes require
explicit AF1-X. Shared ordinary inference preserves actual parameter types for
valid numeric selectors, so those types can anchor arithmetic literals and
constructors. Other unresolved expressions can still produce an incomplete
ordinary trial; this pass does not invent type anchors.

Pipeline arithmetic accepts the same unmarked opcode spellings as ordinary
AF1-X: short mnemonic, SSMC1 name or decimal tag for integer opcodes 64–71
(for example `add`, `int_add_checked` or `64`). Preflight and expansion resolve
these through the shared opcode table. Expansion preserves the authored spelling
and adds the explicit checked route selected by `arithmetic_failure`. Authored
checked suffixes and all other opcode classes remain outside this fragment.

This pass solves consistency constraints without writing inferred types into
the request or changing expansion. An unused expression containing only
untyped literals remains deferred; a resolved constraint does not guarantee
that ordinary AF1-X inference can construct every literal-only expression.
`connections` records `pipeline_integer_connections`, operation and checked
literal counts, unresolved type-node counts, and `rewritten: false`. The pass
shares the original budget and fragment depth/operation bounds. It does not
evaluate arithmetic, assume overflow cannot happen, or reorder operations.

This initial check generates no blocks or operations and shares the invocation
budget. It runs before derive expansion for complete plans, direct trials,
fills, and every declared relation row during bound validation. Its evidence is
`residual-interfaces.json`, available through `show --provenance`. Reports say
`composition: partial` and retain deferred expression typing, ownership,
effects, full control flow and kernel checks. Computed inputs, computed
payloads and operation-defined join values still need those later
checks; the initial preflight does not establish full composition conformance.
Interface conflicts refuse before candidate/draft publication. The ordinary
command event ledger may still record that refusal.

Generated names avoid authored prefixes. The provenance map records
`AUTHORED` decision pointers, `FRAGMENT_DEFINED` construction rules, and
`DERIVED` checked-route transformations. More specific pointers override
their containing region's origin. `Expansion::origin` resolves AF1-X
pointers; `Expansion::lowered_origin` composes the existing AF1-X source
map with residual origins for plain AF1 pointers. Directly copied authored
subtrees retain their suffixes. AF1-X transformations are recorded as
`DERIVED` from the mapped construct and its contained decisions: lowered
operation offsets are not assumed to match authored terminator offsets.
Derived values retain all decision inputs, and missing origins remain
missing. These maps are persisted and available through residual show.
When layering onto another draft, pointers are relocated to the target's
actual index in the layered frame. Composed-map coverage is explicitly
limited to the residual target; unrelated base-frame entries are counted
as unmapped rather than assigned invented provenance.

Bounds are eight nested fragment applications (`MAX_FRAGMENT_DEPTH` = 8), 64 guards/steps/cases or
join arguments per application, and 1,024 aggregate construction items,
followed by existing AF1-X expansion limits. Exhaustion refuses; it never
truncates the program. These are structural limits, not evidence of the
planner's required wall/CPU/memory ceilings.

The fragment manifest is available through `residual::fragments::manifest`.
Further edit lenses, full pre-expansion composition checking, complete
resource enforcement and full usage accounting remain pending. Frontier
planning is described in the later sections. This
implementation is not a release or a cost-reduction result.

### 2.4 Residual CLI and artifacts

`residual try` accepts a file, inline JSON, or stdin (`-`), bounded to
1 MiB while reading. Strict parsing precedes workspace registration:
malformed residual requests create neither drafts nor an events ledger.
An initialized accepted head is required. Derive and the integer-literal
edit lens below are implemented. Missing fragment fields produce a bound plan
with explicit unresolved decisions, as described below.

A current/root base starts a new draft. Derivation on an explicit draft base
layers onto that revision using the existing layer implementation. Literal
edits compose over the source candidate while retaining its created identities.
Both draft routes require the selected revision to remain latest. The resulting revision uses the existing atomic draft
publication, candidate storage, kernel validation, and test runner.
`--on` and `--rebase` are not residual options: the request's base is
authoritative and changes require an explicit revised request.

#### Accepted/draft-graph integer-literal lens

An edit selects `checked_pipeline@1`, bindings
`{"lens":"integer_literal","value":7,"overrides":{"other.entry.amount":9}}`,
1–64 exact qualified operation names in scope, and exactly
`{"outside_scope":true,"boundaries":true}` in preserve. Overrides are optional
and may name only scope members. Each selected operation must load an integer
constant that directly feeds checked arithmetic. Values are JSON integers
within the inherited width; strings, booleans and implicit widening refuse.
The base may be the current accepted graph, its exact root, or an explicit latest
kernel-valid draft revision (`dN@rK`). Draft editing loads the bound candidate,
status, frame and name map, validates the source against the accepted head,
retains its nonce/create order, and appends any new constant identities after
its original creates. The composed candidate is independently validated. The
ordinary revision claim, candidate store and test runner publish the result;
the retained whole frame is not recompiled to create different identities.

Draft edits retain editable AF1/AF1-X syntax via fresh source-map reconstruction,
then check that literal repair changes only the selected immediates. A subsequent
preservation step may add explicit inherited constant declarations as described
below; it checks re-expansion for exactly those additions.
Source test IDs, assertions, table ownership and imported-test sources survive.
Expected outputs are never repaired automatically. The records retain the
sparse expansion/provenance, repaired authoring frame/locations, source binding
and graph-preservation evidence. Missing or ambiguous authoring sites refuse.
The bound loader also compiles the source frame's stated definitions and tests
against the validated source graph, and the retained frame against the composed
graph. These assertion checks publish nothing. They recover anonymous test
names and verify already-applied deletions before checking the remaining frame.
Equal-constant alias differences are allowed only for inline literals; explicit
named references must agree on identity. Disagreement refuses before revision
publication or factored plan creation. A second replay compiles the frame against
the accepted head, borrowing only entity/member identities from the validated
source. New inline constant identities match the complete authored typed value,
not generated names. The replay returns expected bodies and deletions, never
assembly operations. Whole-graph comparison detects omitted definitions, missing
deletions and unstated inherited-field changes, including object metadata.
When every entity is accounted for, reports record correspondence verified with
the inline-constant alias exception. Literal edits can retain created constants
that the latest frame no longer mentions; these and their namespace memberships
are counted separately as `unrepresented_created_constants`. Their historical
provenance remains unproven, so any such remainder keeps
`complete_candidate_correspondence: not_established`. An unrelated omitted
function, type, test or accepted-entity mutation is never such an exception.
For a changed edit, the bound loader makes these constants explicit in the
retained frame's `consts` array, using the validated source graph as authorized
side information. Each value must round-trip exactly through AF1; the source
object must remain byte-identical in the composed graph. Re-expansion must add
only those declarations. Replay repeats until all entities are accounted for,
including any inline alias-selection changes caused by explicit declarations.
Only then may the completed frame be published. The pretty-printed frame plus
its final newline must fit the 16 MiB receipt read bound, and the frame must
respect the receipt reader's value-count and nesting limits.

`residual-inherited-constants.json` records source revision, binding, candidate
SHA-256 and declaration origins (entity/object IDs and authored locations).
The whole-frame provenance marks these locations `INHERITED` from the validated
source graph; historical authorship is not asserted. Subsequent ordinary draft
layers therefore keep these otherwise unstated constants. Source reports still
describe the original frame honestly. No-op edits do not rewrite a source frame
or create a revision merely to add declarations; their original correspondence
limitation remains visible when applicable.
Complete draft plans perform and report source/composed validation; draft
closed-relation plans validate every row without filtering by test results.
The events ledger counts these validations separately from trials. Preparation
errors without a completed evidence record report kernel state as `unknown`,
rather than asserting it did not run. Incomplete/refused source drafts,
some Ripple-derived authoring sites remain open. Factored draft relations use
the separately validated source path described in section 2.8.

Alternatively, bindings may be
`{"lens":"integer_literal","values":{"adjust.entry.amount":7,"other.entry.amount":9}}`.
This mode requires one value for each exact scope member and forbids `value`
and `overrides`. A partial `values` object makes each absent site a separate
decision, such as `/bindings/values/other.entry.amount`; `{}` requests all site
values. Fill supplies all listed decisions in one round and cannot replace an
already authored value. The existing closed-relation contract may bind these
paths; every declared row is checked against all inherited widths before the
plan is published. Separate paths do not establish semantic independence or
authorize factoring. Sections 2.7–2.8 describe the dependency adapter and the
explicit factored CLI contract.

Expansion emits ordinary AF1 replace-op edits with namespace changes disabled.
The changed load affects all of its uses. Old constant objects, including shared
ones, remain unchanged. The compiler mutation list is checked before assembly:
only changed target loads may be replaced, with every field except their
constant reference preserved; new entities must be constants used by those
loads. The proposed graph is then checked before storing names or candidates:
every old non-target entity must remain byte-identical and every requested
typed value must be present. Equal-value targets retain exact identity and bytes.
The kernel remains the authority on candidate validity.

`residual-edit.json` retains source entity/object versions, constants, widths,
checked consumers, requested values, preservation policy, and boundary impact.
The report follows typed static function references transitively to possible
callers and lists affected exports and entry points. It does not claim caller
behavior is unchanged or enumerate dynamic/external callers. Compact results
show at most 16 members per edit inventory list with omission counts; full
evidence remains local. Provenance links authored values to request pointers
and inherited fields to source object versions.

If all typed values already match, no candidate or draft is created.
Construction is complete and native admission is not_attempted. For an
accepted base, kernel is not_run. Requested public cases run against that head;
--all-tests selects its existing tests, while --no-test skips execution.
With neither public cases nor --all-tests, accepted-base checks are zero_ran.
A draft no-op reports its actual source/composed validation, checks the source
graph, and selects the inherited candidate tests as an ordinary draft trial
would. --no-test also skips these checks. A failed or
unexecutable check is retained as failed. The binding is rechecked before
publishing a local immutable rN@1 record under `.sley/residual/`, using atomic
allocation, strict JSON, a typed digest, a 16 MiB record limit and at most
10,000 numbered records. All residual show views accept these handles.
Historical inspection checks record integrity; it does not claim current
binding validity or confer authority.

#### Bound plans and one explicit fill round

`residual plan request.json` previews construction without running a trial.
`residual try` uses the same path when fragment fields are missing, returning
construction incomplete and exit 1. A successful explicit plan command exits 0;
the four result categories still distinguish incomplete construction from
validation. Plans use immutable rN@1 records in the existing residual store.
They retain the request, full binding, missing-field inventory, selected
fragment contract and original trial options. Complete previews retain their
expansion/provenance; incomplete plans report expanded output as unavailable.
No candidate or source draft is created or changed by planning.

Without a `choices` declaration, the route is `explicit_author_decisions`,
with optimization `none`.
It batches required missing fragment fields across known nested guards,
pipelines, branch arms and joins. Existing fields stay authored; missing
fields stay UNRESOLVED even when only one policy value is supported.
No description enumeration or public-test filtering supplies semantics.
The inventory checks closed fragment field schemas and supplied field shapes;
graph types, expression validity, and exhaustiveness still require completed
authoring and the existing compiler/kernel. Bounds are 64 missing fields,
eight nested fragments and 1,024 inspected structural objects. The CLI applies
one aggregate 250 ms fast-path wall budget to this route, including complete
requests, missing-field inventory, binding, reconstruction and preparation.
Later stages cannot restart the inventory clock or adopt the frontier's longer
allowance. This still does not establish the full memory ceiling.

`residual fill rN@1 decisions.json` accepts exactly
`{"residual":1,"plan":"rN@1","choose":{"/bindings/rounding":"toward_zero"}}`.
Strict parsing precedes workspace access. Paths must be exactly the recorded
questions (all missing fields for explicit plans, the frontier for closed
relations); unknown paths, overwrites, missing answers and answers that
introduce further unresolved fields refuse. A complete preview requires an
empty choose object. A missing parent is answered as one complete subtree.
The filled request is strictly parsed and expanded through the ordinary path.

Fill recaptures the original binding and recomputes the decision inventory,
then retains that original binding as an additional guard through candidate
assembly. The completed request receives its own binding. There is no interval
in which a changed head/name map may be silently adopted as a new base.
Source draft revision checks and kernel preconditions remain authoritative.
An atomic, immutable fill marker additionally allows only one trial per plan,
including current-base plans with no prior draft revision. It is claimed only
after strict answer validation and successful expansion. A consumed plan cannot
be retried and its marker must not be automatically removed. After interruption,
inspect the resulting draft first: a complete published successor can recover a
lost acknowledgment without another trial. If no complete successor was published,
explicitly replan; abandoned private revision numbers are skipped. Current-base
and source-draft process-interruption controls exercise these publication boundaries.
The CLI's preparation budget also checks a conservative complete resident
user-address-space memory bound in addition to allocator reservations/peaks.
It includes stacks, mappings and allocator overhead/cache. Virtual slack and
preparation-startup virtual peaks may conservatively require ordinary AF1-X
fallback; unavailable observations refuse rather than claiming enforcement.
The resource artifact discloses the observation sources and scope. Checks are
cooperative before publication, not an instantaneous OS allocation cap.

Malformed answers do not consume a plan. No second automatic question round
is started. Ordinary AF1-X or a complete revised request is always the escape.

Trial options supplied to the original try carry forward. Public-case paths
are resolved before recording the plan; those files remain external test
inputs read at trial time, not semantic constraints or binding authorities.
Filled drafts retain `residual-plan.json` and `residual-fill.json` alongside
the completed request and its ordinary expansion. The input.txt of a filled
draft contains that completed residual request; the fill artifact preserves
the parsed answer and original decision paths. Historical show remains
available after binding changes or consumption.

Each recorded residual revision retains these local artifacts:

| Artifact | Meaning |
|---|---|
| `input.txt`, `residual-request.json` | Direct try: submitted request bytes and parsed request. Fill: completed request; the parsed answer is retained in `residual-fill.json`. |
| `residual-binding.json` | Full captured binding and digest |
| `residual-interfaces.json` | Derive interface preflight, typed binding locators and deferred checks; composition remains partial |
| `residual-budget.json` | Aggregate planning wall/CPU/work and accounted memory observations, explicit coverage and unfinished aggregate memory enforcement; shown by `--provenance` |
| `residual-resolution.json` | Closed-relation reconstruction, selected original row, completed request and decision origins; present only for relation resolution |
| `residual-plan.json`, `residual-fill.json` | For fills: original plan with the complete relation, and the actual author answers |
| `residual-expanded.json` | Fragment-generated AF1-X/AF1 before draft layering |
| `residual-provenance.json` | Decision/rule origins indexed into the layered `frame.json` |
| `frame.json` | Complete layered frame, as for ordinary try |
| `expanded.json`, `sourcemap.json` | Ordinary AF1-X lowering artifacts, when compilation succeeds |
| `residual-source-map.json` | Composition of final lowering locations with residual origins, with explicit coverage |
| `status.json` | Existing draft status plus fragment, binding, and independent result categories |

The default response separates `construction`, `kernel`, `public_checks`,
and `native_admission`. Public checks include externally executed TestCases
and public cases with separate counts. Skipped checks are `not_run`, an
empty executed set is `zero_ran`, and failed or unexecutable cases are
`failed`. Native admission is `not_attempted`: this command never submits
or commits. Compiler obligations and original kernel refusal symbols survive.
Behavioral failures retain expected and actual values.

Default changed-target and failure lists show up to 16 records and explicit
omission counts; full trial output is available with `--verbose`.
Sparse edit previews include only each target's
`scope_index`, `name`, `changed`, and `requested` value; `targets_count` and
`targets_changed` cover the complete inventory, `targets_omitted` counts hidden
rows, and `target_details_omitted:true` marks hidden source records. The edit's
`inspect` command retrieves the full inventory, original object identities,
widths and consumer evidence through `residual show <reference> --provenance`.
`residual show <reference> --targets` retrieves every saved literal target with
its scope index, name, width, previous value, requested value and changed flag,
plus the recorded binding and verification status. It does not include original
object identities or consumer evidence and does not rerun checks. This historical
view is available for literal-edit draft revisions and no-change records; plans,
non-literal edits and incomplete target records refuse. Views are mutually exclusive.
Full evidence remains in `residual-edit.json`; compact rendering never alters
stored artifacts, edit verification, boundaries or refusal information. No-op
replies use the same preview contract, including historical default inspection.
No-op default replies and historical summaries retain check outcome/count and
failure details while omitting the complete `tests` and `public` arrays.
`checks.details_omitted:true`, `tests_omitted`, `public_omitted`, and `inspect`
disclose this omission. `residual show <record> --provenance` retrieves the
original complete `checks` from the saved record without rerunning checks;
`residual try --verbose` continues to return the full arrays. Stored evidence is
unchanged, including for historical records created before this summary repair.
Body omission is explicit. `residual show` requires an exact draft revision or residual record and
inspects its recorded artifacts even after the head changes, labelled `historical: true`. `--expanded`
identifies whether AF1 or only AF1-X is available; `--decisions` shows the
request, and `--provenance` shows bindings and source maps. Ordinary draft,
fill, explain, call and submission paths continue to operate on these drafts.

Events record residual request bytes, expansion bytes and provenance-entry
counts alongside existing result/input/output counters. These are local byte
measurements, not provider token counts or a complete paid-attempt ledger.
Invalid envelopes are deliberately absent from the local ledger because
parsing must precede writes; an external campaign recorder must account for
those attempts too.

### 2.5 Finite-description frontier core

`residual::frontier` implements bounded weighted pair separation over an
explicit finite relation. The mathematical core alone supplies no semantic
entitlement. The CLI connects it to authoring through the following explicit
constraint declaration, checked against the fragment's actual missing fields.

#### Author-declared closed relations

An optional request member has exactly this shape:

```json
{"choices":{"version":1,"contract":"author_closed_relation","rows":[
  {"/bindings/params":[["x","i8"]],"/bindings/returns":"Result<i8,ArithmeticError>","/bindings/rounding":"toward_zero"},
  {"/bindings/params":[["x","i64"]],"/bindings/returns":"Result<i64,ArithmeticError>","/bindings/rounding":"toward_zero"}
]}}
```

Here the ordinary bindings omit exactly `params`, `returns`, and `rounding`.
The author declares that these literal rows are the complete permitted
relation. This constrains the program; it does not assert that enumerated
examples exhaust an external natural-language task. Sampled-family contracts
are refused. Every row must supply exactly every missing field, with no
overwrites, further holes, duplicate rows, or unsupported policies. Declaring
a relation for a request with no missing fields is a constraint conflict.
The ordinary request limits apply, with 1–256 rows and at most 64 fields.

Unbound relation analysis checks request schemas and reconstructibility without
generating code. Before publishing a plan, each derive row undergoes the bound
interface preflight, then fragment expansion; every completion is also compiled
in the bound accepted-head/draft context,
including sparse-edit scope and width checks. One invalid row refuses the
entire relation with its row locator; no row is filtered out. Public tests
never prune the family. These checks do not run the kernel or establish task
correctness. The selected completion still uses the ordinary trial path.

Questions are actual required fragment-field paths, never opaque row IDs.
Their descriptors disclose all distinct supported values. A field's additive
cost is the compact JSON byte length of that whole descriptor plus one byte,
under `disclosed-field-json-bytes-v1`. It is a byte proxy, not billed tokens,
and excludes interaction costs outside that descriptor. CLI selection uses
deterministic greedy separation; optional exact refinement remains a library
operation. Plans report `closed_author_relation` and
`greedy_additive_byte_surrogate`.

Only the selected frontier fields require answers. Checked unique
reconstruction supplies the remaining fields from the authored relation. In
the example, one signature answer distinguishes the two rows; rounding is
entailed by both rows. A literal singleton can resolve directly with no
question round. Without the explicit relation, even a sole supported rounding
policy remains an author decision. Neither case implies task correctness.

`residual-resolution.json` retains the selected original row, completed
request, family digest and origins. Answered fields are AUTHORED at their
actual `residual-fill.json` pointers. Reconstructed fields are DERIVED by
`closed-author-relation-v1`, citing the authored contract, exact row value
and answer dependencies. Expansion provenance and final source maps retain
these origins. Direct singleton sources point into `residual-request.json`;
filled sources point into the original request in `residual-plan.json`.

The full relation remains local and is marked omitted in the compact report.
Use `residual show rN@1 --decisions` to recover it before answering from a
new context. Input/disclosure/authoring effort must be included in future
usage accounting; storing a relation locally does not make it model knowledge.
The CLI analyzes the relation once per invocation, then carries that analysis
and one shared budget through reconstruction, preparation, bound-row compilation
and the final check before publication or fill consumption. Startup executable
hashing and initial envelope parsing precede that budget; ordinary candidate
trials begin after it. This scope is recorded in `residual-budget.json`. A fill starts its
own invocation budget and never resumes a previously spent planning allowance.
Its initial binding capture is inside this budget. Decision inventory, each
closed-relation row fill, selected-row reconstruction, factored fills and their
post-fill inventories all consume this same work/clock budget. Individual inventory
passes retain their existing 250 ms ceiling as well. Public standalone helpers
create a budget for that call; composed callers use `fill_with_budget` and
`Choices::resolve_with_budget` to retain prior consumption. Exhaustion while
filling a relation row carries the row's source locator and stops before its
compiler callback.

Fill measures original and answer JSON with the bounded counting writer before
cloning, charges serialization work, then reserves the completed request's encoding
buffer against the shared memory ceiling. The original request is never mutated.
These reservations do not cover JSON tree clones or parser allocations. Aggregate
memory enforcement and full interaction cost routing remain unfinished; factoring
is described below.

Expansion also uses the invocation budget. `fragments::expand_with_budget`
charges input sizing, name-capture traversal, recursive arithmetic, fragment/region
visits, generated blocks and provenance loops. The builder refuses a block beyond
the existing AF1-X limit of 1,024 before pushing it. It returns no partial expansion
on exhaustion. The standalone `expand` helper creates one budget for its call.

`edit::expand_with_budget` charges exact targets, typed consumer scans, static
caller traversal, exported-function reporting and entry-point scans. Accepted
edits, validated draft composition and dependency-site inspection carry the
caller's budget through the shared lens implementation. These checks preserve
existing frame/provenance and caller-report ordering. Ordinary AF1-X/compiler
internals and individual JSON clones remain bounded by their existing checks;
they are not made preemptible by the residual budget.

#### Mathematical API and guarantees

`Family::parse` accepts strict JSON with exactly `fields` and `descriptions`.
Each field supplies `name`, positive u32 `cost`, and boolean `eligible`.
Each description is an object containing exactly those names and typed JSON
values. Unknown members, duplicate decoded keys, duplicate field names,
duplicate complete descriptions, floats, empty families and incomplete rows
refuse. The normal 1 MiB/depth/value parser limits apply, with at most 64
fields and 256 descriptions per coupled component. Names contain 1–256 bytes.
Costs are declared additive estimates; eligibility is a declared vocabulary,
not evidence that a question is meaningful or that its meaning was disclosed.

`frontier::plan(&family, &mut budget, refine_exact)` computes coverage of every
pair of supplied descriptions. It selects the highest newly separated
pairs-per-cost field using exact integer cross multiplication, breaking ties
by lexical field name. It then removes redundant fields in descending cost
and lexical order. Ineligible fields are never selected. An indistinguishable
pair refuses with VOCABULARY_INCOMPLETE; no sampling or closest match occurs.
The final feasible projection is checked before it can be returned.

Optional exact refinement enumerates every eligible subset only when there
are at most 12 eligible fields. Completion is labelled `exact_additive`;
work/time exhaustion during refinement retains the best already checked feasible projection
as `bounded_best`, without an optimality claim. Larger vocabularies retain
`greedy`. No method claims minimum billed cost. A shared `Budget` enforces
aggregate planning work (12 million charged bitset/work units), a two-second
wall ceiling, and 64 planned fields across calls; callers may tighten these
ceilings but cannot raise them. Exhaustion before a checked feasible frontier
is a LIMIT refusal. These work units are an engineering bound, not CPU seconds
or tokens. CLI requests without a `choices` contract use the same budget with
the wall ceiling tightened to 250 ms. `use_fast_path` preserves the original
start, consumed work and fields, and any tighter caller ceiling. It cannot
restart spent time or increase an allowance. Explicit `choices` versions 1–2
select the two-second finite-frontier route even when their family is a
singleton. `residual-budget.json` reports `planning_route` as
`explicit_fast_path` or `finite_frontier`, together with the actual ceiling.
Linux also measures this thread's CPU through the existing
`/proc/thread-self/schedstat` source and refuses at two CPU-seconds. CPU checks
run at stage boundaries and every five milliseconds of charged work. A clock
that becomes unavailable or moves backwards refuses; platforms without that
clock use the single-thread wall ceiling as a conservative CPU upper bound,
with measured CPU reported as null. A budget cannot move to another thread.
Preparation checks the same budget before binding and between binding rechecks,
resolved-request parsing, expansion, artifact construction, draft layering and
provenance attachment. Closed-relation validation checks between context setup,
expansion, layering and compilation for every row. An exhausted stage cannot
start the following stage with a fresh allowance. Checks are cooperative: one
bounded expansion/compiler step can cross a deadline before its following
checkpoint refuses. No over-budget result is
published or fill consumed by these planning paths. Complete additional working-memory
enforcement remains pending. Sections 2.6–2.7 describe the mathematical
constraint-graph core and the literal-edit dependency adapter. Passing
several families through one budget does not itself prove their independence.

The largest pair-coverage matrix and target bitmap occupy under 266 KiB.
Construction additionally resolves at most 256 × 64 borrowed value references
once, avoiding repeated map lookups during pair comparison without copying
domain values. The implementation is synchronous and creates no workers. This is a bound
on that matrix, not a claim about the entire process's memory consumption.
No global Cartesian product is constructed by this API.

Factored component topology uses fixed arrays for at most 64 parent indexes,
64 component masks and 256 table-to-component indexes. It does not allocate
parent/scope vectors, group maps or component-name sets. Parent traversals and
table membership scans charge the invocation's shared work budget. Components
retain the prior minimum-root order, and each component's descriptors retain
lexical field order. Constraint and component encoding borrow descriptors and
row values directly rather than constructing copied JSON envelopes; emitted
descriptor keys retain their existing byte order. The factored problem identity
also streams the legacy typed canonical encoding directly from the validated
fields, table digests and dependencies. Counting and hashing passes share the
invocation budget; no copied canonical JSON tree or full encoded hash buffer is
constructed. The hash domain, length prefix and digest bytes remain unchanged.
Parsing the encoded family, context binding and returned identity strings still
have the documented memory exclusions below.

Pair-coverage storage, borrowed-value index vectors, greedy scratch, exact
refinement indexes, borrowed join row buffers, join size indexes, component,
closed-relation and completed-fill encoding buffers, decision-domain encoding
keys and borrowed-value vectors, family sorting keys and canonical member indexes reserve
their requested heap capacities before allocation.
Reservations share one 256 MiB ceiling and release capacity on drop, including
error paths. `Budget::limited_with_memory` can tighten this ceiling. A denied or
overflowing reservation remains exhausted even after live reservations drop;
neither a new component nor fast-path selection resets it. The report records
current and peak reserved bytes, the ceiling and exhaustion status. Redundancy
ordering uses an allocation-free sort with the same total tie order; exact tie
comparison consumes iterators without allocating temporary vectors.

Ordinary and factored frontiers reserve their retained field vectors, copied names,
identity strings and component vectors before planning. The initial allowance
covers every possible selected field; once selection finishes, unused string
capacity is released. This permits bounded-best refinement to retain an already
checked projection after work/time exhaustion without acquiring a fresh memory
allowance. Clones share the immutable payload and its reservation, which releases
only when the final owner drops. Multiple retained plans share the construction
budget's ceiling. Reusing an object under another budget does not transfer its charge.

`memory_enforcement` remains `frontier_scratch_and_retained_payload_reservations`.
These requested-capacity reservations exclude parser peak, map nodes, family
digest strings, other retained JSON trees, graph/compiler allocations, Arc storage
and allocator overhead. Their peak is neither process RSS nor total planner memory.

The instrumented workbench binary additionally reports
`aggregate_memory_enforcement: allocator_peak_checked_at_budget_checkpoints` and
an `allocator_heap` observation. Its single planning thread observes the process's
live allocation extents, including size-class rounding, relative to the budget's
startup baseline. This includes planner/compiler allocations omitted from explicit
reservations and remembers released transient peaks. System reallocation observes
source and destination simultaneously, conservatively even for in-place growth.
Nested budgets retain independent peaks; exhaustion never resets on release.
An unavailable observer or invalid accounting refuses at a budget checkpoint.
Library callers without this allocator registration report `not_implemented`.

Observation starts after initial envelope parsing and startup executable hashing,
and ends before persistence and the ordinary candidate trial. Each checkpoint
checks the observed peak against the same additional-memory ceiling (at most
256 MiB); a single allocation can overshoot before that check. Infallible allocation
and OOM behavior are unchanged. Idle cached blocks, system allocator metadata,
stacks and OS mappings remain excluded. This is not a physical-memory/RSS limit
or preallocation cap; the full specification's memory requirement remains open.

Version-1 closed relations use reserved JSON keys and borrowed values to sort and
deduplicate each disclosed domain in the original byte order. JSON types remain
distinct, and duplicate keys release their buffers. Disclosure costs are measured
with the bounded counting writer, without allocating a measurement buffer.
Family encoding borrows the existing rows and fields instead of cloning an
intermediate JSON object. Its reserved buffer stays live while strict parsing and
family canonicalization acquire their own reservations, so their overlapping
capacities share one ceiling. The buffer drops before frontier construction.
All serialization uses the existing work/time budget and a 1 MiB byte ceiling.
Retained request/question trees and parser peak remain excluded as stated above.

Factored joins measure serialized row sizes with a counting writer, without
allocating temporary JSON buffers. Component encoding borrows joined rows and
measures the full envelope against the invocation's remaining 1 MiB materialization
allowance before allocating its reserved output buffer. The second serialization
pass cannot exceed the measured capacity. Both passes charge the same work/time
budget at each writer call (one work unit per started 64-byte chunk, minimum one)
and at entry/exit. Escaped strings and keys use actual JSON byte counts. This
preserves the prior envelope bytes and family identity without cloning all rows
into an intermediate JSON value. Intermediate joins hold sorted key/value
references into the immutable source tables; nested JSON values and key strings
are never copied into those rows. Row-header and entry-vector capacities are
reserved before allocation, including simultaneously live input/output buffers.
The result borrows source tables, so dropping an earlier join releases its
reservation without invalidating later rows. Final family parsing charges adopted
payload capacity as described below; parser peak remains outside reservations.

`Family::parse_with_budget` consumes parsed row objects instead of cloning their
nested values. JSON sorting keys and their outer buffer reserve capacity before
allocation, share the invocation's work/memory allowance, and release on every
exit. Canonical family hashing traverses borrowed fields/rows twice: first to
measure the typed encoding, then to stream the exact length-prefixed bytes into
SHA-256. It creates neither a copied canonical value tree nor a payload-sized
encoding buffer. Sorted object-member indexes are reserved across recursive
visits. Legacy family identities and row ordering are preserved.

Family construction also reserves requested field/row vector capacity, copied
field-name bytes, and observable capacity of retained keys, strings and nested
arrays. Adoption traverses the already parsed tree under the shared work/time
budget before constructing new family buffers. It does not bound the parser's
prior allocations or estimate opaque map-node storage. Immutable families share
their fields, rows and reservation across clones; payloads are not deep-copied.
The charge remains live until the last family owner drops, including when an
owner is dropped on another thread. Simultaneously retained constraint/component
families and construction scratch share the same ceiling. A reservation remains
with its construction budget; callers must use one budget throughout an invocation
to aggregate these charges with later planning.

`Problem::parse_with_budget` shares one budget across its constraint families;
bound dependency validation uses it for both original and augmented declarations.
Standalone `parse` calls use a default budget. Initial JSON parsing still uses
the existing structural limits and is checked before/after. Parser transient
allocations and small scalar formatting buffers are not covered by reservations.

Frontiers bind a typed digest of the complete supplied relation, field costs
and eligibility. Field/row permutations leave identity and tie handling
unchanged. `encode` requires an actual complete member; `decode` requires
exactly the selected fields and exactly one matching member. Boolean true,
integer 1 and string "1" remain distinct. Changed families/costs/vocabularies
refuse as stale. Frontier fields are private and cannot be loaded from an
unchecked certificate. Summaries explicitly state that semantic entitlement,
family completeness for the task, and billed-cost optimality are not established.

Conformance includes the retained research counterexample (greedy 16 versus
exact 12 additive units), deterministic ties, typed domains, membership and
binding failures, structural/work/wall bounds, refinement exhaustion without
false optimality, maximum-size components with u32 costs, and 500 new seeded
finite families checked against an independent projection-based exact oracle.
The new seeded audit does not relabel or replace the supplied Python research.

### 2.6 Factored constraint-graph core

`residual::frontier::factors` plans a finite constraint declaration without
constructing its global Cartesian product. It is a mathematical library API.
The residual CLI continues to use its existing explicit/closed-relation paths;
verified dependency extraction from actual program bindings remains unfinished.
Graph labels supplied to this API are not proof that every semantic dependency
of a Sley program has been included.

`Problem::parse` accepts exactly this structure:

```json
{
  "fields": [
    {"name":"x","cost":1,"eligible":true},
    {"name":"y","cost":2,"eligible":true}
  ],
  "constraints": [
    {"fields":["x"],"rows":[{"x":0},{"x":1}]},
    {"fields":["y"],"rows":[{"y":false},{"y":true}]}
  ],
  "dependencies": []
}
```

Every field needs a literal domain table. Each table supplies complete rows
over exactly its named scope; all tables are conjoined by natural joins with
strict JSON-type equality. Duplicate rows/fields, unknown members, empty
tables and missing domains refuse. Bounds are 64 fields, 256 tables, 256 rows
per table/component, 1,024 dependency edges and the existing strict JSON limits.

Every table scope joins its fields into one connected component, even when its
rows happen to form a Cartesian product. Additional dependency edges have
exactly `kind` and `fields`; the kinds are `type`, `ownership`, `effect`, `order`,
`error_route`, `binding` and `author_relation`. Every edge names at least two
distinct declared fields and merges their components transitively. An edge
constrains factoring, not row values. Actual correlations/allowed combinations
belong in the literal constraint tables. The implementation never splits a
scope because examples appear independent.

`factors::plan(&problem, &mut budget, refine_exact)` joins tables only within a
component, with restrictive tables first and deterministic digest tie ordering.
Contradictions return FAMILY_EMPTY. A join exceeding 256 intermediate rows
returns LIMIT before storing a 257th row. A conservative 1 MiB serialized-byte
check runs before allocating joined reference rows; final component encodings also share a
1 MiB aggregate ceiling. These are materialization limits, not a proof of the
full 256 MiB working-memory requirement. Early limit refusal may occur even if
a later constraint could reduce the family; no sampling or partial solution is
reported instead. Every join and component frontier shares the same budget.

The resulting private `FactoredFrontier` projects/reconstructs one description
component by component, with exact field coverage and membership checks.
Twenty declared independent binary fields retain forty component rows while
representing 1,048,576 complete descriptions. The global product is never
materialized. Cardinality is reported as a decimal string when it fits u128;
otherwise the count is null with an explicit overflow marker. Reconstruction
does not depend on computing that count.

Field, table, row and edge ordering leave identity unchanged. Changes to table
contents, scopes, dependency kinds, costs or eligibility invalidate the bound
frontier. Summaries distinguish graph-relative independence from unestablished
program-dependency completeness, semantic entitlement, task completeness and
billed-cost optimality. Exact additive optimality is reported only when every
component's subset search completes. A checked bounded-best component prevents
that global optimality label, and exhaustion cannot grant later components a
fresh budget.

Conformance covers the million-description case without enumeration, every
coupling kind, shared variables and transitive edges, contradictory/oversized
joins, constant-value amplification, stale graph mutation, typed answers,
aggregate exhaustion and 128 small graphs checked against an independent
exhaustive conjunction/projection oracle with varied field costs.

### 2.7 Typed dependencies for literal edits (library)

`residual::dependencies::extract(program, names, request, budget)` matches each
site in an accepted-graph per-target `integer_literal` request and builds a
conservative local dependency graph. It reuses
`sley_policy::complete_entities::project_complete_entities` and its complete
typed reference edges. There is no second reference extractor or execution
sampling. Missing references, malformed graph inventories, mismatched exact
roots and unmatched lens sites refuse.

The adapter joins endpoints of typed references, including ownership within
functions, control flow, types, calls, function references inside constants,
effects, contracts, initializers and bindings. Thus two sites in one function,
sites with a common caller, and sites sharing a contract predicate remain
coupled even if their literal domains form a Cartesian product. The explicit
exclusions are administrative workspace/package/namespace membership, test
observations, and value reads of immutable integer constants. This lens
preserves old constants and changes selected loads by copy-on-write. Shared
constant identity alone therefore does not connect edits. Workspace contracts
or capability requirements, external dependency bindings and policies on
administrative containers conservatively couple every site. Fixed authored
sites are absent from the question vocabulary but remain in graph traversal.

The private `DependencyGraph` records source versions, matched targets,
components, inclusion/exclusion counts and its exact graph/request binding.
`constrain` re-extracts that binding, requires exactly the missing site fields,
checks every declared domain value against the inherited integer width, and
adds mandatory coupling edges to an author's finite declaration. Author edges
can join more components. No alternatives are pruned. Graph context is also
bound into the resulting `factors::Problem`, so even a changed object with
unchanged components invalidates an old frontier.

Source extraction caps at 65,535 objects and 8 MiB of stored source bytes;
the projected graph caps at 262,144 edges. Work and wall/CPU checkpoints share
the caller's invocation budget. Projection is an existing synchronous bounded
step, so deadline enforcement remains cooperative. These caps do not establish
the complete 256 MiB additional-memory requirement. Coupled components still
obey the factor core's 256-row ceiling and refuse without sampling.

This adapter establishes conservative connectivity in the supplied local typed
graph for this exact lens. It does not establish task completeness, model cost
savings, kernel validity, or relationships imposed by unknown external clients.
The caller must bind the source to the verified workspace head. Section 2.8
describes the CLI integration that retains and rechecks that original binding.

Conformance includes twenty real independent functions yielding forty component
rows for 1,048,576 combinations, followed by kernel validation and execution of
one reconstructed edit. Additional tests use real accepted fixtures and exact
typed object mutations to check calls, constant-held function references,
effects, contracts, administrative/global policies, stale objects, invalid
domains, missing references and shared budget exhaustion. Synthetic graph
fixtures establish extraction behavior, not kernel acceptance of those fixtures.

### 2.8 Factored author constraints in the residual CLI

Accepted-graph or explicit valid-draft `integer_literal` requests with per-target `values` may use
this alternative `choices` contract:

```json
{"choices":{"version":2,"contract":"author_closed_constraints",
  "constraints":[
    {"fields":["/bindings/values/adjust.entry.amount"],
     "rows":[{"/bindings/values/adjust.entry.amount":3},{"/bindings/values/adjust.entry.amount":7}]},
    {"fields":["/bindings/values/other.entry.amount"],
     "rows":[{"/bindings/values/other.entry.amount":3},{"/bindings/values/other.entry.amount":9}]}
  ],"dependencies":[]}}
```

Here scope names both loads and bindings are
`{"lens":"integer_literal","values":{}}`. Each table has exact field scope
and complete rows. The author declares their entire conjunction as the allowed
domain. Tables may overlap; contradictions refuse. Duplicate rows, partial
rows, extra members, fields outside the actual missing-site vocabulary and
unsupported dependency kinds refuse. The optional coupling content is carried
in the required `dependencies` array, using the factor-core edge schema.
No author field costs, completeness flags or independence assertions are
accepted. Mandatory typed graph edges always participate. Default-plus-overrides
edits do not use this contract. Derived functions use the conservative adapter
described below.

For a draft base, `DraftSource::analyze_factors` loads the exact bound candidate,
validates it against the captured accepted head, and uses its complete proposed
graph and captured names for dependency extraction. It checks the receipt before
and after analysis. Newly created source entities participate in coupling and
width checks. The ordinary public accepted-graph extraction/analysis APIs still
refuse draft requests; a caller cannot replace receipt loading with an assertion
that an arbitrary graph represents a draft. The dependency report retains the
source revision, binding, candidate SHA-256 and validation outcome.

Draft planning reports `source_kernel: valid` separately from the selected
completion's kernel outcome. The events ledger records its one dependency-source
validation as `source_kernel_validations`; later source/composed edit validations
are counted separately in `kernel_validations`. Cached binding reentry does not
repeat dependency analysis. Candidate families remain unmaterialized; the
selected completion uses the identity-preserving draft route in section 2.4.

`residual plan` and `residual try` capture a workspace binding **before** graph
analysis, then retain it through preparation/publication and recheck it before
candidate creation. `fill` checks the saved binding, reconstructs the same
deterministic frontier once under one invocation budget, and retains the
original binding as a parent guard. No head or name change silently rebases the
operation. All earlier width, scope, preservation and one-attempt fill rules
still apply. Fixed authored values and every declared table value must fit
their inherited widths, including alternatives that would not be selected.

The route is `closed_author_constraints`; optimization is
`factored_greedy_additive_byte_surrogate`. Question descriptors list
`declared_values`, the union of literal table values, with an explicit note that
the full conjunction applies. These are not a promise that every cross-field
combination is allowed. The positive additive cost is the full disclosed
descriptor's compact JSON bytes plus one, not tokens or an invoice. The planner
asks real missing-site paths in one batch, reconstructs omitted values only
through checked constraints, and never enumerates independent global products.
A literal singleton conjunction resolves directly without an answer round.

Plans report inherited-width checks separately. Compiler and kernel outcomes
remain `not_run`; `compilation_policy` states `selected_completion_on_trial`.
The selected completion uses ordinary compilation, mutation preservation checks,
kernel validation and requested public tests. Public tests never filter the
declared domain. Oversized coupled components refuse before publishing a plan,
even when their authored tables individually look independent.

Derived `ordered_guard_chain`, `checked_pipeline` and `typed_branch_result`
requests also accept this contract over their exact missing-field paths.
Every decision in the single target function belongs to one conservative
component: its signature, bindings, evaluation order, failure routes and effects
may couple the choices. An empty author dependency array never asserts
independence. The joined component retains the 256-description ceiling; requests
that exceed it must provide tighter explicit constraints or use direct authoring.
This adapter does not establish finer independence within a function.

After exact table conjunction, every resulting complete description passes the
existing bound interface preflight. One conflicting completion refuses the whole
family; invalid alternatives are never pruned into an inferred domain. The
report labels `interface_preflight` as `all_joined_rows_passed`, gives the checked
completion count, and retains `composition: partial`. It does not claim inherited
integer-width checks, full expression/ownership/effect closure, compilation or
kernel validity. The selected completion still follows ordinary compilation and
validation. A constraint can supply a whole missing subtree, but that subtree
cannot introduce another question round.

The dependency identity binds the exact request, source root/workspace/epoch,
object inventory and resolved entity names. Draft derivations use the same
receipt-loaded, replay-checked and kernel-validated proposed graph as draft
dependency analysis, including newly declared types. Public accepted-base
analysis refuses draft requests. Fill rechecks the original workspace binding;
neither head nor name drift silently changes the meaning of a table value.

Full `residual-dependencies.json` evidence stays in the plan. Its provenance
view exposes it; default output retains compact counts and digests. Resolution
evidence retains component frontier evidence, the matched row of every original
constraint table, and the completed request. AUTHORED origins point to actual
fill answers. DERIVED origins cite the full authored contract, exact literal
value source, answers, checked problem identity and dependency binding.
Sources use the original table/row order and escaped JSON pointers, including
direct singleton, filled candidate and no-op records. This evidence establishes
reconstruction within the declared local family, not task correctness or cost
savings. Use `residual show rN@1 --decisions` to recover the full contract before
answering from a new context; relation authoring/disclosure costs still count.

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
the frame: in plain AF1, within a function, every operation that cannot
resolve, then every terminator that cannot. In AF1-X, a malformed
terminator is reported before an operation whose type depends on it; the
operation is still listed with that dependency. The first line carries the first problem with
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
plain AF1 and refuses every form below exactly as before (its extension
keys `ripple` and `test_tables` are refused at `/ripple` and
`/test_tables` with the fix `add "afx": 1`, the envelope they need); a
frame whose `afx` is not the number 1 is refused at `/afx`. `sley-agent help
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
`__err`, `__none`). Authored names in blocks that use the dialect and their
function parameters may not contain `__`; plain blocks retain AF1's name
rules. Checked
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
changes stated once. `sley-agent help afx-reference` is the full reference text;
`help afx` serves the concise authoring guide. Intents
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
restated parameters `f` already has at the head, an unambiguous value fill
(the constant load `n__v<k>` that passed `"value"` as argument `k` of call
`n`) may load a changed value. When several fills could belong to earlier
intents, a changed value is an obligation at `/ripple/<i>/value`, never a
guess. A call or test of `f` the frame carries in its old spelling takes
the arguments of the committed call or TestCase of the same name. A call
explicitly changed after the intent that differs from the committed call
is an obligation at `/ripple/<i>` unless `"frame_calls": "new"` covers it;
it is never silently replaced. Live tests keep their arguments. A `guard`
is applied when its function already has the exact shape its mode derives
(entry: a guard block in the chain of guards the function starts with,
calling `g` on the value the guards before it leave for `p`, its checked
value held by a `tuple`/`tuple_get` block, its error passed to the block
that returns it or to the named handler; preserve: a block ends with that
call, continuing without the checked value). An intent with nothing to
derive is listed as `already applied`.

In `try --on`, a follow-up's intent replaces the draft's intent with the
same identity where it stands: `arity` of the same function, or `guard`
with the same checker and `arg`; other intents are appended. Only the
draft's intents are replaced, never one the same follow-up states. When
the draft or the follow-up has several intents of one identity (two guards
with the same checker and `arg`, say, for functions that need different
handlers), a follow-up intent replaces the draft intent of that identity
naming the same functions in `"in"`; one naming other functions is
appended when each draft intent of that identity is replaced, and is
otherwise refused (`AGENT_FRAME_INVALID` at its `/ripple/<j>`), since which
one it replaces cannot be told. An `arity`
intent the follow-up does not restate records the follow-up's provenance
in `"after"`: the functions, tests and test tables it states (a test
without a name as `"(unnamed)"`, which stands for every test without a
name, since one cannot be told from another). The intent's
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
  `E`, and without one it is a hole. The error then reaches the handler
  before anything else `f` does, so a handler that reads (itself or through
  a block after it) a value another block of `f` computes is a hole at
  `/ripple/<i>/in/<j>`: that value is no longer computed on every path to
  it. A block is never chosen as a route by
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

### 5.4 Structured function bodies

An AF1-X function may carry `"body"`, a list of statements, instead of `"blocks"` (ADR-0054). The
workbench lowers it to ordinary blocks before expansion; everything after that (expansion, compilation,
validation, tests, submission) is unchanged, and `view --after cN` shows the lowered blocks.

    {"fn": "f", "params": [["xs", "Vec<i64>"]], "returns": "i64", "body": [statement, ...]}

Statements: `["let", x, e]`; `["var", x, T, e]` and `["set", x, e]` (only a `var` may be set);
`["if", c, [..], [..]]` (else optional); `["for", x, xs, [..]]` (elements of a vector, in order; the
index is not visible); `["while", c, [..]]`; `["return", e]`, or in a function returning `Result`,
`["ok", e]` and `["fail", "Case"]`; `["trap"]`. Variables declared in a branch or loop body go out of
scope at its end. A `let` or `var` in the same statement list as an earlier declaration of the name
rebinds it; inside a nested branch or loop body, declaring a name that is declared outside that body
(a variable, a parameter, or a `for` variable) is refused, because hiding and overwriting the outer
name are both familiar readings. A body must not reach its end without returning, and a statement
after one that returns is refused.

Expressions: a name, an integer, `true`, `false`, a typed literal `{"type": T, "value": v}`, or
`[op, args...]` with

- `add sub mul div rem neg abs`: checked; a failure traps, `op?` returns the `ArithmeticError` from a
  Result function, `op?Case` returns `Err(Case)`; `div` truncates toward zero, `rem` takes the
  dividend's sign; `abs` of the minimum value fails like `neg`;
- `to`: `["to", T, x]` converts between integer types; a value outside `T` fails like a checked
  operation (`to?`, `to?Case`);
- `min max clamp`: total; `clamp(x, lo, hi)` is `min(max(x, lo), hi)`;
- `eq ne lt le gt ge not`, and `and or`, which evaluate both operands;
- `if`: `["if", c, a, b]` evaluates only the chosen value;
- `len` (`u64`), `get` (`[xs, i]`, `u64` index; out of range traps or `get?Case`), `field`
  (`[r, "name"]`), `call` (`["f", args...]`; `call?` unwraps a Result).

Integer literals take the type of the other operand, the declared variable or the return type, else
`i64`; integer types never mix. A refusal names the authored location (`/fns/0/body/2/1`); a compiler
refusal inside lowered blocks is mapped back to the statement that produced it, with the lowered
pointer beside it. A `body` in `patch` is refused: restate a body function through `fns`.
`help structured` is the short reference.

### 5.5 Familiar text proposals (optional frontend)

`try --familiar <file | ->` reads a textual proposal and parses it into exactly the structured-body
frame of section 5.4 (ADR-0055). The frontend is the cargo feature `familiar` of `sley-agent`;
building without it removes the switch (it is then refused with a message naming the feature) and
changes nothing else. The switch must be given: no input is detected or treated as text otherwise,
and inline text is not accepted.

    fn name(p: T, q: T) -> T {
        let x = e            var t: T = e          t = e
        if c { .. } else if c { .. } else { .. }
        for x in xs { .. }   while c { .. }
        return e             return ok(e)          return err(Case)      trap
    }

Each construct maps to one structured form: `let`, `var`, `set`, `if`, `for`, `while`, `return`,
`ok`, `fail`, `trap`. Expressions, loosest first: `a if c else b` (`["if", c, a, b]`, only when `if`
is on the line of `a`), `or`, `and`, `not`, `== != < <= > >=` (`eq` ... `ge`, no chaining), `+ -`,
`* / %` (`add sub mul div rem`), unary `-` (`neg`; a minus before a literal makes a negative
literal), then `f(args)` (`call`), `x.field`, `xs[i]` (`get`). `min max abs clamp len` are the
built-ins of the same names, `to(T, x)` is `["to", T, x]`, and `try(e)` / `try(e, Case)` give every
failing operation in `e` without its own mode the `?` / `?Case` mode (helper calls become `tcall?`,
which unwraps a returned Result). Literals: decimal integers (the 64-bit range; `-9223372036854775808`
is accepted), typed integers (`7u64`), `true`, `false`, and JSON-escaped text in double quotes.
Comments run from `#` to the end of the line, or fill a line that starts with `//`. Newlines and
optional `;` separate statements.

There is no other syntax. Methods, ranges, `break`/`continue`, compound assignment, floating-point
or hexadecimal literals, slices, tuples, bitwise operators, a trailing `//` and spellings from other
languages (`&&`, `!`, `elif`, `let mut`, `True`, `?`) are refused with `AGENT_FRAME_INVALID` at
their line and column, and the revision is recorded as a text draft: its `input.txt` keeps the
text and no frame or candidate is made. A function may not be named after a built-in.

The parsed frame then takes the ordinary path, so arithmetic, conversion, scoping and failure are
exactly those of section 5.4. The draft revision keeps the text as `input.txt` and the structured
frame as `frame.json`; later steps (`try --on`, `fill`, `submit`, `commit`) read the frame, never
the text. Refusals keep the frame's pointers (`/fns/0/body/2/1`) and add, for familiar input, each
pointer's line and column (`"text": {"line": L, "column": C}` on the obligation, and a line in the
refusal). Positions are diagnostics: they never enter canonical bytes, identities or the program.
Nothing renders a program back into this syntax. `help familiar` is the short reference.

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
signature refusal, or, for a function the frame does not state, the
ripple intent that derived it (`/ripple/<i>...`, function-wide; the
`fix:` line then repairs or drops that intent), else says that the
function is not in the frame. Names the
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
| `AGENT_RESIDUAL_PARSE` | malformed residual envelope, duplicate or unknown member, invalid UTF-8, prohibited float, or exceeded parser limit |
| `AGENT_RESIDUAL_VERSION` | residual envelope version is unsupported |
| `AGENT_RESIDUAL_BINDING_STALE` | a bound head, policy, name map, draft, implementation identity, or other dependency changed or became unavailable |
| `AGENT_RESIDUAL_FRAGMENT_UNKNOWN` | fragment family or version is unavailable |
| `AGENT_RESIDUAL_FRAGMENT_SHAPE` | bindings or source region do not match the fragment's supported shape |
| `AGENT_RESIDUAL_CHOICE_MISSING` | an explicit required author decision is missing |
| `AGENT_RESIDUAL_CHOICE_UNKNOWN` | an author decision is outside its declared domain |
| `AGENT_RESIDUAL_SEMANTIC_UNRESOLVED` | an omitted semantic decision lacks adequate provenance |
| `AGENT_RESIDUAL_FAMILY_EMPTY` | declared construction constraints admit no completion |
| `AGENT_RESIDUAL_FAMILY_INCOMPLETE` | completeness of the declared construction family is not established |
| `AGENT_RESIDUAL_VOCABULARY_INCOMPLETE` | the decision vocabulary cannot distinguish required completions |
| `AGENT_RESIDUAL_CONSTRAINT_CONFLICT` | supplied policies or bindings conflict |
| `AGENT_RESIDUAL_LIMIT` | a residual construction or planning bound was reached |
| `AGENT_RESIDUAL_INCONCLUSIVE` | a bounded oracle cannot establish the requested result |
| `AGENT_RESIDUAL_SCOPE` | targets are not exactly identified or have an unsupported scope |
| `AGENT_RESIDUAL_PRESERVE` | an edit lacks an explicit preservation request, or a derive request supplies one |
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
  are changed or deleted by it. In JSON each `changed` entry carries this as
  `existing_export`. A created function's entry, and the entry of a function
  whose visibility the candidate changes, also carries its `visibility` after
  the candidate, so a created exported function reads
  `"existing_export": false, "visibility": "exported"`;
- `tests: X/Y passed [authored A, imported I, provided P]` (counts that are
  zero are left out; `; replaces provided t_1` names the live tests the
  candidate replaces) and one line per failing test; passing tests are
  listed only with `--verbose`;
- `tests: 0 ran` when no TestCase ran, as section 2 describes;
- the `next:` step, which layers on the draft (`try --on d1`), repairs it
  (`fill`) or submits it (`submit d1`).

The JSON result carries the same facts: `next` holds a Valid candidate's next
step (without the `next: ` prefix), and `tests_failed` counts failing authored
and public tests when any fail, so a `Valid` verdict with a failed test is not
read as success.

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

A rebased frame omits a `delete` entry or `patch` block deletion when the
new head already lacks that entity or block. A same-named live entity or
block keeps the deletion, and the rest of the frame is still compiled and
judged on the new head. Thus a tests-only follow-up can follow a committed
deletion without trying to delete it again.

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

Commands that register a workspace append one JSON line to
`.sley/events.jsonl`, also when it is refused before it opens the
workspace (an unreadable frame or delta, a malformed case file), with
these core keys: `seq`, `cmd`, `draft`,
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

Residual try registers its workspace only after strict request parsing, so
invalid residual envelopes create no local ledger. A constructed residual
request adds a `residual` object with numeric/boolean counters for request
bytes, expansion bytes, provenance entries and use of residual try. Other
commands retain their existing key set. Section 2.4 describes the remaining
external accounting obligations.

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
