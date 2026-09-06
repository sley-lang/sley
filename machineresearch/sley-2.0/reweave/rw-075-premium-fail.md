# R2 architecture review — FAIL (preserved negative evidence, RW-075 input)

Status: `R2_ARCHITECTURE_FAIL` (permanent negative evidence; never deleted,
never rewritten). This file preserves the independent premium architecture
review verdict that returned RW-080 to BLOCKED and opened RW-075. It is the
input to the RW-075 repair, not its output.

Review basis (as cited by the review assignment):

- HEAD `b4c3390088deed018f818eb1327f130002c27ab6` with the uncommitted
  RW-070 working state only (no unrelated deltas);
- reviewed tracked/nonignored content inventory digest
  `69cfb5a1f336d4d786f6e3ffaddefa86a700c681ad747904ab0cb688c85c595e`
  (inventory method not reproduced in-tree; the exact bytes are preserved
  by the RW-070 preservation commit `e85b89c04da5ab112a6b4b2e884f6e46f1a819fc`,
  whose parent is the reviewed HEAD; frozen digests re-verified:
  `BOOTSTRAP_PROFILE_1 4f269150...efd630`,
  `host-boundary d935d238...73bdb18a`, `HOST_ABI_V1 e6de00b8...04ecc2`);
- no premium review transcript file was located in-tree; the findings below
  are taken from the RW-075 assignment issued by the premium
  architecture-review role and verified against the cited production
  locations before any repair (assignment instruction: do not work from the
  summary alone — each finding was re-verified in code).

## Findings (all must be repaired in RW-075; RW-080 stays BLOCKED)

- AR-01 BLOCKER — native semantic preparation remains in loaded execution.
  Verified: `crates/sley-vm/src/execute.rs` `LoadedExecutionInput.types:
  &TypeEnvironment` plus `validate_loaded_inputs` ->
  `check_input_shape` -> `types.check_constant` plus
  `types.require_hashable`; `ExecutionSource` carries `&TypeEnvironment`
  into `execute_extended`/`derive_observation_id`/`encode_termination`;
  `TypeEnvironment::new` performs definition-shape judgment, reference
  checks, cycle discovery, and map-key rules
  (`crates/sley-check/src/lib.rs`). The loaded path reruns compiler work.
- AR-02 BLOCKER — callable hashing does not cover required compiler
  identity domains. Verified: `sley-ssmc/src/fingerprint.rs`
  (`fingerprint_type_definition`, `fingerprint_function`,
  `hash_validated_value` with `SLEYSFP1`/`SLEYVHS1` framing),
  `sley-id/src/lib.rs` BLAKE3 domains (`sley2.value-hash.v1`,
  `sley2.semantic-fingerprint.v1`, `sley2.vm-bytecode-cache-key.v1`,
  `sley2.observation.v1`, entity/object/root/transaction/candidate
  domains), `host_abi::image_digest` SHA-256, `derive_cache_key`,
  `derive_observation_id`. No narrow raw primitive over Sley-built
  preimages exists; any native semantic digest service would conceal
  compiler semantics.
- AR-03 HIGH — ApprovedImage does not bind the complete execution
  closure/admission evidence. Verified: `execute.rs` `ApprovedImage {
  digest, cache_key, imports: Vec<EntityId> }` — import IDs only (no row
  schemas), no constants/layouts digests, no profile/epoch/VM/ABI/limits/
  entry/package bindings, no admission receipt. Cache-key preimage
  explicitly excludes image/profile/limits/import-manifest digests
  (`lib.rs` `cache_key_preimage`, `HOST_ABI_V1.md` bindings section).
- AR-04 HIGH — RW-080 bootstrap boundary is insufficiently concrete.
  Verified: `rw-070-r2-handoff.md` section 11 proposes the R3 boundary in
  one paragraph (S ownership, ABI read-only, seed availability) without
  per-module entry points, canonical types, capability lists, dependency
  maps, error contracts, scaffold-vs-algorithm splits, oracle comparisons,
  S definition, construction-manifest provenance, C0 seed-only rules, or
  anti-copy evidence. RW-080 cannot start from it.
- AR-05 MEDIUM — compiler-scale composition/capacity remains unproved.
  Verified: bootstrap closure vectors are small single-function fixtures
  (20 vectors; maxima 34 instructions / 124 fuel per RW-050 record);
  no branching fan-out greater than one, no real image emission from
  supplied structures, no mixed lookup/traversal/error/emission/hashing
  workload, no early-R3 budget, no 1 MiB vs 64 MiB / PSH1 / quadratic-clone
  evaluation.
- AR-06 MEDIUM — staged READY is not the complete aggregate R2 exit gate.
  Verified: `scripts/check_bootstrap_capability.py` audits capability/
  profile readiness (PASS / R2 READY, zero open blockers) over P plus
  staged corpus only; it consumes no RW-060 lifecycle identity, no ABI
  successor acceptance, no closure completeness, no architecture-BLOCKER
  state, and no review verdict. Relabeling it as the R2 exit would overclaim.

## Disposition

`R2_ARCHITECTURE_FAIL`. RW-080 remains BLOCKED. RW-075 repairs AR-01
through AR-06 as a narrower corrective subpackage depending on RW-070.
The historical `BOOTSTRAP_READY TRUE` (RW-070) is preserved but marked
superseded/invalidated for R3 advancement by this FAIL. A fresh aggregate
R2 decision after repairs plus independent review plus premium delta
re-review alone may authorize RW-080.
