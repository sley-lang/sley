<!-- engine: claude-code; observed model: claude-opus-5; scope: 178873d7f9df2da5e616a986a24db8c2559f8ca0; role: ariadne; field: ariadne_contract_review_closure; dispatched: 2026-09-18T18:14:56Z; duration_s: 180; process_exit_code: 0 -->
# Ariadne Council review — root_backed_query_profile
Harness: claude-code
Reviewed checkpoint: 178873d7f9df2da5e616a986a24db8c2559f8ca0

Scope verified: `git rev-parse HEAD` = `178873d7f9df2da5e616a986a24db8c2559f8ca0` (match). Read-only session; no file created, edited, built, or committed; cargo not run.

**Transcript location for this row.** The register row (`section=root_backed_query_profile`, `field=ariadne_contract_review`, disposition `PASS_WITH_P3_P4_FOLLOWUPS_NO_P0_P1_P2`) was traced with `git log -S'"ariadne_contract_review": "PASS_WITH_P3_P4_FOLLOWUPS_NO_P0_P1_P2"' -- machineresearch/sley-2.0/machine-summary.json` → commit `97c1ec30` (2026-09-13), whose `ariadne_contract_review_note` at that commit reads "PASS via harness ariadne final 2026-09-13 on a4b6029… (transcript evidence/review/verdicts/root_backed_query_profile/ariadne_contract_review-a4b6029.md): … 3 P3 record + 3 P4 editorial follow-ups". The originating transcript is therefore `evidence/review/verdicts/root_backed_query_profile/ariadne_contract_review-a4b6029.md` (read in full, lines 1-64; its own verdict line 56 is `PASS_0_P0_0_P1_0_P2_3_P3_3_P4`). The later `_revision_2..5` fields and their transcripts (a809906, c1d4177, e050fe7, 400895e, 97b9117) are separate register fields with their own recorded counts and are not the subject of this row.

**Commands run**
- `git rev-parse HEAD` → 178873d7…
- `git show 97c1ec30 --stat`; `git show 97c1ec30 -- machineresearch/sley-2.0/machine-summary.json` (base-field provenance)
- `git log -S` / `-G` for each closing text (results cited per finding below)
- `python3 scripts/check_root_backed_query_profile.py` → returncode 0; `"result": "PASS"`, `"problems": []`, `"revision": 7`, `"expected_revision": 7`, `"machine_summary_revision": 7`, `"query_classes": 19`, `"status": "S20_310_FULL_IMPLEMENTED_REVIEW_PENDING"`
- `sha256sum -c SHA256SUMS` in `conformance/root-backed-query/v1` → returncode 0; `accepted.json: OK`, `rejected.json: OK`
- Corpus parsed with python3: 27 accepted vectors (`class-01`..`class-19`, `page-edges-1/2`, `page-entry-points-1/2`, `page-namespaces-1/2`, `page-roots-1/2`); 10 mutations including `binding-substituted-fact` → `QUERY_ROOT_MISMATCH` 31008 and `arm-1-snapshot-profile` → `QUERY_PROFILE_UNSUPPORTED` 31000
- Not run: `scripts/generate_root_backed_query_fixtures.py --check` (invokes cargo; prohibited by the session rules). Corpus integrity is evidenced by the SHA256SUMS check and the pure-python checker only.

**Files read (line ranges)**
- `evidence/review/verdicts/root_backed_query_profile/ariadne_contract_review-a4b6029.md:1-64`
- `machineresearch/sley-2.0/machine-summary.json` → `root_backed_query_profile` (`contract_revision`, `fixture_vectors`, `status`, `checker`, lane fields)
- `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:3, 64-99, 254-263`; rev-5 text at `a4b6029` for the same passages via `git show`
- `docs/WORK_PACKAGES.md:36`
- `scripts/check_root_backed_query_profile.py:46-53, 210-231`
- `crates/sley-query/src/root_query.rs:211-238, 688-709, 896-927` and grep of `3363-3404` (arm-1 build/execute pin)
- `docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md:3, 46-62` (spot check only)

## Evidence checked

The a4b6029 transcript names exactly six findings at the unclaimed severities (three P3, three P4; lines 42-47 and 57-62). Each was checked against the source at 178873d7:

**P3-1 `[record] machineresearch/sley-2.0/machine-summary.json (root_backed_query_profile.contract_revision = 4, fixture_vectors = 23)`** — CLOSED. At HEAD the section carries `contract_revision = 7` and `fixture_vectors = 27`, matching the spec status line (`ROOT_BACKED_QUERY_PROFILE_V1.md:3`: "revision 7 (2026-09-15)") and the parsed 27-vector corpus. Closing commits: `fixture_vectors: 27` landed in `97c1ec30` (the same integrator close the transcript anticipated); `contract_revision` was re-pinned to 7 at `c1d41778` ("Records: S20-310 contract_revision 7 pinned after the revision-7 merge"). The checker asserts summary-vs-spec equality (below) and reports `problems: []`.

**P3-2 `[record] docs/WORK_PACKAGES.md:36 — S20-310 row states "contract draft revision 4 (2026-09-11)"`** — CLOSED. `docs/WORK_PACKAGES.md:36` now reads "full profile: implemented under contract draft revision 7 (2026-09-15; …)" with the revision 5/6/7 history. Closing commit: `f02f8de9` (`git log -S'contract draft revision 7' -- docs/WORK_PACKAGES.md`).

**P3-3 `[record] scripts/check_root_backed_query_profile.py:218-228 — revision reported but never asserted against the section record`** — CLOSED. `scripts/check_root_backed_query_profile.py:48-50` pins `CONTRACT_REVISION = 7`; `:214-217` asserts the spec status-line revision equals the pin (`spec-revision:…!=…`) and `:226-227` asserts `section["contract_revision"] == revision` (`machine-summary:contract_revision:…!=spec:…`). Both assertions are unconditional (not gated on COMPLETE status), which is stricter than the transcript's "assert at COMPLETE status" recommendation. Closing commit: `f02f8de9` ("… checker revision binding"). Run at HEAD: returncode 0, PASS, zero problems. The c1d4177 transcript (lines 100-110) independently recorded the negative test (revision reverted → FAIL with both problem strings).

**P4-1 `[contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:243-247 — page-union sentence duplicated verbatim`** — CLOSED. The rev-5 text at a4b6029 carried "…the union of the pages is the complete result and keys strictly increase across the walk, so no page can hide a fact. Because `total_count` is exact on every page and the key order is canonical, the union of the pages is the complete result and no page can hide a fact." At HEAD `:254-259` is a single merged sentence: "then, because `total_count` is exact on every page and the key order is canonical, keys strictly increase across the walk, the union of the pages is the complete result, and no page can hide a fact." Grep for `union of the pages` / `no page can hide a fact` returns one occurrence each (lines 258-259). Closing commit: `f02f8de9` ("section 3 duplicate").

**P4-2 `[contract] docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:76-77 vs :69-74 — arm condition retained in numbered rule list`** — CLOSED. Rev-5 rule 1 read "`snapshot.completeness = CompleteRoot(2)` and `snapshot.context = (schema_epoch, Some(root))`". At HEAD `:89-90` rule 1 reads "`snapshot.context = (schema_epoch, Some(root))` (the completeness arm has already passed the profile gate above)" — the arm condition is no longer a `QUERY_ROOT_MISMATCH` rule; the prose carve-out at `:82-87` stands unchanged. Closing commit: `a8672ba1` (`git log -S'has already passed the profile gate above'`). Engine order still matches: `build_root_query_request` (`root_query.rs:699-705`) and `execute_root_query` (`:904-909, 923`) each run limits → arm (`ProfileUnsupported`) → shape → cursor → `verify()`.

**P4-3 `[implementation-doc] crates/sley-query/src/root_query.rs:211-222 — verify() doc comment lacks the section 1 arm carve-out`** — CLOSED. `root_query.rs:214-221` now states the section 1 carve-out explicitly: arm disagreement is routed through the arm gate as `QUERY_PROFILE_UNSUPPORTED` (precedence item 2) at both entry points before `verify()` runs, and the completeness comparison at `:233` is "a defensive residue that the gated entry points never reach with an arm-1 snapshot; it is not the `QUERY_ROOT_MISMATCH` arm rule." Closing commit: `f02f8de9`. The unreachability claim is still pinned by the test at `:3363-3404` (arm-1 snapshot answers 31000 at build, and at execute if build were to succeed) and by the corpus mutation `arm-1-snapshot-profile` → 31000.

**New-defect sweep at these paths.** Machine-summary section, WORK_PACKAGES row 36, checker, spec sections 1 and 3, `verify()` and both entry points, and the corpus/SHA256SUMS were re-read for regressions; the closeout audit's corpus bullet (`:62`) reads "twenty-seven vectors" and its status line names revision 7. Nothing new found. Later-round P3s (e050fe7 record-gap, c1d4177 closeout counts) belong to the separately recorded `_revision_3/_revision_4` fields, not this row, and the closeout text they cite is already at 27/revision 7 at HEAD.

## Findings
None.

## Assessment
All six findings the a4b6029 transcript carried at P3/P4 are verified closed at 178873d7 by direct re-reading of the current text and code, with closing commits identified by `git log -S` (`97c1ec30`, `c1d41778`, `f02f8de9`, `a8672ba1`). The record/checker items are now enforced mechanically (checker PASS, zero problems, returncode 0, revision 7 asserted on both sides), the two spec editorial items are gone from the text, and the `verify()` doc comment states the section 1 carve-out that the engine order and the 31000 pins already enforced. No new P0-P4 defect was found at the reviewed paths. Limitation: the cargo-backed generator `--check` was not run under the read-only rules; corpus integrity rests on the SHA256SUMS check and the python checker for this review. This closure does not speak to the section's broader status (`S20_310_FULL_IMPLEMENTED_REVIEW_PENDING`), the other two unclaimed rows for this section (nabu, vulcan), or any release readiness claim.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_0_P4_PRIOR_P3_P4_CLOSED
SECTION: root_backed_query_profile
FIELD: ariadne_contract_review_closure
SCOPE_SHA: 178873d7f9df2da5e616a986a24db8c2559f8ca0
FINDINGS: none
SUMMARY: The originating transcript for this row is ariadne_contract_review-a4b6029.md (base field set at 97c1ec30), which named three P3 record findings and three P4 editorial findings. Each is verified closed at 178873d7: machine-summary contract_revision 7 / fixture_vectors 27 (97c1ec30, c1d41778), WORK_PACKAGES row 36 at revision 7 (f02f8de9), checker asserting spec and record revision unconditionally with PASS and zero problems (f02f8de9), the section 3 duplicate sentence merged (f02f8de9), rule 1 rewritten without the arm condition (a8672ba1), and the verify() doc comment carrying the section 1 carve-out (f02f8de9) with the arm gate still first at both entry points. No new defects were found at these paths; the cargo-backed generator check was not run under the read-only rules.
