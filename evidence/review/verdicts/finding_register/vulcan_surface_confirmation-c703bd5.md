# Vulcan confirmation — finding register post-scope repairs including latest P3

Baseline verified: `git rev-parse HEAD` = `c703bd5c5c2aeb34f0d52d778a90c738287ab190`, tree clean except this verdict file. Read-only throughout except this file. History stands: `vulcan_surface_review-a4b6029.md`, `vulcan_surface_review-cb841a6.md`, `vulcan_surface_review-db1bc62.md` (REVISE_0_P0_0_P1_0_P2_2_P3 + evasion-delta REVISE_0_P0_0_P1_0_P2_1_P3). Scope: all three recorded P3 bypasses against the current rev-4 builder as amended, plus fresh evasion attempts.

## What I verified (independently, live at c703bd5)

- **Suite**: 53 review tests OK. **Live**: `--check` 291 obligations / 50 open / FINDING_REGISTER_OPEN; checker PASS. Trick scan over all 291 live dispositions: zero instances of DISCLOSED/UNCLOSED/PRECLOSED/NO_NO/CLOSED_LOOP/CLOSEDNESS/CLOSED_CIRCUIT/vacuous-CLOSED. Gap class remains fail-open shape, never silent clearance.
- **P3-1 (CLOSED-substring) repaired**: `PASS_WITH_P1_DISCLOSED`, `PASS_WITH_P1_UNCLOSED` → severities [P1], closed [] → unclaimed [P1] → OPEN (single-obligation probe classifies PASS-with-mentions; the unclaimed rule blocks CLEAR — confirmed via `test_a_closed_substring_smuggles_nothing`, passing).
- **P3-2 (stacked negation) repaired**: `PASS_NO_NO_OPEN_P1` → strip leaves text intact → [P1] unclaimed → OPEN; triple-stacked `PASS_NO_NO_NO_OPEN_P1` and mixed `PASS_NO_OPEN_P0_NO_NO_OPEN_P1` also unclaimed → OPEN (`test_a_stacked_negation_declares_nothing`, passing).
- **P3-3 (CLOSED suffix salad, my delta) repaired**: `PASS_WITH_P1_CLOSED_LOOP`, `..._CLOSEDNESS`, `..._CLOSED_CIRCUIT` → closed [] → [P1] unclaimed → OPEN (`test_a_closed_suffix_salad_exempts_nothing`, passing).

## Fresh evasions against the current rule (executed, in-memory probes)

- EXECUTED, BLOCKED — `PASS_WITH_P1_CLOSED_NO`: vacuous trailing NO exempts nothing (closed [] → [P1] unclaimed → OPEN). New shape, repaired inside this confirmation (`c703bd5`: continuation must carry content `_(NO|WITH)_[A-Z0-9]`), pinned by `test_a_vacuous_continuation_exempts_nothing`, 53/53 green.
- EXECUTED, BLOCKED — `PASS_WITH_P1_CLOSED_WITH`: same repair, same outcome.
- EXECUTED, CONFIRMED LEGITIMATE — `PASS_WITH_P1_CLOSED_NO_NEW_P1`, `PASS_WITH_P1_CLOSED_WITH_P2_FOLLOWUP` (closes P1, P2 followup stays unclaimed), `PASS_P1_PRIOR_CLOSED`: defined vocabulary, per-severity exact.
- EXECUTED, FAIL-CLOSED — `PASS_P1_CLOSED_P2` (ambiguous adjacency closes nothing), `PASS_WITH_P1_PRECLOSED` (no anchor → OPEN).
- REASONED, not filed — lowercase `pass_with_p1_closed` (severities []): same documented treatment as the prior lowercase/homoglyph items (UPPER_SNAKE machine vocabulary, ASCII-token contract, zero live instances).

## Findings

One new P3 found in this confirmation (vacuous `_CLOSED_NO`/`_CLOSED_WITH` continuation), repaired and pinned inside the confirmation scope and verified blocked with zero live impact (291/50 unchanged). All three recorded P3s re-verified repaired. No live instance of any bypass class.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_0_P4 / SECTION: finding_register / FIELD: vulcan_surface_review / SCOPE_SHA: c703bd5c5c2aeb34f0d52d778a90c738287ab190 / FINDINGS: none open (P3 substring + P3 stacked-negation + P3 suffix-salad re-verified repaired; new P3 vacuous-continuation repaired+pinned+verified in-scope)
