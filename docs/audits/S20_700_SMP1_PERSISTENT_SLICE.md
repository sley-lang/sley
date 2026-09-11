# S20-700 SMP1 Frame Decoder Persistent Slice

Status: scoped persistent landed-surface slice for S20-410; **full S20-700 remains incomplete**

This slice hardens the S20-410 SMP1 frame decoder, hello decoder, and
derived negotiation (`crates/sley-protocol/src/lib.rs`,
`docs/spec/SMP1.md`). It does not cover the deterministic server dispatch,
cancellation and streaming semantics (S20-440), the JSON bridge, or the
explicit-version V2 negotiation path (deferred; `negotiate_identity`
only).

The libFuzzer target has four deterministic input lanes:

- direct bytes exercise the length prefix against the ceiling, the SCB1
  envelope magic, version, contract tag, epoch, digest trailer, the frame
  record shape, kinds, flags, bounded context, and hello body rules;
- rehashed bytes rewrite only the final `ProtocolFrameId` trailer and the
  length prefix so mutations reach the record rules instead of stopping at
  the outer digest;
- bare hello bytes decode as a `Hello` record (rejected inputs stay on
  the rejection arm) while framed hello records decoded in the frame lane
  run the shared negotiation oracle, asserting a repeatable
  `ProtocolHandshakeId` and a selection drawn only from the intersections;
- stream bytes decode as an S20-440 chunk record and reassemble, or are
  split under a small ceiling and reassembled to the same body.

Every accepted frame must bind the exact derived identity, re-encode to the
same bytes, and decode again to the same value. Every rejected input must
carry one of the twelve frozen `PROTOCOL_*` codes 40000 through 40011.

The deterministic corpus comes from `conformance/smp1/v1/accepted.json`
(the request, response, failure, and both hello frames) and
`rejected.json`, plus header-boundary truncation, trailing-byte,
single-bit, and multi-frame concatenated-pair mutation seeds in every
lane (the pairs reach stream reassembly past the single-frame ceiling).
Negotiation executes through the shared oracle on fixture hello frames
decoded in the framed lane; prefix-stripped bodies were removed because
they never decode as hello records. Minimized crashers are retained as
harness regression records (`fuzz/regressions/S20_700_SMP1_001.json`).
Runtime corpus, binaries,
artifacts, and evidence remain under ignored
`evidence/runtime/s20-700-smp1-libfuzzer/` paths.

Focused validation:

```text
cargo test -p sley-protocol --locked
python3 scripts/check_smp1_persistent_fuzz_slice.py
make smp1-persistent-fuzz-smoke
python3 scripts/run_smp1_persistent_fuzz.py --manual
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

## Rounds 7c-7j (REQ-06 re-review wave)

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
