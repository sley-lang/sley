# Clean-Room Disposition Register v1

Status: S20-780 contract draft, revision 2 (2026-09-05); the three Council
review rounds landed 2026-09-04 (Ariadne contract review, Nabu architecture
review, Vulcan surface review) with three freeze-blocking findings, all
closed by this revision; lower-severity findings remain open.

## Boundary

ADR-0002 states that a concept of Sley 1.2.0 may be reimplemented "only after a
disposition records purpose, observed evidence, machine-native relevance,
security impact, new equivalent, decision, and acceptance test", and master
goal section 26.1 requires that "every reused concept has a disposition and
evidence". `docs/ANTI_GOALS.md` names the control. No register existed, and the
clean-room boundary itself had never been mechanically verified.

This document is that register, plus the mechanical statement of the boundary.
It reuses no legacy source, grants no authority, and changes no identity.

## 1. Register entries

Each entry carries the seven ADR-0002 fields. Every entry's evidence is a
tracked artifact of this repository or the frozen legacy evidence recorded in
`machineresearch/sley-2.0/01-legacy-freeze-and-authority.md`.

### 1.1 The frozen 1.2.0 artifact as a succession arm

- **Purpose**: run the predecessor as one arm of the succession benchmark.
- **Observed evidence**: the frozen artifact
  `sley-1.2.0-linux-x86_64.tar.gz`, SHA-256
  `b24f19c6a348751c93c9cf63f6f4154f6132796112c26f9d8c0e71324080dbc7`,
  4,611,024 bytes, and the exact source snapshot digest
  `1c866d360305d0b511dc2c33c4907b33544fc73bc6cb6fa4c0e1687df48eb90e`, both
  recorded at the freeze.
- **Machine-native relevance**: none to the design; the artifact is evidence,
  not a component.
- **Security impact**: the artifact executes, so it runs out of process, from
  the archive path, under the S20-600 adapter's controls, and never inside a
  Sley 2 crate.
- **New equivalent**: none. Sley 2 does not reimplement the 1.2 program model.
- **Decision**: reused as an external oracle and benchmark arm only.
- **Acceptance test**: `make legacy-runner-smoke` verifies the artifact digest
  and version before any use; `scripts/check_legacy_runner.py` pins the
  boundary.

### 1.2 The succession task corpus

- **Purpose**: compare arms on representation-neutral tasks.
- **Observed evidence**: corpus v1, manifest SHA-256
  `7370b6ccb8ccd3f58fa2a90e316edf4bc5a1319b41a55253a2ee14bb5d73988d`,
  15 task classes frozen in `bench/benchmark-plan.json`.
- **Machine-native relevance**: the corpus is representation-neutral by
  construction, so it favours neither a source language nor a semantic graph.
- **Security impact**: none; the corpus is data.
- **New equivalent**: not applicable; the corpus is shared by every arm.
- **Decision**: legacy evidence may inform fixtures and may not define Sley 2
  semantics (`machineresearch/sley-2.0/04-prior-art-and-competitor-baseline.md`).
- **Acceptance test**: `scripts/check_benchmark_baseline.py` pins the corpus
  digest, the classes, the arms, and the zero-trial claim.

### 1.3 Stable graph identity, reimplemented

- **Purpose**: give every Sley 2 entity, object, and state root a
  content-derived identity without reusing the predecessor's graph identity
  scheme.
- **Observed evidence**: succession-matrix row "stable graph identity →
  reimplement → EntityId/ObjectId/StateRoot"; the frozen 1.2.0 source was not
  consulted, and check 1 below finds no legacy source in the tree to copy
  from.
- **Machine-native relevance**: identities anchor candidates, receipts,
  deltas, refs, and the object store.
- **Security impact**: none; no legacy text entered the tree.
- **New equivalent**: `EntityId`, `ObjectId`, and `StateRoot` as specified in
  `docs/spec/IDENTIFIERS_V1.md`.
- **Decision**: reimplemented from the in-repository specification; not a
  reuse.
- **Acceptance test**: `cargo test -p sley-id` pins the identifier domains
  and their byte layouts (7 tests).

### 1.4 Typed graph checking, reimplemented

- **Purpose**: check Sley 2 semantic graphs against the frozen SSMC1 shape
  and the S20-210 through S20-240 profiles without reusing the predecessor's
  typed-checking implementation.
- **Observed evidence**: succession-matrix row "typed graph checking →
  reimplement → SSMC/check kernel"; the frozen 1.2.0 source was not
  consulted, and check 1 below finds no legacy source in the tree to copy
  from.
- **Machine-native relevance**: every candidate phase and every profile
  judgment depends on the kernel's verdicts.
- **Security impact**: none; no legacy text entered the tree.
- **New equivalent**: the SSMC kernel and `sley-check` as specified in
  `docs/spec/SSMC1.md`, `docs/spec/SSMC1_EPOCH1_SCHEMA.txt`,
  `docs/spec/CONTRACT_TEST_PROFILE_V1.md`,
  `docs/spec/CFG_VALIDATION_V1.md`, and `docs/spec/EFFECT_SYSTEM_V1.md`.
- **Decision**: reimplemented from the in-repository specifications; not a
  reuse.
- **Acceptance test**: `cargo test -p sley-ssmc` (10 tests) and
  `cargo test -p sley-check` (75 tests) pin the specified kernel and checker
  behavior.

### 1.5 Effects and authority model, reimplemented

- **Purpose**: model effects, capabilities, and policy without ambient
  authority and without reusing the predecessor's effects and authority
  fixtures.
- **Observed evidence**: succession-matrix row "effects/authority fixtures →
  preserve and reimplement concept → effects + policy/capability"; the frozen
  1.2.0 source was not consulted, and check 1 below finds no legacy source
  in the tree to copy from.
- **Machine-native relevance**: capability-deny and effect-closure verdicts
  gate every candidate that touches authority.
- **Security impact**: none; no legacy text entered the tree.
- **New equivalent**: the effect system, capability tokens and summary, and
  the policy root as specified in `docs/spec/EFFECT_SYSTEM_V1.md`,
  `docs/spec/CAPABILITY_TOKEN_V1.md`,
  `docs/spec/CAPABILITY_SUMMARY_V1.md`, and
  `docs/spec/POLICY_ROOT_V1.md`.
- **Decision**: reimplemented from the in-repository specifications; not a
  reuse.
- **Acceptance test**: `cargo test -p sley-check effects` (16 tests) and
  `cargo test -p sley-policy capability` (10 tests) pin the specified effect
  and capability behavior.

### 1.6 Everything else: no reused concept

No other concept of Sley 1.2.0 is reused. Sley 2 is machine-native lineage: its
canonical encoding, program representation, type and effect systems, object
store, state roots, mutation and transaction model, policy and capability
model, VM, repository model, and protocol are specified in this repository's
contracts and implemented from those specifications. The master goal directs
exactly this ("implement Sley 2.0 from first principles"; "ship no Sley source
syntax, parser, formatter, conventional LSP, human review surface, source
compatibility, or self-hosting requirement"), and section 2 states the
mechanical facts that keep the claim true.

A future reuse requires a new entry here, with all seven fields, and an
update to the machine-summary reuse count and entry list, before the
implementation lands.

## 2. Mechanical boundary

`scripts/check_clean_room_boundary.py` verifies, from the tree alone:

1. **No legacy artifact reference outside the adapter.** No tracked Rust,
   Python, manifest, or JSON file under `crates/`, `oracle/`, `bench/`,
   `fuzz/`, or `scripts/` carries the frozen archive path
   `archive/sley/1.2.0` outside the allowlist. This is a single-sentinel
   search, not a source detector: a renamed copy, a legacy module marker, or
   an unpacked tree would not match it, and no tree-alone check could
   fingerprint those without the legacy layout. That detection is not
   mechanized; it rests on the per-concept construction evidence in section 1
   and the section 3 audit disclaimer.
2. **No legacy dependency.** No crate manifest names a Sley 1.x package, and
   the workspace's only path dependencies are the eighteen Sley 2 crates.
3. **One touchpoint, out of process.** The single reference to the frozen
   artifact is the S20-600 legacy adapter under `bench/legacy/`, which invokes
   it with `subprocess` from the archive path; no crate references it.
4. **The register covers its entries.** Every entry of section 1 carries all
   seven ADR-0002 fields.

A violation is `CLEAN_ROOM_BOUNDARY_VIOLATION` (78000); a register entry
missing a field is `DISPOSITION_INCOMPLETE` (78001).

## 3. What this register is not

- Not a similarity or provenance audit of the legacy source, which no one has
  performed and which this document does not claim.
- Not a license or legal opinion about either project.
- Not an acceptance of the succession benchmark, which has executed no trial.

## 4. Staging

`scripts/check_clean_room_boundary.py` runs under `make quick`. Statuses:
`S20_780_CONTRACT_DRAFT_REVIEW_PENDING` and `S20_780_REGISTER_ACCEPTED` after
the three Council reviews read `PASS`.
