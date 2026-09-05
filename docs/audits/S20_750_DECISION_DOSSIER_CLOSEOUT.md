# S20-750 decision dossier closeout (2026-09-05)

## Findings

| # | Reviewer | Finding | Verdict |
|---|----------|---------|---------|
| 1 | Ariadne | Item 11 "property-test counts" is `EVIDENCED` from unit-test counts; the tree has no proptest, quickcheck, or hypothesis harness, so the item's actual fact is absent and a different fact is substituted | confirmed live: no harness in manifests, lockfile, or sources; fixed |
| 2 | Ariadne | `build_decision_dossier.py:103-105` shadows `inventory` with the test inventory, so `license_disposition_blocked` reads 0 while T52 records 19 `BLOCKED` packages | confirmed live; fixed |
| 3 | Nabu | `derive_decision` ignores its `entries` argument while contract S3 and ADR-0043 decision 3 claim the state derives from the entries, so no rule forbids `PASS` while entries are `GATED` | confirmed live (`entries` unused); fixed |
| 4 | Nabu | The same shadowing makes the SBOM-and-license entry read the test inventory; it cites a source it never reads | confirmed live; fixed |
| 5 | Vulcan | The same shadowing emits `license_disposition_blocked=0` marked `EVIDENCED`, with no test or checker covering it | confirmed live; fixed with tests plus checker cover |

## Fixes

- `scripts/build_test_inventory.py` counts property tests from tracked
  sources: harness use sites in Rust (`proptest!`, `quickcheck!`, qualified
  paths) and Python (`@given`, hypothesis imports), harness dependencies in
  the workspace and crate manifests, with the scanned manifest count
  recorded. Today that is a counted zero (20 manifests, no use sites), which
  is a tracked fact, not an absence of evidence.
- `scripts/build_decision_dossier.py` names the two inventories
  `license_inventory` and `test_inventory`. Item 11 carries the counted
  property-test zero plus its scan detail, keeps the adjacent unit-test,
  fuzz-target, and vector counts under their own names, and its note states
  the harness absence outright. Item 28 reads the license inventory and now
  reports 19 `BLOCKED`, against the source it cites. The findings entry
  carries the open and deferred review counts the decision reads.
- `derive_decision` derives from the entries per the contract section 3
  mapping now written into the contract: findings entry for open reviews,
  deferred lanes, and declared P0/P1/P2; license entry for root approval;
  the six per-arm entries for the succession-trial rule; reproducibility
  entry for single-host attestation. A missing or gated decision-input
  entry fails closed with the unknown fact named. The release-check gate,
  succession thresholds, and approved conditional items still read the
  tracked sources, because no section 30 item carries them; the contract
  says so explicitly.
- `bench/review/tests/test_decision_dossier.py` grows from 12 to 18 tests:
  the arm-entries-drive-the-trial-rule test (sources claiming trials do not
  unblock gated arm entries, and vice versa), missing/gated-input fail-closed
  tests, synthetic divergent-inventory tests proving each entry reads its
  own source, and live pins (19 blocked licenses, counted property zero).
- `scripts/check_decision_dossier.py` cross-checks the tracked dossier's
  SBOM blocked count against T52 and its property-test count against the
  test inventory on every `make quick`.

## Reconciliation

No re-review was needed: unlike the 740 supersession rule, which surfaced
live `REVISE` records, these fixes change reported facts without
contradicting any recorded disposition. The round verdicts stand as `FAIL`
history; the five P0s close by fix, recorded in the machine summary.

## Validation

Tier 1 `make quick` and `make lint` pass at this commit. Tier 2 re-ran for
contract revision 5 on 2026-09-05: `make core`, `make conformance`,
`make adversarial`, and `make fuzz-smoke` all exited 0; the change touches
only the dossier/inventory scripts, their tests, the contract text, and
derived evidence, so no package oracle changes behavior.
The full `make v1` gate was skipped because this is a subsystem handoff, not a
release boundary. `make v2` and `make release-check` remain intentionally
fail-closed.
