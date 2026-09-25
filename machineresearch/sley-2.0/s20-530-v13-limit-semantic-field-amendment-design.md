# S20-530 v13 LIMIT semantic-field amendment

Status: candidate contract amendment

## Problem

The first full checker run with `implementation_complete` true (acceptance
commit `f677b1a`, after the authoritative v12 closeout at `7277af6` and three
`PASS_IMPLEMENTATION` receipts) failed at
`LIMIT-01/object_fanout_directories semantic field frozen_default assertion
differs: semantic field is not the exact compared value or code projection`.

For every LIMIT row subcase the implementation gate checks the
`frozen_default` assertion twice. The dedicated
`require_exact_limit_default_assertion` demands
`assert_eq!(<frozen constant>, <grouped literal>)` and passes. The generic
`require_semantic_assertions`, which runs first over the row's semantic
fields, treats every field not listed in `custom_fields` as a plain value
field and demands that the assertion compare the *field name*
(`frozen_default`) or its `.code()`/`.symbol()` projection to the value. A
LIMIT test compares the frozen constant, never a variable named
`frozen_default`, so the generic check can never pass on this field. The v11
grouped-literal acceptance repaired the first rejection in that function
(`expected value is not an exact equality operand`); the second rejection sits
one statement later and only a full run with real evidence could reach it. The
record-and-continue smoke used before v11 stopped at the first problem per
call and therefore reported only the literal category.

## Decision

V13 changes one thing and nothing else: the LIMIT call site of
`require_semantic_assertions` lists `frozen_default` in `custom_fields`, next
to `no_mutation`, `exact_limit_success`, `limit_plus_one_code`, and
`no_partial_report`, all of which already have dedicated exact checks. The
dedicated `require_exact_limit_default_assertion` remains the sole authority
for the frozen-default assertion, exactly as for the other four fields. The
v11 grouped-literal acceptance in `exact_value_assertion_problem` stays: it is
a correct generalization with positive and hostile self-test controls.

## Rejected alternatives

- Rewriting the 22 LIMIT tests to bind a local named `frozen_default`: a
  source change that rebinds the partition, and it would duplicate what the
  dedicated check already proves.
- Teaching the generic check about frozen constants: duplicates the dedicated
  check's knowledge in a second place.

## Re-freeze scope

Checker only (one line plus the freeze identity, the spec/ADR v13 markers, and
the ADR marker list), spec and ADR v13 paragraphs, contract set, freeze
evidence, and test-plan binding. Sources, runner, reconciler, scanner parity,
and the exception partition (`867bd1d9…`) are unchanged. The v12 closeout
evidence and implementation receipts (bound to contract set `939c2aea…` and
validated commit `7277af6`) cannot satisfy v13 and become a NON-AUTHORITATIVE
record; a fresh captured closeout and fresh implementation receipts are
required after fresh freeze receipts.
