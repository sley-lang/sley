# S20-700 Restricted-Query Request Persistent Slice

Status: scoped persistent landed-surface slice; **full S20-700 remains incomplete**

This slice adds one libFuzzer target at the public typed S20-310 restricted
query boundary. A fixed valid three-entity function snapshot is rebuilt under
rootless and claimed-root contexts. Bounded fuzz-only constructors then select
all four restricted query kinds, resolved or unresolved entity IDs, canonical
or noncanonical kind and seed sets, and zero, tight, maximum, over-maximum, or
raw bounded resource limits.

For every accepted request, the target asserts:

- `QueryId` is derived from the exact request preimage;
- the preimage remains within the frozen request byte ceiling;
- the request fails with `QUERY_SNAPSHOT_MISMATCH` against the alternate
  snapshot;
- a successful response preserves query, snapshot, context, completeness, and
  applied-limit bindings;
- returned counts, depth, response bytes, and charged work stay within the
  accepted limits;
- rebuilding and executing the same typed request produces the same result.

Input is capped at 4,096 bytes. Kind and seed sets are each capped at 16
generated entries. The deterministic synthetic corpus contains 525 seeds.
Corpus, binaries, artifacts, and command evidence remain under ignored
`evidence/runtime/s20-700-query-request-libfuzzer/` paths.

The byte mapping is a fuzz-only typed constructor, not a canonical query
decoder or serialized request contract. This slice covers only the four
restricted modeled-snapshot query kinds. It does not implement the nineteen
root-backed master-goal query classes, truncation, continuation, master context
capsules, useful cache authority, or proven root provenance.

Independent Vulcan review remains deferred because the local Forge OAuth
session returns 401. Mutation candidates, merge, and the full S20-700 finding
register remain required.

Focused validation:

```text
cargo +nightly-2026-02-27 clippy --manifest-path fuzz/Cargo.toml --bin restricted_query_request --target-dir evidence/runtime/s20-700-query-clippy-target -- -D warnings
python3 scripts/check_query_persistent_fuzz_slice.py
make query-persistent-fuzz-smoke
python3 scripts/run_query_persistent_fuzz.py --manual
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
