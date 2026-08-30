# S20-530 candidate descriptor and payload reachability blocker

Status: deferred decoder-before-semantic blocker at
`f8841071dda29b26f373a3840bc0c5eebe732a95`.

## Scope

This note records why the final two frozen COR-07
`receipt_nested_candidate` leaves cannot produce their required semantic
candidate codes from imported candidate bytes under the current mutation
codec. It does not reopen or modify the S20-530 v4 checker, and it does not
authorize a production codec change.

The affected leaves are:

- `target_candidate_mutation_candidate_descriptor_unknown`
- `target_candidate_mutation_candidate_payload_kind`

The frozen leaves require `MUTATION_CANDIDATE_DESCRIPTOR_UNKNOWN` and
`MUTATION_CANDIDATE_PAYLOAD_KIND`, respectively. Both require the result
variant `branch.transaction.commit.codec.candidate` and the direct source
chain `CommitError`, `TransactionCodecError`.

## Current decode and validation order

Candidate import verifies the envelope and digest, decodes the candidate
record, and then validates the decoded record. The public candidate builder
also validates the record before encoding it.

Within candidate-record decoding, each mutation operation is decoded in field
order. Operation fields 2, 3, and 5 establish the mutation class, target kind,
and optional field tag before field 6 decodes the mutation payload.

`decode_mutation_payload` then resolves the operation descriptor before it
returns a `MutationPayload`:

```rust
let descriptor = mutation_operation_descriptor(class, target_kind, descriptor_field)
    .ok_or_else(scb_invalid)?;
```

An unknown descriptor therefore becomes `CandidateError::Scb` with
`SCB_UNION_INVALID` during decoding. The later
`CandidateRecord::validate` call cannot reach its
`CandidateError::DescriptorUnknown` branch for those bytes.

The same decoder validates payload ownership and descriptor shape before it
returns. For create and replace operations it explicitly calls
`matches_descriptor` and converts a mismatch to `scb_invalid()`. The other
operation classes decode through their descriptor-selected payload types and
likewise reject an incompatible union or value as an SCB error. Any decoded
`MutationPayload` that reaches `CandidateRecord::validate` has already passed
the payload compatibility condition that would otherwise emit
`CandidateError::PayloadKindMismatch`.

## Reachability conflict

There is no honest stored-byte corruption that satisfies all three current
layers:

1. strict operation and payload decoding succeeds;
2. candidate semantic validation observes an unknown descriptor or mismatched
   payload;
3. import returns the frozen mutation-candidate code instead of
   `SCB_UNION_INVALID`.

Resealing the candidate and receipt digests does not alter this ordering. A
test-only alternate builder also cannot solve it without bypassing the exact
production import path required by the frozen fixture plan.

## Decision

These two leaves stay deferred. Resolving them requires a separately
authorized production decision to make candidate decoding less semantic and
leave descriptor or payload ownership to `CandidateRecord::validate`, to map
the decoder failures to the semantic candidate codes, or to refreeze the
S20-530 leaves with the reachable SCB result.

Development continues on v4-compatible leaves without weakening strict
candidate decoding or changing production error ownership for test-only
reachability.

## Frozen v4 identity

- Contract set SHA-256: `4c6b0e12836f6601df4cfc67e36fdf754b7bd1a60c7df2ca21c2b6e037ca93fa`
- Evidence payload SHA-256: `cf8491ff4928679554b138f2b2391f3bb4f67699c8aec366e6843e05e9306467`
- Checker raw SHA-256: `44b7d77bd010ae0d82bc5c32de3870133373776913d92418b5af7590cf9e377d`
- Checker self SHA-256: `4941f73a60f1e80807d517cd49ffe53109edc845e24c6933ebdd58ad9e010752`
