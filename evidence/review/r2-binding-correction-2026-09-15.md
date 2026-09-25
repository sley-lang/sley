# R2 / AR-06 live evidence binding review

Date: 2026-09-15. Repository: `/home/dev/Work/workspaces/sley2`.
Observed HEAD: `6989658395c88e4c7d568b376ce3efb014ab3c2b` plus the initial live-binding worktree changes.
Scope: `scripts/check_r2_exit.py`, new `scripts/r2_execution_evidence.py`, and `bench/review/tests/test_r2*.py`, traced into their actual test/checker inputs. This report describes the initial implementation reviewed before the parent agent's follow-up corrections; subsequent changes need a delta recheck.

## Verdict

**REVISE for complete AR-06 implementation closure.** The change genuinely repairs summary-only RW060 evidence and adds current-source review binding, but two substantive coverage gaps and one end-of-gate consistency gap remain below. Existing code is fail-closed on the observed missing current reviews, so this is not a claim that today's gate returns READY.

No qualifying Ariadne/Nabu/premium verdict is supplied by this review. Historical RW070 evidence remains distinct from current RW075 review requirements. The provided gate log reports NOT_READY with live RW060 PASS and all three current RW075 reviews PENDING; that is expected review debt, separate from the remaining implementation corrections.

## Confirmed improvements

- The lifecycle now runs the fixed `cargo test -p sley-repo --test rw060_source_free_lifecycle --locked -- --show-output --test-threads=1` command, checks its exit code, requires a complete unfiltered/unignored successful result, checks five named lifecycle tests, and verifies the emitted frozen roots/transaction/observations/pack identity. Mutable summary fields no longer establish lifecycle success.
- Source binding includes crate sources, build manifests/lock/toolchain, gate Python scripts, relevant frozen profile directories, tracked missing inputs, and untracked nonignored additions. Selected symlink/nonregular inputs refuse.
- Source identity is checked before and after the lifecycle command. Execution output is hashed and the source digest is reported.
- The latest non-infrastructure RW075 lane is selected by numeric round. It must carry exactly one `SOURCE_R2_SHA256: <current digest>` line. A stale later review does not fall back to an earlier pass. Within-file FAIL precedence and exact-token boundaries are preserved. Premium uses the same binding rule.
- RW070 historical review parsing is not silently reclassified as current-source RW075 acceptance. R3 work remains excluded from R2 prerequisites.

## Findings

### R2-BIND-1 — required: include actual external build/runtime inputs

- **File:** `scripts/r2_execution_evidence.py:44`
- **Status:** open in initial reviewed implementation.
- **Evidence:** The selection includes no `docs/` and excludes all `evidence/`, yet `crates/sley-schema/src/lib.rs:14` compiles `docs/spec/SSMC1_EPOCH1_SCHEMA.txt` through `include_bytes!`. The RW060 test directly reads `evidence/validation/anti-goal-conformance.json` at `crates/sley-repo/tests/rw060_source_free_lifecycle.rs:2310`. Neither file participates in the source digest. Aggregate subordinate checkers also read specification files outside the digest. Thus the claimed source binding does not identify the full input surface of the executed evidence or the reviewed gate contracts.
- **Required correction:** Bind the complete repository-controlled source/build/contract/fixture input surface, including external Rust embeds. Explicitly bind runtime evidence files consumed by executed tests, either within the candidate digest or in a separately mandatory, review-bound input manifest. Exclude review transcripts and mutable mirrors to avoid a circular identity, but do not exclude every generated evidence file indiscriminately when a required test reads one. Add focused inventory tests showing external embedded input and runtime-input changes invalidate the binding.
- **Scope note:** This was established by source/dataflow inspection. No source mutation or exploit demonstration was performed.

### R2-BIND-2 — required: execute affected RW075 successor closure fixtures

- **File:** `scripts/r2_execution_evidence.py:15`; `scripts/check_r2_exit.py:158`
- **Status:** open in initial reviewed implementation.
- **Evidence:** The only Cargo command runs the RW060 repository lifecycle. That driver calls `execute_function` through the legacy/reference lifecycle and explicitly uses no adapter inventory; it does not exercise the current v2 package admission/approval/execution route. The six aggregate successor checkers verify frozen JSON, document markers, and source markers; they do not execute affected production admission/package fixtures. This leaves the original premium AR-06 requirement for a revision-bound result of affected production execution/admission fixtures unmet (`machineresearch/sley-2.0/reviews/reweave-rw075-premium-r1-2026-09-06.log:118`). A digest that includes their sources is not a behavioral result for those paths.
- **Required correction:** Add bounded existing RW075 integration suites to the source-bound live runner, require successful completion of explicitly selected important tests, and bind their result/output to the same candidate digest. Include genuine v2 admission, package approval/execution and frozen observation behavior. Preserve RW060 as its own requirement and preserve the independent-review gate. Do not substitute new source-marker checks for these results.
- **Existing native commands:** See the next section; no new failure-inducing program or exploit is needed.

### R2-BIND-3 — required: recheck candidate identity after the whole gate

- **File:** `scripts/r2_execution_evidence.py:99`; `scripts/check_r2_exit.py:268`
- **Status:** open in initial reviewed implementation.
- **Evidence:** The final digest check occurs immediately after the lifecycle run. The aggregate then reads frozen records, runs multiple subprocess checkers against the live worktree, and evaluates transcripts before reporting readiness. There is no digest comparison after those later steps. A normal concurrent edit during those steps can therefore produce a gate result combining lifecycle/review evidence for the initial digest with later source/checker state.
- **Required correction:** Recompute and compare the candidate digest at the end of aggregate evaluation, after behavioral suites, subordinate checks, and review selection, and fail closed on change. For stronger reproducibility, execute from an immutable snapshot/worktree; the bounded correction here is the final comparison. This does not claim to solve arbitrary change-and-revert races or authenticate the machine.

## Existing bounded behavioral suites

Use the same fixed Cargo command structure and bounded timeout as the lifecycle runner. Run these as repository unit/integration validation; the review does not request creation of new exploit inputs.

```sh
cargo test -p sley-vm --test rw075_raw_callable --locked -- --show-output --test-threads=1
cargo test -p sley-vm --test rw075_exec_closure --locked -- --show-output --test-threads=1
cargo test -p sley-vm --test rw075_hydration_workloads --locked -- --show-output --test-threads=1
cargo test -p sley-vm --lib admission_authority::tests --locked -- --show-output --test-threads=1
```

- `rw075_raw_callable` contains actual v2 receipt/admission/approval/execution, including `raw_successor_package_binds_and_mismatches_refuse`, `raw_staged_authority_binds_graphs_to_image`, and `v2_package_section_digests_and_observation_are_frozen` (`crates/sley-vm/tests/rw075_raw_callable.rs:481`, `:630`, `:1689`). It also covers raw callable boundaries and cross-version rules.
- `rw075_exec_closure` covers preserved package execution/closure behavior and the current authority helper. It complements the v2 suite; it should not be described as an exclusively v2 suite.
- `rw075_hydration_workloads` provides composed ordinary workloads and host-boundary regressions. Its production calls predominantly use the v1 API despite a header reference to v2; describe the result accurately.
- `admission_authority::tests` covers stable error codes and the deliberately unavailable Sley-evidence minting ingress. It is a filtered library group: unrelated filtered-out tests are expected and must not be mistaken for omitted required tests. Require the exact selected group tests and no ignored/failed selected tests rather than weakening success parsing globally.

## External input inventory found

These are concrete inputs traced during this review, not a claim that a hand list will cover future transitive dependencies:

| Input | Consumer |
|---|---|
| `docs/spec/SSMC1_EPOCH1_SCHEMA.txt` | Compiled by `sley-schema` and transitively linked into RW060/VM tests |
| `evidence/validation/anti-goal-conformance.json` | Direct runtime read by RW060 source-absence test |
| `docs/spec/HOST_ABI_V1.md`, `docs/spec/HOST_ABI_V2.md` | Aggregate host-ABI freeze checkers |
| `docs/spec/EXEC_PACKAGE_V1.md`, `docs/spec/EXEC_PACKAGE_V2.md` | Aggregate execution-package freeze checkers |
| `docs/spec/BOOTSTRAP_PROFILE_2.md` | Aggregate successor bootstrap-profile checker |
| `docs/spec/RAW_HASH_V1.md`, `docs/spec/HOST_HYDRATION_V1.md` | Also directly read by the v1 execution-package checker |
| Frozen JSON and checksum files under bootstrap-profile, host-ABI, exec-package and raw-hash conformance directories | Already selected by initial digest; retain them |
| Crate manifests, Cargo.lock and root Cargo.toml | Both Cargo inputs and explicit RW060 source-absence checks; already selected |
| `crates/sley-vm/src/{extended,host_abi,exec_package,raw_hash}.rs` | Embedded in existing RW075 integration tests; already covered by `crates/` |

A broad repository-controlled source/docs/conformance inventory plus explicit runtime evidence dependencies is less fragile than adding only today's first missing embed. If broadening includes generated release records or review logs, take care not to make writing the qualifying review change the candidate identity.

## Validation and evidence limits

- Independently ran `python3 -m unittest discover -s bench/review/tests -p 'test_r2*.py' -v`: **7 passed**, exit 0.
- Inspected the supplied `r2-live-gate.log`: live RW060 command exit 0; no lifecycle problems; source digest `9ad2607d43b79f02f7108eef4bece58dfba83380e28bea14483de04f96bf15d7`; output digest `ef2dc8d881c398a5dd0d96b1e0160b9421dfb944ea340503c0e8b4a863e70626`; aggregate NOT_READY, required current reviews PENDING, architecture blocker open.
- The parent reported aggregate exit 3; the text log itself records NOT_READY but does not contain a separate process exit receipt. I did not claim a newly executed aggregate gate during simultaneous correction work.
- Read the existing native suites to identify exact production paths and commands. Their passing status is not established by this review's parser tests or by the old RW060-only log.

## Maintainability

The helper module is small and its separation of input identity, output validation, and subprocess execution is understandable. As more suites are added, prefer one explicit suite-spec table and shared result validation over one hard-coded command and duplicated parsers. Keep each suite's required names, output identities, and filtered-test policy explicit. The existing import-time aggregate gate forces unit tests to extract functions with AST; an import-safe `main()` would simplify future testing, but that refactor is not a blocker for the three corrections above.

This is local trusted-runner evidence, not a signed attestation of the compiler/runtime environment. No formal Council verdict, review-lane impersonation, release approval, or source edits were performed.


## Delta re-review — 2026-09-15, current implementation verdict

**The scoped AR-06 evidence-binding implementation corrections are closed.** This verdict supersedes the initial REVISE verdict for the three implementation findings; their original observations above remain preserved. No open implementation defect was found in this bounded delta re-review after the expected-panic parser correction. This does not provide the required current Ariadne, Nabu, or premium architecture verdict, and does not authorize R2 exit or a release.

Observed HEAD remains `6989658395c88e4c7d568b376ce3efb014ab3c2b` plus the reviewed worktree changes. The independently executed aggregate bound all five native suite results and its final stability check to source digest `15e02e249950652da4f53a21cf130e6ee628c9047c6d54915cb7ab663d701f58`.

### Closure matrix

| Finding | Current status | Verified correction |
|---|---|---|
| R2-BIND-1 | Closed | `scripts/r2_execution_evidence.py:27` makes the externally compiled schema and runtime anti-goal evidence mandatory. The inventory at `:54` now includes all crate files, `.cargo/`, `conformance/`, `docs/spec/`, Python gate scripts, Makefile and host-boundary input, alongside root Cargo/lock/toolchain inputs. The inventory regression at `bench/review/tests/test_r2_execution_evidence.py:49` independently confirms changes to every mandatory input change the digest, while a mutable summary does not; missing tracked inputs refuse. This covers the concrete external inputs and indirect source/checker contracts identified above. |
| R2-BIND-2 | Closed | `scripts/r2_execution_evidence.py:32` identifies required behavioral tests; `:131` runs the three full integration suites and selected admission-authority library group through the shared source-bound runner at `:102`. `scripts/check_r2_exit.py:115` makes their aggregate result mandatory. Success requires matching actual named-pass totals, no failed/ignored selected tests, and each required named test; only the explicitly filtered library group permits unrelated filtered tests. The independent live aggregate passed all four successor/admission suites with exit 0 and no validation problems. |
| R2-BIND-3 | Closed | `scripts/check_r2_exit.py:316` checks source identity after behavioral execution, subordinate checks and review evaluation; `:320` makes that result part of readiness. The independent live aggregate reports the same source digest through this final check. |

### Parser regression found and corrected during delta review

The first delta implementation rejected the successful RW060 run because three normal libtest expected-panic successes use `test NAME - should panic ... ok`, while its named-pass parser accepted only `test NAME ... ok`. An independently executed existing RW060 suite returned exit 0 and 15 passed, but the old validator counted only 12 and reported `missing-complete-successful-test-run`.

The implementer corrected `scripts/r2_execution_evidence.py:78` to accept the exact optional ` - should panic` marker. The regression at `bench/review/tests/test_r2_execution_evidence.py:40` verifies the successful form and rejects FAILED/ignored forms. Matching totals, required names, no-failure/no-ignore summaries, and command exit checks remain mandatory. Status: **closed**, verified by both the parser regression and the subsequent real aggregate run. No new exploit or failure-inducing application input was created for this review.

### Independent validation

- `python3 -m unittest discover -s bench/review/tests -p 'test_r2*.py' -v`: **9 passed**, exit 0.
- `python3 scripts/check_r2_exit.py`: **NOT_READY**, observed process exit **3**. Full captured output: `/home/dev/Work/checkpoints/sley2-finish-20260915/r2-binding-review-live-gate.log`.
- RW060 lifecycle: **PASS**, command exit 0, no problems, emitted identities accepted; output SHA256 `63891199934fd82a8e8655f1f65f74083bd36e903cccf0da8f5377090f22eab5`.
- `rw075_raw_callable`, `rw075_exec_closure`, `rw075_hydration_workloads`, and `admission_authority::tests`: **PASS**, each command exit 0, no problems, output digest and same source digest recorded individually in the log.
- All six subordinate profile/package/ABI checker results, frozen bindings, preserved failure history and historical RW070 lane predicates: **PASS**.
- End-of-aggregate source consistency: **PASS**. No concurrent source drift was observed during this review run.
- The only failing gate predicates are the three current RW075 review requirements, each **PENDING**, plus the architecture blocker that remains open until the premium requirement is met. This is the intended fail-closed result, not an implementation PASS being relabeled as a Council verdict.

The explicit suite table and shared execution/parser helpers remain understandable at this size. The earlier optional import-safe `main()` observation remains a maintenance opportunity, not an unresolved AR-06 correction. The digest identifies repository-controlled inputs in scope; this local trusted-runner evidence does not authenticate arbitrary environment changes, compiler binaries or a malicious local actor. This review made no source edits and supplies no global audit or release approval.
