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

## 2026-09-24: accounting over live runs, Claude Code provider, corrections

- Correction to fix 2 above: "model commands get no proxy variables and so
  no network" was imprecise. Under Codex (`network_access=true`) and
  Claude Code (whose Bash inherits `HTTPS_PROXY`), a model-issued command
  could connect to the loopback forwarder and reach the allowlisted
  provider hosts. Fixed: the in-sandbox forwarder now admits only
  connections held by the provider process itself (socket inode ownership
  in `/proc`); anything else is refused and the runner-side proxy logs
  `deny_local_process` (`test_claude_provider.py`).
- Credential exposure mitigated: no host credential is bound any more.
  Each attempt gets a private copy with only the access token (no refresh
  token, no MCP OAuth entries), so nothing in the sandbox can rotate the
  operator's login; an attempt is refused before launch unless the token
  outlives the wall budget by 30 minutes. Residual: a model command can
  read that short-lived access token (no egress; it could appear in the
  retained transcript). Documented in `bench/live/provider_sandbox.py`.
- Operator decision (relayed 2026-09-24): "ignore the codex, proceed with
  your own usage instead". `PREREGISTRATION-2-claude-code.json` supersedes
  `PREREGISTRATION.json` (sha256 `d9418d97…`, kept unchanged as history;
  the launcher refuses to freeze a superseded preregistration). Tiers
  `claude-haiku-4-5-20251001` / `claude-opus-5-5`, effort medium; seed,
  budgets, order, and attempt count unchanged; halt rule extended to halt
  before the next slot at 90% provider-reported usage-window utilization.
- Live accounting: `bench/accounting/live.py` + `bench/live/live_claims.py`
  (legacy verifier included) + the three section 22 rows; contract text in
  `docs/spec/SUCCESSION_ACCOUNTING_V1.md` section 11. Over the 2026-09-24
  Codex PILOT run: nine harness failures (3 per arm), status PARTIAL,
  `counts_toward_succession` false, every row UNDETERMINED.

## PILOT 2 2026-09-24 (Claude Code, small tier; does not count)

Run `s20-640-pilot2-20260924-claude-small`, label `PILOT`, frozen at
`b3259a22` from `PREREGISTRATION-2-claude-code.json`
(`claude-haiku-4-5-20251001`, effort medium, Claude Code 2.1.280). Raw run
directory (retained, untracked):
`/home/gfarch/Work/checkpoints/sley2-campaign-runs/s20-640-pilot2-20260924-claude-small/`.
Real model turns in all three arms. Tokens are provider-reported
(input = uncached + cache creation + cache reads); cost is the provider's
`total_cost_usd` estimate (subscription usage, not billed).

| Slot | Status | Code | Wall s | Input tok | Output tok | Tools | Est. USD |
|---|---|---|---|---|---|---|---|
| REPAIR legacy | accepted | — | 229 | 132,853 | 2,079 | 8 | 0.03 |
| REPAIR sley_2_0 | rejected | ORACLE_COLLATERAL_TOUCHED | 467 | 4,216,976 | 37,358 | 84 | 0.75 |
| REPAIR raw | accepted | — | 16 | 83,169 | 1,291 | 6 | 0.02 |
| TEST sley_2_0 | harness_failure | CAPTURE_GATE_NO_FINAL | 585 | 5,249,565 | 49,577 | 107 | 0.92 |
| TEST raw | accepted | — | 29 | 118,949 | 2,469 | 8 | 0.03 |
| TEST legacy | accepted | — | 1,833 | 1,460,216 | 18,173 | 50 | 0.31 |
| CONTEXT raw | accepted | — | 23 | 121,441 | 2,375 | 8 | 0.04 |
| CONTEXT legacy | harness_failure | LIVE_PROVIDER_EVENT_INVALID | 0 (not recorded) | 0 (not recorded) | 0 | 0 | 0.29 (stream) |
| CONTEXT sley_2_0 | harness_failure | LIVE_PROVIDER_EXIT_NONZERO | 1,725 | 0 (not recorded) | 0 | 0 | 0.41 (stream) |

Findings (raw failures retained, nothing re-run into this run):

- Harness defect: Claude Code auto-backgrounded slow tool commands
  (`task_updated is_backgrounded`), then ran another query after the
  result. CONTEXT legacy's stream therefore carried two `result` records
  and failed the parser; CONTEXT sley_2_0 ended with exit 1 after the
  model killed hung background shells. Fixed:
  `CLAUDE_CODE_DISABLE_BACKGROUND_TASKS=1` (plus a 300 s default / 600 s
  maximum Bash timeout, auto-memory and CLAUDE.md loading off), and the
  parser now reads the session-cumulative `modelUsage` of the terminal
  result (tests). Preregistration revision 3 supersedes revision 2 for
  these changes, before any counted attempt.
- Harness defect: an attempt whose stream failed to parse recorded wall
  time 0; wall time and peak memory are now kept (test). The retained
  pilot record stays as written.
- Agent behavior, not harness: REPAIR sley_2_0 finished an early wrong
  candidate (the tool's `finish` writes once; later finishes were refused
  "finish exists"), then passed `stored` bytes where `finish` takes the
  record. TEST sley_2_0 never finished and then claimed it had.
- Observation for the operator: a sley_2_0 attempt that submits no final
  candidate is classified `harness_failure` (`CAPTURE_GATE_NO_FINAL`),
  while a raw or legacy agent that leaves the workspace unfixed is judged
  `rejected`. Both are failures in every denominator, but the sley_2_0 case
  is excluded from the non-harness-failure median companions. This is the
  reviewed mediated adjudication and is not changed here.
- Observation: the sley_2_0 tool surface is expensive for the small model
  (4.2–5.2M input tokens, 84–107 tool calls per attempt versus 0.08–0.12M
  and 6–8 for raw files); the legacy tool is slow (about 25–35 s per call).
- Accounting over this run: 9 attempts (raw 3 accepted; legacy 2 accepted,
  1 harness failure; sley_2_0 1 rejected, 2 harness failures), status
  PARTIAL, `counts_toward_succession` false.

## PILOT 3 2026-09-24 (revision 3 check, CONTEXT only; does not count)

Run `s20-640-pilot3-20260924-claude-small`, label `PILOT`, frozen at
`9ae2bf4f` from `PREREGISTRATION-3-claude-code.json`. Raw run directory:
`/home/gfarch/Work/checkpoints/sley2-campaign-runs/s20-640-pilot3-20260924-claude-small/`.

| Slot | Status | Code | Wall s | Input tok | Output tok | Tools | Est. USD |
|---|---|---|---|---|---|---|---|
| CONTEXT raw | accepted | — | 23 | 93,932 | 1,860 | 7 | 0.05 |
| CONTEXT legacy | rejected | ORACLE_UNBOUNDED_READ | 996 | 2,500,784 | 8,327 | 35 | 0.44 |
| CONTEXT sley_2_0 | rejected | ORACLE_IMPACT_INCOMPLETE | 1,258 | 4,842,793 | 66,610 | 79 | 1.00 |

Every stream ended on exactly one result with no backgrounded command;
all three attempts were judged by their oracles (no harness failure).
Provider-reported utilization after the run: five-hour 0.37, seven-day
0.38 (these windows also carry the operator's other Claude use).
Accounting: status PARTIAL, `counts_toward_succession` false.

Validation at the pilot 3 fixes: `bench/live/tests` 365 tests OK (the
real-`claude -p` launch test skipped in the full run; it passed separately
with `SLEY2_LIVE_CLAUDE_ROOT` bound), `bench/accounting/tests` 19 tests OK,
`scripts/check_succession_accounting.py` PASS, `make lint` PASS
(`evidence/build/lint-report.json` restored).

## Preregistration revision 4 2026-09-24 (before any counted attempt)

The pilot 2 observation above is corrected under operator-authorized scope:
an attempt whose provider exits 0 over an otherwise valid capture but ends
without submitting a final candidate is now an agent failure recorded like a
rejection (`rejected`, `AGENT_NO_FINAL`, no oracle run, no final candidate),
in the non-harness-failure median companions like a raw or legacy agent that
leaves the files unfixed. Genuine capture and harness faults stay
`harness_failure`. `PREREGISTRATION-4-claude-code.json` supersedes revision 3
and records the change (oracle digest `a9e597f7…` → `434e808d…`, from the
capture gate in `bench/live/mediated_sley.py`; no other frozen digest moved).
The pilot records above stay as written: they were classified under their
own revision and do not count.
