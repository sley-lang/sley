# S20-420 SMP1 JSON Bridge Closeout

Status: **implemented under the draft SMP1 JSON Bridge v1 contract (revision 9); Council reviews pending, so the package is not complete; the Sley 2 goal remains incomplete**

Date: 2026-09-03; revised 2026-09-14 (revision 9: compiler-derived 4x text ceiling, allocation-free element ceiling with boundary tests, duplicate-key and hello-rendering declarations, envelope and fuzz-slice boundaries, all eighteen P1s closed or bounded)

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
  implementation at `7722d33`; revisions 3 through 5 (method tag zero, the
  `failed` flag, the eighth limit field `max_sessions`); revision 6 closes
  the four 2026-09-04 review P0s (see below).
- Native tests (`cargo test -p sley-json-bridge`): eleven tests pass and the
  fixture emitter is ignored. They cover the embedded table against
  `Method::ALL` (41 methods, 4 reserved, six families), every SMP1 fixture
  frame and both hello frames round-tripping through JSON to identical
  bytes with whitespace and order tolerance, the integer encoding on both
  sides (2^53 - 1 as a number, 2^53 and `u64::MAX` as strings, the string
  form accepted for small values), negative zero reading as the integer
  zero on both spellings, the hello header rule carrying the codec's code
  with the bridge-owned all-zero bounds staying a shape failure, the
  declared u32 fields refusing values above 2^32 - 1, the thirty-six-case
  rejection matrix in
  contract precedence including the size and depth ceilings, unknown
  method tags and unnamed flag bits refused on rendering and every frozen
  name accepted on parsing, 128 identical lexicographic renderings, and
  hello, selected profile (with the fixture's handshake identity), failure,
  chunk, and bounds round trips.
- Independent oracle: 5 vectors, 36 rejections, 41 methods, PASS.
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
- Review slice 2026-09-05 (contract revision 6, closing the four S20-420
  P0s): `-0` and `-0.0` both read as the integer zero (serde_json parses
  either spelling as negative zero, verified in the parser source; the
  reader normalizes it and the oracle's float branch agrees), pinned by
  two matrix mutations expecting the codec's version judgment plus
  field-level probes; the hello session, request id, method, and flags
  are the codec's header rule, so the reader builds the frame and calls
  the codec's new `validate_header` instead of answering shape failures,
  while the all-zero bounds stay the bridge's own shape rule (a second
  silent-rewriting hole closed alongside: a hello text's protocol version
  now reaches codec judgment instead of being dropped); precedence order
  is the record's declared field order, disambiguated from the
  lexicographic emission order both sides already produce; and integer
  widths are declared per field (the six review-named u32 fields plus
  `max_sessions` and method tags), making the pre-existing `u32-overflow`
  vector reproducible from the contract with two new width mutations.

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

Tier 1 `make quick` passed at the commit. Tier 2 ran on 2026-09-03 at
`7722d33` (`make core` 973 tests, `make conformance` 19 oracles,
`make adversarial` 596 tests, `make fuzz-smoke`, and both SMP1 persistent
smoke gates, all exit 0 in 35 seconds of wall time) and is recorded in
`machineresearch/sley-2.0/s20-420-json-bridge-campaign-2026-09-03.md`. The
full `make v1` gate was skipped because this is a subsystem handoff, not a
release boundary; `make v2` and `make release-check` remain intentionally
fail closed.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.
