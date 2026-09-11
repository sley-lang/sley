# S20-700 Context Capsule Builder Persistent Slice

Status: scoped persistent landed-surface slice for the full S20-320 profile; **full S20-700 remains incomplete**

This slice hardens the full S20-320 context capsule builder
(`crates/sley-query/src/context_capsule.rs`,
`docs/spec/CONTEXT_CAPSULE_PROFILE_V1.md`): source binding, fact
dictionaries, omission and continuation status, and the `SLEYCCP1` record
encoder. The restricted S20-320 capsule keeps its own coverage. It does not
begin sessions, protocol, or release packaging.

The libFuzzer target reuses the root-backed query engine grammar: a
structured eighteen-kind root that passes closure rules C1 through C11, an
arm-2 snapshot, and a typed query with limits, continuation flag, and
cursor. Every query the engine answers must capsule, or fail only on a
resource ceiling; every capsule must carry the query identity and class,
strictly ordered entity and root dictionaries with one kind per entity,
in-range relationship and table indexes, `omitted` equal to
`total_count - returned`, and truncation exactly when a next cursor exists.
A capsule is `Complete` only for an untruncated first page with nothing
omitted, and the walk of page capsules keeps one exact total and
never presents a page as complete. Every capsule must be repeatable byte for
byte. The target also covers the `Negotiated` arm encoding through the
authority-delegated primitive: the same answered pair under a fuzz-derived
session carries the session, changes the identity, and keeps the facts.
Session provenance verification is not fuzzed here; it belongs to
`SessionAuthority::bind_context_capsule`, which the target cannot mint by
construction.

The deterministic corpus seeds every class over a minimal complete root
with and without continuation across limit and cursor spreads, plus raw
byte spreads that exercise the root decoder and judgment rejections; it is
checked against `conformance/context-capsule/v1/accepted.json` for contract
and vector-count drift. Runtime corpus, binaries, artifacts, and evidence
remain under ignored `evidence/runtime/s20-700-context-capsule-libfuzzer/`
paths.

Focused validation:

```text
cargo test -p sley-query context_capsule --locked
python3 scripts/check_context_capsule_persistent_fuzz_slice.py
make context-capsule-persistent-fuzz-smoke
python3 scripts/run_context_capsule_persistent_fuzz.py --manual
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
