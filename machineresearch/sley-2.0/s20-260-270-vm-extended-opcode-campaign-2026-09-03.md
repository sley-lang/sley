# S20-260/S20-270 VM Extended Opcode Campaign (2026-09-03)

Status: contract draft revision 1 written by the integrator with every
Council lane unavailable; Ariadne contract review, Nabu architecture
review, and Vulcan surface review queued. Slices E1 through E6 land under
it in order; E7 is excluded until its owners exist.

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
  from the declared result or from an immediate.
- Whether float order predicates should follow IEEE unordered semantics or
  a total order.
- Whether the call-depth ceiling of 256 belongs in the request limits.
- Whether map order by S20-350 canonical key bytes is the right frozen
  rule for runtime maps.

## Records

| Stage | Commit | Tier 1 | Notes |
|---|---|---|---|
| Contract draft revision 1 | `d1217ad` | green | ADR-0039, stage checker |
| Slice E1 (data), revision 2 | `f1a5e6f` | green | `crates/sley-vm/src/extended.rs`; 4 tests (positive, constants and comparisons, 11-case rejection matrix, bytecode and 128 repeats); 5 vectors; fuzz profile-toggle lane |
| Slice E2 (checked integers), revision 3 | `2ff19c0` | green | `checked_integer` in `extended.rs`; 24 exact outcomes over widths 8 and 128, 4 rejections; 3 vectors |
| Slice E3 (floats), revision 4 | `785b92d` | green | `float_operation` and IEEE order in `extended.rs`; NaN canonicalization, fma single rounding, subnormals, 8 comparisons, 4 rejections, 128 repeats; 3 vectors |
| Slice E4 (records, variants, maps), revision 5 | `4f1e197` | green | definition-bound immediates and canonical map order in `extended.rs`; 1 test with 7 rejections; 3 vectors |
| Slice E5 (cells, hashing, globals, references), revision 6 | `4491b1c` | green | register-only cell handles, escape guard, S20-250 hashing, inventory-bound globals and references in `extended.rs`; 1 test with 9 rejections; 3 vectors |
| Slice E6 (direct calls), revision 7 | pending | pending | callee closure lowering and `SLEYBC02` callee table in `lower.rs`, per-frame execution with shared budgets and the 256-frame ceiling in `execute.rs`; 1 test with 4 rejections; 2 vectors |

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
