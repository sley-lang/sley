# Nabu Council review — root_backed_query_profile

Harness: claude-code
Observed model: claude-fable-5-1
Reviewed checkpoint: e050fe75a86c1b2f0bf779561ac2bc70bce34289

Assumption: the task names `crates/sley-query/src/paging.rs`; no such file exists (`find crates -name paging.rs` empty). Paging is the private `page()` inside `crates/sley-query/src/root_query.rs`; reviewed there.

Delta since the archived c1d4177 round (Nabu PASS, Vulcan REVISE 1 P4, Ariadne REVISE 1 P3): `git diff c1d4177..e050fe7` over the spec, `scripts/check_root_backed_query_profile.py`, corpus, oracle, and `crates/sley-repo` is empty. The only engine-file change is a10d871 shortening the doc comment of the unit walk (`root_query.rs:1903-1904`); the function name the checker binds is intact at `root_query.rs:1906`, and `pub fn run_root_query_fresh` at `crates/sley-repo/src/root_query.rs:152`. No non-test line of the engine moved; dependency direction unchanged.

Prior findings verified addressed: ede5ef0 rewrote `docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md:61-73` (27 vectors naming all four walks; 10 mutations naming the 31008 binding and 31000 arm-1 rejections) and `:79-96` (walks over four key types, the revision-7 unit walk named, `sley-query` 104 tests). That closes the Ariadne P3 and the Vulcan P4 of c1d4177. The test inventory records `sley-query` 108 tests with 4 ignored (`evidence/validation/test-inventory.json:303-306`), consistent with 104 passing. The two items the c1d4177 rounds left open by design (section 1 rule-1 conjunct, ADR-0030 section 11) remain outside the operator grant and are not counted.

Records: machine summary `root_backed_query_profile` pins `contract_revision` 7 with `current_delta_review` PENDING in all three lanes (`machine-summary.json:879-883`); base lane fields are held at prior forms pending this round, as the notes state. The `contract_text_review` row remains the section's open register obligation and closes only on lane PASS; acknowledged, not invented.

Checks not performed: checker, fixture `--check`, oracle, `cargo test` (sandbox). The c1d4177 transcripts executed them on byte-identical inputs; nothing in this delta touches those inputs.

VERDICT: PASS
SECTION: root_backed_query_profile
FIELD: current_delta_review.nabu
SCOPE_SHA: e050fe75a86c1b2f0bf779561ac2bc70bce34289
FINDINGS:
NONE
SUMMARY: The post-c1d4177 delta is records-only for this section (ede5ef0 closeout corpus/test bullets now match the revision-7 corpus and the named unit walk; a10d871 shortens one test doc comment without touching engine code, spec, checker, corpus, or dependency edges). Both prior open record findings are verified closed; the two by-design residues stay outside the grant. Nothing actionable for the owner.
