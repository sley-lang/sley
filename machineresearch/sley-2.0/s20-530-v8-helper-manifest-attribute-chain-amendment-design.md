# S20-530 v8 helper-manifest attribute-chain amendment design

Status: INCORPORATED INTO SETTLED V8 HYBRID DESIGN

Owner: Codex orchestrator

Decision date: 2026-08-30, America/New_York

## Purpose

Prepare the smallest mechanical refreeze that makes the frozen S20-530
implementation contract satisfiable. The v7 checker requires every limit
runtime helper-body review record to contain `attribute_chain_sha256`, but the
checker-owned producer cannot emit that field. No possible test plan can pass
both requirements.

The amendment binds the already-governed function-attribute chain into each
helper record. It does not change any helper body, recovery behavior, public
API, matrix row, error, limit, limit event, runtime case, or evidence meaning.

This design does not itself mutate the frozen v7 contract. Publication, push,
deployment, spend, trading, and external runtime mutation remain unauthorized.

## Post-design proof frontier

The helper-manifest amendment is necessary but no longer sufficient for v8.
Subsequent bounded proof-layer audits found incomplete feature-gate authority,
entry-path resolver contradictions, control-ancestry resolver contradictions,
and a measured quadratic scanner hotspot. The complete evidence and required
architecture ruling are recorded in
`s20-530-v8-closeout-proof-frontier-2026-08-31.md`.

Do not request a helper-only v8 freeze review. Clean Nabu, Ariadne, and Vulcan
rulings selected the tightly defined hybrid recorded in
`s20-530-v8-hybrid-proof-path-amendment-design.md`. The final v8 change must
include the exact 14-site gate registry, positional scanner parity, complete
pre-exception entry and control inventories, exact exception ledgers,
independent reconciliation, and trusted local review receipts. The existing
helper producer prototype remains valid inside that settled design.

## Current evidence

`limit_runtime_helper_body_manifest` emits each record in this exact order:

1. `source`;
2. `owner`;
3. `function`;
4. `body_sha256`.

`require_implementation` accepts the generated record only when its exact
field order is:

1. `source`;
2. `owner`;
3. `function`;
4. `attribute_chain_sha256`;
5. `body_sha256`.

A bounded probe loaded the frozen checker, generated its exact 50-record
synthetic helper inventory, and reproduced the four-field versus five-field
contradiction with exit status zero. A full-source probe was cancelled after
more than nine CPU-bound minutes in the known `rust_code_mask` runtime debt;
it is non-authoritative and was superseded by the bounded successful probe.
The mismatch is independent of helper contents because it is fixed by the two
checker code paths.

The neighboring exact-schema pairs were inspected separately:

- mapped test bodies emit and require the same three fields;
- limit-event owner bodies emit and require the same five fields;
- control-ancestry records and ordered scopes emit and require their matching
  ten-field and nine-field shapes;
- limit-event observations emit and require the same four fields;
- runtime cases, dual-site proofs, entry paths, and shared-state manifests are
  compared directly to their checker-owned generated values and digests.

No second producer/validator field-shape contradiction was found in the final
implementation-manifest chain.

The specification already requires the complete attribute chain of each
governed owner and transitive local callable to be SHA-256 bound into its
review record. Adding the missing digest to runtime helper records implements
that existing rule rather than expanding it.

## Amendment A: complete the helper manifest

For each exact helper authority, retain the current source, owner, function,
and raw body lookup. Also isolate the function's complete attribute chain with
the existing `limit_named_function_attribute_chain` authority.

Fail closed when the chain cannot be isolated. Emit
`attribute_chain_sha256` immediately after `function` and before
`body_sha256`, matching the field order already required by
`require_implementation`.

The digest is SHA-256 over the exact raw attribute-chain bytes. An empty chain
therefore remains explicit and reviewable as the SHA-256 of empty bytes.

## Amendment B: add regression controls

Extend the checker self-contract to prove:

- the exact synthetic helper inventory remains 50 records;
- every generated record has the required five fields in the required order;
- every synthetic helper has a well-formed 64-character attribute-chain
  digest;
- adding one inert function attribute changes the affected helper record even
  when its function body remains byte-identical;
- the existing helper-body substitution control still changes the manifest;
- missing or ambiguous helper bodies and attribute chains continue to fail
  closed.

The self-contract must compare producer output to the same field vocabulary
used by `require_implementation`, so a later one-sided schema edit is rejected.

## Contract and evidence updates

The v8 refreeze must update every value transitively changed by the checker
amendment, including:

1. frozen checker raw and checker self-contract digests;
2. the contract-set digest;
3. a new immutable
   `s20-530-crash-recovery-contract-freeze-v8.json` evidence artifact;
4. matching machine-summary evidence path, contract-set, reviews, and later
   freeze-commit binding;
5. fresh Nabu, Ariadne, and Vulcan `PASS_CONTRACT_FREEZE` reviews bound to the
   settled v8 contract set and evidence payload.

The matrix and runner may remain unchanged. The specification and ADR must be
narrowly amended because their current absolute static-resolution language
does not authorize the settled exact manual-review proof path. No v7 review may
be reused. The v7 checker, evidence, reviews, and freeze anchor remain
immutable historical authority.

## Production boundary

v8 changes no Rust source, production projection, recovery module, public API,
wire format, error enum, storage layout, lock protocol, durability cut, limit,
failure precedence, fixture body, mapped test body, or matrix membership. The
change is confined to the structural checker, its hostile self-controls, new
freeze evidence, and machine-summary bindings.

## Rejected alternatives

- Do not remove `attribute_chain_sha256` from `require_implementation`.
- Do not insert a placeholder or body-derived value for the missing digest.
- Do not permit test-plan authors to supply a field the checker cannot derive.
- Do not mutate the immutable v7 evidence or reuse its reviews.
- Do not combine unrelated helper-body, dual-site, control-ancestry, entry-path,
  shared-state, or runtime-evidence implementation work into this refreeze.
- Do not run the monolithic checker while its known parser runtime debt remains
  the controlling development bottleneck.

## Invariants

- The 100 matrix row IDs and `MATRIX_ROWS_SHA256` remain unchanged.
- The 30 limit events and `LIMIT_EVENT_SPECS_SHA256` remain unchanged.
- The 37 limit runtime cases, 50 helper authorities, and five dual-site fields
  remain unchanged.
- All current helper raw-body digests remain unchanged.
- The complete 419-test map and 232 corruption fixtures remain unchanged.
- Contract refreeze and freeze-anchor checkpoint remain separate commits.
- Any post-review contract change invalidates all v8 freeze reviews.

## Prototype and refreeze sequence

1. Add the missing producer field and purpose-built self-controls to an
   unsettled checker candidate.
2. Run Python syntax, formatting, lint, bounded schema, and focused hostile
   controls.
3. Run the owning `sley-txn` and `sley-repo` Tier 2 crate suites only if the
   settled change reaches Rust-affecting validation or final contract review.
4. Recompute the checker self-contract, raw checker, and contract-set digests.
5. Obtain fresh Nabu, Ariadne, and Vulcan freeze reviews against the exact
   settled bytes and evidence payload.
6. Create the v8 evidence, update machine-summary bindings, rerun frozen
   self-integrity, and commit one coherent freeze change set.
7. Record the exact freeze commit in a follow-up checkpoint commit.

Standing Council invocation authority covered the completed design handoffs.
Fresh final-byte freeze and implementation reviews remain mandatory and must
be verified through the trusted local session-receipt path defined by the
settled hybrid design.

## Prototype validation

The exact Amendment A producer change and Amendment B controls were applied
temporarily to the v7 checker without changing v7 evidence. The bounded
prototype produced these results:

- Python compilation: `PASS`;
- Ruff lint: `PASS`;
- Ruff format: initial line-wrapping failure, corrected with `ruff format`,
  authoritative rerun `PASS`;
- `git diff --check`: `PASS`;
- helper-manifest shape: `PASS_50_RECORDS_5_FIELDS`;
- attribute-only mutation binding: `PASS`;
- raw helper-body stability across that attribute mutation: `PASS`.

The bounded dynamic probe completed in less than one second. It used the
checker-owned runtime-case registry, profile map, instrumentation transform,
helper owner rules, manifest producer, and body-key function. It therefore
tested the actual candidate producer against the same exact synthetic helper
inventory used by the checker self-contract.

The prototype checker edit was then removed so the committed repository keeps
the immutable v7 checker bytes until a reviewed v8 freeze can be created. The
monolithic checker and full release gate were not run for this design-only
prototype. Final v8 evidence must rerun the settled applicable checks.

## Acceptance

The v8 refreeze is acceptable only when:

- all 50 helper records have the exact required five-field order;
- each attribute-chain digest is derived from the exact helper declaration;
- an attribute-only mutation changes the manifest while the raw body digest
  remains stable;
- the existing body-substitution control remains effective;
- all neighboring final-manifest shapes remain unchanged and satisfiable;
- checker syntax, formatting, lint, bounded self-controls, and frozen
  self-integrity pass;
- fresh specialist reviews bind to the final v8 payload;
- the worktree is clean after the freeze and follow-up checkpoint commits.

The full `make v1` release gate remains deferred because this mechanical
refreeze is not a release boundary.
