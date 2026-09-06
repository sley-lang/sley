# HOST_HYDRATION_V1 (RW-075 structural hydration contract)

Status: frozen with `EXEC_PACKAGE_V1` (RW-075, 2026-09-06). This document
is the explicit host-hydration contract: for every operation performed
after the execution package is supplied, it classifies the operation and
states the structural/semantic boundary. Anything not listed here requires
explicit review.

## Operation classification

Byte/framing decode: envelope magic/version/length framing;
`SLEYBC02` structural load (`load_image`, `check_image_prefix`);
section length-prefix framing; `TypeExpr`/`TypeDefinition`/`AdapterImport`
structural codecs in `sley_vm::exec_package` (tags, widths, counts carried
without judgment).

Digest verification: SHA-256 over image bytes (`image_digest`); SHA-256
over every section (`section_digest`); package-identity preimage hash;
receipt digest equality; cache-key re-derivation and equality; exact import
rows equality (`==`, full schemas).

Bounds checking: envelope/section ceilings (`EXEC_PACKAGE_*`,
`IMAGE_MAX_BYTES`); count bounds (`MAX_DEFINITIONS`, `MAX_CONSTANTS`,
`MAX_IMPORTS`); cursor overrun checks (refuse `Truncated` before trusting
any bound); value-unit and input-count caps; bridge/fuel ceilings as
before.

Allocation: runtime register file sized from the decoded image; cell table
under the existing caps; section buffers under the ceilings above.

Structural runtime-layout hydration:
`TypeEnvironment::hydrate_verified_definitions` (duplicate-identity plus
count-bound checks only); direct definition lookup by ID for RecordNew
field-count/member agreement (memory-safety shape alignment, never a
well-typedness verdict).

Register/frame setup: input-to-parameter register writes with structural
type equality (`value.value_type == register_type`); call-frame open with
the 256-frame ceiling and shared budgets.

Primitive value execution: the existing `execute_extended_instruction`
value semantics over already-admitted data (order/equality/arithmetic/
collection/cell/bridge mechanics as before, with RecordNew fields from the
structural hydration).

Control-flow execution: branch/cond-branch/switch/trap dispatch as before.

Resource accounting: per-instruction/fuel/value-unit charges,
`BRIDGE_ELEMENT_FUEL`, `raw_hash_fuel` schedule, value-unit caps,
`cancel_at_fuel` cooperative cancellation.

Cancellation: deterministic `Cancelled` termination at the fuel mark; no
live OS cancellation exists in-tree.

Output/result construction: `ConstValue` results typed from decoded
registers; package-bound observation (`SLEYPOBS1` plus every section
digest) via codec-plus-hash only.

## Structural versus semantic, precisely

Structural (host, memory-safety): duplicate identities; count/length
ceilings; cursor bounds; digest equality; `==` on types, rows, IDs;
canonical-codec framing (`encode_const_value` acceptance as an identity
property preventing one map from carrying two byte identities);
operand-count versus field-count agreement; member-ID presence in an
already-admitted record value; 256-frame ceiling.

Semantic (compiler, bound via receipt, never recomputed by the host on
this path): definition-shape validation; reference resolution; cycle
discovery; map-key orderability and hashability and persistability trait
determination; record-schema reconstruction; typechecking; constant
well-formedness beyond codec framing; lowering, fingerprint, gate, and
candidate judgments; dependency resolution answers.

## Anti-shortcut standing probes

`crates/sley-vm/tests/rw075_hydration_workloads.rs` pins: no
`TypeEnvironment::new` on the package path (cyclic bytes hydrate
structurally yet `new` refuses them); no semantic digest service (raw
bytes hashing never equals a semantic value hash); no checker/lowering/
candidate/dependency answers from the host (empty packages refuse; source
probes forbid the identifiers). `crates/sley-vm/src/exec_package.rs`
contains no semantic service call by construction (probe-enforced).
