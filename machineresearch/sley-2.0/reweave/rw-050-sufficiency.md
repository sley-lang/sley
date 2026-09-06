# RW-050 self-hosting sufficiency check (slice 2, 2026-09-06)

Scope: bounded paper/executable check that `BOOTSTRAP_PROFILE_1` can
represent the algorithmic building blocks of every REWEAVE §10.2
toolchain responsibility. This is NOT permission to implement the
compiler, and no compiler code is written here. Method: each
responsibility's required computations are listed, each is mapped to
frozen profile mechanics with executable workload pointers, and the
eleven §5 questions are answered against landed semantics. A missing
capability would route through feature admission and stop the freeze;
none was found, and no convenience primitive was added.

Conventions: `W:` = closure vector in
`conformance/bootstrap-profile/v1/accepted.json` (all gate-admitted
pre-emission); `E:` = vm-extended vector; `L:` = adversarial lane
(repo-native `bridge_adversarial` / `pure_declaration_adversarial`, or
the `vm_canonical_inputs` E8 lane).

## 10.2 item 1 — canonical program/schema-aware SCB decode and encode
with strict rejection and bounded traversal

- Byte inspection: `B2V1` maps request bytes to `UInt(8)` elements in
  order (`W: bytes-round-trip`, `W: image-assemble-emit-n3/n0`).
- Byte construction: `V2B1` maps octet vectors back (`W:` same).
- Length-prefixed/tagged traversal: bounded block-parameter loops
  (`W: vector-push-loop-n5/n0`, `W: graph-worklist-dfs-chain/cycle`),
  length/index arithmetic in E2 widths, order/equality branching (E1).
- Strict rejection as values: `Result`/`Option` failures deconstructed
  through `VariantSwitch` (`W: variant-switch-exhaustive-*`,
  `W: checked-length-traverse-hit/miss`); hard aborts trap
  (`Trap(InternalInvariant)`, in-crate `trap_on_violation`).
- Losslessness: decode/re-encode identity pinned at small and cap
  scale (`L:` round-trip chains, `bridge_capacity_boundary` chain test).

## 10.2 item 2 — SSMC formation, reference/identity checks, type and CFG
checking, effect/contract judgments, mandatory test planning

- Graph formation: record/variant/tuple/vector/map construction
  (`W: record-variant-walk`, `W: map-symbol-table-*`, `W:
  value-hash-chain`; `E: tuple-project`, `E: map-new-sorted`).
- Reference/identity checks: entity identities cross as bytes and are
  compared as values (`W: bytes-round-trip` + E1 equality/order over
  `Bytes`); symbol tables keyed by `Text`/`UInt` probe in bounded time
  (`W: map-symbol-table-*`).
- Type/CFG-shaped judgments: branching on closed-variant structure
  (`W: variant-switch-exhaustive-*`), bounded iteration to fixpoint
  (`W: graph-worklist-dfs-*`), checked arithmetic with failure values
  (`W: multi-function-pass-overflow`).
- Effect/contract judgments as data processing: pure closed functions
  over canonical values (the gate enforces effect-free/contract-free
  toolchain functions); assertion outcomes are `Result` values, never
  hidden authority (`L:` masquerade tests).
- Test planning (deterministic plan construction/selection, G-2 note):
  plan shapes are record/vector/map data built and compared with E1/E4
  ops; no test execution or observation primitive is needed or admitted.

## 10.2 item 3 — Witness rules, integrity analysis, admissibility, sink
integrity after WA integration

- Data/control-flow integrity analysis is graph reachability over maps
  and vectors: iterative worklists with visited sets
  (`W: graph-worklist-dfs-chain` for DAG closure, `-cycle` for
  visited-set termination on cyclic input).
- Admissibility/sink checks are predicate-shaped: boolean combinations,
  equality, and membership probes with trap or `Result` outcomes
  (`W: set-as-map-absent-after-remove`, `W: checked-length-traverse-*`).

## 10.2 item 4 — SSMC-to-executable lowering and deterministic image
assembly

- Lowering-shaped traversal: multi-block CFG with threaded state,
  multi-function structure with error propagation
  (`W: multi-function-pass-ok/overflow`).
- Image assembly: element-wise byte emission to a grown vector crossed
  back to bytes (`W: image-assemble-emit-n3/n0`); capacity behavior is
  the frozen `Index(2)` value discipline (`L:` cap-boundary tests).

## 10.2 item 5 — build driver (closure resolution, checker/lowerer
invocation, output assembly, self-reproduction)

- Closure resolution: iterative reachability with an explicit work
  vector and visited set (`W: graph-worklist-dfs-*`); membership sets
  as maps-to-`Unit` (`W: set-as-map-absent-after-remove`).
- Checker/lowerer invocation: multi-function calls with typed results
  (`W: multi-function-pass-*`).
- Output assembly and deterministic identity: byte emission plus
  content hashing at the native boundary (`W: value-hash-chain`).

## 10.2 item 6 — deterministic discharge profiles belonging to the
language

- Value-level profile logic runs as ordinary pure functions
  (`W: multi-function-pass-*` pattern); cryptographic primitives stay
  explicit bounded native dependencies (`value_hash` at the E5
  boundary, `W: value-hash-chain`), never hidden trust transitions.

## Eleven questions

1. Bounded iteration/traversal over canonical structures: YES — Form A
   backedge loops with block-parameter state (`W: vector-push-loop-*`,
   `W: graph-worklist-dfs-*`, `W: image-assemble-emit-*`); cells
   admitted as the alternate mechanism (`E: cond-drain-loop`).
2. Branching and structured failure: YES — `CondBranch`/`VariantSwitch`
   with `Result`/`Option` values plus explicit `Trap` (eleven switch
   sites across the closure set; `W: variant-switch-exhaustive-*`).
3. Construction of new canonical values: YES — tuples, records,
   variants, vectors (growth via `PSH1`), maps, bytes (`W:
   record-variant-walk`, `W: map-symbol-table-*`, `W:
   image-assemble-emit-*`).
4. Byte inspection/construction: YES — the admitted bridge
   (`W: bytes-round-trip`, `L:` E8 lanes).
5. Symbol/entity lookup: YES — maps keyed by `Text`/`UInt` (all-traits
   key types); entity-id bytes cross the bridge for byte-keyed tables
   (`W: map-symbol-table-*`, `W: bytes-round-trip`).
6. Deterministic maps/sets or equivalent: YES — `OrderedMap` with
   canonical-byte key order plus sets-as-maps-to-`Unit` (`W:
   map-symbol-table-*`, `W: set-as-map-absent-after-remove`, `W:
   graph-worklist-dfs-*` visited sets).
7. Recursive/iterative graph traversal: YES iteratively — explicit
   stack/state vectors with visited sets handle arbitrary depth, which
   the 256-frame ceiling could not as the sole mechanism (`W:
   graph-worklist-dfs-chain/cycle`).
8. Work queues/stacks: YES — vectors as stacks with length-derived
   indices, no arithmetic needed (`W: image-assemble-emit-*`,
   `W: graph-worklist-dfs-*` trails).
9. Canonical output assembly: YES — element-wise emission plus
   byte-crossing (`W: image-assemble-emit-n3/n0`).
10. Error/result propagation: YES — `Result` values across calls and
    blocks with default/fallback arms (`W: multi-function-pass-ok/
    overflow`, `W: checked-length-traverse-miss`).
11. Deterministic hashing at the permitted native boundary: YES —
    `value_hash` (`hash_validated_value` under the frozen epoch) with
    same-input determinism pinned (`W: value-hash-chain`).

## Result

No missing capability was discovered. Nothing was added for
convenience: floats, contract assertions, globals, function references,
generic specialization, recursion, test/effect/capability ops, live
cancellation, and persistent reports stay excluded with per-item
rationale in the freeze record. The profile expresses nontrivial
compiler algorithms (traversal with visited sets, error-propagating
multi-function passes, byte-level decode/emit chains), not merely the
existing bootstrap test vectors.
