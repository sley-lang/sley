# Finding Register v1

Status: S20-740 contract draft, revision 1 (2026-09-03); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review). The mechanics are `scripts/build_finding_register.py`;
implementation state is tracked in the machine summary.

## Boundary

S20-740 needs one machine-readable answer to "which review obligations exist,
which are open, and at what severity" before an independent reviewer can issue
a complete PASS (master goal sections 16.7 and 26.9, dossier item "findings by
severity and disposition"). This contract freezes that register: how review
obligations are derived from the machine summary, how their dispositions are
classified, and which invariant a completed package must satisfy.

It is not the independent review itself, does not issue or accept findings,
and does not change any package's status. The review remains Vulcan's, and
`make release-check` and `make v2` stay fail-closed.

## 1. Source

`machineresearch/sley-2.0/machine-summary.json` is the only source: it is the
tracked record every package updates. The register walks it and collects every
string field whose name contains `review` or `disposition`, except

- fields naming a role, session, actor, or instant (`reviewer_role`,
  `*_session_id`, `*_at`, `*_by`, `*_id`, `*_timestamp`, `*_note`, `*reviews`),
- values that are an ISO instant or a Council session identifier.

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
  "severities": ascending list of the P0..P4 tokens the disposition names,
  "declares_closed_findings": bool,
  "declares_no_open_p0_p1_p2": bool,
  "package_status": the owning section's status, or null
}
```

States are exact:

- `PASS` when the disposition begins with `PASS` or is `VULCAN_PASS`;
- `PENDING` for `PENDING`;
- `DEFERRED` when it begins with `DEFERRED` (a lane that was unavailable);
- `HISTORICAL_ROUND` when it begins with `FAIL` or `REVISE`, which records a
  superseded review round of a package that later passed or was revised;
- `OTHER` otherwise, which the register surfaces rather than silently
  normalizing.

`declares_closed_findings` is true when the disposition names `CLOSED`;
`declares_no_open_p0_p1_p2` is true when it names `NO_OPEN_P0_P1_P2`.

## 3. Register

`evidence/review/finding-register.json`:

```text
register = {
  "contract": "sley2.finding-register.v1",
  "work_package": "S20-740",
  "source": "machineresearch/sley-2.0/machine-summary.json",
  "obligations_digest": SHA-256 of the canonical obligation list,
  "obligation_count": integer,
  "states": { state: count },
  "severity_mentions": { "P0".."P4": count },
  "open_reviews": [ {section, field} ... ] ascending, the PENDING obligations,
  "deferred_reviews": [ {section, field, disposition} ... ] ascending,
  "unclassified": [ {section, field, disposition} ... ] ascending, state OTHER,
  "complete_packages": [section, ...] ascending, sections whose status ends COMPLETE,
  "complete_packages_with_open_reviews": [ {section, field, state} ... ],
  "declared_open_findings": the summary's open_findings counters,
  "result": "FINDING_REGISTER_CLEAR" | "FINDING_REGISTER_OPEN",
  "register_digest": SHA-256 of the canonical register without this field
}
```

Rules:

- the result is `FINDING_REGISTER_CLEAR` exactly when no obligation is
  `PENDING`, `complete_packages_with_open_reviews` is empty, and every declared
  open-finding counter is zero; otherwise it is `FINDING_REGISTER_OPEN` and the
  register names what is open;
- **a completed package may not carry an open review**: a section whose status
  ends in `COMPLETE` with any obligation in state `PENDING`, `DEFERRED`, or
  `OTHER` is `REGISTER_COMPLETION_VIOLATION`, and no register is written. A
  `HISTORICAL_ROUND` obligation is allowed there, because a package reaches
  completion by passing after earlier rounds failed;
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
contract tag, that it agrees with the summary (no drift), that no completed
package carries an open review, that the unit tests pass, and that
`release-check` and `v2` stay `NOT_IMPLEMENTED`.

## 6. Explicit exclusions

- No independent review, no finding acceptance, no severity judgment: the
  register reports what packages recorded, and never edits a disposition.
- No creation, closure, or reopening of findings, and no status change to any
  package.
- No Council dispatch: the register is derived offline from tracked evidence.
- No GA claim, release decision, or publication.

## 7. Clarifications

Revision 1 carries none.
