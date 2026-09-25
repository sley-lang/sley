# S20-730 independent oracle campaign (2026-09-03)

Package: closing the two native-only fixture families of the S20-730
independent conformance report, dependency the landed extended VM profile and
the S20-720 demo fixture, phase M6. Owner of record Vulcan; executed by the
integrator with every Council lane unavailable (ADR-0026).

## What was missing

The coverage report read `INDEPENDENT_CONFORMANCE_PARTIAL`: seventeen of
nineteen fixture families were checked by the independent Python oracle, while
the extended VM vectors were emitted and checked only by the Rust VM, and the
release demo only through the packaged binary. Both were recorded blockers for
the master goal's final independent PASS (section 6.5 requires two
independently implemented codecs before GA).

## Mechanics

| Surface | File | What it verifies independently |
|---|---|---|
| Extended bytecode | `oracle/scb1/src/sley2_scb1_oracle/vm_extended.py` | decodes the `SLEYBC02` container (registers, type expressions, immediates, terminators, callee table) from the contract grammar, checks each artifact's SHA-256, and re-derives every vector's `BytecodeCacheKey` from the 224-byte frozen preimage; also checks the recorded opcode is the entry's last instruction and the recorded instruction count is at least the static count |
| Release demo | `scripts/check_release_demo_vector.py` | re-derives the `RootQueryId` from the recorded `SLEYRQQ1` preimage and checks the response repeats it and the request binding, re-derives the `ExecutionReportId` from the stored `SLEYEXR1` preimage, checks the execute request names the recorded function, and checks the exported exchange carries the recorded head and branch |

Both are codec and identity oracles written from the contract documents. Neither
judges an opcode signature, executes an instruction, or recomputes a value hash,
so the oracle does not become a second semantic kernel.

## Result

- `make conformance` now runs 21 oracle results, all PASS.
- The coverage report reads `INDEPENDENT_CONFORMANCE_COMPLETE`: nineteen of
  nineteen families independently checked, none native-only.
- The decision dossier's SCB1 conformance item records the same.

## Finding

The extended fixture's `opcode` metadata named the last operation of the
fixture's whole operation list, which for a fixture carrying a callee is the
callee's last operation, not the vector's subject: `call-direct-second`
recorded opcode 17 (`tuple_get`) for a `call_direct` vector. The independent
decoder caught it. The emitter now records the entry function's last operation,
and the fixture was regenerated; the bytecode, cache key, value hash, and
observation identity of every vector are unchanged, because the field is
metadata only.

## Open questions for the Council

1. Vulcan: the extended oracle verifies the container and the cache identity
   but not the value hash or observation identity, which would need the program
   and inputs in the fixture and a second semantic implementation. Is the
   codec-and-identity boundary the right one for GA?
2. Ariadne: should the extended fixture record the program graph so a future
   oracle could re-derive the observation identity, or does that invite the
   second kernel the master goal forbids?
3. Nabu: the demo checker lives in `scripts/` while the extended oracle lives
   in the oracle package. Should both live in the package?

## Validation

Landed at `6788cbe`. Tier 1 `make quick` passed. Tier 2 on 2026-09-03:

| Gate | Result | Evidence |
|---|---|---|
| `make conformance` | exit 0 | 21 oracle results PASS (two more than before this package) |
| `make core` | exit 0 | no regression |
| `make adversarial` | exit 0 | no regression |
| `make fuzz-smoke` | exit 0 | no regression |

`make v1` was not run: this is a subsystem handoff, not a release boundary.
