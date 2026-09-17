# RW-080 §1.3 package-builder slice 36: structural section hydration

Status: PROVISIONAL C0 CONSTRUCTION. This is the bounded native loading side
of the candidate byte handoff under the operator development override. It has
not passed the required contract/surface review and is not runtime authority.

## Scope and behavior

The native boundary now reconstructs the exact in-memory execution package
from a serialized v2 envelope. Strict decoders cover constants, layouts,
imports, and the dependency inventory. Every allocation is preflighted by the
section byte ceiling, a frozen language count ceiling, or the minimum encoded
row width. Type decoding covers all twenty structural `TypeExpr` tags under
the existing depth bound. Length-delimited rows must consume all their bytes,
and every section must consume its complete payload.

Constants use the existing canonical constant codec. Layouts preserve record
and variant rows, type parameters, invariants, visibility, nested types, and
function effects. Imports preserve exact schemas and effect identities.
Dependency hydration preserves entry, epoch, root, the full cache profile,
gate counts and closure fingerprints, admitted limits, globals, contracts,
bindings, and optional resource limits. Duplicate identities in each indexed
inventory refuse structurally.

`hydrate_package_envelope_v2` composes framing authentication and all four
section decoders. It compares the dependency entry/epoch/root/profile against
the header, reconstructs `ExecutionPackage`, requires every decoded section to
re-encode byte-identically, and re-derives every digest. The result carries no
admission receipt and performs no reference resolution, type judgment,
contract judgment, closure validation, or evidence minting.

## Verification

The populated round trip covers constants, a record layout, an exact import,
a global, a contract with result/global bindings and resource limits, an
optional cancellation point, and two closure fingerprints. Negative coverage
includes duplicate globals and contracts plus a split-binding envelope whose
dependency bytes and digest agree but whose dependency entry disagrees with
the header. The raw framing decoder accepts that authenticated shape; full
hydration refuses it as `PACKAGE_BINDING_MISMATCH`.

All `sley-vm` package tests and Clippy with warnings denied pass for the slice.

## Explicit remainder

Sley does not yet emit the constants, layouts, imports, or dependency section.
The candidate envelope and hydration surface also still require the recorded
RW-080 contract/surface review. Those are the remaining AT-EC-07 obligations;
R2 remains provisional pending the independent acceptance debt.
