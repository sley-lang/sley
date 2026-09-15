# Ariadne Council review — root_backed_query_profile

Harness: claude-code
Observed model: claude-fable-5-1
Reviewed checkpoint: e050fe75a86c1b2f0bf779561ac2bc70bce34289

Reviewer: Ariadne (contract conformance). Harness: Claude Agent SDK, model Claude Fable 5.1 (claude-fable-5-1). Checkpoint verified: `git rev-parse HEAD` = e050fe75a86c1b2f0bf779561ac2bc70bce34289, `git status --short` empty. Attested SOURCE is 9b4f0648 (the candidate commit); e050fe7 is the records-only closure reviewed here. Read-only: no file written, no builder run in write mode.

Assumption: `crates/sley-query/src/paging.rs` does not exist; the paging layer is `page()`/`page_items()` inside `crates/sley-query/src/root_query.rs`, which is what I reviewed.

Delta since the archived c1d4177 round, scoped to S20-310 (`git diff c1d4177..e050fe7` over the spec, `crates/sley-query`, `crates/sley-repo`, the checker, corpus, oracle):
- `crates/sley-query/src/root_query.rs:1903-1904` (a10d871): the doc comment on `single_item_classes_walk_two_items_at_limit_one` was shortened to two lines; the test body, engine non-test code, spec, checker, corpus, oracle, and fuzz target are byte-unchanged since c1d4177.
- `docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md:61-75, 79-96` (ede5ef0): corpus bullet now names 27 vectors with the four walks (page-namespaces, page-edges, page-roots, page-entry-points) and 10 mutations including the 31008 binding-substituted fact and the 31000 arm-1 snapshot; native-tests bullet names the revision-7 unit walk and the four arms it pins, and the counts "sley-query 104 tests, sley-repo 377 tests (plus the integration binaries), sley-id 7".
- `machineresearch/sley-2.0/machine-summary.json:879-884`: `current_delta_review` PENDING in all three lanes (these are the open review rows; acknowledged, not defects).
- `evidence/review/verdicts/root_backed_query_profile/vulcan_surface_review-c1d4177.md` archived at a10d871.

Checks actually performed (static; see limitations):
- Checker `scripts/check_root_backed_query_profile.py`: `ENGINE_MARKERS` (:119-129) binds only the function name `fn single_item_classes_walk_two_items_at_limit_one()`, so the doc-comment trim cannot fail the marker; `SPEC_REVISION_MARKERS` (:65-104) and the revision assertions (:208-214) are unchanged since c1d4177; summary `contract_revision` 7 (:816) equals the spec Status line.
- Spec section 9 (:474-489) carries the rationale the trimmed doc comment dropped (single-item classes, degenerate walks, unit-walk evidence rule), so no normative content was lost.
- Closeout counts against tracked records: `machine-summary.json:857` `fixture_vectors: 27`; `evidence/validation/test-inventory.json:303-306` sley-query 108 tests with 4 ignored = 104 passing, matching the closeout; sley-id 7 (:278-280) matches. The "sley-repo 377" figure is not derivable from the inventory (406 total, 4 ignored, all targets) and I could not run cargo; the closeout's own parenthetical marks it as lib-only, so I record it as unverified, not wrong.
- Prior findings: Ariadne c1d4177 P3 (stale corpus and native-test bullets) closed by ede5ef0; Vulcan c1d4177 P4 (23/8 counts) closed by the same edit; Nabu c1d4177 PASS. The a809906 items (section 7 attribution, class names, class-1 count, section 3 duplicate, section 9 rule, checker revision binding) were verified closed by all three c1d4177 lanes against the primary sources; nothing in the current delta touches them, and the checker bindings I re-read still pin them.

Findings:
- The c1d4177 delta round itself is nowhere registered. `grep c1d4177` over the machine summary, `evidence/review/finding-register.json`, and `docs/WORK_PACKAGES.md` returns nothing; the base lane fields (:829, :832, :835) and their notes (:867-869) describe only the a809906 round, and ede5ef0's closure of the c1d4177 items is recorded only in a commit message. The register is built from the summary (`build_finding_register.py:293-329`), so it carries no obligation for that round and dossier item 29 undercounts the recorded review history. Same class as the unregistered-round P3s this project has already accepted (0bcc9c6, 9ae09a1).

Limitations: the sandbox refused every `python3 scripts/...` and `python3 -c` invocation and any `cargo` run, so the profile checker, fixture `--check`, oracle, and `cargo test -p sley-query` were not executed here; the operator's recorded results (1419 passed / 16 ignored, quick PASS at 9b4f064) are context only. Every claim above rests on reading the files at e050fe7 and git diffs.

VERDICT: REVISE_0_P0_0_P1_0_P2_1_P3_0_P4
SECTION: root_backed_query_profile
FIELD: current_delta_review.ariadne
SCOPE_SHA: e050fe75a86c1b2f0bf779561ac2bc70bce34289
FINDINGS:
[P3] [record] machineresearch/sley-2.0/machine-summary.json:829-878 - the c1d4177 delta round (Ariadne REVISE_0_P0_0_P1_0_P2_1_P3, Nabu PASS, Vulcan REVISE_0_P0_0_P1_0_P2_0_P3_1_P4; transcripts evidence/review/verdicts/root_backed_query_profile/*-c1d4177.md) has no lane field or note in the summary, the register, or WORK_PACKAGES, and its repair at ede5ef0 is recorded only in that commit's message; the register (built from the summary) therefore omits the round and its closure. Add the next `_revision_N` field per lane with a note naming the transcript and ede5ef0, then rebuild register, GA report, and dossier (records-only).
SUMMARY: The S20-310 delta since c1d4177 is one test doc-comment trim and the closeout text repair; engine, spec, checker, corpus, and oracle are byte-unchanged, the checker's markers still bind the revision-7 paragraphs and the unit walk, and the closeout now describes the real 27/10 corpus and the revision-7 tests. The one defect is a record gap: the c1d4177 round and its ede5ef0 closure are unregistered, so the register cannot show that lineage. REVISE with a single P3; checkers and cargo were not executable in this sandbox.
