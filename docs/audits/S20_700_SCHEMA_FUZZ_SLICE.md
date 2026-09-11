# S20-700 Bounded Schema Fuzz Slice

Status: bounded and persistent landed-surface slices; **full S20-700 remains incomplete**

This slice hardens the completed S20-140 schema boundary without beginning any
blocked mutation, transaction, protocol, merge, or release package.

It adds:

- 512 fixed-seed inputs, each bounded to 2,048 bytes, against the `SLEYEP01`
  bootstrap importer;
- near-canonical truncation, trailing-byte, bit-flip, byte-replacement, and
  arbitrary-byte cases;
- canonical reconstruction checks for every accepted input;
- an exact-selection adversarial test proving unknown epoch and contract IDs do
  not invoke a decoder, and a selected decoder failure does not fall back to a
  different epoch;
- a persistent libFuzzer target for the direct `SLEYEP01` bootstrap importer,
  seeded deterministically from the committed schema-epoch conformance vector;
- exact canonical re-encoding and `SchemaEpochId` preservation assertions for
  every successful persistent decode;
- routine `make fuzz-smoke` coverage and a machine-readable scope checker.

No crash, hang, permissive decode, or fallback finding was discovered. The
persistent target covers only direct schema bootstrap import. It does not cover
registry construction or registry dispatch, and it does not complete the master
goal's required persistent targets for SSMC graph/type/CFG, queries, mutation
candidates, pack import, merge, VM canonical inputs, or adapter responses. Any
future discovered failure still requires a minimized fixture, stable finding
ID, regression test, and root-cause disposition.

Vulcan's earlier independent bounded-slice review found no open P0, P1, or P2
issue. The decoder call counters make epoch/contract fallback assertions
effective. That verdict predates the persistent target. A bounded review
handoff for the new target was attempted but could not start because the local
Forge OAuth session returned 401, so an additional Vulcan review remains
deferred. Neither result is a full S20-700 or release disposition.

Superseded 2026-09-11: Vulcan re-reviews of the repaired harness are filed (round 7 wave); see evidence/review/verdicts/s20_700_schema_persistent_fuzz_slice/.

Focused validation:

```text
cargo test -p sley-schema bounded_schema_bootstrap_import_fuzz_smoke --locked
cargo test -p sley-schema registry_decode_never_falls_back_across_epoch_or_contract --locked
make fuzz-smoke
python3 scripts/check_schema_fuzz_slice.py
make schema-persistent-fuzz-smoke
python3 scripts/run_schema_persistent_fuzz.py --manual
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
