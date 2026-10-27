# Aggregate + review pass — 2026-09-21 (work branch only, successor to b0e52e00)

Scope: Sley only. No ZJX execution-authority change. No mint, signing,
publication, integration, or GA claim. `ga_claimed=false` preserved.
No source files change in this successor (review packets, verbatim verdict
transcripts, and this record only); lint inputs and fuzz targets untouched.

## 0. State verified

- `work/succession-sley20-arm` at `b0e52e00a2bacfd7a874f0b4a7973a352508cebe`
  (local == `origin/work/succession-sley20-arm`), tree carrying only the
  untracked review inputs filed below.
- Main worktree `/home/dev/Work/workspaces/sley2` at `8966da2e`, dirty
  (10 batch-14/operator files, incl. finding-register, claim-retirements,
  decision-dossier, ga-acceptance-report) — never touched.
- `wt2` at `acbc65f0` (`work/finish-20260918`), clean, untouched.
- Historical evidence, checkpoint stores, release artifacts, operator/resource
  holds untouched. `ga_claimed=false` verified at this commit.
- Toolchain pinned `1.93.0`, `--locked`, `CARGO_NET_OFFLINE=true`,
  `CARGO_TARGET_DIR=/home/dev/.cache/sley-agg-b0e52e00/target` (on /home),
  `UV_OFFLINE=true`, `SLEY2_MASTER_GOAL=.../greyforge-managed-home/.../Sley2.0mastergoal.md`
  (sha256 `07791368...aace`).

## 1. Aggregate gate results (disposable checkout at b0e52e00 + restored inputs)

- `make lint` → PASS, exit 0 (`lint-S-b0e52e00.log`): fmt clean, clippy 0
  warnings, report files commit `b0e52e00`, clean tree. The TRACKED
  `evidence/build/lint-report.json` still names `69907ddf`: established
  convention (re-filed only at release-candidate mints; succession commits
  never touch it) — not a defect; left as-is.
- Full `make quick` recipe driven keep-going (130/130 lines,
  `quick-Skeep-b0e52e00.log` + `quick_cmds.txt`): 125 EXIT:0, 5 failures,
  each investigated to root cause (no resolved failure re-cited as blocker):
  - #83 `build_candidate_content_report.py --check` → PASS (restored
    candidate-`69907ddf` bytes; scope explicit: proves the preserved artifact,
    not a release build of this branch).
  - #84 reproducibility → FAIL (attestation binds `69907ddf` sources; 3 new
    succession test files post-candidate). Gate working as designed; clears
    only via a new candidate build + attestation (operator authority).
  - #85 SBOM/provenance → FAIL (`records-closure-ineligible`: succession
    files post-candidate). Same class; same owner.
  - #88 decision dossier → FAIL (`test-inventory:drift`: tracked inventory is
    candidate-era, derived inventory reflects current sources). Truthful
    drift; refresh belongs to the owned `evidence-refresh` process, not ad
    hoc repair (which would blur candidate vs work-branch qualification).
  - #105 supply-chain audit → FAIL (secret-scan counters drift: tracked
    2172 files/60616143 B vs fresh 2353/66682450 B + 2 disposable-tree-only
    untracked files). NO secret findings (diff is counters only). Same class.
  - #129 `cargo check --workspace --locked` → PASS (exit 0).
  - #130 `cargo test --workspace --locked` → exit 101 with EXACTLY ONE failing
    test: `succ_debug_commit::debug_commit_repro` (panics at `:7` on missing
    `SUCC_DEBUG_REPO` env). 27 suites `ok`. Diagnostic-only disposition
    preserved (temporary helper, no fixture env authorized); scoped
    `--skip debug_commit_repro` evidence cited as scoped only, never as an
    unfiltered pass.
- Fuzz evidence reuse: D4 `fuzz-refresh-7bcc5a89.log` (7 lanes PASS) remains
  fresh — no Rust sources, fuzz targets, or lint inputs changed since (this
  successor adds records only). Nothing rerun indiscriminately; no exemption.

## 2. Review submissions (8 dispatches, 8 verdicts, no failures)

Workflow: `forge-council exec <role> "<packet>" --timeout 1800 --execute`
(engine resolved: hermes; one dry-run without `--execute` produced no model
call and was relaunched correctly). Single dispatch per round; no polling
loop, no substitution, no fabricated verdict. No quota/auth/transport
failure occurred. Batch-14 inputs and landed reviews untouched; no completed
scope redispatched.

| Round | Packet (sha256 prefix) | Lane | Verdict | Transcript (sha256) |
|---|---|---|---|---|
| REQ-09 rev1 (`97e50c9b`) | CREATE native trial admission | Ariadne | FAIL `FAIL_NEW_P1_OPEN_NO_P0` (NTA-P1-1 block; P2-1/P2-2 qualify; P3-1 advisory) | `native_trial_admission/ariadne-review-b0e52e00.md` (`2fee4a94`) |
| REQ-10 rev1 (`9f953627`) | CONTEXT bounded discovery | Nabu | REVISE 2P1/1P2/1P4 | `context_bounded_discovery/nabu_architecture_review-b0e52e00.md` (`2eb8fef9`) |
| REQ-09 rev2 (`dd9a02ce`) | answers P1-1/P2-1/P2-2/P3-1 | Ariadne | FAIL, priors CLOSED, new NTA-P1-2 block | `.../ariadne-review-b0e52e00-rev2.md` (`76503db0`) |
| REQ-10 rev2 (`a7aec1e6`) | withdraws item 2, bootstrap respec | Nabu | REVISE narrowed (option-B/head-divergence/cost) | `.../nabu_architecture_review-rev2-b0e52e00.md` (`0ea0d788`) |
| REQ-09 rev3 (`248852dc`) | answers NTA-P1-2 (full probe shape) | Ariadne | PASS `PASS_QUALIFYING_NO_P0_NO_P1`; new NTA-P2-3 qualify (fold into delta text, no rev4) — CLEARED FOR IMPLEMENTATION | `.../ariadne-review-b0e52e00-rev3.md` (`321cfc06`) |
| REQ-10 rev3 (`b817d219`) | withdraws B, head-pinned field 9 | Nabu | REVISE single-issue (reachability) + 2P2 + P3 | `.../nabu_architecture_review-rev3-b0e52e00.md` (`82caefdc`) |
| REQ-10 rev4 (`f3b64457`) | remedy (a): afford workspace.open + probe spec + placement | Nabu | REVISE 0P1/3P2/2P3 — architecture settled, record completion | `.../nabu_architecture_review-rev4-b0e52e00.md` (`09d41abc`) |

Process notes: Nabu rev1 executed `cargo test ... s3_g2_context` inside a
read-only-constrained session (standing reviewer-execution limit); recorded
factually. Reproduced independently here (1 passed, 2 ignored, ~3s); rev2+
rounds complied (no execution). Two Nabu relay lines truncated at 2000 chars
(P1 remedy list tail, SUMMARY tail); rev4 confirmed no lost preferred option.

## 3. Trace corrections owned (not relitigated, fixed in packets)

- REQ-10 rev1 "no served impact route" was WRONG: class 14
  `ReverseImpactClosure{seeds}` (plus classes 13/15-19) decodes at
  `server.rs:3465` on served `query.root`. Method-name grep missed class tags.
- PROFILE_ARGS consumers: nine uses (1 def + 9), not eight.
- `revision_summary` carries fields 1-8 only (field 9 genuinely free);
  `query_root` head pin is `server.rs:2685`, not `:2665`.
- `STALE_ROOT` pinned at exactly one judge site (`:2457-2460`); no other
  frozen oracle in the judge pins it (Ariadne escalation answered).

## 4. CREATE native admission — cleared, exact next work

- Contract cleared (Ariadne rev3 PASS). Fold NTA-P2-3 into delta text at
  implementation: stale probe field[3] pinned to
  `fixed_native_admission_profile().id()` (code-determined; loud failure).
  Re-open conditions: probe candidate change, cross-probe attempt_id reuse,
  frozen-symbol/exact-pin change. Vulcan (authority provisioning) and Nabu
  (executor wiring) implementation reviews follow as scoped.
- Implementation plan (per cleared delta): trial-only serve flag provisioning
  `NativeAuthority` as one unit (supervised-worker executor + test signer +
  receiver test manifests; never release/caller keys) + `NATIVE_TRIAL_PROFILE_ARGS`
  + per-profile stamp guard (allowlist digest frozen) + profile-gated judge
  native encoder (both commit sites `:1188`, `:2449`; fresh session at H1 +
  distinct attempt_id per probe) + checker/spec pin updates.
- BLOCKER for the integrated proof (physical, not contractual): no executor
  on this host can genuinely execute authored tests — supervisor service,
  `/usr/lib/sley`, `/etc/sley-test-supervisor`, `/run/sley-test-supervisor`
  all absent; all `NativeTestExecutor` impls are test-only stubs; no
  worker-outcome→`ExecutedNativeTest` bridge exists. Legacy refusal stays
  unchanged; old profile bytes/digests untouched.

## 5. CONTEXT discovery — architecture settled, record completion specified

- Nabu rev4: no P1; remedy (a) closes reachability (afford `workspace.open`,
  carry accepted-head snapshot identity; pure read pinning accepted head).
  Remaining record items (cheap, same-record): 18-name count gate + spec §9
  edit + contract revision bump (precedent `PHASE3_V2_OFFER_DESIGN.md:76-83`);
  scope field-9 encoding to the workspace.open path (`revision.read` bytes
  unchanged); extend `TOOL_METHODS`, drop the harness relay, author/strike
  the pinning-test claim; Merlin owns the index-domain probe
  (`read_record`+`accept_cached`, no fresh fallthrough); rewrite the
  over-broad denial rationale (`runner.py:116-118`, `test_runner.py:541-546`).
- Witness manifest-literal reads (`succ_witness_context.py:65-67`) are what
  the interface must replace; F1/Bool and private roles stay retired.

## 6. CHECKPOINTED_IN_PROGRESS — exact next actions

1. CONTEXT: answer rev4's three P2s in the same record (allowlist/spec/bump;
   encoder scoping; TOOL_METHODS+relay+pin test) with Merlin probe ownership
   → Nabu sign-off (no redesign expected) → implement (afford workspace.open,
   field 9, probe, tooling, task-input typedef amendment) → integrated
   execute_attempt→discovery→mutation→admission→oracle→append→verify proof
   incl. all five negative classes → Vulcan/Ariadne follow-ons.
2. CREATE: implement the cleared delta incl. NTA-P2-3 fold-in → provision a
   genuine executor (host supervisor install = operator/infra authority, or
   Vulcan-reviewed trial executor) → integrated proof with real execution of
   authored tests (wrong-expectation/authority/stale/failed-execution
   negatives) → Vulcan/Nabu implementation reviews.
3. Revalidate any code-landing successor with the §1 aggregates (lint + full
   130-line quick + affected fuzz lanes); candidate-bound records (#84/#85/
   #88/#105) stay fail-closed until an operator-owned refresh/re-attestation.

Remaining holds (unchanged owners): MODULE/MERGE/CORRUPT dispositions, TYPE
main adoption, DEAD tombstone semantics, EFFECT/CAP rev16, lab coordination,
AR-02, R2, C1, release signing/publication/GA, campaign evidence. Component
proof, trial capability, independent review, release qualification, and
campaign evidence remain separate.
