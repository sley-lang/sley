# RW-080 codec scaffold (§1.1) — provisional C0 construction record

Status: PROVISIONAL SCAFFOLD (2026-09-07, operator development override).
Seed-authored Sley scaffold for the first RW-080 toolchain module. It is
not accepted runtime authority: no BOOTSTRAP_READY, C1, SH1/SH2, or
release claim follows. Independent acceptance (Nabu round-12, premium
round 2, R2 READY) remains pending; the real decoder/encoder arrives at
the RW-100 gate. This file is the §3 construction manifest for the
scaffold closure; behavior is proven by
`crates/sley-vm/tests/rw080_codec_scaffold.rs` (2 tests green).

## 1. Entry conditions (established before construction)

- Successor baseline frozen: profile v2 `fb2d8cc8…`, ABI v2
  `bc564653…`, package v2 `f4958c5e…`, raw `785205fb…` (recomputed
  2026-09-07 during native review; v1 records intact).
- v2 admit → approve → execute path proven by the AR-05 replay tests
  plus the scaffold's own admission (receipt binds profile v2).
- Contract §1.1 interfaces fixed: `codec_main(operation, input)`,
  E1/E2/E4/E5+hashing within the 42 frozen opcodes, B2V1/V2B1/PSH1/RHW1
  only, scaffold = dispatch + vocabulary + unimplemented traps.

## 2. What the scaffold is (and is not)

- IS: one function `codec_main(op: UInt8, input: Bytes) -> UInt8`
  dispatching selectors 0..=3 (`decode_program | encode_program |
  decode_schema | encode_schema`) to unimplemented legs, each trapping
  `Unreachable` (tag 1) carrying its leg index as payload; selector
  outside 0..=3 returns the typed vocabulary value VERSION=6.
- IS NOT: no framing parsed, no bytes produced, no judgment, no
  lowering, no image assembly. All real inputs trap before reading
  `input` (proven: distinct input bytes trap identically). The error
  vocabulary is single-UInt discriminants (TRUNCATED=1 TRAILING=2
  MALFORMED_TAG=3 SHAPE=4 COUNT=5 VERSION=6 LIMIT=7); the real
  CodecError enum arrives with the functioning decoder.

## 3. Construction manifest (every entity/object)

Seed assembler: `crates/sley-vm/tests/rw080_codec_scaffold.rs`
`codec_scaffold()` (fixture-namespace identities `[byte; 32]`, the
established C0 fixture convention; collision-free within the closure
by the disjoint ranges below). Method: `seed-assembled` from explicit
typed Rust literals; no inference, no repair, no embedded answers
beyond the cited bytes. Semantic owner: codec (§1.1). Dependencies:
none (leaf module; the driver feeds it bytes). Created by the seed
assembler; no Sley mutation yet; no prebaked image anywhere.

| entity | id byte(s) | kind |
|---|---|---|
| codec function | 200 | FunctionGraph, params [210, 211], result UInt8, entry block 220 |
| op selector param | 210 | Function param 0, UInt8 |
| input bytes param | 211 | Function param 1, Bytes (unread by design) |
| entry chain block | 220 | tests selector 0 → leg 221 else 226 |
| chain blocks | 226, 227, 228 | test selectors 1, 2, 3; final else → 225 |
| operation legs | 221, 222, 223, 224 | trap Unreachable + payload leg index |
| unknown block | 225 | returns VERSION constant |
| selector constants K0..K3 | 230–233 | UInt8 0..3 (double as leg payloads) |
| VERSION constant | 234 | UInt8 6 |
| operations | 100–112 | sequential construction order: 8 chain (ConstantRef+Equal ×4), 4 leg payloads, 1 version |

Profile/epoch/root (closure evidence): `BOOTSTRAP_PROFILE_2`,
epoch `08`*32, root `09`*32, `CacheProfile::EXTENDED_V1`, generous
test limits (recorded in-fixture, not normative).

## 4. Anti-copy hooks (for the later gates; recorded now, not executed)

- No prebaked image: the package image is lowered in-fixture from the
  cited closure and admitted through the staged authority per case run.
- Mutation hook: changing any selector constant, edge target, or trap
  payload breaks the dispatch tests (legs trap the wrong index or the
  wrong exit is taken). Rejection hook: malformed closures (wrong
  constant type, rewired edge) must fail admission — later RW-140
  fixed-point/self-change gates execute these; the manifest records
  the hook, not the run.

## 5. Separation and limits

- New files only: the test file plus this manifest. No production
  module, checker, gate, or ledger semantic changed for this slice
  (ledger gains one provisional pointer, no status flip; `rw080`
  stays BLOCKED pending formal acceptance).
- Next: real strict decoder/encoder + byte/error parity corpus (RW-100
  gate), only after independent boundary acceptance or further
  explicit operator direction.
