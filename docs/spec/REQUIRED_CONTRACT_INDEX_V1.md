# Required Contract Index v1

Status: S20-770 contract draft, revision 3 (2026-09-14); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review). Revision 2 extends row 17.12 with the accepted entity-read
document and the additive version 2 metadata directory; the twelve
required rows and all digest domains are unchanged. Revision 3 (2026-09-14)
is a curative-notes amendment against the three 2026-09-04 Council reviews
(all FAIL, no P0): it names the true definers in rows 17.1 and 17.12 with
numbered sections, lists the bridge checker in row 17.12, states the
machine-readable non-acceptance in section 3, documents the full checker in
section 5, and adds section 7 curative notes prescribing what the FINAL
draft must contain per finding. The revision 1 history
is retained below and does not review revision 3; its new-delta review is
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
| 17.1 | `sley-scb-object-v1` | `SSMC1_EPOCH1_SCHEMA.txt` (entity body kinds definer) with `SCB1.md` section 2 (standalone envelope), `OBJECT_STORE_V1.md` (stored object record), and `MUTATION_SCHEMA_V1.md` (generated descriptor layer over the manifest) | `sley2.object.v1` | `check_scb1_spec.py`, `check_object_store_spec.py`, `check_mutation_schema.py` | `conformance/scb1/v1` |
| 17.2 | `sley-state-root-v1` | `STATE_ROOT_V1.md` | `sley2.state-root.v1` | `check_state_root_spec.py` | `conformance/state-root/v1` |
| 17.3 | `sley-context-capsule-v1` | `CONTEXT_CAPSULE_PROFILE_V1.md` with `RESTRICTED_QUERY_CAPSULE_PROFILE_V1.md` | `sley2.context-capsule.v1` | `check_context_capsule_profile.py`, `check_restricted_query_capsule_profile.py` | `conformance/context-capsule/v1` |
| 17.4 | `sley-candidate-v1` | `CANDIDATE_RECORD_V1.md` with `MUTATION_VALUE_CODEC_V1.md` | `sley2.candidate.v1` | `check_candidate_contract_freeze.py`, `check_mutation_value_codecs.py` | `conformance/mutation-candidate/v1`, `conformance/mutation-value/v1` |
| 17.5 | `sley-candidate-result-v1` | `CANDIDATE_RESULT_V1.md` | `sley2.candidate-result.v1` | `check_candidate_result_contract.py` | `conformance/candidate-result/v1` |
| 17.6 | `sley-transaction-v1` | `TRANSACTION_MODEL_V1.md` | `sley2.transaction.v1`, `sley2.transaction-receipt.v1` | `check_transaction_contract.py` | `conformance/transaction-receipt/v1` |
| 17.7 | `sley-policy-v1` | `POLICY_ROOT_V1.md` | `sley2.policy-root.v1` | `check_policy_root.py` | native vectors in `crates/sley-policy` |
| 17.8 | `sley-capability-token-v1` | `CAPABILITY_TOKEN_V1.md` with `CAPABILITY_SUMMARY_V1.md` | `sley2.capability-token.v1` | `check_capability_token.py` | native vectors in `crates/sley-policy` |
| 17.9 | `sley-execution-report-v1` | `REPORT_ENVELOPE_PROFILE_V1.md` sections 2 and 5 (execution envelope) | `sley2.execution-report.v1` | `check_report_envelope_profile.py` | `conformance/release-demo/v1` (identity re-derived independently) |
| 17.10 | `sley-test-report-v1` | `REPORT_ENVELOPE_PROFILE_V1.md` sections 6 and 7 (test envelope) with `CONTRACT_TEST_PROFILE_V1.md` | `sley2.test-report.v1` | `check_report_envelope_profile.py`, `check_contract_test_profile.py` | native vectors; no test has been executed, so no corpus of produced reports exists. `REPORT_ENVELOPE_PROFILE_V1.md` section 9.1 states the exact reason: four of the six declared TestCase resource units have no epoch-1 VM counterpart |
| 17.11 | `sley-repository-pack-v1` | `REPOSITORY_PACK_V1.md` with `REPOSITORY_EXCHANGE_V1.md` | `sley2.repository-pack.v1` | `check_repository_pack_spec.py`, `check_repository_exchange_spec.py` | `conformance/repository-pack/v1`, `conformance/repository-exchange/v1` |
| 17.12 | `sley-protocol-handshake-v1` | `SMP1.md` sections 1 and 2 (framing with `sley2.protocol-frame.v1`, handshake) with `ENTITY_READ_PROFILE_V2.md` (accepted entity-read extension) | `sley2.protocol-handshake.v1`, `sley2.protocol-frame.v1` | `check_smp1_contract.py`, `check_smp1_json_bridge_contract.py` | `conformance/smp1/v1`, `conformance/smp1-json-bridge/v1`, `conformance/smp1-json-bridge/v2` |

## 2. Rules

- Every required name maps to at least one defining document that exists, at
  least one checker that `make quick` runs, and a digest domain frozen by
  S20-110 `IDENTIFIERS_V1.md`.
- Defining documents live under `docs/spec/` and are named with their section
  where the citation is section-scoped (row 17.1 names the `.txt` manifest
  itself as the definer, with the `.md` descriptor layer alongside it).
- A required name whose document, checker, or domain is missing is
  `CONTRACT_INDEX_UNSATISFIED`, and the index fails.
- The corpus column may read that no corpus exists, but only with the reason,
  as `sley-test-report-v1` does: no test has been executed under any authority,
  so no produced report exists to pin. A reason must itself be checkable, and
  that one is: the S20-290 checker fails if the VM gains any of the four
  resource units whose absence is the reason. Every corpus cell is non-empty;
  an empty cell fails the checker.
- The digest-domain column proves registry presence; derivation is asserted
  separately by the registry-drift gate, which requires the implementation's
  derived domains and the frozen registry to be the same set in both
  directions (a derived domain the registry does not freeze is drift; a
  frozen row no crate derives is a phantom).
- The index is traceability only. It does not restate a contract's rules, and
  where this table and a defining document disagree, the document wins.

## 3. What this index is not

- Not a claim that the twelve contracts are complete for GA: several are
  implemented under draft revisions with Council reviews pending, which the
  machine summary records per package. Acceptance of this index asserts
  traceability only: the machine summary carries
  `asserts_contract_acceptance: false` and `asserts_completeness: false`
  beside the status, so no downstream consumer may render an
  `S20_770_INDEX_ACCEPTED` status as contract acceptance.
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
document (`.md` and `.txt` under `docs/spec/`), checker (membership in the
`quick` recipe, not a whole-file substring), and corpus path exists, verifies
each digest domain appears backticked in `IDENTIFIERS_V1.md`, requires every
corpus cell to be non-empty, runs the identifier-registry drift gate over
every crate in both directions (drift and phantom), validates the
machine-summary status, contract revision, current-delta review binding,
traceability disclaimers, and required fields, verifies the 77000/77001
symbols in `ERROR_CODES_V1.md`, and requires the `WORK_PACKAGES.md`
reference.

## 6. Revision history

- Revision 1 (2026-09-03): the twelve required names mapped to their
  defining documents, digest domains, checkers, and corpora.
- Revision 2 (2026-09-08): row 17.12 gains the accepted
  `ENTITY_READ_PROFILE_V2.md` document and the additive
  `conformance/smp1-json-bridge/v2` metadata directory alongside the
  existing mappings. No thirteenth identity is added and no required name
  is renamed: the versioned method metadata is additive coverage of the
  same `sley-protocol-handshake-v1` name.
- Revision 3 (2026-09-14): curative-notes amendment against the three
  2026-09-04 Council reviews (all FAIL, no P0). Row 17.1 names
  `SSMC1_EPOCH1_SCHEMA.txt` as the entity-body-kinds definer; row 17.12
  cites `SMP1.md` sections 1 and 2 and lists the bridge checker; rows
  17.9/17.10 cite numbered envelope sections; section 2 states the
  `docs/spec/` scope, the non-empty corpus rule, and the presence-vs-
  derivation split; section 3 states the machine-readable non-acceptance;
  section 5 documents the full checker; section 7 curative notes added.
  Twelve rows and all digest domains unchanged.


## 7. Curative notes (revision 3, 2026-09-14)

These notes answer every finding of the three 2026-09-04 Council reviews
(Ariadne contract, Nabu architecture, Vulcan surface; all FAIL, no P0) with
implementable directives for the FINAL draft. Items marked DONE IN REV 3 are
closed by the revision 3 table, rules, sections, or checker above; items
marked CLOSED BY ROUND 9 were already closed by the invariant-audit repair
that enforces the drift gate in both directions (retained, not re-argued);
all other items are owned work the FINAL draft must land, with the owner
named. This index defines no contract, changes no identity, and grants no
authority.

### Ariadne contract review

- A-P1-1 (17.1 mis-attributes the entity body kinds): DONE IN REV 3. Row
  17.1 now names `SSMC1_EPOCH1_SCHEMA.txt` as the definer with
  `MUTATION_SCHEMA_V1.md` as the generated descriptor layer, and the checker
  validates `.txt` definers under `docs/spec/`.
- A-P1-2 (17.12 cites sections defining none of its listed domains): DONE IN
  REV 3. Row 17.12 cites sections 1 (framing; `sley2.protocol-frame.v1` at
  the `ProtocolFrameId` definition) and 2 (handshake). Whether
  `sley2.session.v1` belongs in this row's domain column with S20-330 as a
  defining document is an open FINAL decision (see N-P1-4 ownership note):
  either keep sections 1-2 with the two listed domains, or add the session
  domain and cite sections 1 through 3.
- A-P1-3 (ADR-0047 records no registry decision): DONE IN REV 3.
  ADR-0047 decision 5 (2026-09-14 record) names the four already-derived
  domains with their owning packages and fixtures, and its boundary states
  that the work derives no new identifier while registering four
  already-derived domains.
- A-P2-4 (drift checked in one direction only): CLOSED BY ROUND 9. The
  checker enforces both directions (`identifier-registry-drift` and
  `identifier-registry-phantom`); live state is 50 derived vs 50 registered
  with zero phantoms, and the reverse direction was demonstrated with a
  synthetic phantom row. Retained.
- A-P2-5 (`IDENTIFIERS_V1.md` says sixteen ADR-0048 rows; ADR-0048 and
  commit `0d57ec5` say fifteen, and 31 + 4 + 15 = 50): DONE alongside rev 3
  as a one-word registry correction (`IDENTIFIERS_V1.md`: "sixteen more" to
  "fifteen more"). Registry owner S20-110 confirms the count on its next
  amendment.
- A-P2-6 (section 5 under-describes the checker): DONE IN REV 3. Section 5
  now lists every behaviour including the drift gate both directions, the
  summary/delta-review binding, the code symbols, and the work-package
  reference.
- A-P2-7 (77000/77001 reserved but never emitted): OPEN, FINAL checker work.
  FINAL must bind the frozen codes to checker outputs: a missing document,
  checker, or domain fails as `CONTRACT_INDEX_UNSATISFIED` (77000) and a
  drift or phantom fails as `CONTRACT_INDEX_DRIFT` (77001), instead of
  untyped `problems` strings. The symbols are verified present in
  `ERROR_CODES_V1.md` today; emitting them changes failure strings, so the
  change ships with the FINAL draft, not silently.
- A-P2-8 (definers validated only as `.md`): DONE IN REV 3. The checker
  accepts `.md` and `.txt` definers under `docs/spec/`; anything else fails.
- A-P2-9 (section citations unverified prose): HALF DONE IN REV 3. File
  existence (plus `.txt`) is checked and cells are normalized to file plus
  numbered section. OPEN half: FINAL must make section pointers checkable
  (file plus heading anchor for numbered sections; the prose-form pointers
  are not machine-checkable as written) so a renumbered or split document
  cannot orphan a pointer with every assertion green.
- A-P3-10 (17.9/17.10 cite unnumbered envelope sections): DONE IN REV 3
  (sections 2/5 and 6/7).
- A-P3-11 (`### 9.1` sits after `## 10` in `REPORT_ENVELOPE_PROFILE_V1.md`):
  OPEN, owned by S20-290. The pointer in row 17.10 resolves today, but the
  FINAL draft must coordinate the heading repair with the report-envelope
  owner rather than editing a foreign contract here.

### Nabu architecture review

- N-P1-1 (Makefile assertion is a whole-file substring): DONE IN REV 3. The
  checker parses the `quick:` recipe block and requires each named checker
  as a recipe-line member; a checker moved out of `quick` now fails even
  when its name still appears elsewhere in the file.
- N-P1-2 (twelve required names never checked against master-goal section
  17): OPEN, FINAL checker work. The authority lives outside the repository
  (`Sley2.0mastergoal.md` section 17), so the machine cannot read it; FINAL
  must freeze the twelve `(number, name)` pairs as a checker constant
  (rejecting renames and duplicate `| 17.n |` rows, which the current
  `len(rows) == 12` guard accepts) and must state in section 2 that the
  authority column is verified by transcription, not by machine. The
  transcription was verified correct at review time.
- N-P1-3 (drift check discriminates by file location, not semantic role):
  OPEN, FINAL checker work with S20-110. The `sley2.` namespace is
  overloaded with non-hash contract labels (22 in `scripts/`/`oracle/`
  today); the check is correct only by file-extension coincidence. FINAL
  must derive hash domains from the domain constant declaration site in
  `sley-id` / the hashing crates instead of a namespace regex over all
  sources, so the first Rust literal carrying a contract id cannot corrupt
  the identity registry to satisfy a traceability checker. Do not widen the
  regex to `scripts/` without the scope decision in V-P2-c.
- N-P1-4 (registry drift is an S20-110 invariant hosted under S20-770
  codes): OPEN, ownership decision owed by S20-110. Either extract the drift
  gate to `scripts/check_identifier_registry.py` under an S20-110-reserved
  code invoked from `make quick` and reduce S20-770 to the domains its own
  table names, or record in this index and ADR-0047 that S20-770 temporarily
  hosts the S20-110 invariant with the owner and migration named. Until
  then ownership is forked in the artifact whose purpose is ownership; the
  coverage itself must not be dropped (no S20-110 checker exists).
- N-P2-5 (drift one-directional): CLOSED BY ROUND 9 (see A-P2-4).
- N-P2-6 (no-corpus escape hatch unenforced): HALF DONE IN REV 3. The
  checker now requires every corpus cell to be non-empty, and section 2
  states the checkability rule. OPEN half: FINAL must link row 17.10 to the
  S20-290 resource-unit assertion by reference (checker-to-checker pin or an
  explicit reviewed-prose exception), because a future "no corpus, because
  reasons" row still passes the gate on authorial care alone.
- N-P2-7 (section pointers unchecked): same work as A-P2-9 OPEN half.
- N-P3-8 (bare-substring vs backticked domain match): DONE IN REV 3. Both
  the table-domain check and the drift check use the backticked form, so a
  prose mention of a withdrawn domain satisfies neither.
- N-P3-9 (definers implicitly constrained to `docs/spec/`): DONE IN REV 3.
  Section 2 states the rule; the checker enforces it.
- N-P3-10 (`rglob` walks `crates/target` before filtering): DONE IN REV 3.
  The traversal prunes `target/` directories during the walk.

### Vulcan surface review

- V-P1 (acceptance token has no machine-readable qualifier): DONE IN REV 3.
  The summary carries `asserts_contract_acceptance: false` and
  `asserts_completeness: false` beside the status (checker-enforced), and
  section 3 states that `S20_770_INDEX_ACCEPTED` asserts traceability only.
- V-P2-a (drift one-directional): CLOSED BY ROUND 9 (see A-P2-4). No
  allowlisted compiled-out domain exists today; FINAL must name one
  explicitly if it introduces a legitimately compiled-out domain (the
  `sley2.s20-530.recovery-ancestry-test-plan.v1` candidate stays registered
  and derived, so it needs no allowlist).
- V-P2-b (rule 3 unenforced by this checker): same work as N-P2-6.
- V-P2-c (22 further `sley2.*.v1` identifiers in Python tooling outside any
  registry): SUBSTANTIALLY CLOSED. `IDENTIFIERS_V1.md` now states the scope
  boundary (registry scope is the crate implementation; `scripts/`/`bench/`
  labels are not hash domains) and `scripts/check_domain_tags_and_strings.py`
  asserts no such label is used by a script that imports or calls blake3,
  with evasion regressions in `--self-test`. FINAL keeps the boundary and
  the gate; widening the drift check to `scripts/` requires the Ariadne/Maat
  scope decision first.
- V-P2-d (registry cites ADR-0047 for an unrecorded change): DONE IN REV 3
  (see A-P1-3).
- V-P3-a (match strictness): DONE IN REV 3 (see N-P3-8).
- V-P3-b (section 9.1 guard fails open on rename/relocation): OPEN,
  upstream of S20-770 (S20-290 owns `check_report_envelope_profile.py`).
  Row 17.10's checkable reason holds today (four literal markers in
  `crates/sley-vm/src/execute.rs`); FINAL must coordinate a path- and
  rename-resistant guard with the S20-290 owner.
- V-P3-c (Makefile substring, no target association): DONE IN REV 3 (see
  N-P1-1).
