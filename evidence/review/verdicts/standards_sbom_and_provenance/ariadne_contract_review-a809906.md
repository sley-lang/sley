# Ariadne contract review — standards_sbom_and_provenance / ariadne_contract_review (revision 5, SCOPE_SHA a809906)

Role: Ariadne, contract-conformance lane. Full review of
`docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md` revision 5 against the four scripts,
the tracked documents, and the release test module at this HEAD, including the
f7df74f changes (revision pointer, bound-inputs sentence, one-cause reporting,
admission test rename) and the a809906 fold repair (`builder_refusal_label`,
`fold_closure_refusals`).

## Scope verification

`git rev-parse HEAD` = `a809906f78f1bfdb9cde8da4c108c4d692dad297`, equal to the
SCOPE_SHA. `git status --short` was empty at dispatch. Read-only: no tracked file
was edited, staged, or generated; no builder ran in write mode; the only file
written is this transcript. The in-process re-derivation below patched module
attributes in a throwaway interpreter and wrote nothing.

## Inputs read in full

- `/tmp/claude-sley2/review-brief.md`, `/tmp/claude-sley2/round-a809906-sections.md`.
- `docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md` (345 lines, revision 5).
- `scripts/build_standards_sbom.py` (699), `scripts/build_release_provenance.py` (540),
  `scripts/records_closure.py` (139), `scripts/check_standards_sbom_and_provenance.py` (447).
- `bench/release/tests/test_standards_sbom.py` (825; 56 tests).
- `git show f7df74f` and `git show a809906` restricted to the section files (diffs read in full).
- Prior Ariadne transcripts in this directory: `-e464ed4`, `-abf4ff0`, `-3320ca9`
  (the "revision-3 PASS with 4 P3" form the register carries as
  `ariadne_contract_review` / `ariadne_contract_review_revision_3`), and `-a4b6029`
  (harness round, PASS-3: one P2, one P3, one P4). Also
  `vulcan_surface_review-70283ce.md` (the P3 that a809906 repairs) and
  `nabu_architecture_review-70283ce.md`.
- `machineresearch/sley-2.0/machine-summary.json` sections
  `standards_sbom_and_provenance` and `s20_710_pre_release_audit`;
  `evidence/review/finding-register.json` rows for this section.
- `evidence/release/sbom/cyclonedx-1.6.json`, `evidence/release/sbom/spdx-2.3.json`,
  `evidence/release/provenance.json`, `evidence/release/reproducibility-report.json`,
  `evidence/security/T52/pre-release-inventory.json` (top-level keys), and the untracked
  `evidence/runtime/s20-720-release-candidate/evidence.json`.
- `docs/audits/S20_710_STANDARDS_SBOM_CLOSEOUT.md` header; `docs/spec/ERROR_CODES_V1.md:540-542`;
  `docs/WORK_PACKAGES.md:64`.

## Tool results (exact)

All commands ran with `SLEY2_MASTER_GOAL=/home/greyforge/machineresearch/Sley2.0mastergoal.md`.

1. `python3 scripts/check_standards_sbom_and_provenance.py` — exit 1. `problems` is exactly
   `["closure:ineligible"]`. `records_closure` = `{"advanced": true,
   "attested_source_commit": "7a94a4a31272a6dc7588aff902dcde81f7d10a4e",
   "records_closure_head": "a809906f78f1bfdb9cde8da4c108c4d692dad297",
   "reason": "records-closure-ineligible: RESUME.md, bench/release/tests/test_standards_sbom.py,
   crates/sley-query/src/root_query.rs, crates/sley-repo/tests/zjx_readiness_witness.rs,
   docs/WORK_PACKAGES.md, docs/audits/S20_LOCAL_COMPLETION_FRONTIER.md,
   docs/audits/S20_ZJX_TRANSPORT_READINESS_AMENDMENT.md,
   docs/audits/S20_ZJX_TRANSPORT_READINESS_CLOSEOUT.md, docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md,
   docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md, docs/status/HANDOFF-2026-09-15-QUALIFICATION.md,
   scripts/build_decision_dossier.py, scripts/build_ga_acceptance_report.py,
   scripts/check_standards_sbom_and_provenance.py"}`; `result` `FAIL`;
   `status` `S20_710_FULL_SBOM_AND_PROVENANCE_IMPLEMENTED_REVIEW_PENDING`;
   `implementation_complete` false; `s20_710_audit_complete` false.
2. `python3 scripts/build_standards_sbom.py --check` — exit 1, one JSON object:
   `code 74001`, `name INVENTORY_INVALID`, `detail "candidate commit 7a94a4a… is not HEAD;
   records-closure-ineligible: <the same 14 paths>; rebuild the candidate on this tree before
   deriving SBOMs"`.
3. `python3 scripts/build_release_provenance.py --check` — exit 1, one JSON object:
   `code 74005`, `name EVIDENCE_INVALID`, same `records-closure-ineligible` reason, ending
   "rebuild the candidate on this tree before deriving provenance".
4. `records_closure.closure_status("7a94a4a…")` called directly: `is_closure False`,
   53 changed paths, 14 ineligible (listed above), `bound_changed []`. The other 39 paths are
   all under `evidence/` or `machineresearch/`.
5. `python3 -m unittest discover -s bench/release/tests -t .` — `Ran 110 tests in 0.318s`, `OK`,
   exit 0. `python3 -m unittest bench.release.tests.test_standards_sbom -v` — 56 tests, all ok
   (52 at 70283ce plus the four `CheckerFoldTests`).
6. In-process re-derivation (read-only; `git_head` pinned to the attested commit exactly as the
   suite's `patch_candidate` does, otherwise real inputs): `build_documents()` and
   `build_file()` produce bytes equal to the tracked `cyclonedx-1.6.json`, `spdx-2.3.json`, and
   `provenance.json` (`True`, `True`, `True`); counts `components 51`, `relationships 138`.
7. Digest cross-checks against the tracked provenance: `Cargo.lock`, `oracle/scb1/uv.lock`,
   `evidence/security/T52/pre-release-inventory.json`, both SBOM documents,
   `evidence/release/reproducibility-report.json`, and
   `evidence/conformance/independent-conformance-report.json` all match the live SHA-256;
   `statement_digest` re-verified; CycloneDX `serialNumber` re-derived
   (`urn:uuid:86d08e5c-68c1-87e4-8d18-cc565012f403`); SPDX namespace re-derived
   (`urn:sley2:spdx:285c97b1…:6d970bf4…`); every emitted license expression parses; components
   sorted by purl; 52 `dependencies` entries (root + 51, the 51 sorted); 52 SPDX packages with
   unique ids; 30 components with `hashes`, 2 with `sley2:locked-artifact-digests`, 19 workspace
   components without `externalReferences`; `sley2:license-disposition-blocked` = `0`.
8. `grep -c a809906` in the three documents: 0, 0, 0.
9. Candidate evidence (untracked): commit `7a94a4a…`, artifact `6d970bf4…`, manifest
   `aea8182d…`, size 2190465, `result PASS`, `working_tree_clean true`, invocation
   `build_release_candidate.py --timeout-seconds=900 --require-clean --no-keep`,
   `reproducibility.result REPRODUCIBLE`. Reproducibility report: two clean `REPRODUCIBLE`
   attestations (primary, secondary) of the same 4-tuple, `MULTI_HOST_REPRODUCIBLE`.
10. `git diff --stat 7a94a4a HEAD -- evidence/release/ evidence/security/T52/
    evidence/conformance/ Cargo.lock oracle/scb1/uv.lock`: only `evidence/release/*` changed
    (records-only re-derivation at 6a2eef7); inventory, lockfiles, conformance report unchanged.
11. `git show a4b6029:scripts/build_standards_sbom.py` lines 38-39, 417: the root license was
    `LicenseRef-Proprietary` before 7804f66; at HEAD it is `Apache-2.0` at lines 38, 312, 362, 419.
12. `python3 -c "import spdx_tools"` — module not installed; no SPDX validator is available in
    this environment, so finding 1 below rests on the SPDX 2.3 specification text.
13. Machine summary: `contract_revision: 5` with the note crediting the a4b6029 P3;
    no `records_closure`/`closure_head` key exists in the standards section at HEAD or at
    6a2eef7 (`git show 6a2eef7:machineresearch/sley-2.0/machine-summary.json | grep -i closure`
    returns only unrelated matches).

## Independently re-derived claims

**Section 1 (inputs).** Both loaders name the section 1 codes: missing inventory 74000,
malformed 74001 (`build_standards_sbom.py:81-100`); missing candidate evidence 74000 on the
SBOM side and 74004 on the provenance side, malformed 74001/74005; a non-PASS or
non-REPRODUCIBLE record 74005 at `build_release_provenance.py:117-123`; missing or
attestation-less report 74004/74005 (`:183-195`). Component completeness (purl, name, version,
ecosystem, license expression, grammar) is 74002 at `:200-225`. Matches.

**Section 2 (CycloneDX).** `bomFormat`/`specVersion`/`version` (`:301-304`); derived serial with
nibbles 12 and 16 set to `8` (`:253-262`, re-verified on the tracked bytes); root component with
name, `2.0.0-alpha.0`, SHA-256 (`:306-313`); tool `sley2-standards-sbom` version `1`; the six
`metadata.properties` named by the contract (`:314-327`, verified live); components ascending
by purl with `bom-ref` = purl, `library`, single `expression`, `hashes` only under the
single-digest rule and the count property otherwise (`:265-296`); `/` normalization and
grammar (`:122-197`, pinned by `LicenseExpressionTests`); no timestamp/host/user/path
(checker markers and the suite's marker test). Matches, with the shape omissions in finding 2.

**Section 3 (SPDX).** `SPDX-2.3`, `CC0-1.0`, `SPDXRef-DOCUMENT`; namespace
`urn:sley2:spdx:<inventory digest>:<artifact digest>` (`:407`, re-verified); fixed instant and
`Tool: sley2-standards-sbom-1` (`:408-416`); one package per inventory package plus the root
with `SPDXRef-<ecosystem>-<name>-<version>` sanitized by `SPDX_ID` (`:347-348`),
`NOASSERTION` conclusions and copyright, checksums under the single-digest rule, purl
`externalRefs`; `DESCRIBES` plus ascending `DEPENDS_ON` (138, equals the summary). Matches,
except the `hasExtractedLicensingInfos` construct in finding 1.

**Section 4 (provenance).** File wrapper contract/statement/attestation/digest (`:436-449`);
statement type, single subject, SLSA v1 predicate, build type; `externalParameters` commit,
artifact name, `make_target` derived from the recorded invocation with the single Makefile
rendering (`:273-279`), verbatim invocation, refusal 74005 without one (`:261-266`),
`working_tree_clean`; `internalParameters` cargo, rustc, `release`, `locked`, remaps, size,
member count; six `resolvedDependencies` with the git commit as `sha1`; builder id
`urn:sley2:builder:local-primary`; `invocationId` = manifest digest; two byproducts; no
`startedOn`/`finishedOn`. Subject authority: `build_statement()` refuses a non-HEAD candidate
except a provable closure (`:213-221`), refuses a candidate no clean `REPRODUCIBLE`
attestation names on the 4-tuple (`:227-238`, 74006), refuses a disagreeing SBOM root
(`:239-245`, 74006), refuses a dirty candidate (`:246-257`, 74005). Matches.

**Section 5 (determinism and check semantics).** Canonical JSON (`canonical()` in both);
`--check` drift codes 74003/74007 (`build_standards_sbom.py:648-664`,
`build_release_provenance.py:492-507`); the mismatch state with
`MISMATCH_TRACKED_VALIDATED` / `MISMATCH_TRACKED_INVALID` and the reconciling command
(`:618-645`, `:457-484`); both `validate_tracked()` functions check shape, determinism pins,
and the attestation binding of the namespace (SBOM) and the 4-tuple subject binding
(provenance) (`:552-590`, `:371-433`). "Names" as the 4-tuple on both builders with the
stated asymmetry (SBOM `PASS` at the gate `:486-491`; provenance `PASS` and `REPRODUCIBLE` at
load `:117-123`) matches `:212-216` exactly — this closes my 3320ca9 P3 on the undefined
"names". Write mode refuses the skew with 74001/74005/74006 as written.

**Records-closure model (section 5 subsection) versus the live ineligible HEAD.** The contract
says an attestation-bound change refuses closed with `SBOM_INVENTORY_INVALID` /
`PROVENANCE_EVIDENCE_INVALID` carrying a `records-closure-…` reason, write mode still names
`make release-candidate-smoke`, the documents stay bound to the attested candidate, and the
closure HEAD is recorded separately, never inside the documents. Observed: 74001 and 74005
with `records-closure-ineligible:` naming the 14 attestation-bound paths (five under
`docs/`, three under `scripts/`, two under `crates/`, one test module, `RESUME.md`, and a
status note, none under the eligible prefixes); each detail names the rebuild; the checker's
`records_closure` block carries `attested_source_commit`, `records_closure_head`, and the
reason; the three documents contain no occurrence of `a809906` and are byte-identical to a
derivation pinned at `7a94a4a` (tool result 6), with every bound digest matching (result 7).
`bound_changed` is empty: the T52 inventory is unchanged since the candidate. The reporting
of this state therefore matches the contract clause for clause. The checker reports the one
underlying cause once (`closure:ineligible`), which is a checker-label convention the
contract does not prescribe but does not contradict (section 7 asks the checker to verify
"both builders report no drift"; a closure refusal is not drift, and the a809906 labeling
now says so explicitly).

**f7df74f, item by item.** (i) `contract_revision` 4 → 5 in the summary, with a note naming
my a4b6029 P3: closed. (ii) Bound-inputs sentence (`:252-260`): now states that the single
bound input is the T52 inventory, that the SBOM pair, the statement, and the reproducibility
report are second-layer (byte-identical re-derivation plus the 4-tuple gate), and that a
reader needs both layers; `records_closure.py:34-48` is unchanged and encodes exactly that
(`BOUND_PATHS` = inventory only; eligible prefixes `evidence/`, `machineresearch/`). This
closes my a4b6029 P4. (iii) One-cause reporting: the f7df74f form dropped all builder
failures when the closure was ineligible, which Vulcan correctly found over-broad at 70283ce.
(iv) `test_builders_admit_an_eligible_closure_without_remint` renamed to
`…_keeping_the_candidate_binding` with a scoping comment; the test compares the candidate-bound
properties, subject, and SPDX version and leaves byte identity to the drift path, which is
what it actually asserts (`:702-727`).

**a809906, the fold repair.** `builder_refusal_label` (`:130-152`) parses the builder's own
stdout record: a `detail` containing `records-closure-` becomes `<label>:closure-refusal`; a
`state` of `MISMATCH_TRACKED_INVALID` becomes `<label>:tracked-invalid`; anything else
`<label>:drift`. `fold_closure_refusals` (`:155-165`) drops only `:closure-refusal` entries
when the checker's own closure computation is ineligible, and relabels `:closure-refusal` to
`:drift` otherwise (a builder that refuses on closure while the checker's independent
computation admits it can only happen if HEAD moved between the two subprocess calls, and
then it surfaces rather than folds — fail-closed). The builders raise with a
`records-closure-` reason only from the `not status.is_closure` branch
(`build_standards_sbom.py:477-485`, `build_release_provenance.py:213-221`), and the
`MISMATCH_TRACKED_INVALID` branch runs before and independently of that gate
(`:618-632`, `:457-471`), so the three labels partition the paths Vulcan traced. The four
`CheckerFoldTests` pin the label derivation (including empty and non-JSON stdout) and both
fold directions. Live run: exactly one `closure:ineligible`, no builder label, which is the
correct outcome because both builders' details carry the closure reason and the tracked
documents are valid (result 6). The Vulcan P3 at 70283ce is repaired as specified.

**Section 6 and 7.** Codes 74000–74007 reserved in `ERROR_CODES_V1.md:540`; each builder
prints one JSON object with `code`/`name` and exits 1. The checker verifies contract tags,
versions, forbidden markers, the namespace binding (`spdx:namespace-unbound` /
`spdx:namespace-not-candidate-bound`), the `(subject, commit)` attestation pin
(`provenance:attestation-unbound` / `provenance:subject-attestation-mismatch`),
`provenance:dirty-candidate`, the unit suite, and the `v2` / `release-check` gates,
in every implementation status. COMPLETE additionally binds the three bare `PASS` tokens.
Matches section 7.

## Prior findings: status at this HEAD

Register/summary form `PASS_0_P0_0_P1_0_P2_4_P3` is the 3320ca9 transcript (four P3, three
P4). The a4b6029 harness transcript is PASS-3 (one P2, one P3, one P4). Both sets:

3320ca9 P3 items:
- P3 [record] missing revision fields / a12fcfb transcript: the register and summary now carry
  `ariadne_contract_review_revision_1..3` and the nabu/vulcan revision rows (rows 331-344).
  Closed as a record matter (the a12fcfb transcript was never filed; the register no longer
  claims it).
- P3 [implementation] SBOM write-gate branches unexercised: closed by
  `test_documents_refuse_a_non_pass_record`, `…_a_manifest_size_mismatch`,
  `…_a_manifest_digest_mismatch` (`test_standards_sbom.py:92-120`).
- P3 [implementation] one-directional positive control: closed by `is_admittable()`
  extracted (`build_release_provenance.py:166-177`), `test_admission_predicate_directly`
  (`:598-613`) with dirty, non-REPRODUCIBLE, malformed and non-dict denials, and
  `test_unattested_pair_is_not_admitted` (`:615-622`).
- P3 [contract] "names" undefined and provenance binding the 2-tuple: closed by
  spec `:212-216` and `build_release_provenance.py:227-233`, `:406-432`.
3320ca9 P4 items: singular Makefile rendering (`:156-157`) closed; SBOM mismatch-state
namespace binding without a clean/REPRODUCIBLE filter (`build_standards_sbom.py:579-589`,
checker `:295-299`) unchanged and still moot because `build_reproducibility_report.py`
records only clean REPRODUCIBLE attestations, consistent with section 7 as written, not
re-listed; closeout document still reads revision 3 (`S20_710_STANDARDS_SBOM_CLOSEOUT.md:5,58,63`)
as the historical closeout of the P0 round, not re-listed.

a4b6029 items:
- P2 self-invalidating live-tree closure test: closed at db1bc62 and confirmed here by
  `test_builders_admit_exactly_the_live_closure_verdict` (`:638-673`), which follows the live
  verdict and refuses on this ineligible HEAD (56/56 green).
- P3 `contract_revision` pointer lag: closed at f7df74f (`contract_revision: 5`).
- P4 bound-inputs sentence: closed at f7df74f (`:252-260`).

## Findings

1. [P3] [contract/standard] `docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md:125-127`,
   `scripts/build_standards_sbom.py:417-423`, `evidence/release/sbom/spdx-2.3.json`
   (`hasExtractedLicensingInfos[0].licenseId` = `Apache-2.0`). SPDX 2.3 clause 10.1 restricts
   extracted licensing information to licenses not on the SPDX License List and requires the
   identifier form `LicenseRef-<idstring>`; `Apache-2.0` is a listed identifier, so the
   document defines a construct the named standard forbids, and the section 3 bullet
   prescribes it ("defines `Apache-2.0` with extracted text … because the declared workspace
   expression is a standard SPDX license identifier" inverts the clause's purpose). This was
   valid at revision 4 (`LicenseRef-Proprietary`, `git show a4b6029:scripts/build_standards_sbom.py:38,417`)
   and became invalid when 7804f66 relabeled the root license without dropping the entry.
   No gate reads the field, nothing is published, and the root package's
   `licenseDeclared: Apache-2.0` already carries the fact, so this is moderate rather than
   release-critical; the fix is to drop the entry and the bullet (the operator-approval
   sentence can live in `creationInfo.comment` or the root package `comment`), then re-derive
   at the queued re-mint. I could not run an SPDX validator here (no `spdx_tools`); the claim
   rests on the specification text.
2. [P4] [contract] `docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md:50,77-81,97-98`. Section 2
   freezes the component shape but omits the emitted per-component `properties`
   (`sley2:ecosystem`, `sley2:license-disposition`, `sley2:locked-source`;
   `build_standards_sbom.py:276-280`), the `$schema` key (`:301`), the root component
   `licenses` (`:312`), that `externalReferences` is emitted only for non-workspace packages
   (`:291-294`), and the leading root `dependencies` entry (`pkg:generic/sley@2.0.0-alpha.0`
   depending on the 19 workspace purls, `:336`), so the live document has 52 entries where
   the text says one per component (51). Section 1 line 50 says the lock digests are "read
   from the inventory" while `build_release_provenance.py:310-311` hashes the live lockfiles
   (the inventory's `cargo_lock_sha256`/`uv_lock_sha256` agree today and lockfiles are
   attestation-bound, so nothing can diverge on a closure HEAD). Editorial; the emitted
   extras are deterministic and harmless.
3. [P4] [contract/record] `docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md:262-264` says the
   closure HEAD is "recorded separately (checker output and machine summary, never inside the
   documents)". The checker's `records_closure` block does this; no machine-summary field
   exists for it, at this HEAD or at the one eligible-closure commit 6a2eef7. Either add the
   summary record on an admitted closure or narrow the sentence to the checker output.

Non-actionable observations (prose only): `builder_refusal_label`'s fallback names every
remaining builder failure `:drift` (including 74000/74002/74004/74006 input and subject
refusals) and its docstring says three failure branches exist; fail-closed and the builder's
own JSON carries the true code, so a label-precision matter for the Vulcan delta lane, not a
contract gap. The base field `ariadne_contract_review` still carries the legacy
`PASS_0_P0_0_P1_0_P2_4_P3` form; this transcript supersedes it and the COMPLETE gate binds
only the bare token, so nothing passes open.

## Summary

Revision 5 is implemented faithfully across the four scripts: every section 1–7 clause I
could execute or re-derive agrees with the code, the tracked documents are byte-identical to
a derivation pinned at the attested candidate 7a94a4a with all seven bound digests matching,
and the checker's reporting of this closure-ineligible HEAD matches the records-closure
section clause for clause (74001/74005 with a `records-closure-ineligible` reason naming 14
attestation-bound paths, one folded cause, the closure HEAD recorded outside the documents,
no HEAD reference inside them). The f7df74f items close my a4b6029 P3 and P4 and the a809906
fold repair closes the Vulcan P3 as specified, with the builder-attributed labeling pinned by
four tests; all four 3320ca9 P3 items are closed. What remains is one moderate standards
defect the license relabel introduced (an `Apache-2.0` extracted-licensing entry that SPDX
2.3 forbids, prescribed by section 3) and two editorial shape/record gaps; none affects a
gate or the queued re-mint, but the extracted-licensing entry should be dropped before the
documents are re-derived. 110/110 release tests and 56/56 standards tests green.

```
VERDICT: REVISE_0_P0_0_P1_0_P2_1_P3_2_P4
SECTION: standards_sbom_and_provenance
FIELD: ariadne_contract_review
SCOPE_SHA: a809906f78f1bfdb9cde8da4c108c4d692dad297
FINDINGS:
[P3] [contract/standard] docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md:125-127 - section 3 prescribes, and build_standards_sbom.py:417-423 emits, a `hasExtractedLicensingInfos` entry whose `licenseId` is the listed identifier `Apache-2.0`; SPDX 2.3 clause 10.1 reserves extracted licensing info for unlisted licenses and requires the `LicenseRef-` form, so the tracked spdx-2.3.json is not a conforming SPDX 2.3 document (introduced at 7804f66 when `LicenseRef-Proprietary` was relabeled). Drop the entry and the bullet; re-derive at the re-mint.
[P4] [contract] docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md:50,77-81,97-98 - the section 2 shape freeze omits the emitted per-component `properties`, `$schema`, root `licenses`, the non-workspace-only `externalReferences` condition, and the leading root `dependencies` entry (52 entries, not one per component); section 1 says lock digests are read from the inventory while build_release_provenance.py:310-311 hashes the lockfiles.
[P4] [contract/record] docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md:262-264 - the closure HEAD is said to be recorded in "checker output and machine summary"; only the checker's `records_closure` block records it, no summary field exists at HEAD or at the eligible-closure commit 6a2eef7. Add the summary record or narrow the sentence.
SUMMARY: Revision 5 is implemented faithfully: every executable clause of sections 1 through 7 agrees with the four scripts, the tracked documents are byte-identical to a derivation pinned at the attested candidate 7a94a4a with all bound digests matching, and the checker reports this closure-ineligible HEAD exactly as the records-closure section requires (74001/74005 with a records-closure-ineligible reason naming 14 attestation-bound paths, one folded cause `closure:ineligible`, the closure HEAD recorded outside the documents and absent from them). The f7df74f revision pointer and bound-inputs rewrite close my a4b6029 P3 and P4, the a809906 builder-attributed fold closes the Vulcan P3 with four pinned tests, and all four 3320ca9 P3 items are closed. Remaining: one moderate SPDX 2.3 conformance defect introduced by the Apache-2.0 relabel and two editorial gaps; no gate is affected. 110/110 release tests and 56/56 standards tests pass.
```
