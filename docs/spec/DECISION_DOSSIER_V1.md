# Decision Dossier v1

Status: S20-750 contract draft, revision 2 (2026-09-03); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review). Revision 2 adds the tracked test inventory as a source, which
evidences the property-test counts item. The mechanics are `scripts/build_decision_dossier.py`; implementation
state is tracked in the machine summary.

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
- No item is ever marked evidenced from a narrative document alone: the
  resolver reads the machine summary, the tracked reports of S20-710 full,
  S20-720, S20-730, and S20-740, or a tracked conformance fixture.

## 1.1 The required items

The dossier covers exactly these thirty-four items, in this order, copied
verbatim from master goal section 30. A dossier that omits, reorders, renames,
or adds an item is `DOSSIER_SOURCE_INVALID`:

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
- `evidence/validation/test-inventory.json` (`sley2.test-inventory.v1`): the
  Rust unit tests per crate, the ignored fixture-refresh emitters, the
  persistent fuzz targets, the Python test functions, and the conformance
  vectors per family, all counted from tracked sources. The inventory runs no
  test: it describes the corpus, and a passing run stays separate evidence.

A missing or unreadable source is `DOSSIER_SOURCE_MISSING`; a source whose
contract tag or shape is wrong is `DOSSIER_SOURCE_INVALID`.

## 3. Decision state

The state is derived, in this precedence, from the entries and the sources:

1. `BLOCKED` when a trustworthy decision cannot be reached because required
   authority, evidence, model access, or execution is unavailable: any open
   review obligation, any deferred review lane, an unapproved root license, no
   executed succession trial, or a fail-closed product gate. The dossier lists
   every reason.
2. `FAIL` when a required gate fails, a release-blocking finding is open (any
   declared P0, P1, or P2 open finding), or recorded evidence contradicts the
   design.
3. `ALPHA_COMPLETE` when implementation gates pass but the succession
   thresholds do not.
4. `CONDITIONAL_PASS` when gates pass with an explicitly approved
   non-correctness P2 item or a material evidence limitation.
5. `PASS` when every criterion passes with no open P0, P1, or P2 finding.

The derivation is total: exactly one state, with its reasons, and a
`decision_authority` field that always reads
`OPERATOR_DECISION_NOT_DELEGATED`. A dossier may never record `PASS` while any
product gate is fail-closed; that combination is `DOSSIER_DECISION_INVALID`.

## 4. Dossier

`evidence/release/decision-dossier.json`:

```text
dossier = {
  "contract": "sley2.decision-dossier.v1",
  "work_package": "S20-750",
  "project": "Sley", "target_version": "2.0.0", "phase": from the summary,
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
the code on failure.

## 6. Staging

`scripts/check_decision_dossier.py` runs under `make quick`. Statuses:
`S20_750_CONTRACT_DRAFT_REVIEW_PENDING`,
`S20_750_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS`,
`S20_750_DOSSIER_IMPLEMENTED_REVIEW_PENDING`, and `S20_750_COMPLETE`, the last
requiring the three Council reviews to read `PASS`, every entry to read
`EVIDENCED`, and an operator decision record. In every implementation status
the checker verifies the dossier exists with its contract tag, that it covers
every section 30 item exactly once in order, that it does not drift, that it
claims no publication, that its decision state is not `PASS` while a gate is
fail-closed, that the unit tests pass, and that `release-check` and `v2` stay
`NOT_IMPLEMENTED`.

## 7. Explicit exclusions

- No release decision, approval, or GA claim: the dossier reports, the
  operator decides.
- No push, tag, upload, deployment, publication, or announcement, and no
  mechanism that could perform one.
- No estimation: an item with no tracked evidence is `GATED`, never inferred.
- No narrative sources: campaign records and audits are context, not evidence.

## 8. Clarifications

Revision 1 carries none.
