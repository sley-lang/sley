# S20-260/S20-270 VM Extended Opcode Campaign (2026-09-03)

Status: contract draft revision 16 (2026-09-17), with slices E1–E6, E7a
and E8 implemented. Revision 16 is design-only (E7 handle-model boundary,
no execution change). Historical Nabu residual corrections passed independent
item-level review (`evidence/review/vm-nabu-correction-review-2026-09-15.md`);
prior verdicts and dated validation below remain preserved.
Current conformance vectors: 31.
Current endpoint status: SMP1 execute profile selector is implemented;
field 6 selects EXTENDED_V1, and omission selects RESTRICTED_V1.

Slices
E1 through E6 landed under revisions 2 through 7 in order; revision 8 added
the judgment-only entry, revision 9 landed slice E7a (`contract_assert`,
opcode 144), revision 11 made the family fuzz lanes reach execution, and
revision 12 answers the Ariadne and Vulcan freeze findings. The rest of E7
is excluded until its owners exist.

## Frontier at start

- The restricted profiles lower and execute three Boolean opcodes and all
  five terminators with frozen vectors, cache keys, and observation
  digests; every other opcode fails closed with `VM_LOWER_OPCODE_UNSUPPORTED`.
- The epoch-1 manifest fixes every opcode's operand count, immediate kind,
  result shape, and failure value; S20-210 fixes the closed builtin failure
  codes; S20-250 fixes the value hash.
- SMP1 `execute` reaches the VM from the endpoint, so each family becomes
  demonstrable end to end.

## Design brief

Contract: `docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md`, ADR-0039, stage
checker `scripts/check_vm_extended_opcode_profile.py`.

- A second cache profile (`EXTENDED_V1`, bytecode `SLEYBC02` with explicit
  immediates); the restricted bytes and identities are untouched.
- Family slices: E1 data, E2 checked integers, E3 floats, E4 aggregates and
  maps, E5 cells and references, E6 direct calls; E7 excluded.
- No new numeric codes; the seven lowering and six execution codes cover
  every failure; builtin failure values use the S20-210 closed codes.

## Open questions for the reviews

- Whether `vector_new` with zero operands should take its element type
  from the declared result or from an immediate. Answered: the declared
  result names `T` (contract E1).
- Whether float order predicates should follow IEEE unordered semantics or
  a total order. Answered: IEEE unordered semantics (contract E3).
- Whether the call-depth ceiling of 256 belongs in the request limits.
  Answered: the ceiling is fixed at 256 live frames and ignores the
  manifest call-depth field; honoring a caller depth needs a new lowering
  profile (contract E6, revision 12).
- Whether map order by S20-350 canonical key bytes is the right frozen
  rule for runtime maps. Answered: the encoding order is frozen and named
  as such, with byte key identity (contract E4, revision 12).

## Records

| Stage | Commit | Tier 1 | Notes |
|---|---|---|---|
| Contract draft revision 1 | `d1217ad` | green | ADR-0039, stage checker |
| Slice E1 (data), revision 2 | `f1a5e6f` | green | `crates/sley-vm/src/extended.rs`; 4 tests (positive, constants and comparisons, 11-case rejection matrix, bytecode and 128 repeats); 5 vectors; fuzz profile-toggle lane |
| Slice E2 (checked integers), revision 3 | `2ff19c0` | green | `checked_integer` in `extended.rs`; 24 exact outcomes over widths 8 and 128, 4 rejections; 3 vectors |
| Slice E3 (floats), revision 4 | `785b92d` | green | `float_operation` and IEEE order in `extended.rs`; NaN canonicalization, fma single rounding, subnormals, 8 comparisons, 4 rejections, 128 repeats; 3 vectors |
| Slice E4 (records, variants, maps), revision 5 | `4f1e197` | green | definition-bound immediates and canonical map order in `extended.rs`; 1 test with 7 rejections; 3 vectors |
| Slice E5 (cells, hashing, globals, references), revision 6 | `4491b1c` | green | register-only cell handles, escape guard, S20-250 hashing, inventory-bound globals and references in `extended.rs`; 1 test with 9 rejections; 3 vectors |
| Slice E6 (direct calls), revision 7 | `04ef631` | green | callee closure lowering and `SLEYBC02` callee table in `lower.rs`, per-frame execution with shared budgets and the 256-frame ceiling in `execute.rs`; 1 test with 4 rejections; 2 vectors |
| Judgment entry, revision 8 | `9fb9ecc` | green | `judge_function_operations` judges every extended operation without lowering; E7 opcodes refused by their own phases |
| Slice E7a (contract assertions), revision 9 | contract revision 9 | green | `contract_assert` judgment plus predicate-frame execution in `extended.rs`/`execute.rs`; 2 vectors (`contract-assert-holds`, `contract-assert-violated`) |
| Fuzz lanes reach execution, revision 11 | contract revision 11 | green | per-fixture requests, completion assertion, pinned refusal code, position-stable seed selection |
| Freeze findings, revision 12 | contract revision 12 | green | `lowerer_version [2, 0, 0]` with the bump rule and full `SLEYBC02` layout in section 1; stated post-construction invariant and liveness bound in section 2; exact equality rows, named negative-zero deviation, pinned float environment, encoding-order decision with byte key identity, absent-key and arity gaps, failure-name mapping, five-fuel derivation in section 3; depth comparison fix (`>`), cell count fix (`>=`), byte-identity dedup and probes in the crate; 2 new pinning tests plus the `call-direct-depth-ceiling` vector (22 vectors); stage checker pins for every finding |

## Slice E1 Tier 2 handoff record (2026-09-03, at `f1a5e6f`)

| Gate | Result | Wall time | Evidence |
|---|---|---:|---|
| `make core` | exit 0 | 12 s | 989 tests passed, 0 failed across 39 test binaries |
| `make conformance` | exit 0 | 12 s | 19 oracle results PASS |
| `make adversarial` | exit 0 | 9 s | 597 tests passed, 0 failed |
| `make fuzz-smoke` | exit 0 | under 1 s | 5 bounded smoke tests passed |
| `make vm-persistent-fuzz-smoke` | exit 0 | 12 s | 626 runs with the profile-toggle lane, PASS; the checker re-anchored at `59162e7` after a stale marker had kept it red since 2026-08-27 |
| `make release-candidate-smoke` | exit 0 | 24 s | artifact REPRODUCIBLE, demo PASS |
| `make sley2-runner-smoke` | exit 0 | 1 s | evidence PASS |
| `make accounting-smoke` | exit 0 | 1 s | evidence PASS |

Logs were captured under the session scratchpad; `make v1` was skipped because this is a subsystem handoff, not a release boundary.

## Slice E2 Tier 2 handoff record (2026-09-03, at `2ff19c0`)

| Gate | Result | Wall time | Evidence |
|---|---|---:|---|
| `make core` | exit 0 | 13 s | 990 tests passed, 0 failed |
| `make conformance` | exit 0 | 10 s | 19 oracle results PASS |
| `make adversarial` | exit 0 | 9 s | 597 tests passed, 0 failed |
| `make fuzz-smoke` | exit 0 | under 1 s | 5 bounded smoke tests passed |
| `make vm-persistent-fuzz-smoke` | exit 0 | 4 s | 626 runs, PASS |

Logs were captured under the session scratchpad; `make v1` was skipped because this is a subsystem handoff, not a release boundary.

## Slice E3 Tier 2 handoff record (2026-09-03, at `785b92d`)

| Gate | Result | Wall time | Evidence |
|---|---|---:|---|
| `make core` | exit 0 | 13 s | 991 tests passed, 0 failed |
| `make conformance` | exit 0 | 10 s | 19 oracle results PASS |
| `make adversarial` | exit 0 | 9 s | 597 tests passed, 0 failed |
| `make fuzz-smoke` | exit 0 | 1 s | 5 bounded smoke tests passed |
| `make vm-persistent-fuzz-smoke` | exit 0 | 4 s | 626 runs, PASS |

Logs were captured under the session scratchpad; `make v1` was skipped because this is a subsystem handoff, not a release boundary.

## Slice E4 Tier 2 handoff record (2026-09-03, at `4f1e197`)

| Gate | Result | Wall time | Evidence |
|---|---|---:|---|
| `make core` | exit 0 | 14 s | 992 tests passed, 0 failed |
| `make conformance` | exit 0 | 12 s | 19 oracle results PASS |
| `make adversarial` | exit 0 | 9 s | 597 tests passed, 0 failed |
| `make fuzz-smoke` | exit 0 | under 1 s | 5 bounded smoke tests passed |
| `make vm-persistent-fuzz-smoke` | exit 0 | 4 s | 626 runs, PASS |

The slice added the `sley-vm` to `sley-mutate` dependency edge for canonical map key bytes; the supply-chain inventory now records 118 dependency relationships and the refreshed `Cargo.lock` digest, pinned in `scripts/check_supply_chain_audit.py` and the machine summary. `make v1` was skipped because this is a subsystem handoff, not a release boundary.

## Slice E5 Tier 2 handoff record (2026-09-03, at `4491b1c`)

| Gate | Result | Wall time | Evidence |
|---|---|---:|---|
| `make core` | exit 0 | 14 s | 993 tests passed, 0 failed |
| `make conformance` | exit 0 | 10 s | 19 oracle results PASS |
| `make adversarial` | exit 0 | 10 s | 597 tests passed, 0 failed |
| `make fuzz-smoke` | exit 0 | under 1 s | 5 bounded smoke tests passed |
| `make vm-persistent-fuzz-smoke` | exit 0 | 4 s | 626 runs, PASS |

`make v1` was skipped because this is a subsystem handoff, not a release boundary.

## Slice E6 Tier 2 handoff record (2026-09-03, at `04ef631`)

| Gate | Result | Wall time | Evidence |
|---|---|---:|---|
| `make core` | exit 0 | 13 s | 994 tests passed, 0 failed |
| `make conformance` | exit 0 | 11 s | 19 oracle results PASS |
| `make adversarial` | exit 0 | 8 s | 597 tests passed, 0 failed |
| `make fuzz-smoke` | exit 0 | 1 s | 5 bounded smoke tests passed |
| `make vm-persistent-fuzz-smoke` | exit 0 | 4 s | 626 runs, PASS |

`make v1` was skipped because this is a subsystem handoff, not a release boundary.

## Revision 12 Tier 2 handoff record (2026-09-05, at `a6868b8`)

| Gate | Result | Evidence |
|---|---|---|
| `make quick` | exit 0 | Tier 0 green |
| `make lint` | exit 0 | clippy clean workspace-wide |
| `make core` | exit 0 | 1047 tests passed, 0 failed |
| `make conformance` | exit 0 | all oracles PASS, including `check-vm-extended` over 22 accepted + 5 rejected vectors with the independently pinned `[2, 0, 0]` lowerer |
| `make adversarial` | exit 0 | 615 tests passed |
| `make fuzz-smoke` | exit 0 | bounded smoke tests passed |
| `make release-candidate-smoke` | exit 0 | clean at `b92357e`, PASS |

`make v1` was skipped because this is a subsystem handoff, not a release boundary.

## Program status at revision 12 (2026-09-05)

Every family slice E1 through E6 plus E7a is implemented under contract
revision 12 (the rest of E7 stays excluded until its owners exist). The
machine summary records
`vm_extended_opcode_profile.status = S20_260_270_EXTENDED_IMPLEMENTED_REVIEW_PENDING`
with twenty-two conformance vectors in `conformance/vm-extended/v1/accepted.json`
and thirty-nine `sley-vm` unit tests. Nabu architecture review is PASS;
the Ariadne contract review and Vulcan surface review re-reviews are queued
in the session review loop with their request texts refreshed to revision
12; their findings land as contract revisions before the freeze that turns
the status to `S20_260_270_EXTENDED_COMPLETE`.

Historical next-step paragraph (superseded by the implemented selector;
current status is recorded above): the next authority-safe package was the
SMP1 appendix C revision 8 execute
profile selector (`S20-410-EXECUTE-PROFILE-SELECTOR`): an `execute` limits
field naming the cache profile so the endpoint, the JSON bridge, the CLI, and
the S20-620 runner can execute under `EXTENDED_V1`. The former
restricted-only endpoint statement no longer describes the current code.
