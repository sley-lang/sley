# S20-700 Semantic-Delta Decoder Persistent Slice

Status: scoped persistent landed-surface slice for S20-510; **full S20-700 remains incomplete**

This slice hardens the S20-510 semantic-delta decoder
(`crates/sley-repo/src/compare.rs`, `docs/spec/SEMANTIC_COMPARISON_V1.md`).
It does not begin merge, conflict objects, protocol, or release packaging;
the merge engine remains the one absent Section 18.5 surface.

The libFuzzer target has two deterministic input lanes:

- direct bytes exercise the exact envelope, version, contract tag, epoch,
  trailer, record shape, canonical order, duplicate, class shape, kind tag,
  and resource rules;
- rehashed bytes rewrite only the final `SemanticDeltaId` trailer so
  mutations reach the payload rules instead of stopping at the outer digest.

Both lanes are bounded to 65,536 payload bytes. An accepted delta must
round-trip byte for byte, bind the exact derived identity, and re-encode
from its decoded form to the same stored record. A rejected input must carry
one of the eleven frozen `COMPARE_*` codes.

The deterministic corpus comes from
`conformance/semantic-comparison/v1/accepted.json` (all nine corpus deltas)
and `rejected.json`, plus truncation, trailing-byte, and single-bit mutation
seeds in both lanes. Runtime corpus, binaries, artifacts, and evidence remain
under ignored `evidence/runtime/s20-700-semantic-delta-libfuzzer/` paths.

Focused validation:

```text
cargo test -p sley-repo compare --locked
python3 scripts/check_semantic_delta_persistent_fuzz_slice.py
make semantic-delta-persistent-fuzz-smoke
python3 scripts/run_semantic_delta_persistent_fuzz.py --manual
```
