# Independent scoped security/record correction review — 2026-09-15

Baseline: `4846bb6a2333efa6360bec3ed2296368943a338b`; reviewed uncommitted changes, identified by the file hashes below. Reviewer: separate Codex `remaining_review_audit` subagent, read-only source review. Parent implements and runs validation.

## Verdict

**PASS for the final inspected correction scope, static review. No remaining actionable finding introduced by this delta was found after the two corrections below.** This is not full security acceptance, GA acceptance, a Council Vulcan verdict, RW075/R2 qualification, or acceptance of the remaining 41 carried finding rows.

Original reference read in full: `evidence/review/verdicts/threat_coverage/independent_security_review-43f2f5b.md`. Its historical disposition remains unchanged, and the current records explicitly say independent security closure is pending.

## Findings found and corrected during this review

### SR-01: isolated S20-530 checker could no longer import — CLOSED

The first version extracted `rust_char_literal_end` and `rust_code_mask` and added a plain sibling import to `check_s20_530_crash_recovery.py`. Its authoritative invocation deliberately uses isolated Python. `verify_s20_530_accepted_state.py:139` invokes `/usr/bin/python3 -I -B`; the checker also records that exact invocation in review receipts. Isolated mode omits the script directory from the import path.

Benign verification actually executed by this reviewer: `/usr/bin/python3 -I -B scripts/check_s20_530_crash_recovery.py --help`. It failed at the new import with `ModuleNotFoundError: No module named 'rust_source_regions'`, exit 1, before any gate ran.

Parent restored the frozen checker exactly to baseline and retained an explicitly documented small lexical mirror in the new helper. I compared the restored file bytes to `git show 4846bb6a:scripts/check_s20_530_crash_recovery.py`: identical. I compared both mirrored function ASTs to the baseline: identical. No isolation, import-search-path, frozen contract, or trusted-file inventory change remains. The added parity test extracts only the frozen lexical functions into a test namespace and compares masks on three substantial Rust sources; it does not execute the aggregate checker or import ambient authority.

### SR-02: authorization audit pointer became an OTHER obligation — CLOSED

The first metadata correction introduced `completion_authorization_note.review_record_audit`. The register walks nested nodes; the parent key's `_note` suffix does not exempt descendant fields. `review_record_audit` therefore became an OTHER disposition whose value was a markdown path.

The parent renamed the leaf to `audit_record`, without changing the classifier. I directly called the existing collector on the updated summary: OTHER rows = `[]`; RW075 includes `independent_completion_review` = PENDING. No real obligation was removed by this correction.

## Inspected implementation and bounded conclusions

### Shared Rust source region helper

- `rust_code_mask` and `rust_char_literal_end` retain the original S20-530 logic (AST equality independently checked). The raw-string prefix expression is also the same at this snapshot.
- The helper is explicitly a lexer and recognizes exact `cfg(test)` inline modules, not all possible Rust conditional or macro semantics.
- Masked comments and literals do not supply matching module markers or brace depth. The recognized opening brace is followed to its matching closing brace, so production after a module and between modules stays production.
- Nested known test modules are included inside their enclosing range without duplicate ranges. An unclosed recognized test module raises instead of silently truncating the file.
- Production text preserves positions/newlines and every byte outside recognized test-module ranges. Unknown cfg forms and test-only single items stay in the production scan, which is conservative for marker hygiene.
- The new tests cover production before/between/after modules, an external test-module declaration, nested comments, raw strings and character braces, a marker inside a literal, malformed module termination, and lexical parity with the unchanged isolated checker.

The helper does not pretend to expand macros, follow module inclusion, resolve names, or prove absence of host access. Its consumers remain textual marker checks. Keeping that boundary is necessary to avoid treating a successful scan as a semantic security proof.

### Host ABI and execution-package marker scans

Both now use the bounded production projection. The previous first-module truncation could omit production after the first test module; the new projection retains it. The host checker still scans its declared closure crate roots and unchanged forbidden marker set. Execution-package scanning keeps the same existing owners/exclusions and minter/receipt marker rules.

No new authority operation or runtime path is introduced. Literal/comment mentions outside known tests can still cause conservative marker failures, and aliases or other semantic forms are not resolved. Those are limits of this textual gate, not new evidence of full host isolation.

### Threat and error-symbol evidence location

- Threat `locate` and `exercised_in` share whole-identifier matching, closing the specific accidental substring credit described in the original T48 finding.
- Inline Rust exercise text now consists of complete known test modules, not a production-file suffix.
- Error-symbol scanning collects bounded inline test regions as exercise sources in addition to its existing corpus/file sources. The scanner no longer relies on production after an early test marker to make the newly tested symbols appear exercised.
- The report and summary now distinguish symbol mentions/explicit traceability from independently judged mitigation. The revised interpretation does not claim that a string match executes a control.

Existing limitations remain: a symbol-naming/freeze assertion can still count as a mention; sibling `*_tests.rs` files without inline modules are not comprehensively discovered by these rules; enum-variant matching is textual, not Rust type resolution. These scanners are evidence indexes. The current scoped change neither advertises semantic coverage nor claims to eliminate those pre-existing limits. Full security acceptance must still read the relevant tests/control paths.

### Added Rust tests

T48: the fixture adds a label only in test code; existing callers pass None. For each of three authority-like labels the test drives the real candidate validator with allowed and denied protected grants, requiring validity only for the allowed case and exact phase-9 `POLICY_GRANT_DENIED` otherwise. It also checks that the label and policy root remain unchanged. This is a meaningful narrow authority-independence regression, not merely a symbol table.

GC: duplicate retained roots exercise the constructor's `RootInvalid` branch; an anchor to a retained root absent from the root inventory exercises `RootMissing` in the real dry-run path. Exact symbols and surviving object paths are asserted. I read the corresponding production branches. These additions are test-only; this diff does not change deletion behavior.

Lowering: deleting one parameter from the BoolAnd fixture drives the real extended-profile operation judgment with an incomplete parameter inventory and checks the exact displayed `VM_LOWER_LOCAL_REFERENCE_INVALID` result. The final code uses `to_string()`; the earlier nonexistent `code_str()` call was corrected before this final review.

I did not execute these Rust tests. Their runtime outcomes remain the parent's validation responsibility.

### R2 exact verdict token correction (scope added by parent)

`review_verdict` and `latest_lane_verdict` now use the same trailing ASCII-identifier boundary as the existing premium parser. Longer identifiers no longer qualify as PASS/FAIL. Existing fail precedence, latest-round ordering, infrastructure-file exclusion and optional trailing notes are preserved. The added test module extracts only these functions, uses disposable benign review records, and checks longer tokens, an accepted scoped note and within-file FAIL precedence.

This corrects only prefix-token parsing. It does not bind a review to the source revision or authenticate RW060 lifecycle evidence; the earlier AR-06 finding remains explicitly open. Nothing in this report supplies the missing Nabu/premium acceptance.

## Record integrity

- Merlin's timed-out implementation handoff remains verbatim in a note with an audit pointer, with no new Merlin PASS.
- The legacy adapter's lane becomes metadata; its actual review dispositions and carried P3s remain.
- RW075's self-review is preserved verbatim as a note and replaced as an active gate input by an explicit PENDING independent completion obligation. Original premium failure and R2 blocked state remain.
- Security's prior PASS-with-findings is retained; the new note says repairs await independent closure.
- Operator authorization is recorded as authorization, not an empirical PASS. Completed two-host artifact identity is scoped to the old source; new source requires a new candidate.
- No GA, publication, or package terminal promotion was observed in this correction scope.

Generated register/report/dossier files were being regenerated concurrently. I reviewed the metadata source and collector results, not final global freshness or the release candidate. This report must not be used to bypass those checks.

## Verification actually performed or observed

Reviewer-performed: static source/transcript/diff inspection; the initial isolated import check; AST equality of the two lexical functions; byte equality of restored S20-530 checker; current collector observation (zero OTHER, explicit RW075 PENDING).

Parent-generated log read: `/home/gfarch/Work/checkpoints/sley2-finish-20260915/scanner-tokens-green.log` records 8 tests passing in 0.553 seconds. I did not rerun them and do not relabel that as independent execution. No complete security suite, full build, full R2 gate or benchmark campaign was executed by this reviewer.

## File identity at final inspection

- `scripts/rust_source_regions.py`: `8789896b8783c20f80ad7f71bc7d90037d6bca6858ea9b938f25f6378ab3438b`
- `scripts/check_s20_530_crash_recovery.py`: `7939c42cdd6b55e8284b221ba73f032ef3b48ecc477dcf7eba8bb7c10e65d95b`
- `scripts/build_threat_coverage_report.py`: `fdf5417259b8e95a2f63cf798e2616381d1ffbed35c64bf37b5ea9c1486fe20f`
- `scripts/check_error_symbol_registration.py`: `e9d8cba970839284a250c7aa9a300e582ad21500c539a143c7c4f81e7904fa7c`
- `scripts/check_host_abi_markers.py`: `4b81d0ef1595fd0def81590011a87ceb1c211ae136b25949bac672e924fab07b`
- `scripts/check_exec_package_markers.py`: `605ba65f241b57893a146ac2af59b95ffbfe497a84136f37237ad38dd39a2ba2`
- `scripts/check_r2_exit.py`: `9b0ec1e3e82a7359f75f9093f850b51548862927033593c472f248e3cdc598a2`
- `bench/review/tests/test_threat_coverage.py`: `5515ad7210b517a46556d72b33ee4e53116f5cc9c01cec0ca3ae6220e75b0929`
- `bench/review/tests/test_r2_review_tokens.py`: `026eee0caf5dd121aeb219166dcf2e7e0b016e3731a19ba675a79d778b3f52ab`
- `crates/sley-policy/src/candidate_validation.rs`: `0b4cee7363978b53588485cd220cd3d4d1949fa50df90ca9561068b1f800f701`
- `crates/sley-repo/src/gc.rs`: `33428c8fd07928e72d2f3ba4c375c67a24a78fe73bb9b005d446468277270c61`
- `crates/sley-vm/src/lower.rs`: `007d002545cf2adfc96b003f53c6b22e1046e1f11d59c3b466d45e02dbcc29dd`
- `machineresearch/sley-2.0/machine-summary.json`: `0c4b1a4392755291a7567bb20b4875d6a814f06dafe2ecd6548cf32960349e62`
- `docs/THREAT_REGISTER.md`: `3c1c2070fe2e64fa87c38e3c612f75520c43c97f2c2cc7f5a51c9f1d0b876c90`
