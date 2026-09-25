# Vulcan Council review — root_backed_query_profile

Harness: claude-code
Observed model: claude-fable-5-1
Reviewed checkpoint: e050fe75a86c1b2f0bf779561ac2bc70bce34289

Delta since the archived c1d4177 round: `git log c1d4177..e050fe7 -- <S20-310 surfaces>` touches exactly two files. `ede5ef0` rewrote the closeout's corpus and native-test bullets; `a10d871` (an S20-750 integration commit) replaced the 16-line doc comment on `single_item_classes_walk_two_items_at_limit_one` with two lines. `git diff --stat c1d4177..e050fe7` over `crates/sley-query/src/root_query.rs` non-test code, `snapshot.rs`, `crates/sley-repo/src/{root_query,index_cache}.rs`, `conformance/root-backed-query`, the oracle, the fuzz target, the fuzz-slice checker, and `check_root_backed_query_profile.py` is empty apart from that comment. The spec is byte-unchanged at revision 7. There is no `paging.rs`; paging lives in `page()`/`page_items()` (`root_query.rs:1284-1512`), re-read and unchanged.

Checks performed: `grep -c '"id":'` gives 27 accepted vectors and 10 rejections; `evidence/validation/test-inventory.json:303-311` records sley-query 108 tests / 4 ignored (104 run) and sley-repo 406 / 4 ignored, consistent with the closeout's "104 ... 377 (plus the integration binaries)"; the machine summary holds `contract_revision: 7`, `fixture_vectors: 27`, three lane fields held at prior PASS forms with `current_delta_review` PENDING in all three lanes, and the register carries those three PENDING rows plus `contract_text_review`. Not executed: the profile checker, fixture `--check`, oracle, `cargo test`, `sha256sum -c` (cwd-bound).

Prior findings: my c1d4177 P4 (closeout corpus 23/8 vs 27/10) is closed by `ede5ef0`: the bullet now names 27 vectors, all four walks (`page-namespaces`, `page-edges`, `page-roots`, `page-entry-points`), 10 mutations including the 31008 binding and 31000 arm-1 rejections, and the native-tests bullet names the unit walk and the 104/377/7 counts. Ariadne's c1d4177 P3 (same bullets) is closed by the same commit. Nabu's c1d4177 verdict was PASS. Section 7, section 9, the checker revision pin, and the engine are as verified at c1d4177.

Adversarial re-read of `page_items` (strict `>` cursor filter, `truncated = remaining > limit`, `RequiredFactOmitted` without `allow_continuation`, single-key chains never paged) found nothing new; the fuzz target and oracle are unchanged.

Limitations: no gate executed; verdict rests on the byte-unchanged mechanics plus the recovery record's `quick_exit: 0` at 9b4f064, which postdates both delta commits.

VERDICT: REVISE_0_P0_0_P1_0_P2_0_P3_2_P4
SECTION: root_backed_query_profile
FIELD: current_delta_review.vulcan
SCOPE_SHA: e050fe75a86c1b2f0bf779561ac2bc70bce34289
FINDINGS:
[P4] [record] crates/sley-query/src/root_query.rs:1903-1904 - the revision-7 doc comment stating the section 9 evidence rule in code (cited at :1903-1920 by the Ariadne and Nabu c1d4177 transcripts) was replaced at a10d871 under an S20-750 commit message ("Bind qualification reports...") on the advice of the development review, with no S20-310 record (closeout, packet, summary) naming the edit; not checker-bound, no gate moved, but an engine-file edit to a reviewed contract surface should be recorded by its owner; restore a one-line "section 9 evidence rule" pointer or note the edit in the closeout
[P4] [record] machineresearch/sley-2.0/machine-summary.json:866 - contract_text_review = PENDING_S20_310_WORDING_DECISION_PACKET_ROUND7_P1_4_IMPLEMENTED: the register's severity grammar reads the packet item range "P1_4" as a P1 mention (evidence/review/finding-register.json:5934-5939 severities ["P1"]), so the open S20-310 row carries a phantom P1 that severity_mentions counts; rename the token (e.g. ROUND7_ITEMS_1_TO_4_IMPLEMENTED) so a wording-packet reference cannot read as a severity
SUMMARY: The S20-310 mechanics, corpus, oracle, fuzz slice, checker, and revision-7 spec are byte-identical to the c1d4177 round I already verified; the only deltas are the closeout bullets (which close my c1d4177 P4 and Ariadne's P3 with figures I re-derived: 27 vectors, 10 mutations, 104 sley-query tests) and a test doc comment shortened by an S20-750 commit. Two P4 record items remain: the unrecorded doc-comment edit and a packet token the register misreads as a P1 severity. No gate ran here; the recovery record's quick pass at 9b4f064 postdates both deltas.
