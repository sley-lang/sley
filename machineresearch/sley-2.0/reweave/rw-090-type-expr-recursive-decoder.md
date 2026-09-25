# RW-090 recursive TypeExpr decoder

Date: 2026-09-18

Status: implemented recursive structural slice; RW-090 remains open

## Result

The Sley codec now validates complete recursive `TypeExpr` trees without a
recursive function-call cycle. The driver owns two indexed maps: pending
canonical node bytes and the depth of each node. It processes nodes in stable
insertion order, invokes the shallow child projector, appends direct and list
children, and returns the original canonical root bytes after exhausting the
worklist.

A child may be appended only when its parent depth is less than 63. A leaf at
depth 63 is accepted; a composite at depth 63 returns
`SCB_RESOURCE_LIMIT`. This matches the native standalone codec boundary of 63
nested containers plus a leaf.

The reachable closure contains 13 functions and has 1,267 parameters, 293
blocks, 526 operations, and 166 constants. Its image is 71,588 bytes. The
approved package digest is
`4db4851ea7eeb6de3319a895afd78a9db0176545acee6f974fc4db2366973be5`
under the codec-profile execution limits.

## Function integration

Function field 3 now invokes the recursive decoder. A nested
`Option<Tuple<Bool, Bytes>>` fixture succeeds, while a tuple containing an
unknown child tag returns `SCB_UNION_INVALID`. The current Function schema
image contains 15 reachable functions, 1,427 parameters, 334 blocks, 601
operations, and 191 constants. Its image is 82,222 bytes and its package
digest is
`b5ad5270511b9ee6f2e11a7247cb24564553a982f84b7988f3b2d8ae4b61963a`
under the codec-profile execution limits.

## Validation

```text
cargo test -p sley-vm --test rw120_toolchain_integration type_expr_recursive_decoder -- --nocapture
cargo test -p sley-vm --test rw120_toolchain_integration function_schema_decoder -- --nocapture
```

The recursive corpus includes all composite families in one nested function
type, a maximum-depth accepted option chain, a nested unknown tag, and a
one-level depth overflow.
