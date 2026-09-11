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
`SLEY_FUZZ_CC` / `SLEY_FUZZ_LIBFUZZER_A` overrides, and the pinned
qualification default itself has no recorded proof on this host (the
evidence `toolchain_versions` field captures exactly what ran).
Re-review of the slice's Vulcan verdict is queued, not assumed.

## Rounds 7c-7i (REQ-06 re-review wave)

Crash minimization uses `-minimize_crash=1` with exact artifacts (the
round-7 `-merge=1` primitive could not minimize a crasher); the coverage
floor measures on-disk corpus files plus 256 mutations; coverage gates
strictly on inline counters with monotonic `ft` (no silent fallback);
the owner gate counts the rlibs cargo linked (fingerprint-authoritative,
`rlib_linkage` recorded) with a newest-per-crate fallback; warnings are
captured from the full streams against an explicit allowlist; builds
refuse ambient `RUSTFLAGS`; prior crashers re-execute every smoke
(crash-to-regression); per-input `-timeout=30` and `-rss_limit_mb=2048`
bound hangs; libFuzzer seeds are recorded; build provenance
(`build_locked`, `sancov_scope`) derives from the executed argv; the
fuzz profile enables overflow checks and debug assertions. Manual
campaigns remain operator exploration (no floor/limits parity by
design). Slice oracles were strengthened per verdict (engine-invariant
asserts, must-reject refusals, narrowed Err arms, constructed-valid
import/decode lanes); the pinned clang-18 default has no recorded proof
on this host.
