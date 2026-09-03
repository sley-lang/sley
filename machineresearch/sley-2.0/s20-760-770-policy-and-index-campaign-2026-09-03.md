# S20-760 epoch migration policy and S20-770 required contract index (2026-09-03)

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
