# S20-530 v8 closeout proof frontier

Status: ACTIVE CONTRACT BLOCKER ANALYSIS

Owner: Codex orchestrator

Evidence date: 2026-08-31, America/New_York

## Purpose

Record the complete test-plan-independent S20-530 proof frontier discovered
after the v7 crash recovery. This packet prevents a helper-manifest-only v8
refreeze from receiving review while three dormant final-closeout
contradictions and the known checker runtime debt remain unresolved.

No frozen v7 checker, evidence, specification, ADR, runner, summary, Rust
source, or repository remote was changed while collecting this evidence.

## Execution boundary

The frozen checker was loaded read-only. Bounded probes called its existing
manifest producers and problem functions against current repository sources.
The initial probes used the exact frozen scanner and were CPU-bound in
`rust_code_mask`. They were intentionally cancelled only after an equivalent
position-based matcher was proven byte-for-byte and used by successful
replacement probes.

The in-memory performance prototype changed four exact expressions:

1. two raw-string regular-expression matches use a position in the original
   source instead of allocating `source[index:]`;
2. the corresponding raw-literal cursor and payload offsets use the returned
   absolute match end.

It changed no file and no checker policy.

## Scanner parity and runtime evidence

Both scanners were compared on all four governing owner sources:

- `crates/sley-store/src/lib.rs`;
- `crates/sley-txn/src/repository.rs`;
- `crates/sley-repo/src/refs.rs`;
- `crates/sley-repo/src/gc.rs`.

The corpus contains approximately 3.4 MB of exact source, including the 1.32
MB transaction owner and 1.84 MB ref owner.

| Scanner | Frozen seconds | Positional seconds | Speedup | Result |
|---|---:|---:|---:|---|
| `rust_code_mask` | 34.420855 | 1.699063 | 20.26x | byte-for-byte parity |
| `rust_string_literal_values` | 35.225229 | 1.739165 | 20.25x | value-for-value parity |

The exact frozen 32-source closure took roughly five minutes to reach its
first probe marker under three concurrent processes. The in-memory positional
candidate reached its 32-source marker in under 30 seconds.

## Finding A: helper-manifest schema contradiction

The previously recorded v8 design remains correct on its original finding:
the helper manifest emits 50 four-field records while
`require_implementation` requires the generated records to have five fields,
including `attribute_chain_sha256`.

The five-field producer prototype passed all bounded controls documented in
`s20-530-v8-helper-manifest-attribute-chain-amendment-design.md`.

## Finding B: incomplete feature-gate authority

The frozen raw gate registry expects seven S20 feature-gated ancestry sites
plus one `cfg(test)` ref site. The current exact source contains 14 S20
feature-gated sites. All seven expected feature sites are present, seven
additional sites are ungoverned, and no expected feature site is missing.

The seven ungoverned sites are:

1. the `recovery_path_read_test_hook` module export;
2. the `s20_530_test_hook_feature_name` function;
3. the repository path-read-hook import;
4. accepted-ancestry object-read injection;
5. branch-ancestry object-read injection;
6. recovery-receipt read injection;
7. ordinary object-load read injection.

Because `normal_build_test_only_ranges` admits only frozen exact feature-gated
items, these seven sites survive the checker-owned normal-build projection.
Consequences under v7 are exact:

- `accepted.object_read` matches nine of ten required statements, then the
  surviving path-read injection interrupts the frozen sink;
- the shared-state manifest sees the gated module and function attributes as
  procedural or unbound authority;
- the production dependency closure incorrectly includes the path-read hook
  module, producing 32 sources instead of the corrected 31-source closure.

An in-memory exact 14-site registry produced the corrected 31-source closure.
With that registry:

- all 30 control-ancestry event manifests were generated;
- shared-state authority passed with 31 source scans, two allowed channels,
  and SHA-256
  `635f38325ec9ce9ecf01f5e057b8d5c3d009ae3ccbea25bd106f35cae33611f1`.

This proves one exact registry amendment closes the original
`accepted.object_read` and shared-state blockers without a Rust change.

## Finding C: entry-path resolver frontier

After correcting the feature-gate projection, the entry-path producer emitted
all 30 event records. The problem function still rejected:

- 13 of 30 events;
- 16 call edges;
- four reverse-caller inventories.

The ref preflight paths reject exact local authorities including
`RefRecoveryScanPlan`, `RecoveryRecordPath`, mutable scan vectors, `usage`, and
their frozen path derivations. Earlier maintenance, layout, lock, filesystem,
and scan success paths also remain unresolved.

Cross-crate branch entry edges report an absent-or-repeated callee. The
accepted-recovery path omits the intentional private
`verify_accepted_recovery_ancestry` caller, so all four accepted events reject
the reverse-caller inventory.

The affected events are:

- `ref.visible_record_read`;
- `ref.visible_origin_read`;
- `ref.orphan_origin_read`;
- `branch.request_batch`;
- `accepted.ancestry_node`;
- `accepted.receipt_read`;
- `accepted.binding_visit`;
- `accepted.object_read`;
- `branch.ancestry_node`;
- `branch.actual_receipt_read`;
- `branch.actual_binding_visit`;
- `branch.actual_object_read`;
- `branch.cached_fact_use`.

These are not production implementation typos. Several implicated bodies are
already exact frozen bodies under the v7 contract. Rewriting them to fit an
incomplete static resolver would weaken the established production boundary.

## Finding D: control-ancestry resolver frontier

After correcting the feature-gate projection, the control-ancestry producer
emitted all 30 event manifests. Complete review found:

- 23 of 30 events rejected;
- 1,666 individual unresolved-control violations.

The dominant unresolved classes are:

- repository-root path joins and descendant provenance;
- maintenance, layout, lock, and filesystem helper closure;
- private struct constructors and mutable work collections;
- `DirEntry`, metadata, file-type, `Option`, `Result`, and iterator receiver
  authority;
- enum constructors and error conversions;
- worklist mutation and match-arm value authority;
- dominating success paths that precede the governed event.

This volume rules out treating the frontier as one missing trusted method or a
small resolver allowlist. A complete resolver build would be a separate major
checker subsystem, not a narrow mechanical v8 patch.

## Required design ruling

The helper digest, complete 14-site gate registry, and positional scanner are
narrow mechanical candidates. The entry and control frontiers require an
architecture and security ruling before checker implementation.

Two defensible options remain:

1. Build and adversarially validate the missing static resolver subsystem for
   all exact entry and control authorities.
2. Freeze exact manual-review exception manifests for only the current
   unresolved records, bind every record to source, owner, function,
   attribute-chain, body, call-site, and canonical manifest digests, retain all
   existing runtime observations, and require final Nabu, Ariadne, and Vulcan
   implementation reviews over the exact closeout source set.

Option 2 is the bounded recommendation because the v7 contract already assigns
these proof layers to final specialist review and the existing producers retain
the complete evidence. It must not become a broad bypass: every exception must
be exact, ordered, digest-bound, hostile-tested, and invalidated by any source
or manifest drift.

Dropping the proof layers, accepting arbitrary unresolved records, weakening
runtime observations, or changing production solely to satisfy the incomplete
resolver are rejected.

## Review and authority gate

Fresh Nabu, Ariadne, and Vulcan contract reviews are required before selecting
or implementing the entry/control ruling. Those handoffs are provider-backed
and require current operator approval. No v7 review may be reused.

The full `make v1` gate remains deferred because no release boundary has been
reached. The monolithic v7 checker remains unsuitable as a development probe
until its measured scanner debt is addressed.

## Running estimate

At this frontier:

- overall Sley 2.0 completion: 44%, moderate confidence;
- active S20-530 completion: 82%, moderate confidence.

The S20-530 estimate decreased from the helper-only prototype estimate because
the complete entry and control frontiers prove a larger contract-refreeze
scope. This is a confidence correction, not lost implementation work.
