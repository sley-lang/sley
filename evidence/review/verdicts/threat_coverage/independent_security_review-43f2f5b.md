# threat_coverage / independent_security_review — Vulcan (independent QA and security lane), re-review at 43f2f5b

Role: Forge Council Vulcan via claude-code, the same lane that issued
`independent_security_review-cb841a6.md` (REVISE_0_P0_0_P1_5_P2_5_P3 + one P4).
Task: re-derive each of the eleven prior findings at HEAD, judge the
structural-classification fix and the T08 duplicate removal, then give the
register's security judgment across all 56 threats.

## Scope verification

`git rev-parse HEAD` = `43f2f5ba738b587a9a0bb365a55f3096527e3e3d` (matches
SCOPE_SHA). Working tree clean. `56bac4b` ("Council drain repairs round 5")
and `43f2f5b` ("Qualification repairs: ... T07/T40 fault-seeded tests,
threat-coverage exercise rule") are both ancestors of HEAD on `main`.
Nothing was written except this transcript.

## Inputs read in full

- `/tmp/.../scratchpad/review-brief.md`
- `evidence/review/verdicts/threat_coverage/independent_security_review-cb841a6.md`
- `scripts/build_threat_coverage_report.py` (HEAD)
- `docs/THREAT_REGISTER.md` (HEAD, table and Realized codes addendum)
- `evidence/security/threat-coverage-report.json` (all 56 rows dumped: state, searched_codes, exercised_in, recorded_exercise, located_file_count, planned_evidence_present)
- `machineresearch/sley-2.0/machine-summary.json` `threat_coverage` block (line 4356 region)
- `evidence/release/decision-dossier.json` security-review item (line 192)
- `git diff 43f2f5b^..43f2f5b` for `crates/sley-repo/src/gc.rs`, `crates/sley-policy/src/candidate_validation.rs`, `docs/THREAT_REGISTER.md`, `evidence/release/decision-dossier.json`, `scripts/check_host_abi_markers.py`
- Source regions read: `gc.rs:1055-1135, 2640-2695`; `candidate_validation.rs:1284-1302, 2123-2280, 2761-2830, 3270-3300, 3440-3480`; `sley-policy/src/lib.rs:3100-3245`; `sley-repo/src/lib.rs:734-742, 1395-1406, 1494-1510, 1660-1690`; `sley-vm/src/execute.rs:3060-3090`; `sley-repo/src/merge.rs:3348-3378, 3760-3782`; `sley-query/src/query.rs:1330-1350`; `sley-query/src/snapshot.rs` tests 1026-1170; `sley-check/src/lib.rs:1490-1506, 1795-1808`; `fuzz/targets/repository_pack_importer.rs:1-60`; `scripts/check_host_abi_markers.py:150-210`

## Tool results (commands and exact results)

| Command | Result |
|---|---|
| `python3 scripts/build_threat_coverage_report.py --check` | `{"mode":"check","result":"PASS","threat_count":56,"states":{"PLANNED_EVIDENCE_PRESENT":9,"STRUCTURAL_CONTROL_RECORDED":3,"SYMBOL_REALIZED_WITH_EXERCISE":44},"p0_p1_without_located_symbol":0,...}` exit 0 |
| `python3 scripts/check_host_abi_markers.py` | `{"contract":"sley2-host-abi-1","markers":"PINNED","result":"PASS"}` exit 0 |
| `cargo test -p sley-policy t07_ --locked` | 1 passed: `candidate_validation::tests::t07_live_bound_target_identity_collides_on_the_live_binding_branch` |
| `cargo test -p sley-repo t40_ --locked` | 2 passed: `gc::tests::t40_seed_reachable_candidate_assertion_is_effective`, `gc::tests::t40_reachable_candidate_in_a_corrupted_plan_trips_the_guard_before_any_delete` |
| `uv run --project oracle/scb1 python scripts/check_root_backed_query_vector.py` | `"result": "PASS", "vectors": 27, "query_classes": 19` |
| `grep -rniw git crates/*/src --include=*.rs` (T50) | no matches |
| `grep -rn "std::env\|std::fs\|std::net\|std::process\|std::time" crates/sley-vm/src` (T29, whole tree, not just the checker's slice) | no matches; `crates/sley-vm/Cargo.toml` has no `rand`/`getrandom` dependency |
| report digest at HEAD | `b276f8e95f27f833c5ce1d0d6f3b71d9ec0d2212ffa3e036d4487fac3e4f3405`; `independent_security_review: "PENDING"` |
| `conformance/scb1/v1/rejected.json` | 26 vectors over 22 codes (T01/T02 addendum claim re-derived exactly) |

Two scripted probes I ran against the builder's own functions (results are
reproduced in the per-item analysis):

1. Marker-position probe: for every `crates/**/*.rs` outside `tests/`, where
   the first `#[cfg(test)]` / `mod tests` token sits as a fraction of the
   file, and which files named `*_tests.rs` / `tests.rs` carry no marker.
2. Strict-versus-tracked exercise probe: `exercised_in()` re-run with a
   source set whose Rust in-file region starts at the actual `#[cfg(test)]
   mod … {` block (plus sibling `*_tests.rs`), compared to the tracked
   rule, per SYMBOL_REALIZED_WITH_EXERCISE row.

## Re-derivation of the eleven prior findings

### 1. P2 record — machine-summary threat_coverage numbers — CLOSED
`machineresearch/sley-2.0/machine-summary.json` `threat_coverage`: 56 /
44 / 3 / 9, `p0_p1_without_located_symbol: 0`, `realized_codes_recorded: 25`.
Tracked report: 56 / 44 / 3 / 9; `by_severity` P0 = 37+3+6, P1 = 7+3.
`--check` PASS proves tracked == derived. T33 is
SYMBOL_REALIZED_WITH_EXERCISE in both. Residual (P4): the summary's
`independent_security_review_note` (line 4356) still says "classify()
exercise definition, GC guard reachability, and sequence-row precision
queued", which describes the state before 43f2f5b.

### 2. P2 contract — classify() exercise definition — CLOSED as filed; P3 residual
The co-location rule is gone: `exercise_sources()` (line 212) contributes a
Rust file only from its first test marker, whole files only under `tests/`,
`test_*.py`, `conformance/`, `fuzz/targets/`, `oracle/`; `classify()` takes
`exercised` from `exercised_in()`; the addendum's "Exercised by" column is
the fallback for variant-only tests. The report's interpretation now says
"names the symbol … never that a production file merely carries some
assertion", which is the honest description of what the rule proves.

Can the new rule still credit a dead or reserved symbol? Yes, two ways:

(a) The marker is the *first* `#[cfg(test)]` or `mod tests` token in the
file (line 229, `min(...)`), and nine production files carry a
`#[cfg(test)]` item near their top, so their entire body is treated as a
test region: `sley-repo/src/gc.rs:6`, `sley-repo/src/refs.rs:100` (of
~18k lines), `sley-protocol/src/lib.rs:17`, `sley-txn/src/repository.rs:160`
(of ~7k), `sley-vm/src/lib.rs:10`, `sley-id/src/lib.rs:48`,
`sley-store/src/lib.rs:266`, `sley-protocol/src/server.rs:205`,
`sley-vm/src/extended.rs:202`. Any symbol whose `=> "SYMBOL"` definition
lives in one of these files is credited by its own definition. At HEAD the
strict-versus-tracked probe shows one row whose `exercised_in` rests
entirely on this: T35 (`VM_LOWER_CACHE_KEY_UNSUPPORTED`,
`exercised_in: ["crates/sley-vm/src/lib.rs#tests"]`, whose only string
mentions are production/doc lines 78, 94, 200, 237; the real assertion is
by variant at `lib.rs:324`). T35's state survives via the addendum record,
so no state is wrong today; fifteen other rows (T01–T04, T06, T14, T16,
T33, T34, T36, T45, T46, T49) list `refs.rs#tests`, `repository.rs#tests`,
`protocol/lib.rs#tests`, `store/lib.rs#tests`, `vm/lib.rs#tests` or
`extended.rs#tests` among their sources but keep genuine sources under the
strict rule.

(b) A frozen-symbol test counts as naming: `sley-repo/src/lib.rs:1503`
`stable_reserved_error_symbol_exists` names `PACK_DECOMPRESSION_LIMIT`, so
if the T42 addendum row were removed the reserved symbol would be credited
again. The rule cannot distinguish a freeze test from a fault seed; the
interpretation text says so, and the addendum record is what carries the
semantic claim.

Under-count in the other direction: sibling test files with no marker
(`sley-protocol/src/server_tests.rs`, `sley-vm/src/extended_tests.rs`,
`sley-json-bridge/src/tests.rs`, `sley-mutate/src/codec/adversarial_tests.rs`,
`fixture_tests.rs`) are invisible to the rule, so e.g. T46's real assertion
(`server_tests.rs:3642`) and T14's retryability table (`server_tests.rs:245`)
do not appear in `exercised_in`.

Sample of SYMBOL_REALIZED_WITH_EXERCISE rows, verified by reading the named
test (all P0 unless marked):

| Row | What drives the failure path | Verdict |
|---|---|---|
| T05 | `sley-schema/src/lib.rs:1582, 1912` assert `SchemaErrorCode::Downgrade` on both return sites (584, 664) | fault-seeded |
| T07 | see item 3 | fault-seeded |
| T08 | `tombstones_graph_errors_and_missing_references_are_distinct` (`candidate_validation.rs:2790-2806`) seeds `tombstones = [target]` and asserts `CANDIDATE_IDENTITY_COLLISION` phase 4 | fault-seeded |
| T09 | same test, `candidate_validation.rs:2808-2825`: parent `fixed(92)` unresolved → `GRAPH_UNRESOLVED_REFERENCE` phase 5 | fault-seeded |
| T10 | `sley-check/src/lib.rs:1291, 1310` assert `TypeErrorCode::DefinitionCycle` (emitted at 703 on `Visit::Active`) | fault-seeded |
| T11 | `sley-check/src/cfg.rs:1772, 1788` assert `CfgErrorCode::ResourceLimit` for block and edge ceilings | fault-seeded |
| T12 | `sley-check/src/lib.rs:1503` `MAX_TUPLE_ITEMS + 1` → `TypeErrorCode::ResourceLimit`; depth closure tests at 1214, 1227 | fault-seeded |
| T13 (P1) | `conformance/root-backed-query/v1/rejected.json:146` expected `QUERY_RESOURCE_LIMIT`; checker PASS over 27 vectors | corpus-driven |
| T14 | `sley-query/src/query.rs:1338-1348` sets `max_returned_edges = 1` and asserts `RequiredFactOmitted`; corpus lines 8, 126 | fault-seeded |
| T19/T20/T21 | `sley-policy/src/lib.rs` `policy_self_oracle_and_protected_entity_isolation_is_pure` seeds changed policy root (101), schema epoch (99), contract root (22) and asserts the three `POLICY_ISOLATION_*_CHANGED` codes; `finalize_mandatory_contract_tests` tests seed omitted contract/test/selection | fault-seeded |
| T22 | `candidate_validation.rs:3450-3480` flips the last token byte, proves the digest is unchanged, asserts `CAP_AUTHENTICATOR_INVALID` phase 9; `t22_/t23_/t24_` tests at `lib.rs:3381, 3406, 3484` | fault-seeded |
| T31 (P1) | `execute.rs:3072-3087` `cancel_at_fuel = Some(0)` → `ExecutionTermination::Cancelled` | fault-seeded |
| T33 | `extended_tests.rs:2696, 2711, 3772-3773` assert `LowerErrorCode::ImmediateMismatch`; oracle `check-vm-extended` (`vm_extended.py:371`) re-derives cache-key preimages | fault-seeded + oracle |
| T35 | `snapshot.rs:1026-1170` walk `CacheDiscardReason::{ContextMismatch, DigestMismatch, …}` on a rebuilt admission; `vm/src/lib.rs:324` asserts `CacheKeyUnsupported` | fault-seeded |
| T37 | `sley-store/src/lib.rs:1613, 1660, 1829, 2014, 2094, 2174, 2254, 2774` assert `RECOVERY_STAGED_OBJECT` inside `mod tests` (933) | fault-seeded |
| T40 | see item 6 | fault-seeded |
| T41 | `fuzz/targets/repository_pack_importer.rs` reseals mutated packs and requires exactly the contract symbol (`ResealExpectation::Reject`), plus unit asserts at `sley-repo/src/lib.rs:1386-1654` for `PACK_VERSION_UNSUPPORTED`, `PACK_DIGEST_MISMATCH`, `PACK_OBJECT_CORRUPT`, `PACK_ROOT_INVALID`, `PACK_DUPLICATE_ENTRY`, `PACK_OBJECT_MISSING`, `PACK_OBJECT_UNEXPECTED`, `PACK_CANONICAL_ORDER`, `PACK_DIGEST_TREE_MISMATCH` | fuzz + fault-seeded |
| T42 (P1) | see item 5 | fault-seeded |
| T43 | `merge.rs:3760-3780` forges `plan.merged_root = root(99)` and asserts `MergeErrorCode::ResultMismatch` on `commit_merge` | fault-seeded |
| T44 | `merge.rs:3348-3368` plan over a conflict → `PlanUnsupported` | fault-seeded |
| T46 | `server_tests.rs:3642` asserts `PROTOCOL_REQUEST_ID_CONFLICT`; `entity-read/v2/rejected.json` vectors | fault-seeded + corpus |
| T51 | `sley-repo/src/lib.rs:1395-1406` flips the last pack byte, asserts `PACK_DIGEST_MISMATCH` and no `objects/` directory | fault-seeded |
| T55 (P1) | `bench/accounting/tests/test_report.py:188, 327, 428, 437` assert `AccountingErrorCode.{FLOAT_FORBIDDEN, CHAIN_INVALID, INCOMPLETE}` by enum (invisible to the string rule; addendum record carries it) | fault-seeded |

Every sampled row names something that actually drives the failure path.

### 3. P2 record — T07 live-binding branch — CLOSED
`Fixture::build(..., live_binding_collision = true)`
(`candidate_validation.rs:2149-2230`) pushes an entity object whose id is
`EntityId::derive(workspace_id, nonce 30, 3, 0)` — the exact id the
candidate's create operation derives — into `base_objects` and binds it in
the state root; no tombstone is supplied. The test (2765-2787) first asserts
the binding is present in `base_state.record.entity_bindings`, then asserts
`CandidateDecision::InvalidIdentity` phase 4 `CANDIDATE_IDENTITY_COLLISION`.
Production predicate at 1289-1295 is `entity_bindings.binary_search(...)
.is_ok() || context.tombstones.binary_search(&expected).is_ok()`; with an
empty tombstone slice only the first arm can be true. Test passes. Addendum
row (`docs/THREAT_REGISTER.md:88`) is accurate.

### 4. P2 record — T20 — CLOSED
Addendum row (line 90) records `POLICY_ISOLATION_SCHEMA_EPOCH_CHANGED` and
states the expected symbol is dead. The report searches the realized code
(`sley-policy/src/lib.rs:150, 1486`) and the isolation test seeds epoch 99
and asserts it. P4 wording: the row says "the candidate validation
isolation tests"; the test is
`policy_self_oracle_and_protected_entity_isolation_is_pure` in
`sley-policy/src/lib.rs`, not in `candidate_validation.rs`.

### 5. P2 record — T42 — CLOSED
Addendum row (line 93) records `PACK_COMPRESSION_UNSUPPORTED` and marks the
expected symbol reserved. `later_profiles_fail_closed`
(`sley-repo/src/lib.rs:1660-1678`) builds a pack with compression profile 1
and asserts `PACK_COMPRESSION_UNSUPPORTED`; production refuses at 739-741.
Only profile NONE is admitted, so no decompression path exists to bomb.

### 6. P3 implementation — T40 GC guard reachability — CLOSED
`delete_planned_candidates` (`gc.rs:1071-1107`) is the old loop body of
`collect_inner`, lifted verbatim (the diff shows only the header hunk and
the new functions; the read/`symlink_metadata`/`remove_file`/`sync_dir`/
partial-failure sequence is untouched), so no deletion behavior changed.
The guard `reachable.contains(&object_id)` runs before `store.read` and
before the unlink of each candidate. `gc_collect_with_injected_reachable_candidate`
(1113-1127, `#[cfg(test)]`) runs the real planner and guard acquisition,
then inserts the first reachable object at index 0 of the candidate list —
a corrupted plan the planner cannot produce, which is the only way to reach
the guard and therefore a legitimate assertion-effectiveness seed for the
P0 property. The test (2662-2691) asserts the symbol, no partial report,
both objects still on disk, and that an honest run afterwards collects
exactly the unreachable object. Both `t40_` tests pass. The planner-side
test remains the "assertion is effective" check for the first line of
defence.

P4: the doc line at 1066-1069 ("nothing is deleted for that run") holds
only because the seeded reachable candidate precedes every unlink. A
corrupted plan with the reachable object later in the list would unlink
earlier (unreachable) candidates first, then return the violation with no
partial report. The P0 property still holds (no reachable object is ever
deleted); a whole-plan intersection check before the loop would make the
doc claim unconditional and preserve the partial record.

### 7. P3 record — T41 — CLOSED (P4 residual)
Addendum row (line 92) now records the `PackErrorCode` family and the
`repository_pack_importer` fuzz target, which I confirmed is a reseal/
mutation driver, not a code allowlist. Residual: `PackErrorCode` does not
match the row regex, so the machine row still searches
`EXCHANGE_PACK_INVALID` and locates only `exchange.rs`; listing the real
string codes (`PACK_OBJECT_CORRUPT`, `PACK_DIGEST_TREE_MISMATCH`, …) would
point the report at the pack layer.

### 8. P3 record — T56 expected-code cell — CLOSED
`docs/THREAT_REGISTER.md:70` cell is now `` `SESSION_UNKNOWN` ``; the report
row's `searched_codes` is `["SESSION_UNKNOWN"]`, `located_file_count` 8.

### 9. P3 record — T35 — CLOSED (P4 residual)
Line 91 records the query-owned rebuild-first `CacheAdmission` control in
`snapshot.rs`; tests at 1026-1170 exercise every discard reason. Residual:
`recorded_exercises()` (line 112) and `realized_codes()` (line 138) assign
per id, so the second T35 row (line 94) overwrites the first and the report
row shows only the VM cache-key control and "the extended vectors"; the
query-side record exists only in the register.

### 10. P3 record — dossier wording — CLOSED (P4 residual)
`decision-dossier.json:192` now reads "44 have a located control that a
test or corpus exercises … (a located symbol proves the named control
exists, not that the threat is mitigated)". The caveat is present and my
sampling found no row where "exercises" is untrue, but the report's own
interpretation says "names the symbol", which is the precise claim; the
dossier and machine-summary sentence should say the same.

### 11. P4 record — T29 no-clock mechanization — CLOSED; new P3 on the scanner
`std::time` is in `FORBIDDEN` (`check_host_abi_markers.py:193`) and the
checker passes. New finding: line 199 defines the scanned production text
as `text.split("#[cfg(test)]")[0]`, i.e. everything before the *first*
`#[cfg(test)]` token. Measured coverage in the closure: `sley-vm/src/lib.rs`
9 of 419 lines (9 top-level fn/impl unscanned), `sley-vm/src/extended.rs`
201 of 2128 lines (44 top-level fn/impl unscanned), `sley-id/src/lib.rs` 47
of 786 lines (6 unscanned). A clock, env, fs, net or process reference
added to `extended.rs` after line 202 would pass this gate. The `unsafe`
half of the claim is compiler-enforced (`#![forbid(unsafe_code)]` at line 1
of all five closure crates), so the gap is confined to the `std::*` /
`Command::new` / `dlopen` / `libloading` markers. My independent whole-tree
grep shows the property holds at HEAD; the mechanization does not cover
what it claims to.

### Structural classification fix and T08 duplicate — VERIFIED
`structural_entries()` now returns `without_symbol - with_symbol`. T35 row 1
(line 91) parses symbol-free (`CacheAdmission`, `ContextMismatch` are
CamelCase, not matched); row 2 (line 94) carries
`VM_LOWER_CACHE_KEY_UNSUPPORTED`, so T35 is not structural. Structural set =
{T28, T29, T50}, matching the report's three STRUCTURAL_CONTROL_RECORDED
rows; each re-verified (store refuses non-regular files via
`symlink_metadata` at `sley-store/src/lib.rs:214, 600, 661, 712`; no
`std::{env,fs,net,process,time}` and no `rand` in `sley-vm`; no
word-boundary `git` in any crate). The duplicate T08 row is removed (diff
confirms; one T08 row at line 89 remains).

## New findings

- P3 [contract] `scripts/check_host_abi_markers.py:199` — hygiene scan truncates at the first `#[cfg(test)]` token (item 11).
- P3 [contract] `scripts/build_threat_coverage_report.py:229` — first-marker test region turns nine production files into "test regions"; T35's `exercised_in` is a production-mention credit today, and any symbol defined in those files is self-crediting (item 2a). Freeze tests count as naming (item 2b).
- P3 [record] `docs/THREAT_REGISTER.md:62` (T48) — expected `CAPABILITY_DENIED` is located only as a substring of `CANDIDATE_VALIDATION_CAPABILITY_DENIED` (`candidate_result.rs:200`) and exercised only through the candidate-result accepted corpus and oracle. The realized control is the phase-9 capability judgment (`POLICY_GRANT_DENIED` and `CAPABILITY_SUMMARY_MISMATCH` at `candidate_validation.rs:3285-3298`, token forgery/replay/scope at `lib.rs:3381-3484`) and the structural fact that entity labels are never consulted for authority; no addendum row records this and no test seeds the register's own required case ("hostile label/prompt metadata"). The control is adequate; the traceability is accidental, as T41's was.
- P4 [record] `machineresearch/sley-2.0/machine-summary.json:4356` — note still says the classify(), GC guard and sequence-row items are "queued".
- P4 [record] `docs/THREAT_REGISTER.md:90` — T20 test location wording.
- P4 [record] `docs/THREAT_REGISTER.md:92` — T41 machine row still searches the exchange-layer symbol.
- P4 [contract] `scripts/build_threat_coverage_report.py:112,138` — per-id overwrite drops the first T35 row from the report.
- P4 [record] `evidence/release/decision-dossier.json:192` and machine-summary interpretation — "exercises" where the report says "names".
- P4 [implementation] `crates/sley-repo/src/gc.rs:1066-1069` — "nothing is deleted for that run" is position-dependent.

## Security judgment

Across all 56 threats: every P0/P1 threat has a realized control. For the
44 SYMBOL_REALIZED_WITH_EXERCISE rows I read the driving test, corpus, fuzz
target or oracle for 24 of them (listed above) and re-confirmed the prior
round's readings for the rest (T03, T04, T06, T23, T24, T25, T26, T27, T30,
T32, T34, T36, T45, T49, T53); each reaches the failure path by seeding a
fault, not by co-existing with an assertion. The three structural rows hold
under independent grep. The nine PLANNED_EVIDENCE_PRESENT rows (T15, T17,
T18, T38, T39, T47, T52, T54, T56) name tests that exist
(`server_tests.rs`, `session.rs`) or point at the S20-390 closeout; note
that `classify()` short-circuits on `evidence_present`, so those rows never
receive an exercise judgment from the builder — the review is their only
exercise check, and the prior round's reading stands. The two P0 gaps of
the prior round (T07 live-binding branch, T40 guard) are now covered by
fault-seeded tests that I ran. The report's interpretation is honest about
what the classifier proves ("names the symbol"); the dossier and summary
sentences say "exercises", a mild overstatement that my sampling does not
contradict. No open P0/P1/P2 remains; the three P3 items are a scanner
coverage gap (property true at HEAD), a residual classifier weakness that
mis-credits no state today, and a traceability record for T48.

```
VERDICT: PASS
SECTION: threat_coverage
FIELD: independent_security_review
SCOPE_SHA: 43f2f5ba738b587a9a0bb365a55f3096527e3e3d
FINDINGS:
[P3] [contract] scripts/check_host_abi_markers.py:199 - production text is everything before the first `#[cfg(test)]` token, so sley-vm/src/lib.rs is scanned for 9 of 419 lines, sley-vm/src/extended.rs for 201 of 2128 (44 production fn/impl unscanned) and sley-id/src/lib.rs for 47 of 786; the T29 no-env/fs/net/process/clock claim is true at HEAD by whole-tree grep but is not mechanized for most of the VM
[P3] [contract] scripts/build_threat_coverage_report.py:229 - the Rust test region starts at the first `#[cfg(test)]`/`mod tests` token, which for gc.rs:6, refs.rs:100, protocol/lib.rs:17, txn/repository.rs:160, vm/lib.rs:10, sley-id/lib.rs:48, store/lib.rs:266, server.rs:205, extended.rs:202 is near the top, so production text in those files still counts as exercise (T35 exercised_in rests solely on vm/lib.rs production lines 78/94/200/237; state held only by the addendum record) and a frozen-symbol test (sley-repo/src/lib.rs:1503) still counts as naming a reserved symbol
[P3] [record] docs/THREAT_REGISTER.md:62 - T48 `CAPABILITY_DENIED` is credited by substring of `CANDIDATE_VALIDATION_CAPABILITY_DENIED` (crates/sley-policy/src/candidate_result.rs:200) via the accepted corpus; the realized control (phase-9 POLICY_GRANT_DENIED / CAPABILITY_SUMMARY_MISMATCH at candidate_validation.rs:3285-3298, token tests at lib.rs:3381-3484, labels never consulted for authority) has no addendum row and no test seeds the register's "hostile label/prompt metadata" case
[P4] [record] machineresearch/sley-2.0/machine-summary.json:4356 - independent_security_review_note still lists the classify(), GC guard and sequence-row items as "queued" after 43f2f5b closed them
[P4] [record] docs/THREAT_REGISTER.md:90 - T20 "Exercised by" names candidate-validation tests; the seeding test is policy_self_oracle_and_protected_entity_isolation_is_pure in crates/sley-policy/src/lib.rs
[P4] [record] docs/THREAT_REGISTER.md:92 - T41 row names `PackErrorCode`, which the row regex cannot parse, so the report still searches EXCHANGE_PACK_INVALID and locates only exchange.rs; list the PACK_* string codes
[P4] [contract] scripts/build_threat_coverage_report.py:112,138 - per-id assignment lets the second T35 row (register line 94) overwrite the first (line 91), so the report row omits the query-owned CacheAdmission control and its snapshot admission tests
[P4] [record] evidence/release/decision-dossier.json:192 - "a located control that a test or corpus exercises" (also machine-summary interpretation) where the report's interpretation says "names the symbol"; caveat present, sampling contradicts nothing
[P4] [implementation] crates/sley-repo/src/gc.rs:1066-1069 - "nothing is deleted for that run" holds only when the reachable candidate precedes every unlink; a later-positioned corrupted entry would trip the guard after earlier unreachable unlinks with no partial report; a whole-plan intersection check before the loop would make the claim unconditional
SUMMARY: All eleven prior findings are closed at 43f2f5b: the machine summary and tracked report agree (56/44/3/9, --check PASS); T07 now has a live-binding fault seed and T40 an injected-plan guard test, both run green and both are legitimate seeds for their P0 properties with no production deletion behavior changed; T20, T35, T41, T42, T56 are recorded accurately; T29's clock claim is in the marker list; the structural rule now requires every addendum row symbol-free and the duplicate T08 row is gone. Reading the driving test, corpus, fuzz target or oracle for 24 of the 44 exercised rows and re-confirming the rest, every P0/P1 threat has a realized control that a fault-seeding check actually reaches, the three structural controls hold under independent grep, and the report's own interpretation is honest. What remains is P3/P4: the host-ABI hygiene scanner and the coverage builder both cut at the first `#[cfg(test)]` token, so the scanner covers a fraction of the VM and the builder can still self-credit symbols defined in nine top-marked files (mis-classifying nothing today), and T48 is traceable only by substring accident. PASS with follow-ups.
```

SECTION: threat_coverage
FIELD: independent_security_review
