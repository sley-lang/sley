# Decision Dossier v1

Status: S20-750 contract draft, revision 7 (2026-09-15); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review; the a809906 round read REVISE in all three lanes and revision 7
closes it). Revision 2 adds the tracked test inventory as a source, which
evidences the property-test counts item. Revisions 3 and 4 added the threat
coverage and GA acceptance reports as cited inputs without a header bump.
Revision 5 names, in section 3, the entry each decision rule derives from;
counts the property-test absence in the test inventory instead of
substituting unit-test counts; and reads the SBOM and license entry from the
license inventory. Revision 6 wires the product gates to the fail-closed
stub, orders the conditional rules so approvals are verifiable, gates
`PASS` on the GA criteria, closes the null-fact and missing-key holes, and
records the residual precision notes. Revision 7 adds the GA acceptance
report contract (section 2.1): every section 26 state is derived from the
fact it cites, the register's own predicates are consumed rather than
re-implemented, the report is digest-bound to the register and verified by
the dossier, and the pipeline order is fixed; it also carries the register
result and unclassified count into rule 1, reads the section 22 thresholds
from the tracked accounting report through one shared derivation, and names
the single-attesting-host cause. The mechanics are
`scripts/build_decision_dossier.py` and `scripts/build_ga_acceptance_report.py`;
implementation state is tracked in the machine summary.

## Boundary

S20-750 owes the complete dossier and the exact release decision state (master
goal section 30 "Final Completion Report" and section 27 "Release Decision
States"). This contract freezes how that dossier is assembled: one entry per
required report item, each resolved from tracked evidence or explicitly marked
gated, and a decision state derived from those entries rather than asserted.

It does not make a release decision, open `make v2` or `make release-check`,
approve anything, or claim GA. Building the dossier while the goal is
incomplete is the point: it names, at any moment, exactly what is evidenced and
what is missing.

## 1. Items

Every item of master goal section 30 is one entry, in the order that section
lists them:

```text
entry = {
  "item": the section 30 item, verbatim,
  "state": "EVIDENCED" | "GATED",
  "value": a scalar or object of tracked facts, or null,
  "evidence": ascending list of repository-relative evidence paths,
  "note": why the item is gated, or how it was resolved
}
```

- `EVIDENCED` means every fact of the item is present in tracked evidence.
- `GATED` means the item awaits an authority, an execution, or a decision that
  has not happened; the note names it. A gated item is never given a value.
  An object whose fields are all null carries no facts: it reads `GATED`
  with no value, the note naming the nulled fields. Every cited evidence
  path must exist; a cited-but-absent file fails the build rather than
  reading as a quiet note.
- No item is ever marked evidenced from a narrative document alone: the
  resolver reads the machine summary, the tracked reports of S20-710 full,
  S20-720, S20-730, and S20-740, or a tracked conformance fixture.

## 1.1 The required items

The dossier covers exactly these thirty-four items, in this order, copied
verbatim from master goal section 30. A dossier that omits, reorders, renames,
or adds an item is `DOSSIER_SOURCE_INVALID`. The "exactly" binds this
revision: the goal's "at minimum" is honored by amending this contract, never
by ad-hoc items.

1. final repository
2. final branch
3. final commit
4. clean-working-tree status
5. frozen legacy artifact verification
6. final schema epoch
7. final state-root demo identifiers
8. SCB1 conformance result
9. independent encoder result
10. SSMC1 corpus counts
11. property-test counts
12. fuzz duration and findings
13. adversarial result
14. crash-injection result
15. security review result
16. succession benchmark methodology
17. strict correctness by arm
18. ACT by arm
19. context bytes and model tokens by arm
20. repair loops by arm
21. invalid committed states
22. stale candidates incorrectly accepted
23. optional Zerolang comparison
24. artifact path
25. SHA-256
26. byte size
27. reproducibility result
28. SBOM and license inventory
29. findings by severity and disposition
30. residual risks
31. evidence gaps
32. independent review result
33. release decision state
34. confirmation that no push, tag, upload, deployment, publication, or public announcement occurred without separate authorization

## 2. Sources

- `machineresearch/sley-2.0/machine-summary.json` (package states, counts,
  identities, legacy freeze, succession, open findings, risks, gaps);
- `evidence/release/reproducibility-report.json` (candidate commit, artifact
  digest, size, reproducibility, clean tree);
- `evidence/release/sbom/cyclonedx-1.6.json` and `spdx-2.3.json` (SBOM);
- `evidence/security/T52/pre-release-inventory.json` (license inventory);
- `evidence/release/provenance.json` (build provenance);
- `evidence/conformance/independent-conformance-report.json` (independent
  encoder and conformance coverage);
- `evidence/review/finding-register.json` (findings by severity and
  disposition, open reviews);
- `evidence/release/ga-acceptance-report.json`
  (`sley2.ga-acceptance-report.v1`): every master-goal section 26 acceptance
  criterion with its evidence and derived state, which the release decision
  item cites. The builder verifies the report's `report_digest` and refuses
  a report whose `register_digest` is not that of the finding register it
  reads (section 2.1);
- `evidence/release/succession-accounting-report.json`
  (`sley2.succession-accounting-report.v1`): the S20-630 accounting report of
  a tracked trial campaign, the single source of the section 22 threshold
  verdict. It is absent until a campaign tracks one; absence is a
  non-passing verdict, never an error. The smoke report under
  `evidence/runtime/` is untracked and is never read;
- `evidence/security/threat-coverage-report.json`
  (`sley2.threat-coverage-report.v1`): how far the M0 threat register's planned
  controls are realized, which the security review item cites as its measured
  input;
- `evidence/validation/test-inventory.json` (`sley2.test-inventory.v1`): the
  Rust unit tests per crate, the ignored fixture-refresh emitters, the
  persistent fuzz targets, the Python test functions, the conformance
  vectors per family, and the property tests per harness, all counted from
  tracked sources. A property-test count of zero is a counted fact: the
  inventory names the manifests, lockfile, and sources it scanned for
  proptest, quickcheck, and hypothesis. The inventory runs no
  test: it describes the corpus, and a passing run stays separate evidence.
- `conformance/state-root/v1/accepted.json`: the accepted state-root
  vectors entry 7 cites as evidence.

A missing or unreadable source is `DOSSIER_SOURCE_MISSING`; a source whose
contract tag or shape is wrong is `DOSSIER_SOURCE_INVALID`. Structural keys
the dossier cannot mean anything without (the summary's project, target
version, and phase; the SBOM shapes) are required, never defaulted: a
missing or renamed structural key fails loudly rather than regrading an
item to `GATED` with a note that reads as resolved. A present key with a
null value is pending data and reads `GATED` instead. The machine summary
also records this dossier's own counters and the register's counters (the
sync write-back); those mirror sections are non-inputs — the builder never
reads them — so the derivation stays acyclic.

## 2.1 GA acceptance report

`scripts/build_ga_acceptance_report.py` derives
`evidence/release/ga-acceptance-report.json` from the machine summary, the
finding register, the independent conformance, threat coverage,
error-symbol, anti-goal, reproducibility, artifact-content, SBOM, license
inventory, and provenance records, and the tracked accounting report. The
report is a decision input of this contract (rule 1's unevidenced count and
rule 5's gate), so it is bound by the same rules as the dossier:

- **Every state is derived from the fact its evidence names.** No criterion
  is a constant. A criterion whose fact is absent or contrary reads
  `AWAITS_REVIEW` (a human judgment is outstanding) or `GATED` (an authority
  or execution this repository does not hold is outstanding), never
  `EVIDENCED`. Neither is a pass and no combination of states is a GA claim.
- **The register's predicates are consumed, not re-implemented.** Package
  completion is the register's `complete_packages` (a status ending
  `COMPLETE`; a mid-string `COMPLETE` names a restricted boundary and is not
  a completion claim, FINDING_REGISTER_V1 section 7). A reviewer lane is
  clear only when the lane has rows, every row reads `PASS` or
  `HISTORICAL_ROUND`, and no row of the lane appears in
  `unclaimed_carried_findings` or `unclassified`. Register clearance is the
  register's own `result`.
- **A recorded verdict evidences only as a complete PASS.** The register's
  token classifier (`build_finding_register.classify_token`, loaded as a
  module) must read the disposition `PASS`, the form must be the bare `PASS`
  token or an enumerated count form whose every count is zero, and the
  register row must be neither unclaimed nor unclassified.
  `PASS_PENDING_CONFIRMATION_2_P0_OPEN`, `PASSED_TO_NEXT_ROUND`, `PASS_2_P1`,
  `PASS_WITH_OPEN_P1`, and a `PASS` naming follow-ups evidence nothing.
  Dossier items 15 and 32 and criteria 26.9.3 and 26.9.4 share this one
  definition; item 32 and 26.9.3 additionally require the register result
  `FINDING_REGISTER_CLEAR`.
- **Candidate evidence.** The dossier and GA builder use the S20-730
  shared admissibility and candidate selector. A missing or tied candidate
  gates its facts. Artifact-content criterion 26.8 reads
  `evidence/release/candidate-content-checks.json`, generated by
  `scripts/build_candidate_content_report.py` from the packaged archive.
  Its digest binds the result, manifest and member checks, forbidden-content
  check, and candidate commit/artifact digest/manifest digest/size. The
  archive member set must match S20-720 section 2, with fixture members read
  from the candidate's Git tree. The tracked T54 repository credential scan
  cannot establish these artifact checks. Refresh and smoke regenerate the
  report; quick rechecks the archive and rejects drift.
- **One threshold key.** Criterion 26.7 and rule 3 read the same derivation
  (`succession_thresholds`): the tracked accounting report must carry the
  contract tag, status `COMPLETE`, evidence status
  `DERIVED_FROM_VERIFIED_CLAIMS` (the S20-630 vocabulary), and every
  threshold row `PASS`. The S20-630 verifier checks the report digest and
  re-derives its evidence status from the recorded claim statuses. Threshold
  names must cover exactly the benchmark plan and S20-630 owner-held
  conditions; missing rows and `NOT_EVALUATED` conditions keep this gated. No machine-summary hand key is read.
- **Digest binding.** The report records `register_digest` and
  `obligations_digest` of the register it was derived from and its own
  `report_digest`. The dossier verifies `report_digest` against the report
  body and `register_digest` against the register it loads; a mismatch is
  `DOSSIER_SOURCE_INVALID`. A hand-edited or stale report cannot remove a
  `BLOCKED` reason.
- **Pipeline order.** Register, then GA report, then dossier, both before
  and after the counter sync (`make evidence-refresh` and
  `make release-candidate-smoke`). `scripts/check_decision_dossier.py` runs
  `build_ga_acceptance_report.py --check` and cross-checks the summary's
  `ga_acceptance` mirror, so `make quick` fails on a drifted report or a
  stale mirror. Unit tests live in
  `bench/review/tests/test_ga_acceptance_report.py`.

## 3. Decision state

The state is derived, in this precedence, from the entries and the sources:

1. `BLOCKED` when a trustworthy decision cannot be reached because required
   authority, evidence, model access, or execution is unavailable: any open
   review obligation (a pending row, an unclassified row, or a register whose
   result is not `FINDING_REGISTER_CLEAR`), any deferred review lane, an
   unapproved root license, no executed succession trial, a single attesting
   host, unevidenced GA acceptance criteria, or a fail-closed product gate.
   The dossier lists every reason.
2. `FAIL` when a required gate fails (evaluated and failed, as opposed to
   unimplemented), a release-blocking finding is open (any open P0 or P1, or
   any open P2 no approval covers), or recorded evidence contradicts the
   design. Evidence-against-design is enforced by the checker (required-items
   coverage, license cross-check, property-count cross-check), not the
   builder.
3. `ALPHA_COMPLETE` when implementation gates pass but the succession
   thresholds do not.
4. `CONDITIONAL_PASS` when gates pass with an explicitly approved
   non-correctness P2 item or a material evidence limitation. Approvals name
   register `section:field` rows in the summary's `approved_conditional_items`
   list; an open P2 row no approval names fails rule 2 instead. An
   unverifiable approval (open P2 counts but no register to match them
   against) fails closed as well.
5. `PASS` when every criterion passes with no open P0, P1, or P2 finding:
   the GA acceptance states all evidenced alongside the findings counts,
   thresholds, and conditionals above.

The derivation is total: exactly one state, with its reasons, and a
`decision_authority` field that always reads
`OPERATOR_DECISION_NOT_DELEGATED`. A dossier may never record `PASS` while any
product gate is fail-closed; that combination is `DOSSIER_DECISION_INVALID`.

Each rule reads the entry that carries its fact, and a missing decision-input
entry fails closed rather than falling back to the sources behind the
entries' backs:

- open review obligations and deferred lanes: the "findings by severity and
  disposition" entry, which carries the pending, deferred, unclassified, and
  unclaimed-carried counts and the register's own result;
- unapproved root license: the "SBOM and license inventory" entry, which
  carries the approval flag read from the license inventory;
- no executed succession trial: the six per-arm entries (items 17 through
  22), which are all `GATED` exactly while no trial has produced per-arm
  evidence;
- single attesting host: the "reproducibility result" entry;
- release-blocking finding open: the "findings by severity and
  disposition" entry's declared P0, P1, and P2 counts.

Four inputs have no section 30 item that carries them, so those rules read
the tracked sources this section names: the release-check and v2 gate states
from the live `scripts/gate_status.py` runs, dual-sourced against the summary
hand field (a hand edit clearing the field cannot clear a gate the stub still
reports closed); the succession thresholds from the tracked S20-630
accounting report through the GA builder's shared derivation (section 2.1);
the GA acceptance states from the digest-verified GA report; and the approved
conditional items from the machine summary. A `GATED` decision-input
entry blocks with the unknown fact named, rather than treating the unknown
as clear.

## 4. Dossier

`evidence/release/decision-dossier.json`:

```text
dossier = {
  "contract": "sley2.decision-dossier.v1",
  "work_package": "S20-750",
  "project": "Sley", "target_version": "2.0.0", "phase": from the summary,
  "gates": {"release-check": gate state, "v2": gate state},
  "entries": [entry, ...] in section 30 order,
  "evidenced": integer, "gated": integer,
  "decision_state": one of section 3,
  "decision_reasons": ascending list of exact reason strings,
  "decision_authority": "OPERATOR_DECISION_NOT_DELEGATED",
  "publication": { "authorized": false, "push": false, "tag": false,
                   "upload": false, "deployment": false, "announcement": false },
  "entries_digest": SHA-256 of the canonical entry list,
  "dossier_digest": SHA-256 of the canonical dossier without this field
}
```

The dossier carries no timestamp, host name, user name, or absolute path, and
is a pure function of its sources' facts, so `--check` detects drift with
`DOSSIER_DRIFT`. Like the finding register, the digest covers the derived
entries rather than the source bytes, because the machine summary also records
this dossier's own counters.

## 5. Codes

S20-750 reserves 76000 through 76003: `DOSSIER_SOURCE_MISSING` (76000),
`DOSSIER_SOURCE_INVALID` (76001), `DOSSIER_DECISION_INVALID` (76002),
`DOSSIER_DRIFT` (76003). The script exits 1 and prints one JSON object naming
the code on failure. `DOSSIER_SOURCE_MISSING` covers absent sources and
absent cited-evidence files; `DOSSIER_SOURCE_INVALID` covers wrong contract
tags and shapes, missing structural keys, and malformed gate, threshold, and
conditional types.

## 6. Staging

`scripts/check_decision_dossier.py` runs under `make quick`. Statuses:
`S20_750_CONTRACT_DRAFT_REVIEW_PENDING`,
`S20_750_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS`,
`S20_750_DOSSIER_IMPLEMENTED_REVIEW_PENDING`, and `S20_750_COMPLETE`, the last
requiring the three Council reviews to read `PASS`, every entry to read
`EVIDENCED`, and an operator decision record in the summary's
`decision_dossier.operator_decision` (set by the operator at the release
decision; item 33 stays value-less until then). In every implementation status
the checker verifies the dossier exists with its contract tag, that it covers
every section 30 item exactly once in order, that it does not drift, that it
claims no publication, that its decision state is not `PASS` while a gate is
fail-closed, that the unit tests pass, that the test inventory and the GA
acceptance report do not drift, that the summary's `decision_dossier` and
`ga_acceptance` mirrors match the derived records, and that `release-check`
and `v2` stay `NOT_IMPLEMENTED`.

## 7. Explicit exclusions

- No release decision, approval, or GA claim: the dossier reports, the
  operator decides.
- No push, tag, upload, deployment, publication, or announcement, and no
  mechanism that could perform one.
- No estimation: an item with no tracked evidence is `GATED`, never inferred.
- No narrative sources: campaign records and audits are context, not evidence.

## 8. Clarifications

Revision 1 carries none.

Revision 7 (2026-09-15) records two changes the contract text had not. First,
f7df74f (2026-09-15) made items 15 and 32 evidence-derived from the recorded
verdict fields (`threat_coverage.independent_security_review`,
`finding_register.independent_review`) instead of reading `GATED` by
construction, and made the GA report's section 26 states evidence-derived
for thirty-four of fifty-two criteria while leaving eighteen as constants.
Second, the a809906 Council round (Ariadne REVISE 4 P2 / 3 P3 / 2 P4, Nabu
REVISE 2 P2 / 5 P3 / 1 P4, Vulcan REVISE 4 P2 / 4 P3 / 2 P4; transcripts
under `evidence/review/verdicts/decision_dossier/*-a809906.md`) found the
eighteen constants, the `startswith("PASS")` classification that admitted
self-contradicting verdicts, lane clearance that ignored unclaimed rows, the
`RELEASE_APPROVED` crash, the unguarded and drifted GA report, the two
threshold hand keys, item 32's unread register, rule 1's unread OTHER rows,
and the stale mirrors. Revision 7 closes them with section 2.1, the rule 1
wording above, the shared threshold derivation, and the checker's GA drift
and mirror checks. Item 15 now reads `GATED` while the recorded verdict
names unclaimed P3/P4 follow-ups: a PASS that still names findings is not a
complete PASS, exactly as the register lists the row unclaimed.

Revision 6 records the fail-closed gate wiring (live stub runs dual-sourced
against the summary hand field, with evaluated-FAILED mapping to `FAIL` and
unimplemented to `BLOCKED`); the ordered conditional rules (open P2 rows
matched against `section:field` approvals, unverifiable approvals failing
closed); the GA gate on `PASS`; the null-fact and missing-key rules; and the
`PASS`-behind-closed-gates guard as unreachable-by-construction defense.

Revision 5 records why the property-test counts item stays `EVIDENCED` with a
zero: the absence of a harness is a counted tracked fact, while substituting
unit-test counts for property-test counts evidences a different fact under
the item's name. It also records why the SBOM and license entry reads the
license inventory and cites it: an entry that cites a source it never reads
is the same substitution with the evidence list as the substituted fact.
