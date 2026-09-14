# S20-760 epoch migration policy and S20-770 required contract index (2026-09-03)

Status: contract drafts revision 3 committed 2026-09-14 (curative-notes
amendments against the three 2026-09-04 Council reviews per package, all FAIL,
no P0); re-review pending. No closeout file exists for either draft package
(drafts with no implementation); none created by this wave.

Two master-goal deliverables that had no document. Owner of record Ariadne for
both; executed by the integrator with the Council lanes unavailable (ADR-0026).
Neither creates an epoch, a contract, an identity, or an authority.

## S20-760: the M6 epoch migration policy

The master goal lists "migration policy for Sley 2.x epochs" among the M6
deliverables (section 16.7) and states the seven migration obligations in
section 6.4, but nothing in the repository said when an epoch is required, what
a migration must prove, or who decides. `SCHEMA_EPOCH_V1.md` freezes only the
epoch record.

`docs/spec/EPOCH_MIGRATION_POLICY_V1.md` (draft revision 1, ADR-0046) freezes:

- the nine epoch-forcing facts, with a concrete test: a change needs an epoch
  when it alters the bytes or the acceptance of an artifact an earlier decoder
  produced or accepted;
- profile separation as the preferred alternative, with this phase's two
  precedents (the extended opcode profile's `lowering_profile 2` and bytecode
  `SLEYBC02`, and the transaction semantic profile value 2);
- the seven migration obligations, each with the evidence that discharges it;
- who decides: schema owner, architecture owner, surface owner, operator;
- ordering (one successor at a time, no skipping, no interleaving) and what
  stays true across an epoch;
- an epoch-2 candidate agenda of three items, explicitly proposals rather than
  decisions: the four unsupported contract kinds, the production-epoch
  fingerprint requirement, and the five E7 opcodes.

The staged checker asserts the tree still carries exactly the epoch-1 bootstrap
record, so the policy cannot quietly become an epoch.

## S20-770: the required contract index

Master goal section 17 names twelve contracts GA must define and validate. All
twelve exist, under the names the implementation gave them, but nothing mapped
the required name to its defining document, digest domain, checker, and corpus.

`docs/spec/REQUIRED_CONTRACT_INDEX_V1.md` (draft revision 1, ADR-0047) is that
mapping, machine-checked: every named document, checker, corpus, and crate must
exist, every digest domain must be frozen in `IDENTIFIERS_V1.md`, and every
named checker must actually be invoked by the Makefile. Where the table and a
defining document disagree, the document wins.

## Finding: four unregistered digest domains

The index failed on its first run. `IDENTIFIERS_V1.md` requires that "adding a
domain requires an ADR, fixtures, and registry drift validation", but four
domains had reached production without reaching the registry:

| Domain | Owning package | Specified in |
|---|---|---|
| `sley2.candidate-attempt.v1` | S20-360 | `CANDIDATE_RESULT_V1.md` |
| `sley2.protocol-frame.v1` | S20-410 | `SMP1.md` |
| `sley2.root-query.v1` | S20-310 full | `ROOT_BACKED_QUERY_PROFILE_V1.md` |
| `sley2.session.v1` | S20-330 | `SESSION_HANDLE_PROFILE_V1.md` |

Each was properly specified and fixtured by its own package; only the central
registry was stale. All four are now registered with their owning packages, and
the index checker compares the implementation's thirty-five derived domains
against the registry on every `make quick`, which is the registry drift
validation the section demanded and nothing performed.

## Validation

Landed at `a43f749` (S20-760) and `783fec8` (S20-770). Tier 1 `make quick`
passed at each commit with both new checkers. Tier 2 on 2026-09-03: `make core`
exit 0, `make conformance` exit 0, `make adversarial` exit 0, `make fuzz-smoke`
exit 0. Six review requests are staged (three per package) for the dispatcher.

## Open questions for the Council

They are written into the six request files; the sharpest are whether the five
E7 opcodes need an epoch or a further profile (Ariadne), whether
profile-separation-first risks an unbounded profile space (Nabu), and whether
four domains reaching production unregistered indicates a process gap beyond
those four (Vulcan).

## Revision 3 answers recorded (2026-09-14, curative-notes amendments, no implementation)

S20-760 (`EPOCH_MIGRATION_POLICY_V1.md` revision 3, ADR-0046 revision 3
record): the section 1 epoch test now scopes to the nine facts for an
existing identity, counts additive changes, and permits a profile only under
a new disjoint identity; section 6 citations corrected (item 1 to sections
1.1/1.3 with preamble, item 2 to `CANDIDATE_RESULT_V1.md` phases 7-8 with
`validation_profile_id` field 4, item 3b to section 3.4 with preamble, item
3d to section 3.2 with its MUST); section 7 records the E7a sequencing
correction (determination 3a consumed before schema-owner review;
independent basis is `CONTRACT_TEST_PROFILE_V1.md` section 2); section 8
claims only the bootstrap record, not a tree scan; the checker pins ADR
decision 6 and the item 2 / item 3c determination facts. Open for the FINAL
draft per section 10 notes: multi-descriptor obligation shape, ref-scoped
ordering, the in-place-widening rule, the profile gate, the retention
anchor, the standing decoder invariant, migration atomicity, failure-code
binding, and exact `ContractSource` names.

S20-770 (`REQUIRED_CONTRACT_INDEX_V1.md` revision 3, ADR-0047 revision 3
record with decision 5 registering the four already-derived domains):
row 17.1 names `SSMC1_EPOCH1_SCHEMA.txt` as the entity-body-kinds definer;
row 17.12 cites `SMP1.md` sections 1 and 2 and lists the bridge checker;
rows 17.9/17.10 cite numbered envelope sections; section 2 states the
`docs/spec/` scope, the non-empty corpus rule, and the presence-vs-
derivation split; section 3 carries the machine-readable non-acceptance
(`asserts_contract_acceptance: false`, `asserts_completeness: false`);
section 5 documents the full checker (quick-recipe membership, `.txt`
definers, backticked domains, both-direction drift). The drift gate already
enforces both directions (round-9 repair, 50 vs 50); the Python-tooling
namespace boundary is stated in `IDENTIFIERS_V1.md` with the
`check_domain_tags_and_strings.py` gate. Open for the FINAL draft per
section 7 notes: frozen `(number, name)` authority pairs, declaration-site
domain derivation, drift-ownership extraction, emitted 77000/77001 codes,
checkable section pointers, the 17.10-to-S20-290 linkage, and the
report-envelope heading repair.
