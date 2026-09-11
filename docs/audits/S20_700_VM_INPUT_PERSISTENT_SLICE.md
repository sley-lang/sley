# S20-700 VM Canonical-Input Persistent Slice

Status: scoped persistent landed-surface slice; **full S20-700 remains incomplete**

This slice adds one libFuzzer target at the public typed restricted S20-270 VM
boundary. It selects nine fixed valid functions:

- six identity functions over `Unit`, `Bool`, `Bytes`, `Text`, `Option<Bool>`,
  and `Result<Bool, Unit>`;
- three Boolean functions using `BoolNot`, `BoolAnd`, and `BoolOr`.

One input lane builds values that match each fixture's declared parameter
types. The other builds bounded raw typed or deliberately mismatched
`ConstValue` inputs. Both lanes vary normal, zero, tight, maximum, and raw
execution limits. Every request is submitted twice to both
`validated_execution_input_hashes` and `execute_function`; the target requires
identical judgments. When both succeed, it also verifies schema epoch, state
root, function, and input-hash count bindings and rederives the exact
observation ID. Canonical fixture inputs under the normal limit profile must be
accepted, preventing a deterministic reject-all regression from passing.

Input is capped at 4,096 bytes. Raw requests contain at most four values,
collections at most four items, and byte or text payloads at most 32 bytes. The
deterministic synthetic corpus contains 625 seeds. Corpus, binaries, artifacts,
and command evidence remain under ignored
`evidence/runtime/s20-700-vm-input-libfuzzer/` paths.

The byte mapping is a fuzz-only typed constructor, not a VM bytecode format.
The target calls the existing public execution API, which re-lowers a validated
graph. Sley 2 has no raw-bytecode decoder or execution entry point, and this
slice claims neither. It covers the restricted three-opcode execution profile
and identity pass-through inputs only. The other 52 opcode signatures,
generics, adapters, live cancellation, execution flags, decoding, persistent
reports, and full S20-270 remain unavailable.

Independent Vulcan review remains deferred because the local Forge OAuth
session returns 401. Mutation candidates, merge, and the full S20-700 finding
register remain required.

Focused validation:

```text
cargo +nightly-2026-02-27 clippy --manifest-path fuzz/Cargo.toml --bin vm_canonical_inputs --target-dir evidence/runtime/s20-700-vm-clippy-target -- -D warnings
cargo test -p sley-vm --locked
python3 scripts/check_vm_execution_profile.py
python3 scripts/check_vm_persistent_fuzz_slice.py
make vm-persistent-fuzz-smoke
python3 scripts/run_vm_persistent_fuzz.py --manual
```

## September 2026 fix record: the extended-family lanes reach execution

The September 4 Council round (three reviewers, four P0 entries, two distinct
defects) found the extended-family lane proved lowering judgment only. The lane reused the outer restricted request, so the E2/E3/E4
fixtures died in input validation before any extended opcode ran; the
determinism assertion compared two identical input errors and the observation
assertion never fired. The seed bytes named families they did not select for
any outer fixture but the Unit one, and the 256-run smoke replayed the
769-seed prefix without reaching most family seeds.

The fix, all in the harness plus contract text, no production-code change:

- `extended_family_lane` takes the cursor and builds each fixture's request
  from its own parameter types under generous limits, asserting the first
  execution completes (success or value failure). Canonical `F32`/`F64`
  constructors apply the contract E3 rule (one quiet NaN, no negative zero).
- The restricted refusal pins `VM_LOWER_OPCODE_UNSUPPORTED` for the
  single-graph families. Fixing the lane exposed that the multi-function E6
  and E7a programs never reach the opcode check: narrowing to owned inventory
  is an extended-profile step, so under restricted the shared flat inventory
  fails the single-graph rule first. That refusal is still the lowering
  profile, never an input error, and the completion assertion above keeps a
  malformed fixture from hiding behind either refusal.
- Lane decisions moved to fixed header offsets consumed before any
  variable-length construction; all 144 family seeds select the family they
  name for every outer fixture (verified by enumeration).
- The E6 fixture threads a Bool argument through a nested callee pair,
  exercising argument copy and the multi-entry callee table.
- The runner requires `--runs` to cover the corpus (default 1024 over 769
  seeds) and records the executed run count from the `Done N runs` line,
  failing when it does not cover the corpus.
- Contract revision 11 binds the 128 repeated executions to the vectors,
  states the per-family reachability obligation with checker verification,
  records that the VM ignores `ContractSource`, and splits lane duty
  (per-family reachability) from vector duty (per-opcode).

Deferred with reasons: E6 recursion and the 256-frame ceiling fixture,
per-opcode vectors for `function_ref` and the map accessors, the
per-signature-rule rejection matrix, the `Cursor` distribution rework, the
fuzz input cap raise, and encoder/decoder disjointness assertions. The E6
separate-inventory question stays with Nabu.

## Rounds 7c-7m (REQ-06 re-review wave)

Crash minimization uses `-minimize_crash=1` with exact artifacts (the
round-7 `-merge=1` primitive could not minimize a crasher); the coverage
floor measures on-disk corpus files plus 256 mutations; coverage gates
strictly on inline counters with monotonic `ft` (no silent fallback);
the owner gate counts the rlibs cargo linked (fingerprint-authoritative,
`rlib_linkage` recorded, fail-closed with no mtime fallback); warnings are
captured from the full streams against an explicit allowlist; builds
refuse ambient `RUSTFLAGS`; prior crashers re-execute every smoke
(crash-to-regression); per-input `-timeout=30` and `-rss_limit_mb=2048`
bound hangs; libFuzzer seeds are recorded; build provenance
(`build_locked`, `sancov_scope`) derives from the executed argv; the
fuzz profile enables overflow checks and debug assertions. Manual
campaigns remain operator exploration (no floor/limits parity by
design). Slice oracles were strengthened per verdict (engine-invariant
asserts, must-reject refusals, narrowed Err arms, constructed-valid
import/decode lanes); the pinned clang-18 default has no recorded proof
on this host.

## Rounds 7k-7m (second re-review wave)

The linked-rlib replay carries the build environment (an env-less replay
rebuilt and relinked the binary after the recorded build); the mtime
fallback is deleted, so a failed cargo query fails the gate instead of
passing on unknown provenance. Crash gating distinguishes new crashes
from retested priors (fixed priors pass with a recorded retest). The
minimize step skips clean-retested artifacts, bounds internal steps, and
keeps partial exact artifacts; durations compute last. Slice oracles
gained engine-invariant asserts, must-reject refusals, narrowed Err
arms, constructed-valid lanes, and a server-fixture validity gate with
a unit-level negotiation self-check; filed regressions replay as corpus
seeds. See the wave's decision packets for elevated owner items.
