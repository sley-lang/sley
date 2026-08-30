# S20-530 COR-07 ref-digest array-shape blocker

Status: deferred contract-shape blocker at `f69f34a68b9fe66ee3c41a3bc457842f0f9d22be`.

## Scope

This note records why the frozen COR-07 key
`("COR-07", "ref_digest", "ref_digest_mismatch")` cannot be implemented
against the S20-530 v4 exact-body contract without another explicit refreeze.
It does not reopen or modify the v4 checker.

## Evidence

The checker-rendered direct test
`cor07_ref_digest_ref_digest_mismatch` passed the static multifault operation
binding check with exactly 122 statements. Compilation then failed at the
frozen assertion:

```rust
::core::assert_eq!(
    ("secondary_probe_branch_ancestry_cycle", m2_secondary_cycle_descriptor.as_slice()),
    ("secondary_probe_branch_ancestry_cycle", [
        (m2_cycle_left_transaction_id, m2_cycle_right_transaction_id),
        (m2_cycle_right_transaction_id, m2_cycle_left_transaction_id),
    ]),
);
```

The preceding frozen declaration fixes `m2_secondary_cycle_descriptor` as an
array of two transaction-ID pairs. Its inherent `as_slice()` method therefore
returns a borrowed slice, while the right side remains an owned array. Rust
reports `E0308`: expected `(&str, &[(TransactionId, TransactionId)])`, found
`(&str, [(TransactionId, TransactionId); 2])`.

The result was reproduced independently on
`rustc 1.93.0 (254b59607 2026-01-19)` with a two-element integer array. An
extension trait defining a same-named `as_slice()` that returns an array does
not change resolution: Rust selects the inherent array method and produces
the same type mismatch. Explicit trait qualification could select the trait,
but the frozen statement does not contain such a call.

## Decision

No source-side implementation can change this assertion's two operand types
without changing a frozen statement or changing public transaction-ID types.
The latter would be an unrelated and unacceptable API mutation. The
uncommitted implementation attempt was removed in full, and the three prior
COR-07 runtime cases remain intact.

This leaf stays deferred. Any future correction requires a separately
authorized S20-530 contract refreeze that changes the right side to a slice,
removes `.as_slice()` from the left side, or otherwise makes both operands the
same type. Until then, development proceeds on other v4-compatible leaves.

## Frozen v4 identity

- Contract set SHA-256: `4c6b0e12836f6601df4cfc67e36fdf754b7bd1a60c7df2ca21c2b6e037ca93fa`
- Evidence payload SHA-256: `cf8491ff4928679554b138f2b2391f3bb4f67699c8aec366e6843e05e9306467`
- Checker raw SHA-256: `44b7d77bd010ae0d82bc5c32de3870133373776913d92418b5af7590cf9e377d`
- Checker self SHA-256: `4941f73a60f1e80807d517cd49ffe53109edc845e24c6933ebdd58ad9e010752`
