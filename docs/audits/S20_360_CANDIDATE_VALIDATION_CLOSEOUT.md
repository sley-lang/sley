# S20-360 Candidate Validation Closeout

Status: **PASS - restricted executable-program-operation-free validation complete; no commit or runtime authority**

## Scope

This closeout covers the pure ordered candidate validator, closed trusted
context, complete epoch-1 entity/reference projection, canonical result codec,
independent result oracle and corpus, and candidate-result persistent fuzz
target. The original success subset was explicitly
executable-program-operation-free; the full operation analysis addendum below
(2026-09-03) replaced that limit with the S20-260/S20-270 E1 through E6
judgment.

This slice does not cover complete operation semantics, mandatory
production-epoch semantic fingerprints, accepted-state writes, capability
budget consumption, atomic commit, receipts, repository refs, CAS, runtime
effects, protocol, benchmark trials, packaging, release, or publication.

## Implemented boundary

- `CandidateValidationContext` derives its own public digest from the exact
  accepted transaction/root/epoch, policy, principal, rebuilt capability
  summary, trusted time, inventory digest, tombstone digest, and effective
  ceilings. Callers cannot supply phase outcomes, roots, diagnostics, selected
  tests, or a context digest.
- `validate_candidate_bytes` runs all fourteen frozen phases in order. It
  strictly imports candidate bytes, checks closed context and freshness,
  rechecks creation identities, applies operations only to an in-memory clone,
  derives all-18-kind references and affected closure, invokes the owning
  type/CFG/effect/contract checkers, verifies policy/capability inputs without
  ledger mutation, finalizes mandatory tests, charges supported work, rebuilds
  the candidate root in memory, and renders validator-owned phase evidence.
- Present TypeDef and Function fingerprint claims are recomputed after their
  owning checker passes. Unsupported-kind claims fail closed. Absent supported
  claims remain a documented restricted-epoch allowance, not GA completion.
- Invalid candidates have one failed phase and a not-run suffix. `VALID` has
  fourteen passed phases and a candidate root. No path writes the object store,
  repository, ref graph, accepted root, policy root, or capability ledger.

## Conformance and adversarial evidence

- The codec-owned fixture generator emits sixteen accepted result vectors:
  `VALID` plus every terminal decision. `--check` proves the committed bytes
  still match the current Rust codec.
- The independent Python oracle strictly decodes the result envelope and
  thirteen-field record, then independently checks decision tags, failed
  phases, phase count, result IDs, monotonic phase shape, diagnostics, sets,
  and root/candidate presence rules.
- Four rejected mutations cover bad magic, digest corruption, trailing bytes,
  and truncation. Native result tests cover deeper phase, decision,
  diagnostic, set, root, and identity shape failures.
- The production result-import libFuzzer target starts from 25 unique seeds,
  reimports accepted bytes twice, rederives the result ID, checks trailer and
  envelope lengths, and asserts the fourteen-phase monotonic shape. The
  bounded 512-run smoke passed without a crash artifact.
- Eleven pipeline tests cover deterministic valid output, input and base-state
  immutability, invalid context, stale roots/preimages, identity/tombstones,
  graph/reference distinctions, exact semantic owner codes, fingerprint
  mismatch, policy/capability/authenticator checks, mandatory contracts/tests,
  expiry, isolation, ceilings, and unsupported operation analysis.

## Independent reviews

Ariadne reviewed all fourteen phases and found no false `VALID` path or
authority leak. Ariadne found one P1 contract mismatch: unsupported operation
analysis returned `INTERNAL_ERROR` while the frozen restricted contract
required `RESOURCE_LIMIT`. The implementation and focused assertion were
corrected, and Ariadne's targeted recheck resolved the finding.

Vulcan rechecked the earlier P3 evidence-breadth gap after corpus expansion.
The review confirmed independent coverage of all sixteen decisions and failed
phases, strict framing corruption coverage, fixture-to-code drift checking, and
no S20-700 or GA overstatement. Vulcan reported no remaining report-grade
P0-P4 finding in the bounded closeout scope.

## Validation

Validation tier: **Tier 2 subsystem handoff**

Affected subsystems: `sley-policy`, SSMC tag decoding used by the candidate
projection, mutation codecs, SCB1 independent oracle, result conformance
fixtures, persistent result fuzzing, and S20 frontier/evidence documents.

Passed checks:

```text
cargo test -p sley-policy candidate_validation -- --nocapture
cargo test -p sley-policy candidate_result -- --nocapture
cargo clippy -p sley-policy --all-targets -- -D warnings
cargo +nightly-2026-02-27 clippy --manifest-path fuzz/Cargo.toml --bin candidate_result -- -D warnings
python3 scripts/generate_candidate_result_fixtures.py --check
uv run --project oracle/scb1 --frozen python -m unittest discover -s oracle/scb1/tests -v
uv run --project oracle/scb1 --frozen sley2-scb1-oracle check-candidate-result --accepted conformance/candidate-result/v1/accepted.json --rejected conformance/candidate-result/v1/rejected.json
make candidate-result-persistent-fuzz-smoke
make quick
make core
make conformance
cargo clippy --workspace --all-targets --locked -- -D warnings
```

The first Tier 1 attempt correctly detected stale S20-700 count assertions and
regenerated secret-scan evidence after the expanded tracked surface. Those
drifts were repaired and the complete `make quick` rerun passed.

The Tier 3 `make v2` and `make release-check` gates were not run. They remain
intentional fail-closed release boundaries for unfinished Sley 2 packages and
are not required for this subsystem development slice.

## Boundary result

Restricted S20-360 validation is complete and independently reviewed. A
`CandidateValidationOutput`, imported `CandidateResult`, or `VALID` decision is
evidence only. S20-390 is the first package allowed to recheck that evidence
and attempt an atomic durable commit with a receipt. The full Sley 2 goal,
complete S20-700, M3-M6 exits, succession proof, and GA remain incomplete.

## Full operation analysis addendum (2026-09-03)

The S20-260/S20-270 extended opcode profile supplied the judgment this slice
waited for, so phase 7 now calls `sley_vm::judge_function_operations` (contract
revision 8, judgment only: no bytecode, no cache key, no execution) once per
function unit after the S20-220 graph report. The phase evidence gained the
judged-operation count and the judgment work, and phase 12 adds the operation
count to the graph work total, so the recorded result identity of the valid
fixture changed with the evidence.

Failure mapping keeps every owner's code: a signature or immediate mismatch is
a phase 7 `CONTROL_FLOW_ERROR` carrying `VM_LOWER_SIGNATURE_MISMATCH` or
`VM_LOWER_IMMEDIATE_MISMATCH`, a lowering resource ceiling is a phase 7
`RESOURCE_LIMIT`, and anything else is a phase 7 `INTERNAL_ERROR`.

The five E7 opcodes stay excluded from phase 7 analysis, and only test
observation 145 is refused unconditionally (phase 11
`TEST_PLAN_OBSERVATION_UNSUPPORTED`, whatever shape it takes). The other four
owners validate shapes and accept well-formed instances — 144 at phase 10,
160, 161, and 162 at phase 8 — so the phase 12 guard is the live refusal path
for well-formed E7 programs, not defense in depth. The malformed-operation
matrix test records owner refusal of ill-formed instances only; well-formed
instances are proven by `well_formed_contract_assert_reaches_the_phase_twelve_guard`
(144 refused at phase 12), `well_formed_effect_operations_pass_the_phase_eight_owner`
(160 and 161 pass phase 8; 162 clears phases 8 and 9 and is refused at phase
12), and the shaped-145 case inside the matrix test.

Still absent: mandatory production-epoch semantic fingerprints, accepted-state
writes, capability budget consumption, atomic commit, receipts, refs, CAS,
runtime effects, and every publication gate. The package's Council reviews of
the extended profile and of this addendum landed 2026-09-04 with three
freeze-blocking findings, closed by the revision recorded in
`docs/spec/CANDIDATE_RESULT_V1.md` section 9; lower-severity findings remain
open and are tracked in the finding register.
