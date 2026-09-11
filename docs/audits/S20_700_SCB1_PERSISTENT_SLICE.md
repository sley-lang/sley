# S20-700 SCB1 Decoder Persistent Slice

Status: scoped persistent landed-surface slice; **full S20-700 remains incomplete**

This slice hardens the SCB1 standalone decoder
(`crates/sley-scb1`, fixture contract `sley2-scb1-v1`): twenty-two
selector lanes decode standalone fixtures and typed payloads
(`UInt`, `SInt`, `Bool`, `Bytes`, `Text`, labels, floats, lists, maps,
options, unions, fixture records), plus an encode-then-decode lane that
constructs canonical uvar/sint/bool/bytes encodings and requires exact
decode, so the re-encode oracle is reachable without forging digests.

Every accepted standalone fixture must re-encode byte-identically with a
stable `ObjectId`; every rejection must carry a stable `ScbErrorCode`.
No panic is admissible. The decoder is safe Rust, so no ASan is
instrumented; this is a bounded smoke, not a probe.

The deterministic corpus is generated under
`evidence/runtime/s20-700-scb1-libfuzzer/corpus` (selector-prefixed
fixtures, 1127 seeds at last count); the pinned qualification toolchain is unchanged
(`clang-18`, pinned libfuzzer path, `nightly-2026-02-27`); local proofs
ran under documented `SLEY_FUZZ_CC` / `SLEY_FUZZ_LIBFUZZER_A` overrides,
and the pinned default has no recorded proof on this host.

## Rounds 7c-7j (REQ-06 review wave)

Crash minimization uses `-minimize_crash=1` with exact artifacts; the
coverage floor measures on-disk corpus files plus 256 mutations;
coverage gates strictly on inline counters with monotonic `ft`; the
owner gate counts the rlibs cargo linked (fingerprint-authoritative);
warnings are captured from the full streams against an explicit
allowlist; builds refuse ambient `RUSTFLAGS`; prior crashers
re-execute every smoke; per-input `-timeout=30` and `-rss_limit_mb=2048`
bound hangs; libFuzzer seeds are recorded. Manual campaigns remain
operator exploration by design.
