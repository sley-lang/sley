# S20-700 SMP1 JSON Bridge Persistent Slice

Status: scoped persistent landed-surface slice for S20-420; **full S20-700 remains incomplete**

This slice hardens the S20-420 SMP1 JSON bridge
(`crates/sley-json-bridge/src/lib.rs`, `docs/spec/SMP1_JSON_BRIDGE_V1.md`):
the reachable resource ceiling (`MAX_JSON_DEPTH`, 32; the MiB-scale text,
frame, and hello-list ceilings are unreachable under the 64 KiB harness
payload cap), the exact object shapes, the declared integer
and hex encodings, the frozen names, and the re-encoding through the frozen
`sley-protocol` codec. It does not cover the codec itself (the S20-410
slice), the server, the CLI, or the versioned exports
(`frame_to_json_for_version`, `frame_from_json_for_version`,
`hello_to_json_versioned`, `selected_to_json`, `METHOD_TABLE_V2_JSON`),
which sit outside all three lanes.

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
