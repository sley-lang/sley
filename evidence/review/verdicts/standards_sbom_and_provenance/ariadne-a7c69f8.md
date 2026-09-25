# Ariadne Council review — standards_sbom_and_provenance

Harness: claude-code
Observed model: claude-fable-5-1
Reviewed checkpoint: a7c69f8efa2267db944929288b2d508d40838fe4

Checks/evidence and limitations. Read ariadne/nabu/vulcan-97b9117 transcripts; `git diff 97b9117..a7c69f8` (25 records files; sbom/, provenance.json, scripts/, docs/, LICENSE, NOTICE, crates/, src/ all unchanged). machine-summary.json:4050 (open_risks S20-710), :4026 (evidence_gaps M6), :4027 (Independent review) carry the restated prose; decision-dossier.json:391, :420, :421 are byte-identical copies (whitespace-stripped line dedupe count 2 each), produced by the verbatim copy at scripts/build_decision_dossier.py:454-468. Tree-wide git grep at a7c69f8 for the four stale phrases (excluding preserved transcripts/rounds) returns nothing. rounds/qualification-97b9117.{json,md} exist; standards transcript sha256s in the round JSON match disk. Summary standards current_delta_review registers ariadne/nabu REVISE_0_P0_0_P1_0_P2_0_P3_1_P4 and vulcan PASS with 97b9117 transcript paths; 400895e values rotated to revision_6/5/7 historical keys. provenance.json blockers remain signing_key_and_transparency_log_unauthorized, final_argus_and_vulcan_dispositions, council_reviews; signed false; subject 7da77f1b…; commit 5313437a; SPDX namespace and CycloneDX hash bind 7da77f1b…. remaining_gates.root_license_text RESOLVED with dispositions and full re-anchor held separately; reproducibility-report MULTI_HOST_REPRODUCIBLE, attestations/5313437-secondary.json present; T54 history_anchor_commit 7804f66 unchanged (only candidate counts/manifest digest moved). Limitations: python and shell-expansion commands denied, so dossier/GA digests not recomputed; builders' --check, checker, and test suites not executed; evidence/validation/qualification-followup-2026-09-15.json records the 97b9117 run (base 400895e, exits 0, 139/90 tests) and no tracked artifact records the asserted post-a7c69f8 refresh/verify/supply-chain runs.
VERDICT: PASS
SECTION: standards_sbom_and_provenance
FIELD: current_delta_review.ariadne
SCOPE_SHA: a7c69f8efa2267db944929288b2d508d40838fe4
FINDINGS:
NONE
SUMMARY: The 97b9117 P4 is closed: the S20-710 open-risk and M6 evidence-gap entries now state approved root licensing, derived unsigned standards documents, and the exercised two-host lane, with final dispositions, the full candidate-history re-anchor decision, signing/transparency, and the release decision held; the dossier copies that prose verbatim and no stale licensing/second-host claim remains on the scoped risk/gap surface. No SBOM, provenance, or code input changed.
