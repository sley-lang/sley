# required_contract_index current-delta review — REQUIRED_CONTRACT_INDEX_V1 revision 3 (delta 23bd4cf) — Vulcan surface lane

Section: `required_contract_index`. Field: `current_delta_review.vulcan`. Role: Vulcan (QA/security, surface lane).

## Scope verification

- `git rev-parse HEAD` = `43f2f5ba738b587a9a0bb365a55f3096527e3e3d` (matches SCOPE_SHA; branch main).
- Summary block read directly: `required_contract_index.contract_revision = 3`, `current_delta_review = {contract_revision: 3, ariadne: PENDING, nabu: PENDING, vulcan: PENDING}`, status `S20_770_CONTRACT_DRAFT_REVIEW_PENDING`, `asserts_contract_acceptance: false`, `asserts_completeness: false`, `contracts_without_corpus: ["sley-test-report-v1"]`, four `domains_added_to_registry`.
- Previously reviewed revision: 2 (closed at `a4b6029` by `evidence/review/verdicts/required_contract_index/delta_review-a4b6029.md`). Delta under judgment: revision 2 -> 3, commit `23bd4cf` (curative notes + records; cross-ref currency), with the later `IDENTIFIERS_V1.md` one-word correction it names.

## Inputs read in full

- `git show 23bd4cf --stat` (13 files) and `git diff a4b6029..HEAD -- docs/spec/REQUIRED_CONTRACT_INDEX_V1.md scripts/check_required_contract_index.py scripts/test_required_contract_index.py docs/adr/ADR-0047-required-contract-index.md` (full).
- `docs/spec/REQUIRED_CONTRACT_INDEX_V1.md` at HEAD: Status paragraph, section 1 table (12 rows), section 2 rules, section 3 boundary, section 5 checker description, section 6 revision history (lines 105-115), section 7 curative notes (lines 116-266).
- `scripts/check_required_contract_index.py` at HEAD: `quick_recipe_checkers` (recipe-block membership parse), `iter_crate_sources` (prunes `target/`), `.md`/`.txt` definer rule, backticked-domain match, non-empty corpus cell rule, both-direction drift/phantom gate, summary field requirements including the two disclaimers.
- `scripts/test_required_contract_index.py`, `scripts/test_current_contract_review.py` (index classes), `Makefile` `quick:` recipe (lines 84-85 run `check_required_contract_index.py` and `check_domain_tags_and_strings.py`).
- `docs/spec/IDENTIFIERS_V1.md` lines 72-90 and 115 (registry count, scope boundary, domain-tags gate); `git diff a4b6029..HEAD -- docs/spec/IDENTIFIERS_V1.md`.
- `scripts/check_report_envelope_profile.py` lines 19 and 92-99 (the four resource-unit markers read from `crates/sley-vm/src/execute.rs`).

## Tool results (executed, exact)

- `SLEY2_MASTER_GOAL=... python3 scripts/check_required_contract_index.py` -> `{"result": "PASS", "problems": [], "required_contracts": 12, "documents_named": 21, "checkers_named": 19, "domains_named": 14, "derived_identifier_domains": 50, "status": "S20_770_CONTRACT_DRAFT_REVIEW_PENDING"}` exit 0.
- `python3 scripts/test_required_contract_index.py` -> `Ran 4 tests ... OK` exit 0 (baseline accepted at revision 3; stale revision 1 refused; bool revision refused).
- `python3 scripts/test_current_contract_review.py` -> `..FF..`; the two failures are the CLI classes (revision 6 vs 7, recorded in the cli transcript); `IndexTerminalAcceptance` and the index baseline pass at revision 3.
- Surface re-derivation script over the section 1 table (parsed independently of the checker): 20 distinct defining documents, all present under `docs/spec/` including `SSMC1_EPOCH1_SCHEMA.txt`; 18 distinct checkers, all present under `scripts/` and all members of the Makefile `quick:` recipe (confirmed by the checker's recipe-membership rule passing); 13 `conformance/...` corpora, every one with `sha256sum -c SHA256SUMS` exit 0:
  `candidate-result/v1` (accepted, rejected OK), `context-capsule/v1` (accepted OK), `mutation-candidate/v1` (accepted, rejected OK), `mutation-value/v1` (accepted, rejected OK), `release-demo/v1` (demo OK), `repository-exchange/v1` (accepted, rejected OK), `repository-pack/v1` (accepted, rejected OK), `scb1/v1` (accepted, rejected OK), `smp1-json-bridge/v1` (roundtrip, rejected, methods OK), `smp1-json-bridge/v2` (methods OK), `smp1/v1` (accepted, rejected OK), `state-root/v1` (accepted OK), `transaction-receipt/v1` (accepted, rejected OK).
- `grep -c` of the four registered domains in `IDENTIFIERS_V1.md` (backticked): `sley2.candidate-attempt.v1` 1, `sley2.protocol-frame.v1` 1, `sley2.root-query.v1` 1, `sley2.session.v1` 1. `git diff a4b6029..HEAD -- docs/spec/IDENTIFIERS_V1.md` -> exactly one line: "sixteen more" -> "fifteen more" (line 72).
- `python3 scripts/check_domain_tags_and_strings.py --self-test` -> `SELF_TEST PASS: 3 cases` exit 0; plain run -> `{"result": "PASS", "registered_domains": 50, "tag_rows": 8, "unregistered_script_labels": 44}` exit 0.
- `python3 scripts/check_report_envelope_profile.py` -> `"result": "PASS"`, `"test_pass_claim": false` (the row 17.10 no-corpus reason remains checkable through the four `execute.rs` markers).

## Independently re-derived claims

1. **Every named surface exists and pins.** All documents, checkers, and corpora named in the revision-3 table exist; every corpus directory has a `SHA256SUMS` that verifies; every checker is a `quick:` recipe line (the checker's new recipe-block parser, not a whole-file substring, enforces this and passes). The only surface without a corpus (`sley-test-report-v1`) carries its stated, checkable reason, and the summary lists it in `contracts_without_corpus`.
2. **Revision-3 table edits are traceability corrections, not surface changes.** Row 17.1 adds `SSMC1_EPOCH1_SCHEMA.txt` (exists) as the entity-body-kinds definer; row 17.12 cites `SMP1.md` sections 1-2 and adds `check_smp1_json_bridge_contract.py` (exists, in `quick:`, PASS at revision 9); rows 17.9/17.10 cite numbered sections. Twelve rows, fourteen backticked domains, all present backticked in `IDENTIFIERS_V1.md`; no thirteenth identity.
3. **Checker claims in section 5 match the code.** Each behaviour listed at lines 92-103 maps to code in the checker diff: `.md`/`.txt` definers under `docs/spec/`, recipe membership, backticked domains, non-empty corpus cell, `iter_crate_sources` drift gate in both directions (50 derived = 50 registered, zero phantoms), summary status/revision/current-delta binding, the two disclaimer booleans, 77000/77001 symbols, and the work-package reference. `test_required_contract_index.py` proves the revision binding refuses stale and mistyped revisions.
4. **Curative notes hide no unverified surface.** Every DONE-IN-REV-3 item was re-derived: A-P1-1/A-P2-8 (`.txt` definer, checker rule), A-P1-2 (17.12 sections), A-P1-3/V-P2-d (ADR-0047 decision 5 records the four already-derived domains; each is registered), A-P2-4/N-P2-5/V-P2-a (both-direction drift gate live, 50/50/0), A-P2-5 (one-word registry correction present), A-P2-6 (section 5), A-P3-10 (numbered sections), N-P1-1/V-P3-c (recipe-block parser), N-P2-6 half (non-empty corpus cell rule), N-P3-8/V-P3-a (backticked match), N-P3-9 (rule stated + enforced), N-P3-10 (`target/` pruned during walk), V-P1 (disclaimer booleans checker-enforced), V-P2-c (`IDENTIFIERS_V1.md:78-90` scope boundary + `check_domain_tags_and_strings.py` in `quick:` with self-test). Items left OPEN (A-P2-7 code emission, A-P2-9/N-P2-7 checkable section pointers, A-P3-11, N-P1-2 master-goal transcription, N-P1-3 declaration-site derivation, N-P1-4 ownership, N-P2-6/V-P2-b reason pin, V-P3-b rename-resistant guard) are stated as open with owners and are all checker-hardening or upstream-owned work; none of them is an existing surface asserted as verified. The index still defines no contract and asserts no acceptance (`asserts_contract_acceptance: false`, `asserts_completeness: false`, section 3 text).
5. **No frozen bytes moved.** The delta touched no corpus; every listed corpus verifies against its sums.
6. **Editorial.** The "Revision 3 (2026-09-14)" record bullet (lines 259-266) sits at the end of section 7 under the "### Vulcan surface review" heading instead of in section 6 "Revision history" (lines 105-115), which therefore ends at revision 2; the Status paragraph carries the revision-3 summary, so nothing is hidden, but a reader of section 6 alone sees no revision-3 entry.

## Per-item analysis

- Adversarial adequacy of the declared surface: index names only surfaces that exist and are pinned; the checker now rejects a checker moved out of `quick:`, an empty corpus cell, a non-backticked domain, and a phantom registry row. PASS.
- Fail-closed behavior: the checker and its unit tests fail closed on stale/mistyped revisions and missing artifacts. PASS.
- Limits/ceilings: not applicable to this index (traceability only).
- Tests/corpora reaching refusal paths: `test_required_contract_index.py` 4/4 and the index classes of `test_current_contract_review.py` pass; all 13 corpora verify. PASS.
- No regression in frozen bytes: verified. PASS.

## Findings

- [P4] [editorial] `docs/spec/REQUIRED_CONTRACT_INDEX_V1.md:259-266` - the revision-3 record bullet is placed under section 7 "### Vulcan surface review" rather than in section 6 "Revision history" (lines 105-115), which stops at revision 2. Move it under section 6.

```
VERDICT: PASS
SECTION: required_contract_index
FIELD: current_delta_review.vulcan
SCOPE_SHA: 43f2f5ba738b587a9a0bb365a55f3096527e3e3d
FINDINGS:
[P4] [editorial] docs/spec/REQUIRED_CONTRACT_INDEX_V1.md:259 - revision-3 record bullet sits under section 7 "### Vulcan surface review" instead of section 6 revision history (:105-115), which ends at revision 2
SUMMARY: The revision-3 delta (23bd4cf) is a curative-notes and traceability-precision amendment: every named surface exists and pins (20 defining documents including SSMC1_EPOCH1_SCHEMA.txt, 18 checkers all members of the Makefile quick recipe, 13 corpora each verifying with sha256sum -c SHA256SUMS), check_required_contract_index.py PASS (12 rows, 14 domains, 50 derived = 50 registered, zero phantoms) and test_required_contract_index.py 4/4 pass at revision 3, the four already-derived domains and the one-word registry correction are present in IDENTIFIERS_V1.md, check_domain_tags_and_strings.py self-test and run PASS, and the row 17.10 no-corpus reason stays checkable through check_report_envelope_profile.py. Every DONE-IN-REV-3 curative note was re-derived against code or text; OPEN items are declared with owners and are checker-hardening or upstream work, hiding no unverified surface; the index still asserts no acceptance or completeness. One P4 editorial follow-up. No P0/P1/P2 open.
```
