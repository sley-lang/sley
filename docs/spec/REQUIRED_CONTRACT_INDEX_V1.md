# Required Contract Index v1

Status: S20-770 contract draft, revision 2 (2026-09-08); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review). Revision 2 extends row 17.12 with the accepted entity-read
document and the additive version 2 metadata directory; the twelve
required rows and all digest domains are unchanged. The revision 1 history
is retained below and does not review revision 2; its new-delta review is
pending.

## Boundary

Master goal section 17 states that "Sley 2.0 GA MUST define and validate at
least these versioned contracts" and names twelve. Every one of them is
defined and validated in this repository, but nothing mapped the twelve
required names to the documents that define them, the digest domains that
identify them, the checkers that validate them, or the corpora that pin their
bytes. An independent reviewer had to rediscover that mapping.

This index freezes it. It defines no contract of its own, changes no identity,
and grants no authority: it is the traceability record from a required name to
the artifacts that satisfy it.

## 1. The twelve required contracts

| # | Required name | Defining document | Digest domain | Checker | Corpus |
|---:|---|---|---|---|---|
| 17.1 | `sley-scb-object-v1` | `SCB1.md` section 2 (standalone envelope) with `OBJECT_STORE_V1.md` (stored object record) and `MUTATION_SCHEMA_V1.md` (entity body kinds) | `sley2.object.v1` | `check_scb1_spec.py`, `check_object_store_spec.py`, `check_mutation_schema.py` | `conformance/scb1/v1` |
| 17.2 | `sley-state-root-v1` | `STATE_ROOT_V1.md` | `sley2.state-root.v1` | `check_state_root_spec.py` | `conformance/state-root/v1` |
| 17.3 | `sley-context-capsule-v1` | `CONTEXT_CAPSULE_PROFILE_V1.md` with `RESTRICTED_QUERY_CAPSULE_PROFILE_V1.md` | `sley2.context-capsule.v1` | `check_context_capsule_profile.py`, `check_restricted_query_capsule_profile.py` | `conformance/context-capsule/v1` |
| 17.4 | `sley-candidate-v1` | `CANDIDATE_RECORD_V1.md` with `MUTATION_VALUE_CODEC_V1.md` | `sley2.candidate.v1` | `check_candidate_contract_freeze.py`, `check_mutation_value_codecs.py` | `conformance/mutation-candidate/v1`, `conformance/mutation-value/v1` |
| 17.5 | `sley-candidate-result-v1` | `CANDIDATE_RESULT_V1.md` | `sley2.candidate-result.v1` | `check_candidate_result_contract.py` | `conformance/candidate-result/v1` |
| 17.6 | `sley-transaction-v1` | `TRANSACTION_MODEL_V1.md` | `sley2.transaction.v1`, `sley2.transaction-receipt.v1` | `check_transaction_contract.py` | `conformance/transaction-receipt/v1` |
| 17.7 | `sley-policy-v1` | `POLICY_ROOT_V1.md` | `sley2.policy-root.v1` | `check_policy_root.py` | native vectors in `crates/sley-policy` |
| 17.8 | `sley-capability-token-v1` | `CAPABILITY_TOKEN_V1.md` with `CAPABILITY_SUMMARY_V1.md` | `sley2.capability-token.v1` | `check_capability_token.py` | native vectors in `crates/sley-policy` |
| 17.9 | `sley-execution-report-v1` | `REPORT_ENVELOPE_PROFILE_V1.md` section on the execution envelope | `sley2.execution-report.v1` | `check_report_envelope_profile.py` | `conformance/release-demo/v1` (identity re-derived independently) |
| 17.10 | `sley-test-report-v1` | `REPORT_ENVELOPE_PROFILE_V1.md` section on the test envelope with `CONTRACT_TEST_PROFILE_V1.md` | `sley2.test-report.v1` | `check_report_envelope_profile.py`, `check_contract_test_profile.py` | native vectors; no test has been executed, so no corpus of produced reports exists. `REPORT_ENVELOPE_PROFILE_V1.md` section 9.1 states the exact reason: four of the six declared TestCase resource units have no epoch-1 VM counterpart |
| 17.11 | `sley-repository-pack-v1` | `REPOSITORY_PACK_V1.md` with `REPOSITORY_EXCHANGE_V1.md` | `sley2.repository-pack.v1` | `check_repository_pack_spec.py`, `check_repository_exchange_spec.py` | `conformance/repository-pack/v1`, `conformance/repository-exchange/v1` |
| 17.12 | `sley-protocol-handshake-v1` | `SMP1.md` sections 2 and 3 with `ENTITY_READ_PROFILE_V2.md` (accepted entity-read extension) | `sley2.protocol-handshake.v1`, `sley2.protocol-frame.v1` | `check_smp1_contract.py` | `conformance/smp1/v1`, `conformance/smp1-json-bridge/v1`, `conformance/smp1-json-bridge/v2` |

## 2. Rules

- Every required name maps to at least one defining document that exists, at
  least one checker that `make quick` runs, and a digest domain frozen by
  S20-110 `IDENTIFIERS_V1.md`.
- A required name whose document, checker, or domain is missing is
  `CONTRACT_INDEX_UNSATISFIED`, and the index fails.
- The corpus column may read that no corpus exists, but only with the reason,
  as `sley-test-report-v1` does: no test has been executed under any authority,
  so no produced report exists to pin. A reason must itself be checkable, and
  that one is: the S20-290 checker fails if the VM gains any of the four
  resource units whose absence is the reason.
- The index is traceability only. It does not restate a contract's rules, and
  where this table and a defining document disagree, the document wins.

## 3. What this index is not

- Not a claim that the twelve contracts are complete for GA: several are
  implemented under draft revisions with Council reviews pending, which the
  machine summary records per package.
- Not an acceptance record, a freeze, or a review.
- Not a substitute for the S20-740 finding register or the S20-750 dossier.

## 4. Codes

S20-770 reserves 77000 and 77001: `CONTRACT_INDEX_UNSATISFIED` (77000) when a
required name loses its document, checker, or domain, and
`CONTRACT_INDEX_DRIFT` (77001) when the index names an artifact that does not
exist.

## 5. Staging

`scripts/check_required_contract_index.py` runs under `make quick`. Statuses:
`S20_770_CONTRACT_DRAFT_REVIEW_PENDING` and `S20_770_INDEX_ACCEPTED` after the
three reviews read `PASS`. The checker parses this table, verifies every named
document, checker, and corpus path exists, verifies each digest domain appears
in `IDENTIFIERS_V1.md`, and verifies each checker is invoked by the Makefile.

## 6. Revision history

- Revision 1 (2026-09-03): the twelve required names mapped to their
  defining documents, digest domains, checkers, and corpora.
- Revision 2 (2026-09-08): row 17.12 gains the accepted
  `ENTITY_READ_PROFILE_V2.md` document and the additive
  `conformance/smp1-json-bridge/v2` metadata directory alongside the
  existing mappings. No thirteenth identity is added and no required name
  is renamed: the versioned method metadata is additive coverage of the
  same `sley-protocol-handshake-v1` name.
