# S20-420 SMP1 JSON Bridge Campaign (2026-09-03)

Status: contract draft revision 1 written by the integrator with every
Council lane unavailable; Ariadne contract review, Nabu architecture
review, and Vulcan surface review queued.

## Frontier at start

- SMP1 revision 5 is implemented (S20-410, S20-440, S20-330): frames,
  hello, selected profile, limit profile, bounded context, failure
  envelope, stream chunks, and a forty-one-method table.
- SMP1 section 8 requires a generated JSON bridge with a declared binary
  encoding, preserved stable codes and omission states, no semantic
  validation, and no participation in program identity.
- The local completion frontier names S20-420 as the next authority-safe
  package and its guard blocks `crates/sley-json-bridge` until the summary
  allows it.

## Design brief

Contract: `docs/spec/SMP1_JSON_BRIDGE_V1.md`, ADR-0034, stage checker
`scripts/check_smp1_json_bridge_contract.py`, method-table generator
`scripts/generate_smp1_json_bridge_table.py` (drift-gated in `make quick`).

- Lowercase hex for bytes; JSON numbers up to 2^53 - 1 and decimal strings
  above; frozen names for kinds, retryability, and features; exact object
  shapes with lexicographic emission.
- `frame_from_json` re-encodes through the frozen `sley-protocol` codec, so
  the bytes stay the only canonical form and wire failures keep their
  `PROTOCOL_*` codes.
- Method names come from the generated table
  `conformance/smp1-json-bridge/v1/methods.json`, embedded and tested
  against the frozen `Method` table.
- Codes 42000 through 42004 (shape, number, hex, method, resource limit).

## Open questions for the reviews

- Whether the bridge should also render the S20-440 body records (cancel,
  stream, budget) as objects instead of hex.
- Whether decimal strings for large integers should be accepted for every
  integer field or only for the frame's request identity.
- Whether the resource ceiling (268,435,456 bytes, depth 32) should follow
  the negotiated frame ceiling instead of a fixed value.

## Records

| Stage | Commit | Tier 1 | Notes |
|---|---|---|---|
| Contract draft revision 1 | `be9843c` | green | ADR-0034, checker, generator, method table |
| Implementation, revision 2, fixture, oracle, fuzz slice | `7722d33` | green | `crates/sley-json-bridge`; 8 native tests; oracle 5 vectors, 31 rejections; fuzz smoke 631 seeds, 632 runs, 12.9 s |

## Tier 2 handoff record (2026-09-03, at `7722d33`)

| Gate | Result | Wall time | Evidence |
|---|---|---:|---|
| `make core` | exit 0 | 12 s | 973 tests passed, 0 failed across 35 test binaries |
| `make conformance` | exit 0 | 10 s | 19 oracle results PASS, including `check_smp1_json_bridge_vector.py` |
| `make adversarial` | exit 0 | 9 s | 596 tests passed, 0 failed |
| `make fuzz-smoke` | exit 0 | 1 s | 5 bounded smoke tests passed |
| `make smp1-json-bridge-persistent-fuzz-smoke` | exit 0 | under 1 s (cached build) | 632 runs, PASS |
| `make smp1-persistent-fuzz-smoke` | exit 0 | 3 s | 653 runs, PASS |

Logs were captured under the session scratchpad; `make v1` was skipped because this is a subsystem handoff, not a release boundary.

## Observations outside the package

- `cargo clippy -p sley-json-bridge --all-targets -- -D warnings` also
  lints path dependencies under `-D warnings` and stopped on a pre-existing
  `too_many_lines` finding in `crates/sley-store/src/lib.rs`
  (`recover_staged_with_limits`, 107 lines), which no repository gate runs
  clippy against. The bridge lint reran with `--no-deps` and is clean; the
  sley-store finding is recorded here, untouched, because that crate is
  frozen under S20-530/S20-540.
