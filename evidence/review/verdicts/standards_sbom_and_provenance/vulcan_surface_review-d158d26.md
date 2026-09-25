<!-- engine: claude-code; observed model: claude-opus-5-5; scope: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e; role: vulcan; field: vulcan_surface_review; dispatched: 2026-09-23T05:46:05Z; duration_s: 674; process_exit_code: 0 -->
# Vulcan Council review — standards_sbom_and_provenance

Harness: claude-code
Reviewed checkpoint: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e

**Scope.** The review was read-only; nothing in the tree was written. `/tmp` (tmpfs) was at 100% inode use for the whole session (`df -i /tmp`: 1048576/1048576), so most Bash calls failed with ENOSPC on the harness's own output file. After the first few commands, I ran everything through the Monitor tool. Every Monitor run shows "exit 1" because the harness cannot write its cwd file. For those runs I report each script's printed result line and did not capture its own exit status.

**Git commands.**
- `git rev-parse HEAD` returned `d158d26bfdda9fc7dc899a1d1a8a141c2da6610e`. The worktree HEAD file (`.git/worktrees/wt/HEAD`) shows the same, so the scope matches.
- `git log --format='%h %s' 8966da2e..HEAD` lists `acbc65f0`, `883361e3`, `f7f9af90`, `5fd10849`, `0f4b8408`, `7c51710b`, `8d063f00` and `d158d26b`.
- I ran `git diff --stat 8966da2e..HEAD` over the section's owned paths. Only these changed:
  - `FINDING_REGISTER_V1.md` +98
  - `build_finding_register.py` +439/−
  - `retire_review_claims.py` +240/−
  - the rebound `evidence/release/*` and `evidence/security/{T15,T47,T54,T56,threat-coverage-report}` records
- `generate_supply_chain_evidence.py`, `check_supply_chain_audit.py`, `build_standards_sbom.py`, `build_release_provenance.py`, `check_standards_sbom_and_provenance.py`, `Cargo.lock` and `oracle/scb1/uv.lock` are absent from the delta.

**Checkers.**
- `python3 scripts/check_supply_chain_audit.py`: `"result": "DEFERRED"`, `t52_local_lock_inventory: PASS`, `t54_high_confidence_scan: PASS`, `release_sbom: false`. DEFERRED is by design.
- `python3 scripts/generate_supply_chain_evidence.py --check`: `{"outputs": 2, "result": "PASS"}`.
- `python3 scripts/check_finding_register.py`: `"problems": []`, `"result": "PASS"`.
- `python3 scripts/retire_review_claims.py --check`: `"retirable": 0`, `stale_closures: []`, `regeneration_divergence: []`, `split_status: []`, `"result": "PASS"`. `closer_disagreements` is non-empty, but it is report-only by contract.
- `python3 scripts/check_standards_sbom_and_provenance.py`: exit 1, `"result": "FAIL"`, `problems: ["closure:ineligible"]`, `attested_source_commit: 69907ddf…`. This is an environment artefact, not a tree defect:
  - The checker reads the gitignored `evidence/runtime/s20-720-release-candidate/evidence.json` (`check_standards_sbom_and_provenance.py:419-436`).
  - In this review worktree that file still records `69907ddf`/`ca7317a9…`.
  - The mint worktree `resume-20260918/wt2` records commit `8d063f00…` and artifact `ecf51188…`, which match the tracked records.
  - No other problem label was raised, so the release-tests leg did not fail.
- `python3 -m unittest discover -s bench/review/tests -t .` (with `tempfile.tempdir` set to `~/.cache`): `Ran 170 tests in 79.344s`, `OK`.

**Files read.**
- `evidence/review/verdicts/standards_sbom_and_provenance/vulcan_surface_review-8966da2.md` (1-130)
- `docs/spec/FINDING_REGISTER_V1.md` (1-20, 60-240)
- `scripts/build_finding_register.py` (540-1009, 1010-1452, 1453-1752)
- `scripts/retire_review_claims.py` (216-675)
- `bench/review/tests/test_retire_review_claims.py` (1-75, 318-417, 539-573, 620-793)
- `bench/review/tests/test_finding_register.py` (995-1054)
- `evidence/review/claim-retirements.json` (176-197, 1995-2170, plus a grep of every `reason`)
- `machineresearch/sley-2.0/machine-summary.json` (6855-6913, 8008, 8185)
- `evidence/release/provenance.json` (1-114)
- `scripts/generate_supply_chain_evidence.py` (300-369)
- `scripts/check_standards_sbom_and_provenance.py` (380-459)

## Evidence checked

**Section records at the mint.**
- **T54 recomputed from git objects** (`ls-tree -r` + `cat-file --batch`, `OUTPUT_PATHS` excluded, the length/path/sha256 framing of `generate_supply_chain_evidence.py:353-356`):
  - HEAD: `ecafe5362d76…a50`, 2187 files, 61034378 bytes. This equals the recorded `candidate_file_manifest_sha256`, `candidate_files_scanned` and `candidate_bytes_scanned`; `untracked_files_scanned` is 0.
  - 8966da2e: `9517f9bf…839a2`, 2172 files, 60565079 bytes. This equals my prior record.
- **Provenance digests:** every named `resolvedDependencies`/`byproducts` digest in `evidence/release/provenance.json` matches the sha256 of the file on disk. That covers `Cargo.lock`, `uv.lock`, the T52 inventory, both SBOMs, the reproducibility report and the conformance report.
- **Bindings:**
  - Subject: `ecf51188…`; commit: `8d063f00…` (`provenance.json:22,43,106`).
  - The reproducibility report binds `ecf51188…`/`8d063f00…` (`:5,7,28-29`).
  - Both SBOMs name `ecf51188`; CycloneDX also names `8d063f00`.
  - No tracked `evidence/release/*` file still names `69907ddf` or `ca7317a9`.
- I did not hash the tarball.

**Per-finding status of my 8966da2e verdict:**

- **P2 `LEADING_CARRY`/`None`-wildcard re-statement key, earliest-scope fold root and weak-read fold guard (8966da2 line 69)**: CLOSED. Every leg of the named mechanism is repaired and pinned:
  - `LEADING_CARRY` now requires `from <sha>` (`build_finding_register.py:1482`).
  - A re-statement keys on its own backticked identifier (`:1508-1527`), and `same_finding` is exact equality (`:1530-1537`).
  - The fold root is the one same-key claim at the named sha; ambiguous matches stay open (`retire_review_claims.py:434-445`).
  - The retired-root guard uses `speaking_lines` with the strong read, shared vocabulary, the OPEN refusal and postdating (`:397-425`).
  - `carried from` counts only when unquoted (`:1540-1546`).
  - My fixture is pinned in `test_the_standards_fixture_retires_the_root_and_folds_nothing` (`test_retire_review_claims.py:689-706`, all four clause shapes → retired 1, folded 0). The distinct-identifier case is pinned in `:674-687`.
  - Regeneration reproduces the ledger (`regeneration_divergence: []`).
  - A residue in the guard's vocabulary view is raised fresh below as the P3.
- **P3 identifier-less distinct originals key equal (`""`) (8966da2 line 84)**: CLOSED.
  - The `shared_vocabulary` exemption now needs a non-empty identifier on the equal key and a carry on either side (`build_finding_register.py:927-929`).
  - `package_open_findings` counts each identifier-less claim once (`:1728-1729`).
  - The case is pinned: "identifier-less originals" → 0 retired by the generic head (`test_retire_review_claims.py:640`), and the same for the identifier-less leading/trailing carry shapes (`:642-643`).
- **P3 `raising_severity` fails open (8966da2 line 86)**: OPEN, narrowed and re-rated P4 (see Findings).
  - Closed leg: a tagged claim whose raising round records no finding line is refused in the open, closed and re-stated lists (`tagged_claim_unraised`, `:675-685`, called at `:710`, `:1377` and `:1630`). This is pinned in `test_finding_register.py:1001-1028`.
  - Still open: untagged claims return `None` (`:657-659`) and skip the binding. Live: 295 untagged claims, 21 with no resolvable raising line and 0 real severity mismatches. My probe first reported two rw090 mismatches, but they came from its 80-character prefix fallback. The true raising lines agree with the lists: `vulcan_surface_review-92fa664.md:49` `[P4]`, and `ariadne_contract_review-92fa664.md:32` `[P3]`.
- **P4 `shared_vocabulary` reads one severity (line 88)**: OPEN. `:897-900` still reads only `p{number}_*`.
- **P4 commit id in transcript filename (line 90)**: OPEN. `:1239-1244` is unchanged.
  - A synthetic head `- **[P4] [record] evidence/review/verdicts/standards_sbom_and_provenance/vulcan_surface_review-1a9f0aa.md — CLOSED.**` relates to a `[record]` claim citing that path (True).
  - The live `@6589c6e`/`@79fdcc6` heads I tried do not relate (False).
- **P4 `is_tracked`/`scope_generation` tolerate git failure (line 92)**: OPEN. `:1317-1318` returns True on `OSError`, and `:552` still maps a failure to `-1`.
- **P4 `_quoted`/`STATUS_OPEN`/cell lookahead contract drift (line 94)**: OPEN, narrowed.
  - Closed leg: the cell lookahead is now case-insensitive (`:791`). `| x | P3 | **CLOSED (leg 2 open)** |` → False, pinned at `test_finding_register.py:997`.
  - Still open: `_quoted` (`:761-768`) does not check parentheses, although the spec says it does (`FINDING_REGISTER_V1.md:13-14`). `- **P3 (T54 stale — CLOSED at 92fa6646) is stale again at HEAD.**` → `is_closure_line(…, "P3")` True.
  - Still open: a lowercase `— open` tail is not read as OPEN. `- **[P4] [contract-drift] \`alpha_beta\` — CLOSED.** leg 2 — open` → True.
- **P4 `[record]` `claim-retirements.json:184,195` "line 24" (line 96)**: OPEN, narrowed.
  - Both dead "line 24" reasons remain (`:184`, `:195`).
  - The second-batch exact-claim entries (`:2051-2169`) now state per-claim grounds, but the earlier 8966da2e entries (`:2032`, `:2039`, `:2046`) keep the boilerplate reason.
- **P4 `regeneration_divergence` keying and stale docstring (line 98)**: CLOSED.
  - It now keys on the full `verified_by` (`retire_review_claims.py:596-597`), and the docstring says reopen → retire → fold (`:581-582`), matching the order at `:584-586`.
  - Pinned in `test_regeneration_keys_on_the_full_reference` (`:539-569`).
- **P4 hop-chained `carried from` keys per hop (line 100)**: OPEN.
  - The ledger origin is still the first named sha, not resolved transitively (`build_finding_register.py:1520-1521`).
  - My `@6589c6e` and `@79fdcc6` `[record]` claims are both still in `p4_open` (`machine-summary.json:6880-6881`).
- **P4 `claim_relation_problem` for scope-less claims (line 102)**: OPEN. `:1034-1045` applies no scope rule when `raising_scope` is `None`.
- **P4 line-span rule admits `42,` (line 104)**: OPEN. `PATH_TOKEN` (`:1070`) and the `ranges` rule (`:1265-1275`) are unchanged: a claim citing `docs/audits/x.md:42,` relates to `- **[P4] [misc] x.md table 42, other — CLOSED.**` even under `strong=True` (True).

**Probes of the repaired mechanism.** Each ran in a tempfile git fixture under `~/.cache`, built the way `RetireReviewClaimsTests.setUp` builds its fixture (three commits a<b<c, one lane):
- **Identifier-less named carry into a retired root.**
  - Root: `@a: [fail-closed-gap] scripts/x.py:10 - the guard fails open on empty input`.
  - Distinct carry: `@b: [fail-closed-gap] (carried from a, OPEN) scripts/x.py:90 - the bound is missing for long input`.
  - Closer at c: `- **[P3] [fail-closed-gap] scripts/x.py:10 — CLOSED.**`.
  - Both keys are `('vulcan', 'fail-closed-gap', 'scripts/x.py', '')`.
  - `retire` → retired 1: the root, under the strong read, because the carry counts toward its vocabulary. The carry is correctly refused.
  - `fold_restatements` → folded 1: the carry is restated into the retired root. `p3_open` is `[]`, `split_status_problems` is `[]`, and `package_restated_claims` accepts the result.
- **Counter-probe.** Adding a third same-kind claim (`scripts/y.py:5`) makes the kind shared → retired 1, folded 0, carry still open. This isolates the cause as the vocabulary view in `closing_lines_name`.
- **Live exposure.** 72 `p*_restated_claims` in total; 0 are identifier-less re-statements of a retired root.

## Findings

[P3] [fail-closed-gap] scripts/retire_review_claims.py:409-417 (`closing_lines_name` drops the retired root from the shared-vocabulary view unconditionally), :438-449 (the named-carry pass accepts an equal `""` key through `same_finding`), scripts/build_finding_register.py:1530-1537, docs/spec/FINDING_REGISTER_V1.md:156-158,175-179 - A residue of the prior P2: the retired-root guard's "retirement read" differs from the read `retire()` applies whenever both keys lack a backticked identifier. Under revision 9 an absent identifier is no identity, so in `retire()` the root counts toward the carry's shared vocabulary and the strong read applies. The guard removes the root from the view anyway, so the kind phrase no longer counts as shared and the weak read lets the root's generic `[kind] file:line — CLOSED` head speak about a distinct carry that names only a different anchor. The fixture shows the prior P2's failure shape again: retire refuses the carry (retired 1), the fold grants it (folded 1, `p3_open` []), and the builder, the split-status refusal and the regeneration check (a fixed point of the same rule) all accept. A third same-kind claim restores the refusal. Live exposure is 0 of 72 folds. - Closure evidence needed: exclude the linked pair from the view only when the key carries a non-empty identifier (or compute the guard with `retire()`'s own `shared_vocabulary`); pin the probe (identifier-less named carry, distinct anchor, retired root closed by a generic head → folded 0); re-derive the ledger.

[P4] [fail-closed-gap] scripts/build_finding_register.py:657-659,708-711,1376-1378,1619-1624 - Narrowed from my 8966da2e P3 (tagged leg closed and pinned). `raising_severity` still returns `None` for every untagged claim, and all three lists skip the binding on `None`. Live: 295 untagged claims, 21 with no resolvable raising line, 0 real mismatches. - Closure evidence needed: bind untagged claims through `raising_scope`'s transcript line, refuse a mismatch, and pin one untagged list transplant.

[P4] [fail-closed-gap] scripts/build_finding_register.py:885-933 (`shared_vocabulary` reads only the claim's own severity, :897-900) - Carried from 8966da2e, unchanged. - Closure evidence needed: compute shared vocabulary across every severity of the lane and pin the rw090 cross-severity probe.

[P4] [fail-closed-gap] scripts/build_finding_register.py:1239-1244 (commit-id rule) - Carried from 8966da2e, unchanged. A commit id embedded in a transcript path satisfies both halves of the rule together with that path (synthetic full-path `[record]` head → True); the live heads I tried do not relate. - Closure evidence needed: exclude commit ids that sit inside a path token, and pin the filename head as non-relating.

[P4] [fail-closed-tolerance] scripts/build_finding_register.py:1317-1318 (`is_tracked` returns True on `OSError`), :552 (`scope_generation` maps a failure to `-1`) - Carried from 8966da2e, unchanged. - Closure evidence needed: fail closed with git's error surfaced, and pin a failing-git test.

[P4] [contract-drift] scripts/build_finding_register.py:761-768 (`_quoted` ignores parentheses, contrary to docs/spec/FINDING_REGISTER_V1.md:13-14), :755 (lowercase `open` is not an OPEN status) - Carried from 8966da2e, narrowed (the cell lookahead is fixed at :791). `- **P3 (T54 stale — CLOSED at 92fa6646) is stale again at HEAD.**` and `… — CLOSED.** leg 2 — open` both still read as closure lines. - Closure evidence needed: track parenthesis depth, read `open` case-insensitively as a status, and add both shapes to the closure-shape test.

[P4] [record] evidence/review/claim-retirements.json:184,195 ("line 24"), :2032,2039,2046 (boilerplate reasons) - Carried from 8966da2e, narrowed (the second-batch entries at :2051-2169 state per-claim grounds). - Closure evidence needed: correct or drop the dead "line 24" reasons, and state the real ground for the three 8966da2e-round entries.

[P4] [contract-precision] scripts/build_finding_register.py:1520-1521 (ledger origin = first named `carried from` sha) - Carried from 8966da2e, unchanged. `machine-summary.json:6880-6881` still holds two open claims for one hop chain. - Closure evidence needed: resolve `carried from` transitively, and pin a two-hop chain → 1 open.

[P4] [fail-closed-gap] scripts/build_finding_register.py:1034-1045 (`claim_relation_problem` applies no scope rule when `raising_scope` is `None`) - Carried from 8966da2e, unchanged. - Closure evidence needed: refuse such closures unless bound exact-claim, and pin a scope-less P1 closure as `SUMMARY_INVALID`.

[P4] [fail-closed-gap] scripts/build_finding_register.py:1070 (`PATH_TOKEN` span `[0-9,-]+`), :1265-1275 - Carried from 8966da2e, unchanged. The span `42,` still relates under the strong read (probe True). - Closure evidence needed: strip trailing separators, require non-trivial spans, and pin `42,` as non-relating.

## Assessment

The section's records are exact at this records-only mint:
- T54 `ecafe536…` (2187 files, 61034378 bytes) equals the HEAD tree recomputed from git objects.
- T52 regenerates identically.
- Every provenance dependency and byproduct digest matches the file on disk.
- The subject `ecf51188…` and commit `8d063f00…` are bound consistently across provenance, the reproducibility report and both SBOMs.
- The section's builders, checker and lock files are untouched by the delta.

The audit (DEFERRED by design), evidence `--check`, the register checker, `retire_review_claims.py --check` and 170 unit tests pass. The section checker FAILs here only because this worktree's gitignored runtime evidence predates the mint (69907ddf); the mint worktree's evidence matches the tracked records. I did not hash the tarball.

The acbc65f0/0f4b8408 repairs close my 8966da2e P2 as named, and they close the equal-`""`-key P3, the tagged leg of the `raising_severity` P3 and the `regeneration_divergence` P4. One mechanism residue remains (the P3 above). The retired-root guard excludes the root from its vocabulary view even when neither key carries an identifier, so for identifier-less named carries the fold again grants what the strong read refused. This is fixture-demonstrated, the counter-probe isolates the cause, and it has no live exposure. Eight carried P4s remain, plus the narrowed `raising_severity` leg (nine P4s in total). No GA or release-readiness claim is made.

VERDICT: REVISE_0_P0_0_P1_0_P2_1_P3_9_P4
SECTION: standards_sbom_and_provenance
FIELD: vulcan_surface_review
SCOPE_SHA: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e
FINDINGS: [P3] [fail-closed-gap] scripts/retire_review_claims.py:409-417,438-449 with scripts/build_finding_register.py:1530-1537 - closing_lines_name drops the retired root from the shared-vocabulary view even for identifier-less keys, so an identifier-less named carry of a distinct finding folds into the retired root via the weak kind-phrase read that retire() refused (fixture: retired 1, folded 1, p3_open []; shared-kind counter-probe folded 0; live 0 of 72) - exclude the pair only for identifier-bearing keys, pin, re-derive; [P4] [fail-closed-gap] scripts/build_finding_register.py:657-659,708-711,1376-1378,1619-1624 - untagged claims skip the raising-severity binding (narrowed from P3; tagged leg closed; live 295 untagged, 21 unresolved, 0 mismatches); [P4] [fail-closed-gap] scripts/build_finding_register.py:885-933 - one-severity shared vocabulary (carried); [P4] [fail-closed-gap] scripts/build_finding_register.py:1239-1244 - commit id inside a transcript path relates (carried); [P4] [fail-closed-tolerance] scripts/build_finding_register.py:1317-1318,552 - git failure tolerated (carried); [P4] [contract-drift] scripts/build_finding_register.py:755,761-768 - parenthesised and lowercase-open shapes read as closures (carried, cell leg closed); [P4] [record] evidence/review/claim-retirements.json:184,195,2032,2039,2046 - dead "line 24" and boilerplate reasons (carried, narrowed); [P4] [contract-precision] scripts/build_finding_register.py:1520-1521 - per-hop carried-from origin (carried); [P4] [fail-closed-gap] scripts/build_finding_register.py:1034-1045 - scope-less closure relation (carried); [P4] [fail-closed-gap] scripts/build_finding_register.py:1070,1265-1275 - line span `42,` relates under the strong read (carried)
SUMMARY: The section's records are exact at d158d26b: T54 `ecafe536…` (2187 files) equals the HEAD tree recomputed from git objects, every provenance and SBOM digest matches the files and binds 8d063f00/ecf51188, and the audit, evidence check, register, retirement check and 170 unit tests pass; the section checker's `closure:ineligible` comes from this worktree's stale gitignored runtime evidence, not the tree. The delta closes my 8966da2e P2, the equal-empty-key P3, the tagged leg of the raising-severity P3 and the divergence-keying P4. One P3 residue remains: the retired-root fold guard drops the root from its vocabulary view even for identifier-less keys, so a distinct identifier-less named carry is folded after the strong read refused it (fixture-demonstrated, no live exposure). Nine P4s remain open: eight carried from 8966da2e and the narrowed untagged leg of the raising-severity P3.
