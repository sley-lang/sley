# Clean-Room Disposition Register v1

Status: S20-780 contract draft, revision 5 (2026-09-14); revision 3 closed
the P0s and addressed the P1/P2/P3 findings; the first re-review round
returned Ariadne PASS and Vulcan PASS with no new findings, and Nabu FAIL
with two residuals (lockfile stanza parsing, transcript-tree bounding), both
closed by revision 4 and confirmed by the Nabu re-review PASS of revision 4
(2026-09-05). Revision 5 is a prose-currency revision only (no rule changes):
it records that the similarity/provenance audit was performed 2026-09-07 as
local evidence with independent review pending
(`machineresearch/sley-2.0/s20-780-similarity-audit-2026-09-07.md`); the
revision 4 Status wording stating that no audit has been performed is
superseded, and §3 now reads as no independent audit claimed. The remaining
gate is recorded as a remaining gate, not waived (§3).

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

Each entry carries the seven ADR-0002 fields plus a machine-readable
`**Lineage**` marker: `reuse` for a concept taken from 1.2, `reimplemented`
for a concept built from an in-repository specification, `not-applicable`
for the closing entry only. Every entry's evidence is a tracked artifact of
this repository or the frozen legacy evidence recorded in
`machineresearch/sley-2.0/01-legacy-freeze-and-authority.md`; the one
authority that is cited but not evidenced is the master goal quoted in
Boundary, which lives outside this repository.

A concept counts as **reimplemented, not reused**, when all three hold: (a)
its normative definition is an in-repository contract named in the entry;
(b) its behavior is pinned by a frozen suite, named in the entry, that runs
with no legacy source in the tree; (c) the entry's observed-evidence field
traces its lineage to that contract rather than to 1.2 behavior. A shared
generic idea (content addressing, canonical encoding) with no 1.2-derived
construction evidence is not a reused concept under this test. The closing
entry's universal claim therefore rests on three stated legs: this
criterion, the section 2 mechanics, and the section 3 audit disclaimer that
names what no check covers.

### 1.1 The frozen 1.2.0 binary artifact as a succession arm

- **Purpose**: run the predecessor binary as one arm of the succession benchmark.
- **Observed evidence**: the frozen artifact
  `sley-1.2.0-linux-x86_64.tar.gz`, SHA-256
  `b24f19c6a348751c93c9cf63f6f4154f6132796112c26f9d8c0e71324080dbc7`,
  4,611,024 bytes, recorded at the freeze.
- **Machine-native relevance**: none to the design; the artifact is evidence,
  not a component.
- **Security impact**: the artifact executes, so the S20-600 adapter stages
  it, never runs it from the archive path: it copies the pinned archive into
  a private temp directory (mode `0o700`), re-verifies digest and size,
  extracts a write-stripped stage, and execs `stage.root/bin/sley` with
  `cwd=stage.root` via `subprocess` (`shell=False`), never inside a Sley 2
  crate. Containment limits are the adapter's own reported values
  (`read_only_mount_enforced: false`,
  `network_isolation: "NOT_ENFORCED_VERSION_ONLY"`), and
  `bench/legacy/README.md` disclaims containment outright (no read-only
  mount, no network namespace); this register claims out-of-process
  execution only, not containment.
- **New equivalent**: none. Sley 2 does not reimplement the 1.2 program model.
- **Decision**: reused as an external oracle and benchmark arm only.
- **Acceptance test**: `make legacy-runner-smoke` verifies the artifact
  digest and version before any use, but it is environment-dependent: it
  needs `/home/dev/archive/sley/1.2.0/...`, outside the repository.
  What the gate runs is `scripts/check_legacy_runner.py` under `make quick`,
  which pins the adapter boundary without executing the artifact.
- **Lineage**: reuse.

### 1.2 The frozen 1.2.0 source snapshot as a provenance anchor

- **Purpose**: anchor the freeze record to the exact source the binary
  artifact was built from, so any future provenance question has a digest to
  start from.
- **Observed evidence**: the exact source snapshot digest
  `1c866d360305d0b511dc2c33c4907b33544fc73bc6cb6fa4c0e1687df48eb90e`,
  recorded at the freeze; no path matching `sley-1.2.0-source*` is tracked
  in this repository, so the snapshot itself is not in the tree.
- **Machine-native relevance**: none to the design; the digest is evidence,
  not a component.
- **Security impact**: none; a digest cannot execute, and the snapshot was
  never consulted during Sley 2 implementation: no implementation entry
  traces its lineage to 1.2 behavior, and the reimplemented entries below
  trace theirs to in-repository contracts.
- **New equivalent**: none.
- **Decision**: recorded as freeze evidence only; not consulted, not
  unpacked, not depended on.
- **Acceptance test**: the checker asserts no tracked file carries the
  snapshot filename outside the reference inventory, and no
  `sley-1.2.0-source*` path is tracked.
- **Lineage**: reuse.

### 1.3 The succession task corpus

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
- **Lineage**: reuse.

### 1.4 Stable graph identity, reimplemented

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
- **Lineage**: reimplemented.

### 1.5 Typed graph checking, reimplemented

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
- **Lineage**: reimplemented.

### 1.6 Effects and authority model, reimplemented

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
- **Lineage**: reimplemented.

### 1.7 Everything else: no reused concept

No other concept of Sley 1.2.0 is reused. Sley 2 is machine-native lineage: its
canonical encoding, program representation, type and effect systems, object
store, state roots, mutation and transaction model, policy and capability
model, VM, repository model, and protocol are specified in this repository's
contracts and implemented from those specifications. The master goal directs
exactly this ("implement Sley 2.0 from first principles"; "ship no Sley source
syntax, parser, formatter, conventional LSP, human review surface, source
compatibility, or self-hosting requirement"), and section 2 states the
mechanical facts that keep the claim true.

A future reuse requires a new entry here, with all seven fields and a
`reuse` lineage marker, and an update to the machine-summary reuse count and
entry list, before the implementation lands.

- **Lineage**: not-applicable.

## 2. Mechanical boundary

`scripts/check_clean_room_boundary.py` verifies, from the tree alone:

1. **Sentinel inventory over every tracked file.** Every tracked file (via
   `git ls-files`) is searched for the sentinel set: the frozen archive
   path, both artifact filenames, the `GreyforgeLabs/sley` origin, and both
   freeze digests. Any hit outside the exact reference inventory is a
   violation, and any inventory path that is no longer tracked is a stale
   entry. The inventory is: the adapter and its tests
   (`bench/legacy/runner.py`, `bench/legacy/tests/test_runner.py`), the
   raw-arm tests that pin the artifact digest
   (`bench/raw/tests/test_runner.py`), the corpus plan that records the arm
   (`bench/benchmark-plan.json`), the freeze record and the documents that
   record it (`machineresearch/sley-2.0/01-legacy-freeze-and-authority.md`,
   `docs/spec/LEGACY_ARTIFACT_ADAPTER_V1.md`, this register), the generated
   records that quote the freeze pins
   (`machineresearch/sley-2.0/machine-summary.json`,
   `evidence/release/decision-dossier.json`), the succession design that
   cites the freeze
   (`machineresearch/sley-2.0/s20-530-v4-semantic-amendment-design.md`), the
   boundary scripts that name the sentinels to search for them
   (`scripts/check_clean_room_boundary.py`,
   `scripts/check_legacy_runner.py`,
   `scripts/check_s20_530_crash_recovery.py`,
   `scripts/verify_s20_530_accepted_state.py`), and the review transcripts
   under `machineresearch/sley-2.0/reviews/`, which record reviews, are never
   imported, and are admitted only with non-executable transcript suffixes
   (`.log`, `.json`, `.md`). This is a sentinel search, not a source detector: a
   renamed copy, a legacy module marker, or an unpacked tree would not match
   it, and no tree-alone check could fingerprint those without the legacy
   layout. That detection is not mechanized; it rests on the per-concept
   construction evidence in section 1 and the section 3 audit disclaimer.
2. **No legacy dependency.** The root workspace manifest, every crate
   manifest, the fuzz manifest, and `Cargo.lock` are parsed for dependencies
   in inline and table form (normal, dev, and build): any `sley` 1.x
   registry version, any `GreyforgeLabs/sley` git source (manifest or
   lockfile, where each `[[package]]` stanza is parsed for name, version,
   and source), and any `sley1`/`sley-1`/`sley_1`/`legacy` name is a
   violation; every path dependency must resolve to a Sley 2 workspace crate.
   The crate directories containing manifests must equal the root workspace's
   declared `crates/*` members in both directions, so an undeclared new crate
   and a stale missing member both fail without freezing an obsolete count.
3. **Bounded touchpoint, out of process.** The tracked Python files carrying
   a sentinel are exactly the executable inventory (the adapter, its tests,
   and the boundary scripts named in check 1); anything else executable is a
   violation. The adapter must import and call `subprocess.Popen` with
   `shell=False`, and must not contain `os.system`, `ctypes`, `dlopen`,
   `os.exec`, `eval(`, or `shell=True`.
4. **The register covers its entries.** Every entry except the closing one
   carries all seven ADR-0002 fields with non-empty bodies and exactly one
   lineage marker; the closing entry carries `not-applicable` and is the
   last entry. The machine-summary reuse count equals the number of `reuse`
   entries, and its entry lists match the entry titles.

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
the three Council re-reviews (`ariadne_review`, `nabu_review`,
`vulcan_review`) read `PASS`.
