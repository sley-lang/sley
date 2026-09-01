# S20-530 v10 cor07 helper environment amendment

Status: candidate contract amendment

## Problem

The first captured v9 closeout execution failed one mapped test.
`refs::tests::cor07_origin_ancestry_binding_semantic_ref_io` applied its
`non_regular` corruption by spawning an external `mkfifo` binary. The frozen
execution environment pins `PATH` to the isolated tool directory, which
provisions no `mkfifo`, so `Command::status()` failed with `NotFound` at first
spawn. The helper had only ever run in developer shells: the v8 closeout
stopped at the archive-mode gate before any mapped test executed. This was the
only external-binary spawn in the three owning crates.

## Decision

V10 changes only the test-only corruption helper
`apply_non_regular_inventory_entry_corruption` in
`crates/sley-repo/src/refs.rs`. It now plants a Unix-domain socket through the
standard library with the existing `plant_non_regular_socket` staged-rename
helper, the same technique the captured environment already executes in the
passing `cor04_non_regular_fails_closed`. Production recovery classification
asserts `!is_file()`, so a socket exercises the same `RefIo` rejection as a
fifo. No production code, matrix row, mapped-test map, reconciler, or
partition count changes.

## Rejected alternatives

- Provisioning `mkfifo` into the isolated tool directory would broaden the
  frozen execution surface to repair a test-only convenience.
- Adding a `libc` or `nix` dev-dependency for `mkfifo(3)` would change the
  lockfile and dependency closure for the same test-only convenience.
- Waiving the mapped test would remove a required matrix leaf.

## Re-freeze scope

The repaired source bytes rebind the frozen scanner parity for
`crates/sley-repo/src/refs.rs` (length, code mask, one removed string
literal), the scanner contract digest, the limit source-set digest, both
exception-ledger digests, and the ordered exception-partition fingerprint. All
entry and control counts, complete manifests, inventories, static-pass sets,
event ordering, and every non-binding exception record field are unchanged
against the v8 frontier. The 12 entry and 297 control exception records whose
`source` is `crates/sley-repo/src/refs.rs` rebind `source_sha256` and
therefore `canonical_leaf_sha256` to the repaired bytes; no record with any
other source changed. Consequently 16 per-event ordered exception digests
(9 entry events, 7 control events), both exception-set aggregates
(`entry_exceptions_sha256`, `control_exceptions_sha256`), both ledger digests,
and the ordered partition fingerprint moved while the exception semantics are
identical. This was verified by deterministic record-by-record comparison of
the v9-bound and v10-bound test plans; an earlier draft of this paragraph
wrongly stated that per-record exception digests were unchanged.
V10 refreezes the specification, ADR, checker, aggregate contract-set digest,
freeze evidence, and test-plan binding. The immutable v8 and v9 freeze
evidence and receipts remain byte-for-byte unchanged as historical authority.
Fresh Nabu, Ariadne, and Vulcan freeze receipts are required, followed by
fresh implementation receipts after captured Tier 2 validation.
