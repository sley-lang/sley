# RW-080 §1.3 lowerer slice 28: built-in switch terminator bytes

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley encoding
algorithm under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

Built-in variant-switch terminators now serialize from the validated Sley
model in the exact native order: UInt32 terminator kind, UInt32 selector,
UInt64 case count, and each case's key, target, and argument vector.

Two nested Sley walkers own the variable-length payload:

- the case walker emits the frozen built-in case-key discriminator `2`, the
  UInt32 built-in key tag, UInt32 target slot, and its argument inventory;
- the argument walker emits its UInt64 length, then UInt32 tag `1` plus a
  register for ordinary values or UInt32 tag `2` alone for case payload.

Both walkers use checked indexed reads and checked UInt64 advance. Missing
elements after a successful length guard trap as internal invariants. Helper
errors propagate unchanged, while arithmetic exhaustion returns the frozen
resource-limit code. The bootstrap lowerer accepts built-in case keys only;
member-key switch serialization remains full-profile RW-160 work.

The focused closure imports only the single UInt8 PSH1 schema and exposes its
octet vector directly.

## Native parity corpus

The native Option switch fixture contains `None` and `Some` cases, multiple
targets, ordinary register arguments, and a case-payload argument. Sley emits
it after a nonempty prefix and the result matches the native terminator bytes
exactly.

All thirty-six tests in `rw080_lower_scaffold` pass; focused Clippy with
warnings denied is clean.

## Explicit remainder

The simple and built-in switch paths must now be unified behind the complete
terminator model and composed into the full block record with reachability.
Function-body block traversal, SLEYBC02 image header/callee framing, and
execution-package assembly remain. The driver remains RW-120 work. RW-080 and
R2 stay provisional pending the recorded independent acceptance debt.
