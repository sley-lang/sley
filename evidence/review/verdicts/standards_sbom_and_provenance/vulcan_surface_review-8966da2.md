<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 8966da2e0a52a6f6e56949e9a5902464c86613c0; role: vulcan; field: vulcan_surface_review; dispatched: 2026-09-23T03:06:06Z; duration_s: 476; process_exit_code: 0 -->
# Vulcan Council review — standards_sbom_and_provenance

Harness: claude-code
Reviewed checkpoint: 8966da2e0a52a6f6e56949e9a5902464c86613c0

What I verified myself. The review was read-only: nothing was written to the tree. Every probe ran in-process, either on the loaded `machine-summary.json` or in a tempfile git fixture built from `RetireReviewClaimsTests.setUp`.

- `git rev-parse HEAD` returned `8966da2e0a52a6f6e56949e9a5902464c86613c0`, so the scope matches. `git log --format='%h %s' 8f774d0c..HEAD` lists `69907ddf` (fix) and `8966da2e` (records-only mint).
- I ran `git diff --stat 8f774d0c..HEAD` (33 files). I read the full `git diff 8f774d0c..HEAD -- scripts/ bench/ docs/`: `build_finding_register.py` +54/−, `retire_review_claims.py` +49/−, `FINDING_REGISTER_V1.md` +15/−, `test_retire_review_claims.py` +13/−. I also read the diffs of `evidence/security/T54/secret-scan.json`, `evidence/release/provenance.json`, `evidence/release/sbom/*`, `evidence/release/reproducibility-report.json`, `evidence/build/lint-report.json` and `evidence/review/claim-retirements.json` (+70).
- `generate_supply_chain_evidence.py`, `check_supply_chain_audit.py`, `build_standards_sbom.py`, `build_release_provenance.py`, `check_standards_sbom_and_provenance.py`, `Cargo.lock` and `oracle/scb1/uv.lock` are absent from the delta stat.
- Checkers:
  - `python3 scripts/check_supply_chain_audit.py`: exit 0, `"result": "DEFERRED"`, `t52_local_lock_inventory: PASS`, `t54_high_confidence_scan: PASS`, `release_sbom: false`. DEFERRED is by design.
  - `python3 scripts/generate_supply_chain_evidence.py --check`: exit 0, `{"outputs": 2, "result": "PASS"}`.
  - `python3 scripts/check_finding_register.py`: exit 0, `"problems": []`, `"result": "PASS"`.
  - `python3 scripts/retire_review_claims.py --check`: exit 0, `stale_closures: []`, `regeneration_divergence: []`, `"result": "PASS"`.
  - `python3 scripts/check_standards_sbom_and_provenance.py`: exit 0, `"result": "PASS"`, `s20_710_audit_complete: false`.
  - `python3 -m unittest bench.review.tests.test_finding_register bench.review.tests.test_retire_review_claims`: `Ran 58 tests … OK`.
- T54: I recomputed the candidate manifest from git objects (`ls-tree -r` + `cat-file --batch`, `OUTPUT_PATHS` excluded, the same length/path/sha256 framing as `generate_supply_chain_evidence.py:335-357`):
  - HEAD: `9517f9bf…839a2`, 2172 files, 60565079 bytes. This equals the recorded T54.
  - 8f774d0c: `8ff9e846…`, 2163 files, 60235526 bytes. This equals the prior record.
- Provenance: every named file digest in `evidence/release/provenance.json` matches the sha256 of the file on disk. That covers Cargo.lock, uv.lock, T52 inventory, both SBOMs, the reproducibility report and the conformance report. The subject is `ca7317a9…`, the commit binds `69907ddf904a…` and the manifest is `3433cb37…`. I did not hash the tarball itself.
- Files read:
  - `evidence/review/verdicts/standards_sbom_and_provenance/vulcan_surface_review-8f774d0.md` (findings at lines 30-50 and 73-83).
  - `scripts/build_finding_register.py`: 602-623, 799-844, 1027-1060, 1092-1121, 1296-1365.
  - `scripts/retire_review_claims.py`: 205-235, 345-445.
  - `bench/review/tests/test_retire_review_claims.py`: 1-80, 225-275, 313-322.
  - `docs/spec/FINDING_REGISTER_V1.md`: 106-133.
  - `scripts/generate_supply_chain_evidence.py`: 290-370, 440-512.
- Probes (results cited under Findings): I audited all 91 live `p*_restated_claims` against their roots. I also ran synthetic fold and retire probes in the fixture and checked generic-head exposure over the live open groups.

## Evidence checked

**Status of each finding from my 8f774d0c verdict:**

- **P2 `same_finding` identifier wildcard / `is_carry` prose words (8f774d0c line 30)**: OPEN, narrowed.
  - Closed legs:
    - `is_carry` (`build_finding_register.py:1355-1365`) now needs `CARRIED_FROM` or `LEADING_CARRY`, so `again`/`unchanged`/`prior` in prose no longer marks a claim as a carry.
    - `same_finding` (`:1346-1353`) no longer treats `""` as a wildcard.
    - The test at `test_retire_review_claims.py:266-274` pins the prose-word shape and the mixed identifier/identifier-less shape.
    - The 17 wrong folds are undone. `s20_700_remaining_surface_audit` has 0 restated entries. The `reproducibility_and_independent_conformance` re-statements are now only the `[note]`/`[record-accuracy]` chains. This section's `:586` `[record]` claim (`vulcan_surface_review@7622776`) is open again.
  - Still open: the re-statement side is still a wildcard, and the new fold guard uses the weak read. See Finding 1.
- **P3 `raising_severity` fails open (8f774d0c line 32)**: OPEN. `build_finding_register.py:602-623` is outside the delta, and no test names `raising_severity`.
- **P4 `shared_vocabulary` computed within one severity (line 34)**: OPEN. It still reads only `p{number}_*` for the claim's own severity (`:809-815`). The delta changed only the skip rule at `:833`.
- **P4 commit-id-in-transcript-filename relation for stop-word kinds (line 36)**: OPEN. The commit rule, now at `:1092-1093`, is untouched.
- **P4 `is_tracked` / `scope_generation` tolerate git failure (line 38)**: OPEN. `:1151-1162` and `:546-553` are untouched.
- **P4 `_quoted` / `STATUS_OPEN` / cell lookahead contract drift (line 40)**: OPEN. `:679` and `:685-692` are untouched.
- **P4 `[record] claim-retirements.json` (line 42)**: OPEN.
  - Lines 184 and 195 still say "line 24" for the `:4704` prefix entries.
  - The five new exact-claim entries added in this delta repeat the boilerplate per-claim `reason` ("names it by kind and file in its head; the kind is shared within the lane, so the binding is exact"). They add an outer "verified line by line 2026-09-19", which states no ground.
- **P4 `regeneration_divergence` keying (line 44)**: OPEN, narrowed.
  - Closed leg: `reopen_all` (`retire_review_claims.py:407-425`) now also returns re-statements to the open list, so the fold set is re-derived.
  - Still open: the key is still `(claim, verified_by.split("#")[0])` (`:441-442`), so a closure re-cited to `#L1` still passes.
  - New stale text: the docstring at `:429-431` still says "reopen → fold → retire" against the new order at `:434-436`.
- **P4 hop-chained `carried from` keys per hop (line 46)**: OPEN. Both `vulcan_surface_review@6589c6e: [record] … carried from 1a9f0aab` and `vulcan_surface_review@79fdcc6: [record] … carried from 6589c6ec` are still separate entries in this section's `p4_open`.
- **P4 `claim_relation_problem` scope-less claims (line 48)**: OPEN. `:906-934` is untouched.
- **P4 line-span rule admits `42,` (line 50)**: OPEN. `:1113-1121` is untouched.

**Live re-statement audit.** I checked all 91 `p*_restated_claims` in `machine-summary.json`. Every one names a `carried from <sha>`.

- In 22 folds, the named sha is not the root's raising scope. Some of these are legitimate multi-hop chains; for example, `_quoted` at 8f774d0 is carried from b58ac1e and folds into the 1a9f0aa `_quoted` root with the same identifier.
- In 20 folds, the re-statement's own first identifier differs from the root's identifier, while a claim in the same list carries that identifier. In 10 of those 20, that matching original is **open**. Examples:
  - This section: `…_revision_11@8f774d0: [fail-closed-gap] …:897-925 (claim_relation_problem …)` is recorded as re-stating `vulcan_surface_review@6589c6e: [fail-closed-gap] …:811-826 (line_speaks_about …)`. Its own original, `…_revision_10@b58ac1e: … claim_relation_problem`, is open in the same list.
  - This section: the `:1104-1112` line-span claim is recorded the same way, against the same `line_speaks_about` root.
  - `release_candidate_packaging`: `scoped_before` ×2, `is_lane_field_name` ×2 and `claim_finding_ids` fold into the `is_closure_line` root; `is_open_line`, `raising_scope` and `p4_open` fold into the `line_speaks_about` root.

## Findings

[P2] [fail-closed-gap] scripts/build_finding_register.py:1300 (`LEADING_CARRY` accepts any leading `(carried…)`/`(prior…)`/`(residual…)` parenthetical, sha or not), :1330 and :1342 (a re-statement keys `None` even when it names its own backticked identifier), :1346-1353 (`None` matches any identifier), scripts/retire_review_claims.py:373-378 (fold root = earliest same-(lane, kind, file) claim by scope, ignoring the round the re-statement names), :388 (guard `line_speaks_about(text[n - 1], entry)` uses the weak read: no `strong`, no `shared`), docs/spec/FINDING_REGISTER_V1.md:121-132 - The prior P2 is carried from 8f774d0c, narrowed.
- **Fixture probe:** root `[fail-closed-gap] scripts/x.py:10 - the \`alpha_beta\` guard is open` and re-statement `[fail-closed-gap] (carried from <root sha>, OPEN) scripts/x.py:90 - the \`gamma_delta\` bound is missing`. A later transcript closes only the root: `- **[P4] [fail-closed-gap] \`alpha_beta\` in scripts/x.py — CLOSED.**`.
  - `retire` correctly refuses the re-statement under the strong read (retired 1).
  - `fold_restatements` then folds it into the retired root (folded 1, restates=`alpha_beta`), because the kind phrase `fail-closed-gap` satisfies the weak guard. A distinct open finding leaves the ledger with no closure line that names it.
  - `(carried from 0000000, OPEN)`, `(carried, OPEN)` and `(prior art)` all key `None`, and each folds into the unrelated `alpha_beta` original.
- **Live projection:** a head `- **[P4] [fail-closed-gap] scripts/build_finding_register.py \`line_speaks_about\` — CLOSED.**` closing this section's open `vulcan_surface_review@6589c6e` root would pass the weak guard for all five re-statements folded under it (strong read: False for each). That includes the whole `shared_vocabulary` chain (`@79fdcc6`, `revision_10@b58ac1e`, `revision_11@8f774d0`), which has no other open claim, so my one-severity P4 would disappear silently.
- **Live records:** 10 re-statements are recorded against a root with a different identifier while their own original is open (listed above).
- Both checkers pass because `package_restated_claims` and `regeneration_divergence` reproduce the same rule as a fixed point. No test pins a re-statement whose identifier or named round differs from the root.
- **Closure evidence needed:**
  - Key a re-statement on its own identifier when it names one.
  - Bind the fold root to the claim of the named `carried from <sha>` round, resolved transitively, and require that sha in `LEADING_CARRY`.
  - Apply the strong read with the section's shared vocabulary in the fold guard at `retire_review_claims.py:388`.
  - Re-derive the ledger.
  - Pin in `test_retire_review_claims` two cases: a distinct carried finding beside a closed root → 0 folded; a re-statement naming a different identifier → 0 folded.

[P3] [fail-closed-gap] scripts/build_finding_register.py:1330,1342 (an original without a backticked identifier keys `""`), :833 (`other_key == own_key` makes `shared_vocabulary` skip it), docs/spec/FINDING_REGISTER_V1.md:126-129 - Fresh (a residue of the prior P2's shared-vocabulary leg). Two **distinct** identifier-less originals of one lane, kind and file get equal keys. `shared_vocabulary` therefore treats each as "this finding", the kind is not shared, and one generic head `- **[P3] [robustness] scripts/build_finding_register.py — CLOSED.**` retires both (fixture: retired 2, `p3_open` []). This contradicts the delta's stated repair "two distinct originals … keep the strong read". The test at `test_retire_review_claims.py:266-274` pins only the mixed identifier/identifier-less pair. Live exposure: I found 4 lane/severity/kind/file groups where a generic head relates to ≥2 open identifier-less claims (`complete_entity_impact_profile` P4 `REQ-01/02/03`; `s20_700_schema_persistent_fuzz_slice` P3; `s20_700_query_persistent_fuzz_slice` P3; `reproducibility_and_independent_conformance` P4 `[closure-grammar]`). Each reads as the same finding re-stated, so there is no live loss today. - Closure evidence needed: treat `""` as "no identity" rather than equal, so identifier-less originals count as other findings unless the line anchor or `carried from` binds them; then pin two identifier-less distinct originals → 0 retired by the generic head.

[P3] [fail-closed-gap] scripts/build_finding_register.py:602-623 (`raising_severity` returns `None` on no match and for every untagged claim), with the binding skipped on `None` in `package_*_claims` - Carried from 8f774d0c unchanged (outside this delta). A tagged claim whose raising transcript has no matching `[Pn]` line is accepted in any severity list, and no unit test names `raising_severity`. - Closure evidence needed: refuse such a claim as `SUMMARY_INVALID`, bind untagged claims through `raising_scope`'s transcript, and pin the whitespace bypass and the closed-list transplant.

[P4] [fail-closed-gap] scripts/build_finding_register.py:799-833 (`shared_vocabulary` reads only the claim's own severity, :809-815) - Carried from 8f774d0c unchanged. `rw090 ariadne_contract_review-db53894.md#L17` (a P4 closure line) still speaks about both `[test-coverage/oracle-parity]` P2 claims; only the severity binding separates them. - Closure evidence needed: compute shared vocabulary across every severity of the lane and pin the rw090 probe.

[P4] [fail-closed-gap] scripts/build_finding_register.py:1092-1093 (commit-id rule) with the `record` stop word - Carried from 8f774d0c unchanged. A transcript filename `<lane>-<sha7>.md` in a `[record]` head satisfies both halves of the commit rule, so one head relates to both of my open `[record]` P4s (`@6589c6e`, `@79fdcc6`). - Closure evidence needed: exclude commit ids embedded in a path token, and pin the filename head as non-relating.

[P4] [fail-closed-tolerance] scripts/build_finding_register.py:1151-1162 (`is_tracked` with `_tracked = None`), :546-553 (`scope_generation` → `-1`) - Carried from 8f774d0c unchanged. When git fails, every path is treated as tracked and a `-1` generation sorts first. - Closure evidence needed: fail closed, surfacing git's error, and pin a failing-git test.

[P4] [contract-drift] scripts/build_finding_register.py:679 (`STATUS_OPEN` accepts `Open`), :685-692 (`_quoted`), the `_own_status` cell lookahead `(?![^)]*OPEN)` - Carried from 8f774d0c unchanged. The nested-parenthesis and lowercase-`open` shapes are still read as closures. - Closure evidence needed: track parenthesis depth, match `open` case-insensitively, and add the shapes to `test_revision_7_closure_line_shapes`.

[P4] [record] evidence/review/claim-retirements.json:184,195 ("line 24") and the five exact-claim entries added in this delta - Carried from 8f774d0c unchanged. The dead "line 24" references remain, most prefix entries lack curated `lines`, and the new entries repeat the boilerplate `reason`, adding only "verified line by line 2026-09-19". - Closure evidence needed: correct or drop the dead entries, add `lines`, and state each binding's real ground.

[P4] [contract-precision] scripts/retire_review_claims.py:428-445 (`regeneration_divergence` keys `(claim, verified_by.split("#")[0])`; its docstring at :429-431 still says "reopen → fold → retire") - Carried from 8f774d0c, narrowed. The `reopen_all` leg is closed: re-statements now return to the open list (:407-425). A `#L1` re-cite still passes, and the docstring contradicts the new order at :434-436. - Closure evidence needed: key on `verified_by` up to the note, correct the docstring, and pin the re-cite tamper.

[P4] [contract-precision] scripts/build_finding_register.py:1299 (`CARRIED_FROM`), :1338-1339 (ledger origin = first named sha) - Carried from 8f774d0c unchanged. My `@6589c6e` and `@79fdcc6` `[record]` claims are still two open entries for one hop chain. - Closure evidence needed: resolve `carried from` transitively, and pin a two-hop chain → 1 open.

[P4] [fail-closed-gap] scripts/build_finding_register.py:906-934 (`claim_relation_problem` applies no scope rule when `raising_scope` is `None`) - Carried from 8f774d0c unchanged. A hand-listed closure or an exact-claim binding for a scope-less claim passes on identity alone. - Closure evidence needed: refuse such closures unless bound by exact claim, apply the own-round rule by the cited transcript's stamp, and pin a scope-less P1 closure as `SUMMARY_INVALID`.

[P4] [fail-closed-gap] scripts/build_finding_register.py:1113-1121 (line-span rule; `PATH_TOKEN` span admits a trailing separator) - Carried from 8f774d0c unchanged. `x.md:42,` still yields the span `42,`. - Closure evidence needed: strip trailing separators, require non-trivial spans, and pin `42,` as non-relating.

## Assessment

The section's own evidence checks out at this records-only mint:
- T54 `9517f9bf…` (2172 files, 60565079 bytes) equals the HEAD tree recomputed from git objects.
- T52 regenerates identically.
- Every provenance dependency and byproduct digest matches the file on disk, and the SBOMs, reproducibility report and lint report bind `69907ddf`.
- The audit (DEFERRED by design), evidence `--check`, register, `retire_review_claims.py --check`, the section checker and 58 unit tests pass.
- I did not hash the tarball.

The 69907ddf repair closes the prose-word leg of my P2. `is_carry` now needs a named carry, the 17 wrong folds are undone (checked in `s20_700_remaining_surface_audit`, `reproducibility_and_independent_conformance` and this section's `:586` claim), and the test pins the prose shapes.

The repair does not close the mechanism:
- A re-statement still keys with a wildcard identifier.
- The fold root ignores the round the re-statement names.
- The new "root's closing line names the re-statement too" guard uses the weak read, where the kind phrase alone passes. A closure the strong read refuses is therefore granted by the fold that runs after it. I demonstrated this in the fixture, and a live projection shows the `shared_vocabulary` chain would vanish when the `line_speaks_about` root closes.

Separately, identifier-less distinct originals still share an equal key and escape the strong read (P3; synthetic, no live distinct pair). Both checkers pass because the regeneration is a fixed point of the same rule. The P3 and eight P4s carried from 8f774d0c remain open, two of the P4s narrowed. No GA or release-readiness claim is made.

VERDICT: REVISE_0_P0_0_P1_1_P2_2_P3_9_P4
SECTION: standards_sbom_and_provenance
FIELD: vulcan_surface_review
SCOPE_SHA: 8966da2e0a52a6f6e56949e9a5902464c86613c0
FINDINGS: [P2] [fail-closed-gap] scripts/build_finding_register.py:1300,1330,1342,1346-1353 with scripts/retire_review_claims.py:373-378,388 - re-statement keys None (wildcard even with its own identifier), fold root ignores the named round, fold guard uses the weak read, so a distinct carried finding folds into an unrelated retired root (fixture: retired 1 then folded 1; live: 10 folds against a different-identifier root while their own original is open; the shared_vocabulary chain would vanish on a line_speaks_about closure) - bind root to named round and own identifier, strong-read guard, re-derive, pin; [P3] [fail-closed-gap] scripts/build_finding_register.py:1330,1342,833 - identifier-less distinct originals key equal ("" == "") so shared_vocabulary skips them and one generic head retires both (fixture: 2 retired) - treat "" as no identity, pin; [P3] [fail-closed-gap] scripts/build_finding_register.py:602-623 - raising_severity fails open (carried); [P4] [fail-closed-gap] scripts/build_finding_register.py:799-833 - one-severity shared vocabulary (carried); [P4] [fail-closed-gap] scripts/build_finding_register.py:1092-1093 - commit id in transcript filename relates [record] heads (carried); [P4] [fail-closed-tolerance] scripts/build_finding_register.py:1151-1162,546-553 - git failure tolerated (carried); [P4] [contract-drift] scripts/build_finding_register.py:679,685-692 - _quoted/Open shapes (carried); [P4] [record] evidence/review/claim-retirements.json:184,195 - dead "line 24" and boilerplate reasons (carried); [P4] [contract-precision] scripts/retire_review_claims.py:428-445 - divergence keying ignores #L, stale docstring (carried, narrowed); [P4] [contract-precision] scripts/build_finding_register.py:1299,1338-1339 - per-hop carried-from keying (carried); [P4] [fail-closed-gap] scripts/build_finding_register.py:906-934 - scope-less closure relation (carried); [P4] [fail-closed-gap] scripts/build_finding_register.py:1113-1121 - line-span `42,` (carried)
SUMMARY: The section's records are exact at 8966da2e: T54 `9517f9bf…` (2172 files) equals the HEAD tree recomputed from git objects, provenance and SBOM digests bind 69907ddf and match the files, and every checker plus 58 unit tests pass. The 69907ddf repair closes the prose-word leg of my 8f774d0c P2 and undoes the 17 wrong folds. The mechanism stays open (P2): a re-statement still keys with a wildcard identifier, its root ignores the round it names, and the new retired-root guard uses the weak read, so a closure the strong read refuses is granted by the fold (fixture-demonstrated, with 10 live wrong-root records). A new P3 covers identifier-less distinct originals sharing an equal key, and the P3 and eight P4s carried from 8f774d0c remain open.
