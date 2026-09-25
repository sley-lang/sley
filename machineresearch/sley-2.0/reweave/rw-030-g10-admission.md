# RW-030 §18 admission record — G-10 byte access/construction (2026-09-06)

Gap: G-10 (RW-040 `rw-040-m2-gap-list-slice-2.json`): no landed E1–E6+E7a
opcode takes apart or builds Bytes; whole-value ops only (order, equality,
`value_hash`). §10.2 items 1 (SCB decode) and 4 (image assembly) require
byte access/construction. Status at admission: OPEN missing capability,
BOOTSTRAP_BLOCKER, `BOOTSTRAP_READY` barred until resolved with contract
review.

This record charters the boundary input and selects the remedy. Production
implementation belongs to RW-050 after prerequisites close; contract proof
(byte/error parity) belongs to RW-090. No new capability is implemented
here.

## 1. Concretely required computations

C1 — SCB decode (§10.2 item 1), behavior reference `crates/sley-scb1`
(native seed; `decode_payload_exact`, `Schema`, `Cursor::new`,
`check_finished`):
- tag dispatch over uvar-prefixed tags (`encode_uvar`/`encode_uvar128`
  counterparts);
- length-prefix read with cursor+length bounds enforcement (overrun =
  strict rejection);
- strict rejection policy (non-canonical forms, trailing bytes via
  `check_finished`, schema mismatch);
- bounded traversal emitting typed graph data (records/variants/options).

C2 — image assembly (§10.2 item 4), emission reference `encode_*`
counterparts in `crates/sley-scb1`:
- lowering output (instruction streams as typed data) planned as an exact
  byte sequence by the Sley image-builder;
- sequence emitted to the derived execution image through confined I/O.

Both computations split into (a) program/schema-aware judgment, which
§10.2 assigns to Sley, and (b) representation transport, which §10.3
permits native (primitive value operations, confined byte I/O).

## 2. Alternatives considered (existing semantics first)

(i) Whole-value Bytes threading — insufficient. Established by execution:
E1 offers only order/equality/hash; input cannot be inspected. Rejected
with evidence, not preference.

 (ii) New Sley-level byte ops in the bootstrap profile (byte-at / slice /
byte-append as E-profile opcodes) — genuine new capability. Requires full
§18 admission anyway, plus new VM semantic surface, typing, fuel rules,
and negative corpus for each op; broadens what RW-050 must freeze and
what RW-070 must deny-abuse coverage for. Heavier than necessary:
everything the Sley side needs except bounded growth (total bounded
read, integer dispatch, iteration, typed failure) already exists. The
admitted `vector-push` in (v) is deliberately NOT byte-specific: one
generic growth op has a smaller review surface than a byte-op family.

(iii) Bounded lossless representation bridge — SELECTED for transport.
Native performs only representation conversion `Bytes <->
Vector<UInt(8)>` (exact element type: 8-bit width exists in the profile,
`VM_EXTENDED_OPCODE_PROFILE_V1.md` E1/E2; the `UInt(8)` type itself is
the authoritative octet-range enforcement, so the converter enforces
only the length cap). Every
judgment (tag dispatch, length/bounds policy, strict rejection,
canonical-form checks, emission planning) runs in Sley over existing
total ops:
- `VectorGet` is total and bounds-safe by construction (OOB yields None;
  `crates/sley-vm/src/extended.rs`, execution arm): cursor advance with
  `None` as end-of-input is typed, no trap path;
- `VectorSet` is bounds-checked (`Result` with `Index` failure);
- E1/E4 integer compare and arithmetic dispatch tags and accumulate
  lengths (exact-width behavior already exercised);
- backedge/`CondBranch` loops and bounded recursion iterate
  (RW-040 loop mapping; 256-frame ceiling, fuel budgets,
  1,048,576-cell cap carried as profile constraints);
- typed failure reports rejection (existing failure exercise).

(iv) Chunked-encode import (Sley passes `Vector<Vector<UInt(8)>>`,
native concatenates) — considered, REJECTED. It avoids a new op only by
forcing static preallocation gymnastics (outer capacity fixed at
lowering; slots filled by index; length carried out-of-band), which
obscures the emission plan and uglifies the import contract (vector +
count instead of one value). The winning property of the bridge — the
exact output byte sequence is one Sley value — is lost.

(v) Bridge + one minimal bounded growth op — SELECTED for
construction. `vector_new` fixes length at lowering arity and
`vector_set` cannot grow, so no existing-semantics path builds an
arbitrary-length output vector (verified: profile E1 table,
`extended.rs` arms). Admitted here as part of this decision (not
implemented here): `vector-push`, generic over `T` and byte-unaware,
`(Vector<T>, T) -> Result<Vector<T>, Limit>` refusing over the frozen
profile length cap, fuel-charged per push. One total generic op; it
carries no byte, schema, or image awareness, so §10.4 is unaffected.
Alternatives (iv) and new byte-specific ops (ii) lose on plan clarity
and review surface respectively.

## 3. Exact implications of the selected bridge

- Types: `Bytes` (opaque at the Sley level except whole-value ops) <->
  `Vector<UInt(8)>`. Octet range is enforced by the exact element type,
  not by a separately reviewed range check; the converter enforces only
  the profile length cap. Output vectors are accumulated with the
  admitted `vector-push` (§2(v)); no other construction path is assumed.
- Bounds: conversion and push refuse input/results over the frozen
  profile length cap (RW-050 freezes the cap; until then neither exists,
  so no bound is assumed). Traversal bounds are Sley-side cursor
  invariants checked against the converted length before each read.
- Failure: total conversion (dishonest input cannot trap the host);
  oversize input/result = typed `Limit` failure (octet range needs no
  separate check: the `UInt(8)` element type enforces it); all
  strict-rejection *policy* (what counts as malformed SCB) is decided
  in Sley, never by the converter.
- Allocation/fuel: allocation is native-capped; every converted element
  charges fuel through the existing `charge_action` mechanism; Sley-side
  vectors remain under profile caps. Exact per-element charge frozen at
  RW-050 profile freeze.
- Encoding: SCB1 per the `sley-scb1` reference. The Sley decoder must
  reach byte/error parity against the reference at RW-090; parity
  failure is a defect in the Sley program, never grounds to move
  judgment into the converter.
- Effects: conversion declares allocation+fuel effects; image emission
  declares confined-write effects bound to the image path. No ambient
  I/O, no network, no ambient filesystem.
- Dependency impact: `sley-vm` gains two conversion primitives plus one
  admitted generic growth primitive (`vector-push`; exact carrier —
  E-profile opcode or versioned host-ABI import — frozen at RW-050
  profile freeze / RW-070 ABI freeze, not pre-frozen here); the two
  conversions always become RW-070 import-manifest entries;
  `vector-push` becomes either an RW-050 E-profile opcode or, if
  selected as a host import, an RW-070 import-manifest entry; RW-090
  gains a parity corpus. No new crate, no FFI surface, no compiler
  service.

## 4. Anti-shortcut constraints (§10.4, binding on RW-050/RW-070)

The converter must not implement program/schema-aware canonical
validation, type checking, lowering, discharge, or compiler-image
assembly on behalf of the Sley toolchain. Concretely it must not parse
tags, dispatch on schema, enforce canonical form, assemble images,
judge Witness trust, or return verdicts (u8 vectors only). The admitted
`vector-push` must stay generic over `T`: no byte, schema, or image
awareness in its typing, failure, or fuel rules. RW-050
attaches negative tests feeding program bytes and asserting vector-only
output; RW-070 denies `compile`/`typecheck`/`lower`/`validate_program`/
`discharge` imports by test. Violation invalidates SH2, not just the
slice.

## 5. Why this is admission, not invention

Concrete required computations (C1/C2) above; alternatives with
existing semantics compared with evidence (i rejected on executed
behavior, ii heavier than needed); safety/encoding/type/effect
implications fixed in §3; dependency impact fixed in §3; independent
contract review via Ariadne + Nabu rounds on this record (transcripts
in `reviews/`; verdicts in the charter closeout). No excluded §18
effect added (no FFI, reflection, source DSL, native compiler
service). Changing this selection later (e.g. to accessor ops) needs a
new admission record, not a quiet profile edit.

## 6. Remaining contract work (not claimed here)

- RW-050 slice 1: implement the two conversions with cap/fuel/failure
  exactly as §3, implement the admitted generic `vector-push` (carrier
  per §2(v); generic over `T`, `Limit`-refusing, fuel-charged); freeze
  the length cap and per-element charges and the push carrier; land
  byte-level workloads (decode read path AND variable-length encode
  construction path) + negative corpus; record all three imports/ops as
  permitted in the profile freeze.
- RW-070: freeze converter ABI/import entries; ship import-denial tests.
- RW-090: Sley codec byte/error parity against `sley-scb1`.
- BOOTSTRAP_PROFILE_1 must not depend on byte access until the RW-050
  slice lands (G-10 acceptance unchanged).

## 7. Amendment A1 — owner-compatible pure-primitive carrier (2026-09-06)

Authority: operator / S20-230-owner decision on `rw-050-slice-1.md` §8,
approving option (i) with the narrower boundary stated there. This section
amends the record per §5 (amendment, not a quiet edit); §§1–6 stand as the
original selection and its rationale.

Why the original carrier was incompatible: §§2–3 selected
`adapter_invoke` entries with empty effect sets, but the landed S20-230
validator (§§1.4/3.3) failed every zero-effect row closed with
`ADAPTER_EFFECT_CARDINALITY`, and effectful rows were uncallable without
S20-230-full effectful lowering (verified in code; Ariadne round 3,
preserved in `reviews/reweave-rw050-ariadne-r3-2026-09-06.log`; RW-050
slice 1 closed as a negative result). The admission had selected imports
without checking owner-contract compatibility — a gap in this record, not
just in the implementation.

Amended carrier: the three admitted operations ride `adapter_invoke` as
registered PURE_DETERMINISTIC host primitives under S20-230 §1.5 (owner
amendment A1, G-10-tied), not as bare effectless imports:

- conversions: frozen shapes `P-BYTES-FROM` / `P-BYTES-TO` (scope `Unit`,
  `Index` failure pin);
- push: frozen shape `P-PUSH` (scope exactly the response type, response
  exactly `Vector` of the request type — the Ariadne HIGH-2 relationship
  pin, now contract; genericity by monomorphization, each concrete use a
  closed row);
- registration: exact identities `SLY1/BRIDGE/` + `B2V1` / `V2B1` /
  `PSH1`, ABI version 1, recorded as permitted-import entries at the
  RW-050 profile freeze and the RW-070 import manifest. This record names
  the registration content; the freezes own it. `host-boundary.json` is
  untouched (byte-stable by design; digest-cited).
- layering: S20-230 judges pure form (shape); positive registration
  (identity/version) is enforced at the lowering/profile/manifest gates,
  which fail closed on missing, unknown, or unregistered zero-effect
  imports. No `AdapterCall` is manufactured; full effectful lowering stays
  out of scope.

Unchanged: §4 anti-shortcut constraints bind RW-050/RW-070 exactly as
written (plus the owner's SH2 semantic-authority list, which §4 already
covers and S20-230 §1.5 now restates); alternatives analysis (§2) stands;
no new capability beyond the three reviewed operations; any further
primitive needs a new owner amendment.

Procedure from here (owner-stated): negative coverage for the closed pure
class, independent semantic/boundary review of the amendment, then
reconciled re-landing of the parked RW-050 prototype with the push pin,
then focused validation. Slice 1 turns positive only on new reviewed
implementation evidence; its negative result stands.
