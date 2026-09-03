# S20-420 SMP1 JSON Bridge Closeout

Status: **implemented under the draft SMP1 JSON Bridge v1 contract (revision 2); Council reviews pending, so the package is not complete; the Sley 2 goal remains incomplete**

Date: 2026-09-03

Validation tier: **Tier 1 plus protocol-focused Tier 2 handoff**

## Claim under review

The JSON bridge is a generated, non-canonical text representation of SMP1
frames and of the records SMP1 owns (hello, selected profile, limit
profile, bounded context, failure envelope, stream chunk). Bytes stay the
only canonical form: `frame_from_json` re-encodes through the frozen
`sley-protocol` codec, nothing in the bridge is hashed, stored, or compared
as identity, and every owner body crosses as opaque hex. The encodings are
declared, never inferred: lowercase hex for bytes, JSON numbers up to
2^53 - 1 and decimal strings above, frozen names for kinds, retryability,
and features, and exact object shapes emitted in lexicographic order
without insignificant whitespace. Method names come from a table generated
from the frozen SMP1 method table, drift-gated in `make quick` and tested
against the crate's `Method` table. The bridge copies omission,
truncation, continuation, codes, symbols, incidents, and details verbatim
and performs no semantic validation. The contract is
`docs/spec/SMP1_JSON_BRIDGE_V1.md` with ADR-0034. It is a draft: every
Council lane was unavailable when it was written and when the
implementation landed, so the Ariadne, Nabu, and Vulcan reviews that freeze
it and complete the package are pending and must pass before the status
above changes.

The implementation provides:

- `sley-json-bridge` (`crates/sley-json-bridge/src/lib.rs`):
  `JsonBridgeErrorCode` with the five codes 42000 through 42004,
  `BridgeError` carrying either a bridge code or the codec's `ProtocolError`
  verbatim, `check_resources` (268,435,456-byte and depth-32 ceilings before
  parsing), `frame_to_json`, `frame_from_json`, `frame_value`,
  `frame_from_value`, `hello_to_json`, `hello_from_json`,
  `selected_to_json`, `failure_to_json`, `failure_from_json`,
  `chunk_to_json`, `chunk_from_json`, `method_table`, `method_by_name`, and
  the embedded `METHOD_TABLE_JSON`;
- the generated method table `conformance/smp1-json-bridge/v1/methods.json`
  (`scripts/generate_smp1_json_bridge_table.py --check` in `make quick`);
- the round-trip fixture `conformance/smp1-json-bridge/v1/roundtrip.json`
  (the five SMP1 fixture frames rendered as JSON) and `rejected.json` (the
  thirty-one-case rejection matrix), refreshed by
  `scripts/generate_smp1_json_bridge_fixtures.py` and drift-gated in
  `make quick`;
- the independent oracle `scripts/check_smp1_json_bridge_vector.py` in
  `make conformance`, which renders every fixture frame from its bytes with
  its own SMP1 record reader, parses the text with its own bridge reader,
  re-encodes with the SMP1 oracle's encoders to the identical bytes, and
  classifies every rejection in contract precedence;
- the S20-700 persistent slice `fuzz/targets/smp1_json_bridge.rs`
  (`make smp1-json-bridge-persistent-fuzz-smoke`,
  `docs/audits/S20_700_SMP1_JSON_BRIDGE_PERSISTENT_SLICE.md`).

## Evidence

- Contract draft revision 1 and ADR-0034 at `be9843c`; revision 2 and the
  implementation in the commit recorded in the campaign record.
- Native tests (`cargo test -p sley-json-bridge`): eight tests pass and the
  fixture emitter is ignored. They cover the embedded table against
  `Method::ALL` (41 methods, 4 reserved, six families), every SMP1 fixture
  frame and both hello frames round-tripping through JSON to identical
  bytes with whitespace and order tolerance, the integer encoding on both
  sides (2^53 - 1 as a number, 2^53 and `u64::MAX` as strings, the string
  form accepted for small values), the thirty-one-case rejection matrix in
  contract precedence including the size and depth ceilings, unknown
  method tags and unnamed flag bits refused on rendering and every frozen
  name accepted on parsing, 128 identical lexicographic renderings, and
  hello, selected profile (with the fixture's handshake identity), failure,
  chunk, and bounds round trips.
- Independent oracle: 5 vectors, 31 rejections, 41 methods, PASS.
- Persistent fuzz smoke: 631 deterministic seeds across three lanes, 632
  runs, PASS in 12.9 seconds; evidence under
  `evidence/runtime/s20-700-smp1-json-bridge-libfuzzer/`.
- `cargo clippy -p sley-json-bridge --all-targets --no-deps -- -D warnings`
  is clean; `make quick` green at the commit.
- Tier 2: see the validation record below.

## Findings closed in flight

- Revision 2 of the contract records the clarifications the
  implementation forced: a valid-hex fixed-length field of another length
  is a shape failure, the hello Frame's fixed field values are a shape rule,
  rendering refuses unknown method tags and unnamed bits, unlisted frozen
  names and wrong JSON types are shape failures before encoding checks,
  `-0` reads as zero, and the hello and failure renderers validate through
  the codec first.
- The contract's rejection precedence (resource, then parse and shape,
  then field encodings in field order, then `PROTOCOL_*`) is exercised
  explicitly by three ordered cases in the matrix.

## Explicitly open and deferred

- **Council reviews.** Ariadne, Nabu, and Vulcan reviews land as contract
  revisions; the campaign record lists the open questions (object forms
  for the S20-440 body records, the string form for every integer field,
  and a negotiated rather than fixed text ceiling).
- The bridge renders only the records SMP1 owns; owner bodies stay hex by
  contract, so a JSON-only client still needs the owner codecs to read a
  query response or a receipt.
- `selected_to_json` has no reader by contract: the selected profile is
  derived by negotiation, never supplied.

## Validation record

Tier 1 `make quick` passed at the commit. Tier 2 is recorded in
`machineresearch/sley-2.0/s20-420-json-bridge-campaign-2026-09-03.md`. The
full `make v1` gate was skipped because this is a subsystem handoff, not a
release boundary; `make v2` and `make release-check` remain intentionally
fail closed.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.
