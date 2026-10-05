# Recorded capability and application run

These artifacts were produced by copies of the three example scripts and the
published 2.0.6 Linux x86_64 agent in a fresh directory outside the source checkout,
with a minimal environment. No private tooling, network, provider or supervisor
was used. This records reproduction on the existing host, not independent human
adoption or a new OS.

- [Executed matrix](MATRIX.md), [result](result.json), and [command transcript](transcript.json).
  Each row links its verdicts, calls, untested/test-definition-only receipts and
  combined code-and-test refusal. The inputs are in `inputs/`.
- [Application verifier result](application-verification.json): 17 canonical cases,
  445 independent-oracle inputs, 11 malformed CSV classes, archive entrypoint,
  package corruption and output-preservation checks.
- [Oracle input, expected and actual values](application-oracle.json). The oracle
  is in the separate verifier; Python does not supply the production reorder rule.
- [v1 ZIP](stock-reorder-v1.zip), [manifest](application-manifest.json),
  [build result](application-build-result.json) and [transcript](application-build-transcript.json).
- [Input CSV](application-inventory.csv), [report](application-report.csv), and
  [receipt](application-report-receipt.json). This mixed-result example contains
  two recommendations and a deliberately invalid zero-pack row; CLI exit 1 is
  expected. The main guide's four valid rows are a separate input fixture.
- [v2 ZIP](stock-reorder-v2.zip), [maintenance frame](maintenance-frame.json),
  [result](maintenance-result.json) and [transcript](maintenance-transcript.json).
  The added reserved-stock regression produces 18 selected tests, a new proposed
  root and new candidate bytes. v1 remains unchanged.
- [Failed maintenance result](failed-maintenance-result.json) and
  [transcript](failed-maintenance-transcript.json): the deliberately wrong
  expected value prevents creating a package.
- [Artifact inventory](ARTIFACTS.json) binds these recorded files and the source
  scripts. The inventory excludes itself and is unsigned; it does not establish
  authenticity or an independent attestation.

All advisory application cases match. Tested application commits deliberately
refuse `TXN_TEST_EVIDENCE_UNSUPPORTED`. Matrix rows also distinguish successful
zero-selected-test transactions from native tested admission. The broader
[audit](https://github.com/sley-lang/sley/issues/21) remains open.

See [capabilities](../../CAPABILITIES.md) and [application instructions](../../STOCK_REORDER.md)
for exact commands, scope, host responsibilities and maintenance steps.
