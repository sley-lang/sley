# S20-700 Merge Persistent Slice

Status: scoped persistent landed-surface slice for S20-520, the eleventh Section 18.5 surface (merge engine); **full S20-700 remains incomplete**

This slice hardens the S20-520 merge engine's decoding and ancestor
surfaces (`crates/sley-repo/src/merge.rs`, `docs/spec/MERGE_V1.md`): the
canonical conflict decoder and the exact common-ancestor rule. The merge
judgment itself runs over three complete roots whose objects the frozen
S20-390 loader verifies; its adversarial coverage comes from the frozen
S20-250 complete-root judgment target, the S20-510 delta decoder target,
and the deterministic merge corpus, not from a synthetic-root harness.

The libFuzzer target has three deterministic input lanes:

- direct bytes exercise the exact envelope, version, contract tag, epoch,
  trailer, record shape, canonical order, empty-set, reason, kind, and
  field rules of the conflict decoder;
- rehashed bytes rewrite only the final `MergeConflictId` trailer so
  mutations reach the payload rules instead of stopping at the digest;
- ancestor bytes decode two head-first ancestries and check that the
  common ancestor is the first entry of ours that theirs contains, or that
  no entry is shared.

Inputs are bounded to 65,536 payload bytes. An accepted conflict must
round-trip byte for byte, bind the exact derived identity, carry at least
one entry, and re-encode from its decoded form to the same record. A
rejected input must carry one of the fourteen frozen `MERGE_*` codes.

The deterministic corpus comes from `conformance/merge/v1/accepted.json`
(every conflict vector's stored bytes) and `rejected.json`, plus
truncation, trailing-byte, single-bit mutation, and ancestor seeds in all
three lanes. Runtime corpus, binaries, artifacts, and evidence remain under
ignored `evidence/runtime/s20-700-merge-libfuzzer/` paths.

## Judgment lane

A second target, `fuzz/targets/merge_judgment.rs`, covers the merge judgment
itself: the first input byte gates the lane (only `0x00` runs), and the rest
is a mutation script of up to sixteen two-byte ops. Each op names a side by
its high bit and an entity slot (`4`, `6`, `16`, `18`, `19`) plus a mutation
(primary field toggle, label set, label clear, or no-op) applied to one fixed
valid base root. Both sides stay encodable by construction; projectability
is left to the judgment, so scripts also reach the extraction-failure path.

Every script must judge deterministically to a well-formed outcome — a
merged root that repeats its root, entity set, and override report, or a
conflict that repeats its bytes and round-trips through the strict decoder
— or to a failure carrying one of the fourteen frozen `MERGE_*` codes. A
panic, a divergent repeat, or an undiagnosed error is a crash.

Seeds are deterministic scripts (every single-mutation script on each side
plus two-sided disjoint, convergent, conflicting, collateral, and
metadata-edit combinations and the empty script) under ignored
`evidence/runtime/s20-700-merge-judgment-libfuzzer/` paths. The judgment
lane runs inside `make merge-persistent-fuzz-smoke` alongside the
decoder lane.

Focused validation:

```text
cargo test -p sley-repo merge --locked
python3 scripts/check_merge_persistent_fuzz_slice.py
make merge-persistent-fuzz-smoke
python3 scripts/run_merge_persistent_fuzz.py --manual
```
