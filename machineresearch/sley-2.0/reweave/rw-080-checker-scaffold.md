# RW-080 checker scaffold (§1.2) — provisional C0 construction record

Status: PROVISIONAL SCAFFOLD (2026-09-08, operator development override
`rw075_correction.operator_override_2026_09_07`). Seed-authored Sley
scaffold for the second RW-080 toolchain module. It is not accepted
runtime authority: no BOOTSTRAP_READY, C1, SH1/SH2, or release claim
follows. Independent acceptance (Nabu round-12, premium round 2, R2
READY) remains pending; the real phase judgments arrive at the RW-100
gate. This file is the §3 construction manifest for the scaffold
closure; behavior is proven by
`crates/sley-vm/tests/rw080_checker_scaffold.rs` (2 tests green).

## 1. Entry conditions (established before construction)

- Successor baseline frozen: profile v2 `fb2d8cc8…`, ABI v2
  `bc564653…`, package v2 `f4958c5e…`, raw `785205fb…` (per the §1.1
  scaffold record; recomputed there, not re-derived here).
- v2 admit → approve → execute path proven by the §1.1 codec scaffold
  plus this scaffold's own admission (receipt binds profile v2).
- Contract §1.2 interfaces fixed: `check_program(program_closure) ->
  CheckResult`, E1/E2/E4 plus E5 cells/hashing, E6 direct calls,
  bridge for diagnostic byte rendering, B2V1/V2B1/PSH1/RHW1 only,
  scaffold = entry plus error vocabulary plus one admitted trivial
  program.
- Owning judgments frozen natively: S20-210 type failures 21xxx
  (`TypeErrorCode`), S20-220 graph/CFG failures 22xxx
  (`CfgErrorCode`), S20-230 effect failures 23xxx (`EffectErrorCode`).
  The scaffold cites these ranges; it does not re-freeze them.

## 2. What the scaffold is (and is not)

- IS: one function `check_program(marker: UInt8, witness: Bytes) ->
  UInt8` with marker 0 returning the trivial-accept value
  (`ACCEPT_EMPTY_PLAN=0`, scaffold-local empty-plan marker), markers
  1..=3 trapping `Unreachable` carrying the judgment-phase index
  (1 type / 2 CFG / 3 effect), and any other marker returning the
  typed vocabulary representative (`TYPE=1`, S20-210 family).
- IS NOT: no closure traversed, no phase judged, no test plan built,
  no diagnostic rendered. All judgment legs trap before reading
  `witness` (proven: distinct witness bytes behave identically). The
  marker stands in for the closure inventory the real entry will
  traverse — the same stand-in class as the §1.1 scaffold's unread
  `Bytes` input. The frozen numeric codes (21xxx/22xxx/23xxx) arrive
  with the functioning checker; the scaffold needs one success value
  and one returnable error code to prove both exits return values.
- Ownership note: this scaffold touches no codec slice. It reuses no
  builder, record, or fixture from the §1.1 Namespace/EntryPoint
  program path; the only shared surface is the frozen v2
  admit/approve/execute boundary both modules must cross.

## 3. Construction manifest (every entity/object)

Seed assembler: `crates/sley-vm/tests/rw080_checker_scaffold.rs`
`checker_scaffold()` (fixture-namespace identities `[byte; 32]`, the
established C0 fixture convention; disjoint from the §1.1 scaffold
ranges by choice although each image admits independently).
Method: `seed-assembled` from explicit typed Rust literals; no
inference, no repair, no embedded answers beyond the cited bytes.
Semantic owner: checker (§1.2). Dependencies: none executable (leaf
scaffold; the driver will feed it closures once the real checker
lands — the codec-inputs dependency of §1.2 binds the real corpus,
not this scaffold). Created by the seed assembler; no Sley mutation
yet; no prebaked image anywhere.

| entity | id byte(s) | kind |
|---|---|---|
| checker function | 201 | FunctionGraph, params [212, 213], result UInt8, entry block 240 |
| marker param | 212 | Function param 0, UInt8 |
| witness bytes param | 213 | Function param 1, Bytes (unread by design) |
| entry chain block | 240 | tests marker 0 → accept 244 else 246 |
| chain blocks | 246, 247, 248 | test markers 1, 2, 3; final else → 245 |
| judgment legs | 241, 242, 243 | trap Unreachable + payload phase index |
| accept block | 244 | returns ACCEPT_EMPTY_PLAN constant |
| unknown block | 245 | returns TYPE constant |
| marker constants K0..K3 | 250–253 | UInt8 0..3 (K1..K3 double as leg payloads) |
| accept constant | 254 | UInt8 0 |
| type constant | 255 | UInt8 1 |
| operations | 120+ | sequential construction order: 8 chain (ConstantRef+Equal ×4), 3 leg payloads, 2 value returns |

Profile/epoch/root (closure evidence): `BOOTSTRAP_PROFILE_2`,
epoch `08`*32, root `09`*32, `CacheProfile::EXTENDED_V1`, generous
test limits (recorded in-fixture, not normative).

## 4. Anti-copy hooks (for the later gates; recorded now, not executed)

- No prebaked image: the package image is lowered in-fixture from the
  cited closure and admitted through the staged authority per case run.
- Mutation hook: changing any marker constant, edge target, or trap
  payload breaks the dispatch tests (legs trap the wrong index, the
  trivial program stops accepting, or the wrong exit is taken).
  Rejection hook: malformed closures (wrong constant type, rewired
  edge) must fail admission — later RW-140 fixed-point/self-change
  gates execute these; the manifest records the hook, not the run.

## 5. Separation and limits

- New files only: the test file plus this manifest. No production
  module, codec slice, checker, gate, or ledger semantic changed for
  this slice (ledger gains one provisional pointer, no status flip;
  `rw080` stays BLOCKED pending formal acceptance).
- Authoring record: one Rust arity slip (`approved` passed by value at
  three call sites, `E0308`) repaired before the first green run; no
  test expectation was written from candidate output or rewritten to
  match it.
- Next: real phase judgments (S20-210/220/230 order with the frozen
  vocabularies) plus the mandatory test plan and the RW-100 corpus —
  only after independent boundary acceptance or further explicit
  operator direction. The real corpus takes codec-produced closures,
  which binds it behind the §1.1 program path; this scaffold does not
  advance or retard that path.
