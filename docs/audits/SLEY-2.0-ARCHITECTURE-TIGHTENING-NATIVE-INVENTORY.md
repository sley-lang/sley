# Sley 2.0 Native / Sley Authority Inventory (architecture-tightening successor record)

Status: campaign deliverable 10 of `SLEY-2.0-ARCHITECTURE-TIGHTENING.md`
(sections 6 and 13), recorded at baseline commit
`560a5f16ebe9edaaee6837b779f6af94ad9ae310`. Successor by reference of
`machineresearch/sley-2.0/reweave/rw-070-host-inventory.json`
(`RW-070-HOST-INVENTORY-1`, base commit `b4c3390`), which stays byte-identical
as RW-070 history; section 4 lists exactly where it is stale. Finding
`AT-NA-01` in `SLEY-2.0-ARCHITECTURE-TIGHTENING-AUDIT.md` owns this record.

Every native operation participating in compilation, lowering, package
construction, loading, validation, execution, bootstrap admission, and
self-host succession carries exactly one class, and every
`SEMANTIC_AUTHORITY` row carries its authority decision with the clause that
assigns it. Checkers run at the baseline: `check_host_abi_{v1,v2,markers}`,
`check_exec_package_{v2,markers}`, `check_bootstrap_profile_2`,
`check_bootstrap_gate_markers` all PASS; `check_r2_exit` reads `NOT_READY`.

## 0. Governing facts established from repository truth

- Current self-host claim level: **SH0_BOOTSTRAP_ONLY**. `machineresearch/sley-2.0/reweave/bootstrap-manifest.json`
  records `S` "does not exist (SH0: no Sley toolchain program)", `C1`/`C2`/`C3` "does not exist",
  `C0` "no preserved image snapshotted yet". No record in-tree claims SH1 or SH2.
- Provisional Sley toolchain modules exist under the operator development override of 2026-09-07
  (`machine-summary.json:3462`): RW-080 codec slices 1 through 7 (`crates/sley-vm/tests/rw080_codec_*.rs`,
  49 tests; records `rw-080-codec-*.md`). Every record states "not accepted runtime authority",
  `rw080` BLOCKED, R2 NOT_READY, no C1/SH claim. These are seed-assembled fixture-namespace graphs
  admitted through the declared C0 seed route, not a canonical `S` root.
- The 2.0 GA path has no self-hosting requirement (`Sley2.0mastergoal.md:1907-1919`, section 14.5:
  "The Rust implementation is the bootstrap and trusted reference"). SH2 is the REWEAVE campaign
  outcome adopted by `docs/adr/ADR-0049-reweave-scope-adoption.md` (naming proposal 2.1.0, C-03).
  REWEAVE section 5 states the ownership transition: before promotion the Rust reference owns
  active checking/lowering; after promotion the native host enforces execution containment only.
- Consequence for spec section 6: the native semantic compiler chain is
  `REQUIRED_NATIVE_FOUNDATION` for Sley 2.0 (C0 / SH0) and `MUST_MIGRATE_TO_SLEY` for the SH2
  campaign line (REWEAVE 10.2 items 1 through 6, already chartered as RW-090 through RW-180 in
  `host-boundary.json` `sley_owned`). No native operation was found that both (a) is reachable
  from the frozen toolchain import registry and (b) decides language semantics. See section 2.

## 1. Native authority inventory

Stage codes: COMP = compilation/checking, LOW = lowering, PKG = package construction,
LOAD = loading, VAL = validation, EXEC = execution, BOOT = bootstrap admission,
SUCC = self-host succession, REPO = repository/lifecycle outside the compiler closure.
Class is exactly one of TRANSPORT, STRUCTURAL_VALIDATION, RESOURCE_ENFORCEMENT,
EXECUTION_MECHANICS, SEMANTIC_AUTHORITY. Decision applies only to SEMANTIC_AUTHORITY rows:
RNF = REQUIRED_NATIVE_FOUNDATION (with the scope it holds at), MMS = MUST_MIGRATE_TO_SLEY
(with the chartered package).

| # | Operation | Crate / file:line | Stage | Class | Authority decision | Proof clause |
|---|---|---|---|---|---|---|
| 1 | `TypeEnvironment::new` (shape, reference, cycle, map-key judgment) | `crates/sley-check/src/lib.rs:218-238` | COMP | SEMANTIC_AUTHORITY | RNF at C0/SH0; MMS at SH2 (RW-100) | Master 14.5; REWEAVE 5 ("before promotion: Rust reference owns active checking/lowering"); 10.1 SH0 row; `host-boundary.json` `sley_owned[1]` names `sley-check` as reference seed |
| 2 | `check_constant`, `require_hashable`, `require_orderable`, `traits`, `instantiate` | `crates/sley-check/src/lib.rs:302-402` | COMP | SEMANTIC_AUTHORITY | RNF at C0/SH0; MMS (RW-100) | same as row 1 |
| 3 | `validate_function_graph` (S20-220 CFG) | `crates/sley-check/src/cfg.rs:231` | COMP | SEMANTIC_AUTHORITY | RNF at C0/SH0; MMS (RW-100) | REWEAVE 10.2 item 2 lists "type and CFG checking" as Sley-owned at SH2; row-1 proof for C0 |
| 4 | `validate_effect_program` (S20-230) | `crates/sley-check/src/effects.rs:261` | COMP | SEMANTIC_AUTHORITY | RNF at C0/SH0; MMS (RW-100) | REWEAVE 10.2 item 2 "effect/contract judgments" |
| 5 | `validate_contract_test_program` (S20-240 test plan) | `crates/sley-check/src/contracts.rs:257` | COMP | SEMANTIC_AUTHORITY | RNF at C0/SH0; MMS (RW-100) | REWEAVE 10.2 item 2 "mandatory test planning" |
| 6 | `fingerprint_function`, `fingerprint_type_definition`, `verify_fingerprint_claim` (SLEYSFP1 canonical projection then BLAKE3) | `crates/sley-ssmc/src/fingerprint.rs:135,177,251` | COMP | SEMANTIC_AUTHORITY | RNF at C0/SH0; MMS (RW-100; preimage by Sley, hash via RHW1) | `rw-075-hash-inventory.md` obligation 1 ("preimage construction: semantic/compiler-owned"); `RAW_HASH_V1.md` "What this is not" |
| 7a | `hash_validated_value` as the `value_hash` opcode 178 over runtime values | `crates/sley-ssmc/src/fingerprint.rs:282`; `crates/sley-vm/src/extended.rs:1930` | EXEC | EXECUTION_MECHANICS | n/a (pinned opcode semantics) | REWEAVE 10.3 bullet 2 "primitive value operations"; `VM_EXTENDED_OPCODE_PROFILE_V1.md` E5; `BOOTSTRAP_PROFILE_2.md` "E5 cells and value hashing"; rw-070 inventory row "whole-value primitive ops (order, equality, value_hash)". See AT-NA-07 for the doc-precision residual |
| 7b | `hash_validated_value` used by the checker/candidate path to identify compiler objects (map-key identity, constants, inputs) | `crates/sley-policy/src/candidate_validation.rs` (via `program.validate_restricted_type_fingerprint_claims`, :780-800) | COMP | SEMANTIC_AUTHORITY | RNF at C0/SH0; MMS (RW-100) | `rw-075-hash-inventory.md` obligation 2 |
| 8 | Candidate validation pipeline `validate_candidate_bytes` (phases 6/7: rows 1-5, 11) | `crates/sley-policy/src/candidate_validation.rs:809,833,886,955`; called from `crates/sley-txn/src/repository.rs:1885` | REPO/COMP | SEMANTIC_AUTHORITY | RNF at C0/SH0 (production repository); MMS for the toolchain closure only (RW-100/RW-120) | REWEAVE 10.3 bullet 7 "repository mechanisms outside the designated compiler closure, where their ownership and delegation remain explicit"; `host-boundary.json` `native_remainder[7]`; `conformance/host-abi/v2/host-abi.json` `semantic_authority_boundary` names candidate validation Sley-owned under SH2 |
| 9 | SCB1 codec: `sley-scb1` encoders/decoders; `sley-mutate` `encode_const_value`/`decode_const_value` | `crates/sley-scb1/src/lib.rs:186-756`; `crates/sley-mutate/src/codec.rs:66,76` | COMP | SEMANTIC_AUTHORITY | RNF at C0/SH0 (reference seed and oracle); MMS (RW-090, provisional slices 1-7 landed under override) | REWEAVE 10.2 item 1; `host-boundary.json` `sley_owned[0]`; `rw-080-contract.md` section 1.1 |
| 10 | Schema epoch canonical bytes, `validate`, `bootstrap_preimage`, registry migration checks | `crates/sley-schema/src/lib.rs:360,393,405,446,642,684` | COMP | SEMANTIC_AUTHORITY | RNF at C0/SH0; MMS (RW-090 schema legs) | REWEAVE 10.2 item 1 "program/schema-aware SCB decoding and encoding" |
| 11 | `judge_function_operations` (S20-260 judgment without emission) | `crates/sley-vm/src/lower.rs:258` | LOW | SEMANTIC_AUTHORITY | RNF at C0/SH0; MMS (RW-110) | REWEAVE 10.2 item 4; `host-boundary.json` `sley_owned[3]` |
| 12 | `lower_function`: CFG validation, register allocation (`Maps::build`), `judge_extended`, `emit_function`, `lower_callees` | `crates/sley-vm/src/lower.rs:290-352,421,614-703` | LOW | SEMANTIC_AUTHORITY | RNF at C0/SH0; MMS (RW-110) | REWEAVE 10.2 item 4 "SSMC-to-executable lowering"; 13.1 "C1 = C0.build(S, P)" |
| 13 | `judge_extended_operation` (opcode signature and result-type derivation; consults `traits`/`require_hashable`) | `crates/sley-vm/src/extended.rs:795,765,1096,1123,1223` | LOW | SEMANTIC_AUTHORITY | RNF at C0/SH0; MMS (RW-110) | same as row 12 |
| 14 | `require_canonical_referenced_constants` | `crates/sley-vm/src/extended.rs:686` | LOW | SEMANTIC_AUTHORITY | RNF at C0/SH0; MMS (RW-110) | same as row 12 |
| 15 | `encode_function`/`encode_body` (SLEYBC02 byte emission, layout/order/tag decisions) | `crates/sley-vm/src/lower.rs:838,857` | LOW/PKG | SEMANTIC_AUTHORITY | RNF at C0/SH0 (seed reference); MMS (RW-110 image builder) | REWEAVE 10.2 item 4 "deterministic assembly of the declared derived execution image"; `rw-075-correction.md` section 6 ("`lower::encode_function` is seed-reference outside the clean closure"); premium r1 log :144 |
| 16 | `cache_key_preimage`/`derive_cache_key` (SLEYBCK1, 12 frozen fields) | `crates/sley-vm/src/lib.rs:202,239` | LOW/VAL | EXECUTION_MECHANICS | n/a | `rw-075-hash-inventory.md` obligation 5 ("concatenation-plus-hash may remain host mechanic for the reference path; the Sley driver reconstructs the identical preimage bytes"); `HOST_ABI_V2.md` Bindings |
| 17 | `resolve_bridge_entry`, `is_push_row`, `resolve_push_row` (positive import manifest, default deny, four rows) | `crates/sley-vm/src/extended.rs:304-391` | LOW/BOOT/EXEC | STRUCTURAL_VALIDATION | n/a | REWEAVE 11 "a positive import manifest and tests denying compiler-service imports"; `HOST_ABI_V2.md` "Permitted native imports (exact, default-deny; 4 rows)" |
| 18 | `bridge_fuel_surcharge`, `BRIDGE_ELEMENT_FUEL`, `raw_hash_fuel` | `crates/sley-vm/src/extended.rs:430`; `raw_hash.rs:110` | EXEC | RESOURCE_ENFORCEMENT | n/a | REWEAVE 10.3 bullet 2 "resource accounting" |
| 19 | `judge_bootstrap_profile` (profile membership, acyclic calls, type-form membership, referenced-row closure, V2 preimage bound, closure fingerprints, presented-image digest commit) | `crates/sley-vm/src/bootstrap.rs:320-460` | BOOT | SEMANTIC_AUTHORITY | RNF at C0 only (seed leg); post-C1 replaced by Sley-produced evidence | `rw-080-contract.md` section 1.4 ("C0 seed (now, before C1 exists): the native staged authority performs judgment plus reference re-lowering as seed/oracle evidence ... confined to C0 and excluded from clean stages"); REWEAVE 13.1 |
| 20 | `admit_v2_package` semantic legs `judge_closure_for_seed`, `reference_lower_for_seed` | `crates/sley-vm/src/admission_authority.rs:157-231` | BOOT/SUCC | SEMANTIC_AUTHORITY | RNF at C0 only; exclusive v2 minter today; excluded from clean stages after C1 (declared, not yet code-verifiable) | `admission_authority.rs:1-48` module contract; `rw-080-contract.md` 1.4 and 1.6; `scripts/check_exec_package_markers.py:95-137` exclusivity pins |
| 21 | `verify_structural_correspondence` (byte equality, gate-claim binding, complete table equality, image digest) | `crates/sley-vm/src/admission_authority.rs:239-289` | BOOT/VAL | STRUCTURAL_VALIDATION | n/a | `rw-080-contract.md` 1.4 "authenticated/bound structural verification only" |
| 22 | `admit_package_v2` (pub(crate) receipt constructor), `AdmissionReceipt` sealed | `crates/sley-vm/src/exec_package.rs:950-960,284-312` | BOOT | STRUCTURAL_VALIDATION | n/a | REWEAVE 11 "bind a verdict to the exact approved toolchain image ... A caller-supplied VALID field is not a verdict"; `EXEC_PACKAGE_V2.md` Rust surface |
| 23 | `approve_package_v2` (receipt/profile/ABI/version/gate-report consistency, cache key) | `crates/sley-vm/src/exec_package.rs:1079-1165` | BOOT/VAL | STRUCTURAL_VALIDATION | n/a | `EXEC_PACKAGE_V2.md` Bindings ("verified without running the gate") |
| 24 | `admit_v2_package_from_sley_evidence` + sealed `SleyAdmissionEvidence` (always refuses) | `crates/sley-vm/src/admission_authority.rs:296-325` | SUCC | STRUCTURAL_VALIDATION | n/a (reserved ingress, no minting path) | `rw-080-contract.md` 1.6 "Admission ingress" |
| 25 | Package section framing codecs `encode_constants_section`, `encode_layouts_section`, `encode_imports_section`, `encode_dependency_section` | `crates/sley-vm/src/exec_package.rs:608,635,664,697` | PKG | TRANSPORT | n/a | `HOST_HYDRATION_V1.md` "Byte/framing decode ... structural codecs (tags, widths, counts carried without judgment)"; premium r1 log :144 ("section encoders frame carried representation data and do not lower graphs") |
| 26 | `section_digest`, `package_digests_v2`, `imports_manifest_digest`, `image_digest` (SHA-256 over exact bytes) | `crates/sley-vm/src/exec_package.rs:197,873,1319`; `host_abi.rs:210` | PKG/VAL | STRUCTURAL_VALIDATION | n/a | `rw-075-hash-inventory.md` obligation 4 ("host mechanic, not language semantics"); REWEAVE 10.3 bullet 3 |
| 27 | `check_image_prefix` | `crates/sley-vm/src/host_abi.rs:170` | LOAD | STRUCTURAL_VALIDATION | n/a | REWEAVE 10.3 bullet 4; 10.4 "A native bytecode verifier may reject malformed execution images for memory safety"; `host-boundary.json` `distinctions_10_4.verifier_vs_checker` |
| 28 | `load_image` + bounds-checked `Cursor` (entry body, callee table, `MAX_TYPE_DEPTH`) | `crates/sley-vm/src/host_abi.rs:234-560` | LOAD | STRUCTURAL_VALIDATION | n/a | same as row 27; `HOST_ABI_V2.md` "Executable image boundary (unchanged)" |
| 29 | `hydrate_layouts` -> `TypeEnvironment::hydrate_verified_definitions` (duplicate identity + count bound only) | `crates/sley-vm/src/exec_package.rs:1167`; `crates/sley-check/src/lib.rs:267-278` | LOAD | STRUCTURAL_VALIDATION | n/a | `HOST_HYDRATION_V1.md` "Structural runtime-layout hydration"; probe `rw075_hydration_workloads.rs` (cyclic bytes hydrate structurally, `new` refuses them) |
| 30 | `verify_package_binding_v2` | `crates/sley-vm/src/exec_package.rs:1253-1317` | VAL | STRUCTURAL_VALIDATION | n/a | `EXEC_PACKAGE_V2.md` "Approved binding" |
| 31 | `execute_approved_package_v2` (verification order, then structural runner) | `crates/sley-vm/src/execute.rs:707-772` | EXEC | EXECUTION_MECHANICS | n/a | REWEAVE 10.3 bullet 1; `execute.rs:600-630` contract comment ("Semantic validation ... is NOT rerun here") |
| 32 | `validate_package_inputs_structural` (structural type equality, `require_canonical_form` via `sley_mutate::encode_const_value`, value units, `hash_validated_value`) | `crates/sley-vm/src/execute.rs:783-813,1808` | VAL | STRUCTURAL_VALIDATION | n/a | `HOST_HYDRATION_V1.md` "canonical-codec framing (`encode_const_value` acceptance as an identity property ...)" and "Structural versus semantic, precisely" |
| 33 | Runner: `execute_core_package`, `run`, `dispatch_terminator`, `prepare_frame`, `return_to_caller`, `unwind` | `crates/sley-vm/src/execute.rs:815,1202,1320,1392,1517,1834` | EXEC | EXECUTION_MECHANICS | n/a | REWEAVE 10.3 bullet 1 |
| 34 | `execute_extended_instruction` (opcode value semantics; RecordNew operand-count vs field-count structural agreement) | `crates/sley-vm/src/extended.rs:1753` | EXEC | EXECUTION_MECHANICS | n/a | REWEAVE 10.3 bullets 1-2; `HOST_HYDRATION_V1.md` "Primitive value execution" |
| 35 | Bridge execution arms B2V1, V2B1, PSH1, RHW1 | `crates/sley-vm/src/extended.rs:560-615` | EXEC | EXECUTION_MECHANICS | n/a | REWEAVE 10.3 bullets 2-3; `host-boundary.json` `bridge_g10.must_not`; `HOST_ABI_V2.md` 4 rows |
| 36 | `raw_blake3_256`, `raw_hash_variant` (`blake3 =1.8.2`, no domain added) | `crates/sley-vm/src/raw_hash.rs:124,142` | EXEC | EXECUTION_MECHANICS | n/a | REWEAVE 10.3 bullet 3; 10.4 "Do not hand-roll cryptography in Sley"; `RAW_HASH_V1.md` |
| 37 | Fuel, cells, frames, value units, input caps, `cancel_at_fuel`: `charge_action`, `charge_value`, `ExecutionLimits`, `MAX_*` | `crates/sley-vm/src/execute.rs:23-66,2007,2030`; `lower.rs:24-34` | EXEC/LOW | RESOURCE_ENFORCEMENT | n/a | REWEAVE 10.3 bullet 2 "memory allocation, resource accounting ... cancellation" |
| 38 | Observation identity `observation_preimage_package`/`finish_package` (SLEYPOBS1) and legacy `derive_observation_id` (SLEYOBS1) | `crates/sley-vm/src/execute.rs:952,1000,2153` | EXEC | EXECUTION_MECHANICS | n/a | `rw-075-hash-inventory.md` obligation 6 ("host derives (mechanic); the Sley driver rebuilds the same bytes to verify evidence") |
| 39 | Legacy `execute_function` (re-lowers via row 12; `validate_inputs` -> `check_input_shape` calls `check_constant`/`require_hashable`) | `crates/sley-vm/src/execute.rs:494,1717,1784-1790` | EXEC | SEMANTIC_AUTHORITY | RNF at SH0 reference runtime only; not reachable from the toolchain registry; superseded for new executions | `rw-075.md` section 1 ("legacy `execute_loaded_image`/`ApprovedImage` path is preserved unchanged ... marked superseded"); REWEAVE 5 "before promotion" |
| 40 | Legacy `execute_loaded_image` (structural load, then S20-270 input checks with `require_hashable` in observation preimage :2248) | `crates/sley-vm/src/execute.rs:529,1752,2248` | EXEC | SEMANTIC_AUTHORITY | RNF at SH0 reference runtime only; superseded | same as row 39 |
| 41 | `sley-id` domain-separated identity derivation (entity/object/root/transaction/candidate domains) | `crates/sley-id/src/lib.rs:338-409` | COMP/REPO | SEMANTIC_AUTHORITY | RNF at C0/SH0; MMS for toolchain-constructed objects (Sley builds the full preimage, hashes via RHW1) | `rw-075-hash-inventory.md` obligation 7; `RAW_HASH_V1.md` "including `sley-id` domain prefixes ... constructed by Sley as bytes" |
| 42 | Persistence and lifecycle: `sley-store`, `sley-txn`, `sley-state-root`, `sley-repo` (refs CAS, packs, locks), `sley-query`, `sley-mutate` (mutation protocol), `sley-policy` (tokens) | crate roots under `crates/` | REPO | TRANSPORT | n/a (row 8 carries the semantic part) | REWEAVE 10.3 bullets 5 and 7; `host-boundary.json` `native_remainder[5],[7]`; rw-070 inventory DEVELOPMENT_ONLY row |
| 43 | Surfaces: `sley-protocol` (calls `TypeEnvironment::new`/`execute_function` at `server.rs:2035,2065`), `sley-adapter`, `sley-cli`, `sley-json-bridge` | crate roots under `crates/` | REPO | TRANSPORT | n/a | REWEAVE 10.3 bullet 6; `rw-060.md` (unreachable from the lifecycle production graph, cited by rw-070 inventory) |
| 44 | C0 seed assembler and test drivers: `bootstrap_closure.rs`, `bridge_adversarial.rs`, `extended_tests.rs` (all `#[cfg(test)]`, `lib.rs:9-18`), `tests/rw070_*`, `rw075_*`, `rw080_*` (e.g. `admit()` at `rw080_codec_program_outer.rs:25153-25218` calls `lower_function`, `judge_bootstrap_profile`, `admit_v2_package`), `scripts/generate_bootstrap_profile_fixtures.py`, `sley-conformance` (`lib.rs:1187,1363,1395`), `conformance/`, `oracle/` | as listed | SUCC/BOOT | SEMANTIC_AUTHORITY | RNF at C0 (the seed build and its oracles), permanently outside the clean self-build closure | REWEAVE 13.1 ("A native script may stage inputs and compare digests; it may not compile, patch, normalize, or repair outputs" applies after C1); 10.3 bullet 8; `rw-080-contract.md` section 4 "C0's allowed seed-only work" |
| 45 | Cryptographic supply: `sha2 0.11` (image/package identity), `blake3 =1.8.2` (sley-id, RHW1) | `crates/sley-vm/Cargo.toml:13-14` | PKG/EXEC | EXECUTION_MECHANICS | n/a | REWEAVE 10.3 bullet 3 "audited cryptographic libraries"; `BOOTSTRAP_PROFILE_2.md` "Dependency closure" |
| 46 | Boundary checkers and gates: `check_host_abi_{v1,v2,markers}.py`, `check_exec_package_{v1,v2,markers}.py`, `check_bootstrap_profile_{1,2}.py`, `check_bootstrap_gate_markers.py`, `check_bootstrap_capability.py`, `check_r2_exit.py`, `build_anti_goal_conformance.py` | `scripts/` | SUCC/VAL | STRUCTURAL_VALIDATION | n/a | REWEAVE 21 validation targets; `host-boundary.json` `gate_set`; hygiene scan `check_host_abi_markers.py:173-207` |

Counts per class (46 numbered rows, 47 classified entries because row 7 is split into 7a and 7b):
SEMANTIC_AUTHORITY 21 (rows 1-6, 7b, 8-15, 19, 20, 39, 40, 41, 44);
STRUCTURAL_VALIDATION 12 (rows 17, 21, 22, 23, 24, 26, 27, 28, 29, 30, 32, 46);
EXECUTION_MECHANICS 9 (rows 7a, 16, 31, 33, 34, 35, 36, 38, 45);
RESOURCE_ENFORCEMENT 2 (rows 18, 37); TRANSPORT 3 (rows 25, 42, 43).

Every SEMANTIC_AUTHORITY entry resolves to REQUIRED_NATIVE_FOUNDATION at C0/SH0 with a cited
clause, and, where REWEAVE 10.2 lists the responsibility, to MUST_MIGRATE_TO_SLEY under an already
chartered package (RW-090, RW-100, RW-110, RW-120). No SEMANTIC_AUTHORITY entry is reachable from
the frozen import registry (row 17) or from the package execution path (rows 27-38); this is
pinned mechanically by `check_exec_package_markers.py` (`exec-rs-forbidden`, `hash-rs-forbidden`,
`minter-exclusivity`) and `check_host_abi_markers.py` (hygiene scan), both PASS at this commit.

## 2. Forbidden SH2 pattern checks (spec section 6)

| Pattern | Result | Evidence |
|---|---|---|
| Sley wrapper calling the native semantic compiler | ABSENT (structurally impossible today) | The only Sley-callable native entries are B2V1/V2B1/PSH1/RHW1 (`extended.rs:304-368`); unknown identities refuse `VM_LOWER_OPCODE_UNSUPPORTED`; eight compiler-service spellings denied (`rw070_native_compiler_services_are_not_admitted`); no `.sley` source, no Sley program in-tree names a compile/typecheck/lower service. The provisional RW-080 codec functions call only those four rows (`rw080_codec_envelope.rs:487-2330` immediates) |
| Native code constructing compiler answers before Sley receives them | ABSENT in the boundary; PRESENT ONLY as declared C0 seed/oracle | `admit_v2_package` runs the native gate and reference lowerer (rows 19-20), but only to compare against a candidate image already produced; it never hands Sley a verdict, a lowering, or a package (`admission_authority.rs:1-48`). RW-080 tests use `sley_scb1` only as an oracle to build expected bytes and compare (`rw080_codec_program_outer.rs:17-21,2430`; no `fallback`/`unwrap_or_else`/`or_else` anywhere in `rw080_codec_*.rs`). `rw075_hydration_workloads.rs` pins "empty packages refuse (no synthesized checker/lowering/image/candidate/dependency answers)" |
| Hidden native preparation of type/layout/runtime inventories the self-host compiler should own | ABSENT on the package path | Package path hydrates structurally only (`hydrate_verified_definitions`, `lib.rs:267`; `exec_package.rs:1-60` contract); `HOST_HYDRATION_V1.md` classifies every post-package operation; probe `rw075_hydration_workloads.rs` proves cyclic definitions hydrate while `TypeEnvironment::new` refuses them. Inventories are carried as digest-bound package sections supplied by the builder, not reconstructed (premium r1 log :55 confirms) |
| Native fallback silently replacing failed self-host semantics | ABSENT | Every failure is typed and terminal: `AuthorityError` (no receipt on any variant), `PackageError`, `ImageError`, `LowerErrorCode` including `VM_LOWER_CACHE_KEY_UNSUPPORTED` "against silent profile fallback" (`rw-070.md` section 3); `admit_v2_package_from_sley_evidence` refuses rather than routing to the seed; REWEAVE 14.2 "No silent fallback is allowed after promotion" is not yet exercisable (no promotion has occurred) |
| Nominal self-hosting where the authoritative transformation still occurs outside Sley | NOT CLAIMED | No record claims SH1 or SH2; `bootstrap-manifest.json` S/C1/C2/C3 null; every RW-080 slice record states "not accepted runtime authority ... no C1/SH claims"; the premium r1 review (AR-07, log :126) already identified that v2 admission requires native semantics and the repair declared it as C0 seed rather than claiming otherwise. Residual (declared, not hidden): clean-stage exclusion of the seed path is declarative until C1 exists (`reweave-rw075-native-r12-2026-09-07.log` AR-04/AR-07 limitation) |

Hygiene evidence at this commit: `check_host_abi_markers.py:173-207` scans production code of
`sley-vm`, `sley-check`, `sley-ssmc`, `sley-id`, `sley-mutate` for `extern "C"`, `dlopen`,
`libloading`, `std::fs`, `std::env`, `std::process`, `std::net`, `Command::new`, `unsafe`
(other than the `forbid` line), and `cargo`/`rustc` references: PASS. `crates/sley-vm/Cargo.toml`
declares no features; grep for `cfg(feature`, `std::env`, `option_env!` in `sley-vm/src`
production code returns nothing.

## 3. Section 13 gate applicability at this commit

| Gate (spec section 13 minimum) | Status | Basis |
|---|---|---|
| C0 builds C1 | NOT_REQUIRED (C1 does not exist) | `bootstrap-manifest.json` C1 "does not exist"; `rw080` BLOCKED (`machine-summary.json` `rw075_correction.rw080`) |
| C1 builds C2 | NOT_REQUIRED | same |
| C2 builds C3 | NOT_REQUIRED | same |
| Required equality/stability relations (13.2) | NOT_REQUIRED | no images |
| Seed absence (13.3) | NOT_REQUIRED for execution; the record explicitly disclaims it | `host-abi.json` v2 `seed_absence.claim`: "RW-070 does NOT prove seed absence; it makes the R3/R4 proof possible" |
| No hidden native semantic fallback exists | APPLICABLE NOW as a static property: HOLDS | section 2 row 4 above; anti-shortcut probes and marker checkers PASS |
| Substantive self-change test (13.4) | NOT_REQUIRED | no C1 |
| Re-run trigger ("any accepted architecture repair touching compiler semantics, executable package, host ABI, bootstrap profile, schema epoch, canonical identity, dependency binding") | NOT TRIGGERED by this audit | every finding below is A, D, E, or a records-only B; none changes code, canonical bytes, ABI, profile, epoch, or identity. If another audit lane (identity closure) accepts a C-class repair, this table must be re-evaluated by that lane |

AT-G6 therefore holds vacuously; AT-G3 depends on AT-NA-01 (inventory completeness) being closed.

## 4. Is `rw-070-host-inventory.json` this inventory, and is it current?

It is the RW-070 predecessor of this inventory: same intent (classify every native operation
reachable from the bootstrap profile), correct method (zero UNKNOWN rows, evidence per row), but
frozen at `base_commit b4c3390088deed018f818eb1327f130002c27ab6` and last changed in commit
`e85b89c` (RW-070 land). It has not been regenerated for RW-075, the RW-075 correction, or the
RW-080 slices. Compared against the crate surface at `560a5f16`, it is missing or stale on:

1. `RHW1` / `raw-blake3-256` primitive row (`HOST_ABI_V2` has four rows; inventory lists three; its
   `sources` cite `BOOTSTRAP_PROFILE_1` and `host_abi.rs` under `HOST_ABI_V1` only).
2. `crates/sley-vm/src/raw_hash.rs` (rows 18, 36 above).
3. `crates/sley-vm/src/exec_package.rs`: section framing codecs, SHA-256 section/package digests,
   `admit_package_v2` (pub(crate)), `approve_package_v2`, `verify_package_binding_v2`,
   `hydrate_layouts` (rows 22, 23, 25, 26, 29, 30).
4. `crates/sley-vm/src/admission_authority.rs`: C0 seed semantic legs, structural correspondence,
   reserved `SleyAdmissionEvidence` ingress (rows 20, 21, 24).
5. `TypeEnvironment::hydrate_verified_definitions` in `sley-check` (row 29).
6. `execute_approved_package`/`execute_approved_package_v2`, `validate_package_inputs_structural`,
   SLEYPOBS1 package observation (rows 31, 32, 38); the legacy `execute_loaded_image` superseded status.
7. `BootstrapProfileVersion` selector on the gate (`bootstrap.rs:79`) and the V2 carried-Bytes
   preimage bound (`bootstrap.rs:410-418`).
8. TEST_ONLY paths: `tests/rw075_exec_closure.rs`, `rw075_hydration_workloads.rs`,
   `rw075_raw_callable.rs`, `rw080_codec_{scaffold,uvar,envelope,program_outer}.rs`; the listed
   `fuzz/targets/vm_canonical_inputs.rs` does not exist in `fuzz/targets/` at this commit
   (`fuzz/Cargo.toml` names `scb1_decoder`, `schema_bootstrap_decoder`, `repository_pack_importer`,
   `type_checker`, `ssmc_graph_cfg_checker`, ...).
9. FORBIDDEN_FOR_SH2 evidence names "hygiene scan in check_host_abi_v1.py"; the scan lives in
   `scripts/check_host_abi_markers.py:173-207` (`check_host_abi_v1.py` contains no hygiene scan).
   `rw-070.md` section 5 repeats the wrong location.
10. Supply pins `sha2` and `blake3 =1.8.2` as admitted cryptographic remainder (row 45).

The RW-075 prose ("Complete native remainder after RW-075", `rw-075.md` section 7) is the only
newer enumeration and it is prose, not the machine record. Section 1 of this file is the complete
inventory at this commit; AT-NA-01 asks for it to be landed as a successor machine record.

