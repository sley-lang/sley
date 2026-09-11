# S20-700 SMP1 Frame Decoder Persistent Slice

Status: scoped persistent landed-surface slice for S20-410; **full S20-700 remains incomplete**

This slice hardens the S20-410 SMP1 frame decoder, hello decoder, and
derived negotiation (`crates/sley-protocol/src/lib.rs`,
`docs/spec/SMP1.md`). It does not cover the deterministic server dispatch,
cancellation and streaming semantics (S20-440), or the JSON bridge.

The libFuzzer target has four deterministic input lanes:

- direct bytes exercise the length prefix against the ceiling, the SCB1
  envelope magic, version, contract tag, epoch, digest trailer, the frame
  record shape, kinds, flags, bounded context, and hello body rules;
- rehashed bytes rewrite only the final `ProtocolFrameId` trailer and the
  length prefix so mutations reach the record rules instead of stopping at
  the outer digest;
- bare hello bytes decode as a `Hello` record and negotiate against a fixed
  server hello, asserting a repeatable `ProtocolHandshakeId` and a selection
  drawn only from the intersections;
- stream bytes decode as an S20-440 chunk record and reassemble, or are
  split under a small ceiling and reassembled to the same body.

Every accepted frame must bind the exact derived identity, re-encode to the
same bytes, and decode again to the same value. Every rejected input must
carry one of the twelve frozen `PROTOCOL_*` codes 40000 through 40011.

The deterministic corpus comes from `conformance/smp1/v1/accepted.json`
(the request, response, failure, and both hello frames) and
`rejected.json`, plus header-boundary truncation, trailing-byte, and
single-bit mutation seeds in every lane. Runtime corpus, binaries,
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
`-Zhost-config` + target rustflags (the old bin-only flag left zero
`sancov` symbols in owner rlibs; the gate now fails closed on zero),
runs-must-cover-corpus enforcement with `SMOKE_RUNS` at twice the corpus,
persistent corpus directories with stale-seed sync, crash minimization
via `-merge=1`, executed/coverage/crash evidence gates, a dedicated build
timeout, and append-only artifacts. The slice proved locally PASS with
executed == requested >= corpus, coverage feedback observed, zero crash
artifacts, and nonzero owner-lib `sancov` counts; the durable record is
`machine-summary.json` `last_local_proof` (runtime `evidence.json` files
are gitignored by design). The pinned qualification toolchain is
unchanged (`clang-18`, pinned libfuzzer path, `nightly-2026-02-27`); the
local proof ran under documented `SLEY_FUZZ_CC` / `SLEY_FUZZ_LIBFUZZER_A`
overrides. Re-review of the slice's Vulcan verdict is queued, not
assumed.
