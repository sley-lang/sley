# Threat Register

Status: M0 planned-control map. Evidence paths are future required outputs and
must not be read as passing evidence until the named work package records them.

`scripts/build_threat_coverage_report.py` measures how far the plan has been
realized and writes `evidence/security/threat-coverage-report.json`, which the
S20-750 dossier reads. It locates each expected failure code in the tree and
classifies it; a located symbol is not a mitigation claim, and the threats it
cannot locate are a work list for the independent security review, not an
assertion that they are untested.

| ID | Threat | Sev | Owner | Expected failure code | Required test | Evidence path |
|---|---|---:|---|---|---|---|
| T01 | malformed SCB1 | P0 | sley-canon | `SCB_MALFORMED` | decoder fuzz/rejection | `evidence/security/T01/` |
| T02 | alternate noncanonical encoding | P0 | sley-canon | `SCB_NON_CANONICAL` | independent rejection vectors | `evidence/security/T02/` |
| T03 | object hash mismatch | P0 | sley-store | `SCB_DIGEST_MISMATCH` | one-byte corruption | `evidence/security/T03/` |
| T04 | object substitution | P0 | sley-store | `STORE_OBJECT_SUBSTITUTION` | wrong-object/preimage fault | `evidence/security/T04/` |
| T05 | schema downgrade | P0 | sley-schema | `SCHEMA_DOWNGRADE` | downgrade negotiation/import | `evidence/security/T05/` |
| T06 | schema epoch confusion | P0 | sley-schema | `SCHEMA_EPOCH_MISMATCH` | cross-epoch object matrix | `evidence/security/T06/` |
| T07 | duplicate logical identity | P0 | sley-id | `ID_DUPLICATE_ENTITY` | creation collision property | `evidence/security/T07/` |
| T08 | identity reuse after deletion | P0 | sley-id | `ID_REUSE_FORBIDDEN` | tombstone recreation | `evidence/security/T08/` |
| T09 | dangling reference | P0 | sley-check | `GRAPH_UNRESOLVED_REFERENCE` | reference negative corpus | `evidence/security/T09/` |
| T10 | hostile graph cycles | P0 | sley-check | `GRAPH_CYCLE_FORBIDDEN` | cyclic graph fuzz | `evidence/security/T10/` |
| T11 | pathological CFG | P0 | sley-check | `CFG_RESOURCE_LIMIT` | hostile CFG corpus | `evidence/security/T11/` |
| T12 | type-checker nontermination | P0 | sley-check | `TYPE_RESOURCE_LIMIT` | recursive-type/fuel fuzz | `evidence/security/T12/` |
| T13 | query explosion | P1 | sley-query | `QUERY_RESOURCE_LIMIT` | fanout/depth adversarial | `evidence/security/T13/` |
| T14 | truncation hides required facts | P0 | sley-query | `QUERY_REQUIRED_FACT_OMITTED` | truncation/validation independence | `evidence/security/T14/` |
| T15 | handle reuse across roots | P0 | sley-query | `SESSION_STALE_HANDLE` | root/session/epoch matrix | `evidence/security/T15/` |
| T16 | oversized mutation list | P1 | sley-mutate | `MUTATION_RESOURCE_LIMIT` | count/byte boundary | `evidence/security/T16/` |
| T17 | stale-root commit | P0 | sley-txn | `STALE_ROOT` | concurrent CAS scenario; restricted S20-390 coverage present | `evidence/validation/s20-390-atomic-commit-closeout-v1.json` |
| T18 | stale-entity commit | P0 | sley-txn | `STALE_ENTITY` | exact preimage mutation; restricted S20-360/S20-390 coverage present | `evidence/validation/s20-390-atomic-commit-closeout-v1.json` |
| T19 | candidate modifies own policy | P0 | sley-policy | `POLICY_SELF_MODIFICATION` | policy-isolation E2E | `evidence/security/T19/` |
| T20 | candidate modifies validator epoch | P0 | sley-schema | `SCHEMA_SELF_MODIFICATION` | epoch-isolation E2E | `evidence/security/T20/` |
| T21 | candidate weakens mandatory tests | P0 | sley-policy | `POLICY_ORACLE_SELF_MODIFICATION` | test-root isolation | `evidence/security/T21/` |
| T22 | capability forgery | P0 | sley-policy | `CAP_AUTHENTICATOR_INVALID` | token bit-flip/issuer matrix; S20-380 unit coverage present | `evidence/security/T22/` |
| T23 | capability replay | P0 | sley-policy | `CAP_REPLAY` | nonce/root/expiry replay; S20-380 unit coverage present | `evidence/security/T23/` |
| T24 | capability scope confusion | P0 | sley-policy | `CAP_SCOPE_MISMATCH` | workspace/effect/resource matrix; S20-380 unit coverage present | `evidence/security/T24/` |
| T25 | adapter impersonation | P0 | sley-adapter | `ADAPTER_IDENTITY_MISMATCH` | adapter ABI/identity swap | `evidence/security/T25/` |
| T26 | adapter response injection | P0 | sley-adapter | `ADAPTER_TYPE_MISMATCH` | typed replay outcome injection | `evidence/security/T26/` |
| T27 | path traversal | P0 | sley-adapter | `ADAPTER_PATH_INVALID` | traversal/separator/Unicode corpus | `evidence/security/T27/` |
| T28 | symlink escape | P0 | sley-adapter | `ADAPTER_SYMLINK_ESCAPE` | confined-root symlink matrix | `evidence/security/T28/` |
| T29 | environment leakage | P0 | sley-vm | `VM_AMBIENT_STATE_FORBIDDEN` | clean/poisoned env equivalence | `evidence/security/T29/` |
| T30 | output flooding | P1 | sley-vm | `VM_OUTPUT_LIMIT` | exact output ceiling | `evidence/security/T30/` |
| T31 | cancellation bypass | P1 | sley-vm | `VM_CANCELLED` | bounded cancel latency | `evidence/security/T31/` |
| T32 | fuel/memory bypass | P0 | sley-vm | `VM_RESOURCE_LIMIT` | nested/call/collection stress | `evidence/security/T32/` |
| T33 | VM nondeterminism | P0 | sley-vm | `VM_NONDETERMINISM_DETECTED` | repeated observation digests | `evidence/security/T33/` |
| T34 | floating host divergence | P0 | sley-vm | `VM_FLOAT_PROFILE_MISMATCH` | cross-build FP vectors | `evidence/security/T34/` |
| T35 | cache poisoning | P0 | sley-query | `CACHE_BINDING_MISMATCH` | root/epoch/policy key faults | `evidence/security/T35/` |
| T36 | derived index treated as canonical | P0 | sley-store | `STORE_DERIVED_INPUT_FORBIDDEN` | corrupt/delete/rebuild cache | `evidence/security/T36/` |
| T37 | crash during object write | P0 | sley-store | `RECOVERY_STAGED_OBJECT` | interruption matrix | `evidence/security/T37/` |
| T38 | crash during receipt write | P0 | sley-txn | `RECOVERY_RECEIPT_INCOMPLETE` | restricted S20-390 interruption matrix present | `evidence/validation/s20-390-atomic-commit-closeout-v1.json` |
| T39 | crash during accepted-head or later ref update | P0 | sley-txn, then sley-repo for S20-500 named refs | `RECOVERY_REF_CAS_INCOMPLETE` | restricted fixed-head S20-390 interruption matrix present; named refs pending | `evidence/validation/s20-390-atomic-commit-closeout-v1.json` |
| T40 | GC deletes reachable object | P0 | sley-store | `GC_REACHABILITY_VIOLATION` | graph/pin/lease property | `evidence/security/T40/` |
| T41 | malicious pack | P0 | sley-repo | `PACK_INVALID` | importer fuzz/corruption | `evidence/security/T41/` |
| T42 | decompression bomb | P1 | sley-repo | `PACK_DECOMPRESSION_LIMIT` | ratio/size boundary | `evidence/security/T42/` |
| T43 | merge loses change | P0 | sley-repo | `MERGE_CHANGE_LOSS` | delta preservation property | `evidence/security/T43/` |
| T44 | merge silently chooses conflict | P0 | sley-repo | `MERGE_CONFLICT_REQUIRED` | incompatible same-entity E2E | `evidence/security/T44/` |
| T45 | protocol downgrade | P0 | sley-protocol | `PROTOCOL_DOWNGRADE` | handshake matrix | `evidence/security/T45/` |
| T46 | request ID confusion | P0 | sley-protocol | `PROTOCOL_REQUEST_ID_CONFLICT` | duplicate/cross-session IDs | `evidence/security/T46/` |
| T47 | cross-workspace leakage | P0 | sley-protocol | `SESSION_WORKSPACE_MISMATCH` | two-workspace isolation | `evidence/security/T47/` |
| T48 | model-generated authority claim | P0 | sley-policy | `CAPABILITY_DENIED` | hostile label/prompt metadata | `evidence/security/T48/` |
| T49 | debug dump used as canonical input | P0 | sley-canon | `SCB_MAGIC_INVALID` | debug-notation input | `evidence/security/T49/` |
| T50 | Git metadata used as Sley state | P0 | sley-repo | `REPO_EXTERNAL_METADATA_FORBIDDEN` | Git-independent reconstruction | `evidence/security/T50/` |
| T51 | ZJX transport tampering | P0 | sley-repo | `PACK_DIGEST_MISMATCH` | decompress/tamper/import | `evidence/security/T51/` |
| T52 | dependency substitution | P1 | release | `RELEASE_DEPENDENCY_MISMATCH` | local lock/source/checksum inventory present; standards SBOM and release provenance pending | `evidence/security/T52/` |
| T53 | release artifact substitution | P1 | release | `RELEASE_ARTIFACT_MISMATCH` | manifest/hash verification | `evidence/security/T53/` |
| T54 | secret committed in fixtures | P1 | release | `RELEASE_SECRET_FINDING` | bounded high-confidence candidate/history scan present; release re-anchor, wider privacy review, and independent disposition pending | `evidence/security/T54/` |
| T55 | benchmark contamination/cherry-pick | P1 | sley-bench | `BENCH_CONTROL_VIOLATION` | manifest denominator/control audit | `evidence/security/T55/` |

P0/P1 evidence requires independent Vulcan disposition. A green test without a
fault-seeding or assertion-effectiveness check remains an open release finding.

## Realized codes (2026-09-03)

The table above is the M0 plan: it names the failure code each control was
expected to produce. Several controls shipped under a different code, so the
coverage report understated them. This addendum records the realized code and
where it is enforced; `scripts/build_threat_coverage_report.py` searches for
these instead of the expected code where an entry exists.

An entry here is a traceability record, not an acceptance: the independent
security review still judges whether the control is sufficient.

| ID | Expected code | Realized code | Enforced in | Exercised by |
|---|---|---|---|---|
| T07 | `ID_DUPLICATE_ENTITY` | `CANDIDATE_IDENTITY_COLLISION` | `crates/sley-policy/src/candidate_validation.rs` phase 4, live-binding branch | `tombstones_graph_errors_and_missing_references_are_distinct` |
| T08 | `ID_REUSE_FORBIDDEN` | `CANDIDATE_IDENTITY_COLLISION` | the same phase 4 check, tombstone branch (`context.tombstones.binary_search`) | the same test, tombstone case |
| T35 | `CACHE_BINDING_MISMATCH` | `VM_LOWER_CACHE_KEY_UNSUPPORTED` plus the binding itself | `crates/sley-vm/src/lib.rs` `cache_key_preimage`, which binds schema epoch, field-schema hash, decoder-limits hash, state root, entry function, and profile so a different binding cannot collide | the extended vectors, whose cache keys the independent oracle re-derives |
| T50 | `REPO_EXTERNAL_METADATA_FORBIDDEN` | structural: no kernel crate references Git, and `STATE_ROOT_V1.md` excludes ref names, ancestry, timestamps, paths, locks, caches, and Git metadata from the root | `docs/spec/REPOSITORY_MODEL_V1.md` and the state-root binding | the clone-equivalence corpus, which reproduces roots outside the producing repository |
| T53 | `RELEASE_ARTIFACT_MISMATCH` | `PACKAGE_NOT_REPRODUCIBLE` (72005) and `REPRO_ATTESTATION_CONFLICT` (73003) | `scripts/build_release_candidate.py` byte-for-byte comparison; `scripts/build_reproducibility_report.py` cross-host digest agreement | `make release-candidate-smoke` and `bench/release/tests/` |
| T55 | `BENCH_CONTROL_VIOLATION` | `ACCOUNTING_INCOMPLETE`, `ACCOUNTING_CHAIN_INVALID`, `ACCOUNTING_ARM_UNKNOWN`, `ACCOUNTING_FLOAT_FORBIDDEN` | `bench/accounting/report.py`, which counts every attempt in the denominator and refuses a broken claim chain | `bench/accounting/tests/test_report.py` |
| T01 | `SCB_MALFORMED` | `SCB_MAGIC_INVALID`, `SCB_VERSION_UNSUPPORTED`, `SCB_LENGTH_OVERFLOW`, `SCB_TRAILING_BYTES`, `SCB_FIELD_MISSING`, `SCB_FIELD_UNKNOWN`, `SCB_UNION_INVALID`, `SCB_BOOL_INVALID` | `crates/sley-scb1/src/lib.rs` strict decode | `conformance/scb1/v1/rejected.json`, 26 vectors over 22 codes, checked by the independent oracle |
| T02 | `SCB_NON_CANONICAL` | `SCB_VARINT_NON_MINIMAL`, `SCB_FIELD_ORDER`, `SCB_MAP_ORDER`, `SCB_MAP_DUPLICATE`, `SCB_FLOAT_NON_CANONICAL`, `SCB_LABEL_NOT_NFC` | the same decoder, which refuses rather than rewrites | the same rejected corpus |
| T10 | `GRAPH_CYCLE_FORBIDDEN` | `TYPE_DEFINITION_CYCLE` for type graphs; `GRAPH_UNRESOLVED_REFERENCE` and the S20-220 inventory rules for entity graphs | `crates/sley-check/src/lib.rs` and `crates/sley-check/src/cfg.rs` | the sley-check unit corpus and the semantic-checkers persistent fuzz slice |
| T19 | `POLICY_SELF_MODIFICATION` | `POLICY_ISOLATION_POLICY_ROOT_CHANGED`, `POLICY_ISOLATION_CONTRACT_ROOT_CHANGED` | `crates/sley-policy/src/lib.rs` isolation judgment | the candidate validation isolation tests |
| T21 | `POLICY_ORACLE_SELF_MODIFICATION` | `POLICY_FINAL_REQUIRED_TEST_MISSING`, `POLICY_FINAL_REQUIRED_TEST_NOT_SELECTED`, `POLICY_FINAL_REQUIRED_CONTRACT_MISSING` | the same final-report judgment | the mandatory test planning tests |
| T28 | `ADAPTER_SYMLINK_ESCAPE` | structural: the object store stats every path with `fs::symlink_metadata` and refuses anything that is not a regular file, so a symlink never resolves | `crates/sley-store/src/lib.rs` | the S20-700 object-store symlink slice, three cases with zero writes outside the root |
| T16 | `MUTATION_RESOURCE_LIMIT` | `SCB_RESOURCE_LIMIT` when a candidate's operations or preconditions exceed the full validation profile's ceilings; `GRAPH_RESOURCE_LIMIT` for the program graph | `crates/sley-mutate/src/candidate.rs`, `crates/sley-policy/src/candidate_program.rs` | the mutation-candidate conformance corpus and the `mutation_candidate` fuzz target |
| T25 | `ADAPTER_IDENTITY_MISMATCH` | `AdapterErrorCode::IdentityMismatch` (28001), a numeric variant rather than a string constant, with `CAP_ADAPTER_MISMATCH` when a token bound to one adapter is presented by another | `crates/sley-adapter/src/lib.rs` | `identity_abi_and_effect_swaps_fail_without_mutation` and `scripts/check_reference_adapter_profile.py` |
| T26 | `ADAPTER_TYPE_MISMATCH` | `AdapterErrorCode::TypeMismatch` (28004) at the reference adapter and `ADAPTER_INVOKE_TYPE` at the effect boundary | `crates/sley-adapter/src/lib.rs`, `crates/sley-check/src/effects.rs` | the adapter unit corpus and the `adapter_responses` fuzz target |
| T27 | `ADAPTER_PATH_INVALID` | `AdapterErrorCode::PathInvalid` (28006), enforced with the canonical path ceilings `MAX_PATH_COMPONENT_BYTES` and `MAX_REQUEST_PATH_BYTES` | `crates/sley-adapter/src/lib.rs` | the reference adapter path cases and the same fuzz target |
| T29 | `VM_AMBIENT_STATE_FORBIDDEN` | structural: no VM source references the environment, filesystem, clock, network, process table, or a random source, so no opcode can read ambient state; execution sees only its inputs, cells, and program | `crates/sley-vm/src/execute.rs` | the anti-goal conformance report's process and network checks, and the extended VM vector corpus |
| T30 | `VM_OUTPUT_LIMIT` | `VM_EXEC_RESOURCE_LIMIT` carrying `ResourceKind::OutputUnits` | `crates/sley-vm/src/execute.rs` | the VM execution unit corpus and the `vm_canonical_inputs` fuzz target |
| T31 | `VM_CANCELLED` | `VM_EXEC_CANCELLED` | `crates/sley-vm/src/execute.rs` | the VM execution unit corpus |
| T32 | `VM_RESOURCE_LIMIT` | `VM_EXEC_RESOURCE_LIMIT` over five ceilings (instructions, fuel, value units, output units, call depth) and `VM_LOWER_RESOURCE_LIMIT` at lowering | `crates/sley-vm/src/execute.rs`, `crates/sley-vm/src/lower.rs` | the VM execution unit corpus and the `vm_canonical_inputs` fuzz target |
| T33 | `VM_NONDETERMINISM_DETECTED` | structural: determinism is a property of the encoding rather than a detected failure. Canonical NaN, negative-zero folding, map order by encoded key bytes, and the absence of host state leave nothing nondeterministic to admit; a divergence surfaces as a cache-key or vector mismatch | `crates/sley-vm/src/extended.rs` | the independent oracle's `check-vm-extended`, which re-derives every accepted vector's 224-byte cache-key preimage |
| T34 | `VM_FLOAT_PROFILE_MISMATCH` | `SCB_FLOAT_NON_CANONICAL` at the codec boundary, with `canonical_f32` and `canonical_f64` folding every NaN to one bit pattern and negative zero to positive zero before a result is encoded | `crates/sley-vm/src/extended.rs`, `crates/sley-scb1/src/lib.rs` | the E3 float vectors and the SCB1 rejected corpus |
| T36 | `STORE_DERIVED_INPUT_FORBIDDEN` | `STORE_OBJECT_SUBSTITUTION`, since every read re-derives the digest from the record's bytes and refuses a mismatch; the one cached index is read-only, disposable, and never read by validation, comparison, merge, commit, exchange, GC, or recovery | `crates/sley-store/src/lib.rs`, `crates/sley-repo/src/index_cache.rs` | the object store unit corpus and the index cache tests |
| T43 | `MERGE_CHANGE_LOSS` | `MERGE_RESULT_MISMATCH`: the committed merge's state root must equal the plan's merged root or the branch does not advance; `MERGE_COMPARE_FAILED` covers the comparison itself | `crates/sley-repo/src/merge.rs` | the merge unit corpus and the `merge_conflict_decoder` fuzz target |
| T44 | `MERGE_CONFLICT_REQUIRED` | a divergent delete, add, kind, metadata, or field edit yields a `ConflictEntry` and the merge returns a conflict outcome instead of a merged entity set; `MERGE_PLAN_UNSUPPORTED` refuses a plan built over one | `crates/sley-repo/src/merge.rs` | the merge unit corpus and the same fuzz target |
