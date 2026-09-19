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
deterministic synthetic corpus contains 789 seeds. Corpus, binaries, artifacts,
and command evidence remain under ignored
`evidence/runtime/s20-700-vm-input-libfuzzer/` paths.

The byte mapping is a fuzz-only typed constructor, not a VM bytecode format.
The target calls the existing public execution API, which re-lowers a validated
graph. This slice does not cover the loaded-image path
(`sley_vm::load_image`/`execute_loaded_image`, an RW-070 owner obligation
flagged for escalation): no fuzz target in this lane touches it, and the
slice checker forbids those symbols. The lane covers the restricted
three-opcode execution profile, identity pass-through inputs, and the nine
extended-family fixtures (E1 constant, E2 arithmetic, E3 float, E4 map, E5
cell, E6 direct call, E7a contract assertion, E8 bridge adapter_invoke).
Generics, live cancellation, execution flags, decoding, persistent reports,
and full S20-270 remain unavailable.

Independent Vulcan reviews dispatch through forge-council; the review
transcripts are filed under `evidence/review/verdicts/`. Mutation
candidates, merge, and the full S20-700 finding register remain required.

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
  variable-length construction; all 162 family seeds (9 fixtures x 9 families x 2 profiles) select the family they
  name for every outer fixture (verified by enumeration).
- The E6 fixture threads a Bool argument through a nested callee pair,
  exercising argument copy and the multi-entry callee table.
- The runner requires `--runs` to cover the corpus (default 1576 over 788
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

## Repair round (completion oracle + must-reject + records)

The correct-lane re-review (REQ-08 item 3, REVISE) found the completion
oracle one step short of the contract: `is_ok()` admits
InternalInvariant, Trap, and ResourceLimit outcomes. Fixed at the root
(target, not checker-only): both the outer lane (canonical fixtures
under normal limits) and the family lane now require
`ExecutionTermination::Success` and require the Success payload to be
result-canonical (`sley_mutate::encode_const_value` succeeds, closing
the E3 result-canonicality gap too). Every family's value failure (E2
Arithmetic, E4 DuplicateKey, E7a ContractViolation) is a
Success(Result::Err) value, so the strengthening is uniform; the
checker pins the new assert messages.

The raw lane carries a must-reject oracle: a count mismatch asserts
`Err(Exec(InputCountMismatch))`, any type mismatch asserts failure
(a fail-open validation regression would otherwise stay green on
determinism). The E3 float-canonicality sub-draw feeds raw u64 bits
with the F64 type: refusal must come from exactly one of the two
canonicality layers — `TYPE_FLOAT_NON_CANONICAL` at the type layer for
constructed values, `VM_EXEC_INPUT_NOT_CANONICAL` on the codec path —
and success requires canonical bits plus Success termination. The first
version of the sub-draw pinned only the Exec layer and crashed on
`ff ff 02` (retained as `crash-b55c33e9…`, retests clean): a
harness-oracle error, never an engine defect, filed as
`fuzz/regressions/S20_700_VM_001.json`. Note the codec float arm is
unreachable at the VM boundary for floats (`check_constant` always
precedes `require_canonical_form` with an identical predicate): it is
kept as the documented second layer because the map-order lane reaches
it for maps, not because floats can reach it. The earlier bridge tamper
no-op (`20 45 6b`, RW-050) is filed as `S20_700_VM_002.json`.

Records: seed counts corrected (788 seeds, 1576-run floor); lane text
covers E1–E8; the loaded-image path is re-scoped to this slice (RW-070
owner obligation, checker-forbidden symbols); the checker validates the
durable proof record (PASS, floor coverage, no new crashes, owner
sancov, source-commit shape). Adequacy notes for the next target round:
E6/E7a accepted any `LoweringError::Cfg` (not the specific single-graph
failure) until the round below pinned `GRAPH_INVENTORY_MISMATCH`; the
map-order lane covers one top-level map shape; limit profiles 1–3 assert
determinism only.

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

## Round 9 (2026-09-19) — unlanded-E7 negative lane and V-06

- Item 10 (advisory, carried since 76227765): `unlanded_opcode_lane` runs
  when `family_gate % 3 == 1` and builds a one-operation program over the
  opcodes slice E7a did not land — `TestObserve` (145, observation
  immediate), `EffectRequest` (160, no immediate), `CapabilityNarrow` (162,
  index immediate) — selected by `family_selector % 3`. Under both
  `EXTENDED_V1` and `RESTRICTED_V1` the program must be refused by exactly
  `VM_LOWER_OPCODE_UNSUPPORTED`; acceptance, an input error or a signature
  judgment panics. Both selectors derive from header bytes already read, so
  every existing seed keeps the lane it names.
- V-06: the E6/E7a restricted refusal now pins
  `CfgValidationError::Cfg(GRAPH_INVENTORY_MISMATCH)` (the function's block
  list does not cover the shared flat inventory — the single-graph rule)
  instead of any `Cfg` refusal.
- The slice checker pins both lanes' markers.

