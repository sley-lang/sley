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
and per-record exception digests other than the two ledger headers are
unchanged and were compared against the v8 frontier values before acceptance.
V10 refreezes the specification, ADR, checker, aggregate contract-set digest,
freeze evidence, and test-plan binding. The immutable v8 and v9 freeze
evidence and receipts remain byte-for-byte unchanged as historical authority.
Fresh Nabu, Ariadne, and Vulcan freeze receipts are required, followed by
fresh implementation receipts after captured Tier 2 validation.
