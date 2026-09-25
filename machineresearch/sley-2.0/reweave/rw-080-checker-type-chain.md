# RW-080 §1.2 checker slice 4: Sley-owned unary type chains

Status: PROVISIONAL C0 CONSTRUCTION. This is an executable Sley type-checker
slice under the operator development override. It is not RW-100 completion,
C1, self-hosting evidence, or runtime authority.

## Scope and behavior

`unary_type_chain_checker` accepts a runtime vector of unary wrapper tags, a
leaf tag and payload, and the in-scope type-parameter count. The admitted
projection supports `Vector`, `Option`, and `LocalCell` wrappers around `Bool`,
`UInt`, or `TypeParameter` leaves. Sley walks every wrapper with
`VectorLen`, `VectorGet`, and a CFG backedge, then judges the selected leaf.

The program enforces the frozen `MAX_TYPE_DEPTH` boundary before leaf
validation, accepts only epoch-1 integer widths 8/16/32/64/128, and requires a
type-parameter index below the supplied declaration count. Success returns the
number of represented type nodes. Failures preserve:

- `TYPE_DEPTH_LIMIT` (`21000`);
- `TYPE_WIDTH_INVALID` (`21001`);
- `TYPE_PARAMETER_OUT_OF_SCOPE` (`21002`); and
- `TYPE_RESOURCE_LIMIT` (`21018`) for checked loop/count overflow.

Unknown wrapper or leaf tags are unreachable for this admitted typed
projection and trap `InternalInvariant`; they are not remapped to a native
semantic error that has no corresponding `TypeExpr` value.

## Native parity and negative corpus

`native_unary_type_chain` independently reconstructs the selected nested
`TypeExpr` and invokes `TypeEnvironment::check_type`. Tests compare valid
mixed wrappers, an unwrapped Boolean, and an in-scope parameter, repeat Sley
execution for determinism, and pin both sides of the depth boundary. The
negative corpus covers invalid width, out-of-scope parameter, and the native
precedence where excessive nesting hides a deeper invalid width. All ten tests
in `rw080_checker_scaffold` pass; focused Clippy with warnings denied is clean.

## Explicit remainder

This slice covers unary structural recursion only. Tuples, maps, results,
functions, named definitions and cycles, definition/member inventories,
traits, constants, arbitrary decoded `TypeExpr` objects, effects, contracts,
and mandatory test planning remain RW-100 work. RW-080 and R2 stay provisional
pending the recorded independent acceptance debt.
