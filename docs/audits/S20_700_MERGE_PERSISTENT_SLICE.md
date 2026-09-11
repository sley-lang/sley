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

## Repair round 7 (REQ-06 fuzz repair wave)

The REQ-06 REVISE findings against this slice's harness are repaired in
the runner: `--locked` builds, workspace-wide owner-lib coverage via
`-Zhost-config` + target rustflags with trace-compares/pc-table (the old
bin-only flag left zero `sancov` symbols in owner rlibs; the gate now
fails closed on zero family-wide symbols and zero symbols in the slice's
owner rlib), an on-disk coverage floor (corpus files plus 256 guaranteed
mutations, replacing seed-count enforcement, which decayed as libFuzzer
added inputs), persistent corpus directories with stale-seed sync, crash
minimization via `-minimize_crash=1` with exact artifacts (round-7c fixed
the `-merge=1` primitive, which merges corpora and cannot minimize a
crasher), executed/inline-counter-coverage/crash evidence gates, a
dedicated build timeout, and append-only artifacts. The slice proved
locally PASS with executed >= floor, inline 8-bit counters observed,
zero crash artifacts, and nonzero owner-rlib `sancov` counts; the durable
record is `machine-summary.json` `last_local_proof` (runtime
`evidence.json` files are gitignored by design). The decoder is safe
Rust, so no ASan is instrumented; the oracle and seed neighbourhood are
unchanged (bounded smoke, not a probe). The pinned qualification
toolchain is unchanged (`clang-18`, pinned libfuzzer path,
`nightly-2026-02-27`); the local proof ran under documented
`SLEY_FUZZ_CC` / `SLEY_FUZZ_LIBFUZZER_A` overrides. Re-review of the
slice's Vulcan verdict is queued, not assumed.
