# RW-090 TypeExpr child projection

Date: 2026-09-18

Status: implemented shallow composite slice; RW-090 remains open

## Result

The Sley codec now parses every `TypeExpr` family without a recursive call
cycle. A shallow child projector validates one canonical node and returns two
optional direct children plus an indexed map of list children. This shape is
the input boundary for the bounded iterative worklist.

The projector covers:

- tuple element lists;
- named types with exact 32-byte definition identity and argument list;
- vector, option, and local-cell unary children;
- ordered-map and result two-field records;
- function-reference parameter lists, result child, and strictly increasing
  effect identities; and
- every previously validated leaf family.

Exact two-field and three-field record projectors first compare the encoded
field count, then reject unknown tags before extracting required values. This
preserves native missing-versus-unknown precedence rather than inferring it
from a sequence of map lookups.

The reachable closure contains 12 functions and has 1,127 parameters, 266
blocks, 479 operations, and 151 constants. Its image is 64,094 bytes. The
approved package digest is
`535d77a523307ad72119d91f32332f115152ca80a217b963368741bddf693414`
under the codec-profile execution limits.

## Validation

```text
cargo test -p sley-vm --test rw120_toolchain_integration type_expr_children_decoder -- --nocapture
```

The positive corpus covers all eight composite families plus a leaf. Negative
cases cover a nonminimal tuple list, missing and unknown pair fields, a short
named definition identity, unordered function effects, and an unknown union
tag.

This slice validates one node and exposes its children. The recursive decoder
now consumes this boundary with explicit depth accounting and Function field-3
integration.
