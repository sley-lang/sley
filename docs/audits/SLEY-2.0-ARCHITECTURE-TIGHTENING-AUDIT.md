# Sley 2.0 Architecture Tightening Audit

Status: IN_PROGRESS (activated 2026-09-08 06:52 UTC). Governing specification:
`/home/greyforge/machineresearch/SLEY-2.0-ARCHITECTURE-TIGHTENING.md`
(sha256 `861e57f9e35de315bed88ee8bc2e946854b8a60bdeaf897f6bc1e9e9ccb805ac`).
Campaign branch: `arch/tighten-r1`, worked in a pinned worktree so the two
in-flight lane sessions on `main` are never read while moving.

Every finding below carries exactly one disposition from the spec's section 3
table. Nothing is implemented without a classification, and a concern the
repository already resolves is recorded as `A_ALREADY_SOLVED` with proof.

## 1. ARCH_TIGHTEN_BASELINE (AT-G0)

```text
ARCH_TIGHTEN_BASELINE:
  canonical_repo:                 /home/greyforge/sley2
                                  origin https://github.com/GreyforgeLabs/sley.git (PUBLIC)
  canonical_branch:               main
  baseline_commit:                560a5f16ebe9edaaee6837b779f6af94ad9ae310
  baseline_parent:                5044168a448b46de22b29189d039bd1cc3b3056a
  baseline_subject:               rw-080 slice 7 record: Namespace body Sley decode/encode
                                  C0 construction record (2026-09-08 02:43 -0400)
  reweave_master_commit_or_digest:
    path:    /home/greyforge/machineresearch/SLEY_2X_REWEAVE_MASTER_SPEC_V1.md
    rev:     REWEAVE-1.0 (2026-09-05)
    sha256:  61d20471906b00fab05a9e0f174ad9c616e734a5f38fd590ed44af2533535a63
    mirror:  /home/greyforge/machineresearch/SLEY_REWEAVE_MASTER_PACKAGE_V1/
             sley-reweave-v1/SLEY_2X_REWEAVE_MASTER_SPEC_V1.md (byte-identical)
    adopted: docs/adr/ADR-0049-reweave-scope-adoption.md (2026-09-06)
  sley_2_0_master_commit_or_digest:
    path:    /home/greyforge/machineresearch/Sley2.0mastergoal.md
    name:    Sley 2.0 Machine Genesis (Black Loom)
    sha256:  e1e153468316d56e111af0f6f3bffa0d66601e3f5e2fe898a5af7fdfb14fc719
    in-repo: machineresearch/sley-2.0/00..25-*.md dossier chapters,
             machineresearch/sley-2.0/machine-summary.json
  active_schema_epoch:            SSMC1 epoch 1, bootstrap SchemaEpochId
                                  ae5b235713b46c04f73c1decd0fb0bb57c5557d0fe89dae7ddac4a7dba25564e
                                  (conformance/schema-epoch/v1/bootstrap.json sha256
                                  6c1c4396b99bb57f3d646fe5938f5393e606c8a5323f609d48c0bfaad4e1223a)
  active_host_abi:                HOST_ABI_V2, contract sley2-host-abi-2, digest
                                  bc564653302a73eb5f998427250a2bb7cd87f5685ef12619bd4ae1f1b2af70d5
                                  (HOST_ABI_V1 preserved byte-identical as history)
  active_exec_package_version:    EXEC_PACKAGE_V2, contract sley2-exec-package-2, digest
                                  f4958c5e3d57762173b881288b008af17d45b5f07a431fcc442d9eec5770da94
  active_bootstrap_profile:       BOOTSTRAP_PROFILE_2, contract sley2-bootstrap-profile-2, digest
                                  fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459
                                  (bootstrap-manifest.json still records P = BOOTSTRAP_PROFILE_1;
                                  see the spec-synchronization findings)
  self_host_claim_level:          SH0 (bootstrap-manifest.json: S, C1, C2, C3 do not exist;
                                  C0 image not yet snapshotted; RW-080 slices 1..7 PROVISIONAL)
  tree_state:                     CLEAN at baseline_commit; one untracked local-only directory
                                  `.forge/slices/` (slice-contract scratch, not repository truth)
  remote_state:                   origin/main = 1d573a07066e29ac6e96c6dfa72c46e086365031
                                  (90 commits, sanitized public mirror v2.0.0-alpha.1,
                                  pushed 2026-08-30T06:13:05Z); NO merge base with main;
                                  590 local commits never pushed to any remote
  open_reviews:                   finding register FINDING_REGISTER_OPEN (218 obligations,
                                  69 open reviews, 10 deferred, 2 unclassified);
                                  RW-080 BLOCKED pending Nabu round-12, premium round 2,
                                  S20-780 independent acceptance; S20-770 lane under
                                  Council re-review (lane2 branches, 14 to 21 commits
                                  ahead of an older main)
  open_gates:                     R2 exit NOT_READY; C0->C1->C2->C3 succession not started;
                                  release-candidate-smoke re-attest pending since S20-330
                                  closure (RESUME.md 2026-09-05); five operator gates
                                  (narrowed schema-epoch decision, succession trials,
                                  root license text, second-host attestation, release
                                  decision); publication_authorized = false
```

## 2. Activation boundary and exclusions

Two Sley 2.0 sessions were still committing when this campaign activated
(last `main` commit 02:43 local, last lane worktree commit 02:41 local,
activation 02:52 local). Per spec section 1 they are not interrupted:

- **Lane 3, RW-080 (REWEAVE codec construction slices).** Commits land on
  `main` directly plus scaffold branches `lane3/rw080-*`. Owned surface:
  `crates/sley-vm/tests/rw080_*.rs`, `machineresearch/sley-2.0/reweave/rw-080-*.md`.
  EXCLUDED from this campaign's edit scope. Audit reads only the committed
  baseline.
- **Lane 2, S20-770 (required-contract index enforcement).** Branches
  `lane2/s20770-*`, not yet merged. Owned surface:
  `docs/spec/REQUIRED_CONTRACT_INDEX_V1.md`, `scripts/check_required_contract_index.py`.
  EXCLUDED from this campaign's edit scope; findings that touch the index are
  recorded for that lane rather than edited here.

This campaign edits only the paths it owns: `docs/audits/SLEY-2.0-ARCHITECTURE-TIGHTENING-*`,
new files under `scripts/` and `docs/status/` named in accepted findings, and
the exact spec or contract patches of accepted `B`/`C` findings, each in its
own commit. Merge into `main` happens by fast-forward or plain merge only,
after the lane sessions have produced stable handoffs.

## 3. Finding register

Populated per audit area as each mandatory audit closes. Record format is the
spec's section 18 block. Identifiers: `AT-SI` semantic identity, `AT-EC`
executable closure, `AT-NA` native authority, `AT-IG` identity graph, `AT-HH`
hash hygiene, `AT-MW` machine write, `AT-CL` conformance layers, `AT-BG`
BLACKGLASS, `AT-WB` Witness boundary, `AT-SS` spec synchronization, `AT-RF`
remote freshness.

### 3.1 Remote freshness and inspectability (spec sections 15 and 16)

```text
FINDING_ID: AT-RF-01
TITLE: The GitHub remote holds an unrelated sanitized lineage; the active Sley 2.0 history has never been pushed anywhere
HYPOTHESIS: The active development branch is remotely inspectable at validated commit boundaries.
REPOSITORY_EVIDENCE:
  git merge-base main origin/main -> none (local root 3a0fd1b4, remote root ca6ef13c)
  git rev-list --left-right --count main...origin/main -> 590 ahead, 90 behind
  origin/main 1d573a07 "docs: establish canonical Sley 2 identity"; 9bb8eb88
    "chore: authorize sanitized public source mirror (v2.0.0-alpha.1)" (2026-08-28)
  gh repo view GreyforgeLabs/sley -> visibility PUBLIC, pushedAt 2026-08-30T06:13:05Z
  origin/main:repository-policy.json forbids "private evidence",
    "machine-local absolute paths", "operator-only authority files";
    origin/main:scripts/check_public_repository_policy.py enforces it in CI
  local main: 73 tracked files contain /home/greyforge paths,
    163 Council review transcripts under machineresearch/sley-2.0/reviews/,
    machine-summary.json publication_authorized = false
  scripts/check_remote_consistency.py --repo /home/greyforge/sley2 --branch main ->
    HISTORY_RELATED = False, RESULT = FAIL (UNRELATED_HISTORY, BEHIND_UPSTREAM,
    AHEAD_UNDECLARED, UNTRACKED_FILES)
SPEC_EVIDENCE: SLEY-2.0-ARCHITECTURE-TIGHTENING.md 15.2 (remote SHOULD lag by at most one
  bounded unit; handoff commit MUST be on GitHub unless a documented safety reason prevents
  it), 15.3 (never push private qualification material; never claim GitHub is current when
  validated commits are local only), 15.7 (private repository requires an authorized
  connection; the project MUST NOT weaken privacy to improve review convenience);
  ~/machineresearch/reference/open-source-rules.md pre-push scrub checklist
CURRENT_BEHAVIOR: No remote reviewer can inspect any of the 590 active-lineage commits.
  The only remote is a public mirror cut by history rewrite on 2026-08-28, nine days and
  590 commits stale, and the repository's own public policy forbids pushing the active
  lineage to it unsanitized.
DESIRED_INVARIANT: The active branch and every validated handoff commit resolve on a GitHub
  remote whose visibility matches the content, with the full SHA citable in review packets.
DISPOSITION: C_PRE_FREEZE_REPAIR
RATIONALE: A material remote-inspectability defect that section 15 names as an engineering
  requirement. It is not solvable inside this session: a plain push is forbidden by the
  recorded public policy, and both remedies are outward-facing decisions reserved to the
  operator. The documented safety reason (15.2) for the current gap is recorded here.
  Smallest correct repair, recommended: create a PRIVATE GitHub repository for the active
  lineage (for example GreyforgeLabs/sley2), set it as the upstream of main, push main and
  arch/tighten-r1, and keep GreyforgeLabs/sley as the sanitized public mirror refreshed only
  by the existing rewrite pipeline under a separate publication decision. Alternative:
  refresh the public mirror now, which is a publication act and stays gated by
  publication_authorized = false.
CANONICAL_IMPACT: none (repository hosting only)
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: operator authorization for the private remote (or for a mirror refresh);
  then `git remote` reconfiguration and a first push; no history rewrite of main
TESTS_REQUIRED: scripts/check_remote_consistency.py PASS on main after the push
  (HISTORY_RELATED = True, BEHIND_COUNT = 0); public-policy checker stays green on the mirror
SPECS_TO_UPDATE: remote-head locator (AT-RF-03) once the remote exists; RESUME/handoff packet
CODE_OWNERSHIP: operator (remote creation); campaign integrator (remote configuration, push)
REVIEW_REQUIRED: operator decision; AT-G7 cannot pass before it
```

```text
FINDING_ID: AT-RF-02
TITLE: No repository check reported LOCAL_HEAD, UPSTREAM_HEAD, ahead/behind, dirty state and untracked files at handoff
HYPOTHESIS: A lightweight consistency gate exists (spec section 16).
REPOSITORY_EVIDENCE: `ls scripts | grep -iE 'remote|git|status'` at 560a5f16 -> only
  scripts/gate_status.py, which prints a fixed NOT_IMPLEMENTED record and exits 2; no
  Makefile target inspects the remote relation; handoffs in RESUME.md cite commits without
  push status.
SPEC_EVIDENCE: SLEY-2.0-ARCHITECTURE-TIGHTENING.md 16 (required fields; expected handoff
  state; AHEAD_COUNT > 0 only when declared; the checker SHALL NOT auto-push).
CURRENT_BEHAVIOR: Remote state was discovered by hand and the 590/90 unrelated-history gap
  went unreported in every handoff since 2026-08-28.
DESIRED_INVARIANT: One deterministic, read-only command answers the section 16 fields and
  fails closed on dirty tree, behind upstream, undeclared ahead, unrelated history, missing
  upstream, or untracked relevant files.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Purely additive tooling with no canonical, schema, ABI or self-host effect; the
  audit itself needed it to state the baseline truthfully.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: none
TESTS_REQUIRED: `python3 scripts/check_remote_consistency.py --self-test` (8 scenarios in
  throwaway repositories: clean pushed, dirty, ahead undeclared, ahead declared, untracked,
  behind, unrelated history, no upstream) -> SELF_TEST PASS; live run on main reproduces
  the AT-RF-01 state
SPECS_TO_UPDATE: none (Makefile target `remote-consistency`, kept out of `quick` because a
  branch is legitimately ahead mid-slice; run at handoff)
CODE_OWNERSHIP: scripts/check_remote_consistency.py, Makefile target remote-consistency
REVIEW_REQUIRED: independent review at campaign close (AT-G8)
```
