# S20-700 Complete-Root Snapshot Decoder Persistent Slice

Status: scoped persistent landed-surface slice for the full S20-300 profile; **full S20-700 remains incomplete**

This slice hardens the arm-2 complete-root index snapshot decoder
(`crates/sley-query/src/snapshot.rs`,
`docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md`). The restricted
arm-1 decoder keeps its own coverage through the restricted query target;
this target accepts only arm `2` and treats every other arm as a rejection.
It does not begin root-backed queries, capsules, sessions, protocol, or
release packaging.

The libFuzzer target has two deterministic input lanes:

- direct bytes exercise the exact magic, version, profile, option, context,
  arm, inventory kind, edge, reverse-group, length, and trailer rules;
- rehashed bytes rewrite only the final `IndexSnapshotId` trailer so
  mutations reach the structural rules instead of stopping at the outer
  digest.

Both lanes read the expected context from the candidate's own header (the
schema epoch and the bound root after option tag `2`), so a context failure
is always a real mismatch. Both lanes are bounded to 65,536 payload bytes.
An accepted record must equal its decoded bytes, carry arm `2` and a bound
root, bind the exact derived identity, and decode again to the same
snapshot. A rejected input must carry one of the eleven frozen
`INDEX_SNAPSHOT_*` codes 30000 through 30010.

The deterministic corpus comes from
`conformance/complete-root-index-snapshot/v1/accepted.json` (the frozen
eighteen-kind arm-2 record) and `rejected.json`, plus header-boundary
truncation, trailing-byte, and single-bit mutation seeds in both lanes.
Runtime corpus, binaries, artifacts, and evidence remain under ignored
`evidence/runtime/s20-700-complete-root-snapshot-libfuzzer/` paths.

Focused validation:

```text
cargo test -p sley-query snapshot --locked
python3 scripts/check_complete_root_snapshot_persistent_fuzz_slice.py
make complete-root-snapshot-persistent-fuzz-smoke
python3 scripts/run_complete_root_snapshot_persistent_fuzz.py --manual
```
