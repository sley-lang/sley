# Native Ed25519 closure — 2026-09-17

Commit `71600c23e7950f001f438e2245c3c372542e599e` closes the native
signature-mechanics gap recorded by the N0–N8 completion ledger.

The implementation pins `ed25519-dalek` 3.0.0 and uses its strict verifier.
Both native-commit admission and native-exchange import reconstruct the
canonical signature preimage from parsed facts, resolve the receiver trust
grant, and verify the signature before accepting or promoting native history.
Strict verification rejects noncanonical signatures and small-order public or
R points. The transaction owner exposes a zeroizing acceptance signer over an
exact 32-byte secret seed. The supervisor owner exposes a zeroizing measurement
signer that loads an exact 32-byte root-provisioned key file.

Validation on the committed tree:

- `cargo test -p sley-test-runner -p sley-txn -p sley-repo -p sley-protocol --locked`: PASS.
- `cargo clippy --no-deps -p sley-test-runner -p sley-txn -p sley-repo -p sley-protocol --all-targets --locked -- -D warnings`: PASS.
- Acceptance-message, acceptance-signature, measurement-signature and weak-key
  mutations refuse with `HISTORICAL_TRUST_REJECTED`.
- `python3 scripts/build_anti_goal_conformance.py --check`: PASS (9 HOLDS,
  16 REVIEW_ONLY).

This closes cryptographic signing and verification only. The privileged
supervisor installation and positive measured worker execution remain separate
qualification work.
