# S20-530 v8 hybrid proof-path amendment design

Status: SETTLED DESIGN - READY FOR IMPLEMENTATION

Owner: Codex orchestrator

Decision date: 2026-08-31, America/New_York

## Decision

S20-530 v8 will use a tightly defined hybrid proof path.

The checker must retain every entry-path and control-ancestry record it can
resolve statically. It may assign an exact manual-review disposition only to
the complete checker-generated set of records that remain unresolved. A full
Rust static resolver is not required for v8, and a broad manual bypass is not
permitted.

The current exact unresolved baseline is:

- 20 entry records across 13 of 30 events, consisting of 16 call-edge records
  and four reverse-caller inventories;
- 1,666 control-ancestry atoms across 23 of 30 events.

These values are candidate freeze inputs, not count-only allowances. The v8
contract must bind every ordered leaf and reject any record, order, source, or
digest drift.

No Rust production source, public API, recovery behavior, matrix row, limit,
runtime observation, failure precedence, or test membership changes under
this decision.

## Governing Council rulings

The decision is supported by clean, read-only Council handoffs against clean
HEAD `b6d08f484094319eed8f9bc4d82135a2fc750090` and the same six immutable
input digests.

### Ariadne

Verdict: `PASS_V8_SLEY_CONTRACT_REPAIRED`.

Ariadne ruled that a complete resolver is not required when exceptions
preserve unresolved obligations, remain exact and digest-bound, and retain
fresh final implementation reviews. The full review is bound to session
`forge-ariadne-20260831T004417-162948-31084`:

- session JSON SHA-256:
  `2aade2cf47fe60f5ddd0981f8627454a0f772dc843ebce784cf5ad13e3280b75`;
- trajectory SHA-256:
  `ef99b2a4d04a2606a75d346ab36cfed0260c4b27107df4abfa2e8a7faf83b7c6`.

Its clean repair confirmation is session
`forge-ariadne-20260831T005413-171724-506`:

- session JSON SHA-256:
  `7a9a77407e4f6b96b5acaa150f2e94669e4858ef2a6120e1df0b8230a4187d6a`;
- trajectory SHA-256:
  `8c6bff1d354166d3c1a15eb7421b525b85e10ba77e1392996b1912049021d9ae`.

### Nabu

Verdict: `PASS_V8_ARCHITECTURE`.

Nabu selected the tightly defined hybrid, assigned completeness to the
checker-generated inventories, assigned ordered exception records to the test
plan, and required a spec/ADR proof-path amendment before refreeze. The clean
review is session `forge-nabu-20260831T005454-173405-7229`:

- session JSON SHA-256:
  `b84d80305a3d0e8e66b5f145a8dcb23ce7fa77155811aca7f1b483e1d4d1c905`;
- trajectory SHA-256:
  `1f9816b49e603ab5bab9df51e4d98fa5ab053dddff497fe0b9b846d687725a4b`.

### Vulcan

Verdict: `PASS_V8_SECURITY_DESIGN`.

Vulcan accepted the hybrid only with an independently implemented
reconciliation path, exact closed schemas, hostile partition controls, and
review receipts checked against trusted local orchestration authority. The
clean review is session `forge-vulcan-20260831T010430-182273-24947`:

- session JSON SHA-256:
  `7d712b006ea29300293efd854ffbb17480209c8244afc3e57669107e9dc902ae`;
- trajectory SHA-256:
  `266f4b711825898f7602d836e3513299587990d4704511307b498d68ed12f9a1`.

Earlier overlapped or warning-bearing handoffs are non-authoritative. They
must not be cited as v8 review evidence.

## Required proof-path language

The specification and ADR must explicitly permit the following equivalent
proof path before v8 refreeze:

1. Generate complete entry-path and control-ancestry manifests for all 30
   governed events from the exact reviewed source set.
2. Preserve every statically resolved record as `STATIC_PASS`.
3. Permit `EXACT_MANUAL_REVIEW_EXCEPTION` only for a generated unresolved
   leaf that exactly matches the frozen exception ledger.
4. Give an exception only the meaning
   `UNRESOLVED_BY_V8_STATIC_RESOLVER` and
   `FINAL_SPECIALIST_REVIEW_REQUIRED`.
5. Never represent an exception as statically resolved, trusted, ignored,
   allowed, unreachable, or a false positive.
6. Retain all runtime observations, N/N+1 limit cases, public-root evidence,
   dual-site proofs, source scans, and final implementation reviews.
7. Reject production changes made solely to fit resolver incompleteness.

The current absolute static-resolution language cannot remain unchanged while
accepting exceptions. The v8 spec and ADR edits must be narrow, explicit, and
digest-bound. The immutable v7 specification, ADR interpretation, checker,
evidence, and reviews remain historical authority for v7.

## Complete inventory and partition contract

The checker must generate the pre-exception inventory before reading or
applying the exception ledger.

For each proof layer:

```text
STATIC_PASS intersect EXCEPTIONS = empty
STATIC_PASS union EXCEPTIONS = COMPLETE_GENERATED_INVENTORY
```

The equality is exact and ordered. The checker must fail on:

- a missing, additional, duplicated, or reordered record;
- a newly unresolved record;
- an exception that now resolves statically;
- a record whose source, owner, function, attribute chain, body, lexical site,
  parent manifest, or canonical leaf digest changes;
- an unknown layer, record kind, reason code, event, or ordinal;
- a null, optional, wildcard, prefix, range, or reason-only identity.

The checker owns source discovery, manifest generation, static classification,
schema validation, partition equality, and hostile self-controls. The v8 test
plan owns the exact ordered exception ledgers. Closeout evidence copies only
their canonical digests and counts, never a relaxed alternate inventory.

## Exception schemas

All objects use exact ordered JSON fields. Unknown, omitted, duplicated, or
reordered fields fail closed.

### Common fields

Every exception record contains:

1. `layer`;
2. `record_kind`;
3. `reason_code`;
4. `event_id`;
5. `event_ordinal`;
6. `record_ordinal`;
7. `source`;
8. `source_sha256`;
9. `owner`;
10. `function`;
11. `signature_sha256`;
12. `attribute_chain_sha256`;
13. `body_sha256`;
14. `lexical_site_sha256`;
15. `parent_manifest_sha256`;
16. `unresolved_record_sha256`;
17. `canonical_leaf_sha256`.

Closed values are:

- `layer`: `ENTRY_PATH` or `CONTROL_ANCESTRY`;
- `reason_code`: `UNRESOLVED_BY_V8_STATIC_RESOLVER`;
- disposition at the enclosing ledger:
  `FINAL_SPECIALIST_REVIEW_REQUIRED`.

### Entry variants

Entry call-edge records add exact path and edge ordinals, caller, callee,
call-site digest, and generated edge digest. Reverse-caller records add the
target callee, complete ordered caller inventory, and inventory digest.

The only `record_kind` values are:

- `STATIC_ENTRY_CALL_EDGE_UNRESOLVED`;
- `STATIC_REVERSE_CALLER_INVENTORY_UNRESOLVED`.

### Control variants

Control records add exact scope ordinal, atom ordinal, atom kind, ordered-scope
digest, and generated atom digest.

The only control `record_kind` value is:

- `STATIC_CONTROL_AUTHORITY_UNRESOLVED`.

The final implementation may normalize field names needed to match the
checker-owned canonical records, but any schema change after final review
invalidates that review and requires a new settled digest.

## Top-level digest bindings

Each ledger and the v8 test plan bind:

- contract-set digest;
- validated commit;
- corrected ordered 31-source closure and digest;
- exact ordered 14-site feature-gate registry and digest;
- positional scanner implementation and self-contract digests;
- complete pre-exception entry and control manifest digests;
- ordered static-pass set digests;
- ordered exception set digests;
- per-event counts and ordered leaf digests;
- review-free evidence-payload digest;
- final Nabu, Ariadne, and Vulcan receipt set digest.

The corrected shared-state manifest remains exactly 31 sources, two allowed
channels, and SHA-256
`635f38325ec9ce9ecf01f5e057b8d5c3d009ae3ccbea25bd106f35cae33611f1`.

## Independent reconciliation

The main checker cannot be its own only completeness witness. V8 must add a
small separately implemented reconciliation validator over serialized
manifests. It must not import resolver helpers or exception-generation logic.

The reconciler must:

1. validate exact schemas and canonical JSON encoding;
2. validate all per-event counts and ordered leaf digests against frozen v8
   values;
3. prove the disjoint and complete partition for both layers;
4. reject cross-event swaps, reordered leaves, hidden omissions, duplicate
   leaves, and count-preserving substitutions;
5. validate enclosing source, gate, scanner, manifest, ledger, and contract
   digests;
6. emit a deterministic reconciliation payload consumed by v8 evidence.

Hostile tests must mutate the serialized inventories independently of the
main producer so a common producer/validator omission does not pass merely by
recomputing a count or enclosing digest.

## Mechanical checker amendments

V8 also includes three already-proven mechanical corrections:

1. Replace the four allocation-heavy raw-string matching expressions with the
   position-based equivalent, then prove byte-for-byte mask and value-for-value
   literal parity.
2. Freeze all 14 exact feature-gated sites plus the separate exact
   `cfg(test)` ref-operation site, producing the corrected 31-source closure.
3. Emit all 50 helper records with exact field order `source`, `owner`,
   `function`, `attribute_chain_sha256`, `body_sha256`.

The scanner hostile corpus covers nested and unterminated comments, raw-string
boundary hash counts, byte and C strings, escaped literals, chars, lifetimes,
Unicode, malformed input, and token-prefix boundaries. Frozen and positional
scanners must accept, reject, mask, and extract identically.

## Reviewer provenance

External signatures are not required for the declared local-repository threat
model. V8 does not claim resistance to compromise of the trusted host,
OpenClaw bootstrap, validation UID, or operator session authority.

Fresh final Nabu, Ariadne, and Vulcan reviews must bind:

- reviewer role and phase verdict;
- unique request nonce and request digest;
- session ID, timestamp, session JSON digest, and trajectory digest;
- final contract-set and validated-commit digests;
- complete source-set, scanner, entry ledger, control ledger, and
  review-free evidence digests.

The orchestrator must verify receipt existence and digests against trusted
local session authority before recording final review evidence. Repo-authored
review rows or hashes alone are insufficient. The deterministic checker may
validate the copied receipt schema and bindings, but final closeout must also
record the successful external receipt-verification command and payload.

External signatures become mandatory only if a future contract claims
resistance to a malicious committer, validation-UID actor, or compromised
local orchestration authority.

## Hostile controls

The v8 self-contract and reconciler must reject at least:

- source, contract, gate, scanner, and enclosing-manifest drift;
- record deletion, insertion, duplication, reordering, and cross-event swaps;
- stale exceptions and exceptions for statically resolved leaves;
- new same-file or new-source direct, UFCS, trait, macro, and function-pointer
  callers;
- broadened, moved, nested, duplicated, renamed, or added test gates;
- caller, callee, receiver, function, owner, or source substitution;
- conditionally delegated paths, preceding early exits, literal-derived
  controls, hidden filesystem sentinels, shared-state gates, and collection
  mutation;
- unsigned-in-scope, wrong-role, wrong-phase, wrong-request, wrong-commit,
  missing-session, stale, replayed, or digest-drifted review receipts;
- helper record field drift, duplicate authority, malformed digest, missing or
  ambiguous declaration, attribute-only mutation, and body-only mutation;
- frozen-versus-positional scanner disagreement on source or hostile corpus.

## Unchanged invariants

- 100 matrix rows and `MATRIX_ROWS_SHA256`;
- 30 limit events and `LIMIT_EVENT_SPECS_SHA256`;
- 37 limit fields, 42 field-event memberships, and 37 runtime cases;
- 50 helper authorities and five dual-site fields;
- 419 mapped tests and 232 corruption fixtures;
- all public-root, lock, limit, error, no-mutation, no-partial-report, runtime,
  and failure-precedence semantics;
- contract refreeze and freeze-anchor checkpoint remain separate commits;
- any final-byte change invalidates all final v8 reviews.

## Implementation sequence

1. Amend the spec and ADR with the exact hybrid proof-path language.
2. Implement and hostile-test the positional scanner parity change.
3. Freeze the exact 14-site registry and corrected 31-source closure.
4. Complete the five-field helper producer and self-controls.
5. Generate canonical unresolved entry and control inventories.
6. Define exact ledgers and implement partition enforcement.
7. Implement the separate serialized-manifest reconciler and hostile corpus.
8. Add receipt schemas and trusted-session verification evidence.
9. Run Tier 1 checks throughout and Tier 2 contract refreeze validation on the
   settled candidate.
10. Obtain fresh final Nabu, Ariadne, and Vulcan reviews against exact bytes.
11. Create v8 evidence and one coherent contract-refreeze commit.
12. Record the exact freeze commit in a separate checkpoint commit.

The full `make v1` gate remains deferred because this is not a release
boundary.

## Running estimate

At design settlement:

- overall Sley 2.0 completion: 44%, moderate confidence;
- active S20-530 completion: 80%, moderate confidence.

The S20-530 estimate is the median of the orchestrator estimate and the clean
independent Ariadne, Nabu, and Vulcan estimates. Remaining work is dominated by
checker implementation, independent reconciliation, the hostile corpus,
receipt binding, v8 refreeze evidence, and fresh final-byte reviews.
