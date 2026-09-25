# Sley 2 Architecture

Status: normative baseline, first written at M1. The crate table and the
dependency order below describe the workspace as of 2.0.1.

## Target state

Sley 2 owns one semantic authority: a narrow Rust kernel that validates and
executes immutable SSMC1 objects encoded in SCB1. Higher layers may transport,
query, cache, display, or benchmark those facts but may not redefine them.

```text
agent -> generated adapter -> SMP1 -> query/mutation -> checker/policy
                                                   -> transaction engine
                                                   -> object store/repository
                                                   -> deterministic VM/adapters
```

## Dependency law

Dependencies point one way, from the canonical layers up to the transport.
The workspace realizes this order (each crate depends only on crates in
earlier layers, and not on every one of them):

```text
id -> scb1 + ssmc -> schema -> state-root + store
ssmc -> check -> query + mutate -> vm -> tests -> policy -> txn -> repo
vm + tests -> conformance + test-runner        policy -> adapter
kernel crates -> protocol -> json-bridge -> cli        test-runner -> cli
```

Transport, CLI, adapters, benchmarks, optional ZJX compression, Git, Siglum,
and every model provider remain outside the semantic kernel. No dependency may
point from a kernel crate to `sley-cli`, `sley-json-bridge`, or a Greyforge
product adapter. The succession benchmark is Python under `bench/`, not a
crate, and nothing in `crates/` depends on it.

## Crates

| Crate | Sole authority |
|---|---|
| `sley-id` | domain-separated identifiers and digests |
| `sley-scb1` | SCB1 bytes and strict canonical decode |
| `sley-ssmc` | entity, type, and constant model |
| `sley-schema` | schema epochs and generated field contracts |
| `sley-state-root` | deterministic, ancestry-independent state roots |
| `sley-store` | immutable object persistence and corruption checks |
| `sley-check` | graph, reference, type, CFG, effect, and contract validity |
| `sley-query` | derived indexes, bounded queries, and capsules |
| `sley-mutate` | typed candidates, operations, and preconditions |
| `sley-vm` | deterministic derived bytecode and the reference VM |
| `sley-tests` | native test evidence: plans, reports, and approvals |
| `sley-policy` | protected policy roots and candidate validation |
| `sley-adapter` | bounded reference adapter contracts |
| `sley-conformance` | deterministic execution and test report envelopes |
| `sley-txn` | atomic commit, receipts, and the accepted head |
| `sley-repo` | refs, branches, comparison, merge, conflicts, exchange, GC |
| `sley-protocol` | SMP1 framing, negotiation, and identity scoping |
| `sley-json-bridge` | generated non-canonical JSON mapping of SMP1 frames |
| `sley-test-runner` | host-side native test supervision |
| `sley-cli` | thin machine wrapper; no semantic rules |

## Canonical and derived state

Canonical state is limited to facts required for program identity and kernel
judgment. Reverse indexes, caches, bytecode, short handles, rankings, reports
not explicitly bound into program state, JSON, and debug notation are derived.
Derived state must be reproducible or disposable and cannot grant authority.

## Version identities

- `EntityId` is stable logical identity.
- `ObjectId` addresses immutable canonical bytes.
- `StateRoot` addresses ancestry-independent semantic state.
- `TransactionId` addresses a parent-bound receipt and therefore repository
  ancestry.
- `SchemaEpochId` addresses the rules needed to decode and judge a state.

The policy root is protected and separately versioned. A candidate binds the
exact policy root used for judgment but may not modify it in the same
transaction.

## Durability

Commit order is validate, stage immutable objects, verify, promote, write and
sync receipt, sync directories, compare-and-swap the ref, then report success.
A crash may create unreachable objects; it may never expose partial accepted
state.

## Evolution

Epoch migrations decode with the old epoch, build a new state, record both
roots in a migration transaction, and preserve the old root. Silent fallback,
normalization, or schema downgrade is forbidden.
