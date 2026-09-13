# Nabu architecture review — `s20_610_offline_raw_runner.nabu_review` (HEAD a4b6029)

Scope verified: `git rev-parse HEAD` = `a4b60294e4dd75133fa08f92bf16022fb05bb807`. Read-only except this file; no edits to `machine-summary.json`, the checker, or any other file.

## Question

Does the S20-610 runner satisfy the TO_OFFLINE append-only contract — i.e. is it genuinely offline (no external command, no provider/model execution, no Sley-1 interaction, 0 actual trials) and genuinely append-only (create-only manifest, append-only digest-claim chain) — despite `external_head_anchor`, `artifact_bytes_verified`, `oracle/accounting_claims_verified`, `accepted_change_tokens_derived`, and `public_claim_authorized` all being false?

## Evidence executed

- `python3 scripts/check_raw_baseline_runner.py` → `result: PASS`, `problems: []`, 16 codes / 17 controls / 25 metrics / 4 smoke tests, `external_command_adapter: false`, `provider_or_model_execution: false`, `sley_1_2_interaction: false`, `actual_trials: 0`, `machine_summary_registered: true`. Exit 0.
- `python3 -m unittest discover -s bench/raw/tests` → 4/4 ok (`test_success_and_timeout_are_chained_and_complete`, `test_control_drift_and_external_execution_fail_closed`, `test_duplicate_and_trial_limit_preserve_existing_chain`, `test_tamper_and_noncanonical_manifest_are_detected`). Exit 0.
- Independent digest/count reconciliation: benchmark-plan SHA-256 `e0549d51…aef24`, corpus SHA-256 `7370b6cc…988d`, 25 metrics, 17 `run_freeze_required_fields`, 15 corpus tasks, required arms exactly `raw_files/sley_1_2_0/sley_2_0` (`zerolang` present but `required: false`), 16 `RawErrorCode` members (61000–61015). All match the summary row and the checker pin.

## Offline — verified from source

- `bench/raw/runner.py:11-20` imports only `fcntl, hashlib, json, os, re, stat, datetime, enum, pathlib, typing`. No `subprocess/socket/requests/urllib/http` (checker AST-gates these; confirmed by read). No `os.system/os.popen/Popen/run_command//home/greyforge/sley` strings.
- Execution surfaces are injected `Protocol`s only (`AgentAdapter/ToolAdapter/OracleAdapter/WorkspaceAdapter/AccountingClock`, runner.py:121-159) with docstrings stating no implementation is supplied. The module never calls `datetime.now` (timestamps are parsed via `strptime` at :282 and supplied by records); grep confirms no `exec/spawn/fork/unlink/remove/rename/chmod/symlink/truncate` anywhere in the module.
- `validate_run_manifest` fail-closes `execution_mode != offline_injected` and `external_command_policy != forbidden` to `EXTERNAL_EXECUTION_FORBIDDEN` (runner.py:311-314); the drift test proves the `allowed` policy is rejected.

## Append-only — verified from source

- Manifest: `write_run_manifest` (runner.py:452-470) does `mkdir(exist_ok=False)` + `O_WRONLY|O_CREAT|O_EXCL` + file and directory `fsync`. No overwrite path exists; a second create raises `APPEND_FAILED`.
- Claims: `append_trial_digest_claim` (runner.py:659-711) opens with `O_RDWR|O_CREAT|O_APPEND`, takes `LOCK_EX`, re-verifies the complete existing chain, rejects duplicate trial IDs / task-seed pairs and the schedule ceiling, writes exactly one canonical line, and fsyncs both file and directory descriptor. The module exposes no rewrite or deletion API.
- Verification (`_parse_and_verify_records`, :608-646) detects noncanonical bytes, truncation (missing final newline), reordering, broken `previous_record_digest` linkage, digest tampering, and duplicates — all exercised by the smoke tests, which assert the exact `CHAIN_INVALID` / `MANIFEST_INVALID` / `TRIAL_DUPLICATE` / `CONTROL_MISMATCH` / `EXTERNAL_EXECUTION_FORBIDDEN` codes.

## False-by-design fields — scoped, not violated

- `external_head_anchor: false`: the spec itself (RAW_BASELINE_RUNNER_V1.md:107-111) states a wholly replaced local chain cannot prove rollback history without a separately retained head digest and forbids misrepresenting the local chain as WORM storage; ADR-0017 consequences repeat it. The contract requires the mechanism plus this honest limitation, both present.
- `artifact_bytes_verified / oracle_claims_verified / accounting_claims_verified: false`: spec :93-100 defines the chain as verifying only bytes and ordering with no promotion API. The runner enforces the scoping — any record not carrying exactly `UNVERIFIED_INJECTED_DIGEST_CLAIMS` / `UNVERIFIED_ADAPTER_CLAIM` fails `RECORD_INVALID` (runner.py:565-570). A "verified" claim is unrepresentable, which is the requirement.
- `accepted_change_tokens_derived: false`: non-null ACT is rejected with "ACT is derived only by S20-630" (runner.py:502-504).
- `public_claim_authorized: false`, `actual_trials: 0`: match the spec's explicit gaps (:123-128) and ADR-0017 consequences; no trial, accounting, or publication path exists in the module.

## Note (not a finding)

`machine-summary.json:3514` and `scripts/check_raw_baseline_runner.py:135,173` still pin `nabu_review = REVISE_TO_OFFLINE_APPEND_ONLY_CONTRACT`. That is the pre-verdict state this review supersedes; pin maintenance is explicitly deferred until after a PASS, so it is recorded here as process context, not as a contract defect. No code, spec, or record change is required for PASS.

## FINDINGS

None. No open P0–P4.

```
VERDICT: PASS_NO_OPEN_P0_P1_P2_P3_P4 / SECTION: s20_610_offline_raw_runner / FIELD: nabu_review / SCOPE_SHA: a4b60294e4dd75133fa08f92bf16022fb05bb807 / FINDINGS: none
```
