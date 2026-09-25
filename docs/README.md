# Sley Documentation

Everything about Sley 2, organized by what you're trying to do.

> **Start here:** [Quickstart](QUICKSTART.md) to run it · [Concepts](CONCEPTS.md)
> to understand it · [Architecture walkthrough](https://sleylang.org/tutorial)
> to see a change travel through the system.

---

## Guides

| Guide | Read it when you want to… |
|---|---|
| 🚀 [Quickstart](QUICKSTART.md) | Install `sley`, run the demo, and drive a session by hand |
| 🧠 [Concepts](CONCEPTS.md) | Understand graphs, identities, the change lifecycle, and the glossary |
| 🧭 [Architecture walkthrough](https://sleylang.org/tutorial) *(sleylang.org)* | Follow state → proposal → validation → transaction → new state, step by step |
| 🏗️ [Architecture](../ARCHITECTURE.md) | Learn crate authority, the dependency law, and commit durability order |
| 🐍 [Example client](examples/smp1_json_client.py) | Copy a working SMP1 JSON-lines client (Python, standard library only) |
| 📦 [Sley 2.0.0 release notes](release/SLEY-2.0.0.md) | See what shipped, how to verify the artifact, and the known limits |
| 🩹 [Sley 2.0.1 release notes](release/SLEY-2.0.1.md) | See the fixes since 2.0.0, including four contributed by Fred Nix |
| ❓ [FAQ](https://sleylang.org/faq) *(sleylang.org)* | Get quick answers about scope, status, and Sley 1.x |

## Specifications

Every spec under [`spec/`](spec/) is normative: the implementation, the
independent oracle, and the conformance corpora all follow it exactly. Specs
are versioned (`_V1`, `_V2`) and revised in place with a recorded history.

### 🧬 Core format and program model

| Spec | Defines |
|---|---|
| [SCB1](spec/SCB1.md) | The canonical binary encoding: strict, minimal, and domain-separated |
| [SSMC1](spec/SSMC1.md) · [epoch-1 schema](spec/SSMC1_EPOCH1_SCHEMA.txt) | The program form: 18 entity kinds, types, opcodes, and terminators |
| [Identifiers](spec/IDENTIFIERS_V1.md) | Every ID and hash domain |
| [Schema epochs](spec/SCHEMA_EPOCH_V1.md) · [Epoch migration](spec/EPOCH_MIGRATION_POLICY_V1.md) | Schema versioning and explicit migration |
| [State root](spec/STATE_ROOT_V1.md) | The deterministic, ancestry-independent state digest |
| [Complete entity model](spec/COMPLETE_ENTITY_IMPACT_PROFILE_V1.md) | Every entity body, and its impact relationships |

### ✅ Checking and semantics

| Spec | Defines |
|---|---|
| [Type system](spec/TYPE_SYSTEM_V1.md) | The deterministic core type system |
| [CFG validation](spec/CFG_VALIDATION_V1.md) | Control-flow and value-use rules |
| [Effect system](spec/EFFECT_SYSTEM_V1.md) | Exact effect closure and scope typing |
| [Contracts and tests](spec/CONTRACT_TEST_PROFILE_V1.md) | Contract bindings and test planning |
| [Fingerprints](spec/FINGERPRINT_IMPACT_PROFILE_V1.md) · [Production fingerprints](spec/PRODUCTION_FINGERPRINT_PROFILE_V1.md) | Semantic fingerprints and impact |
| [Required contract index](spec/REQUIRED_CONTRACT_INDEX_V1.md) | Which contracts a state must satisfy |

### ✏️ Changing programs

| Spec | Defines |
|---|---|
| [Mutation schema](spec/MUTATION_SCHEMA_V1.md) | Generated mutation descriptors for every kind and field |
| [Mutation value codec](spec/MUTATION_VALUE_CODEC_V1.md) | The exact encoding of proposed values |
| [Candidate record](spec/CANDIDATE_RECORD_V1.md) | The `SLEYCAN1` candidate |
| [Preconditions](spec/PRECONDITION_PAYLOAD_V1.md) · [Expiry](spec/EXPIRY_V1.md) | Stale-safe preconditions and candidate expiry |
| [Capability summary](spec/CAPABILITY_SUMMARY_V1.md) | What a candidate declares it needs |
| [Validation profile](spec/VALIDATION_PROFILE_V1.md) · [Candidate result](spec/CANDIDATE_RESULT_V1.md) | The 14-phase validator and its result record |
| [Transaction model](spec/TRANSACTION_MODEL_V1.md) | Atomic commit, receipts, and the accepted head |

### 🔎 Context for agents

| Spec | Defines |
|---|---|
| [Root-backed queries](spec/ROOT_BACKED_QUERY_PROFILE_V1.md) | Typed queries against a verified root |
| [Context capsules](spec/CONTEXT_CAPSULE_PROFILE_V1.md) | Evidence envelopes around complete answers |
| [Entity reads](spec/ENTITY_READ_PROFILE_V2.md) | Bounded entity and signature reads (protocol v2) |
| [Complete-root snapshots](spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md) | Derived indexes over a whole root |
| [Restricted snapshots](spec/INDEX_SNAPSHOT_PROFILE_V1.md) · [Restricted queries](spec/RESTRICTED_QUERY_PROFILE_V1.md) · [Restricted capsules](spec/RESTRICTED_QUERY_CAPSULE_PROFILE_V1.md) | The earlier restricted profiles, kept as normative |

### ⚙️ Execution

| Spec | Defines |
|---|---|
| [Execution model](spec/EXECUTION_MODEL_V1.md) | Deterministic execution semantics |
| [VM lowering](spec/VM_LOWERING_PROFILE_V1.md) · [VM execution](spec/VM_EXECUTION_PROFILE_V1.md) | Bytecode lowering and the VM |
| [Extended opcodes](spec/VM_EXTENDED_OPCODE_PROFILE_V1.md) | The extended opcode set |
| [Report envelopes](spec/REPORT_ENVELOPE_PROFILE_V1.md) | Execution and test reports |
| [Native test admission](spec/NATIVE_TEST_ADMISSION_V1.md) · [execution](spec/NATIVE_TEST_EXECUTION_V1.md) · [reservations](spec/NATIVE_TEST_RESERVATIONS_V1.md) | Native tests (protocol v3) |

### 🗄️ Repository

| Spec | Defines |
|---|---|
| [Repository model](spec/REPOSITORY_MODEL_V1.md) | Objects, refs, and ancestry |
| [Object store](spec/OBJECT_STORE_V1.md) | Immutable content-addressed storage |
| [Refs and branches](spec/NATIVE_REFS_BRANCHES_V1.md) | Atomic named refs and fast-forward CAS |
| [Semantic comparison](spec/SEMANTIC_COMPARISON_V1.md) · [Merge](spec/MERGE_V1.md) | Semantic diff, deterministic merge, and conflict objects |
| [Repository pack](spec/REPOSITORY_PACK_V1.md) · [Exchange](spec/REPOSITORY_EXCHANGE_V1.md) | Export, import, and clone |
| [Garbage collection](spec/GARBAGE_COLLECTION_V1.md) | Retention and collection |
| [Crash recovery](spec/CRASH_RECOVERY_MATRIX_V1.md) · [native commit and clone](spec/CRASH_RECOVERY_MATRIX_NATIVE_V1.md) | Old-or-new-never-partial recovery |

### 🔐 Policy and security

| Spec | Defines |
|---|---|
| [Policy root](spec/POLICY_ROOT_V1.md) | The protected policy root |
| [Capability tokens](spec/CAPABILITY_TOKEN_V1.md) | Scoped, budgeted grants |
| [Reference adapters](spec/REFERENCE_ADAPTER_PROFILE_V1.md) | Bounded host adapters |
| [Threat register](THREAT_REGISTER.md) | Every threat with its mitigation and evidence |
| [Security policy](../SECURITY.md) | Reporting and the security model |

### 📡 Protocol and CLI

| Spec | Defines |
|---|---|
| [SMP1](spec/SMP1.md) | The machine protocol: framing, handshake, sessions, bounds, cancellation, and every body record |
| [Sessions and handles](spec/SESSION_HANDLE_PROFILE_V1.md) | Negotiated sessions and stale-safe handles |
| [JSON bridge](spec/SMP1_JSON_BRIDGE_V1.md) | The exact JSON-lines mapping |
| [CLI](spec/SLEY_CLI_V1.md) | The `sley` command, its report, and exit statuses |
| [Error codes](spec/ERROR_CODES_V1.md) | The registry of stable failure codes |

### 🔁 Self-hosting (REWEAVE, the Sley 2.1 track)

| Spec | Defines |
|---|---|
| [Bootstrap profile 1](spec/BOOTSTRAP_PROFILE_1.md) · [profile 2](spec/BOOTSTRAP_PROFILE_2.md) | The staged bootstrap toward a self-hosted toolchain |
| [Host ABI v1](spec/HOST_ABI_V1.md) · [v2](spec/HOST_ABI_V2.md) · [Host hydration](spec/HOST_HYDRATION_V1.md) | The host interface for self-hosted stages |
| [Exec package v1](spec/EXEC_PACKAGE_V1.md) · [v2](spec/EXEC_PACKAGE_V2.md) | Executable package envelopes |
| [Raw hash](spec/RAW_HASH_V1.md) | Raw hashing for self-hosted code |

### 📦 Release, reproducibility, and benchmarks

| Spec | Defines |
|---|---|
| [Release packaging](spec/RELEASE_CANDIDATE_PACKAGING_V1.md) | The release artifact and its checks |
| [Reproducibility](spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md) | Two-build and second-host reproducibility, plus independent conformance |
| [SBOM and provenance](spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md) | CycloneDX, SPDX, and build provenance |
| [Finding register](spec/FINDING_REGISTER_V1.md) · [Decision dossier](spec/DECISION_DOSSIER_V1.md) | How findings and release decisions are recorded |
| [Trial runner](spec/SLEY2_TRIAL_RUNNER_V1.md) · [Raw baseline runner](spec/RAW_BASELINE_RUNNER_V1.md) · [Succession accounting](spec/SUCCESSION_ACCOUNTING_V1.md) | The succession benchmark |
| [External comparison](spec/EXTERNAL_COMPARISON_AVAILABILITY_V1.md) | Comparisons against external systems |
| [Legacy artifact adapter](spec/LEGACY_ARTIFACT_ADAPTER_V1.md) · [Clean-room register](spec/CLEAN_ROOM_DISPOSITION_REGISTER_V1.md) | The boundary with Sley 1.x |

## Decisions and governance

| Document | What's in it |
|---|---|
| [Architecture Decision Records](adr/README.md) | Append-only ADRs, from [machine-native lineage](adr/ADR-0001-machine-native-lineage.md) to [REWEAVE adoption](adr/ADR-0049-reweave-scope-adoption.md) |
| [Anti-goals](ANTI_GOALS.md) | What Sley must never become, and the test that enforces each rule |
| [Work packages](WORK_PACKAGES.md) | The work-package DAG that built 2.0 |
| [Audits and closeouts](audits/) | The closeout record for each work package, fuzz slices, and architecture audits |

## Repository map

| Path | Contents |
|---|---|
| [`crates/`](../crates/) | The 20-crate Rust workspace. Each crate owns one authority. |
| [`oracle/`](../oracle/) | The independent Python SCB1 oracle |
| [`conformance/`](../conformance/) | Cross-implementation corpora: accepted and rejected vectors for every surface |
| [`fuzz/`](../fuzz/) | Persistent libFuzzer targets |
| [`bench/`](../bench/) | The succession benchmark, accounting, and the release demo |
| [`evidence/`](../evidence/) | Tracked evidence: builds, SBOMs, reproducibility, and reviews |
| [`scripts/`](../scripts/) | Checkers and generators behind the `make` gates |

## Elsewhere

- 🌐 **[sleylang.org](https://sleylang.org)**: the Sley website, with [docs](https://sleylang.org/docs), the [walkthrough](https://sleylang.org/tutorial), the [FAQ](https://sleylang.org/faq), and [llms.txt](https://sleylang.org/llms.txt)
- 𝕏 **[@SleyLanguage](https://x.com/SleyLanguage)**: release news and updates
- 🔥 **[Greyforge Labs](https://greyforge.tech)** ([@GreyforgeLabs](https://x.com/GreyforgeLabs)): the team behind Sley
- 📰 **[The machine-native break](https://greyforge.tech/chronicles/sley-120-machine-native-break)**: why Sley left source code behind
- 🏛️ **[Sley 1.x](https://github.com/GreyforgeLabs/sley-legacy)**: the frozen, human-readable legacy lineage
