<!-- engine: claude-code; observed model: claude-opus-5; scope: 178873d7f9df2da5e616a986a24db8c2559f8ca0; role: vulcan; field: legacy_adapter_contract_review_closure; dispatched: 2026-09-18T18:23:23Z; duration_s: 203; process_exit_code: 0 -->
# Vulcan Council review — s20_600_frozen_legacy_adapter
Harness: claude-code
Reviewed checkpoint: 178873d7f9df2da5e616a986a24db8c2559f8ca0

Scope verified: `git rev-parse HEAD` → `178873d7f9df2da5e616a986a24db8c2559f8ca0` (matches SCOPE_SHA). Lane files (`bench/legacy/runner.py`, `bench/legacy/tests/test_runner.py`, `scripts/check_legacy_runner.py`, `docs/adr/ADR-0018-frozen-legacy-artifact-adapter.md`, `docs/spec/LEGACY_ARTIFACT_ADAPTER_V1.md`) have no diff between the closing commit `0c94714e` and HEAD (`git diff --stat 0c94714e..HEAD` empty) and are clean in the worktree (`git status --porcelain` empty for those paths). The only later commit touching `bench/legacy/` is `1605528e`, which adds `bench/legacy/task_runner.py` and S3 fixtures — a different surface, not part of this row.

**Transcript located.** The row's transcript is `evidence/review/verdicts/s20_600_frozen_legacy_adapter/vulcan_review-575f497.md` (FIELD `legacy_adapter_contract_review`, ROUND final, disposition `PASS_0_P0_0_P1_0_P2_5_P3`); `machine-summary.json` `legacy_adapter_repair_note` points to it. The other two transcripts (`9d608c4` REVISE for the same field; `e464ed4` REVISE for field `vulcan_review`) were read as provenance only. The register also lists a sibling row (`vulcan_review`, same disposition) — not this row; I issue no verdict on it.

**Findings named at the unclaimed severity (P3), verbatim from the transcript:**
1. `[P3] VS-LA-R2-A [tests]` — member-count sub-case: with default ceilings the 17th regular member trips the file count before the member count can fire (same code asserted). Fix: contract ceiling overrides per case.
2. `[P3] VS-LA-R2-B [tests]` — sley exec/digest overrides hit manifest equality before the contract-level checks. Fix: manifest-consistent overrides.
3. `[P3] VS-LA-R2-C [checker]` — `artifact_size_bytes` reconciled against nothing. Fix: format from the contract + register loop entry.
4. `[P3] VS-LA-R2-D [record]` — ADR origin-host sentence unqualified. Fix: qualify or generalize.
5. `[P3] VS-LA-R2-E [implementation]` — teardown `temporary.cleanup()` untranslated. Fix: translate to STAGING_FAILED.

Commands run: `git rev-parse HEAD`; `git log --oneline 575f497..HEAD -- <lane files>`; `git log -S "VS-LA-R2"` → single hit `0c94714e`; `git show 0c94714e` (full lane diff read); `git blame` on `runner.py:826-836`, `test_runner.py:218-219`, `check_legacy_runner.py:171,186`, ADR `:23-25` — all attribute to `0c94714e`; `python3 scripts/check_legacy_runner.py` → exit 0, `"result": "PASS"`, `"problems": []`, 15 codes, 27 tests, artifact `b24f19c6…` verified (1568/1067/8,610,725), two retained records listed; `python3 -m unittest discover -s bench/legacy/tests -v` → `Ran 27 tests … OK`; one read-only branch probe (below). Files read: `runner.py:495-609, 780-836, 1087-1146`; `test_runner.py:60-220, 449-467, 596-650`; `check_legacy_runner.py:1-280`; ADR `:18-35`; spec `:25,45,50`.

## Evidence checked

**VS-LA-R2-A — CLOSED** (commit `0c94714e`). `test_runner.py:121,218-219` add `contract_overrides` applied via `dataclasses.replace`; `test_archive_ceilings_fail_closed` `member_count` case (`:437-443`) sets `max_regular_files: 64` against the default `max_archive_members=32` (`:212`). Trace at `runner.py:505-508`: 3 dirs + 32 payload + 3 metadata = 38 members; the 33rd trips `member_count > 32` at `:507` before the regular-file gate at `:521` (35 files < 64). Probe at HEAD confirms the branch: with override → `ARCHIVE_LIMIT_EXCEEDED 'archive members'`; without override (the pre-fix shape) → `'regular files'`. The `file_count` case still reaches `'regular files'` as intended.

**VS-LA-R2-B — CLOSED** (`0c94714e`). `test_runner.py:610-616, 626-632` now use `extra_payload_files={"bin/sley": …}` so both manifest (`_manifest(payload_files)` `:130`) and archive carry the same mode/bytes; `ArchivedFile` equality at `runner.py:571-574` passes and control reaches `:577-578` / `:579-583`. Probe: nonexec → `PAYLOAD_MISMATCH 'bin/sley executable'`; drifted → `PAYLOAD_MISMATCH 'bin/sley digest'`; the old `mode_overrides` shape → `'bin/sley'` (manifest equality), confirming the prior mislabel is gone.

**VS-LA-R2-C — CLOSED** (`0c94714e`). `check_legacy_runner.py:171` formats `f"{FROZEN_CONTRACT.artifact_size_bytes:,}"` and requires it in the spec (spec `:25` "size 4,611,024 bytes"); `:186` adds `("artifact_size_bytes", FROZEN_CONTRACT.artifact_size_bytes)` to the register reconciliation loop (register value 4611024). Member/payload count literals are likewise derived from the contract (`:176-178`). Checker ran green.

**VS-LA-R2-D — CLOSED** (`0c94714e`). ADR-0018 `:23-25` now reads "On the origin host as of 2026-09-11, two 10-second timeouts, one 30-second timeout, and a successful longer smoke are retained …", followed by the host-local/not-tree-reproducible paragraph (`:26-31`). Checker tokens `successful longer smoke` and `host-local` (`:160-163`) still hold.

**VS-LA-R2-E — CLOSED** (`0c94714e`). `runner.py:831-836` wraps `temporary.cleanup()` in `try/except OSError` raising `LegacyRunnerError(STAGING_FAILED, "stage teardown: …")`; `run_version_smoke` catches `LegacyRunnerError` at `:1107` and emits the `harness_failure` record with `failure_code`/`failure_detail` (`:1108-1129`), so a teardown failure reaches the evidence path rather than a bare traceback. Setup/mkdir paths remain wrapped (`:784-802`).

Follow-ups F/H/I (P4, not in this row's unclaimed set) were incidentally confirmed present: drain pacing `:947-952`, checker comment `:104-108`.

## Findings
None.

## Assessment
All five P3 findings named by `vulcan_review-575f497.md` are closed by commit `0c94714e`, and the lane files are byte-identical between that commit and the scope SHA. I did not accept the commit message: each fix was traced through the current verifier (`runner.py:505-583`) and, for A and B, confirmed by a branch probe at HEAD that reports the detail string of the branch actually reached — the labelled branches (`archive members`, `bin/sley executable`, `bin/sley digest`) now fire where the pre-fix shapes fire elsewhere. The checker exits 0 with `PASS` and no problems against the host artifact, and the 27-test suite passes. No new defect found at these paths. Unscored observation for the record (not a defect, standard in the lane is symbol-level assertion, which is met): `test_archive_ceilings_fail_closed` and the sley-identity test assert `code` only, so a future regression of A/B's branch labelling would not be caught by the suite — asserting `caught.exception.detail` would harden that.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_0_P4_PRIOR_P3_CLOSED
SECTION: s20_600_frozen_legacy_adapter
FIELD: legacy_adapter_contract_review_closure
SCOPE_SHA: 178873d7f9df2da5e616a986a24db8c2559f8ca0
FINDINGS: none
SUMMARY: The original transcript for this row (vulcan_review-575f497.md) names five P3s, VS-LA-R2-A through VS-LA-R2-E; each is verified closed at 178873d7 by commit 0c94714e via source trace, git blame, a read-only branch probe, `scripts/check_legacy_runner.py` (exit 0, PASS, no problems) and the 27/27 unittest run. Lane files are unchanged since the closing commit. No new P0–P4 defects were found at these paths; the row is closable.
