# S20-700 Adapter-Response Persistent Slice

Status: scoped persistent landed-surface slice; **full S20-700 remains incomplete**

This slice adds one libFuzzer target at the public typed restricted S20-280
reference-fixture boundary. It calls the production `invoke_reference_adapter`
judgment for all eight reference kinds. Generic replay varies both success and
declared-failure responses across six bounded structural type schemas. The
seven concrete fixtures exercise their exact frozen response contracts.

The target starts from a bounded valid caller-owned fixture and applies up to
four mutations from 26 import, effect, request, limit, cancellation, state,
replay-binding, and response-injection classes. For every input it asserts:

- equal initial state and request produce equal results and final state;
- every rejected invocation leaves the complete fixture unchanged;
- successful receipts bind exact pre-state, post-state, call index, action
  delta, captured-output count, and response or failure type;
- GenericReplay returns the exact stored typed response and consumes one entry;
- rerunning a successful fixture under a different `StateRoot` preserves its
  outcome and state but changes the transcript ID;
- canonical fixtures under generous bounded limits remain accepted.

Input is capped at 4,096 bytes. Payloads are capped at 32 bytes, collections at
four items, and mutation application at four classes per input. The
deterministic synthetic corpus contains 821 seeds. Corpus, binaries, artifacts,
and command evidence remain under ignored
`evidence/runtime/s20-700-adapter-responses-libfuzzer/` paths.

The byte mapping is a fuzz-only typed constructor, not a serialized adapter
request or response codec. This target uses the conformance-only in-memory
S20-280 fixture API. It does not exercise the authorized S20-380 wrapper, grant
live host access, integrate adapter opcodes with the VM, create persistent
execution reports, or complete S20-280 GA. Files, environment, clock, random,
and replay data remain bounded request-owned memory only.

Independent Vulcan review remains deferred because the local Forge OAuth
session returns 401. Mutation candidates, merge, and the full S20-700 finding
register remain required.

Focused validation:

```text
cargo +nightly-2026-02-27 clippy --manifest-path fuzz/Cargo.toml --bin adapter_responses --target-dir evidence/runtime/s20-700-adapter-clippy-target -- -D warnings
cargo test -p sley-adapter --locked
python3 scripts/check_reference_adapter_profile.py
python3 scripts/check_adapter_responses_persistent_fuzz_slice.py
make adapter-responses-persistent-fuzz-smoke
python3 scripts/run_adapter_responses_persistent_fuzz.py --manual
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
