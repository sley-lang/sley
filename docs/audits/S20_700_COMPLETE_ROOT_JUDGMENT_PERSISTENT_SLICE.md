# S20-700 Complete-Root Judgment Persistent Slice

Status: scoped persistent landed-surface slice for the full S20-250 profile; **full S20-700 remains incomplete**

This slice hardens the S20-250 full complete-root closure judgment
(`crates/sley-query/src/complete_root.rs`,
`docs/spec/COMPLETE_ENTITY_IMPACT_PROFILE_V1.md` section 6). It does not
begin merge, comparison, protocol, or release packaging, and it does not fuzz
object decoding, which the S20-700 schema and pack slices already cover.

The libFuzzer target decodes a structured eighteen-kind request from raw
bytes: an entity count, then per entity a kind, an identity byte, and the
kind's identity and set fields, then a flags byte read from the end of the
input. A set's length is one byte below four and two bytes for four through
twenty-four, which is the widest set that can name every entity a request
carries, so a seed encodes a whole fixture set rather than its first four
members. The flags select four deterministic lanes (raw versus canonical sets,
decoded versus kept entity order) crossed with five fact lanes (facts from
decoded bodies, or bound entities, entry points, and dependency roots from
further bytes in every combination including all three), so every closure
rule and every canonicality check is reachable, including the fact-only
rejection vectors, which now encode their own mismatched facts instead of
collapsing onto accepted seeds.

A passing judgment must be repeatable, must equal the plain `ImpactIndex`
over the same request, must cover exactly the request's entities, must name a
workspace entity of the request, and must produce no edge outside the bound
inventory. A failing judgment must carry one of the frozen `IMPACT_*` codes.
Inputs are bounded to 4,096 bytes.

The deterministic corpus comes from
`conformance/complete-entity-impact/v1/accepted.json` and `rejected.json`,
encoded into the target's grammar under all twenty flag lanes (four bases
crossed with five fact lanes), plus
trailing-byte, minimal, and single-bit mutation seeds. Runtime corpus,
binaries, artifacts, and evidence remain under ignored
`evidence/runtime/s20-700-complete-root-libfuzzer/` paths.

Focused validation (SLEY_FUZZ_CC/SLEY_FUZZ_LIBFUZZER_A overrides exported;
the pinned clang-18 default returns BLOCKED on hosts without it):

```text
cargo test -p sley-query complete_root --locked
python3 scripts/check_complete_root_persistent_fuzz_slice.py
make complete-root-persistent-fuzz-smoke
python3 scripts/run_complete_root_persistent_fuzz.py --manual
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
