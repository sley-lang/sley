# S20-530 empty source-chain inference blocker

Status: deferred contract-shape blocker, expanded for COR-06 at
`84f088194decc82a6bb28568688bb9c3ed27b570`.

## Scope

This note records why sixteen frozen COR-06 and COR-07 leaves with empty
direct error source chains cannot be implemented against the S20-530 v4 exact
assertion contract without another explicit refreeze. It does not reopen or
modify the v4 checker.

The affected COR-06 leaves are:

- `head_checksum/head_shape_before_receipt_missing`
- `head_checksum/head_checksum_before_receipt_corrupt`
- `receipt_missing/repository_recovery_receipt_incomplete`
- `manifest_length/repository_txn_object_inventory_mismatch`

The affected non-multifault COR-07 leaves are:

- `origin_format/branch_record_field_shape`
- `ref_format/ref_name_invalid`
- `ref_format/ref_name_reserved`
- `ref_format/ref_field_shape`
- `ref_format/ref_name_collision`
- `origin_ref_binding/ref_branch_binding_mismatch`
- `origin_ref_binding/recovery_named_ref_incomplete`
- `target_binding/ref_target_mismatch`
- `origin_ancestry_binding/branch_origin_mismatch`
- `origin_ancestry_binding/branch_ancestry_cycle`
- `origin_ancestry_binding/branch_resource_limit`
- `origin_ancestry_binding/semantic_ref_io`

The four empty-chain leaves covered by frozen multifault plans use labeled
tuple operands, which provide enough type context and compile. They are not
blocked by this issue. The separate `ref_digest/ref_digest_mismatch`
multifault leaf remains blocked by the array-versus-slice shape recorded in
`s20-530-cor07-ref-digest-array-shape-blocker.md`.

## Evidence

The frozen direct semantic assertion is accepted by the checker only in this
shape, or with its checker-accepted empty-vector alternative:

```rust
::core::assert_eq!(
    crate::refs::tests::exact_error_source_chain(&error),
    [],
);
```

The frozen helper returns a const-generic array:

```rust
fn exact_error_source_chain<const N: usize>(
    error: &(dyn ::std::error::Error + 'static),
) -> [&'static str; N]
```

On `rustc 1.93.0 (254b59607 2026-01-19)`, the direct assertion fails with
`E0282` because neither the helper call nor the zero-length array literal gives
the macro enough information to infer the const length and empty-array element
type. A minimal reproducer is:

```rust
fn exact_error_source_chain<const N: usize>() -> [&'static str; N] {
    Vec::new().try_into().unwrap()
}

fn main() {
    assert_eq!(exact_error_source_chain(), []);
}
```

Adding only `::<0>` still leaves the empty literal's element type unresolved.
The smallest tested direct form that compiles supplies both pieces of type
information:

```rust
assert_eq!(
    exact_error_source_chain::<0>(),
    [] as [&str; 0],
);
```

The v4 checker rejects that form because it is not the exact direct helper
output and literal pair. Its accepted `Vec::<String>::new()` alternative also
cannot compile with the frozen array-returning helper: Rust reports `E0277`
because `[&str; _]` has no `PartialEq<Vec<String>>` implementation. The labeled
tuple form used by the multifault assertions compiles, but the checker rejects
it for these direct semantic assertions.

The COR-06 fixture plans use different corruption and direct-probe helpers,
but their semantic tail freezes the same untyped empty direct source-chain
assertion. This note makes no additional claim about whether another fixture
constraint would also block an individual leaf.

## Decision

No test-side implementation can satisfy both Rust type checking and the
frozen direct source-chain assertion rule for these twelve leaves. Changing
the helper return type would also violate the frozen helper body and would
disturb the already-compiling nonempty-chain assertions.

These sixteen leaves stay deferred. A future separately authorized S20-530
refreeze can resolve the blocker by freezing a typed zero-length array
assertion, a labeled tuple assertion, or a compatible non-generic source-chain
helper. Development continues on v4-compatible leaves with nonempty source
chains.

## Frozen v4 identity

- Contract set SHA-256: `4c6b0e12836f6601df4cfc67e36fdf754b7bd1a60c7df2ca21c2b6e037ca93fa`
- Evidence payload SHA-256: `cf8491ff4928679554b138f2b2391f3bb4f67699c8aec366e6843e05e9306467`
- Checker raw SHA-256: `44b7d77bd010ae0d82bc5c32de3870133373776913d92418b5af7590cf9e377d`
- Checker self SHA-256: `4941f73a60f1e80807d517cd49ffe53109edc845e24c6933ebdd58ad9e010752`
