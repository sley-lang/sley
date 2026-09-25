# required_contract_index current-delta review — Ariadne (contract lane) — index r3 (delta 23bd4cf) at scope 43f2f5b

Section: `required_contract_index`. Field judged: `current_delta_review.ariadne`. Role: Ariadne, contract lane.

## Scope verification

`git rev-parse HEAD` = `43f2f5ba738b587a9a0bb365a55f3096527e3e3d` (matches SCOPE_SHA; branch main).
Summary block `required_contract_index`: `contract_revision: 3`, `status: S20_770_CONTRACT_DRAFT_REVIEW_PENDING`,
`current_delta_review: {contract_revision: 3, ariadne: PENDING, nabu: PENDING, vulcan: PENDING}`,
`asserts_contract_acceptance: false`, `asserts_completeness: false`, `contracts_without_corpus: ["sley-test-report-v1"]`,
historical reviews `ariadne_contract_review: FAIL_0_P0_3_P1_6_P2_2_P3`, `nabu_architecture_review: FAIL_0_P0_4_P1_3_P2_3_P3`,
`vulcan_surface_review: FAIL_0_P0_1_P1_4_P2_3_P3`.
Previously judged: revision 2 at scope `a4b6029` (`delta_review-a4b6029.md`, PASS all lanes). This review covers revision 2 → 3 only.

Delta bounded: commit `23bd4cf` ("P-C §§2.5/2.8 (S20-760/770): rev-3 curative notes + records; cross-ref currency").
Index-relevant hunks (`git show 23bd4cf -- docs/spec/REQUIRED_CONTRACT_INDEX_V1.md scripts/check_required_contract_index.py scripts/test_required_contract_index.py`):
Status paragraph rev 2→3; row 17.1 definer reordered to `SSMC1_EPOCH1_SCHEMA.txt` first; rows 17.9/17.10 cite numbered sections 2/5 and 6/7; row 17.12 cites SMP1 sections 1 and 2 and adds `check_smp1_json_bridge_contract.py`; §2 gains the `docs/spec/` rule, the non-empty-corpus rule, and the presence-vs-derivation split; §3 gains the machine-readable non-acceptance sentence; §5 documents the full checker; §7 curative notes added. Checker: `quick_recipe_checkers()` recipe parser, `iter_crate_sources()` with `target` pruning, `.txt` definers, backticked domain match, empty-corpus refusal, two new summary keys. Test script: revision 2→3 in three places. (`EPOCH_MIGRATION_POLICY_V1.md` r3 in the same commit is out of scope.)

## Inputs read in full

- `docs/spec/REQUIRED_CONTRACT_INDEX_V1.md` (267 lines, Status rev 3).
- `scripts/check_required_contract_index.py` (250 lines); `scripts/test_required_contract_index.py` (121 lines); `scripts/test_current_contract_review.py` (index cases lines 128-146).
- `docs/adr/ADR-0047-required-contract-index.md` lines 1-14, 46; `docs/WORK_PACKAGES.md:70`; `docs/spec/IDENTIFIERS_V1.md:72`; `docs/spec/SMP1.md` lines 65-124 headings and line 76; `docs/spec/REPORT_ENVELOPE_PROFILE_V1.md` headings (lines 19-283); `scripts/check_report_envelope_profile.py` lines 88-102; `Makefile` `quick:` recipe (lines 3-60) and `lint:`.
- Precedent `evidence/review/verdicts/required_contract_index/delta_review-a4b6029.md`.

## Tool results (run with `SLEY2_MASTER_GOAL` set)

- `python3 scripts/check_required_contract_index.py` → `{"result": "PASS", "problems": [], "required_contracts": 12, "documents_named": 21, "domains_named": 14, "checkers_named": 19, "derived_identifier_domains": 50, "status": "S20_770_CONTRACT_DRAFT_REVIEW_PENDING"}` exit 0.
- `python3 scripts/test_required_contract_index.py` → 4 tests OK (valid baseline at revision 3; stale/bool/missing section-revision refusals).
- `python3 scripts/test_current_contract_review.py` → `IndexTerminalAcceptance.test_accepted_all_pass_accepted` (revision 3, ACCEPTED status with all-PASS) passes; the 2 failures in that file are the CLI cases (revision 6 fixtures) and belong to the `cli` section, not this one.
- `cargo test -p sley-cli -p sley-json-bridge --locked` → green (run for the package set; no crate is in this section's scope).

## Independently re-derived claims

- **Every named artifact exists and is run where the text says** (own script over the parsed table, independent of the checker): 12 rows `17.1`–`17.12`; 21 document citations (20 unique; `REPORT_ENVELOPE_PROFILE_V1.md` twice) all exist under `docs/spec/`, including `SSMC1_EPOCH1_SCHEMA.txt`; 14 digest domains all present backticked in `IDENTIFIERS_V1.md`; 19 checker citations (18 unique; `check_report_envelope_profile.py` twice) all exist under `scripts/` and every one is a recipe line of the `quick:` target (`check_smp1_json_bridge_contract.py` at Makefile:30, `check_smp1_contract.py` at :27); all 14 `conformance/...`/`crates/...` corpus cells are existing directories. The checker's 21/19 are occurrence counts, consistent with my 20/18 unique.
- **Registry drift both directions**: 50 `"sley2.*"` literals derived from `crates/**/*.rs` (target pruned) and 50 backticked `sley2.*` domains in `IDENTIFIERS_V1.md`; unregistered = [], phantom = []. Matches note A-P2-4 "50 derived vs 50 registered with zero phantoms".
- **Revision-3 notes marked DONE IN REV 3 are true against the checker**: `.txt` definers accepted (line 139); backticked domain match (146); `quick` recipe membership rather than whole-file substring (31-51, 154); empty corpus cell refused (156-158); `os.walk` prunes `target` (54-62); `asserts_contract_acceptance`/`asserts_completeness` enforced `False` (222-223); ACCEPTED status requires all three lanes PASS (107-111); status/contract-revision/current-delta binding (200-214); 77000/77001 symbols (227-230); WORK_PACKAGES reference (231-232). §5's description enumerates exactly these behaviours.
- **Row citations resolve**: SMP1 `## 1. Framing` (line 65) contains `digest domain = sley2.protocol-frame.v1 -> ProtocolFrameId` (line 76) and `## 2. Handshake` (124), as note A-P1-2 states; REPORT_ENVELOPE `## 2. Execution envelope input evidence`, `## 5. Execution report preimage`, `## 6. Restricted test aggregation`, `## 7. Test report preimage`, and `### 9.1` located after `## 10` (line 283 > 261) exactly as A-P3-11 records; the four resource-unit markers `max_memory_bytes`, `max_output_bytes`, `max_call_depth`, `wall_timeout` are asserted absent from `crates/sley-vm/src/execute.rs` by `check_report_envelope_profile.py:92-100` (V-P3-b "four literal markers" true).
- **Companion records**: ADR-0047 Status "draft at revision 3" with a revision-3 record and decision 5 "Four already-derived domains registered, none coined (2026-09-14 ...)" (A-P1-3 true); `IDENTIFIERS_V1.md:72` reads "fifteen more" (A-P2-5 correction landed); `WORK_PACKAGES.md:70` S20-770 row "Draft revision 3 (2026-09-14, ADR-0047, Council review pending)"; `scripts/check_domain_tags_and_strings.py` exists with `--self-test` and blake3 rules (V-P2-c).
- **Finding counts in the notes match the recorded historical verdicts**: Ariadne A-P1-1..3 / A-P2-4..9 / A-P3-10..11 = 3/6/2; Nabu N-P1-1..4 / N-P2-5..7 / N-P3-8..10 = 4/3/3; Vulcan V-P1 / V-P2-a..d / V-P3-a..c = 1/4/3 — identical to the summary's `FAIL_0_P0_3_P1_6_P2_2_P3`, `FAIL_0_P0_4_P1_3_P2_3_P3`, `FAIL_0_P0_1_P1_4_P2_3_P3` ("all FAIL, no P0" true). The 2026-09-04 transcripts themselves are not under `evidence/review/verdicts/required_contract_index/` (only the r2 delta and the d384f0f finals are), so the notes were checked against the summary fields, not the original transcripts.
- **Claims the notes make about open work are stated as open, not as done**: N-P1-2 (`len(rows) == 12` accepts duplicate `| 17.n |` rows — true of checker line 127-128), A-P2-7 (codes reserved, problems untyped — true), A-P2-9/N-P2-7 (section pointers not machine-checked — true), N-P1-4 (drift gate hosted under S20-770 — true). No overstatement found.
- **Revision-3 boundary claims**: "Twelve rows and all digest domains unchanged" — the diff touches document/checker cells of rows 17.1, 17.9, 17.10, 17.12 only; domain cells byte-identical; required names unchanged. "Defines no contract, changes no identity, grants no authority" — no domain, code, or corpus changed. True.
- **Prose count inconsistency**: note N-P1-3 (line 197-198) says the `sley2.` namespace is "overloaded with non-hash contract labels (20 in `scripts/`/`oracle/` today)" while V-P2-c (line 241) says "22 further `sley2.*.v1` identifiers in Python tooling"; my count of quoted `sley2.*` literals under `scripts/` and `oracle/` not present in the registry is 22. Non-normative prose in a curative note; editorial only.
- **Placement**: the "Revision 3 (2026-09-14): curative-notes amendment ..." history bullet (lines 259-267) sits as the last item of the `### Vulcan surface review` list in §7 instead of in `## 6. Revision history` (whose list stops at revision 2). Editorial.

## Per-item analysis

- Table cells: all artifacts exist, all checkers run in `make quick`, domains frozen, corpora present. No finding.
- Rules/§3/§5: each rule is enforced by the checker as described; the non-acceptance qualifier is machine-readable and checker-enforced. No finding.
- Curative notes: every DONE claim verified; every OPEN claim is a true statement of current checker behaviour; counts match the recorded verdicts. Two editorial slips (count 20 vs 22; misplaced history bullet).
- Checker/test delta: `test_required_contract_index.py` re-pinned to 3 and green; the index cases in `test_current_contract_review.py` re-pinned to 3 and green.

```
VERDICT: PASS
SECTION: required_contract_index
FIELD: current_delta_review.ariadne
SCOPE_SHA: 43f2f5ba738b587a9a0bb365a55f3096527e3e3d
FINDINGS:
[P4] [editorial] docs/spec/REQUIRED_CONTRACT_INDEX_V1.md:259-267 - the revision-3 history bullet is appended to the "### Vulcan surface review" list in section 7 instead of "## 6. Revision history", whose list stops at revision 2
[P4] [editorial] docs/spec/REQUIRED_CONTRACT_INDEX_V1.md:197-198 - note N-P1-3 says "20 in `scripts/`/`oracle/` today" while note V-P2-c (line 241) says 22 and the live count of unregistered quoted `sley2.*` literals under scripts/ and oracle/ is 22
SUMMARY: The revision-3 delta is a curative-notes amendment and it says exactly what the checker and repository do: every document (21 citations incl. the new `.txt` definer), digest domain (14, all frozen), checker (19 citations, every one a `quick:` recipe line incl. the newly listed bridge checker), and corpus directory named in the table exists and is run where the text says; the registry-drift gate holds in both directions (50/50, no drift, no phantoms); section 5 enumerates the checker's actual behaviours; the DONE/OPEN notes are true against the checker source and their per-lane counts equal the recorded 2026-09-04 verdict strings; ADR-0047, IDENTIFIERS_V1.md, and WORK_PACKAGES carry the revision-3 records; checker, index tests, and the index terminal-acceptance case are green. Two editorial items only (a misplaced history bullet and a 20-vs-22 prose count); nothing report-grade is open on this delta.
```
