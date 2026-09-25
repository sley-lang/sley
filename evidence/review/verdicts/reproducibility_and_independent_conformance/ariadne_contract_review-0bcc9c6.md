**Baseline verified:** `git rev-parse HEAD` = `0bcc9c646f110e2e433e8fe3e9b913d49d84c531`. No files written. Working tree carries the two untracked RW-080 slices, three untracked S20-250 verdict transcripts, and a modified `machine-summary.json` whose diff is confined to the S20-250 section (lines 318-326); that modification is outside this section and is not adjudicated here.

## Method and assumptions

- Read-only. Tool policy again refused `python3` execution, so `build_independent_conformance_report.py --check`, the staged checker, and `test_reproducibility.py` were not re-run. Every claim rests on static reading of the `c5973c9..0bcc9c6` diff, `sha256sum -c` on every manifest, and git queries. Where I say a checker "fires", that is its code path applied to git facts I verified.
- The prior verdict at `c5973c9` is not rewritten; this is a same-lane superseding verdict on the current tree.
- Evidence artifact: this transcript, to be filed by the integrator at `evidence/review/verdicts/reproducibility_and_independent_conformance/ariadne_contract_review-0bcc9c6.md` (I may not write it).

## Verified facts

| Check | Result |
|---|---|
| Delta | exactly one commit `0bcc9c6`; lane files: contract, ADR-0040, builder, tests, oracle `refresh`, `smp1-json-bridge/v2/SHA256SUMS`, report, summary |
| Manifests | `sha256sum -c` passes in all five multi-file or sibling corpora: `bootstrap-profile/v2`, `exec-package/v2`, `host-abi/v2`, `smp1-json-bridge/v2`, `entity-read/v2` (3 JSON incl. `inputs.json`) |
| Report | every family carries `tracked_versions` and `siblings`; the four v2 siblings appear with `kind: tracked_sibling`, `pinned_version: v1`, digests; all four sibling manifest digests I recomputed are present in the report; `result: INDEPENDENT_CONFORMANCE_COMPLETE` |
| Builder | `validate_corpus_versions()` runs first in `build_report`; sibling records go through the same `version_record` (manifest required, `SUMS_MISMATCH` on drift); `sums_file`/`sums_consistent` are now constant `true`, dead guard removed; `runner_label` reads the script for the oracle import, so entity-read reports `oracle/scb1` |
| Tests | new: missing pinned dir, pin outside COVERAGE, sibling enumeration without depth; `CORPUS_VERSION == {"entity-read": "v2"}` still asserted |
| Oracle `refresh` | stages `inputs.json` and sums every `*.json` in the output dir; test updated to expect three-line manifest |
| Contract rev 5 | §3 multi-version rule, fail-closed manifest rule, pin-outside-coverage rule, schema with `tracked_versions`/`siblings`; §10 records the v2 pin rationale, emission direction, and why siblings carry no depth |
| ADR-0040 / summary | revision 5; summary `attested_commit` = `84bfa9c9…` matching the report |
| Attestation | `reproducibility-report.json` still attests `84bfa9c`; artifact surface (`crates`, `Cargo.lock`, `T52/pre-release-inventory.json`, conformance subset) has dozens of changed files to HEAD; local S20-720 record is at `5b70052` with `working_tree_clean: false`; `history_problems` appends `stale:…`, `make quick` (Makefile:79) is red at the scope SHA |

## Disposition of prior findings

Closed by the delta with verification above: P1 [contract] sibling enumeration; P2 [contract] dead sums guard; P2 [record] summary `attested_commit`; P3 [implementation] refresh manifest; P3 [contract] §10 v2 rationale; P3 [record] ADR revision; P4 docstring; P4 runner label.

Partially closed: P2 [contract] emission direction. The direction is now disclosed (good), but the added justification is wrong; see the new P2 below.

Open, adjudicated: P1 [record] stale attestation.

## Adjudication of the stale-attestation P1

Downgraded to **P2 [record], non-blocking for this field**, retained as a release-gate blocker in the register. Rationale:

1. It is a record defect the contract correctly rejects. The freshness mechanism (§2/§6/§8, `history_problems`) is the closure of my original P0 and is now demonstrated working against real evidence. A contract review cannot hold the contract in REVISE for behaving as specified.
2. The cure requires no change to contract, builder, or checker. Nothing in this field's scope is defective.
3. The operator constraint is explicit: neither the clean-tree rule nor the untracked slices may be disturbed. In this checkout the cure is therefore unavailable, and I record that plainly.
4. The constraint does not foreclose the cure in principle. A clean clone of `0bcc9c6` (a clone, not a linked worktree, given Vulcan's open P3 on commondir HEAD resolution) contains no untracked slices, so `make release-candidate-smoke` there yields `working_tree_clean: true` at the scope commit; §5.1's `--emit-attestation` / `--attest` path merges it into the tracked report, committed last so the artifact surface is unchanged between the attested commit and its child. That is an operator sequencing decision, not a contract matter, and I only name it.

Consequence stated for the integrator: `make quick` stays red at `0bcc9c6` until that decision is taken; no downstream release claim may cite this tree as reproducible.

## Findings on the current tree

**P2 [contract]** `docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md` §10, revision-5 paragraph: "There is no automated Rust consumer of these vectors by design: S20-130 independence forbids the Rust implementation from depending on oracle outputs." Unsupported and contradicted by the tree. §4 and `docs/WORK_PACKAGES.md:19` constrain only the oracle's direction (the oracle must not acquire a Rust dependency). `oracle/scb1/README.md:15` states that "committed expected bytes are consumed independently by the Rust fixture tests", and `crates/sley-scb1/tests/conformance.rs:45`, `crates/sley-mutate/src/codec/fixture_tests.rs:11-18` do exactly that. No document in `docs/`, the summary, or the oracle README forbids Rust from consuming oracle outputs. This is invented rationale in a governing contract; read as authority it would wrongly bar AT-MW-02/S20-310 from binding a Rust consumer. Fix: keep the factual disclosure (oracle-derived vectors, no automated Rust consumer today, Rust tests use hand-built fixtures), state that the binding is AT-MW-02/S20-310 scope, and either drop the "forbids" clause or cite the actual concern (master goal 6.5, the oracle must not become the semantic source of truth for Rust) if that is what is meant.

**P2 [record]** `evidence/release/reproducibility-report.json`: stale attestation `84bfa9c`, adjudicated above. Non-blocking for this field; blocks `make quick` and any release claim; cure path named; owner integrator/operator.

**P3 [implementation]** `scripts/generate_smp1_json_bridge_table.py:171`: `--protocol-version 2` rewrites `conformance/smp1-json-bridge/v2/methods.json` but nothing re-mints the manifest minted by hand in `0bcc9c6` (`v1`'s is minted by `generate_smp1_json_bridge_fixtures.py:88`). The next table regeneration lands `CONFORMANCE_SUMS_MISMATCH`. Fail-closed; same shape as the entity-read refresh gap just closed.

**P3 [implementation]** `bench/release/tests/test_reproducibility.py`: the rev-5 normative rules "a version without a manifest is `FIXTURE_UNREADABLE`" and "sibling manifests are sums-checked" are implemented in `version_record` but no test exercises either (existing tests cover malformed JSON and a mismatched pinned manifest only).

**P4 [contract]** §10: rev-5 delta regressed "reading as a semantically judged one" to "reading as semantically judged one".

**P4 [implementation]** `build_independent_conformance_report.py` `family_record`: versions are enumerated with `startswith("v")` over on-disk directories, not the contract's `v<N>` over tracked paths; a non-version directory is silently skipped and a `vendor`-style name would be treated as a version. Not exercised today; `--check` drift keeps it fail-closed.

**P4 [implementation]** `check_reproducibility_and_independent_conformance.py`: contract revision (spec header, ADR-0040, summary `contract_revision`) and summary `attested_commit` remain hand-synchronised and unbound; all corrected this round, but the drift pattern has now recurred twice.

**P4 [implementation]** `runner_label`: a recipe command naming a `scripts/check_*.py` absent from disk raises an unwrapped `FileNotFoundError` instead of a coded `ConformanceError`.

Handoffs (advisory, no files written): §10 wording to the integrator (contract owner is this lane; the edit is one paragraph); table-generator manifest and the two missing tests to merlin with vulcan for test adequacy; attestation refresh decision to the operator via the integrator.

---

VERDICT: REVISE
SECTION: reproducibility_and_independent_conformance
FIELD: ariadne_contract_review
SCOPE_SHA: 0bcc9c646f110e2e433e8fe3e9b913d49d84c531
FINDINGS:
[P2] [contract] docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md §10 rev-5 paragraph - "S20-130 independence forbids the Rust implementation from depending on oracle outputs" is unsupported by §4/WORK_PACKAGES S20-130 and contradicted by oracle/scb1/README.md:15 and Rust fixture tests consuming committed vectors (sley-scb1 conformance.rs:45, sley-mutate fixture_tests.rs:11); invented rationale in a governing contract; keep the disclosure, drop or correctly attribute the prohibition
[P2] [record] evidence/release/reproducibility-report.json - attests 84bfa9c, artifact surface changed to HEAD, local S20-720 record at 5b70052 working_tree_clean:false; downgraded from P1 in this field: contract and checker reject it correctly, no contract/builder/checker change needed, refresh blocked in this checkout by the standing operator constraint; a clean clone of 0bcc9c6 satisfies §1 without disturbing rule or slices; make quick stays red and no release claim may cite this tree until the operator decides
[P3] [implementation] scripts/generate_smp1_json_bridge_table.py:171 - v2 table regeneration does not re-mint the hand-minted conformance/smp1-json-bridge/v2/SHA256SUMS; next regeneration lands CONFORMANCE_SUMS_MISMATCH
[P3] [implementation] bench/release/tests/test_reproducibility.py - no test for version-without-manifest FIXTURE_UNREADABLE nor sibling manifest mismatch; rev-5 rules implemented but unbound by tests
[P4] [contract] §10 - typo regression "reading as semantically judged one"
[P4] [implementation] scripts/build_independent_conformance_report.py family_record - version enumeration is startswith("v") on disk, not v<N> over tracked paths
[P4] [implementation] scripts/check_reproducibility_and_independent_conformance.py - contract revision and summary attested_commit still hand-synchronised and unbound; drift pattern recurred twice
[P4] [implementation] scripts/build_independent_conformance_report.py runner_label - missing recipe script raises uncoded FileNotFoundError
SUMMARY: The rev-5 delta closes the substantive contract defect from my prior review: every tracked corpus version is now digested, manifest-checked, and declared, siblings carry no depth claim, the once-missing smp1-json-bridge/v2 manifest exists and verifies, pins outside the coverage map fail closed, the dead sums path is gone, the oracle refresh is promote-safe, and every prior record inaccuracy (summary attested_commit, ADR revision, docstring, runner label) is corrected; I recomputed every manifest and cross-checked the sibling digests into the tracked report. The stale reproducibility attestation is adjudicated down to a non-blocking P2 record for this field because the contract and checker reject it exactly as specified and the fix is an operator sequencing decision rather than a contract change, while remaining a red Tier-1 gate for the release lane. The verdict is REVISE rather than PASS on one narrow P2: the new §10 paragraph justifies the absence of a Rust consumer with a prohibition that S20-130 does not state and that the tree's own SCB1 and mutation-value fixture tests contradict, which under the no-invented-rationale rule must be corrected before the contract is fit to freeze. Remaining P3/P4 items are fail-closed generator, test-binding, and hygiene gaps. Not re-executed in this session: scripts and tests (tool policy); all claims rest on static reading, sha256sum, and git.
