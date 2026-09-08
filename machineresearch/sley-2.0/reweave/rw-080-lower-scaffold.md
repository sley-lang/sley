# RW-080 lowerer scaffold (§1.3) — provisional C0 construction record

Status: PROVISIONAL SCAFFOLD (2026-09-08, operator development override
`rw075_correction.operator_override_2026_09_07`). Seed-authored Sley
scaffold for the third RW-080 toolchain module. It is not accepted
runtime authority: no BOOTSTRAP_READY, C1, SH1/SH2, or release claim
follows. Independent acceptance (Nabu round-12, premium round 2, R2
READY) remains pending; the real lowering plus package assembly arrive
at the RW-110 gate. This file is the §3 construction manifest for the
scaffold closure; behavior is proven by
`crates/sley-vm/tests/rw080_lower_scaffold.rs` (4 tests green).

## 1. Why this unit was runnable (selection)

A repository-wide dependency sweep (2026-09-08) ranked every residual
Sley 2.0 unit against ownership, predecessors, and semantic frost:

- RW-080 §1.1 codec remainder, `codec_main` legs, ZigZag/Text-NFC/
  floats/maps/records/entity kinds: Lane-1-owned, excluded.
- RW-080 §1.2 real checker + RW-100 corpus: blocked on complete
  18-kind codec-produced closures (only Lane-1 WIP, uncommitted).
- RW-080 §1.3 lowerer/builder real: blocked on checked inputs (§1.2).
- RW-080 §1.4 driver (scaffold or real): `BuildError` has no frozen
  native vocabulary (zero hits repo-wide) and the manifest shape needs
  judged graphs that do not exist — a scaffold would invent codes.
- Oracle vectors: every frozen identity (BOOTSTRAP_PROFILE_2,
  HOST_ABI_V2, EXEC_PACKAGE_V2, RAW_BLAKE3_V1, SCB1, SSMC1 epoch-1)
  has accepted/rejected vectors plus an independent checker
  (`INDEPENDENT_CONFORMANCE_COMPLETE`); the one thin spot
  (entity-impact, 0 rejected) sits on a draft contract.
- S20-700 hardening targets (S20-380 wrapper, VM adapter opcodes,
  live confinement, persistent execution/replay reports): semantics
  owned by unfrozen S20-280-full/S20-380-full plus the schema-epoch
  decision.
- Native local track otherwise: every remaining boundary waits on a
  Council review, a schema-epoch decision, or operator authority
  (frontier audit).

The §1.3 lowerer scaffold was the single runnable unit: every
predecessor frozen, no Lane-1 shape needed, no Lane-2 surface
touched, new files only.

## 2. Entry conditions (established before construction)

- Successor baseline frozen: profile v2 `fb2d8cc8…`, ABI v2
  `bc564653…`, package v2 `f4958c5e…`, raw `785205fb…` (per the §1.1
  scaffold record; recomputed there, not re-derived here).
- v2 admit → approve → execute path proven by the §1.1 codec and
  §1.2 checker scaffolds plus this scaffold's own admission (receipt
  binds profile v2).
- Contract §1.3 interfaces fixed: `lower(function_closure) ->
  Result<LoweredModel, LoweringError>` (frozen `LowerErrorCode`
  vocabulary); `build_package` deliberately excluded (no frozen
  `BuildError`; see §5). E1/E2 data, checked integers, bridge for
  byte emission, raw hash for digest inputs; B2V1/V2B1/PSH1/RHW1
  only; scaffold = entry plus vocabulary plus one admitted trivial
  closure.
- Owning judgments frozen natively: S20-260 lowering failures
  26000–26006 (`LowerErrorCode::numeric`, `crates/sley-vm/src/lib.rs`),
  symbols `VM_LOWER_*` (`ERROR_CODES_V1.md`, S20-260). The scaffold
  cites these codes; it does not re-freeze them.
- Baseline: `0a909e1` (main tip; contains all predecessors including
  the mature seed-assembler harness API, absorbs no Lane-1/2
  divergence — Lane 1's work is uncommitted, Lane 2's line is
  separate and unneeded).
- Ownership check: no `rw080_lower|builder|driver` file exists
  (tracked or untracked); Lane-1 dirty files
  (`rw080_codec_program_outer.rs`, `rw080_ns_probe.rs`,
  `.forge/slices/*.json`) untouched; Lane-2 surfaces
  (`check_required_contract_index.py`,
  `REQUIRED_CONTRACT_INDEX_V1.md`) untouched; no new identifier
  domain declared (no registry impact).

## 3. What the scaffold is (and is not)

- IS: one function `lower(marker: UInt32, witness: Bytes) ->
  UInt32` with marker 0 returning the trivial-accept value
  (`LOWER_OK_EMPTY=0`, scaffold-local empty-model marker), markers
  1..=7 trapping `Unreachable` carrying the lowering-leg index in
  frozen `LowerErrorCode` order (1 ProfileUnsupported / 2
  OpcodeUnsupported / 3 SignatureMismatch / 4 ImmediateMismatch / 5
  LocalReferenceInvalid / 6 CacheKeyUnsupported / 7 ResourceLimit),
  and any other marker returning the typed vocabulary representative
  (`PROFILE_UNSUPPORTED=26000`, leg 1). UInt32 carries the 26xxx
  codes, which do not fit the precedents' UInt8.
- IS NOT: no closure lowered, no callee table built, no package
  assembled, no image emitted, no verification performed. All
  lowering legs trap before reading `witness` (proven: distinct
  witness bytes behave identically). The marker stands in for the
  checked-closure inventory the real entry will traverse — the same
  stand-in class as the §1.1 scaffold's unread `Bytes` input and the
  §1.2 scaffold's marker. The real `LoweredModel`, callee table, and
  `build_package` bytes arrive with the functioning lowerer; the
  scaffold needs one success value and one returnable error code to
  prove both exits return values.
- Ownership note: this scaffold touches no codec slice and no
  checker leg. It reuses no builder, record, or fixture from the
  §1.1 Namespace/EntryPoint program path or the §1.2 scaffold; the
  only shared surface is the frozen v2 admit/approve/execute
  boundary every module must cross.

## 4. Construction manifest (every entity/object)

Seed assembler: `crates/sley-vm/tests/rw080_lower_scaffold.rs`
`lower_scaffold()` (fixture-namespace identities `[byte; 32]`, the
established C0 fixture convention; function, parameter, constant,
block, and operation identities are pairwise disjoint within this
image, so references resolve to the intended table entries. These
bytes are fixture-local and freeze no production ABI; numeric reuse
across independent scaffold images is not an execution collision —
each image admits independently). Method: `seed-assembled` from explicit typed Rust
literals; no inference, no repair, no embedded answers beyond the
cited bytes. Semantic owner: lowerer (§1.3). Dependencies: none
executable (leaf scaffold; the driver will feed it checked closures
once the real checker lands — the checked-inputs dependency of §1.3
binds the real corpus, not this scaffold). Created by the seed
assembler; no Sley mutation yet; no prebaked image anywhere.

| entity | id byte(s) | kind |
|---|---|---|
| lower function | 216 | FunctionGraph, params [214, 215], result UInt32, entry block 230 |
| marker param | 214 | Function param 0, UInt32 |
| witness bytes param | 215 | Function param 1, Bytes (unread by design) |
| entry chain block | 230 | tests marker 0 → accept 245 else 231 |
| chain blocks | 231–237 | test markers 1..=7; final else → 246 |
| lowering legs | 238–244 | trap Unreachable + payload leg index |
| accept block | 245 | returns LOWER_OK_EMPTY constant |
| unknown block | 246 | returns PROFILE_UNSUPPORTED constant |
| marker constants K0..K7 | 200–207 | UInt32 0..7 (K1..K7 double as leg payloads) |
| accept constant | 208 | UInt32 0 |
| profile constant | 209 | UInt32 26000 |
| operations | 100+ | sequential construction order: 8 chain (ConstantRef+Equal ×8), 7 leg payloads, 2 value returns |

Profile/epoch/root (closure evidence): `BOOTSTRAP_PROFILE_2`,
epoch `08`*32, root `09`*32, `CacheProfile::EXTENDED_V1`, generous
test limits (recorded in-fixture, not normative).

## 5. Anti-copy hooks (for the later gates; recorded now, not executed)

- No prebaked image: the package image is lowered in-fixture from the
  cited closure and admitted through the staged authority per case run.
- Mutation hook: changing any marker constant, edge target, or trap
  payload breaks the dispatch tests (legs trap the wrong index, the
  trivial closure stops accepting, or the wrong exit is taken); the
  native-parity test breaks if `LowerErrorCode` numerics/symbols
  drift. Rejection hook: malformed closures (wrong constant type,
  rewired edge) must fail admission — later RW-140
  fixed-point/self-change gates execute these; the manifest records
  the hook, not the run.

## 6. Separation and limits

- New files only: the test file plus this manifest. No production
  module, codec slice, checker, gate, or ledger semantic changed for
  this slice.
- Authoring record: two Rust transcription slips in the
  `ExecutionPackage` literal (`globals`/`contracts` as slices plus a
  nonexistent `functions` field, then missing gate/limit fields),
  both repaired against the §1.2 precedent before the first green
  run (`E0308`/`E0560`); no test expectation was written from
  candidate output or rewritten to match it.
- `build_package` excluded with reason: no frozen `BuildError`
  vocabulary exists, and the entry's inputs (checked closure bytes,
  test-plan digest) are unproducible before the §1.2 real corpus.
  The §1.4 driver scaffold is excluded for the same reason plus the
  unproducible manifest.
- Next: real single-function lowering over checked closures plus the
  `build_package` assembler and the RW-110 corpus — only after the
  §1.2 real checker produces checked inputs, or further explicit
  operator direction. This scaffold does not advance or retard the
  §1.1 program path.
- F1 repair (post-review descendant of `a525f66`): independent
  review found the fixture function identity (202) sharing a byte
  with marker constant K2 (202). The function moved 202 → 216, an
  otherwise unused fixture-local byte; marker values, block control
  flow, constants, and vocabulary are untouched, and all externally
  observed scaffold behavior is unchanged. A new in-fixture test asserts the
  within-image identity sets are pairwise disjoint, so this class
  of error cannot silently recur.
