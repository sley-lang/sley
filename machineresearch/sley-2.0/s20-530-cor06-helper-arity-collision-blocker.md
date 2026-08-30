# S20-530 COR-06 helper arity collision blocker

Status: deferred frozen helper-name blocker at
`3ff31643b8672d3d6ffe3add90e102c0102ee394`.

## Scope

This note records why 68 frozen COR-06 leaves cannot be added to the current
Rust test module without changing an immutable S20-530 v4 helper call or an
existing frozen ANC-04 helper. It does not reopen or modify the v4 checker.

The collision affects these frozen COR-06 probe classes:

- `import_transaction_receipt_error`: 65 leaves
  - 53 `receipt_digest` leaves
  - four `root` leaves
  - eight `policy` leaves
- `object_store_read_error`: three leaves
  - one `object_missing` leaf
  - two `object_digest` leaves

The two COR-06 host I/O leaves use unique path-bound probe names and are not
affected. The remaining four COR-06 leaves have separate contract-shape or
source-chain blockers and are outside this note.

## Frozen call collision

Every affected COR-06 fixture plan freezes a three-argument direct probe call.
The receipt form is:

```rust
import_transaction_receipt_error(&fixture, &fixture_observation, &fault_path)
```

The object-store form is:

```rust
object_store_read_error(&fixture, &fixture_observation, &fault_path)
```

ANC-04 already freezes two-argument functions with the same names in the same
`crate::repository::tests` module:

```rust
fn import_transaction_receipt_error<FixtureType: NestedReceiptMultifaultFixture>(
    _fixture: &Fixture,
    m2_fixture: &FixtureType,
) -> Result<ImportedTransactionReceipt, TransactionCodecError>
```

```rust
fn object_store_read_error(
    fixture: &Fixture,
    m2_fixture: &VerifierNestedStoreCycleFixture,
) -> Result<Vec<u8>, MappedStoreProbeError>
```

Rust does not overload free functions by arity or parameter type. On
`rustc 1.93.0 (254b59607 2026-01-19)`, adding the required three-argument
definition under either frozen name produces `E0428` because the value name is
defined more than once. Leaving only the ANC-04 definition makes each COR-06
call fail with `E0061` because it supplies three arguments to a two-argument
function.

Renaming a COR-06 probe would make the rendered fixture plan differ from the
frozen checker. Renaming or changing either ANC-04 function would invalidate
the already accepted ANC-04 exact evidence. A trait, macro, or optional
argument cannot make both frozen free-function call shapes resolve under one
Rust value name without changing at least one frozen definition or call.

## Decision

These 68 leaves stay deferred. A future separately authorized S20-530 refreeze
can assign distinct COR-06 probe names, move one helper set behind a namespace,
or deliberately revise both frozen call sites and helper bodies together.

Development continues on v4-compatible leaves with collision-free probe names.
No production codec or object-store behavior is changed to work around this
test-module namespace conflict.

## Frozen v4 identity

- Contract set SHA-256: `4c6b0e12836f6601df4cfc67e36fdf754b7bd1a60c7df2ca21c2b6e037ca93fa`
- Evidence payload SHA-256: `cf8491ff4928679554b138f2b2391f3bb4f67699c8aec366e6843e05e9306467`
- Checker raw SHA-256: `44b7d77bd010ae0d82bc5c32de3870133373776913d92418b5af7590cf9e377d`
- Checker self SHA-256: `4941f73a60f1e80807d517cd49ffe53109edc845e24c6933ebdd58ad9e010752`
