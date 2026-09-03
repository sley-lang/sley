# S20-420 SMP1 JSON Bridge Campaign (2026-09-03)

Status: contract draft revision 1 written by the integrator with every
Council lane unavailable; Ariadne contract review, Nabu architecture
review, and Vulcan surface review queued.

## Frontier at start

- SMP1 revision 5 is implemented (S20-410, S20-440, S20-330): frames,
  hello, selected profile, limit profile, bounded context, failure
  envelope, stream chunks, and a forty-one-method table.
- SMP1 section 8 requires a generated JSON bridge with a declared binary
  encoding, preserved stable codes and omission states, no semantic
  validation, and no participation in program identity.
- The local completion frontier names S20-420 as the next authority-safe
  package and its guard blocks `crates/sley-json-bridge` until the summary
  allows it.

## Design brief

Contract: `docs/spec/SMP1_JSON_BRIDGE_V1.md`, ADR-0034, stage checker
`scripts/check_smp1_json_bridge_contract.py`, method-table generator
`scripts/generate_smp1_json_bridge_table.py` (drift-gated in `make quick`).

- Lowercase hex for bytes; JSON numbers up to 2^53 - 1 and decimal strings
  above; frozen names for kinds, retryability, and features; exact object
  shapes with lexicographic emission.
- `frame_from_json` re-encodes through the frozen `sley-protocol` codec, so
  the bytes stay the only canonical form and wire failures keep their
  `PROTOCOL_*` codes.
- Method names come from the generated table
  `conformance/smp1-json-bridge/v1/methods.json`, embedded and tested
  against the frozen `Method` table.
- Codes 42000 through 42004 (shape, number, hex, method, resource limit).

## Open questions for the reviews

- Whether the bridge should also render the S20-440 body records (cancel,
  stream, budget) as objects instead of hex.
- Whether decimal strings for large integers should be accepted for every
  integer field or only for the frame's request identity.
- Whether the resource ceiling (268,435,456 bytes, depth 32) should follow
  the negotiated frame ceiling instead of a fixed value.

## Records

| Stage | Commit | Tier 1 | Notes |
|---|---|---|---|
| Contract draft revision 1 | pending | pending | ADR-0034, checker, generator, method table |
