# Operator decision record: ADOPT-REWEAVE-2026-09-06 (RECORDED)

## The decision (verbatim, 2026-09-06)

> OPERATOR DECISION — ADOPT-REWEAVE-2026-09-06
>
> I APPROVE adoption request ADOPT-REWEAVE-2026-09-06, exactly within
> the scope reviewed and accepted in:
>
> independent-review/review-addendum-rev2-2026-09-06.md
>
> Read the actual adoption-request.json, reviewed governance diff,
> validation plan, addendum, and applicable repository instructions.
> Bind this decision to their exact existing identities/digests.
> Do not silently substitute a revised request or expanded diff.
>
> REWEAVE-1.0 is the adopted implementation goal for the existing
> Sley Machine lineage. Preserve the current implementation as the
> bootstrap/reference foundation. SH2_SELF_HOSTED_TOOLCHAIN and
> amended WITNESS are required; full MCR1 remains deferred.
> The RW-240 Coherence design-only audit remains required.
>
> This is an operator scope-adoption decision, not a test result,
> independent review PASS, or release authorization.

(The decision's operative sections 1-4 and limits, as issued, govern the
recording below. No signature, credential, or token is attached to this
file: the authority is the operator's issued message itself, quoted here so
nothing is paraphrased into a broader grant. This file records; the operator
decided.)

## Identity bindings (no substitution)

- Request: `reweave-r1/adoption-request.json` sha256
  `f6348b25fd65ede6cbff76adbae2ffeb418a4fff0d8843091117610f14e6f103`
- Reviewed diff + validation plan:
  `independent-review/proposed-governance-diff.md` sha256
  `d2ce79d5a8a90517c3fbee2d466a4fc2295cb64e6d0cdb002969846ee0f0f937`
- Accepting review:
  `independent-review/review-addendum-rev2-2026-09-06.md` sha256
  `52fd47c51914f892f972ebe513af2dfbba7070979cc6a2fa59a5e49decd0c52c`
- Starting implementation verified before any write: `/home/dev/sley2`
  `main@9b7054c10e29d06833f3af832eb6bf656995d6fa`, clean tree, no stashes,
  branch `main`; standing ahead/behind vs `origin/main` is the known
  local-only state, not a mismatch; nothing reset or synchronized.
- Recorded in ADR-0049 (`docs/adr/ADR-0049-reweave-scope-adoption.md`),
  append-only, naming the prior policy texts.

## Scope applied (exactly the reviewed items 1-4, nothing else)

- `CONTRIBUTING.md`: appended the reviewed scoping sentence only; all other
  prohibitions byte-identical (Ariadne PASS).
- `docs/ANTI_GOALS.md:30`: row re-scoped to authorized-campaign wording with
  the reviewed acceptance evidence (Ariadne PASS).
- `scripts/build_anti_goal_conformance.py`: six-crate denylist unconditional
  and untouched; added `evaluate_campaign_declarations` (Nabu/Vulcan PASS
  round 2 after two exact fail-closed corrections: OSError guard on the
  boundary read; gate must be a non-blank string).
- No compiler/runtime semantic change, no engine promotion, no SH3, no full
  MCR1, no spend, no publication/deploy/release/tag/push, no checker
  weakening, no retroactive approval, no blanket permission.

## Focused validation plan (exact retained results)

- (a) `build_anti_goal_conformance.py` write: PASS (25 goals, HOLDS 9,
  REVIEW_ONLY 16, violations []); `--check`: PASS (same counts).
- (b) `build_ga_acceptance_report.py` write: PASS (52 criteria, EVIDENCED 34,
  AWAITS_REVIEW 16, GATED 2 — gate states unchanged, still fail-closed);
  `--check`: PASS. The write refreshed two stale register-derived strings
  (210→216 obligations, 73→67 open) and the unpushed-commit count
  (523→543); all are pre-existing drift from the S20-330 re-review and later
  local commits, unrelated to this amendment.
- (c) Probe (real module code, fake lock in /tmp): lock `{sley-canon,
  wasmtime}` → native hit `[wasmtime]` → VIOLATED. Denylist trips.
- (d) Probes (real `evaluate_campaign_declarations`, /tmp fixtures):
  no-registry → `(True, 'no declared SH2 work items')`;
  declarations-without-boundary → False; digest-mismatch → False;
  exact-citation-plus-gate → True; gates `true/1/{}/[]/'   '/null` → all
  False with 'names no staged SH2 gate'; directory-at-boundary-path → False.
- (e) Independent lane reviews by non-author reviewers: Ariadne PASS (round
  1); Nabu FAIL round 1 (High: unhandled boundary read; Medium: truthy gate)
  → corrected exactly → Nabu PASS round 2; Vulcan FAIL round 1 (Medium:
  truthy gate) → corrected exactly → Vulcan PASS round 2. Transcripts:
  `machineresearch/sley-2.0/reviews/reweave-adoption-*-2026-09-06.log`.
  Gate-name-set validation stays review-owned until RW-030 charters the set.
- (f) Dossier: regenerated via the builder (derived, ADR-0043 intact);
  content byte-identical, `--check` PASS. The dossier carries the GA release
  decision only; this adoption lives in this record + ADR-0049, which is the
  S20-750-area recording the reviewed mechanism names. No finding-register
  change: the new lane verdicts are recorded in the transcripts + ADR-0049,
  not as register obligations (register scope is S20 review states,
  untouched).

## Ownership bindings (R-3 resolved, nothing manufactured)

- Accountable authority: the operator, issuer of ADOPT-REWEAVE-2026-09-06.
- Implementation: the established REWEAVE implementation-author workflow in
  this Muse/OpenCode session, continued under that authority (the requestor
  named in adoption-request.json). Local commits use the repo's existing
  commit identity; no model label is presented as an authenticated principal.
- Independent review: kept separate (Ariadne/Nabu/Vulcan lanes above; the
  delta addendum author). No self-certification: the lane PASS verdicts are
  the lanes', quoted in the transcripts.
- Role assignments per the reviewed mapping (work-package-mapping.json) and
  `docs/WORK_PACKAGES.md` owners: RW-010 ← S20-020/S20-030 (Codex lane:
  record/integration); RW-020 ← S20-770 (Ariadne: traceability/ledger),
  S20-740 (Vulcan: review dispositions), S20-750 (Codex: dossier/record).
  Protected role assignments and worker permissions are unchanged.
- Coverage: every RW package with null owners in the distributed
  `work-packages.json` stays null there (package preserved as-reviewed);
  the binding for the now-active packages RW-010/RW-020 is this record.
  Later packages are bound by their own slices when dependency-ready.

## Residuals

- R-1 (candidate-set mapping reconciliation): disposition SIGNED as assigned
  future work by this operator direction (§2 of the decision); the
  reconciliation itself remains open, owned by the RW-020 role assignments
  above. Blocks nothing accepted today; required at a future RW-020 review.
- R-2 (1010 NOT_ASSESSED implementation evidence): assigned future work
  (RW-040 audit, package gates). Not conditionable on mapping acceptance.
- R-4 (2 sley2-side unclassified register dispositions): recorded external
  condition, correctly out of ledger scope; RW-020 roles confirm at gate.
- R-5 (draft UNREVIEWED): DISCHARGED by the delta addendum (this record's
  §f notes the ledger was not regenerated).
- O-1/O-2: applied to the live rev2 `reweave-r1/baseline.json` exactly as
  the reviewer specified (W210_WITNESS per-copy wording; 14-verdict field
  enumeration: eight vulcan_review-field fuzz slices, invariant_audit
  independent_review, threat_coverage independent_security_review).
  Verified against the register and the R1-local summary. No other package
  byte touched; rev1 snapshot, rev2 evidence, and review artifacts intact.

## Gate closures recorded here (effective in the adoption commit)

- RW-010 (minimum gate: protected adoption record): COMPLETE. This file is
  the protected record; adoption installed per §Scope and validated per
  §Validation.
- RW-020 (minimum gate: no unclassified active obligations): COMPLETE using
  the technically accepted ledger draft (addendum §3; not regenerated) plus
  the ownership bindings above and the R-1 disposition signature. Ledger
  facts reconfirmed: 1010/1010 records, 20/20 obligations, 0 unclassified,
  0 duplicates, 0 gaps; sole VERIFIED obligation REQ-LINEAGE.
- R1 verdict: PASS. Adoption installed+validated; mapping accepted;
  ownership bound; boundary proposal held as proposal (RW-030 charters the
  record); required reviews complete (delta addendum + three lane PASSes).
  R0 stands PASS (F1 closed by the addendum; F2 retained as observed FAIL
  with documented staleness, unchanged by this amendment).
- The finding register, dossier decision state (BLOCKED, GA release scope),
  and GA acceptance gates are unchanged in state by this adoption.

## Resumed campaign

- Pause ended at R1 PASS. RW-030 and RW-040 are siblings under RW-020;
  RW-050 requires both. First post-R1 assignment: the reviewed RW-040
  baseline-defects/bootstrap-capability audit proposal (accepted proposal
  only — not counted as a completed package). Narrow, tested slices;
  working Rust code preserved unless a demonstrated defect, requirement, or
  reviewed transition justifies change; independent review at the required
  semantic boundaries.
