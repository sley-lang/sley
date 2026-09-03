# ADR-0034: JSON bridge as a generated, non-canonical representation

Status: proposed; the S20-420 contract is a draft at revision 2 with
Council review pending; implemented at `crates/sley-json-bridge`
(2026-09-03) with a round-trip fixture, an independent oracle, and a
persistent fuzz slice

Date: 2026-09-03

## Context

SMP1 section 8 requires a JSON bridge generated from the protocol
contracts with a declared binary encoding, preserved stable codes and
unknown or omission states, and no semantic validation, and it forbids
JSON from participating in program identity. The frozen SMP1 codec
(S20-410) and its records now exist, and the method table is frozen in
`docs/spec/SMP1.md`.

The Council lanes were still unavailable at this draft (see ADR-0026); the
design is the integrator's and is submitted to Ariadne, Nabu, and Vulcan as
soon as a lane returns.

## Decision

1. **Bytes stay canonical.** `frame_from_json` re-encodes through the
   frozen codec; the JSON text is never hashed, stored, or compared as
   identity, and every owner body crosses as opaque hex.
2. **Declared encodings, no inference.** Lowercase hex for bytes, JSON
   numbers up to 2^53 - 1 and decimal strings above, frozen names for
   enumerations, booleans for flags and features, exact object shapes.
3. **Generated method table.** The names of the bridge are the frozen
   method names; a generator reads the contract table into a fixture that
   the crate embeds and tests against the `Method` table, so the bridge
   cannot drift from the contract.
4. **States copied, codes verbatim.** Omission, truncation, continuation,
   codes, symbols, incidents, and details are copied; nothing is derived
   or collapsed.
5. **Codes.** Five `JSON_BRIDGE_*` codes 42000 through 42004 precede the
   codec's `PROTOCOL_*` codes only for resource, shape, and encoding
   failures.
6. **Staging.** `scripts/check_smp1_json_bridge_contract.py` binds the
   contract, ADR, work-package row, and summary section, and fails closed
   if the bridge crate appears before the summary allows it.

## Consequences

- S20-430 can wrap the CLI over JSON without a second semantics.
- A client that only speaks JSON still receives every code and every
  omission state the protocol defines.
