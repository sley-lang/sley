# S20-390 extended semantic profile campaign (2026-09-03)

Package: the transaction model revision that lets a receipt name the analysis
that validated its program, dependency S20-360 full operation analysis, phase
M3. Owner of record Merlin; executed by the integrator with every Council lane
unavailable (ADR-0026). No new authority: no test execution, no runtime
effects, no publication.

## Contract

- `docs/adr/ADR-0045-transaction-semantic-profile-names-the-analysis.md` with
  `docs/spec/TRANSACTION_MODEL_V1.md` draft revision 2 (commit metadata table
  and its rules).
- Addendum in `docs/audits/S20_390_ATOMIC_COMMIT_CLOSEOUT.md`; summary section
  `s20_390_atomic_commit`; frontier checker pins the new status.

## Mechanics

| Surface | Change |
|---|---|
| `crates/sley-txn/src/codec.rs` | `SEMANTIC_PROFILE_EXTENDED_OPERATIONS_V1 = 2`, `CommitMetadata::extended_operations_v1`, `CommitMetadata::for_program`, and a decoder that accepts exactly the two metadata triples |
| `crates/sley-txn/src/repository.rs` | `commit` selects the profile from the validated program (`carries_operations`); a trusted genesis keeps profile 1 because it performs no analysis |
| Corpus | the accepted receipt corpus gained the `ORDINARY_EXTENDED` vector, emitted from a real operation-carrying commit; the operation-free vectors are byte-identical |
| Oracle | `oracle/scb1/.../transaction_receipt.py` decodes `semantic_profile`, derives the vector label from it, and accepts both triples, so profile 2 is verified by an implementation sharing no code with Rust |
| Test | `commit_names_the_semantic_profile_that_validated_the_program` covers both directions and the five bound objects of the operation-carrying commit |

## In-flight repairs

1. The first design refused an operation-carrying trusted genesis with
   `TXN_SEMANTIC_PROFILE_UNSUPPORTED` (39023, `3a56342`). Running the release
   demo emitter proved that refusal wrong: the repository test-support genesis
   already installs operation entities, so the "operation-free" wording never
   described a genesis. The refusal and its error code were removed, and the
   revision states the value's real meaning: profile 1 says no operation
   analysis ran in the transaction, which is exactly true of a genesis and of
   an operation-free commit.
2. `make conformance` then failed on the oracle's own unit gate, which pinned
   two accepted vectors; it now expects three and names why.
3. Repeated manual counter syncing after each evidence rebuild was replaced by
   `scripts/sync_evidence_counters.py`, run inside `make evidence-refresh`
   between the two register and dossier builds. It converges in one pass
   because both documents digest their derived entries rather than the summary
   bytes.

## Validation

Landed at `198379d` with the oracle gate repair at `dc6e788`. Tier 1
`make quick` passed at each commit. Tier 2 on 2026-09-03: `make core` exit 0
(12 s, 997 tests), `make adversarial` exit 0 (10 s, 597 tests),
`make transaction-receipt-persistent-fuzz-smoke` exit 0 (9 s),
`make exchange-persistent-fuzz-smoke` exit 0 (12 s). `make conformance` failed
at `198379d` (the oracle unit gate above) and passed after the repair, 19
oracle results PASS. `make v1` was not run: this is a subsystem handoff.

## Open questions for the Council

1. Ariadne: profile 1 now reads "no operation analysis ran in the
   transaction". Is restating a frozen value's meaning acceptable when the
   original phrase never described a trusted genesis, or does the genesis need
   its own value?
2. Nabu: should the semantic profile be derived from the candidate result
   record (which knows what was judged) rather than from the proposed entity
   set the commit path inspects?
3. Vulcan: the corpus now carries one vector per profile; is a rejection
   vector for an unknown metadata triple needed beyond the decoder's
   `TXN_FIELD_SHAPE` unit coverage?
