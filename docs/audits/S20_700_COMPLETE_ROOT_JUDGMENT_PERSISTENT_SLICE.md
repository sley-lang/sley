# S20-700 Complete-Root Judgment Persistent Slice

Status: scoped persistent landed-surface slice for the full S20-250 profile; **full S20-700 remains incomplete**

This slice hardens the S20-250 full complete-root closure judgment
(`crates/sley-query/src/complete_root.rs`,
`docs/spec/COMPLETE_ENTITY_IMPACT_PROFILE_V1.md` section 6). It does not
begin merge, comparison, protocol, or release packaging, and it does not fuzz
object decoding, which the S20-700 schema and pack slices already cover.

The libFuzzer target decodes a structured eighteen-kind request from raw
bytes: an entity count, then per entity a kind, an identity byte, and the
kind's identity and set fields, then a flags byte read from the end of the
input. A set's length is one byte below four and two bytes for four through
twenty-four, which is the widest set that can name every entity a request
carries, so a seed encodes a whole fixture set rather than its first four
members. The flags select four deterministic lanes (raw versus canonical sets,
decoded versus kept entity order) and whether the three root facts come from
the decoded bodies or from further bytes, so every closure rule and every
canonicality check is reachable.

A passing judgment must be repeatable, must equal the plain `ImpactIndex`
over the same request, must cover exactly the request's entities, must name a
workspace entity of the request, and must produce no edge outside the bound
inventory. A failing judgment must carry one of the frozen `IMPACT_*` codes.
Inputs are bounded to 4,096 bytes.

The deterministic corpus comes from
`conformance/complete-entity-impact/v1/accepted.json` and `rejected.json`,
encoded into the target's grammar under all four flag lanes, plus
trailing-byte, minimal, and single-bit mutation seeds. Runtime corpus,
binaries, artifacts, and evidence remain under ignored
`evidence/runtime/s20-700-complete-root-libfuzzer/` paths.

Focused validation:

```text
cargo test -p sley-query complete_root --locked
python3 scripts/check_complete_root_persistent_fuzz_slice.py
make complete-root-persistent-fuzz-smoke
python3 scripts/run_complete_root_persistent_fuzz.py --manual
```
