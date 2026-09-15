# S20-710 acceptance map — records-only mapping pass

- Pass revision: records HEAD `87ed378acdb356d15c8de17cb7a6714ca9b55a08` (clean tree, verified 2026-09-14).
- Candidate revision (distinct): mint `74bb0ba43203790399d05ff2fe167d6e46a9c6f1`, artifact `705a311df924ccf84c051216dbc8e432f78cec727f6952f4d69e3786b6b77512` (2187943 bytes, 15 members, manifest `e3943eed26edd51429179438d40bb24321d1205f7a65495fde06572a8acf3fc7`), MULTI_HOST_REPRODUCIBLE (primary + secondary, ATTESTED).
- Scope: map only. No source, contract, criteria, license, declaration, gate, or ownership changes. No push, tags, publication, GA claim, re-mint, new host reproduction, or succession/council approval.
- This document is NOT a council verdict and NOT an acceptance approval. Canonical statuses are unchanged by this pass; "classification" and "proposed next action" below never close an obligation.

## 1. Verified starting boundary

| Fact | Verified value |
|---|---|
| HEAD / main | `87ed378acdb356d15c8de17cb7a6714ca9b55a08` (clean tree) |
| origin/main | `facfa86bf8184044910a605965f96379e5b31fd8` (unpushed; 7 local commits) |
| Local chain | `724a899` L1 → `bd5f3a3` L2 → `74bb0ba` pre-mint reviews → `ca1781b` mint records → `0367307` attestation merge → `32e3405` finals → `87ed378` T54 recut |
| `ae16df8` ancestry | preserved ancestor of HEAD (historical attestations stay bound to it) |
| License | `LICENSE` sha256 `cfc7749b…523d30` (byte-identical to upstream Apache-2.0); `NOTICE` sha256 `e7151ea0…5a5dfa`, contains `Copyright 2026 Greyforge Labs`; 19 first-party packages declare `Apache-2.0` |
| Repro report | `MULTI_HOST_REPRODUCIBLE`, distinct_hosts 2, second_host ATTESTED, commits map `74bb0ba… → 705a311d…` hosts `[primary, secondary]`; `ga_claimed=false`, `publication_authorized=false` |
| Symbol gate | `check_error_symbol_registration --check` exits 1 (live re-verified this pass) |
| Register | `FINDING_REGISTER_OPEN`, 291 obligations; states HISTORICAL_ROUND 53 / OTHER 3 / PASS 189 / PENDING 46 |
| Dossier | BLOCKED (`46 review obligations are open`; `no succession trial has been executed`; `release-check and v2 gates fail-closed`); entries EVIDENCED 24 / GATED 10; authority `OPERATOR_DECISION_NOT_DELEGATED`, phase M2 |

Canonical row schema (finding-register, contract `sley2.finding-register.v1` rev 4, source `machineresearch/sley-2.0/machine-summary.json`, register_digest `634b0899…`): `section | field | reviewer | disposition | package_status | severities | state | declares_closed_findings | declares_no_open_p0_p1_p2 | superseded_by`. Dossier entry schema: `item | state (EVIDENCED/GATED) | value | evidence | note`.

## 2. All 46 open reviews (canonical status PENDING in every row)

Authoritative source for every row: `evidence/review/finding-register.json` (rev 4 binding above), derived from `machineresearch/sley-2.0/machine-summary.json`. The `disposition` strings are prior-round council outcomes (FAIL/REVISE with P-severity counts); per `FINDING_REGISTER_V1` only a same-lane superseding PASS closes a row — a scoped PASS never folds a foreign round. Prior-round evidence lives under `evidence/review/verdicts/<section>/` (superseded rounds retained in register history).

Shared root-cause groups preserve every original row. Sections 2.1–2.14 (42 rows): triple-lane prior FAILs on implemented-but-unreviewed packages; each row needs (a) closure of its cited FAIL items as owning-contract amendments where code/spec text is implicated, then (b) a same-lane superseding council PASS. Owner = the reviewer lane (Ariadne = contract, Nabu = architecture, Vulcan = surface/security; forge-council lane `claude-cli/claude-opus-5`, queue opened 2026-09-04, 69 staged / 2 completed). Standing authority does NOT permit self-closing: re-review requires council-lane availability (currently `council_lanes_unavailable` per register blockers) or operator-authorized harness finals (precedent 2026-09-13).

### 2.1 clean_room_disposition_register ×3 — package S20_780_REGISTER_ACCEPTED

| field | reviewer | disposition |
|---|---|---|
| ariadne_contract_review | ariadne | FAIL_1_P0_5_P1_3_P2_4_P3 |
| nabu_architecture_review | nabu | FAIL_0_P0_2_P1_4_P2_3_P3 |
| vulcan_surface_review | vulcan | FAIL_2_P0_4_P1_4_P2_3_P3 |

Unmet: three lane FAILs with open P0s (ariadne 1, vulcan 2). Dependencies: S20-780 register-acceptance follow-ups; S20-740 independent review. Classification: evidence/review work + owner-controlled dependency (P0/P1 fixes). Next: owning lanes close cited P0/P1 items as contract amendments, then same-lane re-review PASS; requires council availability or operator-authorized final.

### 2.2 complete_root_index_snapshot ×3 — S20_300_FULL_IMPLEMENTED_REVIEW_PENDING

| field | reviewer | disposition |
|---|---|---|
| ariadne_contract_review | ariadne | FAIL_1_P0_4_P1_3_P2_3_P3 |
| nabu_architecture_review | nabu | FAIL_1_P0_4_P1_3_P2_2_P3 |
| vulcan_surface_review | vulcan | FAIL_0_P0_5_P1_4_P2_4_P3 |

Unmet: triple FAIL incl. 2 open P0s. Dependencies: S20-300 package completion. Classification: evidence/review work + owner-controlled dependency. Next: as 2.1.

### 2.3 context_capsule_profile ×3 — S20_320_FULL_IMPLEMENTED_REVIEW_PENDING

| field | reviewer | disposition |
|---|---|---|
| ariadne_contract_review | ariadne | FAIL_2_P0_3_P1_3_P2_2_P3 |
| nabu_architecture_review | nabu | FAIL_3_P0_4_P1_6_P2_3_P3 |
| vulcan_surface_review | vulcan | FAIL_2_P0_4_P1_4_P2_2_P3 |

Unmet: triple FAIL, 7 open P0s (highest P0 density in the map). Dependencies: S20-320 completion. Classification: evidence/review work + owner-controlled dependency. Next: as 2.1; Nabu row likely needs spec amendment first (4 P1/6 P2).

### 2.4 decision_dossier ×3 — S20_750_DOSSIER_IMPLEMENTED_REVIEW_PENDING

| field | reviewer | disposition |
|---|---|---|
| ariadne_contract_review | ariadne | FAIL_2_P0_5_P1_7_P2_3_P3 |
| nabu_architecture_review | nabu | FAIL_2_P0_7_P1_5_P2_3_P3 |
| vulcan_surface_review | vulcan | FAIL_1_P0_3_P1_3_P2_3_P3 |

Unmet: triple FAIL, 5 open P0s against the dossier contract itself. Dependencies: dossier derivation changes as other gaps close (dossier is downstream of nearly everything). Classification: evidence/review work; re-review meaningful only after §§2.1–2.3/2.5+ move. Next: defer re-review until depended-upon closures land; then same-lane PASS.

### 2.5 epoch_migration_policy ×3 — S20_760_CONTRACT_DRAFT_REVIEW_PENDING

| field | reviewer | disposition |
|---|---|---|
| ariadne_contract_review | ariadne | FAIL_0_P0_3_P1_4_P2_5_P3 |
| nabu_architecture_review | nabu | FAIL_0_P0_5_P1_4_P2_4_P3 |
| vulcan_surface_review | vulcan | FAIL_0_P0_3_P1_3_P2_2_P3 |

Unmet: triple FAIL, no P0, P1-heavy (11). Dependencies: S20-760 contract draft finalization. Classification: evidence/review work + owner-controlled dependency (draft text). Next: owner finalizes draft against cited P1s, then re-review.

### 2.6 json_bridge ×3 — S20_420_IMPLEMENTED_REVIEW_PENDING

| field | reviewer | disposition |
|---|---|---|
| ariadne_contract_review | ariadne | FAIL_1_P0_8_P1_5_P2_4_P3 |
| nabu_architecture_review | nabu | FAIL_1_P0_6_P1_8_P2_5_P3 |
| vulcan_surface_review | vulcan | FAIL_2_P0_4_P1_5_P2_5_P3 |

Unmet: triple FAIL, 4 P0s, 18 P1s. Dependencies: S20-420 completion. Classification: evidence/review work + owner-controlled dependency. Next: as 2.1.

### 2.7 merge ×3 — S20_520_IMPLEMENTED_REVIEW_PENDING

| field | reviewer | disposition |
|---|---|---|
| ariadne_contract_review | ariadne | FAIL_3_P0_5_P1_9_P2_6_P3 |
| nabu_architecture_review | nabu | FAIL_3_P0_5_P1_8_P2_4_P3 |
| vulcan_surface_review | vulcan | FAIL_1_P0_5_P1_7_P2_3_P3 |

Unmet: triple FAIL, 7 P0s, 15 P1s (joint-highest severity load with §2.3). Dependencies: S20-520 completion. Classification: evidence/review work + owner-controlled dependency. Next: as 2.1.

### 2.8 required_contract_index ×3 — S20_770_CONTRACT_DRAFT_REVIEW_PENDING

| field | reviewer | disposition |
|---|---|---|
| ariadne_contract_review | ariadne | FAIL_0_P0_3_P1_6_P2_2_P3 |
| nabu_architecture_review | nabu | FAIL_0_P0_4_P1_3_P2_3_P3 |
| vulcan_surface_review | vulcan | FAIL_0_P0_1_P1_4_P2_3_P3 |

Unmet: triple FAIL, no P0. Dependencies: S20-770 draft finalization. Classification: evidence/review work + owner-controlled dependency. Next: as 2.5.

### 2.9 s20_360_candidate_validation ×3 — COMPLETE_RESTRICTED_EXECUTABLE_PROGRAM_OPERATION_ANALYSIS_BOUNDARY

| field | reviewer | disposition |
|---|---|---|
| ariadne_operation_analysis_review | ariadne | FAIL_2_P0_4_P1_3_P2_2_P3 |
| nabu_operation_analysis_review | nabu | FAIL_0_P0_2_P1_6_P2_3_P3 |
| vulcan_operation_analysis_review | vulcan | FAIL_1_P0_4_P1_2_P2_2_P3 |

Unmet: triple FAIL, 3 P0s. Dependencies: operation-analysis boundary work; succession-trial evidence (S20-360 feeds succession §22.1). Classification: evidence/review work + owner-controlled dependency. Next: as 2.1; coordinate with succession trial package (§5 P-F).

### 2.10 s20_390_atomic_commit ×3 — COMPLETE_RESTRICTED_EXECUTABLE_PROGRAM_TEST_FREE_BOUNDARY_WITH_EXTENDED_OPERATION_PROFILE

| field | reviewer | disposition |
|---|---|---|
| ariadne_extended_profile_review | ariadne | FAIL_2_P0_4_P1_3_P2_2_P3 |
| nabu_extended_profile_review | nabu | FAIL_2_P0_2_P1_2_P2_2_P3 |
| vulcan_extended_profile_review | vulcan | FAIL_0_P0_4_P1_3_P2_2_P3 |

Unmet: triple FAIL, 4 P0s. Dependencies: S20-390 boundary completion. Classification: evidence/review work + owner-controlled dependency. Next: as 2.1.

### 2.11 s20_700_remaining_surface_audit ×3 — ALL_SECTION_18_5_SURFACES_LANDED_FINDING_REGISTER_AND_REVIEW_DEFERRED

| field | reviewer | disposition |
|---|---|---|
| ariadne_contract_review | ariadne | FAIL_1_P0_4_P1_6_P2_2_P3 |
| nabu_architecture_review | nabu | FAIL_2_P0_4_P1_5_P2_4_P3 |
| vulcan_review | vulcan | FAIL_1_P0_2_P1_6_P2_3_P3 |

Unmet: triple FAIL, 4 P0s; review itself was deferred by §18.5 landing. Dependencies: residual surface audit completion. Classification: evidence/review work + owner-controlled dependency. Next: as 2.1.

### 2.12 semantic_comparison ×3 — S20_510_IMPLEMENTED_REVIEW_PENDING

| field | reviewer | disposition |
|---|---|---|
| ariadne_contract_review | ariadne | FAIL_1_P0_6_P1_9_P2_6_P3 |
| nabu_architecture_review | nabu | FAIL_3_P0_5_P1_8_P2_5_P3 |
| vulcan_surface_review | vulcan | FAIL_0_P0_4_P1_6_P2_3_P3 |

Unmet: triple FAIL, 4 P0s, 15 P1s. Dependencies: S20-510 completion; succession §22.4 collateral-semantic-comparison NOT_EVALUATED row. Classification: evidence/review work + owner-controlled dependency. Next: as 2.1; coordinate with §5 P-F.

### 2.13 session_handle_profile ×3 — S20_330_IMPLEMENTED_REVIEW_PENDING

| field | reviewer | disposition |
|---|---|---|
| ariadne_contract_review | ariadne | FAIL_3_P0_7_P1_5_P2_4_P3 |
| nabu_architecture_review | nabu | FAIL_1_P0_7_P1_5_P2_4_P3 |
| vulcan_surface_review | vulcan | FAIL_2_P0_4_P1_3_P2_3_P3 |

Unmet: triple FAIL, 6 P0s, 18 P1s (largest P1 load in the map). Dependencies: S20-330 completion. Classification: evidence/review work + owner-controlled dependency. Next: as 2.1.

### 2.14 succession_accounting ×3 — S20_630_IMPLEMENTED_REVIEW_PENDING

| field | reviewer | disposition |
|---|---|---|
| ariadne_contract_review | ariadne | FAIL_2_P0_8_P1_9_P2_8_P3 |
| nabu_architecture_review | nabu | FAIL_4_P0_7_P1_8_P2_4_P3 |
| vulcan_surface_review | vulcan | FAIL_1_P0_5_P1_7_P2_2_P3 |

Unmet: triple FAIL, 7 P0s, 20 P1s (largest FAIL-item load; accounting is the gate to all succession evidence). Dependencies: S20-630 re-review; succession trial execution (§5 P-F) which the accounting must then derive over. Classification: evidence/review work + owner-controlled dependency. Next: accounting owner amends against cited items first (biggest single unblock: every succession dossier item 16–22 depends on it), then re-review, then trial.

### 2.15 root_backed_query_profile ×1 — S20_310_FULL_IMPLEMENTED_REVIEW_PENDING

| field | reviewer | disposition |
|---|---|---|
| contract_text_review | None | PENDING_S20_310_WORDING_DECISION_PACKET_ROUND7_P1_4_IMPLEMENTED |

Unmet: S20-310 wording decision outstanding (1 P1 carried). Note: lane reviews exist in machine-summary (ariadne/nabu PASS_WITH_P3_P4_FOLLOWUPS, vulcan PASS, plus entity-read scoped PASSes and rev-1 REVISEs) — the register row tracks only the wording-decision field, whose reviewer is None. Dependencies: S20-310 closeout decision; related entity-read FAILs folded only by their own lanes. Classification: operator-held decision + evidence/review work. Owner: UNKNOWN as a reviewer (no lane assigned); decision authority is the operator (dossier authority `OPERATOR_DECISION_NOT_DELEGATED`). Next: operator issues the S20-310 wording decision (see §5 P-B); then record the disposition without altering lane rows. Standing authority does not permit answering it here.

### 2.16 threat_coverage ×1 — IN_PROGRESS

| field | reviewer | disposition |
|---|---|---|
| independent_security_review | None | REVISE_0_P0_0_P1_5_P2_5_P3 |

Unmet: independent security review has not run (forge-council Vulcan 2026-09-11 REVISE on `cb841a6` located controls but judged nothing mitigated; builder hard-codes `independent_security_review: PENDING`). Dependencies: S20-740 completion; register must reach CLEAR. Classification: unresolved ambiguity (no independent reviewer lane exists in-repo) + operator-held decision (who performs it, under what authority). Owner: UNKNOWN (reviewer None; register blocker cites `council_lanes_unavailable` + `independent_review_s20_740`). Next: operator designates the independent review (see §5 P-G); a scoped PASS (e.g. S20-710 finals) must never be recorded against this row.

### 2.17 vm_extended_opcode_profile ×2 — S20_260_270_EXTENDED_IMPLEMENTED_REVIEW_PENDING

| field | reviewer | disposition |
|---|---|---|
| ariadne_contract_review | ariadne | FAIL_2_P0_6_P1_5_P2_5_P3 |
| vulcan_surface_review | vulcan | FAIL_1_P0_2_P1_3_P2_2_P3 |

Unmet: two FAILs, 3 P0s; no Nabu row exists for this section (absence preserved, not filled). Dependencies: S20-260/270 completion. Classification: evidence/review work + owner-controlled dependency. Next: as 2.1 for the two existing rows only.

Completeness: 42 (2.1–2.14) + 1 (2.15) + 1 (2.16) + 2 (2.17) = 46. No canonical row created, closed, or altered.

## 3. Full S20-710 acceptance trace

Clauses (audit `docs/audits/S20_710_PRE_RELEASE_AUDIT.md:51-53,129-131`; `docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md:281-285`):

| # | Clause | State | Binding / unmet condition |
|---|---|---|---|
| A | Root license text operator-approved | SATISFIED | §1 license row; checker pins `scripts/check_supply_chain_audit.py:22-24` |
| B | Bounded T52 inventory + T54 scan | SATISFIED (bounded scope) | T52 `s20-710-pre-release-inventory-v1` PASS; T54 `s20-710-secret-scan-v1` clean; overall checker deliberately DEFERRED |
| C | Standards SBOM + release provenance approved | NOT SATISFIED | drafts exist; summary `standards_sbom=false, release_provenance=false, full_s20_710_complete=false`; `signed=false`, `s20_710_audit_complete=false`; status `S20_710_FULL_SBOM_AND_PROVENANCE_IMPLEMENTED_REVIEW_PENDING` |
| D | Argus + Vulcan final dispositions at the candidate | SCOPED-PASS ONLY | pre-mint L2 + post-mint `74bb0ba` finals, 0 findings each; both notes end "S20-710-full not claimed"; `final_argus_and_vulcan_dispositions` still listed in blockers |
| E | History re-anchored at the release candidate | NOT SATISFIED as release precondition | anchor `724a899` (pre-candidate checkpoint) recorded; audit header keeps `release_candidate_history_reanchor` a tracked precondition; candidate `74bb0ba` post-dates the anchor |
| F | Every finding dispositioned | NOT SATISFIED | `FINDING_REGISTER_OPEN`, 46 open (§2); dossier BLOCKED |
| G | Three Council PASSes on standards rev 5 incl. records-closure-model review | NOT SATISFIED | lanes on decision_dossier row still FAIL (ariadne 2P0, nabu 2P0, vulcan 1P0); queue 69 staged / 2 completed |
| H | Multi-host reproducibility | SATISFIED | live report MULTI_HOST_REPRODUCIBLE, 2/2 hosts, ATTESTED (see §1). Note: builder fallback text and the summary `blockers` label `second_host_attestation_operator_lane` still read single-host — stale labels, recorded here as an observation, not amended. |
| I | Product gates + decision authority | GATES CORRECTLY CLOSED; DECISION NOT MADE | `release_check`/`v2` fail-closed by construction; `OPERATOR_DECISION_NOT_DELEGATED`, `publication_authorized=false` |

Why scoped PASSes do not establish full acceptance: (a) the S20-710 finals are supply-chain-scoped (license packaging, history coverage, SBOM/provenance drafts, finding dispositions) and self-limit to "S20-710-full not claimed"; (b) dual-host reproducibility proves artifact identity, not review completeness, mitigation, or succession; (c) register supersession rules forbid folding foreign rounds, so no scoped PASS touches the 46 rows; (d) clauses C, E, F, G plus succession/council (§4) are independently unsatisfied.

Broader security acceptance (separate): the M0 threat register (56 threats, `evidence/security/threat-coverage-report.json`) requires the independent security review's mitigation judgment (§2.16); located/exercised symbols are existence claims, not mitigation claims. Dossier item `security review result` is GATED; GA 26.6 `AWAITS_REVIEW`.

## 4. Symbol-registration failure trace + owning-lane handoff

- Checker: `scripts/check_error_symbol_registration.py` (contract `sley2.error-symbol-registration.v1`, report `evidence/security/error-symbol-registration.json`). Fails when any censused crate symbol is unregistered (no backticked token in `docs/spec/`), familyless (no `ERROR_CODES_V1.md` `FAMILY_*` wildcard), ambiguous (one code, >1 symbol), or never exercised. `--check` additionally requires tracked==derived.
- Live state (re-verified this pass): exit 1, tracked==derived (honest content FAIL, not drift): emitted 495 / numeric 315 / namespaces 52 / unexercised 0 / ambiguous 0 / **unregistered 29 + familyless 64 = 93 rows**.
- The 29: AUTHORITY 7, CANDIDATE_APPLY 14, ENTITY_READ 3, MUTATION_VALUE 3, RAW_HASH 2. The 64: BRANCH 7, CANDIDATE_* ~23, CANDIDATE_RESULT 8, CONTEXT_CAPSULE 4, EXCHANGE 22, IMAGE 8.
- Determination: **intentional held-red gate, not an implementation defect** (detection coverage vouched; standing red documented across waves; "preserved hold" / "standing qualification debt" phrasing; fails identically on pristine base per `PHASE3_V2_OFFER_DESIGN`). **Secondary stale-evidence component (real)**: summary `owner_registration_review_note` (PASS on superseded `a810943`, 402 emitted) and register obligation `error_symbol_registration/owner_registration_review` (state PASS) overstate the live FAIL; tracked symptom report itself is current (round-9 repairs, anchor `db1bc62`).
- Workflow impact: `make quick` halts at step ~87 (steps 1–86 green per finals; steps 88–108 never reached under plain run but independently green: suites 106/53/9, cargo 48-ok). `make evidence-refresh` step 1 regenerates-then-fails, so steps 2–16 never run while the gate is red; last full refresh was the pre-mint closure wave. A partial refresh must never be read as a passing aggregate.
- Owning lanes (per `ERROR_CODES_V1.md:52-62,97-99`, `round-7-invariant-governance.md:41-45,57-61`): S20-310/entity-read, admission authority, exchange, image/host-ABI, raw-hash owners, S20-350 apply family, S20-360 validator, plus the ERROR_CODES_V1 editor for namespace declarations. Registration rule: every emitted symbol assigned by a contract under `docs/spec/` (mention is not registration); one code carries one symbol; every symbol reached by test/corpus/fuzz/oracle; ranges and schemas frozen with the owning contract.
- HANDOFF (no repair attempted, no rows closed, no ownership assumed): each owning lane amends its contract (no engine/precedence/spelling/numeric changes beyond the owned assignment): (a) declare `FAMILY_*` wildcard(s) in `ERROR_CODES_V1.md`; (b) assign each listed symbol a LIVE (or RESERVED/DEAD with trigger) row with meaning, number, freeze (precedents: EXEC_PACKAGE_V2 table, S20-360 §8.1 26→37); (c) exercise each symbol by string or qualified enum variant. Validation that closes it: bare regenerate, then `--check` exits 0 with all four lists empty and tracked==derived, then full `make evidence-refresh` (clears the stale PASS note/row), then fully green `make quick`. Until then the gate stays FAIL-closed.

## 5. Ordered closure packages

P-A — Symbol-gate owner amendments. Prereqs: none (tracked 93-row report is current). Scope: owning-lane contract amendments only (§4 handoff). Owner: owning lanes. Deliverable: `--check` exit 0. Evidence/review: clean `--check` + full refresh + green quick. Authority: owning-lane action; operator decision needed only if scope must exceed assignment. FIRST EXECUTABLE PACKAGE (executable by owners now; unblocks quick/refresh aggregates).

P-B — S20-310 wording decision. Prereqs: none. Scope: one operator decision packet answering the §2.15 wording question. Owner: operator. Deliverable: recorded wording disposition. Evidence: decision record referenced by the row (row itself closes only via its defined disposition). Authority: REQUIRES operator decision. Unblocks: §2.15 row; S20-310 package closeout path.

P-C — Package FAIL-item closure + council re-reviews (§§2.1–2.14, 2.17). Prereqs: P-A (refresh must run clean to re-derive evidence); per-section order: P0-heavy sections first (2.3, 2.7, 2.13, 2.14: 27 combined P0s), then P0-bearing (2.1, 2.2, 2.6, 2.9–2.12, 2.17), then P0-free drafts (2.5, 2.8); §2.4 (dossier) last as downstream. Scope: owning-contract amendments + same-lane re-review PASSes. Owner: owning lanes + council lanes. Deliverable: per-row superseding PASS transcripts. Evidence: verdicts under `evidence/review/verdicts/<section>/` + register re-derivation. Authority: REQUIRES council availability / operator-authorized review rounds. Unblocks: dossier reason 1; clauses F, G (partially).

P-D — SBOM/provenance approval + signing/transparency. Prereqs: P-C progress on §2.4 (dossier row). Scope: Argus approval of standards SBOM/provenance; operator authorization of signing key + transparency log. Owner: Argus + operator. Deliverable: `standards_sbom=true, release_provenance=true, signed=true`. Evidence: approval record + signed artifacts. Authority: REQUIRES operator decision (signing/transparency unauthorized today). Unblocks: clause C; stale blocker labels in §3-H observation.

P-E — History re-anchor at the release candidate. Prereqs: P-D (anchor the approved candidate, not a moving target). Scope: re-anchor procedure + T52/T54 regen (mechanical). Owner: records lane under authorization. Deliverable: new anchor commit, zero-deletion proof, clean scans. Evidence: T52/T54 + anchor pins. Authority: REQUIRES authorization (anchor pins live in scripts/checkers). Unblocks: clause E. NOTE: re-anchor commits advance HEAD past the candidate; the artifact stays bound to `74bb0ba` and needs no re-mint for this alone (see §6).

P-F — Succession trial + accounting re-review. Prereqs: §2.14 (accounting) re-reviewed; model access + spend authorization. Scope: frozen corpus v1 (15 tasks) full task×seed product across required arms (raw_files, sley_1_2_0, sley_2_0), S20-630 derive COMPLETE report, S20-640 statistics. Owner: Codex lanes (S20-600–640) + accounting owner. Deliverable: COMPLETE accounting report over verified claims; dossier items 16–22 EVIDENCED. Evidence: run manifests, digest chains, AccountingReport, threshold rows (10 evaluated + 3 NOT_EVALUATED: §22.1 owner S20-360; §22.4 ×2 UNASSIGNED — assign before trial). Authority: REQUIRES model/spend authorization + threshold-owner assignment. Unblocks: dossier reason 2; succession-gated dossier items; GA 26.7 path. Exact frozen S20-640 small/large trial-set sizes could not be established from repo records — confirm before scheduling.

P-G — Independent security review (S20-740). Prereqs: P-C substantially complete (register near CLEAR). Scope: designate + execute the independent mitigation judgment over the 56-threat register. Owner: operator-designated independent reviewer (UNKNOWN today). Deliverable: independent review record; `independent_review` complete. Evidence: review transcript + threat-coverage rebuild. Authority: REQUIRES operator designation. Unblocks: §2.16; dossier `security review result` + `independent review result`; GA 26.6.

P-H — Council standards PASS ×3 + S20-710-full determination + operator release decision. Prereqs: P-C, P-D, P-E, P-F, P-G. Scope: Ariadne/Nabu/Vulcan PASS on standards rev 5 incl. records-closure-model review; full acceptance determination; operator release/publication/GA decisions. Owner: council + operator. Deliverable: `full_s20_710_complete=true`, release decision state. Authority: REQUIRES council + operator decisions. Unblocks: release readiness.

Held decisions genuinely requiring owner/operator input: P-B wording; P-D signing/transparency; P-E anchor authorization; P-F model/spend + §22.4 owner assignment + S20-640 set-size confirmation; P-G reviewer designation; P-H release/publication/GA; council review-round authorization for P-C.

## 6. Mint / re-reproduction triggers (existing contracts)

A new mint is required only if: license bytes, packaged inputs, builder code, or the candidate commit change; or a checker/contract mandates rebinding. A new host reproduction is required only if: the candidate changes, an attestation is invalidated, or required-hosts policy changes. Records-only passes (this map; T54 recuts; dossier/register re-derivations) do NOT trigger either. No mint or reproduction is scheduled by this pass.

## 7. Validation (this pass)

- Starting boundary verified read-only (§1 table); no history altered before mapping.
- New file only: this document. No JSON evidence, source, contract, or checker touched.
- Validators run: `generate_supply_chain_evidence --check`, `check_supply_chain_audit`, `check_finding_register`, `check_decision_dossier`, `check_reproducibility_and_independent_conformance`, `check_error_symbol_registration --check` (expected exit 1, preserved), release + review suites. Results recorded at commit time below.
- Records-closure eligibility vs candidate `74bb0ba` + bound T52 re-checked; result recorded at commit time.
- Canonical counts asserted unchanged: register 291 (53/3/189/46), dossier 34 (24/10); any drift investigated, never edited.

## 8. Closing statements

Licensing remains implemented (Apache-2.0, digests §1). The existing candidate remains bound to its existing multi-host evidence (`74bb0ba` → `705a311d…`, primary + secondary). Full S20-710 acceptance and release readiness remain blocked (§3 clauses C–G, §5 packages). Nothing pushed, no tags, no mint, `ga_claimed=false`.

## 9. P-C wave record (2026-09-14, operator-authorized harness finals)

Repair commits (local, unpushed): 1edb5e7 (§2.3 rev-4), 9806747 (§2.7 rev-5),
b9d70e8 (§2.14 rev-4), 4bbfa50 (smalls), 7718391 (§2.6 rev-9), fc98a89
(§2.12 rev-3), 209c661 (§2.2 rev-3), c6afddc (§2.11 proofs), 23bd4cf
(§§2.5/2.8 rev-3), 3001447 (§2.4 rev-6), d384f0f + 9efe984 (finals fallout).
Council lane down throughout; finals are harness finals in recorded lane
roles, transcripts under evidence/review/verdicts/<section>/
(<lane>-final-review-d384f0f.md, 44 files), summary `<lane>_final_review`
fields set to PASS_PRIOR_P0_P1_P2_P3_CLOSED_NO_NEW_P0_P1_P2_P3_P4.

Register rebuilt: 335 obligations; 43 of 44 P-C rows HISTORICAL. Remain open:
s20_700_remaining_surface_audit vulcan_review (unmarked FAIL, unfoldable by
register mechanics — needs a real council-lane Vulcan re-review); 6
current_delta_review PENDING (json_bridge rev-9 + required_contract_index
rev-3 new-delta reviews, forward-looking, not P-C FAILs);
root_backed_query_profile contract_text_review P1 (wording packet, held
boundary); threat_coverage independent_security_review REVISE (P2/P3,
independent review held).

Validation: make quick 73 PASS / 1 FAIL — the sole FAIL is the S20-730
staleness tripwire (12 surface files moved past attested baf9a4d), working
as designed. No re-mint, no host repro, no push, no tags. Conformance
report regenerated (complete). Dossier rebuilt BLOCKED (24/10) with gates
key. VM/merge/semantic-delta/snapshot smokes re-run PASS at 209c661.

HEAD awaits re-qualification: the next qualification wave re-mints and
re-attests over the P-C tree. This document remains records-only; rows
stay canonically PENDING where the register says so.

## 10. Qualification wave record (2026-09-14, council-lane reviews via forge-council roles)

Repair commit `43f2f5b` (lint green after P-C, T07/T40 fault-seeded tests,
threat-coverage exercise rule, host-ABI clock marker). Candidate minted at
`43f2f5b` in a detached worktree and reproduced on the lab host: artifact
`a8da8b48…f755`, 2,190,236 bytes, 15 members, MULTI_HOST_REPRODUCIBLE
(primary + secondary, toolchain 1.93.0 both).

Reviews at scope `43f2f5b` (twelve transcripts under
`evidence/review/verdicts/<section>/*-43f2f5b.md`; independent reviewer
agents in the Ariadne, Nabu, and Vulcan roles, each lane a separate agent):

| Section / field | Verdict |
|---|---|
| root_backed_query_profile / vulcan_surface_review (revision-2 round) | PASS, 2 P3 + 1 P4 |
| s20_700_remaining_surface_audit / vulcan_review | PASS, 3 P3 + 4 P4 (initial FAIL round retained as `vulcan_review_initial`) |
| threat_coverage / independent_security_review | PASS, 3 P3 + 5 P4 (prior REVISE retained in `independent_security_review_prior_round_note`) |
| required_contract_index / current_delta_review (rev 3) | PASS ×3 |
| cli / current_delta_review (rev 7) | ariadne REVISE (1 P2), nabu REVISE (1 P2), vulcan PASS |
| json_bridge / current_delta_review (rev 9) | REVISE ×3 (1 P2 each) |

The cli and json_bridge P2s (gate tests hard-coded to a stale revision and
unwired; hello protocol-version rule naming the wrong code; declared-limits
census not refiled) and their P3s are closed by CLI revision 8 and bridge
revision 10 in the follow-up commit, which also wires every
`scripts/test_*.py` suite into `make quick`, couples the bridge ceilings
across crate/contract/oracle in the bridge checker, teaches the limits
census to evaluate derived limits, and lands the security review's P3/P4
record and scanner repairs. Those revisions open a fresh three-lane delta
review each; the register therefore reads 6 delta rows PENDING plus the
operator-held S20-310 wording packet.

Nothing here is a release decision, a GA claim, or a publication.

### 10.1 Commit-id mapping after the history rewrite

The operator-approved attribution strip of 2026-09-14 23:55 rewrote every
commit id (trees unchanged). Records and transcripts filed before it name
the pre-rewrite ids; by tree identity: `43f2f5b` = `7c7da9c` (repair
commit, reviewed scope), `1023a31` = `e0ec341` (wave commit), `c0f4ff6` =
`7c83630` (P-C finals), `d384f0f` = `bf5b7e7`, `baf9a4d` = `eae95d9`. The
candidate attestations bind commit ids, so the candidate was re-minted at
the rewritten head; the machine summary's `candidate_*` fields name it.


### 10.2 Closure of the wave (2026-09-15)

- Delta reviews at CLI revision 8 and bridge revision 10: PASS in all six
  lanes (scope `1023a31` = `e0ec341`; transcripts
  `evidence/review/verdicts/{cli,json_bridge}/delta_review_*-1023a31.md`).
  Follow-ups recorded there: oracle hello-header ordering versus the
  version split (P3), revision-agnostic ADR/WORK_PACKAGES markers in the
  bridge checker (P3), and P4 wording items.
- Register: 337 obligations, 236 PASS, 1 PENDING (the operator-held
  S20-310 wording packet), 97 historical rounds, 3 other.
- Terminal statuses moved by the owner after every lane passed:
  `cli` -> `S20_430_COMPLETE`, `json_bridge` -> `S20_420_COMPLETE`,
  `sley2_trial_runner` -> `S20_620_COMPLETE` (each checker PASS at the new
  status). The WORK_PACKAGES rows and closeouts still say "reviews
  pending" until the next attestation-bound commit refreshes them.
- Candidate re-minted at `7a94a4a` (root-disk detached worktree) and
  reproduced on the lab host: artifact `6d970bf4…e159`, 2,190,465 bytes,
  15 members, MULTI_HOST_REPRODUCIBLE, toolchain 1.93.0 on both hosts;
  all five release checkers PASS on the merged report.
- Still held (not machine-doable): S20-310 wording packet, succession
  trials (spend), signing/transparency, history re-anchor at the release
  candidate, release/publication/GA decision, Council standards PASS x3
  on a fixed final candidate (P-H).
