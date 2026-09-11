# Ariadne contract review, REQ-07 item 1 — release_candidate_packaging (re-review after re-mint, SCOPE_SHA e464ed4)

Read-only; nothing written. HEAD verified equal to SCOPE_SHA at dispatch.

HEAD verified as `e464ed49cadda55ea6ef848d7943639c67dd31f0`. Review complete; no files written.

**Verification performed (read-only)**
- `git merge-base --is-ancestor 56bac4b HEAD`: ancestor-ok.
- `git diff --name-only 56bac4b HEAD`: only `decision-dossier.json`, `provenance.json`, `reproducibility-report.json`, `secret-scan.json`; zero `ARTIFACT_INPUT_PATHS` surface files changed, and `git status --porcelain` over the surface is empty. The freshness rule in `check_reproducibility_and_independent_conformance.py:142-187` is satisfied at HEAD by inspection.
- `report_digest` recomputed independently (`grep -v '"report_digest"' | sha256sum`) = `ec36b0ae…e033`, matches `reproducibility-report.json:40`.
- Attestation fields (`56bac4b`, `ac4ce59a…870b`, 2131797 bytes, manifest `86e09320…2624`, 14 members, `working_tree_clean: true`, `differing_members: []`) match the task statement and the 9c99e74 diff; toolchain equals `rust-toolchain.toml` pin 1.93.0.
- Prior verdict artifacts untouched; this is a new round.

**Assumptions and limits**
- Python execution, `rustc --version`, `cargo --version`, and reads/hashes under `~/cache/worktrees/sley2-repro-56bac4b` were denied by the session sandbox. I could not rerun either checker or hash the re-minted artifact; "repro checker PASS with zero problems" is taken from the 9c99e74 message and checked by hand against the checker's logic, not re-executed. The artifact digest is verified only as internally consistent across report, dossier, and commit message.

```
VERDICT: REVISE
SECTION: release_candidate_packaging
FIELD: ariadne_contract_review
SCOPE_SHA: e464ed49cadda55ea6ef848d7943639c67dd31f0
FINDINGS:
- [P1] [record] machineresearch/sley-2.0/machine-summary.json:2927-2943 - the section under review still names candidate e20b9adc / artifact ec20da7b / manifest 114a940a (two attestations ago); the tracked report attests 56bac4b / ac4ce59a / 86e09320. The closeout (S20_720_RELEASE_CANDIDATE_CLOSEOUT.md:128-131) made these fields the Nabu P0 fix for manifest self-description, and no checker guards them, so the register misstates the built candidate.
- [P1] [record] machineresearch/sley-2.0/machine-summary.json:3160,3163 - sibling S20-730 section carries attested_commit 84bfa9c9 and an open P1 "stale 84bfa9c9 attestation" that the 9c99e74 re-mint superseded; the register now contradicts evidence/release/reproducibility-report.json:7 in the exact field the freshness finding was tracked in.
- [P2] [record] evidence/release/provenance.json:24-26,107 - provenance and both SBOMs bind subject 8ee23a8d at 5b70052 (--allow-dirty, working_tree_clean false) while the attestation and dossier bind ac4ce59a at 56bac4b; the release chain names two candidates. Disclosed truthfully in e464ed4 and in the predicate, S20-710 checker fail-closed red per that message (not re-run here), cross-refers to REQ-07 item 2.
- [P2] [contract] docs/spec/RELEASE_CANDIDATE_PACKAGING_V1.md:150-160 - section 7 defines evidence.json as the record but it is gitignored and per-checkout; build_reproducibility_report.py:23 and build_release_provenance.py:21 each read "the local evidence.json", so a report minted in a linked worktree and provenance minted in the operator tree can lawfully describe different artifacts, which is exactly what happened. The contract needs a binding rule (tracked derived records must name one candidate identity) or a checker that enforces it.
- [P2] [implementation] scripts/build_release_candidate.py:539-540 - on PACKAGE_NOT_REPRODUCIBLE the evidence written is the failure stub; the comparison computed at :516-517 (archive_only flag, full differing member list) is discarded and only 10 members in a 500-char detail survive, contradicting section 6 (:142-146) "exact differing member paths and whether the difference is archive-only" and section 7 (:153).
- [P3] [implementation] scripts/build_release_candidate.py:396 - section 3 (:87) requires `version` to name CLI and protocol versions; the check asserts only `"protocol_version":1`.
- [P4] [contract] docs/spec/RELEASE_CANDIDATE_PACKAGING_V1.md:44-46 - section 1 step 4 lists three scan needle classes; section 5 (:130-133) and :477-479 scan five (adds /home-remapped and username). Step 4 should match section 5.
- [P4] [record] machineresearch/sley-2.0/machine-summary.json:2932 - offline_tests: 4; bench/release/tests/test_packaging.py has 8.
- [P4] [record] git worktree list - the re-mint lane ~/cache/worktrees/sley2-repro-56bac4b is still registered and holds the only copy of artifact ac4ce59a (dist/ is ignored); either retain it deliberately as evidence with a pointer or remove it per the disk hygiene rule.
SUMMARY: The re-mint itself is sound and honestly recorded: 56bac4b is an ancestor of HEAD with no artifact-surface file changed since, the report digest recomputes, the sole primary attestation names 56bac4b / ac4ce59a / 2131797 bytes with a clean tree and no differing members, the stale 84bfa9c9 attestation was replaced rather than normalized, and both records-only commit messages state the remaining red conditions plainly. The packaging contract and build script agree on the load-bearing mechanics (remap order, deterministic tar, self-describing manifest inside the digest, clean-tree requirement, forbidden-content needles, double build). What blocks PASS is the record layer: the register section this verdict is filed to still names the e20b9adc candidate, the sibling section still names 84bfa9c9 as stale, and provenance/SBOM bind a third dirty candidate at 5b70052, so a reader of the tracked records cannot identify one release candidate. Repair is records-only for the P1s (update candidate_* and attested_commit/p1_open from the report, or add a checker cross-check) plus a contract rule binding derived records to one candidate identity and the section 6 evidence-on-failure fix; none of it touches the attestation. Independent execution of the checkers and a hash of the re-minted artifact remain outstanding because this session could not run Python or read the re-mint worktree.
```
