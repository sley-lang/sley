# What Sley verifies

Sley reports several different kinds of success. Read each result with its
schema epoch, validation or execution profile, exact state and policy context.
`Valid`, an expected-value match, an accepted transaction and a correct
application are different conclusions.

This guide explains the implemented boundaries; it does not qualify a host,
release artifact or proposed profile. A specification's own status and the
evidence for the exact implementation still matter.

## Guarantee matrix

| Question | Evidence and scope | What that evidence does not establish |
|---|---|---|
| Is the representation structurally valid? | Decoding, schema and graph-reference checks establish canonical bytes, admitted fields and resolved references for the selected epoch. See [SCB1](spec/SCB1.md) and [candidate validation](spec/VALIDATION_PROFILE_V1.md). | That the program produces the intended answers, or that every structurally representable feature is supported by checking or execution. |
| Do types, control flow and declarations check? | The supported validation profile checks types, value use, control flow, effect closure and contract/test shapes. [Contract checking](spec/CONTRACT_TEST_PROFILE_V1.md) validates bindings and plans tests; it does not run predicates or prove them true. | General behavioral equivalence, correctness on every input, or support for forms the current profile refuses. |
| Is the change permitted and admissible? | Fresh validation checks the exact base, protected policy, principal, capabilities and declared budgets. The [transaction owner](../crates/sley-txn/src/repository.rs) separately applies its commit gate. The v1 route refuses a nonempty selected-test set; native admission requires its bound execution/resource evidence and separately authorized acceptance statement. | Authority from an imported `Valid` record, a caller's success flag, a signature alone, or yesterday's policy context. Native source code and protocol negotiation alone do not qualify a deployed supervisor. |
| Did tested behavior match? | A recorded observation matches the expected value hash or trap code for the particular inputs. The workbench uses this [comparison rule](../crates/sley-tests/src/compare.rs) for its advisory `pass` result. | Correctness outside those cases, independence or completeness of the oracle, resource compliance, or commit admission. A wrong expectation can match a wrong implementation. |
| Were resources bounded and measured? | The [advisory policy](spec/SLEY_AGENT_V1.md#101-testcase-resource-reporting) applies declared fuel and fixed VM caps, and identifies five unapplied declarations. Native admission additionally verifies bound [measurement attestations and trust](../crates/sley-txn/src/native_commit.rs); host enforcement needs qualification of the actual supervisor, worker and configuration. | That declared budgets were consumed, that VM units equal host bytes, or that an attestation's signature independently proves the measuring/enforcing deployment correct. Wall time and memory observations are host-specific evidence. |
| Is this the same state? | A verified [state root](spec/STATE_ROOT_V1.md) binds the canonical state record and its referenced identities. Transaction identity additionally binds history. Reconstructed matching roots establish identity under the specified hash and encoding rules. | A bug-free program, identical transaction history from the state root alone, cross-host performance, or correctness against user requirements. |
| Was the change durably accepted? | The [transaction model](spec/TRANSACTION_MODEL_V1.md) and [recovery contract](spec/CRASH_RECOVERY_MATRIX_V1.md) govern verified objects, receipt persistence and accepted-head publication. Inspect the accepted transaction and receipt under the relevant profile. | That a draft or submission was committed, or that `.sley/` candidate/metadata publication has the transaction store's durability contract. Storage and deployment assumptions still apply. |
| Does the application solve the user's task? | An explicit requirement set, representative workflows, independent acceptance checks and declared host scaffolding provide evidence for the measured scope. Report omitted, unsupported and failed cases. | A conclusion supplied by `Valid`, a digest or a finite passing test suite alone. Economic advantage additionally needs complete cost and success-rate accounting for an identified artifact and comparison. |

## Reading command results

| Result | Read it as |
|---|---|
| `c1: Valid` | The candidate passed the supported static validation phases in that context. Validation did not advance the accepted head. |
| `tests: 3/3 passed` or `"pass": true` | Those advisory comparisons matched. Read `test_execution_policy` for which resource declarations the runner applies. This is not native admission evidence. |
| `submitted c1` | The workbench wrote the candidate to the submission file. `submit` does not commit it. |
| `committed c1 as transaction …` | The selected commit route returned an accepted transaction. Its receipt/profile determine the evidence admitted; this still says nothing about unspecified application requirements. |
| `sley serve` exits `0` | The stream was processed, possibly including failed answers. Inspect each response's result/error; see [CLI exit codes](QUICKSTART.md#5-commands-and-exit-codes). |

For example, a candidate can be `Valid`, match all advisory cases and be
submitted, yet the v1 commit route still refuses it because selected tests need
native evidence. Removing tests to obtain an empty selection does not establish
tested behavior. Use the separately qualified admission path when that evidence
is required; an advisory result cannot be promoted into it.

Deterministic VM observations are scoped to the same verified program, inputs,
execution profile and relevant request limits. Measured wall time, memory and
deployment qualification are separate. The advisory workbench does not store
its comparisons as authoritative execution reports.

Implementation anchors: [advisory executor](../crates/sley-agent/src/exec.rs),
[CLI result/submit/commit handling](../crates/sley-agent/src/cli.rs),
[static validator](../crates/sley-policy/src/candidate_validation.rs), and
[transaction admission and persistence](../crates/sley-txn/src/repository.rs).
This explanatory matrix addresses [audit #21, item 6](https://github.com/sley-lang/sley/issues/21).
It adds no execution, admission or application guarantee.
