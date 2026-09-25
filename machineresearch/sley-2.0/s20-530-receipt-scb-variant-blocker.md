# S20-530 receipt SCB variant blocker

Status: deferred carrier-and-variant blocker, expanded to every visible role at
`dacdc7ac513bc123cc8e199405d02d7c0a6f5918`.

## Scope

This note records why nine normalized corruption cases, instantiated as 27
frozen COR-06 and COR-07 leaves, cannot produce the required top-level
`TransactionCodecError::Scb` variant from the current transaction receipt wire
format. It does not reopen or modify the v4 checker, and it does not authorize
a production wire-format change.

Each normalized case is instantiated for the accepted revision, branch-head
revision, and branch-origin revision. The affected selectors are:

- `receipt_scb_contract_unknown`
- `receipt_scb_epoch_mismatch`
- `receipt_scb_bool_invalid`
- `receipt_scb_utf8_invalid`
- `receipt_scb_label_not_nfc`
- `receipt_scb_float_non_canonical`
- `receipt_scb_field_order`
- `receipt_scb_map_order`
- `receipt_scb_map_duplicate`

Every frozen leaf requires its role-specific top-level SCB variant:
`commit.codec.scb` for COR-06 or
`branch.transaction.commit.codec.scb` for COR-07. The frozen fixtures classify
every corruption as `receipt_envelope` and preserve the corresponding direct
source chain.

## Current receipt carrier

The outer receipt is an SCB envelope containing magic, envelope version,
payload length, one nine-field receipt record, and the trailing `ReceiptId`.
The receipt record fields are:

1. format version
2. transaction ID
3. stored transaction bytes
4. optional stored candidate bytes
5. optional stored candidate-result bytes
6. stored state-root bytes
7. stored policy-root bytes
8. object-manifest list
9. durability profile

There is no outer receipt contract identifier or schema epoch. The outer
receipt record also has no boolean, UTF-8 string, label, or floating-point
field, and it contains no map value. Its record fields are decoded before the
nested byte fields are imported.

Twelve compatible outer receipt cases are implemented and pass for both
COR-07 branch roles at the status commit: magic invalid, version unsupported,
digest mismatch, trailing bytes, varint non-minimal, integer overflow, length
overflow, field missing, field unknown, field duplicate, union invalid, and
resource limit. Their accepted-role COR-06 instances remain behind the
separate helper-arity collision.

## Carrier and variant mismatch

The remaining error codes require carriers outside the outer receipt shape:

- `SCB_CONTRACT_UNKNOWN` and `SCB_EPOCH_MISMATCH` require a nested state-root
  or policy-root carrier. Those imports preserve
  `TransactionCodecError::StateRoot` or `TransactionCodecError::PolicyRoot`,
  rather than converting the failure to `TransactionCodecError::Scb`.
- `SCB_BOOL_INVALID`, `SCB_UTF8_INVALID`, and `SCB_FLOAT_NON_CANONICAL`
  require a nested candidate or candidate-result carrier. Those imports
  preserve `TransactionCodecError::Candidate` or
  `TransactionCodecError::CandidateResult`.
- `SCB_FIELD_ORDER`, `SCB_MAP_ORDER`, and `SCB_MAP_DUPLICATE` require nested
  structured state, policy, candidate, or candidate-result data. They likewise
  preserve the owning nested error variant.
- `SCB_LABEL_NOT_NFC` has no receipt-record carrier and no reachable nested
  receipt route that yields the required top-level SCB variant.

Resealing both the nested artifact and outer `ReceiptId` can make a nested
failure observable, but it cannot change the owning `TransactionCodecError`
variant. Reclassifying nested failures as top-level SCB errors would discard
the current error ownership contract and conflict with the existing codec
implementation.

## Decision

These 27 leaves stay deferred. Implementing them honestly requires a
separately authorized S20-530 refreeze that changes the expected variant and
fixture carrier, removes an unreachable leaf, or deliberately revises the
production receipt format and error-ownership contract. Development continues
on v4-compatible leaves without mutating production wire semantics to satisfy
test-only expectations.

## Frozen v4 identity

- Contract set SHA-256: `4c6b0e12836f6601df4cfc67e36fdf754b7bd1a60c7df2ca21c2b6e037ca93fa`
- Evidence payload SHA-256: `cf8491ff4928679554b138f2b2391f3bb4f67699c8aec366e6843e05e9306467`
- Checker raw SHA-256: `44b7d77bd010ae0d82bc5c32de3870133373776913d92418b5af7590cf9e377d`
- Checker self SHA-256: `4941f73a60f1e80807d517cd49ffe53109edc845e24c6933ebdd58ad9e010752`
