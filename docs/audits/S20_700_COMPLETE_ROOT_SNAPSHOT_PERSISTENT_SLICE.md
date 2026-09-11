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
is always a real mismatch. Both lanes are bounded to 65,536 payload bytes,
so the record-length `ResourceLimit` path (64 MiB) is unreachable under
this harness while the count-based `ResourceLimit` is reachable.
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

## Rounds 7c-7m (REQ-06 re-review wave)

Crash minimization uses `-minimize_crash=1` with exact artifacts (the
round-7 `-merge=1` primitive could not minimize a crasher); the coverage
floor measures on-disk corpus files plus 256 mutations; coverage gates
strictly on inline counters with monotonic `ft` (no silent fallback);
the owner gate counts the rlibs cargo linked (fingerprint-authoritative,
`rlib_linkage` recorded, fail-closed with no mtime fallback); warnings are
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

## Rounds 7k-7m (second re-review wave)

The linked-rlib replay carries the build environment (an env-less replay
rebuilt and relinked the binary after the recorded build); the mtime
fallback is deleted, so a failed cargo query fails the gate instead of
passing on unknown provenance. Crash gating distinguishes new crashes
from retested priors (fixed priors pass with a recorded retest). The
minimize step skips clean-retested artifacts, bounds internal steps, and
keeps partial exact artifacts; durations compute last. Slice oracles
gained engine-invariant asserts, must-reject refusals, narrowed Err
arms, constructed-valid lanes, and a server-fixture validity gate with
a unit-level negotiation self-check; filed regressions replay as corpus
seeds. See the wave's decision packets for elevated owner items.
