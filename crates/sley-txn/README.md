# sley-txn

`sley-txn` owns Sley 2 transaction-core and complete-receipt bytes, fresh
commit-time validation orchestration, immutable-object installation, and the
fixed durable accepted-head compare-and-swap primitive.

It also exposes `TransactionRepository::verified_revision(TransactionId)` and
`VerifiedRevision` for read-only verification of arbitrary durable transaction
receipts and their live object closure without treating the result as an
accepted-head claim.

Transaction operations hold shared repository-maintenance ownership before
the accepted-head lock. `sley-repo` GC holds that same maintenance boundary
exclusively, preventing object deletion from interleaving transaction or ref
verification and visibility.

For candidate-affected native tests, `build_candidate_supervisor_request`
constructs the selected test's bounded supervisor request from the validator's
proposed root and complete object inventory. It checks the native plan against
that candidate and derives the worker input hashes from the Sley `TestCase`.
This prepares transport only; the qualified measured executor is still absent.

It does not own named refs, branches, merge, protocol, source, Git, providers,
or policy transitions. The currently implemented profile is restricted to
executable programs containing no semantic operation entities and to an empty
selected-test set. Candidate mutation operations remain supported within that
explicit boundary, as specified by `docs/spec/TRANSACTION_MODEL_V1.md`.
