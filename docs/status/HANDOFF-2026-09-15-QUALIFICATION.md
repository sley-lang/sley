# Sley 2.0.0 qualification handoff, 2026-09-15

Written at a safe checkpoint on operator request. Everything below is
verified on disk; nothing is predicted.

## Where things stand

| Thing | State |
|---|---|
| Live checkout | `/home/dev/Work/workspaces/sley2`, branch `main`, HEAD `70283ce`, clean, pushed (`origin/main == HEAD`; the remote reports the repository was renamed to `sley.git` under the same owner, the push to `sley2.git` still fast-forwards) |
| Campaign worktree | `/home/dev/Work/workspaces/sley2-campaign`, branch `campaign/s20-640` at `70283ce`, clean, no work committed yet |
| Attested candidate | `7a94a4a` -> artifact `6d970bf4…e159` (2,190,465 bytes, 15 members), MULTI_HOST_REPRODUCIBLE (primary + lab `secondary-host`), toolchain 1.93.0 both |
| Candidate staleness | HEAD carries attestation-bound changes after `7a94a4a` (`docs/spec`, `scripts/`, `crates/sley-query`), so `check_reproducibility_and_independent_conformance` and the standards checker's closure branch report stale/ineligible until the next mint. Expected; re-mint after the review cycle lands. |
| Finding register | 375 obligations, 1 PENDING (`root_backed_query_profile.contract_text_review`, closes when the three lanes review revision 6) |
| GA acceptance report | 48 EVIDENCED, 2 AWAITS_REVIEW (S20-740 independent review; register CLEAR), 2 GATED (section 22 thresholds; final commit fixed by a release decision). States are now derived from recorded evidence, not hard-coded (`f7df74f`). |
| Decision dossier | BLOCKED (1 open obligation; 4 GA criteria not evidenced; no succession trial; product gates fail-closed by design) |
| Terminal statuses | S20-250 full, 260/270 extended, 300 full, 320 full, 330, 400/410, 420, 430, 510, 520, 620, 630, 720 COMPLETE; S20-760 POLICY_ACCEPTED; S20-770 INDEX_ACCEPTED |
| `make quick` at `70283ce` | green except the staleness tripwire named above |

## What this session did (commits `f7df74f`, `70283ce`)

- `ROOT_BACKED_QUERY_PROFILE_V1.md` revision 6 closes the round-7 wording
  packet under the 2026-09-15 authorization (sections 3, 7, 9, 10) with a
  paging-layer unit walk `single_item_classes_walk_two_items_at_limit_one`.
- `build_ga_acceptance_report.py` derives every section 26 criterion from
  recorded evidence; `build_decision_dossier.py` derives the security-review
  and independent-review items from recorded verdicts.
- S20-760 and S20-770 accepted after three-lane final PASS; zero-finding
  PASS forms normalized to the bare `PASS` token the acceptance checkers
  bind (history kept in `_note` fields); reproducibility Vulcan lane
  rotated; a self-contradicting root-query verdict value corrected.
- Standards SBOM: revision pointer aligned, bound-inputs sentence
  rewritten, checker reports one cause on an ineligible closure, admission
  test renamed to its claim.
- WORK_PACKAGES rows of the completed packages, the frontier audit (dated
  update), and a superseding handoff record
  (`~/.config/greyforge/agents/handoffs/2026-09-15-sley2-qualification-reconciliation.json`)
  reconciled to the live state.

## In flight when this note was written

Three independent review agents (Ariadne, Nabu, Vulcan roles, Claude Code
harness, read-only) were dispatched against scope `70283ce` for four
sections each: `root_backed_query_profile` (revision 6),
`standards_sbom_and_provenance`, `reproducibility_and_independent_conformance`,
`decision_dossier`. They write only transcripts under
`evidence/review/verdicts/<section>/<lane>-70283ce.md`. Whoever resumes:
check for those twelve files; if present, read their footers and record the
lane fields in `machineresearch/sley-2.0/machine-summary.json` per the rules
in `RESUME.md`, then rebuild the register and dossier. If absent, dispatch
the lanes again with the brief at `/tmp/claude-sley2/review-brief.md`
(recreate it from `RESUME.md` if `/tmp` was cleared).

## Next steps in order

1. File the twelve verdicts; when all lanes PASS: `finding_register`
   `independent_review` needs its own S20-740 independent review (a fresh
   Vulcan-role lane over the CLEAR register), then `S20_740_COMPLETE`;
   `reproducibility_and_independent_conformance` -> `S20_730_COMPLETE`;
   `standards_sbom_and_provenance` stays REVIEW_PENDING until signing.
2. Re-mint on both hosts (procedure in `RESUME.md`), pin the candidate,
   `make evidence-refresh`, `make quick`, commit, push.
3. History re-anchor at the release candidate: move `HISTORY_ANCHOR` in
   `scripts/generate_supply_chain_evidence.py` to the candidate's parent
   so the re-anchor commit is itself the candidate, prove zero deletions
   between anchor and candidate, re-mint.
4. Succession campaign (S20-640). Design findings so far, none implemented:
   - No task fixture exists for any of the 15 corpus tasks in any arm; the
     Sley 2 arm's only fixture is the S20-540 genesis exchange (a Bool
     program). Fixtures must be authored: SSMC1 programs via a Rust
     emitter (pattern: `crates/sley-repo/src/exchange.rs`
     `emit_repository_exchange_vector_for_fixture_refresh`), Sley 1.x
     `.sley` sources for the legacy arm, and a raw-file representation
     (undecided; Python 3 source with unit tests is the natural
     "ordinary raw-file editing" baseline; this is a benchmark-design
     choice that needs a council review and should be confirmed by the
     operator).
   - The frozen S20-610 manifest admits only `execution_mode:
     offline_injected` / `external_command_policy: forbidden`; a live
     model call is neither. A contract revision must add a
     `live_provider` mode with per-arm declared execution surfaces
     (`sley`, frozen `bin/sley`, `python3`) and a pinned model driver.
   - Model driver: `codex exec --json -m <model> --skip-git-repo-check`
     works non-interactively and reports `turn.completed` usage
     (verified: `gpt-5.6-sol` answered a probe; ~18.7k fixed input
     tokens of system overhead per turn, equal for every arm). Claude
     Code `claude -p --output-format json` is the alternative but shares
     this session's rate-limited subscription. Small/large model pair
     candidates: `gpt-5.6-sol` / `gpt-6-astra` (availability of the
     latter not yet probed).
   - Sley 2 arm agent tooling: expose the two-operation endpoint handle
     to the model through an MCP bridge, with typed encode/decode helpers
     built on `oracle/scb1/src/sley2_scb1_oracle` (pattern:
     `bench/sley2/entity_read_edit_demo.py`); record every model event as
     raw evidence; any shell command in the Sley 2 arm is a privileged-
     context harness failure.
   - Legacy arm: full S20-600 runner is unbuilt (task mapper, workspace
     staging beyond `--version`, containment, oracle via `bin/sley run`).
   - Two corpus tasks (S2B-EFFECT-001 adapter replay, S2B-CAP-001 runtime
     capability denial) depend on E7 adapter execution that the 2.0
     contracts gate on a schema-epoch decision held by the schema owner;
     they can be run and will fail honestly in the Sley 2 arm unless that
     decision is made.
5. Signing and transparency: S20-710 contract excludes them; a revision
   plus an operator key decision are required. Nothing signed.
6. Release decision: operator's; the dossier derives it.

## Operator decisions still needed

- Schema-epoch decision for E7 adapter execution (schema owner).
- Raw-arm representation for the succession benchmark (confirm Python 3
  source + unit tests, or name another).
- Model pair and spend for the campaign runs (Codex OAuth lane proposed).
- Signing key and transparency-log choice; release/publication decision.
