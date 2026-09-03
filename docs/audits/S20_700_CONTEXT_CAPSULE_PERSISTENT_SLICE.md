# S20-700 Context Capsule Builder Persistent Slice

Status: scoped persistent landed-surface slice for the full S20-320 profile; **full S20-700 remains incomplete**

This slice hardens the full S20-320 context capsule builder
(`crates/sley-query/src/context_capsule.rs`,
`docs/spec/CONTEXT_CAPSULE_PROFILE_V1.md`): source binding, fact
dictionaries, omission and continuation status, and the `SLEYCCP1` record
encoder. The restricted S20-320 capsule keeps its own coverage. It does not
begin sessions, protocol, or release packaging.

The libFuzzer target reuses the root-backed query engine grammar: a
structured eighteen-kind root that passes closure rules C1 through C11, an
arm-2 snapshot, and a typed query with limits, continuation flag, and
cursor. Every query the engine answers must capsule, or fail only on a
resource ceiling; every capsule must carry the query identity and class,
strictly ordered entity and root dictionaries with one kind per entity,
in-range relationship and table indexes, `omitted` equal to
`total_count - returned`, and truncation exactly when a next cursor exists.
A capsule is `Complete` only for an untruncated first page with nothing
omitted, and the walk of page capsules keeps one exact total and
never presents a page as complete. Every capsule must be repeatable byte for
byte.

The deterministic corpus seeds every class over a minimal complete root
with and without continuation across limit and cursor spreads, plus raw
byte spreads that exercise the root decoder and judgment rejections; it is
checked against `conformance/context-capsule/v1/accepted.json` for contract
and vector-count drift. Runtime corpus, binaries, artifacts, and evidence
remain under ignored `evidence/runtime/s20-700-context-capsule-libfuzzer/`
paths.

Focused validation:

```text
cargo test -p sley-query context_capsule --locked
python3 scripts/check_context_capsule_persistent_fuzz_slice.py
make context-capsule-persistent-fuzz-smoke
python3 scripts/run_context_capsule_persistent_fuzz.py --manual
```
