# S20-700 Root-Backed Query Engine Persistent Slice

Status: scoped persistent landed-surface slice for the full S20-310 profile; **full S20-700 remains incomplete**

This slice hardens the full S20-310 root-backed query engine
(`crates/sley-query/src/root_query.rs`,
`docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md`): the nineteen classes, the
input binding, typed continuation cursors, exact-then-paged results, and
the `SLEYRQR1` record encoder. The restricted S20-310 queries keep their own
target. It does not begin capsules, sessions, protocol, or release
packaging.

The libFuzzer target decodes a structured eighteen-kind root with the
complete-root judgment grammar, keeps only roots that pass closure rules C1
through C11, builds the arm-2 snapshot and a synthetic binding table for
them, then decodes a typed query, applied limits, a continuation flag, and a
cursor from the remaining bytes. Every accepted response must equal its
record length, carry the class tag, count its payload exactly, be
repeatable byte for byte, and never be truncated without continuation. A
continuation walk of up to sixteen pages must keep the exact total count on
every page and union to it when the walk started from the first page. Every
failure must carry one of the eleven frozen `QUERY_*` codes 31000 through
31010.

The deterministic corpus seeds every class over a minimal complete root with
and without continuation across limit and cursor spreads, plus raw byte
spreads that exercise the root decoder and judgment rejections. The corpus
is checked against `conformance/root-backed-query/v1/accepted.json` for
contract and class-count drift. Runtime corpus, binaries, artifacts, and
evidence remain under ignored `evidence/runtime/s20-700-root-query-libfuzzer/`
paths.

Focused validation:

```text
cargo test -p sley-query root_query --locked
python3 scripts/check_root_query_persistent_fuzz_slice.py
make root-query-persistent-fuzz-smoke
python3 scripts/run_root_query_persistent_fuzz.py --manual
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
