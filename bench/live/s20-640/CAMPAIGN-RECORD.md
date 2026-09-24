# S20-640 live succession campaign record

Append-only. Branch `campaign/s20-640-live`, based on `ba9ebf49` (the
integrated succession arm). The preregistration is
`bench/live/s20-640/PREREGISTRATION.json`, committed before any live
attempt. No counted attempt has run. `ga_claimed=false`.

## Operator decisions

- 2026-09-24, verbatim: Council reviews "no longer required" — 2026-09-24.
  Every Council-transcript prerequisite of the campaign (S20-620 revision 8
  lane verdicts, the TYPE fixture packet revision 3 review, the S20-630
  review line) is waived by this decision. The waiver removes a process
  gate only; it changes no oracle, fixture, threshold, or task.
- 2026-09-24, verbatim: "you always have my permission" (campaign and
  provider spend). The full campaign starts only after the operator binds
  it to the release-candidate commit and says go.

## Readiness at preregistration (2026-09-24T03:32Z)

| Prerequisite | State | Evidence |
|---|---|---|
| Frozen plan, corpus, task statements | MET | plan `e0549d51…`, corpus `7370b6cc…`, statements `737c0c8b…` (checked again at every freeze) |
| Preregistered tiers, seeds, budgets, retry, order, halt rule | MET | `PREREGISTRATION.json`, this commit |
| 90-attempt minimum (15 × 3 × 1 seed × 2 tiers) | MET by design | `scheduled_attempts` = 45 per tier manifest |
| Oracle freeze | MET for this tree | `oracle_digest` `a9e597f7…` recomputed before every slot; TYPE revision 3 manifest adopted on this tree, its review waived above |
| Arm isolation | MET after fix 2 below | before: raw/legacy agents could read `bench/fixtures/*/fixture` (the positive programs) and oracles; sley_2_0 could not |
| Provider environment isolation | MET after fix 2 | before: operator `~/.codex` (AGENTS.md, ~200 skills, memories, sessions) entered every arm's context |
| sley_2_0 real-provider launch | MET after fixes 2 and 3 | before: provider binary, home, and network absent in the sandbox; Codex's command sandbox refused the gateway socket |
| Per-slot binding (commit, clean tree, digests, binaries) | MET after fix 4 | `bench/live/launch.py verify_binding` |
| Over-budget attempts retained | MET after fix 1 | before: record validation raised and the slot was lost |
| Package / dossier binding | PENDING (operator) | the counted runs freeze at the release-candidate commit with its `sley` build; not yet available |
| Live-attempt accounting | MISSING | `bench/accounting/report.py` reads the offline claim chains (`raw/`, `sley2/`), not `attempts.jsonl`; no legacy chain verifier; three section 22 rows stay `NOT_EVALUATED`. A threshold table cannot be derived from a live run until an attempts-to-accounting adapter exists |
| Provider availability | BLOCKED | every model on the account: `You’ve hit your usage limit. … try again at Sep 26th, 2026 9:30 PM.` (probed 2026-09-24T03:13Z with gpt-5.6-sol, gpt-6-astra, gpt-5.6-luna, and through the sandbox with gpt-6-luna) |
| Zerolang arm | NOT RUN | plan `UNESTABLISHED`; optional |

## Harness defects fixed before the pilot

1. `attempts._validate_metrics` rejected the honest metrics of an attempt
   the runner had classified `LIVE_PROVIDER_BUDGET_EXCEEDED`, so
   `build_attempt` raised and the slot was never appended. Over-budget
   metrics are now admitted for exactly that outcome and refused for every
   other (`test_campaign.py`).
2. One outer provider sandbox for all three arms
   (`bench/live/provider_sandbox.py`): fresh sandbox HOME, fresh provider
   home with only the credential bound, provider release directory
   read-only, no network namespace route out except an allowlisting CONNECT
   proxy (`chatgpt.com`, `auth.openai.com`, port 443) reached through a
   loopback forwarder; model commands get no proxy variables and so no
   network. raw_files and sley_1_2_0 see only system directories, their
   workspace, and (legacy) the frozen tool closure; no fixture, oracle, or
   repository path (`test_provider_sandbox.py`, proven by in-sandbox
   probes).
3. Codex's workspace-write command sandbox refuses every socket connect,
   AF_UNIX included, unless its network access is enabled; the mediated
   sley_2_0 tool therefore could not reach its gateway from any
   model-issued command. `CodexExecAdapter(command_network_access=True)`
   enables it for every arm; it is safe only inside fix 2's namespace.
   Regression: the stand-in attempt through the real provider sandbox is
   accepted with the flag and records `LIVE_PROVIDER_EXIT_NONZERO` with
   zero gateway exchanges without it.
4. `bench/live/launch.py`: freeze from the preregistration, per-slot
   binding re-derivation, preregistered order, no re-run of recorded
   slots, halt on provider unavailability (`test_launch.py`).

## PILOT 2026-09-24 (does not count toward the campaign)

Run `s20-640-pilot-20260924-small`, label `PILOT`, frozen by
`bench.live.launch freeze` at `5b8b1d35` (clean tree), tier small
(`gpt-6-luna`, medium), codex-cli 0.155.1, debug `sley`
`d9a24d48…`, judge driver `faf507bd…`. Raw run directory (retained,
untracked): `/home/gfarch/Work/checkpoints/sley2-campaign-runs/s20-640-pilot-20260924-small/`
(manifest, `attempts.jsonl`, artifact store, captures, egress logs).
Slots: REPAIR, TEST, CONTEXT × raw_files, sley_1_2_0, sley_2_0 at seed
640, run through `execute_attempt` with the provider sandbox and
`--continue-on-provider-unavailable` (pilot only).

| Slot | Status | Code | Provider exit | Wall ms | Tokens in/out |
|---|---|---|---|---|---|
| REPAIR sley_1_2_0 | harness_failure | LIVE_PROVIDER_EXIT_NONZERO | 1 | 4538 | 0/0 |
| REPAIR sley_2_0 | harness_failure | LIVE_PROVIDER_EXIT_NONZERO | 1 | 5445 | 0/0 |
| REPAIR raw_files | harness_failure | LIVE_PROVIDER_EXIT_NONZERO | 1 | 4837 | 0/0 |
| TEST sley_2_0 | harness_failure | LIVE_PROVIDER_EXIT_NONZERO | 1 | 4778 | 0/0 |
| TEST raw_files | harness_failure | LIVE_PROVIDER_EXIT_NONZERO | 1 | 4614 | 0/0 |
| TEST sley_1_2_0 | harness_failure | LIVE_PROVIDER_EXIT_NONZERO | 1 | 5938 | 0/0 |
| CONTEXT raw_files | harness_failure | LIVE_PROVIDER_EXIT_NONZERO | 1 | 4646 | 0/0 |
| CONTEXT sley_1_2_0 | harness_failure | LIVE_PROVIDER_EXIT_NONZERO | 1 | 4728 | 0/0 |
| CONTEXT sley_2_0 | harness_failure | LIVE_PROVIDER_EXIT_NONZERO | 1 | 4928 | 0/0 |

Every provider stream ends `turn.failed` with "You’ve hit your usage
limit. … try again at Sep 27th, 2026 1:30 AM." (UTC inside the sandbox;
2026-09-26 21:30 local). No model turn ran, so the pilot proves the launch
path only: in all three arms the confined provider started, authenticated
with the bound credential, reached `chatgpt.com:443` through the egress
proxy, and was refused by the account; the proxy denied
`sdmntprsouthcentralus.oaiusercontent.com:443` (provider asset fetch)
identically in every arm; provider stderr was empty; all nine records
verify (`bench.live.launch status`). Zero token metrics on these records
are the harness-failure convention (no `turn.completed` usage exists), not
measurements. The host credential file was unchanged afterwards (same
inode and size). A model-turn pilot must be re-run after the usage window
resets, before the counted campaign.

Validation at `5b8b1d35`: `python3 -m unittest discover -s bench/live/tests`
with `SLEY2_SLEY_BINARY`, `SUCC_JUDGE_TEST_BINARY`, and
`SLEY2_LIVE_PROVIDER_ROOT` bound and bwrap present: 353 tests OK, none
skipped (335 OK at `ba9ebf49` before the fixes).
