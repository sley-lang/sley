# Review rounds rev6-rev9 + gate pass — 2026-09-22 (successor to 95d42fda)

Scope: Sley only. No ZJX execution-authority change. No mint, signing,
publication, integration, or GA claim. `ga_claimed=false` preserved.
Records-only successor (4 packets + 3 transcripts + this record);
staging, capture, typed-driver, lint inputs, and fuzz targets
untouched. No restart of completed work. Commit `10becd4d`
carries the round records; this record lands in the successor.

## 0. State verified before work

- `work/succession-sley20-arm` at `95d42fda691bc5d8ec468478529b7b174d3a8759`
  (local == origin), clean tracked tree. Expected checkpoint met
  exactly; no newer legitimate work to preserve beyond it.
- Main worktree at `8966da2e`, dirty (21 files), never touched. `wt2`
  at `acbc65f0`, clean, untouched. `ga_claimed=false` verified.
  Batch-14 inputs/index untouched; all review traffic kept separate
  from batch 14 (single dispatches, no polling, no substitution;
  REQ-09 not redispatched).
- Toolchain pinned `1.93.0`, `--locked`, `CARGO_NET_OFFLINE=true`,
  user-owned target dirs under `/home` outside the repo;
  `SLEY2_MASTER_GOAL` stable (`07791368...aace`). Tracked
  `evidence/build/lint-report.json` still names `69907ddf` per
  succession convention (re-filed only at release-candidate mints);
  restored after each lint run, never committed.

## 1. Review submissions (4 packets, 3 verdicts, 1 provider failure)

Baseline for all rounds: `95d42fda` (sources identical to `c1e8db56`;
records-only delta). Lane Nabu, section `context_bounded_discovery`.

- REQ-10 rev6 (`2ba40e08...`, 225 lines; answered rev5 P1/P2s/P3/P4
  incl. the `95d42fda`-subject `0P1` forward-record correction) →
  Nabu `REVISE_0_P0_1_P1_4_P2_2_P3_1_P4` (dispatch exit 0, engine
  hermes; transcript `.../nabu_architecture_review-rev6-95d42fda.md`,
  sha256 `4f8de2f0...` incl. writing-lane PACKET_SHA256 record).
  Architecture settled; five mechanical transcription/ordering/
  convention/coverage/mechanism defects.
- REQ-10 rev7 (`24d2b505...`, 213 lines; hoisted/converted/anchored
  extraction, field-name convention, widened floor, mechanism (a),
  namespace statement) → Nabu
  `REVISE_0_P0_1_P1_2_P2_1_P3_0_P4` (exit 0; transcript
  `.../nabu_architecture_review-rev7-95d42fda.md`, sha256
  `590df37e...` incl. hash record). Fixes 3/5 confirmed complete;
  four new mechanical faults with inline remedies.
- REQ-10 rev8 (`01153512...`, 135 lines; adopted all four inline
  remedies: `:178` hoist, TOOL_METHODS edit + folded D1, revision-5
  binding start, `on <40-hex>` notes) → Nabu
  `REVISE_0_P0_0_P1_2_P2_2_P3_1_P4` (exit 0; transcript
  `.../nabu_architecture_review-rev8-95d42fda.md`, sha256
  `e617e0c1...` incl. hash record). P1 cleared; Fix 1 and the note
  template verified correct by mechanical reconstruction (AST parse
  + regex execution in reviewer scratch); ownership call ratifying
  transcript-less rev4 approvals recorded. Remaining: pin test now
  required (rev8's cost premise falsified by `sley2_tool.py:51`
  module-level runner import); `ALLOWED_COMMANDS` omission
  (mediated lane); uncommitted-chain record fault (P3-1); capture
  vocabulary sentence (P3-2).
- REQ-10 rev9 (`dbfe12f3...`, 134 lines; all four rev8 remedies:
  pin test with withdrawn premise, edit 4.1, vocabulary sentence,
  assumption-4 confirmation, record-state statement) → NO VERDICT.
  Attempt 1: tool-level 30-min timeout, empty transcript, no engine
  process surviving. Attempt 2 (single retry, same command):
  `forge-council exec` internal `--timeout 1800` raised
  `subprocess.TimeoutExpired`, exit 1, empty transcript
  (`/tmp/nabu-rev9-dispatch2.err` retained). Real provider/engine
  execution failure, retained as the result; no further retries (no
  polling loop). Rev9 packet committed in `10becd4d` awaiting
  disposition.

Record-closure note: rev8 P3-1 is closed structurally — rev6/rev7
packets and transcripts are committed in `10becd4d`, so the next
packet's SUPERSEDES names committed paths. Rev9's Item 5 stated
"untracked at dispatch, landing in the round commit": the commit
fulfilled it before any verdict, as the item allowed ("in or before
this round").

## 2. CREATE (carried, not started this pass)

Cleared REQ-09 rev3 disposition stands (implementation clearance +
NTA-P2-3 fold-in: stale-probe field[3] pinned to
`fixed_native_admission_profile().id()`; failure loud via
`TXN_RECEIPT_BINDING_MISMATCH`). No CREATE code touched this pass:
the turn was consumed by four CONTEXT review rounds plus the
provider failure, and mixing an uncleared-CONTEXT checkpoint with
partial CREATE plumbing would be incoherent. CREATE's genuine
executor dependency is unchanged (no host supervisor, no worker-
outcome bridge; 25 unit tests not re-counted as proof). CONTEXT was
not gated on CREATE at any point.

## 3. Aggregate checks

| Check | Command (wt-succ) | Exit | Result |
|---|---|---|---|
| Formatting+clippy+checkers | `make lint` (pinned env, `CARGO_TARGET_DIR=/home/dev/.cache/sley-agg-b0e52e00/target`, `UV_OFFLINE=true`) | 0 | PASS at `10becd4d` (fmt clean, 0 clippy warnings, clean tree; tracked lint-report restored after) |
| Full `make quick` / fuzz | — | not rerun | Reused per freshness rules: records-only delta, no Rust/Python/spec/fuzz/lint-input change since the `c1e8db56` PASS. Candidate-bound rows stay fail-closed |

## 4. CHECKPOINTED_IN_PROGRESS — exact next action

Re-dispatch REQ-10 rev9 (packet `dbfe12f3...`, committed at
`10becd4d`) via `forge-council exec nabu ... --execute` — the two
timeouts were engine execution failures returning no verdict, not a
disposition. Baseline note for that dispatch: branch tip is now
`10becd4d`; sources are identical to the packet's stated `95d42fda`
baseline (records-only delta), so re-dispatch as-is with the tip
recorded in the dispatch log, or restate the baseline if the lane
requires it. Then, per the returned disposition: on clearance,
implement the approved delta (Fix 1 hoist, gate extension +
`completion-unbound-review`, revision-5 binding, allowlist/tool/
mediated/checker/spec/summary/closeout/WORK_PACKAGES edits, pin
test, vocabulary note, both-direction regression demonstrations) →
integrated discovery+repair proof incl. all five negative classes →
Vulcan/Ariadne follow-ons; on REVISE, answer it the same way.

Remaining holds (unchanged owners): MODULE/MERGE/CORRUPT, TYPE main
adoption, DEAD tombstone, EFFECT/CAP rev16, lab, AR-02, R2, C1,
release signing/publication/GA, campaign evidence. Component proof,
trial capability, independent review, release qualification, and
campaign evidence remain separate.
