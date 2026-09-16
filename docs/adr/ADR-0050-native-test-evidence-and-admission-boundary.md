# ADR-0050: Native test evidence and admission boundary

Status: proposed N0 owner contracts (2026-09-15); accepted architecture with
independent zero-open-issue design review. Wire contracts and implementation
acceptance remain pending. This ADR grants no runtime acceptance by itself.

## Problem

The semantic checker admits pure epoch-1 TestCases and candidate validation
finalizes mandatory selection, but existing reports prove only incomplete
comparison. Four declared resource units lack complete enforcement. Transaction
v1 correctly rejects selected tests, and SMP601/602 are reserved. Removing
those guards would create unsupported accepted history.

## Decisions

1. Preserve epoch-1 semantic rejection, old full-v1 static results, old incomplete
   reports, transaction/receipt v1 guards and admitted vectors.
2. Add distinct native execution, plan, measurement, approval and acceptance
   records. VM owns low-level execution types; tests depends one-way on VM/
   conformance; host supervision stays outside pure owners. N2 precedes N1.
3. Recheck every member of stronger native selection against all protected
   limits; include created/replaced TestCases even when target is unchanged.
4. Enforce native fuel, exact canonical output bytes and per-request depth;
   measure/enforce worker memory and wall time through a root supervisor and
   distinct-credential system-managed workers. Qualify both hosts with actual
   pre-exec placement, cleanup and key-isolation evidence.
5. Keep deterministic reports separate from measured attestations. Preserve
   exact nonsecret historical context and require a separate role-authorized
   commit-admission signature, even when selected tests are empty.
6. Introduce transaction/receipt v2 with complete bound evidence. Atomic writer
   ownership and receipt-before-head durability remain. Never trust a parsed
   report or caller-provided success as commit authority.
7. Add a native exchange format with disjoint identity. Direct bundle records
   and canonical64KiB receipt chunks preserve existing SCB1 Bytes limits;
   older exchange decoders remain unchanged. Receiver controls historical trust.
8. Add SMPv3 feature/method negotiation and explicit report paging/replay/status;
   preserve old entity handles/reserved refusals. Unknown outcome is not retry-safe.
9. Hold unimplemented identities/tags in a separate reservation ledger. Promote
   them to the live implementation registry with code, vectors and review.

## Contracts

* `docs/spec/NATIVE_TEST_EXECUTION_V1.md`: exact VM, deterministic report and
  measured supervisor contracts.
* `docs/spec/NATIVE_TEST_ADMISSION_V1.md`: policy, historical trust, approval,
  transaction/receipt, native exchange and SMP appendices.
* `docs/spec/NATIVE_TEST_RESERVATIONS_V1.md`: collision-checked domains, magics,
  profile/method/feature/error reservations, currently unimplemented.

## Alternatives rejected

Removing guards, promoting old Match to Passed, unit conversion by convention,
same-process timeout, self-authorizing imported keys, unexported report sidecars,
and silently widening old wire profiles all fail the evidence/compatibility
boundary. Pure supported tests do not need a semantic epoch bump; unsupported
observations/contracts/effectful schema still require successor work.

## Consequences and acceptance

New source must implement N2→N1→{N3,N4}→N5→N6→N7, qualify actual packaged
execution/recovery, then receive independent review before native commits
accept. Broader production fingerprints, effects/successor epoch, corpus/live
benchmark/accounting and release gates remain separate implementation tasks.
No past archive, receipt, vector or approval is overwritten by this proposal.
