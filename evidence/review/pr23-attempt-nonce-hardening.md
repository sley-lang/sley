# PR #23 attempt nonce hardening

Work package: the native supervisor attempt-entropy boundary in PR #23.
This adapts the existing supervisor request path; it introduces no Sley 1
implementation and changes no canonical kernel or admission contract.
The governing contract is `docs/spec/NATIVE_TEST_EXECUTION_V1.md` section 7:
the measured attestation binds a host-random 32-byte attempt nonce.

## Evidence and diagnosis

Inspected the Rust SARIF for analysis `1851194253`, head
`3351a01842dadbcaf41c4808255b78482c059f08`, obtained from
`GET /repos/sley-lang/sley/code-scanning/analyses/1851194253` with
`Accept: application/sarif+json` on 2026-10-03 UTC. The downloaded JSON's
SHA-256 is `e4eeabed9c0589140d71087bf00072158f8a295815ec513dd56557479bc6b900`.
It contains 22 `rust/hard-coded-cryptographic-value` results. GitHub has
20 associated inline review comments; those counts describe different things.

- Seventeen results are constant arguments in test-only supervisor request
  constructors. Two more are the deliberately zero and restored nonzero
  values in the staging rejection test.
- Two results start at the SCB1 varint accumulator and worker declared-limit
  array initializers. Their paths end at the byte-to-hex helper in staging.
  The decoder values are not entropy sources. The upstream CodeQL rule treats
  any function parameter named exactly `nonce` as a heuristic sink, including
  this formatting helper.
- The remaining result starts at the executor's zero-initialized array. Its
  path omits the successful `File::read_exact` overwrite and the zero/duplicate
  checks. Source review found no path that dispatches that initial value after
  a failed or incomplete read. This evidence does not establish a production
  hard-coded nonce vulnerability.

## Remediation and preserved behavior

The host boundary now obtains its bytes through pinned `getrandom` 0.4.3.
The helper returns only after `getrandom::fill` succeeds and provides no
fallback. The transaction executor still rejects zero and already-used bytes,
preserves the exact returned bytes, and refuses entropy failure before building
or dispatching a request. Its private injectable source supports deterministic
failure/reuse tests without changing the production source.

Operational supervisor request fixtures use the same OS entropy helper. Their
semantic program inputs and golden wire vectors remain fixed. The nonce
round-trip test compares with the generated input instead of a literal.
The all-zero rejection fixture remains deliberately all-zero.

The staging helper is named `hex_bytes(bytes)` to describe its actual role.
It still renders the same 64 lowercase hex characters, including leading zeros;
a test covers all 256 byte values. It does not create or validate entropy.
The SCB1 decoder and worker parser remain unchanged. There are no query
exclusions, alert dismissals, changes to security settings, or renamed nonce
parameters at actual supervisor request-construction boundaries.

The dependency lock and generated third-party license notices include
`getrandom` and its target-specific `r-efi` dependency. No dependencies already
in the lock were upgraded.

This is code maintenance only. It does not install a supervisor, qualify a
native host, enable admission, or establish a new support claim.

## Validation

Using Rust 1.93.0, four build jobs, incremental compilation disabled, and
Cargo dev/test debug information disabled:

- Affected-crate tests: runner 87 passed; transaction 257 passed, one ignored;
  protocol 130 passed, four ignored.
- `make conformance`, `make adversarial`, `make fuzz-smoke`, and `make lint`:
  passed. Strict workspace Clippy reported zero warnings; formatting passed.
- Native fixture regeneration check, independent native wire vectors and native
  dependency graph checks: passed.
- Generated third-party notices: exact rendering and locked-package checks passed.
- The first workspace run hit the AF1-X `expansion_time_stays_near_linear`
  one-second assertion at 1.122 seconds. Running the entire AF1-X target on
  the untouched PR head also failed at 1.096 seconds (46 tests passed, one
  failed); running that timing test alone passed at 0.959 seconds. This is a
  timing-sensitive baseline limit. Its source and threshold are unchanged.

The complete workspace run and fresh hosted CodeQL result must be checked on
PR #23. A previous-head scan or successful analysis job alone does not prove
that the aggregate security check passes.
