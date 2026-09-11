# S20-700 Repository-Pack Import Persistent Slice

Status: scoped persistent landed-surface slice; **full S20-700 remains incomplete**

This slice hardens the completed S20-170 root/object-only repository-pack
importer. It does not begin refs, transactions, compression, signatures,
clone-equivalent exchange, merge, or release packaging.

The libFuzzer target has two deterministic input lanes:

- direct bytes exercise the exact outer envelope, digest, profile, schema,
  state-root, object-closure, verifier, and clean-store import path;
- rehashed bytes replace only the final `RepositoryPackId` trailer so mutations
  can reach inner payload checks instead of stopping at the outer digest.

Both lanes are bounded to 65,536 payload bytes. A failed import must leave the
clean store without an `objects` tree. A successful import must bind the exact
pack ID, find no preexisting objects, and import a second time with identical
roots and present-only object accounting. The fixture verifier is intentionally
limited to the two S20-170 conformance object contracts. It is not a production
object-schema registry or authority.

The deterministic corpus comes from
`conformance/repository-pack/v1/accepted.json` and includes direct and rehashed
canonical, truncation, trailing-byte, and single-bit mutation seeds. Runtime
corpus, binaries, artifacts, and evidence remain under ignored
`evidence/runtime/` paths.

This slice does not cover the deferred full S20-540 pack, merge, protocol,
mutation-candidate, VM-input, or adapter-response surfaces. A bounded Vulcan
handoff could not start because the local Forge OAuth session returned 401, so
independent review of this persistent addition remains deferred.

Superseded 2026-09-11: Vulcan re-reviews of the repaired harness are filed (round 7 wave); see evidence/review/verdicts/s20_700_pack_persistent_fuzz_slice/.

Focused validation:

```text
cargo test -p sley-repo bounded_pack_import_fuzz_smoke_rejects_rehashed_mutations --locked
python3 scripts/check_pack_persistent_fuzz_slice.py
make pack-persistent-fuzz-smoke
python3 scripts/run_pack_persistent_fuzz.py --manual
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
