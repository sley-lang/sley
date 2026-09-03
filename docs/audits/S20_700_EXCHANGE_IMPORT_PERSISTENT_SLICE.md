# S20-700 Repository-Exchange Import Persistent Slice

Status: scoped persistent landed-surface slice for S20-540; **full S20-700 remains incomplete**

This slice hardens the S20-540 repository-exchange preflight and importer
(`crates/sley-repo/src/exchange.rs`, `docs/spec/REPOSITORY_EXCHANGE_V1.md`).
It does not begin merge, comparison, protocol, or release packaging.

The libFuzzer target has two deterministic input lanes:

- direct bytes exercise the exact outer envelope, trailer, payload, embedded
  pack, receipt, head, branch, closure, and target rules;
- rehashed bytes replace only the final `RepositoryExchangeId` trailer so
  mutations can reach inner payload and closure checks instead of stopping at
  the outer digest.

Both lanes are bounded to 65,536 payload bytes. A failed preflight must also
fail the import and must leave the fresh target path absent. A successful
preflight must bind the exact exchange identity, report a leaf count of
`1 + receipts + branches + 1`, import into a fresh target with the same
identity, head, receipt count, and branch count, and reject a second import of
the complete clone with `EXCHANGE_TARGET_NOT_EMPTY`. The verifier is the
epoch-1 entity-object verifier used by the frozen conformance fixture; it is
not a production object-schema registry or authority.

The deterministic corpus comes from
`conformance/repository-exchange/v1/accepted.json` and includes direct and
rehashed canonical, truncation, trailing-byte, and single-bit mutation seeds.
Runtime corpus, binaries, artifacts, and evidence remain under ignored
`evidence/runtime/s20-700-exchange-import-libfuzzer/` paths.

Focused validation:

```text
cargo test -p sley-repo exchange --locked
python3 scripts/check_exchange_persistent_fuzz_slice.py
make exchange-persistent-fuzz-smoke
python3 scripts/run_exchange_persistent_fuzz.py --manual
```
