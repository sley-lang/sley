# Nabu architecture review — standards_sbom_and_provenance records-closure amendment (rev 5, SCOPE_SHA a4b6029)

Read-only review; HEAD verified equal to SCOPE_SHA at dispatch (`git rev-parse HEAD` =
`a4b60294e4dd75133fa08f92bf16022fb05bb807`). No files edited except this verdict.
Scope: the rev 5 closure architecture — `scripts/records_closure.py` (full),
`git show a4b6029 -- scripts/build_standards_sbom.py scripts/build_release_provenance.py
scripts/check_standards_sbom_and_provenance.py`, the 7 new tests, live execution.

**Architecture.** One narrow predicate module (`records_closure.py`, 139 lines, no I/O
besides git, no writes) stands between the candidate-commit check and derivation in
both builders. Admission is conjunctive (`:62-70`): verifiable diff, strictly advanced,
zero ineligible paths, zero bound-path changes. Refusal reuses the existing fail-closed
codes 74001/74005 with a `records-closure-…` reason appended to the detail, so no new
code path, no new exit shape, no new coupling: the checker, the suite, and downstream
consumers see the same codes they already handle. The eligible set is minimal and
syntactic (`ELIGIBLE_PREFIXES = ("evidence/", "machineresearch/")`, `:35`); the bound
set is a single content root, the T52 inventory (`BOUND_PATHS`, `:46-48`); everything
else — crates, scripts, specs/contracts, lockfiles, Makefile, conformance, bench — is
attestation-bound by position and forces refusal. There is no general skew tolerance:
no timestamp/hash windowing, no "close enough" commit distance, no allowlist override.

**Misbinding analysis (attack sketches, all closed).**
1. Changed crate or build input under a records-only claim: any such tracked path is
outside the eligible prefixes → `records-closure-ineligible` → 74001/74005. Live proof:
18 ineligible paths at HEAD, both builders refuse, suite pins refusal with code +
reason-substring assertions (`test_builders_refuse_an_ineligible_closure`).
2. Changed T52 inventory (the SPDX namespace input): `BOUND_PATHS` hit →
`records-closure-bound-changed` → refuse, pinned at unit and builder level
(`test_bound_input_change_refuses`, `test_builders_refuse_a_bound_artifact_change`).
The inventory transitively covers both lockfiles via recorded digests, and lockfiles
are themselves positionally ineligible, so the binding is double-covered.
3. Hand-minted documents committed under `evidence/` (positionally eligible, not
bound): admission is only half the guarantee; derivation is a pure function of
attestation-bound inputs and `--check` demands byte-identity (74003/74007) plus the
SPDX-namespace and provenance-subject attestation bindings. `test_builders_admit_an_eligible_closure_without_remint`
asserts the derived properties/subject equal re-derivation (no re-mint, no drift
tolerance). A forged document diverges from re-derivation and fails.
4. Tampered tracked attestation (`evidence/release/reproducibility-report.json`,
positionally eligible, not in `BOUND_PATHS`): write-mode derivation reads the live
untracked candidate evidence and demands the full 4-tuple match against a clean
REPRODUCIBLE attestation (`build_standards_sbom.py:492-508`,
`build_release_provenance.py:224-239`); a tracked-only forgery cannot satisfy a
candidate it does not name, and the subject digest must equal an attested digest
(`PROVENANCE_SUBJECT_MISMATCH` otherwise). The minimal `BOUND_PATHS` is therefore
sound, with the 4-tuple cross-check doing the second half of the work.
5. Unverifiable diff (unknown/shallow/unavailable commit, git failure): fail-closed to
non-closure with a `records-closure-unverifiable` reason, pinned by
`test_unverifiable_git_refuses_closed` and by the amended
`test_a_candidate_from_another_commit_refuses` (commit `0*40`).
6. Untracked smuggling: `git diff --name-only` covers tracked paths only, but
derivation inputs are the tracked inventory, the tracked attestation, and the
untracked candidate evidence, the last constrained by the 4-tuple gate; an untracked
crate file enters none of these, so admitted derivations stay byte-identical.

**Narrowness confirmed.** The bypass touches exactly one condition (HEAD equality) in
exactly two functions, and only on a proven predicate. Closure HEAD is recorded in
checker output (`records_closure` block with `attested_source_commit`,
`records_closure_head`, `reason`) and never inside the documents (0 occurrences of
`a4b6029` in all three; all bind `36f0e1f`). No artifact is rebuilt for a permitted
advancement (admit test derives without remint).

**Honest live-state note.** At this HEAD the architecture refuses by design (non-closure:
the amendment's own scripts/spec/bench/crates changes). Suite 105: 104 pass; the single
failure is the live-tree integration test asserting a state this commit falsifies —
test hygiene, not architectural unsoundness. Checker red (`sbom:drift`,
`provenance:drift`, `closure:ineligible`, `release-tests:fail`) is the correct
fail-closed posture, and the summary keeps both standards flags false.

**FINDINGS:**

- [P3] `scripts/records_closure.py:46-48` - `BOUND_PATHS` contains only the T52 inventory; the reproducibility report (the subject-authority input) relies on the 4-tuple cross-check rather than positional binding. Sound as analyzed above, but adding `evidence/release/reproducibility-report.json` to `BOUND_PATHS` would make the narrowness structural instead of relying on the second-layer gate, at no cost to legitimate closures (records-only waves never re-mint it). Defense-in-depth suggestion, not a defect.
- [P3] `scripts/check_standards_sbom_and_provenance.py:353-373` - the closure accounting shell-outs to git and re-imports `records_closure` inline rather than sharing the builders' already-computed verdict; triplicated invocation (two builders + checker) could disagree only if HEAD moves mid-run, in which case all three fail closed anyway. Note only; no change requested.
- [P4] `scripts/records_closure.py:127-132` - prefix matching via `startswith` on `evidence/` / `machineresearch/` is correct for git's forward-slash output on all platforms, and `diff --name-only` omits mode-only noise inconsistently across git versions; neither affects the fail-closed direction (extra names only add refusal surface). Informational.

**SUMMARY:** The closure predicate is minimal, conjunctive, fail-closed on every
unverifiable input, and bypasses nothing but HEAD equality under a proof the tests pin
on both builders with exact codes and reasons. No architectural path lets a changed
crate, a changed bound input, or a hand-minted document through. Narrowness holds.

VERDICT: PASS-3 / SECTION: standards_sbom_and_provenance / FIELD: nabu_architecture_review / SCOPE_SHA: a4b60294e4dd75133fa08f92bf16022fb05bb807 / FINDINGS: 3
