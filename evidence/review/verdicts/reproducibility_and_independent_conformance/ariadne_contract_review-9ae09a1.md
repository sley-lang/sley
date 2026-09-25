**Baseline verified:** `git rev-parse HEAD` = `9ae09a142830a4857c553bad27433143999e6864`. No files written. Working tree carries the two untracked RW-080 slices plus untracked `evidence/review/requests/REQ-05-entity-read-scoped.md`; none is adjudicated here.

## Method and assumptions

- Read-only. Tool policy again refused `python3` (both `build_independent_conformance_report.py --check` and an inline canonical-JSON digest), so the builder, staged checker, `test_reproducibility.py`, and the Rust owner test were not executed by me. Every claim rests on static reading of the `0bcc9c6..9ae09a1` diff, `sha256sum -c` on the five multi-file or sibling manifests, `jq` over the tracked report, and git queries.
- `accepted.json`'s inner `manifest.inputs_sha256` is a canonical-JSON digest (`json.dumps(sort_keys=True)`, `oracle/scb1/.../entity_read.py:1967`), which `jq` cannot reproduce byte-exactly; I verified instead that the delta re-minted `inputs_sha256`, `encoder_sha256`, and `refresh_head_revision` together with the `inputs.json`/`accepted.json` edits, and that the outer `SHA256SUMS` verifies. The inner digest is bound by `check_entity_read_vectors.py` in the `make conformance` recipe, which I did not run.
- Prior verdicts at `c5973c9` and `0bcc9c6` are not rewritten; this is a same-lane superseding verdict on the current tree.
- Evidence artifact: this transcript, to be filed by the integrator at `evidence/review/verdicts/reproducibility_and_independent_conformance/ariadne_contract_review-9ae09a1.md`.

## Verified facts

| Check | Result |
|---|---|
| Delta | exactly one commit `9ae09a1`; lane files: contract §3/§10, builder, tests, entity-read v2 corpus + manifest, report, oracle `entity_read.py`, oracle tests, `crates/sley-query/src/entity_read.rs` (+287, new corpus test), `sley-query/Cargo.toml` (`serde_json` dev-dep) |
| §10 replacement | claims `accepted_corpus_vectors_match_owner_and_encoder` reproduces response bytes, work charge, and object count from `include_str!` of the frozen v2 files: **verified** at `entity_read.rs:1881-1950` (asserts `body`, `work_units`, `returned_entities` per case, `cases.len() == 23`); precedent claim verified at `sley-scb1/tests/conformance.rs:45,82` and `sley-mutate/.../fixture_tests.rs:11-18`; direction claim ("independence forbids the oracle from depending on the Rust implementation, not the reverse") matches §4 and `WORK_PACKAGES.md:19` |
| Manifests | `sha256sum -c` passes: `entity-read/v2` (3 files), `smp1-json-bridge/v2`, `bootstrap-profile/v2`, `exec-package/v2`, `host-abi/v2` |
| Report | `tracked_corpus_directories: 29` equals the 29 `v<N>` directories on disk; `fixture_directories: 25`; entity-read record digests and byte sizes recompute exactly (4 files); `shape.accepted.json.cases == 23`, `rejected.json.cases == 91`, matching `jq` counts; `result: INDEPENDENT_CONFORMANCE_COMPLETE` |
| Builder | version enumeration `re.fullmatch(r"v\d+")`, non-matching directory is `FIXTURE_UNREADABLE`; `read_sums` raises `SUMS_MISMATCH` on conflicting duplicate names; `runner_label` keys on `^\s*(import|from)\s+sley2_scb1_oracle`; `fixture_shape` records mapping sizes, excluding key `manifest` |
| Tests | new `test_a_version_without_a_manifest_fails_closed`, `test_conflicting_duplicate_manifest_lines_fail_closed`; inventory +2 Python here, +2 Rust unit |
| Contract text | §3 schema gains `tracked_corpus_directories` and `mapping key: size` with the family/corpus count sentence; §10 typo fixed; header still "revision 5 (2026-09-11)" |
| Attestation | `reproducibility-report.json` still attests `84bfa9c`, `working_tree_clean: true` there; this delta itself adds `Cargo.lock` and `crates/sley-query` to the artifact-surface diff, so `history_problems` still appends `stale:…` and `make quick` stays red |
| Records | summary section unchanged this round: `contract_revision: 5`, `attested_commit` = report, coverage counts match; the three `*-0bcc9c6.md` transcripts for this section were filed in `9ae09a1` but the summary carries no `*_revision_2` fields for them, and the derived register lists only the `c5973c9` rounds (`superseded_rounds` empty for this section) |

## Disposition of prior findings

Closed by the delta with verification above: **P2 [contract]** §10 invented prohibition (replaced with a verified, correctly attributed statement and a real Rust consumer); **P3 [implementation]** missing manifest/duplicate tests; **P4 [contract]** §10 typo; **P4 [implementation]** `startswith("v")` enumeration; **P4 [implementation]** runner label substring.

Carried, unchanged: **P2 [record]** stale attestation (standing operator constraint; my prior adjudication holds: contract and checker reject it as specified, no contract/builder/checker change needed, cure is a clean clone of the scope commit and an operator sequencing decision); **P3 [implementation]** `generate_smp1_json_bridge_table.py:171` does not re-mint `smp1-json-bridge/v2/SHA256SUMS`; **P4 [implementation]** checker still hand-synchronises revision/attested_commit; **P4 [implementation]** `runner_label` uncoded `FileNotFoundError`.

## Findings on the current tree

**P2 [record]** `evidence/release/reproducibility-report.json`: attests `84bfa9c`; surface widened again by this delta. Non-blocking for this field per prior adjudication; blocks `make quick` and any release or `S20_730_COMPLETE` claim.

**P3 [record]** `machineresearch/sley-2.0/machine-summary.json` `reproducibility_and_independent_conformance`: the `0bcc9c6` round (three REVISE transcripts filed in this very commit) is not recorded, so `evidence/review/finding-register.json` derives only the `c5973c9` rounds for this section and the lineage skips a round. The other two sections reviewed on `0bcc9c6` did receive their summary entries in `9ae09a1`.

**P3 [record]** `docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md` header: the §3 schema changed (`tracked_corpus_directories`, mapping sizes) and the §10 rationale was rewritten, but the text is still "revision 5 (2026-09-11)" and the rev-5 narrative does not mention either; two distinct contract texts now share one revision identifier, and nothing in summary, ADR-0040, or the checker distinguishes them. Bump to revision 6 or fold the additions into the rev-5 header sentence.

**P3 [implementation]** `scripts/generate_smp1_json_bridge_table.py:171`: carried; next `--protocol-version 2` regeneration lands `CONFORMANCE_SUMS_MISMATCH`. Fail-closed.

**P4 [contract]** §3: the builder's two new fail-closed rules (a non-`v<N>` directory under a family is `FIXTURE_UNREADABLE`; conflicting duplicate manifest lines are `SUMS_MISMATCH`) and the `manifest` key exclusion from `shape` are implemented but not stated.

**P4 [implementation]** `bench/release/tests/test_reproducibility.py`: no assertion binds `tracked_corpus_directories` to the sum of `tracked_versions`, and no test exercises the non-version-directory rejection.

**P4 [implementation]** `scripts/check_reproducibility_and_independent_conformance.py`: carried hand-synchronised revision/attestation bindings (now the mechanism by which the P3 revision-identity gap goes undetected).

**P4 [implementation]** `runner_label`: carried uncoded `FileNotFoundError`.

**P4 [record]** `evidence/review/requests/REQ-02-s20-730.md`: request still describes the rev-4 ask with no round history for `c5973c9`, `0bcc9c6`, or `9ae09a1`; title carries an em-dash.

Handoffs (advisory, no files written): summary round record and revision identifier to the integrator; table-generator manifest and the two test bindings to merlin with vulcan for adequacy; attestation refresh decision to the operator via the integrator.

---

VERDICT: PASS
SECTION: reproducibility_and_independent_conformance
FIELD: ariadne_contract_review
SCOPE_SHA: 9ae09a142830a4857c553bad27433143999e6864
FINDINGS:
[P2] [record] evidence/release/reproducibility-report.json - attests 84bfa9c, artifact surface widened again by this delta (Cargo.lock, crates/sley-query); carried, non-blocking for this field under the prior adjudication and the standing operator constraint; make quick stays red and no release or S20_730_COMPLETE claim may cite this tree until the operator refreshes it from a clean clone
[P3] [record] machineresearch/sley-2.0/machine-summary.json reproducibility_and_independent_conformance - the 0bcc9c6 round (three REVISE transcripts filed in 9ae09a1) has no summary fields, so finding-register.json derives only the c5973c9 rounds for this section and the lineage skips a round
[P3] [record] docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md header - §3 schema (tracked_corpus_directories, mapping sizes) and §10 rationale changed under an unchanged "revision 5 (2026-09-11)" label; bump to revision 6 or extend the rev-5 narrative so summary/ADR/checker bind to one text
[P3] [implementation] scripts/generate_smp1_json_bridge_table.py:171 - carried; v2 table regeneration does not re-mint conformance/smp1-json-bridge/v2/SHA256SUMS; fail-closed via CONFORMANCE_SUMS_MISMATCH
[P4] [contract] §3 - non-v<N> directory rejection, conflicting-duplicate manifest rule, and the manifest-key exclusion from shape are implemented but unstated
[P4] [implementation] bench/release/tests/test_reproducibility.py - no test binds tracked_corpus_directories to sum(tracked_versions) or exercises the non-version-directory rejection
[P4] [implementation] scripts/check_reproducibility_and_independent_conformance.py - carried hand-synchronised contract revision and attested_commit; the revision-identity gap above passes it undetected
[P4] [implementation] scripts/build_independent_conformance_report.py runner_label - carried uncoded FileNotFoundError for a missing recipe script
[P4] [record] evidence/review/requests/REQ-02-s20-730.md - no round history for c5973c9/0bcc9c6/9ae09a1; title carries an em-dash
SUMMARY: The one item that held my prior verdict at REVISE is closed on the current tree: the §10 paragraph no longer invents an S20-130 prohibition, it now states the independence direction exactly as §4 does and cites a Rust owner test that I verified exists and reproduces every accepted entity-read v2 vector's response bytes, work charge, and object count from the frozen files through include_str!, the same pattern the scb1 and mutation corpora use. The remaining implementation deltas are each verified against the tree: ^v\d+$ fail-closed enumeration, conflicting-duplicate manifest rejection, import-line runner labelling, mapping sizes in shape (accepted cases 23 machine-recorded), tracked_corpus_directories 29 matching the 29 on-disk v<N> corpora, two new fail-closed tests, and a coherently re-minted entity-read v2 corpus whose outer manifest verifies. The contract is fit as a contract, so this field passes; the PASS does not make the section gate green: the reproducibility attestation is still stale under the standing operator constraint and stays a P2 record item that blocks make quick and any completion claim, and two new P3 record gaps (the unrecorded 0bcc9c6 round in the summary/register, and a contract text that changed under an unchanged revision label) plus the carried generator re-mint P3 should be closed by the integrator before freeze. Not re-executed in this session: builder --check, checker, Python and Rust tests (tool policy); all claims rest on static reading, sha256sum, jq, and git.
