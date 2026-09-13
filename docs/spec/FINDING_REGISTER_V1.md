# Finding Register v1

Status: S20-740 contract draft, revision 4 (2026-09-13); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review). The mechanics are `scripts/build_finding_register.py`;
implementation state is tracked in the machine summary.

Revision 4 closes the two precision gaps the Vulcan re-review of the live
register kept open as P3s: a `PASS` that still names findings blocks
clearance unless the review declares them closed or the section tracks them
(`unclaimed_carried_findings`), and sections whose status says `COMPLETE`
without satisfying the completion test are named with their open counts
(`mid_string_complete_packages`). Neither repair reclassifies a verdict.

Revision 2 answers the three Council reviews of the revision-1 draft (8
P0s); the answers are itemized in section 7. No register value changed
meaning silently: every tightened rule is stated here before it runs.

## Boundary

S20-740 needs one machine-readable answer to "which review obligations exist,
which are open, and which severity tokens their dispositions name" before an
independent reviewer can issue a complete PASS (master goal sections 16.7 and
26.9, dossier item "findings by severity and disposition"). This contract
freezes that register: how review obligations are derived from the machine
summary, how their dispositions are classified, and which invariant a
completed package must satisfy.

It reports per-obligation severity *tokens named in dispositions*, not
per-finding severity: a disposition that closes two P1s and a bare `PENDING`
both appear honestly for what they record, and a bare pending review carries
no severity because none was recorded. The per-finding record the dossier
item ultimately needs (id, title, severity, disposition, owning package,
closing commit) is future work named in section 7, not this register.

It is not the independent review itself, does not issue or accept findings,
and does not change any package's status. The review remains Vulcan's, and
`make release-check` and `make v2` stay fail-closed.

## 1. Source

`machineresearch/sley-2.0/machine-summary.json` is the only source: it is the
tracked record every package updates. The register walks it and collects every
string field whose name contains `review` or `disposition`, plus every
lane-named string leaf (`ariadne`, `nabu`, `vulcan`, `merlin`, `codex`)
directly under a `review`/`disposition` record such as
`current_delta_review`, except

- fields naming a role, session, actor, or instant, by anchored suffix:
  `reviewer_role`, `*_session_id`, `*_at`, `*_by`, `*_id`, `*_timestamp`,
  `*_note`, and `*reviews`. The anchors are exact: a field that merely
  contains one of these words elsewhere is collected, not dropped;
- values that are an ISO-8601 instant (`^\d{4}-\d{2}-\d{2}T`) or a Council
  session identifier (`^forge-` or `^[a-z]+-[a-z]+-s20-`), which name a
  review event rather than record its disposition;
- the register's own verdict field `independent_review`: it is the review's
  output, not an input obligation. Collecting it would make
  `S20_740_COMPLETE` unreachable, because the verdict must read `PENDING`
  until the review it awaits has happened. The stage checker asserts it
  separately per status instead.

A missing summary is `REGISTER_SUMMARY_MISSING`; a summary that is not an
object, or that carries no review obligation at all, is
`REGISTER_SUMMARY_INVALID`.

## 2. Obligations

Each collected field is one obligation:

```text
obligation = {
  "section": dotted path of the owning object ("(root)" at the top level),
  "field": field name,
  "disposition": the recorded string,
  "state": "PASS" | "PENDING" | "DEFERRED" | "HISTORICAL_ROUND" | "OTHER",
  "reviewer": the lane token the field name carries, or null,
  "severities": ascending list of the P0..P4 tokens the disposition names
    outside negations,
  "declares_closed_findings": bool,
  "declares_no_open_p0_p1_p2": bool,
  "package_status": the owning section's status, or null,
  "superseded_by": the superseding PASS field, or null
}
```

States are exact over the first underscore-delimited token, matched against
the closed head set `PASS`, `PENDING`, `DEFERRED`, `FAIL`, `REVISE`, plus the
declared alias table (`VULCAN_PASS` reads `PASS`; the table lives in the
builder and this section names every entry, so no alias is ever silent):

- `PASS` when the head token is `PASS` or the disposition is a declared
  alias; except a `PASS` head that still claims an open finding outside a
  negation group (`P0_OPEN`, `OPEN_P1`) contradicts itself and is `OTHER`;
- `PENDING` when the head token is `PENDING`;
- `DEFERRED` when the head token is `DEFERRED` (a lane that was unavailable);
- `HISTORICAL_ROUND` when the head token is `FAIL` or `REVISE` **and** the
  same section records a `PASS` obligation carrying the same reviewer lane
  token that closes this round (`superseded_by` names that field); a
  `FAIL`/`REVISE` round with no closing `PASS` was never re-reviewed, so it
  stays `PENDING`. Closure runs from the qualified or early round toward the
  general or later review: an `initial`/`first`/`revision-N` round folds into
  the `final` or unmarked review, and a qualified subject round folds into
  the strictly less qualified review. A slice `PASS` never closes the
  overall `FAIL` it belongs to, and a scoped `PASS` never closes a round
  from another subject. Round ordering follows the field-name
  convention (`initial` before `final`, revision numbers ascending) and is
  enforced: a cross-core fold needs round evidence (an early token on the
  round or a late token on the `PASS`), so an older general `PASS` never
  closes a newer qualified `FAIL`, and two unmarked rounds never fold
  across cores;
- `OTHER` otherwise, which the register surfaces rather than silently
  normalizing.

`reviewer` is the first of `ariadne`, `nabu`, `vulcan`, `merlin`, `codex`
appearing in the field name, or null when the field names no lane; a
`FAIL`/`REVISE` round with no lane token can never match a superseder and
stays open.

`severities` strips every `NO_OPEN_P0...` and `NO_NEW_P0...` negation group
before scanning, then deduplicates: `PASS_NO_OPEN_P0_P1_P2` carries no
severity tokens, while `PASS_PRIOR_P2_P3_CLOSED_NO_NEW_P0_P1_P2_P3_P4`
carries `P2, P3`. Count-prefixed encodings name zero counts explicitly, and
a zero count is an absence claim like a negation: `FAIL_0_P0` names no `P0`
mention, and an all-zero `PASS_0_P0_0_P1_0_P2_0_P3` carries none at all. The list is distinct tokens per obligation, not a finding
count, and a negated or zero-count token is an absence claim, never a mention.

`declares_closed_findings` is true when the disposition names `CLOSED`;
`declares_no_open_p0_p1_p2` is true when it names `NO_OPEN_P0_P1_P2`.

## 3. Register

`evidence/review/finding-register.json`:

```text
register = {
  "contract": "sley2.finding-register.v1",
  "contract_revision": integer,
  "work_package": "S20-740",
  "source": "machineresearch/sley-2.0/machine-summary.json",
  "obligations_digest": SHA-256 of the canonical obligation list,
  "obligation_count": integer,
  "states": { state: count },
  "severity_mentions": { "P0".."P4": count },
  "obligations": [ obligation ... ] ascending, the full payload the
    obligations_digest covers,
  "open_reviews": [ {section, field, disposition, severities} ... ]
    ascending, the PENDING obligations with what each records,
  "unclaimed_carried_findings": [ {section, field, disposition,
    unclaimed_severities} ... ] ascending: PASS obligations that still name
    severities (after valid-negation strip and zero-count absence) where
    every named severity is unaccounted for: outside the review's own
    per-severity `CLOSED` scope (only lane words may stand between the
    severity and the `_CLOSED` anchor, the anchor needs a right word
    boundary, and the claim must be terminal except for absence
    (`NO_...`) and followup (`WITH_...`) declarations carrying content
    (a bare trailing `NO`/`WITH` keyword is vacuous) — so a `P2`
    closure never covers carried `P3/P4` followups, `DISCLOSED`/
    `UNCLOSED` substrings never exempt, and `P1_CLOSED_CIRCUIT` word
    salad is not a closure claim),
    outside a valid negation group (a group stacked under `NO_` is void
    and declares nothing), and outside the section's per-package open
    claims. Visible in severity_mentions but able to survive into a CLEAR
    read, so they block clearance until claimed or closed — without
    reclassifying the verdict.
  "mid_string_complete_packages": [ {section, status, open_obligations} ... ]
    ascending: sections whose status contains `COMPLETE` but does not end
    `COMPLETE` (restricted / proposal / boundary language). Not a
    completion claim, so not a violation; named here with open counts so
    the precision gap lives in the artifact, not in prose.
  "deferred_reviews": [ {section, field, disposition} ... ] ascending,
  "unclassified": [ {section, field, disposition} ... ] ascending, state OTHER,
  "superseded_rounds": [ {section, field, disposition, superseded_by} ... ]
    ascending, the HISTORICAL_ROUND obligations and what superseded each,
  "complete_packages": [section, ...] ascending, sections whose status ends COMPLETE,
  "complete_packages_with_open_reviews": [ {section, field, state} ... ],
  "declared_open_findings": the summary's open_findings counters,
  "package_open_claims": { "section.field": open count } ascending, every
    per-package p0..p4 open list length and open count the summary carries,
  "result": "FINDING_REGISTER_CLEAR" | "FINDING_REGISTER_OPEN",
  "register_digest": SHA-256 of the canonical register without this field
}
```

Rules:

- the summary's `open_findings` must carry exactly the non-negative int
  counters `p0` through `p4`; anything else is `REGISTER_SUMMARY_INVALID`,
  because a missing or non-numeric counter would let the register read
  clear vacuously. Every per-package open list must be a list and every
  per-package open count a non-negative int, or the summary is likewise
  invalid;
- the result is `FINDING_REGISTER_CLEAR` exactly when no obligation is
  `PENDING`, no obligation is `OTHER`, `unclaimed_carried_findings` is
  empty, `complete_packages_with_open_reviews`
  is empty, every top-level open-finding counter is zero, and every
  per-package open claim is zero; otherwise it is `FINDING_REGISTER_OPEN`
  and the register names what is open. A `DEFERRED` lane is recorded
  unavailability, not an open finding, so it does not block clearance by
  itself;
- **a completed package may not carry an open review**: a section whose status
  ends in `COMPLETE` (but not `INCOMPLETE` or `NOT_COMPLETE`) with any
  obligation in state `PENDING`, `DEFERRED`, or `OTHER` is
  `REGISTER_COMPLETION_VIOLATION`, and no register is written. A
  `HISTORICAL_ROUND` obligation is allowed there, because a package reaches
  completion by passing after earlier rounds failed, and `superseded_rounds`
  names the passing field that proves it;
- the register carries no timestamp, host name, user name, or path outside the
  repository, and is a pure function of the summary's obligations, so `--check`
  detects drift with `REGISTER_DRIFT`;
- the digest covers the derived obligation list rather than the summary bytes,
  because the summary also records this register's own counts: a byte digest
  would never reach a fixed point, while an obligation digest is stable under
  the counter updates the checker cross-checks;
- `FINDING_REGISTER_OPEN` is the expected state while Council lanes are down;
  it is not a failure of this package, and the checker does not require
  `FINDING_REGISTER_CLEAR`. S20-740 itself cannot complete until the register
  reads `FINDING_REGISTER_CLEAR` and an independent reviewer records a PASS.

## 4. Codes

S20-740 reserves 75000 through 75003: `REGISTER_SUMMARY_MISSING` (75000),
`REGISTER_SUMMARY_INVALID` (75001), `REGISTER_COMPLETION_VIOLATION` (75002),
`REGISTER_DRIFT` (75003). The script exits 1 and prints one JSON object naming
the code on failure.

## 5. Staging

`scripts/check_finding_register.py` runs under `make quick`. Statuses:
`S20_740_CONTRACT_DRAFT_REVIEW_PENDING`,
`S20_740_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS`,
`S20_740_REGISTER_IMPLEMENTED_REVIEW_PENDING`, and `S20_740_COMPLETE`, the last
requiring the three Council reviews to read `PASS`, the register to read
`FINDING_REGISTER_CLEAR`, and an independent review record. In every
implementation status the checker verifies the register exists with its
contract tag and revision, that its obligation payload matches the summary
(count and digest, not just the tallies), that no completed package carries
an open review, that the unit tests pass, and that `release-check` and `v2`
stay `NOT_IMPLEMENTED`. The checker pins the summary's `contract_revision`
to this document's revision, so a contract edit without its revision number
fails the gate.

## 6. Explicit exclusions

- No independent review, no finding acceptance, no severity judgment: the
  register reports what packages recorded, and never edits a disposition.
- No creation, closure, or reopening of findings, and no status change to any
  package.
- No Council dispatch: the register is derived offline from tracked evidence.
- No GA claim, release decision, or publication.

## 7. Clarifications

Revision 4 (2026-09-13) closes the two precision gaps the Vulcan re-review
of the live register kept open as P3s: a `PASS` that still names findings
blocks clearance unless the review declares them closed or the section
tracks them in its per-package open claims (`unclaimed_carried_findings`;
states unchanged, verdicts not reclassified), and mid-string-`COMPLETE`
statuses are named with open counts (`mid_string_complete_packages`).

Revision 3 (2026-09-11) enforces the round ordering section 7 already
required: a cross-core fold needs round evidence, so the older general
`PASS`es no longer close the newer qualified `FAIL`s they predate (the
live cases were six S20-360/S20-390 rounds reading closed), lane-named
leaves under `review`/`disposition` records are collected as obligations
(the live case was five `current_delta_review` records reading invisible),
and supersession requires lane-core compatibility so a scoped `PASS` can
never fold a foreign round.

Revision 2 (2026-09-05) answers the Council reviews of the revision-1 draft:
a `FAIL`/`REVISE` round needs a same-lane superseding `PASS` (Ariadne P0-1,
Vulcan P0-2; the two restricted-query Nabu `REVISE` records were re-reviewed
to `PASS` for exactly this rule); classification is first-token over a closed
head set with a declared alias table, and a self-contradicting `PASS` is
`OTHER` (Ariadne P0-2); open reviews carry disposition and severities while
the Boundary promises tokens, not per-finding severity (Nabu P0-1); the
register's own verdict is asserted per status instead of collected, so
completion is reachable (Nabu P0-2); clearance reads the per-package open
claims beside the top-level counters (Vulcan P0-1); the obligation payload is
part of the frozen shape and digest-checked (Vulcan P0-3); severity tokens
exclude negations (Vulcan P0-4).

Completion is tested by status suffix (`COMPLETE`, excluding `INCOMPLETE`
and `NOT_COMPLETE`); mid-string boundary statuses such as
`S20_340_COMPLETE_IMMUTABLE_DESCRIPTORS_ONLY` are not treated as complete
for the violation check, but their unsuperseded reviews still block
clearance as `PENDING`, so the gap is reporting precision, not a silent
pass. Since revision 4 the precision gap lives in the artifact: every
status containing `COMPLETE` without satisfying the suffix test is named
in `mid_string_complete_packages` with its open-obligation count.

Round ordering is a field-name convention the register reads
directionally: closure runs early-or-qualified toward late-or-general, so a
slice `PASS` beside an overall `FAIL` does not supersede (the live case is
the S20-700 surface audit, whose slice `PASS`es leave the audit `FAIL`
open), and neither would a `PASS` recorded before its `FAIL`.

The per-finding record (id, title, severity, disposition, owning package,
closing commit, cross-checked by this register) that an independent review
starts from is future S20-740 work, not this contract.
