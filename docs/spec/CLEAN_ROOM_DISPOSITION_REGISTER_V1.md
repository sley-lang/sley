# Clean-Room Disposition Register v1

Status: S20-780 contract draft, revision 1 (2026-09-03); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review).

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

### 1.3 Everything else: no reused concept

No other concept of Sley 1.2.0 is reused. Sley 2 is machine-native lineage: its
canonical encoding, program representation, type and effect systems, object
store, state roots, mutation and transaction model, policy and capability
model, VM, repository model, and protocol are specified in this repository's
contracts and implemented from those specifications. The master goal directs
exactly this ("implement Sley 2.0 from first principles"; "ship no Sley source
syntax, parser, formatter, conventional LSP, human review surface, source
compatibility, or self-hosting requirement"), and section 2 states the
mechanical facts that keep the claim true.

A future reuse requires a new entry here, with all seven fields, before the
implementation lands.

## 2. Mechanical boundary

`scripts/check_clean_room_boundary.py` verifies, from the tree alone:

1. **No legacy source in the tree.** No file under `crates/`, `oracle/`, or
   `bench/` carries the legacy source snapshot's paths or a legacy module
   marker, and no legacy source archive is unpacked into the repository.
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
