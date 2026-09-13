Baseline verified: `git rev-parse HEAD` = `db1bc623d01e838d49c153feb0be05a7502b8794`. Read-only except this verdict file. The rev-4 package under review is the uncommitted working-tree diff against that HEAD (spec +35, builder +118, checker +17, tests +76, register, summary). This re-review supersedes `vulcan_surface_review-a4b6029.md`.

## What I verified (independently, plus script outputs)

- **Scripts**: `python3 scripts/build_finding_register.py --check` → PASS, obligations 291, open_reviews 52, result FINDING_REGISTER_OPEN, exit 0. `python3 scripts/check_finding_register.py` → result PASS, problems [], exit 0. `python3 -m unittest discover -s bench/review/tests -t .` → 48 tests, OK.
- **Live artifact**: contract_revision 4 (register and summary agree), states HISTORICAL_ROUND=53/OTHER=3/PASS=183/PENDING=52, severity_mentions P0=57/P1=101/P2=113/P3=131/P4=21. `obligations_digest` and `register_digest` recompute exactly.
- **P3-1 repair CLOSED on live data**: `unclaimed_carried_findings` names 39 rows, including the entire prior P3-1 family — reproducibility nabu `PASS_WITH_P1_P3_P4_FOLLOWUPS_NO_P0_P2` (unclaimed P1/P3/P4) and ariadne ×2, mutation_value_profile vulcan, root_backed_query_profile ×3, s20_700 fuzz-slice rows. Carried findings can no longer survive silently into a CLEAR read.
- **P3-2 repair CLOSED on live data**: `mid_string_complete_packages` names 10 sections with open counts, including the three prior offenders — s20_360 (3), s20_390 (3), mutation_value_profile (1). Hyphen/suffix-adjacent variants are correctly partitioned: `S20_999-COMPLETE` satisfies the suffix test (violation path), `S20_999_COMPLETED` lands in the mid list.
- **The 4 new tests pin the rules**: the 4 mid/unclaimed InvariantTests pass in isolation; sabotaging `unclaimed_carried()` to `[]` in-memory flips `test_an_unclaimed_carried_finding_blocks_clearance` to FAIL (CLEAR instead of OPEN), and sabotaging `mid_string_complete()` to `[]` flips the mid-string test to FAIL. Both pins HOLD.
- **Checker shape checks fire**: the exact set comparisons at `check_finding_register.py:226-238` reject a carried entry missing `unclaimed_severities` and a mid entry missing `open_obligations` (demonstrated against the checker logic); `def unclaimed_carried(` / `def mid_string_complete(` script markers present.

## Evasion attempts (all executed against the rev-4 builder, in-memory only)

P3-1 (PASS carrying open severity, attempting to dodge `unclaimed_carried_findings` and every other CLEAR blocker):
1. EXECUTED, BLOCKED — novel WITH-wording `PASS_WITH_P1_FOLLOWUPS_NO_P0_P2`: severities [P1], no CLOSED, untracked → unclaimed, synthetic register reads OPEN. Repair holds.
2. EXECUTED, BLOCKED — zero-count-adjacent `PASS_1_P1` / `PASS_00_P1`: nonzero/`00` prefixes are not the `0` absence claim, severities [P1] → unclaimed, OPEN. (`PASS_0_P1` carries none, which is the defined absence semantics, not an evasion.)
3. EXECUTED, BYPASSES — double negation `PASS_NO_NO_OPEN_P1`: the NEGATION regex strips the inner `NO_OPEN_P1`, severities [] → not unclaimed, synthetic single-obligation register reads CLEAR. Finding 1 below.
4. EXECUTED, BYPASSES — CLOSED-substring smuggling `PASS_WITH_P1_DISCLOSED` (also `..._UNCLOSED`): `"CLOSED" in value` is a substring test, so a disposition that merely contains the letters CLOSED exempts itself from `unclaimed_carried_findings`; severities [P1] stay visible in `severity_mentions` but non-blocking, synthetic register reads CLEAR — end-to-end proof. A disposition saying findings were *disclosed* (still open) or *unclosed* (explicitly not closed) clears. Finding 2 below. No live disposition uses either trick (scanned all 291 obligations: no DISCLOSED/UNCLOSED/NO_NO instances; no bare-substring CLOSED).

P3-2 (status saying COMPLETE while escaping both `complete_packages` and `mid_string_complete_packages`):
1. EXECUTED — lowercase `S20_999_complete` escapes both lists (`"COMPLETE" in status` is case-sensitive). REASONED, not a finding: the status vocabulary is machine-written UPPER_SNAKE, no live non-upper status exists (scanned all summary sections), and a lowercase status would violate every other status consumer; no live instance, no realistic path.
2. EXECUTED, CAUGHT — `S20_999_COMPLETED` and `COMPLETE_RESTRICTED_X` both contain `COMPLETE` without satisfying the suffix test → correctly land in the mid list, never in `complete_packages`.
3. REASONED, not executed as a live threat — unicode homoglyph (`COMPLΕTE` with U+0395) escapes both lists, but homoglyph injection into tracked evidence is outside the ASCII-token contract and has no live instance.

## Remaining live issues (2 × P3, both demonstrated end-to-end to CLEAR, neither with live instances)

1. The carried-findings exemption is a substring test: any future `PASS` disposition containing the letters CLOSED anywhere (DISCLOSED, UNCLOSED, ENCLOSED) silently opts out of `unclaimed_carried_findings` while still naming severities, and the register can read CLEAR. One-line fix: token-anchor the CLOSED test the way SKIP_FIELD already anchors its suffixes.
2. Stacked negations strip to nothing: `PASS_NO_NO_OPEN_P1` (double negation = semantically open P1) yields zero severities, invisible even in `severity_mentions`, and the register can read CLEAR. Fix alongside finding 1: anchor negation matching to a single leading group or reject stacked `NO_` prefixes as OTHER.

VERDICT: REVISE_0_P0_0_P1_0_P2_2_P3
SECTION: finding_register
FIELD: vulcan_surface_review
SCOPE_SHA: db1bc623d01e838d49c153feb0be05a7502b8794
FINDINGS:
P3 [contract] scripts/build_finding_register.py:337,262 (`"CLOSED" in value`) - CLOSED-substring smuggling bypasses the rev-4 carried-findings blocker: `PASS_WITH_P1_DISCLOSED` classifies PASS with severities [P1] yet `declares_closed_findings` is true by substring, so it never lands in `unclaimed_carried_findings`; a synthetic single-obligation summary reads FINDING_REGISTER_CLEAR end-to-end (executed). Same for `..._UNCLOSED`, which literally means not closed. No live instance among the 291 obligations (scanned), so the gap is fail-open shape, not silent clearance.
P3 [contract] scripts/build_finding_register.py:56,170 (`NEGATION = re.compile(r"NO(_NEW)?(_OPEN)?_(P[0-4]_?)+")`) - stacked-negation bypasses the rev-4 carried-findings blocker: `PASS_NO_NO_OPEN_P1` strips the inner `NO_OPEN_P1` group, `severities_of()` returns [], so the row is neither unclaimed nor visible in `severity_mentions`; a synthetic single-obligation summary reads FINDING_REGISTER_CLEAR end-to-end (executed). No live instance among the 291 obligations (scanned: no `NO_NO` dispositions), so the gap is fail-open shape, not silent clearance.

---
## RE-REVIEW 2026-09-13 (evasion-delta; original verdict above untouched)

Baseline: `git rev-parse HEAD` = `db1bc623d01e838d49c153feb0be05a7502b8794`, repair in working tree, read-only except this file. Suite: `python3 -m unittest discover -s bench/review/tests -t .` → 51 tests, OK. Live: `build_finding_register.py --check` → PASS, 291 obligations, result FINDING_REGISTER_OPEN (expected: open reviews pending).

### 1. Prior bypasses re-executed (synthetic single-obligation summaries, `example_package.vulcan_review`, zeroed counters)
- EXECUTED, BLOCKED — `PASS_WITH_P1_DISCLOSED`: `severities_of`=[P1], `closed_severities`={} → unclaimed [P1], severity_mentions P1=1, result FINDING_REGISTER_OPEN.
- EXECUTED, BLOCKED — `PASS_WITH_P1_UNCLOSED`: same shape → [P1] unclaimed, OPEN.
- EXECUTED, BLOCKED — `PASS_NO_NO_OPEN_P1`: `strip_negations` leaves text intact, `severities_of`=[P1], `negated_severities`={} → [P1] unclaimed, visible in severity_mentions, OPEN. Both prior P3s are repaired; the 2 pinning tests (`test_a_stacked_negation_declares_nothing`, `test_a_closed_substring_smuggles_nothing`) pass.

### 2. Fresh evasions against the new rule (executed, same harness)
- EXECUTED, BYPASSES — `PASS_WITH_P1_CLOSED_LOOP`: `closed_severities`=[P1] (regex `P1(?:_(?:PRIOR|P[0-4]))*_CLOSED` prefix-matches `_CLOSED` inside `_CLOSED_LOOP`), severities [P1] fully "closed" → unclaimed [], result FINDING_REGISTER_CLEAR end-to-end. Anchor present, semantics unrelated (feedback-loop engineering term, not a finding closure). New finding 1.
- EXECUTED, BYPASSES — `PASS_WITH_P1_CLOSEDNESS`: same root cause (`_CLOSED` prefix of `_CLOSEDNESS`) → CLEAR. Same finding.
- EXECUTED, BYPASSES — `PASS_WITH_P1_CLOSED_CIRCUIT`: `_CLOSED` followed by ` _CIRCUIT` still matches (no right boundary) → CLEAR. Same finding.
- EXECUTED, BLOCKED — triple-stacked `PASS_NO_NO_NO_OPEN_P1`: inner group still `NO_`-prefixed → void → [P1] unclaimed, OPEN. Confirmation.
- EXECUTED, BLOCKED — mixed `PASS_NO_OPEN_P0_NO_NO_OPEN_P1`: valid group strips P0 (`negated`=[P0]), void group leaves P1 → [P1] unclaimed, OPEN. Confirmation.
- EXECUTED, BLOCKED — `PASS_00_P1`: `00`≠`0` absence claim → [P1] unclaimed, OPEN; `PASS_WITH_P1_PRECLOSED` (no `_CLOSED` anchor) → OPEN; `PASS_P1_CLOSED_P2` correctly closes only P1, P2 unclaimed → OPEN. Confirmations.
- Live scan: 0 instances of `CLOSED_LOOP`/`CLOSEDNESS`/`CLOSED_CIRCUIT`/`_CLOSED[A-Z0-9]`/`DISCLOSED`/`UNCLOSED`/`NO_NO` among summary strings; the 15 live `P_CLOSED_…` hits are legitimate `…_CLOSED_NO_NEW_…` closures. Gap is fail-open shape, not silent clearance.

### 3. Delta judgment
New findings (delta only): 1 × P3.
1. P3 [contract] scripts/build_finding_register.py:201 (`re.search(rf"{severity}(?:_(?:PRIOR|P[0-4]))*_CLOSED", disposition)`) — the `_CLOSED` anchor has no right word-boundary, so `_CLOSED` prefix-matches longer words and trailing phrases: `PASS_WITH_P1_CLOSED_LOOP`, `PASS_WITH_P1_CLOSEDNESS`, `PASS_WITH_P1_CLOSED_CIRCUIT` all classify PASS with severities [P1] yet claim P1 closed, unclaimed [], synthetic single-obligation register reads FINDING_REGISTER_CLEAR end-to-end (executed, three variants, one root cause). Fix: right-anchor the closure (e.g. require `_CLOSED` at end or followed by `_`+`NO_`/end, the way the left side is anchored), or allow only the known trailing negation vocabulary after `_CLOSED`.

RE-REVIEW: REVISE_0_P0_0_P1_0_P2_1_P3 / SCOPE: evasion-delta
