# Vulcan pre-mint final: re-qualification freeze (artifact surface + P-A substance)

Baseline verified: `git rev-parse HEAD` = `befd55f3156dd3a26645ac5b73f72804f6888bd7` (short `befd55f`, the P-A freeze). `git status --porcelain` is clean. Scope is the `befd55f` tree, with substance review of the P-A owner-scoped symbol amendments (`6a65331`, `520b47e`). Prior transcripts stay scoped to their commits and are preserved.

## What I reviewed (P-A substance)

| Amendment | Finding |
|---|---|
| `ERROR_CODES_V1.md` +20 family bullets (`6a65331`) | namespaces 52 → 72; every family names its owning contract; no blanket `CANDIDATE_*` — sibling CANDIDATE subgroups stay with their owners and the concurrence rule (`:72-80`) is preserved, not bypassed |
| Numeric claims in family bullets | verified against owning contracts: BRANCH 50011–50017 (`NATIVE_REFS_BRANCHES_V1.md:402-408`), EXCHANGE 54000–54021 (`REPOSITORY_EXCHANGE_V1.md:477-502`), CONTEXT_CAPSULE 32008–32011 (`CONTEXT_CAPSULE_PROFILE_V1.md:207-210`), CANDIDATE_RESULT 36100–36107 (`CANDIDATE_RESULT_V1.md:419`); unassigned/numberless stated where true (AUTHORITY, IMAGE, RAW_HASH) |
| `EXEC_PACKAGE_V2.md` authority vocabulary (7 rows, all LIVE) | `AuthorityError` enum (`admission_authority.rs:101-121`) carries exactly these 7 variants; meanings match the doc comments; no numerics claimed; RW-075/RW-080 semantics untouched; frozen `EXEC_PACKAGE` sections unamended (round-7 item 2 not triggered — frozen `PACKAGE_*` symbols are not emitted) |
| `CANDIDATE_RESULT_V1.md` new §8.3 (14 rows, phase-5 `INVALID_GRAPH`) | `CandidateApplyError` (`apply.rs:23-56`) carries exactly 17 variants = 2 wrappers with dedicated mappings + `ExactPreimageMismatch` (§8.1 phase-3) + the 14 tabled symbols; `code()` returns the documented strings verbatim (validator never renames); `INVALID_GRAPH` is a listed terminal state; `source_numeric None` + `Permanent` confirmed at `candidate_apply_failure` |
| `ENTITY_READ_PROFILE_V2.md` §4 aliases (3 rows) | Display impl (`entity_read.rs:90-92`) emits exactly these strings; wire keeps `PROTOCOL_*`/`QUERY_*` codes (40008/40009/31007); no new error numbers |
| `MUTATION_VALUE_CODEC_V1.md` (3 rows, numeric 0) / `RAW_HASH_V1.md` (2 rows, unassigned) | emissions confirmed in source (`value.rs:77`, `raw_hash.rs:91-92`); SMP1 §8 symbol-only convention cited; owner freezes numerics by amendment |
| `refs.rs` test-role rename (`520b47e`) | all 446 changed lines are the renamed literals; `ROLE_BRANCH_` occurs 0× outside `#[cfg(test)]` (365× inside); the 8 non-`ROLE_` `BRANCH_*` in non-test code are the registered failure symbols; no engine/precedence/spelling/numeric/behavior change |
| S20-350 concurrence | no S20-350 symbol claimed (`MUTATION_CAPSULE_*`/`MUTATION_CANDIDATE_*` untouched); the `CANDIDATE_APPLY_*` bullet documents validator ownership (emitting module declares S20-360, no S20-350 document names these symbols) |

## Compatibility assessment

**Numeric/behavioral compatibility: preserved.** Specs add assignment tables only; the sole code change renames test-fixture literals. Meaning, defining contract, family, and responsible owner established per symbol; no invented generic families, no suppressed diagnostics, no deleted obligations, no weakened validation; checker semantics unchanged.

**Owner procedure:** amendments executed through the owning contracts named above (commits `6a65331`/`520b47e`); technical review is this transcript plus the verification battery below. Passing checkers are recorded as verification, not as owner approval; no owner approval is manufactured and none reserved to another lane is consumed.

**Prior findings:** no new P0–P4 in this scope. Held out of scope (unchanged): second-host attestation for the new candidate, SBOM/provenance re-derivation, P-B wording, P-C reviews, signing/transparency, re-anchor, succession, independent review, council, publication, GA.

## Live command outputs

- `python3 scripts/check_error_symbol_registration.py --check` → `PASS`, 72 namespaces / 495 emitted / 315 numeric, unregistered [] / familyless [] / ambiguous [] / unexercised [], tracked==derived, exit 0
- `SLEY2_MASTER_GOAL=/home/gfarch/machinelibrary/Sley2.0mastergoal.md python3 scripts/check_candidate_result_contract.py` → `PASS`, 37 validator symbols, exit 0 (env prerequisite resolved via documented configuration; checker semantics unchanged)
- `check_exec_package_v2` → PASS; `check_mutation_value_codecs` → PASS; `check_entity_read_vectors` (via `uv run --project oracle/scb1 --frozen`) → PASS (91 rejections); `check_repository_exchange_spec` → `S20_540_COMPLETE`; `check_host_abi_v1` + `check_host_abi_markers` → PASS
- `cargo test -p sley-repo --lib` → 372 passed, 0 failed, 4 ignored
- `cargo check --workspace --locked --offline` → `Finished`, exit 0
- release/review/invariant suites → 106/106, 53/53, 9/9 OK

VERDICT: PASS / SECTION: s20_710_pre_release_audit / FIELD: final_vulcan_disposition / SCOPE_SHA: befd55f3156dd3a26645ac5b73f72804f6888bd7 / FINDINGS: 0_P0_0_P1_0_P2_0_P3_0_P4
