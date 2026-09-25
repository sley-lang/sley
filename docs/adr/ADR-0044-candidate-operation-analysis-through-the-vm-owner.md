# ADR-0044: candidate operation analysis runs through the VM judgment owner

Status: accepted; implemented 2026-09-03 under the S20-260/S20-270 extended
opcode profile draft with Council review pending; decision 4 corrected
2026-09-05 after the S20-360 full review rounds; no commit,
execution, or runtime authority is added

Date: 2026-09-03

## Context

S20-360 candidate validation could produce `VALID` only for programs whose
projected SSMC1 graph contained no semantic `Operation` entity: anything else
failed phase 12 with `CANDIDATE_OPERATION_ANALYSIS_UNSUPPORTED`. That limit
existed because no owner judged opcode signatures; the restricted VM profile
implemented three Boolean opcodes only.

The extended opcode profile now judges all six families E1 through E6 across
the fifty-two remaining opcodes, with lowering, execution semantics, and
conformance vectors. Its `lower_function` entry, however, refuses a Function
that declares type parameters, effects, or contracts, which candidate programs
legitimately have, and it emits bytecode and a cache key the validator neither
needs nor may derive.

## Decision

1. **One judgment owner, one entry.** The VM crate gains
   `judge_function_operations`: judgment only, no bytecode, no cache key, no
   callee lowering, no execution, and no profile restriction that belongs to
   another owner. `lower_function` is untouched, so the frozen restricted path
   and its vectors are unchanged.
2. **Judged where the graph is known good.** The validator calls it in phase 7,
   after the S20-220 report for the same unit, so a malformed graph is still a
   graph failure and never surfaces as an opcode failure.
3. **Codes are preserved, decisions are the phase's.** A signature or immediate
   mismatch becomes phase 7 `CONTROL_FLOW_ERROR` with the exact `VM_LOWER_*`
   symbol and numeric code in the diagnostic, because the result contract pins
   one decision per phase; a lowering resource ceiling stays a resource limit,
   and anything else is an internal error.
4. **E7 stays unanalyzable and fails closed.** The five E7 opcodes are
   excluded from phase 7 judgment. Only test observation 145 is refused
   unconditionally, by its phase 11 owner. The other four owners validate
   shapes and accept well-formed instances — 144 at phase 10, 160, 161, and
   162 at phase 8 — so the phase 12 guard is the live refusal path for
   well-formed E7 programs, not defense in depth. Opcode 144 is owned since
   profile slice E7a; it stays excluded because its static typing belongs to
   the S20-240 checker.
5. **Work is accounted.** The judgment work joins the phase 7 evidence and the
   operation count joins the phase 12 graph-work total, so an operation-heavy
   candidate meets the same ceilings as any other work.

## Consequences

- Candidate validation now accepts real executable programs, which is the
  precondition for S20-390 full commit of such candidates.
- The valid fixture's result identity changed, because phase 7 evidence gained
  two values; the recorded identity was updated in the same commit.
- `sley-policy` now depends on `sley-vm` (acyclic: the VM depends on check,
  ssmc, id, and mutate only), and the supply-chain inventory records the new
  edges.
- If an E7 owner lands, its opcode becomes analyzable by removing it from the
  excluded set, not by weakening the guard.
