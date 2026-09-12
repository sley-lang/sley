# Vulcan re-review — `s20_600_frozen_legacy_adapter.legacy_adapter_contract_review` (qualifying, HEAD 575f497)

Execution of both `pytest` and the checker was denied by the sandbox; everything below is a static trace from source at HEAD `575f497` (confirmed equal to the task's HEAD; working tree clean apart from two unrelated untracked `.forge/slices` files).

**Assumptions stated explicitly**
- `python3 -m pytest …` and `python3 scripts/check_legacy_runner.py` were both denied; I did not run them. Every claim below is derived by tracing test → runner path by hand. The sandbox also blocks reads outside the repo, so the pinned artifact was not rehashed by me.
- Corroboration that the artifact verifies on this host: two retained records (not written by me) at `evidence/runtime/s20-600-legacy-smoke/`, both with full `verification` blocks matching `FROZEN_CONTRACT`. The filename digest prefix of each equals the SHA-256 of its bytes (`f1fdef19e1d26489…`, `d496357cfe2b0805…`), so the create-only naming is intact.
- `test_escaped_grandchild_does_not_spin_the_drain` depends on a `setsid` binary on `PATH=/usr/bin:/bin` (util-linux). If absent, `sh` exits 127 → status `failed` → the test fails rather than passing vacuously.
- `tarfile.addfile` writes a hand-built `TarInfo` name verbatim (no leading-slash strip on write or read), so the `absolute` case reaches `pure.is_absolute()` at `runner.py:265`. Even if a stdlib version stripped it, `pure.parts[0] != contract.top_level_directory` at `:267` yields the same code.
- `make legacy-runner-smoke` was not run (it writes evidence).

## Six acceptance criteria — unchanged, re-confirmed at HEAD

1. **Pinned identity** — `runner.py:469-479`: `O_NOFOLLOW` open, `fstat` size check, streamed SHA-256 on the open descriptor, `artifact.seek(0)` before `tarfile.open`. Private copy re-hashed while copied (`:612-651`) and fully re-verified (`:806`). `FROZEN_CONTRACT` (`:117-131`), spec:21-31/45/50, register `:3444-3462`, and both retained records agree on sha/size/commit/tree digest/`bin/sley` digest/1568/1070/1067/8,610,725.
2. **Confined extraction** — `:667-771`: no `extract`/`extractall`; `_member_relative_path` (`:257-275`) + `relative_to(stage_parent)` (`:660-663`); `O_EXCL|O_NOFOLLOW` `0o600` (`:704-712`); write bits stripped (`:747`), dirs `0o555` deepest-first (`:762-763`).
3. **Per-file rehash** — verification `:535-537`, extraction `:713-746` vs verified digest.
4. **Single argv** — `VERSION_ARGUMENTS = ("--version",)` (`:38`); executable gate `:845-846`; argv built internally `:877`.
5. **Bounded execution** — `Popen` `shell=False`, `close_fds=True`, `stdin=DEVNULL`, exact 12-key env, `start_new_session=True` (`:903-913`); timeout `0 < t ≤ 120` finite (`:847-854`); output cap; `killpg(SIGKILL)` on deadline/overflow; **now also** drain bound + reap-against-deadline (see repair 4).
6. **Create-only evidence** — `:1139-1178` unchanged: `O_EXCL|O_NOFOLLOW` `0o600`, file + dir fsync, `FileExistsError → EVIDENCE_WRITE_FAILED`.

## Repair (1) — every stable code + contamination path has a producing test

Traced each of the 18 new tests (9 → 27; the commit says "15 new", the count is 18) to the runner line that raises. All 15 codes are produced **and asserted by symbol**:

| Code | Test | Runner line reached |
|---|---|---|
| ARTIFACT_MISSING | `test_missing_artifact_path_reports_missing:379-388` | `:225-226` FileNotFoundError |
| ARTIFACT_IDENTITY_MISMATCH | `test_outer_identity_drift…:246-256` | `:471-472` size |
| ARCHIVE_INVALID | `test_garbage_archive_reports_invalid:390-415` | `:503` `tarfile.open` ReadError → `:546-547` |
| ARCHIVE_MEMBER_UNSAFE | traversal/symlink/duplicate `:258-270`; hardlink/fifo/device → `:271-272`; absolute → `:265-266`; setuid `0o4755` → `:273-274`; wrong-top `intruder-top` → `:267-268`; dir drift (`skip_directories=(TOP/bin,)`) → `:559-560` | as listed |
| ARCHIVE_LIMIT_EXCEEDED | `member_bytes` 200 KiB > 128 KiB → `:519-520`; `total_bytes` 3×100 KiB > 256 KiB → `:541-542`; `member_count`/`file_count` → `:521-522`; oversized manifest 70 KiB > 64 KiB → `:528-530` | see P3-A |
| MANIFEST_INVALID | not_json / not_object / duplicate_key → `:290-295`; `del manifest["source"]` → `:331`→`:301`; `omit_manifest` → `:549-550`; `file_count += 1` → `:403-405` | correct (test comment at `:517-519` honestly explains why counts are INVALID not MISMATCH) |
| MANIFEST_IDENTITY_MISMATCH | `publication_authorized=True` → `:358-359`; tree digest → `:344-348` | correct |
| PAYLOAD_MISMATCH | corrupt fixture → `:571-574`; `stowaway.txt` → `:566-568`; nonexec / drifted sley → `:571-574` | see P3-B |
| STAGING_FAILED | copy into nonexistent dir → `:620` OSError → `:650-651`; mocked `os.chmod` ENOSPC → `:759-767` | correct |
| COMMAND_NOT_ALLOWED | `nan` timeout → `:850`→`:854` | string symbol asserted `:331` |
| COMMAND_TIMEOUT | `sleep 2`/0.05 s; hung-child-closed-fds → `:978-981`; escaped grandchild → `:928-936` | `:287`, `:664`, `:683` |
| COMMAND_OUTPUT_LIMIT | `yes X`/1 KiB → `:962-968` | `:314` |
| COMMAND_FAILED | exit 7 → `:1017-1019` | `test_command_failed_reports_symbol:644-652` — closes prior P4 |
| EVIDENCE_WRITE_FAILED | duplicate record → `:1171-1174` | `:350-354` |
| INTERNAL_INVARIANT | `not-a-digest` → `:242-243`; size 0 → `:244-245` | `:612-631` |

No test asserts a wrong code and no test is vacuous. Two sub-cases exercise a *different branch than labelled* (same code) — scored P3 (VS-LA-R2-A/B), not P1, because the asserted code is genuinely produced by a real verifier rejection.

## Repair (2) — timeout-evidence continuity

- `smoke-20260912T162641Z-d496357cfe2b0805.json`: `"status":"timeout"`, `"failure_code":"LEGACY_COMMAND_TIMEOUT"`, `"timeout_seconds":2.0`, `"return_code":-9`, `"duration_ms":2001`, full `execution` and full `verification` block. Shape matches spec:72-75 exactly.
- ADR-0018:25-31 and register scope now say host-local, gitignored, reproducible on any host holding the artifact, not tree-reproducible, no digest index. Accurate and honest. Residual wording issue → P3-D.

## Repair (3) — checker derivations

Counts derived from enum + AST discovery; spec reconciliation and register reconciliation verified; BLOCKED distinct on ARTIFACT_MISSING; evidence dir read and listed. Predicted `PASS` with 15 codes, 27 tests, two retained records.

## Repair (4) — drain bound, wait classification, evidence wrap

Drain bound with worst-case wall = timeout + 5 s + 1 s + 1 s; wait-against-deadline classification; staging setup/chmod errors reach the evidence path as STAGING_FAILED. Both behavior tests genuinely fail against the pre-fix runner by construction. Residuals → P3-E, P4-F.

## FINDINGS

- **[P3] VS-LA-R2-A [tests]** `bench/legacy/tests/test_runner.py` member-count sub-case: with default ceilings the 17th regular member trips the file count before the member count can fire (same code asserted). Fix: contract ceiling overrides per case.
- **[P3] VS-LA-R2-B [tests]** sley exec/digest overrides hit manifest equality before the contract-level checks. Fix: manifest-consistent overrides.
- **[P3] VS-LA-R2-C [checker]** `artifact_size_bytes` reconciled against nothing. Fix: format from the contract + register loop entry.
- **[P3] VS-LA-R2-D [record]** ADR origin-host sentence unqualified. Fix: qualify or generalize.
- **[P3] VS-LA-R2-E [implementation]** teardown `temporary.cleanup()` untranslated. Fix: translate to STAGING_FAILED.
- **[P4] VS-LA-R2-F [implementation]** select timeout 0.0 during drain grace → bounded 5-s hot spin. Fix: pace by drain deadline.
- **[P4] VS-LA-R2-G [checker]** BLOCKED exits 0; consumers must parse JSON.
- **[P4] VS-LA-R2-H [checker]** token-presence proxy sound only jointly with the unittest run.
- **[P4] VS-LA-R2-I [record]** repair-note hygiene (uncommitted/15/tests/typo).
- **[P4] carried** evidence-dir symlink, 90-s default vs measured margin.

## SUMMARY

All four required-for-PASS repairs are real and verified from source. Every one of the 15 stable codes now has a producing test asserting the symbol; the local timeout record exists with the full spec shape; the checker derives counts, reconciles the contract, and reports BLOCKED distinctly; the drain is bounded with correct classification and evidence reachability. Residual P3s are branch-label mismatches, an unreconciled size field, one stale sentence, and an untranslated cleanup path — none affects the six acceptance criteria.

```
FIELD legacy_adapter_contract_review
ROUND final
VERDICT PASS
SEVERITY_LINE 0_P0_0_P1_0_P2_5_P3
SCOPE repaired lane at 575f497 — bench/legacy/runner.py, bench/legacy/tests/test_runner.py, scripts/check_legacy_runner.py, docs/spec/LEGACY_ARTIFACT_ADAPTER_V1.md, docs/adr/ADR-0018, machine-summary s20_600 section, evidence/runtime/s20-600-legacy-smoke/ (static trace; pytest/checker execution denied)
Disposition = PASS_0_P0_0_P1_0_P2_5_P3
```
