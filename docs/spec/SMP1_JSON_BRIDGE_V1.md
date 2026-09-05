# SMP1 JSON Bridge v1

Status: S20-420 contract draft, revision 4 (2026-09-03); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review). Revision 2 records the clarifications found while implementing
revision 1 (section 8); revision 3 names method tag zero (section 9) for the
S20-430 endpoint; revision 4 follows SMP1 revision 6 by naming the `failed`
response flag. The implementation is `crates/sley-json-bridge`;
implementation state is tracked in the machine summary.

The bridge is a generated, non-canonical text representation of SMP1
frames and of the records SMP1 itself owns. It exists so that a client
without an SCB1 encoder can read and write frames; it owns no semantics,
performs no validation beyond shape, and never participates in any program
identity. It composes, and never alters, `docs/spec/SMP1.md` (revision 6):
the frame, hello, selected profile, limit profile, bounded context,
failure envelope, stream chunk, and method table are the bridge's only
subjects. Owner bodies (queries, capsules, candidates, receipts, exchange
bytes, and every other frozen record) cross the bridge as opaque bytes.

The authority rule (SMP1 section 8) is:

> JSON is non-canonical and cannot participate in any program identity.
> The bridge preserves stable codes and unknown or omission states, uses a
> declared binary encoding, and contains no semantic validation.

## 1. Declared encodings

- **Bytes.** Every byte string (bodies, identities, digests, names) is a
  JSON string of lowercase hexadecimal digits with an even length and no
  prefix. Uppercase, odd length, or non-hex characters are
  `JSON_BRIDGE_HEX_INVALID`.
- **Integers.** An unsigned integer whose value is at most 2^53 - 1 is a
  JSON number; a larger value is a JSON string of decimal digits without
  sign, leading zeros, or separators. A number with a fraction, exponent,
  sign, or a string that is not such a decimal is `JSON_BRIDGE_NUMBER_INVALID`.
  A reader accepts both forms for any integer field.
- **Booleans and enumerations.** Frame kinds, retryability, and features
  travel as their frozen names (below); flags as an object of booleans.
- **Objects.** Every object carries exactly the fields of its record in
  this contract; an unknown field, a missing field, or a null is
  `JSON_BRIDGE_SHAPE_INVALID`. Field order in the emitted text is
  lexicographic and the text carries no insignificant whitespace, so equal
  values emit equal text; a reader accepts any order and whitespace.

## 2. Objects

```text
Frame {
  "protocol_version": integer,
  "session": hex[64] | null,          // null only for hello, session.open,
                                      // and the sessionless creators
  "request_id": integer,
  "kind": "request" | "response" | "event" | "hello",
  "method": string,                   // the frozen method name, or "" for tag 0
  "flags": { "cancel": bool, "stream": bool, "failed": bool },
  "bounds": BoundedContext,
  "body": hex
}
LimitProfile {
  "max_frame_bytes", "max_entities", "max_edges", "max_depth",
  "max_response_bytes", "max_work", "max_inflight": integer
}
BoundedContext {
  "applied_limits": LimitProfile,
  "returned_bytes", "returned_entities", "returned_edges",
  "reached_depth", "omitted": integer,
  "truncated": bool, "continuation": bool
}
Hello {
  "protocol_versions": [integer], "schema_epochs": [hex[64]],
  "limits": LimitProfile, "methods": [string],
  "features": { "cancel", "stream", "json_bridge", "checksum": bool },
  "adapters": [hex[64]], "effects": [hex[64]]
}
SelectedProfile {
  "protocol_version": integer, "schema_epoch": hex[64],
  "limits": LimitProfile, "methods": [string],
  "features": { ... as Hello ... }, "adapters": [hex[64]], "effects": [hex[64]],
  "handshake_id": hex[64]
}Failure {
  "code": integer, "symbol": string, "phase": integer,
  "retryability": "never" | "after_requery" | "after_capability"
                  | "after_limit_change" | "transient_host",
  "incident": hex[64] | null, "details": hex
}
StreamChunk { "index": integer, "total": integer, "bytes": hex }
```

The `handshake_id` renders the transcript-bound identity of SMP1 section
2 (both hello bodies plus the selection preimage) as opaque data. The
bridge defines nothing about the handshake: identity always comes from
the wire transcript, never from this text, and there is no
`selected_from_json` reader by contract.

`method` names are the frozen names of the SMP1 method table (`session.open`
through `report`); the generated table `conformance/smp1-json-bridge/v1/methods.json`
lists every name with its tag, family, and reserved flag, and the crate
embeds and tests it against the frozen table. A name outside the table is
`JSON_BRIDGE_METHOD_UNKNOWN`; the bridge never invents a tag.

## 3. Operations

```text
frame_to_json(bytes)   : decode the SMP1 frame with the frozen codec under
                         the absolute ceiling, then emit Frame
frame_from_json(text)  : parse Frame, then encode with the frozen codec;
                         the bytes are the only canonical form
hello_to_json / hello_from_json, failure_to_json / failure_from_json,
chunk_to_json / chunk_from_json, selected_to_json
```

`frame_from_json` re-encodes through `sley-protocol`, so a frame that
would be invalid on the wire fails with its `PROTOCOL_*` code, never with a
bridge code; the bridge adds shape and encoding failures only. A JSON text
larger than 268,435,456 bytes, or nested deeper than 32 levels, is
`JSON_BRIDGE_RESOURCE_LIMIT` before parsing.

## 4. Unknown and omission states

The bridge copies `omitted`, `truncated`, and `continuation` from the
bounded context and never derives them; a failure keeps its exact `code`
and `symbol`; an `incident` digest and `details` bytes are preserved
verbatim. Nothing in the bridge maps a code to a message or collapses
codes into classes.

## 5. Stable failures

| Numeric | Symbolic code |
|---:|---|
| 42000 | `JSON_BRIDGE_SHAPE_INVALID` |
| 42001 | `JSON_BRIDGE_NUMBER_INVALID` |
| 42002 | `JSON_BRIDGE_HEX_INVALID` |
| 42003 | `JSON_BRIDGE_METHOD_UNKNOWN` |
| 42004 | `JSON_BRIDGE_RESOURCE_LIMIT` |

Precedence: resource (`JSON_BRIDGE_RESOURCE_LIMIT`), then parse and shape
(`JSON_BRIDGE_SHAPE_INVALID`), then field encodings (`JSON_BRIDGE_NUMBER_INVALID`,
`JSON_BRIDGE_HEX_INVALID`, `JSON_BRIDGE_METHOD_UNKNOWN`) in field order,
then the frozen codec's `PROTOCOL_*` codes.

## 6. Required evidence

- the generated method table, drift-gated against `docs/spec/SMP1.md` and
  tested against the frozen `Method` table;
- round-trip vectors: every frame of `conformance/smp1/v1` rendered as JSON
  and parsed back to the identical bytes, with an independent Python
  rendering from the fixture bytes;
- the number, hex, shape, and method failure matrix and the precedence
  over `PROTOCOL_*` codes;
- 128 equal renderings producing identical text;
- an S20-700 persistent target over `frame_from_json`;
- Tier 1 plus Tier 2 validation, and the Ariadne, Nabu, and Vulcan reviews
  with every report-grade finding closed.

## 7. Explicit exclusions

This contract does not claim: JSON forms of owner bodies (queries,
capsules, candidates, receipts, packs), which stay hex; JSON Schema
publication; streaming JSON over a transport; the CLI (S20-430); runtime,
benchmark, packaging, release, or GA.

## 8. Revision 2 clarifications

- A fixed-length byte field (`hex[64]`) whose hex is valid but of another
  length is `JSON_BRIDGE_SHAPE_INVALID`; `JSON_BRIDGE_HEX_INVALID` names
  only uppercase, odd-length, and non-hex text.
- A `hello` Frame carries a null `session`, a zero `request_id`, an empty
  `method`, both flags false, and the all-zero bounds; any other value is
  `JSON_BRIDGE_SHAPE_INVALID`. Its `body` is the hello record, which the
  reader decodes and re-encodes through the codec (`PROTOCOL_*` on
  failure).
- Rendering a frame whose method tag is outside the table, or whose flag
  or feature bits have no frozen name, fails with
  `JSON_BRIDGE_METHOD_UNKNOWN` or `JSON_BRIDGE_SHAPE_INVALID`; the bridge
  never invents a name.
- A frozen-name field (`kind`, `retryability`) with an unlisted string is
  `JSON_BRIDGE_SHAPE_INVALID`; a JSON type other than the declared one
  (a boolean for an integer, a number for hex) is
  `JSON_BRIDGE_SHAPE_INVALID` before any encoding check.
- The JSON text `-0` reads as the integer zero; every other signed form is
  `JSON_BRIDGE_NUMBER_INVALID`.
- `hello_to_json` and `failure_to_json` validate through the codec first,
  so a value the codec would not encode fails with its `PROTOCOL_*` code.

## 9. Revision 3: method tag zero and the failure envelope

- Method tag zero is the frozen "no method" value of hello frames and of
  the frame-level failure responses the server answers without a session
  (SMP1 section 3, S20-410). It renders as the empty name on every frame
  kind and the empty name reads as tag zero; the bridge invents nothing,
  and a request naming it is judged by the server, not the bridge.
- `BridgeError::envelope` is the failure envelope an endpoint answers with
  when a text cannot be bridged: the bridge's or the codec's numeric code
  and symbol, phase zero, `never`, no incident, no details. The bridge
  still contains no semantic validation; the envelope only names the
  bridge's own failure in the codec's record.
