# Executed capability observations

Published 2.0.6 Linux x86_64 workbench, exact executable hash in `result.json`. Finite cases only. A call establishes lowering occurred but does not time it separately.

| Probe | Representation | Validation | Lowering | Advisory execution | Authoritative test evidence | Untested commit | Selected-test commit |
|---|---|---|---|---|---|---|---|
| [checked-integers](rows/checked-integers.json) | canonical candidate created | Valid | observed via successful VM calls | matched finite cases | not supplied by this v1 route | accepted | TXN_TEST_EVIDENCE_UNSUPPORTED |
| [boolean-branches](rows/boolean-branches.json) | canonical candidate created | Valid | observed via successful VM calls | matched finite cases | not supplied by this v1 route | accepted | TXN_TEST_EVIDENCE_UNSUPPORTED |
| [vectors-and-loops](rows/vectors-and-loops.json) | canonical candidate created | Valid | observed via successful VM calls | matched finite cases | not supplied by this v1 route | accepted | TXN_TEST_EVIDENCE_UNSUPPORTED |
| [named-records](rows/named-records.json) | canonical candidate created | Valid | observed via successful VM calls | matched finite cases | not supplied by this v1 route | accepted | TXN_TEST_EVIDENCE_UNSUPPORTED |
| [named-variants](rows/named-variants.json) | canonical candidate created | Valid | observed via successful VM calls | matched finite cases | not supplied by this v1 route | accepted | TXN_TEST_EVIDENCE_UNSUPPORTED |
| [text-equality](rows/text-equality.json) | canonical candidate created | Valid | observed via successful VM calls | matched finite cases | not supplied by this v1 route | accepted | TXN_TEST_EVIDENCE_UNSUPPORTED |
| [ordered-map-lookup](rows/ordered-map-lookup.json) | canonical candidate created | Valid | observed via successful VM calls | matched finite cases | not supplied by this v1 route | accepted | TXN_TEST_EVIDENCE_UNSUPPORTED |
| [floating-addition](rows/floating-addition.json) | canonical candidate created | Valid | observed via successful VM calls | matched finite cases | not supplied by this v1 route | accepted | TXN_TEST_EVIDENCE_UNSUPPORTED |
| [local-cells](rows/local-cells.json) | canonical candidate created | Valid | observed via successful VM calls | matched finite cases | not supplied by this v1 route | accepted | TXN_TEST_EVIDENCE_UNSUPPORTED |
| [direct-calls](rows/direct-calls.json) | canonical candidate created | Valid | observed via successful VM calls | matched finite cases | not supplied by this v1 route | accepted | TXN_TEST_EVIDENCE_UNSUPPORTED |
| [declared-type-parameter](rows/declared-type-parameter.json) | canonical raw candidate created | Valid | VM_LOWER_PROFILE_UNSUPPORTED | not reached | not measured | accepted | not measured |
| [effect-definition](rows/effect-definition.json) | canonical kind exists in schema; raw authoring refuses kind | not reached | not reached | not reached | not reached | not reached | not reached |
| [contract-definition](rows/contract-definition.json) | canonical kind exists in schema; raw authoring refuses kind | not reached | not reached | not reached | not reached | not reached | not reached |

For each executable row, adding only TestCase definitions to unchanged accepted code also commits: the kernel selects zero tests, although the workbench runs the newly authored cases. The separate combined code-and-test candidate selects two tests and refuses at commit. These definition-only commits are not native tested admission; see each row's verdicts and receipts.

Effect/contract rows stop at the unsupported raw-kind reader, before body validation. They do not establish kernel rejection of those semantic kinds or test an effectful runtime. No native supervisor, host effect adapter or authoritative test evidence was exercised.
