# S20-700 SMP1 JSON Bridge Persistent Slice

Status: scoped persistent landed-surface slice for S20-420; **full S20-700 remains incomplete**

This slice hardens the S20-420 SMP1 JSON bridge
(`crates/sley-json-bridge/src/lib.rs`, `docs/spec/SMP1_JSON_BRIDGE_V1.md`):
the text resource ceilings, the exact object shapes, the declared integer
and hex encodings, the frozen names, and the re-encoding through the frozen
`sley-protocol` codec. It does not cover the codec itself (the S20-410
slice), the server, or the CLI.

The libFuzzer target has three deterministic input lanes:

- text bytes parse as a `Frame` object; every accepted text must encode to
  bytes that render to text which parses back to the identical bytes and
  frame identity, and renders identically again;
- frame bytes decode with the frozen codec and render as a `Frame` object;
  every accepted frame must parse back to the identical bytes;
- record bytes parse as `Hello`, `Failure`, and `StreamChunk` objects, and
  every accepted value must render and parse back to itself.

Every rejection must carry one of the five frozen `JSON_BRIDGE_*` codes
42000 through 42004 or one of the twelve `PROTOCOL_*` codes 40000 through
40011; no other failure and no panic is admissible.

The deterministic corpus comes from
`conformance/smp1-json-bridge/v1/roundtrip.json` (the five rendered
fixture frames and their bytes), `rejected.json` (the thirty-one rejection
texts), a hello, failure, and chunk record in every declared form, plus
truncation, trailing-byte, and single-bit mutation seeds in every lane.
Runtime corpus, binaries, artifacts, and evidence remain under ignored
`evidence/runtime/s20-700-smp1-json-bridge-libfuzzer/` paths.

Focused validation:

```text
cargo test -p sley-json-bridge --locked
python3 scripts/check_smp1_json_bridge_persistent_fuzz_slice.py
make smp1-json-bridge-persistent-fuzz-smoke
python3 scripts/run_smp1_json_bridge_persistent_fuzz.py --manual
```
