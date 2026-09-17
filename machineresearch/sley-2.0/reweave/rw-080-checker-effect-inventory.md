# RW-080 §1.2 checker slice 6: Sley-owned effect-set inventories

Status: PROVISIONAL C0 CONSTRUCTION. This is an executable Sley effect-checker
slice under the operator development override. It is not RW-100 completion,
C1, self-hosting evidence, or runtime authority.

## Scope and behavior

`effect_set_inventory_checker` accepts runtime vectors for effect-definition
identities, a function's declared effect set, and the function's sorted/unique
local closure after operation scanning. Reusable Sley CFG subgraphs walk the
complete vectors: one requires strict definition and declaration ordering; a
nested loop resolves every declaration and local effect against the definition
inventory. Only after those passes succeed does Sley compare the complete
declared and computed sets.

The checker preserves the relevant native phase order and numeric codes:

- definition order and duplicates return `EFFECT_SET_NOT_CANONICAL` (`23002`);
- declaration order and duplicates return `GRAPH_INVENTORY_MISMATCH` (`22001`)
  because CFG validation precedes the S20-230 declaration lookup;
- unresolved declarations precede unresolved local requests and return
  `EFFECT_UNRESOLVED_ENTITY` (`23000`); and
- unequal resolved sets return `EFFECT_CLOSURE_MISMATCH` (`23003`).

Every loop index advances with checked arithmetic. Overflow maps to
`EFFECT_RESOURCE_LIMIT` (`23013`), while impossible `VectorGet(None)` paths
trap `InternalInvariant`. Success returns the closure size, zero call edges,
one closure round, and native-equivalent local closure work.

## Native parity and negative corpus

`native_effect_set_inventory` independently builds the corresponding effect
definitions, declared function set, and one `EffectRequest` operation per local
effect, then calls `validate_effect_program`. Valid cases cover empty, sparse,
and longer inventories and repeat execution for determinism. The negative
corpus covers unsorted and duplicate definitions, unsorted declarations,
unresolved declarations and requests, both closure-mismatch directions, and
multi-fault precedence.

All fourteen tests in `rw080_checker_scaffold` pass; focused Clippy with
warnings denied is clean.

## Explicit remainder

The local-closure input is the sorted/unique projection produced after request
scanning; this slice does not yet derive that projection from an arbitrary
operation vector. Multiple functions, direct-call propagation, wrong-kind
identities, effect-definition type validation, request typing, adapters,
capabilities, contracts, resource maxima, arbitrary decoded program objects,
and mandatory test planning remain RW-100 work. RW-080 and R2 stay provisional
pending the recorded independent acceptance debt.
