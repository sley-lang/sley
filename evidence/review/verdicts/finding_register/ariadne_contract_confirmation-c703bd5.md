# Ariadne confirmation — finding register post-scope repairs

Baseline verified: `git rev-parse HEAD` = `c703bd5c5c2aeb34f0d52d778a90c738287ab190`, tree clean except this verdict file (the `c703bd5` delta over pushed `1ac7f12` is exactly the vacuous-continuation repair: builder + spec + one pinning test, confirmation-lane authorized). Read-only throughout except this file. This confirmation supersedes nothing by deletion: `ariadne_contract_review-db1bc62.md` (REVISE_0_P0_0_P1_0_P2_1_P3 plus scoped delta-PASS on the per-severity repair) stands as history. Scope: the per-severity `closed_severities` rule as twice amended since the delta-PASS (suffix right-boundary, vacuous-continuation content requirement), the carried/completion rules on live data, and the PGP-armour disposition. The delta-PASS scope was affected by both amendments, so it is re-verified here in full rather than carried.

## What I verified (independently, live at c703bd5)

- **Suite**: `python3 -m unittest discover -s bench/review/tests -t .` → 53 tests, OK (52 prior + `test_a_vacuous_continuation_exempts_nothing`).
- **Live register**: `build_finding_register.py --check` → 291 obligations, open_reviews 50, result FINDING_REGISTER_OPEN; `check_finding_register.py` → result PASS. `unclaimed_carried_findings` 40 rows, `mid_string_complete_packages` 10 sections.
- **My P3 repair holds as amended**: the coarse per-row CLOSED exemption is now per-severity `closed_severities` (`build_finding_register.py:190-208`). Live check of the exact row my P3 named — s20_700 fuzz-slice `PASS_PRIOR_P2_CLOSED_NO_OPEN_P0_P1_P2_WITH_P3_P4_FOLLOWUPS` — under the current code: severities [P2, P3, P4], closed [P2] only, unclaimed [P3, P4], blocks CLEAR. The carried P3/P4 followups no longer escape behind a P2 closure.
- **Per-severity scope is exact**: `PASS_WITH_P1_CLOSED_NO_NEW_P1` closes P1; `PASS_WITH_P1_CLOSED_NO_NEW_P2` closes only P1 (P2 stays unclaimed); `PASS_P1_CLOSED_P2` closes nothing (fail-closed on ambiguous adjacency); `PASS_P1_PRIOR_CLOSED` closes P1 (documented lane word).
- **Suffix amendment re-verified** (affected my scope): `PASS_WITH_P1_CLOSED_LOOP`, `..._CLOSEDNESS`, `..._CLOSED_CIRCUIT` all yield closed [] → [P1] unclaimed → OPEN. The pinning test passes.
- **Vacuous-continuation amendment verified** (new in this scope): `PASS_WITH_P1_CLOSED_NO` and `PASS_WITH_P1_CLOSED_WITH` yield closed [] → [P1] unclaimed → OPEN. All 15+ live `..._CLOSED...` dispositions use content-carrying continuations (`_NO_NEW_...`, `_NO_OPEN_...`, `_WITH_P3_P4_FOLLOWUPS`) and still close exactly as before — the amendment breaks zero live rows (register counts unchanged: 291/50 before and after).
- **PGP-armour disposition retained**: `SECRET_PATTERNS` holds 21 names, no PGP entry; the rationale stands in-code (`generate_supply_chain_evidence.py:49-53` — header-only shape self-fires on the repo's own audit verdicts, findings have no disposition path, revisit needs a finding-disposition queue). No pattern added or removed since the delta; the live T54 scan is PASS.

## Fresh evasions attempted (same lane, current code)

- `PASS_WITH_P1_CLOSED_NO_NEW` (full legitimate form) → closed [P1]. Control, still the defined closure vocabulary.
- Lowercase `pass_with_p1_closed` → severities [], invisible. REASONED, not filed: consistent with the prior lowercase/homoglyph treatment (machine-written UPPER_SNAKE vocabulary, ASCII-token contract, zero live instances). A future lowercase disposition vocabulary would be a contract change, not a repair.
- No other continuation keyword after `_CLOSED` was found to exempt: the regex admits exactly end-of-string or `NO_`/`WITH_` followed by content.

## Findings

None open. The one P3 of record (coarse CLOSED exemption) is repaired, twice amended against evasion, and verified closed on live data including its named row. The vacuous-continuation shape was found and repaired inside this confirmation's scope and is verified blocked with a pinning test.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_0_P4 / SECTION: finding_register / FIELD: ariadne_contract_review / SCOPE_SHA: c703bd5c5c2aeb34f0d52d778a90c738287ab190 / FINDINGS: none open (P3 coarse-CLOSED-exemption repaired+verified; suffix + vacuous amendments verified; PGP disposition retained)
