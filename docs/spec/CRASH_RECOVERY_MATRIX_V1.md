# Crash Recovery Matrix v1

Status: S20-530 contract frozen; implementation state is tracked separately.

## 1. Scope and authority

This contract defines deterministic local recovery and crash-injection evidence
for the S20-150 object store, S20-390 fixed accepted head, S20-500 native refs,
and S20-180 garbage collection. It does not add a second commit engine or a
cross-component repair oracle.

Component ownership remains unchanged:

- `sley-store` owns immutable object promotion and object-stage cleanup;
- `sley-txn` owns receipts, the fixed `accepted` head, transaction-stage
  cleanup, complete revision verification, and repository-maintenance guards;
- `sley-repo` owns branch records, named refs, ref-stage cleanup, and GC;
- callers compose component recovery while holding one exact exclusive
  repository-maintenance guard.

The dependency direction remains `sley-repo -> sley-txn -> sley-store`. S20-530
must not introduce the reverse edge, an aggregate repair record, runtime
authority, branch-local commit, merge, clone, protocol, or release behavior.

Fault injection is test-only. No production API accepts a fault selector.

Once this contract is frozen, this specification, ADR-0023, the structural
checker, and the S20-530 validation runner remain byte-for-byte immutable.
Implementation state is recorded only in the machine summary and closeout
evidence. Closeout never rewrites a frozen contract status or regenerates a
contract review against changed normative text.

## 2. Accepted outcome model

After an injected interruption in an already-initialized ordinary commit, the
fixed accepted head is exactly one of:

1. the old complete verified transaction and root; or
2. the complete new verified transaction and root with its exact receipt and
   object closure.

No partial receipt, root, object closure, head record, or ref record may be
accepted as visible state. Unreachable immutable objects and complete orphan
receipts may remain for later GC or pack policy. Recovery never promotes them,
rolls a visible pointer backward, or guesses intent from timestamps, names,
enumeration order, stage contents, or record age.

Trusted-genesis initialization has no old accepted head. After an injected
genesis interruption, recovery reports exactly one of:

1. `accepted_transaction_id=None` and zero verified ancestry; or
2. one complete verified trusted genesis with ancestry count one.

The same no-partial rule applies. A complete orphan genesis receipt before head
visibility does not make the repository initialized.

The fixed accepted head and each named branch are separate visibility pointers.
The following cross-component states are valid after interruption:

- accepted old, branch old;
- accepted old, branch new;
- accepted new, branch old;
- accepted new, branch new.

A visible named branch may advance only to a complete verified durable
transaction. A branch pointing to incomplete, missing, corrupt, or mismatched
transaction evidence is a hard failure. Recovery does not infer that a branch
should follow the fixed accepted head.

## 3. Exclusive recovery boundary

Every repository-level cleanup mutation requires exclusive ownership of
`locks/maintenance.lock`. Public transaction and ref recovery methods either
acquire that ownership themselves or accept an already-held guard that covers
the exact canonical repository root and is exclusive. A shared or wrong-root
guard fails with the component's existing I/O or exclusive-lock error. Guard
inactivity is not a public state: ownership ends when the guard is dropped.

`sley-store` cannot depend upward on the transaction-owned maintenance-guard
type. Its low-level `recover_staged` API therefore retains an explicit
caller-held exclusive-startup/recovery precondition. Repository recovery calls
it only from `sley-txn` while the exact exclusive maintenance guard is held.
Direct store unit tests use one isolated owner. This documented precondition
does not authorize a reverse dependency or an unguarded composite recovery
path.

The exact repository API additions are:

```rust
pub fn TransactionRepository::acquire_exclusive_maintenance(
    &self,
) -> Result<RepositoryMaintenanceGuard, CommitError>;

pub fn TransactionRepository::recover_with_maintenance(
    &self,
    maintenance: &RepositoryMaintenanceGuard,
) -> Result<RecoveryReport, CommitError>;

pub fn BranchRepository::recover_refs_with_maintenance(
    &self,
    maintenance: &RepositoryMaintenanceGuard,
) -> Result<RefRecoveryReport, BranchError>;

pub fn recover_gc_witness(
    store: &ObjectStore,
    maintenance: &RepositoryMaintenanceGuard,
) -> Result<GcWitnessRecoveryStatus, GcError>;
```

`GcWitnessRecoveryStatus` is exactly the closed public enum `{ RemovedExact,
RemovedIncomplete, Absent }`.

Complete ancestry verification remains transaction-owned. The narrow public
inter-crate recovery surface is:

```rust
pub fn ObjectStore::bounded_object_len(
    &self,
    object_id: ObjectId,
) -> sley_store::Result<u64>;

pub struct RecoveryRevisionClaim {
    transaction_id: TransactionId,
    workspace_id: WorkspaceId,
    state_root: StateRoot,
    schema_epoch_id: SchemaEpochId,
    policy_root_id: PolicyRootId,
    dependency_roots: Vec<StateRoot>,
}

impl RecoveryRevisionClaim {
    pub fn new(
        transaction_id: TransactionId,
        workspace_id: WorkspaceId,
        state_root: StateRoot,
        schema_epoch_id: SchemaEpochId,
        policy_root_id: PolicyRootId,
        dependency_roots: Vec<StateRoot>,
    ) -> Self;
}

pub struct RecoveryAncestryRequest {
    first_claim: RecoveryRevisionClaim,
    head_claim: RecoveryRevisionClaim,
}

impl RecoveryAncestryRequest {
    pub fn with_claims(
        first_claim: RecoveryRevisionClaim,
        head_claim: RecoveryRevisionClaim,
    ) -> Self;
}

pub struct RecoveryWorkUsage {
    pub receipt_bytes: u64,
    pub binding_visits: u64,
    pub object_verifications: u64,
    pub object_bytes: u64,
}

pub struct RecoveryAncestryHeadReport {
    pub head_transaction_id: TransactionId,
    pub verified_transactions: u64,
    pub work: RecoveryWorkUsage,
}

pub struct RecoveryAncestryReport {
    pub heads: Vec<RecoveryAncestryHeadReport>,
    pub verified_transactions: u64,
    pub work: RecoveryWorkUsage,
}

pub enum RecoveryAncestryError {
    Cycle,
    LimitExceeded,
    ClaimMismatch {
        request_index: u64,
        claim_index: u64,
    },
    Verification(CommitError),
}

pub fn TransactionRepository::verify_branch_recovery_ancestries_with_maintenance(
    &self,
    maintenance: &RepositoryMaintenanceGuard,
    requests: &[RecoveryAncestryRequest],
) -> Result<RecoveryAncestryReport, RecoveryAncestryError>;
```

`bounded_object_len` is the only store-layout metadata bridge added by S20-530.
It resolves the existing confined final path, rejects a symlink or non-regular
entry, preserves `STORE_OBJECT_NOT_FOUND`, enforces the existing per-object SCB1
byte ceiling, and returns the exact metadata length. Under exclusive maintenance
and immutable-final ownership, transaction recovery charges that length before
calling the existing verifying `ObjectStore::read`. It never stats
`ObjectStore::object_path` directly or interprets store fan-out layout.

`RecoveryRevisionClaim` has exactly the six facts shown by its constructor and
private fields. `with_claims` carries exactly two ordered generic slots. Ref
recovery assigns slot zero to the branch-origin target and slot one to the
visible-ref head. The traversal head is derived solely from slot one's claim;
there is no second caller-supplied head authority. The public method is
branch-recovery-only, permits at most 4,096 requests, and always applies the
frozen per-pointer and union ceilings. Accepted recovery uses a private
transaction-owned single-head core with the accepted ceilings. Production
callers cannot construct custom limits or raise a frozen ceiling. Private test
helpers may inject smaller limits.

Input order is caller-defined. Ref recovery supplies one claimed request per
visible branch in canonical branch-name order. Duplicate heads are legal.
Empty branch input succeeds with empty head reports, zero verified union
transactions, and zero work.

Ref recovery never calls `list_branches_locked`, `resolve_locked`, or a
transaction verifier while constructing its record preflight. Instead, one
private raw-record preflight consumes the preceding bounded origin/ref
inventory's exact final-path vectors. It charges metadata length before every
content read, charges each visible branch before reading or retaining its ref,
decodes every record once, sorts decoded refs by canonical branch name, rejects
duplicates, and validates exact path/name/digest/origin-ref binding. Remaining
origins become a separately charged, sorted orphan report. The resulting
private `RefRecordPreflight` is bound immutably and, without intervening code,
feeds exact ordered ancestry claims to the single transaction verifier call.
Reversal, clearing, filtering, legacy-helper substitution, or any other
post-processing is outside the contract.

Before any ancestry traversal, the verifier directly verifies slot zero and
then slot one for every request in input order. It compares the six exact claim
facts and returns the first `ClaimMismatch` indices without starting ancestry.
If origin and head target the same transaction, actual verification may be
reused, but both slot comparisons still occur. A directly verified claim target
outside the request head's ancestry consumes work but does not increase that
head's or the union's verified transaction count. Raw branch name, origin/ref
digest, and origin/ref binding checks remain repository-owned and precede this
transaction-owned direct-target phase.

After one head traversal completes, both claimed transaction IDs must occur in
that head's logical ancestry. The first unreachable slot returns the same
indexed `ClaimMismatch`. Reachability is checked only after a complete bounded,
cycle-free traversal, so an earlier cycle or traversal-limit error keeps its
typed precedence.

Each head report counts its complete logical ancestry and logical isolated work
even when union convergence lets later requests reuse verified facts without
repeated reads. Within one request, direct-target work reused by its ancestry is
charged once. Across requests, cached revision work is charged logically to
each request that uses it, so per-pointer exact-limit outcomes do not depend on
branch order. The union count includes each transaction once, and union work
records only actual first-time charged work. The verifier validates the
same-root exclusive guard, does not reacquire maintenance, and acquires
`accepted.lock` exactly once after a ref caller already holds `refs.lock`.
Calling the public verifier while `accepted.lock` is already held is forbidden.
Direct-target cache entries are revision-verified facts only; they are not
ancestry-completed until the traversal has visited their parents and processed
their exit marker.

The private accepted core maps `Cycle` to `TXN_PARENT_SHAPE` and
`LimitExceeded` to `TXN_RESOURCE_LIMIT`. Ref recovery maps `Cycle` to
`BRANCH_ANCESTRY_CYCLE`, `LimitExceeded` to `BRANCH_RESOURCE_LIMIT`, claim slot
zero to `BRANCH_ORIGIN_MISMATCH`, and slot one to `REF_TARGET_MISMATCH`.
`Verification(error)` preserves the exact lower
transaction, codec, policy, root, object, or host error. In particular, a nested
codec `TXN_RESOURCE_LIMIT` remains a verification error and is not reclassified
as a branch-owned budget failure. Branch ancestry store failures are explicitly
wrapped through `CommitError` into `Verification`.

Transaction recovery never calls the public branch verifier. Its private
accepted core acquires `accepted.lock` exactly once for the pointer snapshot and
direct-target verification, or receives an explicit private already-locked
token. It does not reacquire the lock recursively. Exclusive maintenance keeps
the accepted pointer and receipts stable if the private implementation releases
the pointer-snapshot lock before complete ancestry verification.

The existing no-argument transaction and ref recovery methods remain and
acquire their own exclusive maintenance guard.

`CROSS-05` has exactly three fresh-fixture subcases:

1. `caller_held_exclusive_sequence` acquires one exclusive guard through
   `TransactionRepository::acquire_exclusive_maintenance`, runs transaction,
   ref, and GC-witness recovery in that order with the same guard, and retains
   the guard while commit, ref, and GC contenders are started.
2. `transaction_no_arg_wrapper` invokes the exact no-argument transaction
   `recover` wrapper. One direct `#[cfg(test)]` thread-local, root-scoped hook
   named `hold_transaction_recovery_before_exclusive_drop` signals only after
   guarded recovery returns and waits immediately before that wrapper drops its
   exclusive guard.
3. `ref_no_arg_wrapper` applies the same protocol to the exact no-argument
   `recover_refs` wrapper through
   `hold_ref_recovery_before_exclusive_drop`.

The caller-held subcase starts three distinct contender threads through exact
`::std::thread::spawn` calls. Inside each closed contender body, the thread
opens `<canonical-root>/locks/maintenance.lock` through exact standard-library
file APIs. Commit and ref contenders require `File::try_lock_shared` to return
exact `WouldBlock`; the GC contender requires `File::try_lock` to return exact
`WouldBlock`. Each sends its typed
`Cross05Blocked::Commit(bool)`, `Cross05Blocked::Ref(bool)`, or
`Cross05Blocked::Gc(bool)` result through an exact absolute
`::std::sync::mpsc::sync_channel(0)` and proves each send through exact
`is_ok()` assertion. The `OwnerHeld`, `ReleaseOwner`, and completion sends use
the same no-`Debug` assertion form. The commit fixture transfers owned candidate bytes and
copyable parent and principal facts into its contender. That contender builds
the borrowing `CommitInput` locally, so the exact absolute `thread::spawn`
closure remains safe and does not leak test data to obtain a `'static`
lifetime. Each contender immediately invokes the corresponding normal commit,
ref, or GC operation, then sends a typed completion derived from that exact
operation result through a second zero-capacity channel only after the call
returns. There is no unrelated statement or call between the blocked send and
the production operation. The local `CommitInput` construction is the one
required commit pre-operation statement. The transaction-owner crate cannot
depend upward on the ref or GC owner, so each no-argument wrapper subcase
instead runs the recovery wrapper in one owner thread and uses exact absolute
zero-capacity `OwnerHeld` and `ReleaseOwner` channels with the exact hook.

After all three caller-held blocked tokens or a wrapper's `OwnerHeld` token is
received, every role's exact blocked boolean must be true. Wrapper tests prove
the same two shared and one exclusive `maintenance.lock` attempts return exact
`WouldBlock`. The observing thread derives GC-witness absence from
`symlink_metadata(<canonical-root>/locks/gc.lock)` returning exact `NotFound`.
Only then does it explicitly drop the caller-held guard or send
`ReleaseOwner`. Caller-held completion results must show all three actual
operations succeeded. Each wrapper must join successfully and repeat the two
shared and one exclusive probes after release, with every post-release probe
succeeding and being explicitly unlocked. No sleep, deadline, timeout, poll,
yield, spin, or thread-completion query is evidence. The exact ordered evidence fields are
`exclusive_observed`, `commit_blocked`, `ref_blocked`, `gc_blocked`,
`gc_witness_absent_while_blocked`, `commit_resumed`, `ref_resumed`,
`gc_resumed`, and `recovery_completed`.

Static production validation binds the dynamic proof to the real lock paths.
Normal commit and ref mutation acquire shared repository maintenance before
mutation. GC collection acquires exclusive repository maintenance before any
`gc.lock` create, open, read, metadata, write, sync, rename, or remove. Each
no-argument recovery wrapper acquires one exclusive guard, calls its guarded
core with that same guard, invokes only its exact direct `#[cfg(test)]` hold
hook after the guarded call and before guard drop, then returns the guarded
result. The hooks and their installation state are absent from the normal-build
projection.

The normal mutation wrappers have selector-free guarded delegation. Their
private cores keep the guard borrowed for the whole mutation:

```rust
TransactionRepository::commit
  -> acquire_shared_maintenance
  -> commit_with_maintenance(input, &maintenance)

BranchRepository::create_branch
  -> acquire_shared_maintenance
  -> create_branch_with_maintenance(name, origin_transaction_id, &maintenance)

BranchRepository::advance_branch
  -> acquire_shared_maintenance
  -> advance_branch_with_maintenance(
       name,
       expected_head,
       new_head,
       &maintenance,
     )
```

`BranchRepository::acquire_shared_maintenance` and its private exclusive
counterpart delegate to the transaction-owned same-root acquisition and map
only the owner error. They do not acquire a lower ref lock first. The public
transaction and ref no-argument recovery wrappers each use this exact shape:

```rust
let maintenance = self.acquire_exclusive_maintenance()?;
let result = self.recover_with_maintenance(&maintenance);
#[cfg(test)]
hold_transaction_recovery_before_exclusive_drop(&self.root, &maintenance);
result
```

The ref wrapper substitutes `recover_refs_with_maintenance` and
`hold_ref_recovery_before_exclusive_drop` without changing the ordering.

`acquire_exclusive_gc` validates and canonicalizes the root, initializes the
maintenance boundary, acquires exclusive repository maintenance, and only then
delegates to `acquire_exclusive_gc_with_maintenance`. That private core owns the
guard, performs all `gc.lock` access, and moves the same guard into
`ExclusiveGcGuard`. No acquisition-path witness token or operation may precede
the exclusive acquisition.

S20-530 supersedes the earlier witness-first GC acquisition sequence. The lock
order is:

```text
maintenance.lock exclusive
  -> gc.lock durable witness when collecting
  -> refs.lock when recovering or verifying named refs
  -> accepted.lock when verifying transaction targets
```

Collection does not acquire `refs.lock` or `accepted.lock`; the diagram fixes
one global ordering for operations that use the corresponding suffix. A
collector acquires exclusive maintenance before creating and syncing its
witness and keeps maintenance held until after witness removal and lock-
directory sync. Therefore, a recovery owner holding exclusive maintenance can
prove that an exact surviving witness has no live collector behind it.

Transaction cleanup may acquire and release `accepted.lock` before a caller
begins ref cleanup under the same maintenance guard. It must never hold
`accepted.lock` while acquiring `refs.lock`. Cooperating transaction and ref
operations use shared maintenance ownership and therefore cannot interleave
with cleanup or collection.

S20-530 adds no aggregate recovery engine. A full local recovery sequence is:

1. initialize and acquire one exclusive maintenance guard;
2. run transaction-owned object, receipt, and fixed-head recovery with that
   guard;
3. run repository-owned branch/ref recovery with that same guard;
4. recover the exact GC witness, when present, with that same guard;
5. inspect the three component-owned reports or statuses;
6. release the guard.

Recovery does not automatically run collection. A later collect still requires
one complete caller-owned retention snapshot and its own guarded replan.

The GC witness is a durable crash marker, not a separately held operating-
system file lock. Witness-first acquisition is forbidden. No collector may
create or retain the witness without already holding exclusive maintenance.

## 4. Owned-stage and retry rules

Recovery removes only an exact component-owned stage name within that
component's exact directory depth. Prefix-only or suffix-only matches are not
owned. Unknown regular leaf files are preserved. A symlink, non-directory
fan-out component, non-regular leaf entry, uppercase or malformed fan-out, or
other traversal ambiguity fails closed before cleanup of that owned tree.

Object fan-out and receipt fan-out are exactly two levels of two lowercase hex
bytes. Exact owned stage names are:

```text
.sley-store-stage-<object-id-lower-hex-64><pid-lower-hex-8><counter-lower-hex-8>.tmp
.sley-txn-stage-<canonical-decimal-pid>-<lower-hex-token-16>.tmp
.sley-head-stage-<canonical-decimal-pid>-<lower-hex-token-16>.tmp
.sley-branch-stage-<canonical-decimal-pid>-<lower-hex-token-16>.tmp
.sley-ref-stage-<canonical-decimal-pid>-<lower-hex-token-16>.tmp
```

The object-stage process ID is exactly eight lowercase hexadecimal digits and
decodes to `1..=4,294,967,295`. A decimal process ID is greater than zero, has
no leading zero, and fits the same unsigned 32-bit range. Every other hex field
is fixed width and lowercase. An object stage is owned only when its
embedded object ID selects the exact containing `<aa>/<bb>` fan-out. Any
missing byte, extra byte, uppercase hex byte, alternate numeric spelling, or
prefix/suffix lookalike is preserved as an unknown regular leaf file.

Deleting a stage is not complete until its containing directory is synced. A
successful recovery visit syncs every validated leaf directory even when the
current invocation removed zero files. This makes a retry repair the durability
boundary when a prior attempt removed a stage and failed before directory sync.

Reports count only files removed by the current invocation. A second successful
recovery returns zero removals, preserves the same visible state, and emits the
same verified pointer facts. Object recovery events and orphan-origin reports
remain deterministically sorted by their existing keys.

## 5. Closed recovery limits

Recovery applies the following hard ceilings before an owner may return a
successful report:

| Surface | Closed limit | Exact failure |
|---|---:|---|
| object recovery fan-out directories | 65,792 | `STORE_IO` |
| object recovery leaf entries | 524,288 | `STORE_IO` |
| final objects within object recovery | 262,144 | `STORE_IO` |
| object stages removable/reportable | 262,144 | `STORE_IO` |
| receipt recovery fan-out directories | 65,792 | `TXN_RESOURCE_LIMIT` |
| receipt recovery leaf entries | 524,288 | `TXN_RESOURCE_LIMIT` |
| final receipts within receipt recovery | 262,144 | `TXN_RESOURCE_LIMIT` |
| receipt stages removable/reportable | 262,144 | `TXN_RESOURCE_LIMIT` |
| accepted-head directory entries | 4,096 | `TXN_RESOURCE_LIMIT` |
| accepted-head stages removable/reportable | 4,095 | `TXN_RESOURCE_LIMIT` |
| branch-origin recovery fan-out directories | 65,792 | `BRANCH_RESOURCE_LIMIT` |
| branch-origin recovery leaf entries | 131,072 | `BRANCH_RESOURCE_LIMIT` |
| final branch-origin records | 65,536 | `BRANCH_RESOURCE_LIMIT` |
| branch-origin stages removable/reportable | 65,536 | `BRANCH_RESOURCE_LIMIT` |
| branch-origin record bytes read | 1,073,741,824 | `BRANCH_RESOURCE_LIMIT` |
| visible-ref recovery fan-out directories | 65,792 | `BRANCH_RESOURCE_LIMIT` |
| visible-ref recovery leaf entries | 69,632 | `BRANCH_RESOURCE_LIMIT` |
| visible-ref stages removable/reportable | 65,536 | `BRANCH_RESOURCE_LIMIT` |
| visible-ref record bytes read | 268,435,456 | `BRANCH_RESOURCE_LIMIT` |
| immutable orphan origins reported | 65,536 | `BRANCH_RESOURCE_LIMIT` |
| visible branches verified | 4,096 | `BRANCH_RESOURCE_LIMIT` |
| public branch-recovery ancestry requests | 4,096 | `RecoveryAncestryError::LimitExceeded` |
| accepted ancestry transactions | 65,536 | `TXN_RESOURCE_LIMIT` |
| accepted ancestry receipt bytes | 1,073,741,824 | `TXN_RESOURCE_LIMIT` |
| accepted ancestry binding visits | 4,194,304 | `TXN_RESOURCE_LIMIT` |
| accepted ancestry object verifications | 2,097,152 | `TXN_RESOURCE_LIMIT` |
| accepted ancestry object bytes | 1,073,741,824 | `TXN_RESOURCE_LIMIT` |
| ancestry transactions per visible branch | 65,536 | `BRANCH_RESOURCE_LIMIT` |
| ancestry receipt bytes per visible branch | 1,073,741,824 | `BRANCH_RESOURCE_LIMIT` |
| ancestry binding visits per visible branch | 4,194,304 | `BRANCH_RESOURCE_LIMIT` |
| ancestry object verifications per visible branch | 2,097,152 | `BRANCH_RESOURCE_LIMIT` |
| ancestry object bytes per visible branch | 1,073,741,824 | `BRANCH_RESOURCE_LIMIT` |
| unique ancestry transactions across all visible branches | 262,144 | `BRANCH_RESOURCE_LIMIT` |
| ancestry receipt bytes across all visible branches | 4,294,967,296 | `BRANCH_RESOURCE_LIMIT` |
| ancestry binding visits across all visible branches | 16,777,216 | `BRANCH_RESOURCE_LIMIT` |
| ancestry object verifications across all visible branches | 8,388,608 | `BRANCH_RESOURCE_LIMIT` |
| ancestry object bytes across all visible branches | 4,294,967,296 | `BRANCH_RESOURCE_LIMIT` |

Every ceiling is enforced incrementally, not after building an unbounded
inventory or report. Before accepting directory, leaf, final, stage, orphan,
visible-branch, ancestry, work, or report item N, recovery checked-adds that
item to the owning counter and verifies the resulting value against the closed
limit. At limit plus one it stops before retaining the extra entry, reading its
payload, scheduling its mutation, or appending a report item. A directory entry
may be obtained only to classify and count that entry; `read_dir().collect()`
and equivalent collect-then-check plans are forbidden. The public branch
verifier checks request count before allocating per-request state or verifying
the first claim. Every temporary vector, set, stack, cache, plan, and report
owned by recovery remains at or below its corresponding ceiling throughout the
call. Private small-limit tests record peak scanned and retained counts and
prove that neither exceeds the injected ceiling.

The checker owns one immutable `LIMIT_EVENT_SPECS` registry in the exact
30-event order. It binds 37 unique qualified fields through 42 exact field-event
memberships. Its SHA-256 is
`d136d756ff34d0c4a351a2841ce336fbb6a4e4fdaad502da43bd3b03015c4b3d`.
Each event fixes its owner function, trusted measurement source, identity,
atomic checked additions, retained-work additions, post-add guards,
post-guard counter commits, first governed sink, and forbidden bypass tokens.
The six trusted source classes are one-unit admission, checked request-slice
length, regular-file metadata length, bounded object length, branch ancestry
admission, and immutable cached work. The checker rejects a sink or limit-field
access outside its registered event window in the owning function. Five fields
intentionally have two memberships: origin record bytes and the four branch
per-pointer cached-work fields. All other qualified fields have exactly one.

The normal-build control grammar is closed to ASCII code after comments and
string literals are projected away. Before trusting a standard-library method,
the checker inventories module, block, and re-exported `use` authority across
the complete local dependency closure. The only admitted external control
import is the exact `unicode_normalization::UnicodeNormalization` import in
`sley-scb1`; every other external extension-trait path fails closed. Every
normal-build `extern crate` declaration is forbidden. Every function-body
macro is inventoried and must resolve as one exact unqualified built-in macro
from the closed checker set. A governed event owner admits only the
non-diverging `format!`, `matches!`, `vec!`, and `write!` subset. All
normal-build attributes are inventoried outside already frozen `macro_rules!`
definitions and must match the closed inert built-in attribute grammar. The
complete attribute chain of every governed owner and transitive local callable
is SHA-256 bound into its review record, so an external procedural attribute
cannot rewrite reviewed code after source parsing.

Every nontrivial ordered event scope and every dominating success or
fallthrough edge carries an explicit edge-liveness record. A runtime-rooted
edge must trace to a concrete parameter, concrete owner state, an owned usage
accumulator, an exact filesystem observation, a trusted standard method on
one of those roots, or a prior governed limit event. Only an unconditional
loop, `if true`, an unguarded match arm, or a checker-proven literal match
selection may use static liveness, and static liveness is never sufficient for
a dominating exit. Literal, frozen-constant, zero-input helper,
literal-derived, and empty-iteration predicates therefore fail closed. The
separate exact runtime-observation manifest still binds every registered event
to the mapped N and N+1 test that proves the event was reached and its charge
was observed.

S20-530 v8 uses one tightly defined hybrid for the entry-path and
control-ancestry proof layers. The checker first generates the complete
pre-exception inventory for all 30 governed events. Every resolved leaf remains
`STATIC_PASS`. Only a generated unresolved leaf that exactly matches its
ordered, digest-bound ledger record receives
`UNRESOLVED_BY_V8_STATIC_RESOLVER` and
`FINAL_SPECIALIST_REVIEW_REQUIRED`. That disposition is not a static pass,
trust grant, ignored finding, reachability claim, or false-positive ruling.
Missing, additional, duplicated, reordered, newly unresolved, newly resolved,
or source-drifted leaves fail closed. Runtime observations, N and N+1 cases,
public-root evidence, dual-site proofs, complete source scans, and fresh final
Nabu, Ariadne, and Vulcan implementation reviews remain mandatory.

The exact entry partition is 110 complete leaves: 90 static passes and 20
manual-review exceptions across 13 events. The exact control partition is
5,213 complete atoms: 3,547 static passes and 1,666 manual-review exceptions
across 23 events. The checker freezes the complete v8 exception partition
under one ordered fingerprint. Its SHA-256 is
`5b475cc8c4c1f5abbab836d29fc28fe0270fa0eef58b73b6d01a1253f051c816`.
The fingerprint binds the corrected 31-source closure, 14 feature-gated
production occurrences plus the separate test-only reference site, positional
scanner contract, complete manifests, complete inventories, static-pass sets,
exception sets, ledger bytes, and all counts.

The independent witness is
`scripts/reconcile_s20_530_exception_ledgers.py`. It imports no checker module
or resolver helper. From the serialized test-plan manifests, it independently
reconstructs every structural leaf and atom, then verifies exact ordered union,
disjointness, counts, event partitions, canonical record hashes, and top-level
digests. Its path and bytes are part of the contract set, its exact manifest is
bound in the test plan and closeout evidence, and its real-package invocation
is a required Tier 2 command.

Receipt verification derives paths only through the non-creating
`receipt_path_readonly` authority. One exact private
`recovery_receipt_metadata` helper uses `symlink_metadata`, maps `NotFound` to
`RECOVERY_RECEIPT_INCOMPLETE`, rejects a symlink or non-regular entry as
`TXN_IO`, and preserves every other operating-system error through
`CommitError`. The accepted-head verifier propagates those `CommitError` values
directly. The branch verifier maps each receipt path, metadata, receipt-decoding,
and binding-inspection `CommitError` through
`RecoveryAncestryError::Verification`. No `From<CommitError>` widening,
generic-I/O replacement of a missing receipt, or directory-creating recovery
path is permitted.

`LIMIT-03` splits transaction-owned enforcement from repository-owned error
mapping. The `branch_requests` and ten `branch_*` ancestry/work subcases invoke
the private transaction verifier core with only the target ceiling reduced and
prove exact-limit success plus `RecoveryAncestryError::LimitExceeded` at limit
plus one. The repository-owned `visible_branches` subcase separately proves
that the public ref-recovery path maps that typed limit to
`BRANCH_RESOURCE_LIMIT` without a partial ref report. Production ref recovery
calls the public transaction verifier exactly once, so the mapping is common
to every transaction-owned branch ceiling and is not duplicated per counter.

Directory and leaf counts are separate so a legal maximum inventory does not
consume its budget merely because exact fan-out directories exist. The 65,792
directory ceiling is 256 first-level plus 65,536 second-level fan-out
components. Leaf counts include finals, stages, and preserved regular
lookalikes so an attacker cannot evade a ceiling with unknown names. Each
removal count has its own ceiling. Recovery reports use checked conversion to
`u64`. Existing GC inventory, edge, report, and allocation limits remain
unchanged. Tests use limit-injected private helpers to prove exact-limit success
and limit-plus-one failure without creating hundreds of thousands of files.

Origin and ref byte counters charge the metadata length before every actual
record-file read, including repeated reads. The counter must have capacity
before the file is opened for content decoding. This bounds strict decoding
work even when every individually legal record approaches its codec ceiling.

The transaction report includes the number of unique accepted-ancestry
transactions verified through trusted genesis. The ref report includes the
number of unique transactions verified across all visible branch ancestries.
Traversal is deterministic head-first depth-first order over canonical parent
order. A completed convergent node is counted once. Missing or corrupt deep
ancestors preserve their exact transaction error. A repeated active node is
`TXN_PARENT_SHAPE` for fixed-head recovery and `BRANCH_ANCESTRY_CYCLE` for ref
recovery.

An ancestry work unit records exact receipt bytes, binding visits, object
verifications, and object bytes. Receipt bytes charge every receipt-file read
needed by an isolated request, including a repeated parent read. Binding visits
charge every entity-binding entry traversed by inventory verification or a
parent/child relationship comparison. Object verifications charge every bound
object checked. Object bytes use `bounded_object_len` and are charged before the
corresponding verifying read. All additions use checked `u64` arithmetic.

The accepted traversal has one request, so its logical and actual usage are the
same and use the accepted ceilings. Branch recovery keeps one logical isolated
budget per request and one actual union budget in canonical branch-name order.
On first verification, the exact unit must fit both the current request and
union budgets before the read or scan begins. If a later request reuses cached
revision or relationship facts, the recorded unit must still fit and is charged
to that request's logical budget, while union actual usage does not increase.
A failure therefore occurs before an over-limit actual operation or cached-fact
use and before cleanup mutation or a successful report.

## 6. Fixed fault matrix

Every durability row freezes exactly one injection point. `old` and `new` refer
to the visible pointer at the instant the injected error is returned. `retry
sync` means a second faulted cleanup attempt must still reach the same
directory-sync hook after the first attempt already removed the stage.

Rows `COR-01` through `COR-08` and `LIMIT-01` through `LIMIT-03` are evidence
families rather than durability injection points. Their exact subcase keys are
frozen after the table. Every subcase uses a fresh fixture. The first five
families prove the exact positive owned-stage grammar plus negative grammar,
fan-out, final-binding, file-kind, and unknown-leaf preservation. Limit
subcases prove exact-limit success and limit-plus-one owner-code failure with no
mutation of that invocation's owner tree, partial report, or partial success.

| ID | Owner | Injected boundary | Required immediate state | Required recovery/retry result |
|---|---|---|---|---|
| `OBJ-01` | store | before object stage write | no stage or final object | stage and final remain absent; no promotion |
| `OBJ-02` | store | during object stage write | incomplete owned stage; no final object | stage removed; final remains absent |
| `OBJ-03` | store | after durable verified stage, before promotion | complete owned stage; no final object | stage removed; final remains absent |
| `OBJ-04` | store | after final-object link, before first directory sync | complete visible final and owned stage; durability is uncertain | recovery syncs leaf, removes stage, and final re-verifies exactly |
| `OBJ-05` | store | after final-object link and first directory sync, before stage cleanup | complete durable final object and owned stage | stage removed; final re-verifies exactly |
| `OBJ-06` | store | after object stage unlink, before second directory sync | complete final; stage absent; put returns `STORE_IO` | exact put retry redurabilizes final and directory |
| `OBJ-07` | store | recovery-stage unlink, before recovery directory sync | stage absent; recovery returns `STORE_IO` | retry sync occurs with zero removals; final visibility unchanged |
| `OLAY-01` | store | fixed `objects/scb1` layout component create, before parent sync | no final object depends on the component | exact existing-component retry reaches the same sync hook |
| `OLAY-02` | store | first object fan-out create, before parent sync | no stage or final object is created | exact existing-component retry reaches the same sync hook |
| `OLAY-03` | store | second object fan-out create, before parent sync | no stage or final object is created | exact existing-component retry reaches the same sync hook |
| `TOBJ-01` | txn | before the first changed-object store write | accepted old; no new object from this commit | recovery twice leaves old complete head |
| `TOBJ-02` | txn | after a nonempty proper subset of a multi-object manifest is durable | accepted old; unreachable complete subset allowed | recovery twice leaves old complete head; no partial root is visible |
| `TOBJ-03` | txn | after every changed object is durable, before receipt stage | accepted old; unreachable complete objects allowed | recovery twice leaves old complete head |
| `TXN-01` | txn | during receipt-stage write | accepted old; incomplete receipt stage | first recovery removes one receipt stage; second removes zero |
| `TXN-02` | txn | after complete verified receipt stage, before final link | accepted old; complete owned stage only | recovery removes stage; no receipt becomes visible |
| `TXN-03` | txn | after final-receipt link, before receipt-directory sync | accepted old; complete receipt and owned stage may exist | recovery removes stage; exact commit retry must redurabilize receipt |
| `TXN-04` | txn | after first receipt-directory sync, before receipt-stage unlink | accepted old; complete durable receipt and stage | recovery removes stage; old head remains complete |
| `TXN-05` | txn | after receipt-stage unlink, before second receipt-directory sync | accepted old; complete receipt; stage absent | recovery resyncs leaf with zero removals; old head remains complete |
| `TXN-06` | txn | after second receipt-directory sync, before head work | accepted old; complete orphan receipt allowed | recovery twice leaves old complete head |
| `HEAD-01` | txn | before accepted-head stage creation | accepted old | recovery twice leaves old complete head |
| `HEAD-02` | txn | during accepted-head stage write | accepted old; incomplete head stage | recovery removes stage; old head verifies |
| `HEAD-03` | txn | after verified head stage, before rename | accepted old; complete head stage | recovery removes stage; old head verifies |
| `HEAD-04` | txn | after head rename, before head-directory sync | accepted new | recovery redurabilizes cleanup boundary; new head verifies |
| `HEAD-05` | txn | after head-directory sync, before response | accepted new | recovery twice leaves new complete head |
| `GEN-01` | txn | before the first trusted-genesis object store write | accepted absent | recovery twice reports uninitialized with zero ancestry |
| `GEN-02` | txn | after a nonempty proper subset of multi-object genesis is durable | accepted absent; unreachable complete subset allowed | recovery twice reports uninitialized with zero ancestry |
| `GEN-03` | txn | after every genesis object is durable, before receipt stage | accepted absent; unreachable complete objects allowed | recovery twice reports uninitialized with zero ancestry |
| `GEN-04` | txn | during trusted-genesis receipt-stage write | accepted absent; incomplete receipt stage | recovery removes stage and reports uninitialized |
| `GEN-05` | txn | after complete verified genesis receipt stage, before final link | accepted absent; complete owned stage only | recovery removes stage; no receipt becomes visible |
| `GEN-06` | txn | after final genesis-receipt link, before first receipt-directory sync | accepted absent; complete receipt and stage may exist | recovery removes stage and reports uninitialized |
| `GEN-07` | txn | after first genesis receipt-directory sync, before receipt-stage unlink | accepted absent; complete durable receipt and stage | recovery removes stage and reports uninitialized |
| `GEN-08` | txn | after genesis receipt-stage unlink, before second receipt-directory sync | accepted absent; complete receipt; stage absent | recovery resyncs leaf and reports uninitialized |
| `GEN-09` | txn | after second genesis receipt-directory sync, before head work | accepted absent; complete orphan receipt allowed | recovery twice reports uninitialized |
| `GEN-10` | txn | before trusted-genesis head-stage creation | accepted absent | recovery twice reports uninitialized |
| `GEN-11` | txn | during trusted-genesis head-stage write | accepted absent; incomplete head stage | recovery removes stage and reports uninitialized |
| `GEN-12` | txn | after verified genesis head stage, before rename | accepted absent; complete head stage | recovery removes stage and reports uninitialized |
| `GEN-13` | txn | after genesis head rename, before head-directory sync | accepted new trusted genesis | recovery verifies ancestry count one |
| `GEN-14` | txn | after genesis head-directory sync, before response | accepted new trusted genesis | recovery twice verifies ancestry count one |
| `RCV-01` | txn | create `transactions/v1`, before syncing `transactions` | no accepted success depends on the component | exact existing-component retry reaches the same sync hook |
| `RCV-02` | txn | first receipt fan-out create, before parent sync | receipt is not linked; accepted remains old | exact existing-component retry reaches the same sync hook |
| `RCV-03` | txn | second receipt fan-out create, before parent sync | receipt is not linked; accepted remains old | exact existing-component retry reaches the same sync hook |
| `RCV-04` | txn | receipt-stage delete, before leaf sync | stage absent; recovery returns `TXN_IO` | retry sync occurs with zero removals |
| `RCV-05` | txn | head-stage delete, before head-directory sync | stage absent; recovery returns `TXN_IO` | retry sync occurs with zero removals |
| `GUARD-01` | txn | transaction recovery receives a same-root shared guard | cleanup has not begun | `TXN_IO`; all repository bytes remain unchanged |
| `GUARD-02` | txn | transaction recovery receives a wrong-root exclusive guard | cleanup has not begun | `TXN_IO`; both repository roots remain unchanged |
| `RLAY-01` | repo | create `branches/v1`, before syncing `branches` | no visible branch depends on the component | exact existing-component retry reaches the same sync hook |
| `RLAY-02` | repo | create the first branch-origin fan-out component, before parent sync | no origin or ref is linked | exact existing-component retry reaches the same sync hook |
| `RLAY-03` | repo | create the second branch-origin fan-out component, before parent sync | no origin or ref is linked | exact existing-component retry reaches the same sync hook |
| `REF-01` | repo | during immutable-origin stage write | no origin or visible ref | origin stage is removed; no record is promoted |
| `REF-02` | repo | after complete verified origin stage, before final link | no origin or visible ref | origin stage is removed; no record is promoted |
| `REF-03` | repo | after origin-record link, before first directory sync | no ref required; orphan origin allowed | exact create retry re-verifies and redurabilizes origin |
| `REF-04` | repo | after first origin-directory sync, before origin-stage unlink | durable origin and stage; no visible ref | recovery removes stage and reports orphan origin |
| `REF-05` | repo | after origin-stage unlink, before second origin-directory sync | durable origin; stage absent; no visible ref | recovery resyncs leaf and reports orphan origin |
| `REF-06` | repo | during initial visible-ref stage write | durable origin; no complete visible ref | ref stage is removed; orphan origin is reported |
| `REF-07` | repo | after complete verified visible-ref stage, before final link | durable origin; no visible ref | ref stage is removed; orphan origin is reported |
| `REF-08` | repo | after initial visible-ref link, before first directory sync | complete visible ref may exist | exact create retry redurabilizes it before `PRESENT` |
| `REF-09` | repo | after first initial-ref directory sync, before ref-stage unlink | durable visible ref and stage | recovery removes stage; complete branch verifies |
| `REF-10` | repo | after initial-ref stage unlink, before second directory sync | durable visible ref; stage absent | recovery resyncs leaf; complete branch verifies |
| `REF-11` | repo | during advance-ref stage write | branch old; incomplete owned ref stage | recovery removes stage; old branch verifies |
| `REF-12` | repo | after complete verified advance-ref stage, before rename | branch old; owned ref stage exists | recovery removes stage; old branch verifies |
| `REF-13` | repo | after advance-ref rename, before directory sync | branch new | recovery and exact advance retry verify complete new branch |
| `REF-14` | repo | after advance-ref directory sync, before response | branch new | recovery twice leaves complete new branch |
| `REF-15` | repo | branch-origin recovery-stage unlink, before leaf sync | stage absent; recovery returns `REF_IO` | retry sync occurs with zero removals |
| `REF-16` | repo | visible-ref recovery-stage unlink, before leaf sync | stage absent; recovery returns `REF_IO` | retry sync occurs with zero removals |
| `GUARD-03` | repo | ref recovery receives a same-root shared guard | cleanup has not begun | `REF_IO`; origins, refs, and stages remain unchanged |
| `GUARD-04` | repo | ref recovery receives a wrong-root exclusive guard | cleanup has not begun | `REF_IO`; both repository roots remain unchanged |
| `CROSS-01` | txn/repo | complete orphan receipt is durable before head work, and branch advance to it is interrupted before ref rename | accepted old; branch old | both recoveries preserve the two old complete pointers |
| `CROSS-02` | txn/repo | complete orphan receipt is durable before head work, then a branch durably advances to it | accepted old; branch new | both recoveries preserve independent complete pointers; no inferred follow or rollback |
| `CROSS-03` | txn/repo | accepted child is durable, and branch advance to it is interrupted before ref rename | accepted new; branch old | both recoveries are idempotent; both revisions verify |
| `CROSS-04` | txn/repo | accepted child and branch advance to it are both durable | accepted new; branch new | both recoveries are idempotent; ancestry remains complete |
| `CROSS-05` | txn/repo | transaction, ref, and GC-witness recovery share one exclusive maintenance owner | concurrent commit, ref, and GC operations block; `gc.lock` is absent | all three operations resume only after ownership releases; caller-held recovery results were already successful and each no-argument wrapper then returns successfully |
| `ANC-01` | txn | fixed accepted head has multi-revision ancestry | visible head is complete | recovery verifies every ancestor through trusted genesis and reports the unique count |
| `ANC-02` | repo | visible branches have shared and distinct ancestry | every ref target is complete | recovery verifies the deterministic bounded union and counts convergence once |
| `ANC-03` | txn | a deep accepted ancestor is missing | accepted pointer bytes remain unchanged | `RECOVERY_RECEIPT_INCOMPLETE`; no report or repair |
| `ANC-04` | txn | a deep accepted ancestor is corrupt | accepted pointer bytes remain unchanged | exact transaction/codec error; no report or repair |
| `ANC-05` | repo | a deep visible-branch ancestor is missing | ref pointer bytes remain unchanged | `RECOVERY_RECEIPT_INCOMPLETE`; no report or repair |
| `ANC-06` | repo | a deep visible-branch ancestor is corrupt | ref pointer bytes remain unchanged | exact transaction/codec error; no report or repair |
| `ANC-07` | txn | fixed-head ancestry traversal encounters an active repeated node | accepted pointer bytes remain unchanged | `TXN_PARENT_SHAPE`; no report or repair |
| `ANC-08` | repo | visible-branch ancestry traversal encounters an active repeated node | ref pointer bytes remain unchanged | `BRANCH_ANCESTRY_CYCLE`; no report or repair |
| `GCW-01` | repo | after GC-witness create, before any byte write | exclusive maintenance remains held until injected return; empty witness may remain | after lock release, recovery removes it and reports `REMOVED_INCOMPLETE` |
| `GCW-02` | repo | during exact GC-witness byte write | exclusive maintenance remains held until injected return; strict `SLEYGC01` prefix may remain | after lock release, recovery removes it and reports `REMOVED_INCOMPLETE` |
| `GCW-03` | repo | after exact witness write, before witness-file sync | exact witness may remain | exclusive recovery removes, syncs, and reports `REMOVED_EXACT` |
| `GCW-04` | repo | after witness-file sync, before lock-directory sync | exact witness may remain | exclusive recovery removes, syncs, and reports `REMOVED_EXACT` |
| `GCW-05` | repo | after exact witness remove, before lock-directory sync | witness absent; recovery returns `GC_EXCLUSIVE_LOCK_REQUIRED` | retry resyncs the lock directory and reports `ABSENT` |
| `GCW-06` | repo | recovery and collection race for exclusive maintenance | only one owner can create, inspect, or remove the witness | the waiter observes the post-owner state; no live witness is misclassified |
| `GUARD-05` | repo | GC-witness recovery receives a same-root shared guard | witness inspection/removal has not begun | `GC_EXCLUSIVE_LOCK_REQUIRED`; witness remains unchanged |
| `GUARD-06` | repo | GC-witness recovery receives a wrong-root exclusive guard | witness inspection/removal has not begun | `GC_EXCLUSIVE_LOCK_REQUIRED`; both roots remain unchanged |
| `GC-01` | repo | before candidate delete | object remains; partial report names it | retry deletes and syncs it; next retry is idempotent |
| `GC-02` | repo | after candidate delete, before leaf sync | object absent; partial report names it as failed but not durably deleted | retry resyncs validated leaf before success; next retry is idempotent |
| `COR-01` | store | object owned-stage grammar, fan-out, final-binding, file-kind, and unknown-leaf evidence family | one fresh exact-owned or negative object-tree fixture | exact owned stage is removed and synced; lookalikes and unknown regular leaves are preserved; ambiguous tree state returns `STORE_IO` without mutation |
| `COR-02` | txn | receipt owned-stage grammar, fan-out, final-binding, file-kind, and unknown-leaf evidence family | one fresh exact-owned or negative receipt-tree fixture | exact owned stage is removed and synced; lookalikes and unknown regular leaves are preserved; ambiguous tree state returns `TXN_IO` without mutation |
| `COR-03` | txn | accepted-head owned-stage grammar, file-kind, and unknown-leaf evidence family | one fresh exact-owned or negative head-tree fixture | exact owned stage is removed and synced; lookalikes and unknown regular leaves are preserved; ambiguous tree state returns `TXN_IO` without mutation |
| `COR-04` | repo | branch-origin owned-stage grammar, fan-out, final-binding, file-kind, and unknown-leaf evidence family | one fresh exact-owned or negative origin-tree fixture | exact owned stage is removed and synced; lookalikes and unknown regular leaves are preserved; ambiguous tree state returns `REF_IO` without mutation |
| `COR-05` | repo | visible-ref owned-stage grammar, fan-out, final-binding, file-kind, and unknown-leaf evidence family | one fresh exact-owned or negative ref-tree fixture | exact owned stage is removed and synced; lookalikes and unknown regular leaves are preserved; ambiguous tree state returns `REF_IO` without mutation |
| `COR-06` | txn | accepted visible-closure corruption evidence family | pointer bytes remain present | exact owner error; no rewrite or successful report |
| `COR-07` | repo | branch/ref visible-closure corruption evidence family | pointer bytes remain present | exact owner error; no promotion, deletion, rewind, or successful report |
| `COR-08` | repo | GC-witness corruption evidence family | witness remains | `GC_EXCLUSIVE_LOCK_REQUIRED`; recovery never deletes arbitrary corruption |
| `LIMIT-01` | store | object recovery exceeds a closed directory, leaf, final, stage, or report limit | owned tree is not cleaned | `STORE_IO`; no success report |
| `LIMIT-02` | txn | transaction recovery exceeds a closed receipt, head, accepted-ancestry, or accepted-work counter | owner tree and pointers remain unchanged | `TXN_RESOURCE_LIMIT`; no success report |
| `LIMIT-03` | repo | ref recovery exceeds a closed origin, ref, orphan, visible, ancestry, or branch-work counter | owner tree and pointers remain unchanged | `BRANCH_RESOURCE_LIMIT`; no partial success report |

Existing S20-500 layout, hard-link, and ref-recovery regressions may satisfy
rows `RLAY-01` through `RLAY-03`, `REF-03`, `REF-08`, and `REF-16` when their assertions match
this contract. The S20-530 closeout records the exact test-to-row map. No row
may be claimed solely from code inspection.

The exact evidence-family subcase keys are:

| Family | Ordered subcase keys |
|---|---|
| `COR-01` | `owned_stage`, `prefix_suffix_lookalikes`, `object_id_case_width`, `pid_zero_case_width`, `counter_case_width`, `object_id_fanout_mismatch`, `fanout_case_shape`, `final_name_fanout_binding`, `unknown_ascii_regular`, `unknown_non_utf8_regular`, `symlink`, `non_regular` |
| `COR-02` | `owned_stage`, `prefix_suffix_lookalikes`, `pid_decimal_grammar`, `token_case_width`, `fanout_case_shape`, `final_name_fanout_binding`, `unknown_ascii_regular`, `unknown_non_utf8_regular`, `symlink`, `non_regular` |
| `COR-03` | `owned_stage`, `prefix_suffix_lookalikes`, `pid_decimal_grammar`, `token_case_width`, `unknown_ascii_regular`, `unknown_non_utf8_regular`, `symlink`, `non_regular` |
| `COR-04` | `owned_stage`, `prefix_suffix_lookalikes`, `pid_decimal_grammar`, `token_case_width`, `fanout_case_shape`, `final_name_fanout_binding`, `unknown_ascii_regular`, `unknown_non_utf8_regular`, `symlink`, `non_regular` |
| `COR-05` | `owned_stage`, `prefix_suffix_lookalikes`, `pid_decimal_grammar`, `token_case_width`, `fanout_case_shape`, `final_name_fanout_binding`, `unknown_ascii_regular`, `unknown_non_utf8_regular`, `symlink`, `non_regular` |
| `COR-06` | `head_checksum`, `receipt_missing`, `receipt_digest`, `root`, `policy`, `object_missing`, `object_digest`, `manifest_length` |
| `COR-07` | `origin_format`, `origin_digest`, `ref_format`, `ref_digest`, `origin_ref_binding`, `target_binding`, `target_transaction`, `origin_ancestry_binding` |
| `COR-08` | `non_prefix_bytes`, `oversize`, `symlink`, `non_regular` |
| `LIMIT-01` | `object_fanout_directories`, `object_leaf_entries`, `final_objects`, `object_stages` |
| `LIMIT-02` | `receipt_fanout_directories`, `receipt_leaf_entries`, `final_receipts`, `receipt_stages`, `head_entries`, `head_stages`, `accepted_ancestry`, `accepted_receipt_bytes`, `accepted_binding_visits`, `accepted_object_verifications`, `accepted_object_bytes` |
| `LIMIT-03` | `origin_fanout_directories`, `origin_leaf_entries`, `final_origins`, `origin_stages`, `origin_record_bytes`, `ref_fanout_directories`, `ref_leaf_entries`, `ref_stages`, `visible_ref_record_bytes`, `orphan_origins`, `visible_branches`, `branch_requests`, `branch_ancestry_per_pointer`, `branch_receipt_bytes_per_pointer`, `branch_binding_visits_per_pointer`, `branch_object_verifications_per_pointer`, `branch_object_bytes_per_pointer`, `branch_ancestry_union`, `branch_receipt_bytes_union`, `branch_binding_visits_union`, `branch_object_verifications_union`, `branch_object_bytes_union` |

The grouped owned-entry subcases have exact internal coverage:

- `prefix_suffix_lookalikes`: prefix and suffix lookalikes;
- `object_id_case_width`: uppercase, short, and long object-ID fields;
- `pid_zero_case_width`: zero, uppercase, short, and long object-stage PID
  fields;
- `counter_case_width`: uppercase, short, and long object-stage counters;
- `pid_decimal_grammar`: zero, leading-zero, `u32` overflow, and nondigit
  decimal PIDs;
- `token_case_width`: uppercase, short, and long 16-hex tokens;
- `fanout_case_shape`: malformed and uppercase fan-out components;
- `final_name_fanout_binding`: malformed final name, uppercase final name, and
  final-record/fan-out mismatch;
- `symlink` and `non_regular`: fan-out, stage, and final entries for fan-out
  owners, or stage and accepted entries for the head owner.

Every owned-entry subcase records an ordered `coverage_assertions` object. Its
keys are the exact cases above, or the subcase name itself for a single-case
subcase, and each maps to a distinct in-body assertion containing that case
name as a code token. `COR-02/symlink` and `COR-04/symlink` instead bind their
three exact owned symlink paths and regular cleanup canary through the primary
fixture facts of their checker-rendered multifault plan. They do not also claim
a second, mutually exclusive owned-entry body.

The other 48 owned-entry subcases use one exact five-sequence statement plan:
fresh owner and baseline, owner-relative path bindings, fixture setup,
pre-operation facts and the single production recovery call, then exact
post-operation outcomes. Their four plan semantics are derived from concrete
result, report, operation-delta, path-snapshot, and owner-tree assertions.
Shadow locals named `expected_result`, `owned_stage_removed`, `preserved`, or
`no_mutation` are not evidence and are rejected as unreviewed body authority.

Every `non_regular` owned-entry fixture uses the checker-pinned
`plant_non_regular_socket` test helper. The helper proves the destination is
absent unless the case is the explicit COR-03 accepted-head replacement,
binds at a unique system-temporary pathname shorter than 108 bytes, renames
the live socket inode into the exact final fixture path, and returns the live
socket handle. A direct `UnixDatagram::bind` against a final owner path is not
valid evidence.

`COR-07/origin_ancestry_binding` uses an origin and head whose six direct claim
facts are each correct, while the origin transaction is not reachable from the
head. It must return `BRANCH_ORIGIN_MISMATCH` without mutation or a successful
report.

The remaining corruption-family result codes are exact:

| Family/subcase | `expected_result` |
|---|---|
| `COR-06/head_checksum` | `REF_HEAD_CORRUPT` |
| `COR-06/receipt_missing` | `RECOVERY_RECEIPT_INCOMPLETE` |
| `COR-06/receipt_digest` | `SCB_DIGEST_MISMATCH` |
| `COR-06/root` | `SCB_DIGEST_MISMATCH` |
| `COR-06/policy` | `SCB_DIGEST_MISMATCH` |
| `COR-06/object_missing` | `STORE_OBJECT_NOT_FOUND` |
| `COR-06/object_digest` | `SCB_DIGEST_MISMATCH` |
| `COR-06/manifest_length` | `TXN_OBJECT_INVENTORY_MISMATCH` |
| `COR-07/origin_format` | `BRANCH_RECORD_FORMAT_VERSION` |
| `COR-07/origin_digest` | `BRANCH_RECORD_DIGEST_MISMATCH` |
| `COR-07/ref_format` | `REF_FORMAT_VERSION` |
| `COR-07/ref_digest` | `REF_DIGEST_MISMATCH` |
| `COR-07/origin_ref_binding` | `REF_BRANCH_BINDING_MISMATCH` |
| `COR-07/target_binding` | `REF_TARGET_MISMATCH` |
| `COR-07/target_transaction` | `RECOVERY_RECEIPT_INCOMPLETE` |
| `COR-07/origin_ancestry_binding` | `BRANCH_ORIGIN_MISMATCH` |
| every `COR-08` subcase | `GC_EXCLUSIVE_LOCK_REQUIRED` |

The `COR-06` and `COR-07` values in that compact table identify the public
namespace anchor only. Each namespace contains an exact ordered `cases` object
whose leaves preserve every applicable concrete owner error, variant path, and
`Error::source` chain. The eight `COR-06` namespace leaf counts are exactly
`2, 1, 48, 9, 8, 1, 3, 1`, for 73 leaf tests. The eight `COR-07` namespace leaf
counts are exactly `2, 1, 5, 1, 2, 1, 71, 76`, for 159 leaf tests.

The normalized visible-revision authority has exactly 71 transaction cases.
It includes both state-root and policy-root digest variants, only the host
`commit.io` case for `TXN_IO`, and only nested `commit.codec.transaction` for
`TXN_RESOURCE_LIMIT`. Recovery-owned guard and limit failures are not repeated.
`COR-07/target_transaction` wraps all 71 cases as
`branch.transaction.<commit variant>` with `CommitError` prepended to the
source chain. The first 71 leaves of
`COR-07/origin_ancestry_binding` apply the same transformation to the origin
target. Its final five leaves are ref-owned origin and ancestry failures. The
complete ref-owned authority contains 17 cases, including distinct semantic
`REF_IO` and host-I/O `REF_IO` variants.

Visible SCB errors are bound to the nested carrier that can expose them through
production receipt import. Contract, epoch, field-order, map-order, and
duplicate-map corruptions use `commit.codec.state_root.scb`. Boolean, UTF-8,
and noncanonical-float corruptions use `commit.codec.candidate`. The
recovery-unreachable outer-receipt `SCB_LABEL_NOT_NFC` case is absent. Imported
candidate descriptor-unknown and payload-kind corruptions preserve their
selectors but expect the decoder-first `SCB_UNION_INVALID` result.

The checker owns one immutable `CORRUPTION_FIXTURE_SPECS` registry in the exact
232-leaf order. Its SHA-256 is
`f337ac7a3355e614ad2fd513b9bbe50af922de70ded9ffaa2cc8f4f93f58bb2c`.
The registry contains 213 visible-revision plans, two accepted-head-pointer
plans, and 17 ref-owner plans. Every key is exactly
`(row_id, group_id, leaf_id)`, and every value contains this closed data-only
schema:

```text
owner_source
fixture_family
recovery_operation
expected_code
expected_variant
expected_source_chain
target_role
artifact_role
identity_recipe
path_recipe
corrupter_class
probe_class
selector
required_facts
distinct_from_roles
```

No field contains arbitrary Rust text supplied by evidence. Checker-owned
renderers turn the closed identifiers into exact fixture, path, probe, and
assertion windows. `GROUPED_ERROR_SPECS` and grouped recovery-operation
ownership are derived from this registry, rather than maintained as parallel
fixture authority.

The 71 visible-revision plans use exactly these corrupter classes before being
instantiated for accepted, branch-head, and branch-origin roles: 12 receipt
envelope, 13 nested transaction, 14 nested candidate, eight nested candidate
result, nine nested state-root, eight nested policy-root, one semantic manifest,
one absent receipt, one path-bound receipt host-I/O, two object-byte, one absent
object, and one path-bound object host-I/O plan. The branch-head identity is
decoded from the visible ref record. The branch-origin identity is decoded from
the origin record. Their transaction IDs and receipt paths are proven unequal,
and each selected fault path is proven equal to its role path and unequal to the
other role path. Object plans additionally use role-distinct changed objects and
prove unequal object IDs and paths. Replacing an origin plan with its paired
head plan is a mandatory negative control.

The 17 ref-owner plans use class-specific facts. Single-record cases bind one
exact origin or ref path derived from the selected branch name. Name-collision
evidence separately binds the encoded branch name and path branch name and
proves they differ. Pair cases bind both origin and ref paths, both file kinds,
and both bytes or the exact selected absence. Target-mismatch evidence binds
the decoded ref target to the independently verified head receipt. The
origin-mismatch topology proves all six claim facts independently for both the
origin and head, then proves that the otherwise valid origin is not reachable
from the head.

Logical cycle and resource-limit plans have no singular filesystem artifact
fact. A cycle uses an exact two-node `L -> R -> L` graph, exact ordered request
claims, an owner-root-bound plan installation, and the same private production
traversal core because a cryptographically valid content-addressed receipt
cycle cannot be constructed on disk. A logical resource-limit plan proves
exact N and N+1 chains, lowers only the selected target ceiling, and records
that scanned and retained peaks remain at or below N. The repository test
enables the frozen transaction test-support feature because dependency crates
do not receive ordinary `cfg(test)`. Semantic-I/O evidence binds a non-regular
entry at the exact canonical inventory path. Host-I/O evidence instead binds a
regular pristine artifact and a path-exact `ErrorKind::Other` injector; a
symlink or directory cannot stand in for the host-I/O source chain.

The cross-crate cycle seam has one empty transaction feature and one repository
dev-only activation. The normal dependency remains feature-free:

```toml
# crates/sley-txn/Cargo.toml
[features]
s20-530-test-hooks = []

# crates/sley-repo/Cargo.toml
[dev-dependencies]
sley-txn = { path = "../sley-txn", features = ["s20-530-test-hooks"] }
```

The transaction module declaration, two transaction owner guards, two
transaction activation seams, and one transaction parent-substitution seam use
exactly `#[cfg(any(test, feature = "s20-530-test-hooks"))]`. The repository
ref-owner guard uses exactly `#[cfg(test)]` because Cargo features are
crate-local and the mapped ref evidence is an in-crate unit test. These are the
only seven gated hook sites. The hook is a root-bound, single-active,
thread-local `L -> R -> L` parent-vector substitution. It runs only after the
durable revision is verified or recovered from the verified cache and before
parent expansion. It cannot fabricate errors, claims, IDs, counters, limits,
work, or probe results. Installation rejects nesting, and the plan clears on
observation consumption or unwind. No other manifest may enable the feature.
The checker audits the raw path, owner, gate, and normalized bytes for all seven
sites before it removes only those exact ranges from its normal-build
projection.

Every grouped leaf adds `fixture_plan_sha256` and an ordered
`fixture_assertions` object. The plan digest is checker-derived from the frozen
registry. Its assertions directly prove target identity, artifact path, file
kind, bytes or absence, selector effect, direct probe variant and code, and an
untargeted control before the production recovery call. Helpers return observed
paths, IDs, bytes, and probe outcomes only. They do not return expected booleans,
codes, roles, or usage. Direct filesystem mutation, fault-controller mutation,
or a second fixture root outside the checker-owned fixture window is forbidden.

Each grouped namespace object has exactly `result` and `cases`. Every leaf is a
normal fresh-fixture error-evidence object with its own unique mapped test.
Omission, reordering, anchor-only evidence, equal-code variant substitution,
target/origin reuse, source-chain truncation, duplicate tests, and comment or
string-only assertions are all negative controls.

The checker owns one immutable `RECOVERY_PROVENANCE_SPECS` registry in the
exact 10-case order. Its SHA-256 is
`9d40d476b8838760fc81d69531e4aa10339f51465ffa646c50a10fbf2e715d14`.
It covers both `GUARD-01` and `GUARD-02` operation variants plus `GUARD-03`,
`GUARD-04`, `ANC-03`, `ANC-05`, `ANC-07`, and `ANC-08`. Each record fixes the
owner source, fixture family, guard relation, pointer role, topology recipe,
fault role, ordered canary roles, snapshot roots, and ordered required facts.
The operation and expected error code, variant, and source chain remain derived
from the existing closed error registries.

Same-root guard fixtures bind one real canonical owner root, acquire a shared
guard from that root, prove it covers the owner, and independently prove the
missing direct-head receipt would lose as `RECOVERY_RECEIPT_INCOMPLETE`.
Wrong-root fixtures create two fresh real roots, prove their canonical forms
differ, acquire an exclusive guard only from the guard root, and snapshot both
trees. Transaction and ref cleanup canaries are materialized in root/canary
order and directly prove path derivation, regular kind, exact bytes, and
preservation. Verifier requests are nonempty and contain distinct genesis and
head claims derived from the owner fixture.

Deep-missing fixtures construct durable `H -> P -> D -> G`, derive each parent
from the preceding verified receipt, remove only depth-two `D`, and prove the
genesis, direct-parent, and head receipts remain regular. Cycle fixtures first
prove durable `L -> R -> G`, then install one owner-root-bound logical
`L -> R -> L` parent override after revision verification and before parent
expansion. Observations must record both durable and logical edges, consume the
plan exactly once per node, and bind the resulting cycle to the shared private
production traversal core. Evidence for every covered case appends its
checker-derived `fixture_plan_sha256` and exact ordered `fixture_assertions`
after `semantic_assertions`.

The closeout JSON `matrix_test_map` preserves exact matrix order. Every
durability row maps to a `PASS` object with `fresh_fixture=true`, exactly one
Rust function name in `tests`, and a nonempty `assertions` array containing
exact assertion-macro snippets from that function's brace-delimited body. A
mapped function must have exactly one function attribute, plain `#[test]`.
Ignored, conditionally compiled, `should_panic`, or otherwise attributed
functions do not count. Commented test attributes, comment-only text, strings
containing assertion text, helper bodies after the mapped function, and source
inspection without an executed test do not count.

The 62 non-GC durability rows use this exact retry-protocol partition. `H`
means one invocation of the row's test-only fault helper, `R` means one normal
component recovery invocation, and `O` means one normal non-injected owner
operation retry. `H2` uses the same row selector and the same selector payload
twice against the same fresh fixture. No other row invokes its fault helper
twice.

| Protocol | Exact rows |
|---|---|
| `H1_R1` | `OBJ-01` through `OBJ-05`; `TXN-02`, `TXN-04`, `TXN-05`; `HEAD-02` through `HEAD-04`; `GEN-04` through `GEN-08`, `GEN-11` through `GEN-13`; `REF-01`, `REF-02`, `REF-04` through `REF-07`, `REF-09` through `REF-12` |
| `H1_R2` | `TOBJ-01` through `TOBJ-03`; `TXN-01`, `TXN-06`; `HEAD-01`, `HEAD-05`; `GEN-01` through `GEN-03`, `GEN-09`, `GEN-10`, `GEN-14`; `REF-14` |
| `H1_O1` | `OBJ-06`, `REF-03`, `REF-08` |
| `H1_R1_O1` | `TXN-03`, `REF-13` |
| `H2_O1` | `OLAY-01` through `OLAY-03`; `RCV-01` through `RCV-03`; `RLAY-01` through `RLAY-03` |
| `H2_R1` | `OBJ-07`, `RCV-04`, `RCV-05`, `REF-15`, `REF-16` |

The injected results are exact. `OBJ-*` and `OLAY-*` return `STORE_IO`.
`TOBJ-*`, `TXN-02` through `TXN-06`, `GEN-01` through `GEN-03`, `GEN-05`
through `GEN-09`, and every `RCV-*` row return `TXN_IO`. `TXN-01` and
`GEN-04` return `RECOVERY_RECEIPT_INCOMPLETE`. Every `HEAD-*` row and
`GEN-10` through `GEN-14` return `RECOVERY_REF_CAS_INCOMPLETE`. Every
`RLAY-*` and `REF-*` row returns `REF_IO`.

Each of these 62 row objects has exactly these ordered fields:

```text
result
fresh_fixture
protocol
expected_result
tests
assertions
operation_bindings
result_bindings
result_assertions
tree_assertions
path_assertions
report_assertions
```

`operation_bindings` names every fault-helper, recovery, and ordinary retry
result in protocol order. Each injected error is extracted directly from its
bound helper result and projected through the component's real `code()` or
`symbol()` method. Every call uses the exact owner receiver whose root is
snapshotted. Repeated fault calls use identical non-selector arguments, and an
ordinary retry uses the same arguments as its injected call. Expected helper,
recovery, and retry method counts are exhaustive within the mapped test.
Recovery and ordinary retry assertions directly compare a field, method, or
indexed value projected from their bound result; a local success boolean is not
report evidence. Every phase snapshots the exact owner tree plus two direct
owner-root join-chain witness paths. Paths that may be absent use the frozen
`exact_optional_path_snapshot` helper, which distinguishes `NotFound` from
every other metadata error. `tree_assertions` and `path_assertions` cover every
protocol phase and reject reflexive comparisons. The implementation review is
therefore responsible for the row-specific interpretation of the reviewed
snapshot comparisons, while the checker binds those comparisons to the exact
operations and filesystem witnesses.

From the three before-fault snapshots through every bound operation, injected
error extraction, exact result assertion, and the following three snapshots,
the protocol is one contiguous top-level statement skeleton. No unbound retry,
recovery, repair, or filesystem mutation may run before a phase is observed.

Every semantic evidence field is linked through a `semantic_assertions` object
whose exact field keys map to assertion snippets already present in that
subcase's `assertions` array and in the mapped function body. Each mapping uses
a distinct assertion containing that exact semantic field name as a code token.
A generic, reused, or unrelated assertion cannot support `no_mutation`, `expected_result`,
`owned_stage_removed`, `preserved`, `exact_limit_success`,
`limit_plus_one_code`, or `no_partial_report`.

Rows `CROSS-01` through `CROSS-04`, `ANC-01`, and `ANC-02` do not use generic
success booleans. Their ordered evidence fields are `result`, `fresh_fixture`,
`tests`, `assertions`, `operation_bindings`, `report_assertions`,
`pointer_assertions`, and `closure_assertions`. Every operation is bound to its
actual report, every pointer comparison uses the frozen exact path snapshot,
and every expected transaction identity comes from the fresh fixture rather
than from the observed result.

From the exclusive maintenance binding through both ordered rounds, guard
assertions, before snapshots, recovery calls, after snapshots, guard release,
and post-release reads form one contiguous top-level statement skeleton.
Unbound recovery, repair, filesystem mutation, or other code cannot run inside
that observation window.

The four cross-component fixtures contain trusted genesis `old`, its direct
complete child `new`, exactly one visible branch, and no orphan origin. One
exclusive same-root guard executes transaction recovery, ref recovery,
transaction retry, and ref retry in that exact order. Both pointer files are
snapshotted before and after each round. Reports and independently loaded
revisions satisfy this exact table:

| Row | Accepted pointer | Branch pointer | Transaction ancestry | Ref ancestry union | First removed ref stages |
|---|---|---|---:|---:|---:|
| `CROSS-01` | `old` | `old` | 1 | 1 | 1 |
| `CROSS-02` | `old` | `new` | 1 | 2 | 0 |
| `CROSS-03` | `new` | `old` | 2 | 1 | 1 |
| `CROSS-04` | `new` | `new` | 2 | 2 | 0 |

`ANC-01` uses a four-transaction linear accepted chain through trusted genesis.
Two transaction recoveries each report ancestry count four, zero removals, and
the unchanged accepted pointer. `ANC-02` uses
`genesis -> shared -> {alpha, beta}` with two distinct visible branch heads.
Two ref recoveries each report two visible branches, ancestry union four, zero
removals, no orphans, and unchanged alpha and beta ref bytes. Ref recovery
builds one request per visible branch in canonical branch-name order, invokes
the transaction ancestry verifier exactly once, and copies its unique union
count into `RefRecoveryReport::verified_ancestry_transactions`.

Every family row maps its exact ordered subcase keys to objects with the same
test fields. For `COR-01` through `COR-05`, `owned_stage` records
`expected_result=PASS_REMOVED_AND_SYNCED`, `owned_stage_removed=true`,
`preserved=false`, and `no_mutation=false`. Each negative grammar or unknown
regular-leaf subcase records `expected_result=PASS_PRESERVED`,
`owned_stage_removed=false`, `preserved=true`, and `no_mutation=true`. Each
`fanout_case_shape`, `final_name_fanout_binding`, `symlink`, or `non_regular`
subcase instead records the exact owner error in `expected_result`, with the
same preservation and no-mutation fields. The exact owner errors are
`STORE_IO` for `COR-01`, `TXN_IO` for `COR-02` and `COR-03`, and `REF_IO` for
`COR-04` and `COR-05`.

Fatal transaction owned-entry cases bind `TXN_IO` through
`CommitError::Transaction`. Fatal ref owned-entry cases bind `REF_IO` through
`BranchError::Branch`. These narrow variants do not change independent host
I/O cases that legitimately use `CommitError::Io` or `BranchError::Io`.
The classified owned-entry variants have an exact empty `Error::source` chain;
they do not retain an underlying host `io::Error` object.
Every fatal case proves its regular cleanup canary's exact before and after
snapshot, kind, expected kind, and owner tree. COR-04 and COR-05 establish the
canonical refs directories and `locks/refs.lock` before the fresh baseline.
COR-03 symlink and non-regular cases record trusted-genesis `heads/accepted`
as a changed fixture path after explicit replacement, not as an added path.
The legacy `cleanup_canary_hashes` keys ending in `before_sha256` and
`after_sha256` are deterministic equality-proof bindings over the row,
subcase, optional grouped leaf, subject, and exact mapped assertion. They are
not claimed as hashes of runtime snapshot bytes. The checker recomputes each
binding exactly; the executed Rust assertion proves the actual before/after
snapshot equality.

Every `COR-06` through `COR-08` and limit-family subcase records
`no_mutation=true`. Each `COR-06` through `COR-08` subcase also records the
exact table-bound `expected_result`, and that field's mapped assertion contains
the exact returned symbolic code. Each limit subcase records
`exact_limit_success=true`, the exact `limit_plus_one_code`, and
`no_partial_report=true`.

For an evidence-family invocation, `no_mutation=true` means a byte-for-byte
snapshot of the exact owner tree is unchanged by the preserving or failing
call. A composite caller still invokes owners sequentially, so this flag does
not claim that a
later owner failure rolls back cleanup already completed by an earlier owner.

The structural checker freezes a SHA-256 over all five cells of every matrix
row, not only the IDs. It requires each mapped name to be one exact plain
`#[test] fn` in the row's owning source file, rejects any other function attribute,
rejects reuse of a test function across rows or subcases, and verifies every
recorded assertion macro inside that exact function body. Generic semantic and
coverage field names must appear as Rust code tokens, not only in a comment or
string. COR-01 through COR-05 use the purpose-built concrete owned-entry rules
above instead of shadow semantic identifiers.
The checker removes only syntax items gated by exact `#[cfg(test)]` when
scanning production Rust; a test-only item never truncates later production
source.

The exact map contains 419 unique Rust tests after grouped error leaves are
expanded. The test plan also records, in mapped-test order, each source path,
Cargo-qualified test name, and raw brace-delimited body SHA-256. Its canonical
`mapped_test_bodies_sha256` is copied into closeout evidence and is part of the
review-free implementation payload. Implementation verdicts therefore bind to
the reviewed bodies as well as the complete workspace source set and validated
commit.

Closeout evidence binds execution to the complete reviewed workspace-input
closure, not a four-file sample. The canonical closure includes every tracked
or nonignored untracked workspace file, including crate manifests,
`Cargo.lock`, production modules, tests, scripts, specifications, and frozen
evidence. Only the machine summary, the closeout JSON, and the exact twenty-seven
runner-named S20-530 command logs are validation outputs and excluded. Any
other file under the log directory remains an input. Existing excluded output
paths must still be regular Git/workspace entries. The checker opens the
machine summary and closeout JSON through owner-bound, bounded,
descriptor-relative `O_NOFOLLOW` traversal with stable pre/post metadata;
parent or final symlinks, non-regular files, executable/world-writable modes,
wrong uid/gid, and oversized files fail closed. Every JSON authority rejects
duplicate keys at every nesting level and all non-finite numeric spellings
before canonical hashing. The runner refuses any other dirty input and every
workspace-input symlink, then records the ordered regular-file
path-to-mode-and-SHA-256 map, its canonical
digest, the validated Git commit and tree, the exact ordered mapped test list,
the owning crate, the exact Cargo-qualified name for each mapped function, and
the reviewed mapped-body manifest digest.

The runner executes an unfiltered Cargo test listing for each owning crate and
stores the captured output in a deterministic log. Every mapped function must
occur under its exact Cargo-qualified name in its owning crate's actual test
list. It then executes every Tier
2 command from Section 10, captures combined output, and records log SHA-256,
exit status zero, an empty skipped-check list, duration, environment, validated
commit, source-set digest, and mapped-test-list digest. The checker rehashes
every input and output log and rejects missing, stale, reordered, skipped,
nonzero, or self-described but unbound evidence.

This is a workspace-source closure under an exact recorded host trust boundary,
not a claim of machine-wide hermetic execution. The authoritative closeout
invocation starts from an independently sanitized process environment:

```text
env -i LC_ALL=C PATH=/usr/bin:/bin /usr/bin/python3 -I -B scripts/run_s20_530_validation.py
```

The trusted pre-Python bootstrap consists of the OS dynamic loader, the initial
`/usr/bin/python3 -I -B` entry, and the checker and runner bytes at process
start. The authoritative runner verifies the frozen checker, runner,
specification, and ADR hashes before it runs nested contract or hostile
self-tests. A direct `--self-test` invocation is diagnostic and
non-authoritative. A caller-controlled environment that can act before Python
startup is likewise non-authoritative.

The recorded host TCB consists of the Linux kernel, procfs, system libraries,
recorded executables, their resolved path components and every actor able to
rewrite those components, the Rust dependency-acquisition surface, and the
host toolchain. Recorded executable files are regular, non-group-writable,
non-world-writable, and rebound by owner, group, mode, and SHA-256 around use.
Git runs only after fd-bound verification of the exact safe local config and
comment-only info/exclude bytes, with local includes, attributes, grafts,
alternates, modules, replace metadata, and shallow state rejected. That local
authority is rebound before and after every Git command and included in the
execution-profile digest.

The trust boundary also excludes a cooperating command that fabricates and
then restores evidence or authority, and any unrelated actor with the
validation uid or validation gid that mutates and restores repository,
validation-output, or executable-path authority between checks. Validation
outputs are reached one component at a time through descriptor-relative
`O_NOFOLLOW` directory opens. Output reads require a bounded regular file with
the recorded uid and gid and stable pre/post descriptor metadata. The Linux
child-subreaper and bounded-capture controls terminate and reap descendant
processes on normal exit, timeout, or output overflow. The runner pins the unreaped leader immediately with a pidfd. It selects descendants from exact
`(parent_pid, start_time)` procfs identities, revalidates each identity after
opening its pidfd, and signals only through pidfds. Cleanup authority reserves
file descriptors before spawn, retains enough capacity after real `EMFILE` or
`ENFILE`, processes saturated descendant sets in bounded batches, polls and
reaps every pinned batch, and requires three separated fresh empty-tree scans
before cleanup succeeds.

Freeze reviews use exact `PASS_CONTRACT_FREEZE` verdicts bound to the frozen
contract-set digest and the review-free freeze-evidence payload digest.
Implementation reviews use exact `PASS_IMPLEMENTATION` verdicts bound to that
contract digest, the closeout source-set digest, validated commit, and the
review-free closeout-evidence payload digest. A verdict from one phase, source
set, commit, or evidence payload cannot satisfy another.

## 7. Fault and result vocabulary

Test-only injection selectors are closed enums local to the owning crate. Tests
must inject one boundary per fresh fixture. An injected error returns the
existing owner code:

The exact selector authority is 70 rows. Every enum and private helper below is
one direct `#[cfg(test)]` item. Any selector-bearing parameter or hook statement
inside an otherwise normal-build item is itself directly gated by exact
`#[cfg(test)]`. The normal-build projection contains none of these enum,
variant, helper, parameter, or hook identifiers. Normal production calls have
selector-free signatures.

There is no `None` or `Disabled` variant and no environment-variable,
feature-gated, repository-field, global, raw-path, boolean, closure, arbitrary
byte-count, object-count, or candidate-index selector. Every mapped durability
row constructs exactly its one ordered variant and passes it to exactly its one
mapped helper. No test or helper is reused for another row.

`sley-store/src/lib.rs` owns this exact enum:

```rust
enum StoreDurabilityCut {
    Obj01BeforeObjectStageWrite,
    Obj02DuringObjectStageWrite,
    Obj03VerifiedObjectStageBeforePromotion,
    Obj04FinalObjectLinkBeforeFirstLeafSync,
    Obj05FirstLeafSyncBeforeObjectStageUnlink,
    Obj06ObjectStageUnlinkBeforeSecondLeafSync,
    Obj07RecoveryObjectStageUnlinkBeforeLeafSync {
        object_id: ObjectId,
    },
    Olay01Scb1DirectoryCreateBeforeObjectsSync,
    Olay02FirstObjectFanoutCreateBeforeParentSync,
    Olay03SecondObjectFanoutCreateBeforeParentSync,
}
```

Its exact private helpers are:

- `ObjectStore::put_with_store_durability_cut` for `OBJ-01` through `OBJ-06`
  and `OLAY-01` through `OLAY-03`;
- `ObjectStore::recover_staged_with_store_durability_cut` for `OBJ-07`.

`sley-txn/src/repository.rs` owns this exact enum:

```rust
enum TransactionDurabilityCut {
    Tobj01BeforeFirstChangedObjectPut,
    Tobj02AfterFirstChangedObjectDurable,
    Tobj03AfterAllChangedObjectsBeforeReceiptStage,
    Txn01DuringReceiptStageWrite,
    Txn02VerifiedReceiptStageBeforeFinalLink,
    Txn03FinalReceiptLinkBeforeFirstLeafSync,
    Txn04FirstReceiptLeafSyncBeforeStageUnlink,
    Txn05ReceiptStageUnlinkBeforeSecondLeafSync,
    Txn06SecondReceiptLeafSyncBeforeHeadWork,
    Head01BeforeAcceptedHeadStageCreate,
    Head02DuringAcceptedHeadStageWrite,
    Head03VerifiedAcceptedHeadStageBeforeRename,
    Head04AcceptedHeadRenameBeforeHeadSync,
    Head05AcceptedHeadSyncBeforeResponse,
    Gen01BeforeFirstGenesisObjectPut,
    Gen02AfterFirstGenesisObjectDurable,
    Gen03AfterAllGenesisObjectsBeforeReceiptStage,
    Gen04DuringGenesisReceiptStageWrite,
    Gen05VerifiedGenesisReceiptStageBeforeFinalLink,
    Gen06FinalGenesisReceiptLinkBeforeFirstLeafSync,
    Gen07FirstGenesisReceiptLeafSyncBeforeStageUnlink,
    Gen08GenesisReceiptStageUnlinkBeforeSecondLeafSync,
    Gen09SecondGenesisReceiptLeafSyncBeforeHeadWork,
    Gen10BeforeGenesisHeadStageCreate,
    Gen11DuringGenesisHeadStageWrite,
    Gen12VerifiedGenesisHeadStageBeforeRename,
    Gen13GenesisHeadRenameBeforeHeadSync,
    Gen14GenesisHeadSyncBeforeResponse,
    Rcv01TransactionsV1CreateBeforeTransactionsSync,
    Rcv02FirstReceiptFanoutCreateBeforeParentSync,
    Rcv03SecondReceiptFanoutCreateBeforeParentSync,
    Rcv04ReceiptRecoveryStageUnlinkBeforeLeafSync {
        transaction_id: TransactionId,
    },
    Rcv05HeadRecoveryStageUnlinkBeforeHeadSync,
}
```

Its exact private helpers are:

- `TransactionRepository::ensure_layout_with_transaction_durability_cut` for
  `RCV-01`;
- `TransactionRepository::commit_with_transaction_durability_cut` for
  `TOBJ-01` through `TOBJ-03`, `TXN-01` through `TXN-06`, `HEAD-01` through
  `HEAD-05`, and `RCV-02` through `RCV-03`;
- `TransactionRepository::initialize_trusted_genesis_with_transaction_durability_cut`
  for `GEN-01` through `GEN-14`;
- `TransactionRepository::recover_with_transaction_durability_cut` for
  `RCV-04` through `RCV-05`.

`sley-repo/src/refs.rs` owns this exact enum:

```rust
enum NativeRefDurabilityCut {
    Rlay01BranchesV1CreateBeforeBranchesSync,
    Rlay02FirstOriginFanoutCreateBeforeParentSync,
    Rlay03SecondOriginFanoutCreateBeforeParentSync,
    Ref01DuringOriginStageWrite,
    Ref02VerifiedOriginStageBeforeFinalLink,
    Ref03OriginLinkBeforeFirstLeafSync,
    Ref04FirstOriginLeafSyncBeforeStageUnlink,
    Ref05OriginStageUnlinkBeforeSecondLeafSync,
    Ref06DuringInitialRefStageWrite,
    Ref07VerifiedInitialRefStageBeforeFinalLink,
    Ref08InitialRefLinkBeforeFirstLeafSync,
    Ref09FirstInitialRefLeafSyncBeforeStageUnlink,
    Ref10InitialRefStageUnlinkBeforeSecondLeafSync,
    Ref11DuringAdvanceRefStageWrite,
    Ref12VerifiedAdvanceRefStageBeforeRename,
    Ref13AdvanceRefRenameBeforeLeafSync,
    Ref14AdvanceRefLeafSyncBeforeResponse,
    Ref15OriginRecoveryStageUnlinkBeforeLeafSync {
        branch_name: BranchName,
    },
    Ref16VisibleRefRecoveryStageUnlinkBeforeLeafSync {
        branch_name: BranchName,
    },
}
```

Its exact private helpers are:

- `BranchRepository::ensure_layout_with_native_ref_durability_cut` for
  `RLAY-01`;
- `BranchRepository::create_branch_with_native_ref_durability_cut` for
  `RLAY-02` through `RLAY-03` and `REF-01` through `REF-10`;
- `BranchRepository::advance_branch_with_native_ref_durability_cut` for
  `REF-11` through `REF-14`;
- `BranchRepository::recover_refs_with_native_ref_durability_cut` for
  `REF-15` through `REF-16`.

`sley-repo/src/gc.rs` owns this exact enum:

```rust
enum GcDurabilityCut {
    Gcw01WitnessCreateBeforeWrite,
    Gcw02DuringWitnessWrite,
    Gcw03WitnessWriteBeforeFileSync,
    Gcw04WitnessFileSyncBeforeLockDirectorySync,
    Gcw05WitnessRemoveBeforeLockDirectorySync,
    Gcw06CollectionOwnsMaintenanceBeforeWitnessAccess {
        gate: Arc<GcMaintenanceRaceGate>,
    },
    Gc01BeforeSecondCandidateDelete,
    Gc02SecondCandidateUnlinkedBeforeLeafSync,
}
```

Its exact private helpers are:

- `acquire_exclusive_gc_with_gc_durability_cut` for `GCW-01` through
  `GCW-04` and `GCW-06`;
- `recover_gc_witness_with_gc_durability_cut` for `GCW-05`;
- `gc_collect_with_gc_durability_cut` for `GC-01` through `GC-02`.

Every `during write` cut writes exactly `floor(total_len / 2)` durable-prefix
bytes, and its fixture proves `0 < prefix < total_len`. `TOBJ-02` and `GEN-02`
fire only after the first canonical manifest object returns durable, and each
fixture contains at least two objects. `GC-01` and `GC-02` target canonical
deletion-candidate index 1 in the required three-candidate fixture.

Only `OBJ-07`, `RCV-04`, `REF-15`, `REF-16`, and `GCW-06` carry payloads. The
first four payloads identify the exact leaf retried after its stage disappears.
The `GCW-06` gate separately signals that exclusive maintenance is held and
waits for release before any witness access; its recovery waiter invokes the
normal production API.

| Surface | Existing result |
|---|---|
| object write, promote, or object recovery host failure | `STORE_IO` |
| receipt or transaction layout host failure | `TXN_IO` |
| incomplete receipt stage write | `RECOVERY_RECEIPT_INCOMPLETE` |
| fixed-head stage, rename, or response-boundary injection | `RECOVERY_REF_CAS_INCOMPLETE` |
| native ref host or recovery failure | `REF_IO` |
| GC witness recovery failure | `GC_EXCLUSIVE_LOCK_REQUIRED` |
| GC delete or directory-sync failure | `GC_DELETE_IO` with partial `GcReport` |

Exact lower-layer codec, object, receipt, root, policy, and ref corruption codes
remain unchanged. S20-530 introduces no catch-all recovery code and does not
reinterpret corruption as an interrupted write.

## 8. Recovery report contract

`sley-txn::RecoveryReport` remains the transaction-owned report and contains:

- object stages removed by this invocation;
- receipt stages removed by this invocation;
- accepted-head stages removed by this invocation;
- the verified accepted transaction ID, or `None` when no accepted head is
  present;
- the number of unique accepted-ancestry transactions verified through trusted
  genesis.

The added public field is exactly:

```rust
pub verified_ancestry_transactions: u64
```

When `accepted_transaction_id` is `None`, this count is zero. S20-530 adds no
initialization marker, so recovery treats an absent head as uninitialized and
does not guess whether a previously initialized head was removed outside the
cooperating crash model. The separate `accepted_head` read API continues to
return `REF_HEAD_MISSING` when a caller requires initialized state.

`sley-repo::RefRecoveryReport` remains the ref-owned report and contains:

- branch-origin stages removed by this invocation;
- visible-ref stages removed by this invocation;
- the count of fully verified visible branches;
- deterministically sorted immutable orphan origins;
- the number of unique transactions verified across all visible branch
  ancestries.

The added public field is also exactly:

```rust
pub verified_ancestry_transactions: u64
```

Its owner type keeps the two reports unambiguous.

GC-witness recovery returns one closed component-owned status:

- `REMOVED_EXACT` only after a regular witness with exact `SLEYGC01` bytes was
  removed and the lock directory synced;
- `REMOVED_INCOMPLETE` only after an empty file or strict byte prefix of
  `SLEYGC01` was removed and the lock directory synced;
- `ABSENT` only after absence was observed and the lock directory synced.

Any other bytes, oversize file, symlink, or non-regular witness state is never
one of these statuses and is never deleted.

`sley-repo::GcReport` remains the GC-owned collection report. On a partial
delete failure, `deleted_objects` contains only objects whose unlink and
containing-directory sync both completed before the failure. `failed_object`
names the current object whether unlink failed or unlink succeeded but its
directory sync failed. The retry replans from current verified inventory and
redurabilizes validated leaf directories before it may report collection
success.

No report grants authority to update a pointer, delete an orphan receipt or
origin, reconstruct bytes, or select a rollback target.

## 9. Corruption precedence and confinement

Cleanup and visible-state verification are distinct. Successfully removing
owned stages does not make corrupt surviving state acceptable. Recovery must
return the first exact component failure when:

- an accepted head is malformed or targets incomplete transaction evidence;
- a visible named ref is malformed, unbound, or targets incomplete transaction
  evidence;
- a repository-owned traversal component is a symlink or has invalid shape;
- an immutable final object or receipt selected by a visible closure fails its
  existing codec, digest, manifest, root, policy, or inventory checks.

Recovery never rewrites a final object, receipt, branch origin, ref, or head.
It never promotes a stage, parses a partial stage as authority, scans outside
the exact repository root, or deletes lookalike files.

The repository root and already-validated real directory components remain
trusted local storage. Cooperating Sley callers obey maintenance and component
locks. A privileged or external actor that replaces a directory entry while an
operation holds those locks is outside S20-530, as it was for S20-500. Static
symlinks and non-regular entries are rejected. Descriptor-relative no-follow
hardening may be added later without changing this recovery contract. The
no-outside-root claim is limited to this cooperating local threat model.

The checker owns one immutable `MULTIFAULT_OVERLAY_REGISTRY` in the exact
15-case order. Its SHA-256 is
`5562e9b7f78039276284581cfd057708ed705edcc219e5375f5c993b783c78b3`.
The first nine records cover the non-grouped ANC, owned-entry, and limit
precedence cases. The final six records cover the grouped `COR-06` and
`COR-07` leaves. Every record fixes one owner source, recovery operation,
fresh-fixture recipe, receiver, same-root exclusive guard, primary and
secondary overlays, distinctness class, exact winner and loser authorities,
primary-only repair class, and both operation profiles. Grouped overlays bind
their primary and secondary roles, paths, corrupters, and probes back to exact
`CORRUPTION_FIXTURE_SPECS` records.

The six grouped leaves preserve those fixture probes as direct-leaf authority.
Their M2 windows select two-argument test adapters through one separate closed
`GROUPED_M2_PROBE_ADAPTER_REGISTRY` with 11 exact key-and-side records and
SHA-256
`c5b1f1cb81b733d64d4a2667beaa52383b484fb81e8c62315d68c759f04b1e70`.
The 11 records use seven distinct private helpers: three transaction-owner
adapters for grouped pointer, absent-receipt, and receipt-digest probes, plus
four ref-owner adapters for origin import, origin ancestry, ref import, and ref
target binding. Every rendered direct M2 probe supplies exactly `&fixture` and
`&m2_fixture`. A grouped adapter cannot reuse the corresponding legacy
three-argument leaf helper, cross owners, or survive the normal build. The
`COR-07/ref_digest/ref_digest_mismatch` secondary side has no adapter because
its closed cycle descriptor replaces a direct probe call.

Four cycle overlays additionally carry one typed `CycleEpochContract` with
`epoch_budget`, `operation_1_claims`, `operation_1_edge_counts`,
`operation_2_claims`, `operation_2_edge_counts`, and `primary_fault_node`.
`ANC-04/accepted_nested_codec_before_cycle` has `(2, 1, (1, 0), 1,
(1, 1), RIGHT_RECEIPT)`. `ANC-04/verifier_nested_store_before_cycle` has the
same counts with `RIGHT_OBJECT`. `ANC-06/ref_nested_store_before_cycle` has
the same counts with `DEPTH_ONE_RIGHT_ANCESTOR_OBJECT`.
`COR-07/ref_digest/ref_digest_mismatch` has `(1, 0, (0, 0), 1, (1, 1),
None)`. The closed epoch plan is installed exactly once before either fault
probe or recovery call. Each owning recovery operation that reaches ancestry
claims and finalizes one immutable ordinal through an RAII guard on every
return path. An operation that fails before ancestry records zero claims and
zero edge counts. The primary repair window cannot install, rearm, inspect, or
drain the plan. One final
take after the second operation and its state assertions drains the exact
observations and clears the plan. A secondary descriptor assertion before the
first operation is non-consuming; no production ancestry traversal probe may
consume an epoch before operation one.
`primary_fault_node` is proved before operation one from the exact right-side
receipt or object path and identity. It is not accepted as installer or drain
telemetry. Plan owner-root and digest assertions are also pre-operation. Only
epoch claims, edge counts, and budget exhaustion come from the single final
drain. Actual plan clearing is proved by the frozen `take` body and an isolated
hook module test that installs a fresh independent plan after a completed take.

Every multifault test uses one contiguous checker-rendered M2 window. It first
constructs the secondary-only baseline with a pristine primary carrier, proves
the secondary fault directly, activates and proves the primary fault, then
invokes the exact owning recovery operation. The first call must return the
exact winner and leave the owner tree plus both fault observations unchanged.
The test repairs only the primary fault back to the captured secondary-only
baseline, reuses the same receiver, canonical root, guard, and arguments, and
invokes the owning recovery operation a second time. The second call must
return the exact loser, including exact enum fields where applicable, and must
again leave the tree and secondary state unchanged. A loser-exclusion assertion
is not evidence of the loser. Limit cases may change only the selected private
profile field from one to two between calls. The three-symlink cases must repair
all three exact links and sync every distinct parent.

For a multifault case, the matrix entry's base error semantics are an exact
projection of operation one's M2 result and first unchanged-tree snapshot; the
entry does not bind a second generic `error` or `result` authority outside the
M2 window. Grouped multifault leaves likewise use their complete M2 fixture,
probe, result, repair, and snapshot plan as the sole corruption proof and omit
the mutually exclusive generic fixture and preflight blocks. The two limit
multifault cases, `LIMIT-02/final_receipts` and `LIMIT-03/final_origins`, first
complete their ordinary exact-limit success and limit-plus-one failure prefix.
They then append one isolated M2 suffix over a fresh fixture: operation one
proves the limit winner, raising only that selected limit from one to two
exposes the corruption loser on operation two, and both calls preserve the
whole owner tree and their direct fault observations.

The evidence object appends the ordered fields
`multifault_plan_sha256`, `primary_fixture_assertions`,
`secondary_fixture_assertions`, `distinctness_assertions`,
`primary_probe_assertions`, `secondary_probe_assertions`,
`m2_operation_bindings`, `m2_result_assertions`, `m2_repair_assertions`, and
`m2_cycle_epoch_assertions`, and `m2_snapshot_assertions`. Every assertion is
checker-rendered, direct, unique,
present in the mapped test, and bound to its exact M2 phase. No new fixture,
receiver, guard, hidden repair, third recovery call, nested or unreachable
assertion, conditional exit, thread, process, unsafe block, or foreign call may
enter the window.

Within each owner, exact precedence is:

1. root, exclusive-guard, and fixed layout validation;
2. a complete bounded read-only inventory plan over the owned recovery tree;
3. visible pointer decoding, direct target verification, and immutable closure
   verification;
4. complete bounded ancestry, counter, and report-size verification;
5. exact owned-stage cleanup and leaf sync;
6. deterministic successful report construction.

Steps 1 through 4 are read-only. Any failure in them leaves the invocation's
entire exact owner tree byte-for-byte unchanged. The inventory plan records
candidate owned stages but does not unlink them. Transaction recovery completes
its receipt, head, accepted-closure, ancestry, and limit preflight before it
calls store cleanup or deletes transaction-owned stages. Ref recovery completes
its origin, ref, orphan-report, visible-closure, ancestry, and limit preflight
before it deletes repository-owned stages. An injected host failure during step
5 may leave partial stage cleanup, which the retry rows cover.

Exact pointer and codec errors precede semantic cross-record and ancestry
comparisons. Composite recovery runs transaction recovery, then ref recovery,
then GC-witness recovery. It never deletes a witness or ref stage after an
earlier transaction failure, but cleanup completed by one owner is not rolled
back if a later owner fails its read-only preflight.

## 10. Validation and closeout

The structural checker is `scripts/check_s20_530_crash_recovery.py`, and the
only closeout execution recorder is `scripts/run_s20_530_validation.py`. The
checker must verify the exact 100-row matrix, owner boundaries, exclusive
recovery rules, test-to-row evidence, complete input closure, captured command
logs, phase-bound reviews, report semantics, unchanged dependency direction,
and honest machine-summary state.

Tier 1 is `make quick`. Tier 2 for S20-530 is:

```text
cargo test -p sley-store --lib --locked
cargo test -p sley-txn --lib --locked
cargo test -p sley-repo --lib --locked
cargo test --workspace --locked
python3 scripts/check_scb1_spec.py
python3 scripts/check_oracle_independence.py
cargo test -p sley-scb1 --locked
cargo test -p sley-schema --locked
cargo test -p sley-store --locked
cargo test -p sley-state-root --locked
cargo test -p sley-repo --locked
cargo test -p sley-txn --locked
uv run --project oracle/scb1 --frozen python -m unittest discover -s oracle/scb1/tests -v
uv run --project oracle/scb1 --frozen sley2-scb1-oracle check --accepted conformance/scb1/v1/accepted.json --rejected conformance/scb1/v1/rejected.json
uv run --project oracle/scb1 --frozen sley2-scb1-oracle check-mutation-value --accepted conformance/mutation-value/v1/accepted.json --rejected conformance/mutation-value/v1/rejected.json
uv run --project oracle/scb1 --frozen sley2-scb1-oracle check-mutation-candidate --accepted conformance/mutation-candidate/v1/accepted.json --rejected conformance/mutation-candidate/v1/rejected.json
uv run --project oracle/scb1 --frozen sley2-scb1-oracle check-candidate-result --accepted conformance/candidate-result/v1/accepted.json --rejected conformance/candidate-result/v1/rejected.json
uv run --project oracle/scb1 --frozen sley2-scb1-oracle check-transaction-receipt --accepted conformance/transaction-receipt/v1/accepted.json --rejected conformance/transaction-receipt/v1/rejected.json
uv run --project oracle/scb1 --frozen python scripts/check_schema_epoch_vector.py
uv run --project oracle/scb1 --frozen python scripts/check_state_root_vector.py
uv run --project oracle/scb1 --frozen python scripts/check_repository_pack_vector.py
cargo test -p sley-mutate mutation_value_codec_adversarial --locked
cargo test -p sley-adapter authorized_adapter_request_binding_confusion_fails_before_charge --locked
python3 scripts/reconcile_s20_530_exception_ledgers.py evidence/validation/s20-530-crash-recovery-test-plan-v1.json
cargo fmt --all -- --check
```

The closeout requires independent architecture, specification, and adversarial
review with no open P0 through P4 finding. Full `make v1`, `make v2`, and the
release gate remain release-boundary work unless an independent finding widens
the required validation scope.

## 11. Explicit exclusions

S20-530 does not implement semantic comparison, merge/conflict objects,
clone-equivalent exchange, tags, ref deletion, force movement, named-branch
candidate commit, protocol recovery commands, CLI recovery commands, runtime
deployment, remote repair, benchmark execution, release packaging, or GA.
