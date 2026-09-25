# RW-080 §1.3 lowerer slice 34: ordered transitive-callee image table

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley lowering and
encoding composition under the operator development override. It is not
RW-110 completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

The complete image entry now accepts a runtime vector of complete callee facts.
Sley computes its UInt64 length, writes that count after the root body, walks
every element in supplied order, runs each element through the same complete
semantic function-body lowerer used for the root, and appends every resulting
body. Empty and nonempty inventories share the same byte path.

Each callee fact carries identity, parameter registers, encoded register and
result types, entry slot, declared block count, and complete block facts.
Lowering or encoding errors propagate unchanged. Checked index advance and
bridge failures use the frozen `VM_LOWER_RESOURCE_LIMIT` value, while an
impossible in-range vector miss traps as an internal invariant.

The current boundary requires its caller to provide the transitive inventory
in canonical ascending identity order. This slice owns traversal, semantic
lowering, count emission, and serialization of that inventory; discovering the
transitive call graph and proving the supplied order remain driver work.

## Native parity corpus

The parity fixture contains a root that calls function `0x14…`, which in turn
calls leaf `0x0a…`. Native lowering discovers the two-level closure and emits
the callee bodies in ascending identity order (`0x0a…`, then `0x14…`), which
differs from discovery order. The Sley image encoder consumes those semantic
facts, walks both callee rows, and returns bytes exactly equal to the native
extended-profile `LoweredFunction::bytes` image.

All forty-two tests in `rw080_lower_scaffold` pass; focused Clippy with warnings
denied is clean.

## Explicit remainder

Transitive callee discovery and canonical-order validation still sit outside
this bounded encoder entry. Execution-package assembly and removal of the
temporary duplicate body/header lowering pass also remain. The driver remains
RW-120 work. RW-080 and R2 stay provisional pending the recorded independent
acceptance debt.
