# ADR-0023: Crash recovery ownership and retry boundary

Status: accepted for S20-530 implementation

Date: 2026-08-28

## Context

S20-150, S20-390, S20-500, and S20-180 already define crash-safe local write
patterns for immutable objects, receipts, the fixed accepted head, native refs,
and GC. Their component reports and error namespaces are intentionally
separate. S20-530 must prove the master goal's old-or-complete-new invariant at
every transaction durability boundary without creating a second repair engine
or treating repository debris as intent.

Two retry hazards need an explicit system rule. First, a process may unlink an
owned stage or GC candidate and fail before syncing its containing directory.
On retry the entry is already absent, so cleanup based only on a fresh removal
count cannot make that prior unlink durable. Second, cleanup under shared
maintenance ownership could interleave with otherwise valid transaction or ref
work even if each component lock is locally correct.

The fixed accepted head and a named branch are also independent pointers. A
durable transaction may become accepted before a branch advance starts or
finishes. Recovery must distinguish this valid lag from partial transaction
state.

## Decision

S20-530 freezes the 100-row matrix in
`docs/spec/CRASH_RECOVERY_MATRIX_V1.md`. Every durability row has one test-only
injection boundary, one immediate visibility classification, and one
deterministic recovery or retry assertion. Corruption and limit evidence-family
rows instead freeze an ordered subcase vocabulary. Every subcase uses a fresh
fixture, and every limit subcase proves exact-limit success plus limit-plus-one
failure without mutation, partial report, or partial success.

The first five corruption rows are ordered owned-entry classification families.
They prove one exact positive stage shape plus prefix/suffix lookalikes, PID and
token grammar, fan-out and final-path binding, ASCII and non-UTF-8 unknown
regular leaves, symlinks, and non-regular entries. The checker accepts only a
brace-delimited function with plain `#[test]` as its sole function attribute
and an actual in-body assertion macro. Commented, conditionally compiled,
ignored, and `should_panic` functions cannot supply evidence. Production
scanning excludes individual exact `#[cfg(test)]` items without discarding
later production code.

Owned-entry evidence derives `expected_result`, `owned_stage_removed`,
`preserved`, and `no_mutation` from exact recovery outcomes, tree deltas,
reports, path snapshots, and cleanup canaries. It does not introduce shadow
locals named after those plan fields. Forty-eight subcases use one exact
five-sequence owned-entry body. `COR-02/symlink` and `COR-04/symlink` use their
exact two-operation multifault bodies as the sole body authority and bind the
same owned-entry facts to the precedence winner and first-operation snapshots.
Their secondary probes use distinct M2 helper identities, and their classified
winner variants carry exact empty `Error::source` chains.

This sole-authority rule applies to every grouped multifault leaf: its matrix
base error fields project operation one's exact M2 result and unchanged-tree
snapshot, so no redundant generic corruption fixture, result local, or
preflight body can compete with the M2 plan. The two limit multifault cases keep
their normal exact-limit/limit-plus-one runtime prefix and append a fresh,
isolated M2 suffix. That suffix changes only the selected limit from one to two
between calls, proving the limit winner first and the corruption loser second
without owner-tree mutation.

Test-only non-regular fixtures bind a Unix datagram socket at one guarded short
path under the system temporary directory and rename the inode into the final
owner path. Direct final-path socket binds are not evidence. COR-03 explicitly
replaces the trusted-genesis `heads/accepted` entry and records it as changed.
COR-04 and COR-05 establish canonical refs layout and `locks/refs.lock` before
their fresh baseline so production layout creation is not misreported as
recovery mutation.

`COR-06` and `COR-07` keep their eight public labels as namespaces but expand
to 73 and 159 exact error-preservation leaves. The leaves preserve concrete
owner codes, variant paths, and complete `Error::source` chains. All 71
transaction-visible revision errors are independently wrapped for both the ref
target and origin ancestry lanes, while 17 ref-owned cases remain distinct.
The complete matrix map therefore contains 419 unique mapped Rust tests rather
than one anchor test per namespace.

The visible-revision authority binds each SCB result to a production-reachable
nested carrier. State-root structure owns contract, epoch, field-order,
map-order, and duplicate-map faults; typed candidates own Boolean, UTF-8, and
noncanonical-float faults. The unreachable outer-receipt label-normalization
leaf is removed, and decoder-first candidate descriptor and payload faults
expect `SCB_UNION_INVALID`.

One generated 232-key corruption-fixture registry owns the concrete stimulus
for every grouped leaf. It derives grouped error tuples and recovery-operation
ownership, and freezes the owner source, target role, artifact and identity
recipes, path recipe, closed corrupter and probe classes, selector, direct
witness facts, and any role that must be distinct. The 71 visible-revision
recipes are generated once and instantiated for accepted, branch-head, and
branch-origin roles. This avoids three copied grammars while still requiring
head and origin IDs, receipt paths, and role-specific object paths to differ.
Two accepted-head-pointer recipes and 17 explicit ref-owner recipes complete
the 232 leaves. Evidence records the registry-derived plan digest and direct
fixture assertions; test helpers may return observations but never expected
codes, roles, booleans, or usage.

Grouped M2 probe calls have a separate private helper identity from their
direct corruption-leaf probes. The leaf registry continues to name the
three-argument diagnostic helpers that bind an observation and exact fault
path. A closed 11-record grouped adapter registry selects seven private
two-argument helpers that accept only the owner fixture and grouped M2 fixture.
The checker freezes each adapter's key, side, owner, exact signature, call
arity, and normal-build exclusion. This prevents Rust helper-name collisions
without weakening leaf probe authority. The ref-digest cycle's secondary side
continues to use its checker-owned cycle descriptor and has no direct adapter.

The 62 non-GC durability rows use six closed retry protocols:
`H1_R1`, `H1_R2`, `H1_O1`, `H1_R1_O1`, `H2_O1`, and `H2_R1`. Only 14 `H2`
rows invoke the same fault helper and selector payload twice. Every row binds
the injected helper result to the exact component error code, orders its normal
recovery or owner retry calls, and snapshots the exact owner tree plus two
owner-root-relative paths at every protocol phase. An exact optional-path
snapshot helper represents absence without converting other metadata errors to
absence. Report, pointer, and closure assertions inspect the bound operations
and snapshots; local success booleans are not evidence.

The four cross-component success rows use trusted genesis `old` and its direct
complete child `new`. One exclusive guard executes transaction recovery, ref
recovery, transaction retry, and ref retry in order while both visibility
pointers are snapshotted around each round. `ANC-01` freezes a four-node linear
accepted chain. `ANC-02` freezes `genesis -> shared -> {alpha, beta}` and a
four-transaction union across two visible branches. Ref recovery supplies
ancestry requests in canonical branch-name order, invokes the transaction
verifier once, and reports the verifier's exact unique union count.

Successful CROSS and ANC evidence uses one contiguous critical statement
window from exclusive-guard acquisition through both ordered snapshot rounds,
guard release, and exact post-release reads. No unbound repair, extra recovery,
or other statement may intervene between a bound operation and its observed
state.

The checker forbids the legacy `list_branches_locked`/`resolve_locked` path in
recovery. It freezes a private raw-record preflight that consumes the bounded
inventory's exact origin/ref final paths, charges record bytes before reads and
visible/orphan counts before retention, decodes each record once, sorts refs by
canonical name, rejects duplicates, validates exact path/name/digest/binding,
and produces immutable visible and orphan vectors without transaction access.
The visible vector must feed exact request construction and the single
transaction verifier contiguously. This rejects legacy-helper substitution,
unmetered verification, post-sort reversal, clearing, filtering, or other
mutation of verifier order.

The frozen specification, this ADR, the structural checker, and the validation
runner remain byte-for-byte immutable after contract review. Implementation
status lives in machine summary and closeout evidence instead of changing the
specification status. Contract reviews use exact phase-specific verdicts bound
to the frozen hashes. Final implementation reviews bind separately to the
complete closeout source-set digest, validated commit, and review-free evidence
payload digest.

The test plan also records every mapped function's owning source,
Cargo-qualified name, and raw brace-delimited body digest. Its canonical
mapped-body digest is copied into closeout evidence before implementation
reviews are attached. A review cannot approve one set of test bodies and be
replayed against another set with the same test names.

The frozen checker does not hash whole evolving owner sources. It freezes the
normalized normal-build maintenance-lock authority module, exact immutable
guard and wrapper bodies, and the required guarded call edges. It separately
scans every owner source for root aliases, authority shadows, deferred mutation,
and escaped maintenance-lock path authority. Recovery cores and mapped tests may
then be implemented without changing the frozen checker, while their public
surfaces, first guard validation, semantic assertions, and runtime exclusion
behavior remain checker-owned. A checker self-control adds one required recovery
type and one required mapped test to the in-memory source set and proves that
contract authority advances to semantic validation instead of rejecting valid
implementation growth.

Stable commit, branch-create, branch-advance, accepted-head, and maintenance
authority bodies are frozen before recovery implementation. The two recovery
cores remain structurally checked evolving regions because their final bodies do
not exist at contract freeze. Each public guarded recovery method becomes one
exact limits-profile delegate, and the corresponding private `*_and_limits`
core must validate the borrowed exclusive guard as its first direct statement.
The preimplementation checker accepts the earlier public guarded body only until
that private core exists. A synthetic closeout-shape control proves the delegate
and inner guard checks are jointly satisfiable. Runtime `CROSS-05` exclusion
tests bind those cores to the real lock boundary. The checker does not claim that
lexical inspection can prove the absence of every deliberately computed internal
lock path. Final Vulcan review therefore owns arbitrary same-process lock-path
mutation analysis over the exact closeout source-set digest, in addition to the
checker and runtime evidence.

S20-530 v8 adopts one narrow hybrid proof path for entry paths and control
ancestry. The checker still generates the complete pre-exception inventory for
all 30 events and preserves every resolved leaf as `STATIC_PASS`. It may assign
`UNRESOLVED_BY_V8_STATIC_RESOLVER` with
`FINAL_SPECIALIST_REVIEW_REQUIRED` only to a generated unresolved leaf that
matches the exact frozen ledger. The entry partition is 110 complete leaves,
90 static passes, and 20 exceptions across 13 events. The control partition is
5,213 complete atoms, 3,547 static passes, and 1,666 exceptions across 23
events. Their ordered fingerprint SHA-256 is
`5b475cc8c4c1f5abbab836d29fc28fe0270fa0eef58b73b6d01a1253f051c816`.
This is an equivalent proof path, not a broad bypass: record, order, source,
attribute-chain, body, lexical-site, parent-manifest, classification, or digest
drift fails closed, while every runtime observation and final specialist review
remains required.

The standalone `scripts/reconcile_s20_530_exception_ledgers.py` witness imports
no checker resolver code. It independently reconstructs the complete structural
inventories from serialized test-plan manifests and verifies the exact static
and exception partition. Its bytes join the frozen contract set, its identity is
bound in the test plan and closeout evidence, and its invocation is an exact
Tier 2 command. The immutable v7 contract and evidence remain historical
authority for v7; v8 refreezes the amended specification, ADR, checker, runner,
reconciler, partition ledgers, and fresh review receipts as one new contract.

Closeout execution evidence covers the full workspace-input closure, including
all tracked and nonignored untracked source, test, manifest, lockfile, script,
specification, and frozen-evidence inputs. Only closeout outputs are excluded.
The runner refuses other dirty inputs and all workspace-input symlinks, records
the canonical regular-file path/mode/hash closure
and validated Git commit/tree, proves each mapped plain test occurs under its
exact qualified name in the actual owning-crate Cargo test list, and captures
every Tier 2 command's output,
zero exit status, and empty skipped-check list. The checker rehashes the closure
and command logs before accepting the evidence.

Excluded output paths are never source-set inputs, but an existing workspace or
Git entry at one of those paths must still be a regular file. Summary and
closeout JSON reads use bounded descriptor-relative `O_NOFOLLOW` traversal,
stable pre/post metadata, exact uid/gid and safe-mode checks, and strict UTF-8
JSON. Duplicate keys at any depth, `NaN`, infinities, exponent overflow, parent
symlinks, final symlinks, non-regular entries, and oversized artifacts fail
closed before review-payload hashing.

That execution proof is a workspace-source closure under an exact recorded
host trust boundary, not machine-wide hermeticity. The authoritative runner is
entered only as `env -i LC_ALL=C PATH=/usr/bin:/bin /usr/bin/python3 -I -B
scripts/run_s20_530_validation.py`. The OS loader, initial isolated Python
entry, and initial checker and runner bytes are bootstrap TCB. Authoritative
closeout verifies all frozen contract hashes before nested contract or hostile
self-tests; direct `--self-test` is diagnostic only. The named host TCB also
includes the Linux kernel, procfs, system libraries, Rust dependency
acquisition, the recorded toolchain, every resolved executable path component,
and every actor able to rewrite one of those components.

Executable files are rebound by uid, gid, non-writable mode, and SHA-256. Git
local config and comment-only info/exclude bytes are fd-bound before and after
each Git command; includes, attributes, grafts, alternates, modules, replace
metadata, and shallow state are forbidden. Validation output paths are opened
component by component relative to trusted descriptors with `O_NOFOLLOW`, and
bounded output reads require stable pre/post descriptor metadata. The model
excludes a command that fabricates then restores authority and an unrelated
actor holding the validation uid or gid that mutates then restores repository,
output, or executable-path authority between checks.

All repository-level recovery cleanup holds `locks/maintenance.lock`
exclusively. Transaction and ref repositories expose component-owned recovery
under an exact already-held exclusive guard so a caller can compose one full
local recovery sequence without an aggregate repair report. Default recovery
methods acquire the same exclusive ownership themselves. The lower-level
`sley-store::recover_staged` API retains its explicit caller-held exclusivity
precondition because the store cannot depend upward on the transaction guard;
transaction recovery invokes it only while the guard is held. The dependency
remains `sley-repo -> sley-txn -> sley-store`.

Recovery validates owned traversal shape and removes only exact stage names.
Every successful cleanup visit syncs each validated leaf directory even when
the current invocation removes zero files. This closes the remove-before-sync
retry gap for object, receipt, fixed-head, and ref stages. Transaction layout
creation applies the same validate-and-parent-sync rule on both new and
`AlreadyExists` paths.

Each owner completes one read-only preflight before any cleanup mutation. That
preflight validates the full owned tree shape, closed counters, visible pointer
and immutable closure, complete ancestry, and report bounds. A corruption or
limit failure therefore leaves that invocation's exact owner tree byte-for-byte
unchanged. The later cleanup phase may fail after a partial stage deletion, and
the fixed retry rows cover that separate host-I/O case. Sequential composite
recovery does not roll back cleanup completed by an earlier owner when a later
owner fails.

Collection remains owned by `sley-repo::GcReport`. A guarded collect retry
first completes the full bounded read-only plan, then syncs every validated
object leaf directory before deletion or successful return. This preserves
corruption/limit precedence while redurabilizing a prior unlink that failed
before directory sync, without inventing a deletion journal or claiming that
the absent object was deleted by the retry.

The accepted outcome is evaluated per visibility pointer. A fixed accepted
head or named ref is either its old complete verified target or its complete
new verified target. All four accepted-old/new and branch-old/new combinations
are valid when both visible targets independently verify. No recovery path
promotes stages, rewinds pointers, follows the fixed head implicitly, or
guesses from time or filesystem order.

Recovery verifies complete bounded transaction ancestry through trusted
genesis, not only the visible receipt and direct parent. The fixed accepted
head has a 65,536-transaction limit. Ref recovery has the same per-branch limit
and a 262,144-transaction unique-union limit. Directory scans and report lists
also have closed owner-specific ceilings. Limit failures return the existing
owner resource code, except object recovery uses its existing `STORE_IO`
because the store has no general recovery-resource symbol.

Transaction counts alone are not a work bound because one legal revision may
contain a large state root and object closure. Accepted recovery therefore also
caps cumulative receipt bytes at 1 GiB, binding visits at 4,194,304, object
verifications at 2,097,152, and object bytes at 1 GiB. Each visible branch uses
the same per-pointer limits. Ref recovery additionally caps the canonical union
at 4 GiB receipt bytes, 16,777,216 binding visits, 8,388,608 object
verifications, and 4 GiB object bytes. Checked counters are charged before the
corresponding read, scan, or cached-fact use. Each branch is charged the exact
logical work of its isolated ancestry even when a convergent transaction is
reused from cache. The canonical union is charged only for first-time actual
work, so per-branch outcomes are independent of branch order while the union
still measures physical recovery work.

A transaction-owned multi-head verifier enforces those limits. Its public
surface is branch-recovery-only with fixed numeric ceilings; accepted recovery
uses a private single-head core. It returns a typed cycle, budget-exceeded, or
underlying-verification outcome so accepted and ref owners map only their own
conditions without collapsing a genuine nested transaction error. Generic
expected revision claims let the transaction owner verify the origin first and
the head second before ancestry, while returning only request and claim indices
on mismatch. The head is derived from the second claim, so no duplicate head
authority exists. Per-head logical usage and union actual usage remain auditable
without returning complete revision payloads.

The claim's six private fields and the request's two private claim slots are
exact checker-enforced layouts. An extra private head, limit, or authority field
is a contract violation even when the public constructors retain their frozen
signatures.

Object-byte precharge stays store-owned. A narrow
`ObjectStore::bounded_object_len` method performs the existing confined
final-path, file-kind, and standalone-size checks and returns exact metadata
length. The transaction verifier charges that length before using the existing
verifying object read. It does not duplicate store fan-out interpretation.

The public verifier acquires `accepted.lock` exactly once and must not be called
while that lock is already held. Transaction recovery uses its private core
without recursive acquisition. Exclusive maintenance makes it safe to release a
pointer-snapshot lock before continuing read-only ancestry verification.

Ref preflight separately caps cumulative origin-record reads at 1 GiB and
visible-ref record reads at 256 MiB. It charges metadata length before each
content read, so high record counts cannot multiply an individually legal
codec size into unbounded decoding work.

An exact stale GC witness is recoverable only under the same exclusive
maintenance guard. Recovery validates its type and exact `SLEYGC01` bytes,
removes it, and syncs the lock directory. It never uses age to classify a
witness and never deletes malformed witness state. Collection itself remains a
separate caller-owned retention-snapshot operation.

S20-530 supersedes the earlier witness-first sequencing. Collection now
acquires exclusive maintenance before creating and syncing the witness, and it
keeps maintenance held until the witness is removed and the lock directory is
synced. Witness-first acquisition is forbidden. An exclusive recovery owner
can therefore distinguish an exact stale witness from live collection without
using age or process metadata.

An empty witness or strict byte prefix of `SLEYGC01` is the only incomplete
witness shape owned by an interrupted create/write. Exclusive recovery may
remove that bounded prefix and reports it separately from an exact witness.
Any other bytes or file kind remain corruption and are preserved with a hard
failure.

Fault selectors remain private test mechanisms. The normative specification
freezes one direct `#[cfg(test)]` enum per owner, 70 ordered variants, their
exact payload exceptions, and one exact private helper mapping per row. No
selector identifier or disabled-selector plumbing survives the normal-build
projection. Stable machine error vocabularies are unchanged. Production APIs gain only the exact
exclusive-maintenance recovery methods, the closed GC-witness recovery status,
the bounded object-length metadata bridge, the transaction-owned typed
multi-head verifier and its bounded report types, and one checked ancestry-count
field on each existing component report frozen by the normative contract.
Component-owned reports remain separate and count only removals performed by
the current call.

An absent accepted head remains the existing uninitialized recovery state and
reports `accepted_transaction_id=None` with zero verified ancestry. S20-530
adds no initialization marker and does not guess whether an external actor
removed an earlier head. The separate accepted-state read continues to return
`REF_HEAD_MISSING` when initialized state is required.

Trusted-genesis crash rows make that absence explicit. Before genesis head
rename, recovery may report only uninitialized absence. After rename, it may
report only one complete verified trusted genesis with ancestry count one.

## Consequences

- Recovery excludes cooperating commits, ref operations, and GC while cleanup
  and verification are in progress.
- Repeated recovery is observationally idempotent even after a previous
  delete-before-sync failure.
- Complete orphan objects, receipts, and branch origins may remain, but cannot
  become visible authority through recovery.
- Complete transaction ancestry is verified under closed scan and report
  bounds before cleanup begins or recovery reports success.
- An exact stale GC witness has a deterministic exclusive recovery path, while
  malformed witness state fails closed.
- GC acquisition now follows maintenance then witness, eliminating the live-
  waiter ambiguity in witness recovery.
- Legal maximum object inventory plus exact fan-out directories remains within
  recovery limits because directory, leaf, final, and stage ceilings are
  counted separately.
- A response-lost operation can be fully durable even though its caller saw an
  error; verification classifies current state without replaying intent.
- Full repository recovery is composition of owner APIs, not a new record,
  transaction type, or repair policy.
- S20-530 adds tests and bounded durability hardening only. Comparison, merge,
  clone, protocol, runtime, benchmark, and release work remain separate.

## Rejected alternatives

### Recover under shared maintenance ownership

Component locks would prevent some races but would not freeze the repository as
one recovery snapshot. Cleanup is an exclusive maintenance operation.

### Sync only when the current call removes a file

This cannot close an earlier remove-before-sync interruption because the retry
observes no file. Validated leaf directories must be synced on a zero-removal
retry.

### Journal every deletion

An additional mutable recovery journal creates another crash protocol and is
unnecessary for this local bounded matrix. Idempotent enumeration plus leaf
resync is sufficient.

### Infer pointer repair from orphan records

An orphan receipt or branch origin does not prove which user-visible pointer
was intended to advance. Promotion or rollback would guess authority and is
forbidden.

### Create one aggregate recovery report

That would blur owner error namespaces and encourage a second repository
engine. The caller may retain both existing component reports under one guard.

### Treat an unlinked but unsynced GC object as a completed deletion

`GcReport::deleted_objects` promises successfully deleted and synced objects.
An object unlinked before a failed directory sync remains `failed_object` and
is not added to the completed list. Retry leaf resync closes durability.
