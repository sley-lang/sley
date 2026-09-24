# Review request REQ-10 — CONTEXT bounded discovery contract delta

- Baseline tree: `b0e52e00a2bacfd7a874f0b4a7973a352508cebe` on
  `work/succession-sley20-arm` (verify: `git rev-parse HEAD`; tree clean).
  Packet identity: sha256 of this file at dispatch (recorded in the gate record).
- Lane: Nabu (architecture: new served read path). Explicitly out of scope:
  Vulcan review of the new method's budget/continuation abuse surface (follows
  on the implementation) and Ariadne review of the profile-text composition.
  F1/Bool manifest literals and private impact-role lists are retired by the
  de-literalized judge and must not be revived by the verdict.
- Governing clauses: `RESTRICTED_QUERY_PROFILE_V1.md`, `CONTEXT_CAPSULE_PROFILE_V1.md`,
  `ROOT_BACKED_QUERY_PROFILE_V1.md`, `COMPLETE_ENTITY_IMPACT_PROFILE_V1.md`;
  no-whole-store rule; cumulative budgets; continuation behavior
  (omitted/truncated/continuation accounting); mediated-evidence judge route
  (any added member via pre-image diff + structural complete-closure check;
  re-emitted manifest); `bench/live/MEDIATED-INPUT-ACCOUNTING.md`
  ("not established": CONTEXT task-spec gate retained).
- Current limitation (verified by source trace): no served impact-enumeration
  route exists — `crates/sley-protocol/src/server.rs` carries no impact method
  (impact appears only in index-snapshot error mapping and body decoding);
  the owner capability `ImpactGraph::transitive_impact` (bounded reverse
  reachability, canonicality + resource limits, no partials on failure —
  `crates/sley-query/src/lib.rs:460-560`) is unserved. Served reads need IDs,
  server queries need an unmintable snapshot, inventory is whole-store
  (forbidden). The 601/602 diagnostic test routes exist
  (`server.rs:1405,1476`) but are agent-denied (`runner.py:140-141`) and derive
  tests, not entities. The deterministic stand-in receives member identities
  as invocation arguments (disclosed scaffolding in
  `bench/live/tests/test_mediated_context.py`, never a fairness proof).
  Authorized task inputs name no target typedef ("add a required record
  field", no identity or type) and no permitted starting capsule/query handle.
- Proposed delta (smallest integration, additive unless noted):
  1. Task-input amendment (authorized inputs only): name the target record
     typedef and the required-field semantics — task intent supplied, not
     discovered.
  2. One new served bounded read method in the query family exposing
     `transitive_impact` over the verified revision: seeds restricted to
     task-named entity IDs, root-consistent (stale root refuses like 601),
     bounded (`MAX_IMPACT_SEEDS`/`MAX_IMPACT_ENTITIES` + page/continuation with
     omitted/truncated accounting + cumulative budget charge), canonical
     failures closed with no partial result. No whole-store enumeration, no
     caller-supplied closure, no assembled answer set.
  3. Trial allowlist + run bindings for the new method only; claim fields;
     existing profiles and bindings untouched.
- Affected run/tool bindings: trial adapter allowlist, run claim rows, the new
  method's checker + vectors, `MEDIATED-INPUT-ACCOUNTING.md` gate entry.
- Executable acceptance criteria: integrated `execute_attempt` → discovery
  (starting capsule/query handle from authorized inputs only; actual
  continuation where needed) → agent mutation → production admission → real
  oracle → append → verify, with complete impact repair authored through the
  interface; rejection/non-acceptance for incomplete discovery, inconsistent
  continuation, exceeded budgets, and missing authoritative evidence; no
  private IDs as witness arguments, no hidden manifest, no precomputed impact
  closure, no inventory dump. Deterministic proofs stay separate from model
  trials.
- Constraints: read-only review. No file writes. Verdict in reply text only, in
  the REQ verdict format. No register field exists for this scope yet; propose
  the verdict naming convention in the reply.
