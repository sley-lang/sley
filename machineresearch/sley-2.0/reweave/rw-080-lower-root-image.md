# RW-080 §1.3 lowerer slice 33: root SLEYBC02 image bytes

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley lowering and
encoding composition under the operator development override. It is not
RW-110 completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

One Sley entry now emits the canonical outer `SLEYBC02` image for a complete
root function with an empty transitive-callee table. It composes, in order:

- the exact eight-byte `SLEYBC02` magic;
- the big-endian UInt32 image version `1`;
- the complete semantic lowering and byte encoding of the root body; and
- the big-endian UInt64 callee count `0`.

The root body still passes through the complete Sley semantic lowerer before
encoding. All fixed-width fields and byte chunks are appended through the
existing Sley helpers, and the resulting octet vector is closed through V2B1.
Bridge refusal maps to the frozen `VM_LOWER_RESOURCE_LIMIT` value; errors from
the body and append helpers propagate unchanged.

The closure uses exactly B2V1, the single UInt8 PSH1 schema, and V2B1. The
test-only execution allowance is 100,000 dispatched instructions and 1,000,000
fuel because the full semantic and serialization closure exceeds the earlier
10,000-instruction fixture limit; all other fixture limits remain unchanged.

## Native parity corpus

The native three-block Option-switch function is lowered through the native
reference solely to produce the comparison oracle. The Sley entry independently
lowers its runtime facts and emits the whole image. Its returned bytes compare
exactly with `LoweredFunction::bytes`, covering the magic, version, full root
body, and empty callee table boundary.

All forty-one tests in `rw080_lower_scaffold` pass; focused Clippy with warnings
denied is clean.

## Explicit remainder

This bounded slice deliberately accepts no callee inventory. Encoding the
ordered nonempty transitive-callee table, assembling the execution package,
and removing the temporary duplicate body/header lowering pass remain. The
driver remains RW-120 work. RW-080 and R2 stay provisional pending the recorded
independent acceptance debt.
