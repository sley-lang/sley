# S20-530 object-not-found source-helper blocker

Status: deferred frozen helper and source-kind blocker at
`e78e7c8cad036d0312345ce6616d12f27f027727`.

## Scope

This note records why the final frozen COR-07 `object_absent` leaf cannot
execute its required exact source-chain assertion under the S20-530 v4
checker. It does not reopen or modify the checker, and it does not authorize a
production object-store or error-ownership change.

The affected leaf is:

- `target_object_store_object_not_found`

The frozen leaf requires:

- result code `STORE_OBJECT_NOT_FOUND`
- variant `branch.transaction.commit.store`
- direct source chain `CommitError`, `StoreError`, `io::Error(NotFound)`
- corrupter class `object_absent`
- probe class `object_store_read_error`

## Reachable production result

Removing the selected head object is sufficient to reach the required
production error. `ObjectStore::read` performs path metadata inspection,
maps the host `NotFound` error through `StoreError::io`, and preserves that
host error as the `StoreError` source. Transaction and branch recovery then
preserve the required `CommitError::Store` ownership.

The runtime source chain is therefore exactly the chain named by the frozen
leaf. The blocker is not production reachability.

## Frozen helper conflict

The checker freezes the complete body of
`crate::refs::tests::exact_error_source_chain`. Its I/O branch accepts only:

```rust
::std::io::ErrorKind::Other => "io::Error(Other)"
```

Every other I/O kind enters the frozen panic branch. The repository helper
matches that checker-owned body exactly.

The leaf's required assertion would be:

```rust
::core::assert_eq!(
    crate::refs::tests::exact_error_source_chain(&error),
    ["CommitError", "StoreError", "io::Error(NotFound)"],
);
```

At runtime, the helper reaches the preserved `io::ErrorKind::NotFound` and
panics before returning the array. Adding a `NotFound` match arm would make the
test executable, but `error_source_helper_problem` would then reject the
helper because its body differs from the frozen checker rendering.

## Decision

This leaf stays deferred. No test-side object corruption can both preserve
the required `NotFound` source and make the frozen helper accept it. A future
separately authorized S20-530 refreeze can add the `NotFound` label to the
exact helper body, or revise the leaf's source-chain requirement.

Development continues on v4-compatible leaves without collapsing the real
object-store source chain or mutating the immutable checker.

## Frozen v4 identity

- Contract set SHA-256: `4c6b0e12836f6601df4cfc67e36fdf754b7bd1a60c7df2ca21c2b6e037ca93fa`
- Evidence payload SHA-256: `cf8491ff4928679554b138f2b2391f3bb4f67699c8aec366e6843e05e9306467`
- Checker raw SHA-256: `44b7d77bd010ae0d82bc5c32de3870133373776913d92418b5af7590cf9e377d`
- Checker self SHA-256: `4941f73a60f1e80807d517cd49ffe53109edc845e24c6933ebdd58ad9e010752`
