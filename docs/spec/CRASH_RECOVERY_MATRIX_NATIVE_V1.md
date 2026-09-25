# Crash Recovery Matrix — Native Commit and Clone

Status: N8 qualification contract; frozen at land. Implementation state is
tracked separately in closeout evidence, never by editing this matrix.

## 1. Scope and authority

This contract defines deterministic local recovery and crash-injection
evidence for the native commit path (`sley-txn` format-2 receipts, the
attempt journal, and the mixed-format accepted head) and the native clone
import path (`sley-repo` `exchange/v2` marker discipline). It mirrors the v1
`CRASH_RECOVERY_MATRIX_V1.md` row discipline without touching it: no frozen
v1 variant, probe, row-encoded name, or checker expectation changes. The v1
matrix, ADR-0023, and the S20-530 validation runner remain byte-for-byte
immutable and authoritative only at their accepted historical state.

Component ownership remains unchanged:

- `sley-txn` owns native receipt staging, the attempt journal, the native
  head advance through the shared CAS, and journal reconciliation under
  recovery;
- `sley-repo` owns the native clone marker and clone convergence;
- the shared object promotion, receipt fan-out layout, and head CAS already
  covered by v1 rows are reused, never duplicated: v1 `Tobj*`, `Obj*`,
  `Head*`, `Ref*`, and `Rcv*` hooks fire for native bytes through the same
  code, and the native rows below cover only native-specific boundaries.

The dependency direction remains `sley-repo -> sley-txn -> sley-store`.

Fault injection is test-only. No production API accepts a fault selector.
Native probes live in `#[cfg(test)]` thread-local one-shot selectors
(`NativeCommitDurabilityCut` in `crates/sley-txn/src/repository.rs`,
`NativeCloneDurabilityCut` in `crates/sley-repo/src/native_exchange.rs`)
with the same install/take/drop discipline as the v1 selectors. Shared v1
hooks (`Head04` below) are installed directly, never wrapped in a production
parameter.

## 2. Accepted outcome model

After an injected interruption in an already-initialized native commit, the
accepted head is exactly one of:

1. the old complete verified transaction and root (either format); or
2. the complete new verified native transaction and root with its exact
   receipt and object closure.

No partial receipt, root, object closure, head record, or journal claim may
be accepted as visible state. Unreachable immutable objects and complete
orphan format-2 receipts may remain for later GC or pack policy. Recovery
never promotes them, rolls a visible pointer backward, or guesses intent
from timestamps, names, enumeration order, stage contents, or record age.

The attempt journal adds exactly one honest subtlety over the v1 model: a
`PromotionStarted` claim whose receipt verifies but whose head never moved
is left intact by reconciliation — neither committed nor cleared — and any
resubmission of that attempt refuses `NATIVE_COMMIT_OUTCOME_UNKNOWN` rather
than forking a second commit. Only a fresh attempt identifier may commit.
A `PromotionStarted` claim whose receipt does not verify reconciles to
`OutcomeUnknown` with its identities cleared, exactly like a torn v1
receipt never becoming visible.

For clone imports, the outcome model is convergence: after any injected
marker interruption, exactly one retry lands the byte-identical clone an
uninterrupted import would produce, with no resume state left behind.

## 3. Native commit rows

| ID | Owner | Injected boundary | Required immediate state | Required recovery/retry result |
|---|---|---|---|---|
| `NTXN-01` | txn | during native receipt-stage write | outcome-unknown; journal `PromotionStarted`; accepted old; incomplete native stage | first recovery removes one receipt stage; claim reconciles to `OutcomeUnknown` with identities cleared; fresh attempt commits |
| `NTXN-02` | txn | after complete verified native stage, before final link | outcome-unknown; journal `PromotionStarted`; accepted old; complete owned stage only | recovery removes stage; claim reconciles to `OutcomeUnknown`; fresh attempt commits |
| `NTXN-03` | txn | after final native receipt link, before receipt-directory sync | outcome-unknown; journal `PromotionStarted`; accepted old; complete receipt and owned stage may exist | recovery removes stage; the verified receipt keeps the claim promoting; same-attempt resubmission refuses outcome-unknown; fresh attempt commits |
| `NTXN-04` | txn | after first native receipt-directory sync, before stage unlink | outcome-unknown; journal `PromotionStarted`; accepted old; complete durable receipt and stage | recovery removes stage; old head remains the only complete accepted revision; claim stays promoting; fresh attempt commits |
| `NTXN-05` | txn | after native receipt-stage unlink, before second receipt-directory sync | outcome-unknown; journal `PromotionStarted`; accepted old; complete receipt; stage absent | recovery resyncs leaf with zero removals; claim stays promoting; resubmission refuses; fresh attempt commits |
| `NTXN-06` | txn | after native receipt durable and verified, before head work | outcome-unknown; journal `PromotionStarted` with identities; accepted old; complete orphan native receipt allowed | recovery leaves the verified claim promoting; old head kept; fresh attempt commits |
| `NJRNL-01` | txn | admitted journal record write fails | `TXN_IO`; no worker ran; no journal record; accepted old | identical resubmission admits and commits; journal records the commit |
| `NJRNL-02` | txn | running-transition journal write fails | `TXN_IO`; journal `Admitted`; no worker ran; accepted old | identical resubmission executes once and commits |
| `NJRNL-03` | txn | promotion-claim journal write fails | outcome-unknown; execution ran; journal `Running` with no identities; accepted old | recovery skips the non-promoting record; the same attempt resubmits through admission and commits |
| `NJRNL-04` | txn | committed-record journal write fails | outcome-unknown; journal `PromotionStarted` with identities; head already advanced | recovery reconciles the verified claim against the moved head to `Committed`; attempt status resolves committed |
| `NHEAD-04` | txn | after native head rename, before head-directory sync (shared v1 `Head04` hook) | outcome-unknown; journal `PromotionStarted`; head reads new like v1 `HEAD-04` | recovery redurabilizes the boundary and reconciles the verified claim to `Committed`; new head verifies |

## 4. Native clone rows

| ID | Owner | Injected boundary | Required immediate state | Required recovery/retry result |
|---|---|---|---|---|
| `NCLONE-01` | repo | during clone marker temporary write | `EXCHANGE_IO`; torn temporary; no marker promoted | retry removes the torn temporary and converges to the byte-identical clone |
| `NCLONE-02` | repo | after verified marker temporary, before rename | `EXCHANGE_IO`; complete owned temporary only | retry starts clean and converges to the byte-identical clone |
| `NCLONE-03` | repo | after marker rename, before first directory sync | `EXCHANGE_IO`; marker renamed but durability uncertain | retry reuses the matching marker identity and converges to the byte-identical clone |

## 5. Test mapping

Each row maps to exactly one test. Row IDs are descriptive only; unlike the
v1 rows they are not parsed by any frozen checker, so no historical
validation can confuse them with v1 rows.

- `NTXN-01` through `NTXN-06`: `ntxn01_...` through `ntxn06_...` in
  `crates/sley-txn/src/repository.rs` (`native_commit_tests`), driven by
  `commit_native_with_commit_durability_cut`.
- `NJRNL-01` through `NJRNL-04`: `njrnl01_...` through `njrnl04_...` in the
  same module.
- `NHEAD-04`: `nhead04_native_head_rename_before_sync_accepts_new`, which
  installs the shared v1 `Head04AcceptedHeadRenameBeforeHeadSync` cut
  directly around `commit_native`.
- `NCLONE-01` through `NCLONE-03`: `nclone01_...` through `nclone03_...` in
  `crates/sley-repo/src/native_exchange.rs`, installing
  `NativeCloneCutSelection` around `import_native_exchange`.

Probes:

- `fail_selected_native_receipt_stage_write_cut` (half-write, flush, sync,
  then `RecoveryReceiptIncomplete`) and four boundary probes inside
  `persist_native_receipt`; the `Ntxn06` probe maps to outcome-unknown
  because any post-promotion failure is outcome-unknown by contract.
- Journal probes at the four `write_attempt_record` / `transition_attempt`
  sites in `commit_native_inner`: pre-promotion sites surface `TXN_IO`
  through the existing mapping; promotion and commitment sites map to
  outcome-unknown exactly like a real write failure there.
- Marker probes inside `install_native_stage_marker`: temporary half-write,
  pre-rename, and post-rename boundaries, all surfacing `EXCHANGE_IO`.
