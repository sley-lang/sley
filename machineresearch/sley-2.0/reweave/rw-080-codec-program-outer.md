# RW-080 §1.1 program slice 3: record framing engine (FixtureEmptyObject complete, program outer bound) — provisional C0 construction record

Status: PROVISIONAL (2026-09-07, operator development override
`rw075_correction.operator_override_2026_09_07`). Two Sley functions with
real strict record algorithms through the v2 admit/approve/execute
boundary, plus a bound program-outer contract with measured native
fixtures. Not accepted runtime authority; `rw080` stays BLOCKED and the
R2 gate stays NOT_READY pending independent Nabu round-12 review plus
premium round 2 plus S20-780 independent acceptance.

## 0. Package-name reconciliation (no new planning campaign)

Reuse of slices 1-2 reconciliation (`rw-080-codec-uvar.md` §0,
`rw-080-codec-envelope.md` §0, no new campaign, no alias beyond those
notes):

- RW-080: construction stage and contract (`rw-080-contract.md`).
- RW-090: codec byte/error parity acceptance gate.
- RW-100: semantic checker/test-planner gate only (never imported here).

## 1. What was built

Seed-authored Sley functions (test file is the seed-assembler record):

- `decode_empty(payload: Bytes, unit: Unit) -> Result<Unit, Bytes>` —
  exact `decode_empty_record` behavior
  (`crates/sley-scb1/src/lib.rs:960`): count via `CallDirect`
  `decode_uvar` width 64, `count > 65535` refuses `RESOURCE_LIMIT`
  (`read_record_field_count`, `lib.rs:1287`), `count != 0` refuses
  `FIELD_UNKNOWN`, trailing (`pos != len`) refuses `TRAILING_BYTES`.
  Uvar `Err` bytes forwarded unchanged. `Ok` carries `Unit` (input unit;
  `Unit` has one value). Sley owns parsing, count decision, trailing
  check, and verdict. No loops (single uvar + comparisons).
- `encode_empty(unit: Unit) -> Result<Bytes, Bytes>` — canonical `00`
  emission (`encode_uvar(0)`): `VectorNew` empty, `PSH1` push `0u8`,
  `V2B1` output. No loops (fixed 1B, sequential pushes, no backedges).

Images: decode-only (entry empty-decode, `CallDirect` callee
`decode_uvar`), encode-only (no callee). Each admitted under
`BOOTSTRAP_PROFILE_2` and approved through `admit_v2_package` (C0 seed
route, declared). Bridge uses: decode 1 (`B2V1`), encode 2 (`PSH1`,
`V2B1`). No `RHW1` here (digest stays in the envelope slice).

Source: `crates/sley-vm/tests/rw080_codec_program_outer.rs` (this
record's seed-assembler artifact; `build_decode`/`build_exact`/
`build_encode` reused verbatim from slice 1 for the callee; outer
`build_outer_decode` and reqbool `build_fixture_reqbool_decode` present
as `dead_code` in-progress framing (count + field-1 chain, parked as
`UNKNOWN`, explicitly not claimed complete; lands next with loops +
field-2 + return + `encode_outer`). Entity namespaces per image are
two-byte `(namespace, index)` ids under disjoint `Ns` (decode
51/52/53/54, empty-decode 51/55/56/57 sharing `k=51`; empty-encode
61/62/63/64), deterministic and collision-free by construction.

## 2. Leg status and canonical binding (no invented wire formats)

Contract §1.1 names four operations symbolically. Binding to accepted
contracts (SCB1 normative, SSMC1 normative, `sley-mutate/object.rs`
reference, `sley-schema` registry, `SSMC1_EPOCH1_SCHEMA.txt` manifest):

| Leg (scaffold selector) | Canonical binding | Status |
|---|---|---|
| 0 `decode_program` | SCB1 standalone envelope (`SCB1.md` §2: magic 8B `SLEYSCB1`, version uvar 1 w64, contract_tag uvar w32, epoch 32B, payload_len uvar w64, payload Record, digest 32B BLAKE3-256 `domain ++ preimage`, no trailing; §9: 67,108,864B / 64 MiB standalone, 16,777,216B payload, 64 depth, 65,535 fields, 1M elements, 134,217,728B allocation; §10 order: magic, version, contract (incl. declared mismatch), epoch, len (`RESOURCE_LIMIT` past max), bounds (`LENGTH_OVERFLOW`), trailing (before digest), digest, then payload) with `contract_tag=200`, `contract_domain=sley2.object.v1`, `digest_domain_tag=3`, `kind_tag=200` (`SSMC1.md` §2); payload is closed `EntityObject` Record tags 1..4 (`entity_id FixedBytes32!`, `body EntityBody!` union 1..18, `label NormalizedLabel?`, `semantic_fingerprint FixedBytes32?`; `SSMC1_EPOCH1_SCHEMA.txt:6,25`; fields ascending, required `{1,2}`, optional `{3,4}`, duplicates/order/unknown/missing per SCB1 §5; references are `EntityId` (never paths/labels/addresses; `SSMC1.md` §2, `sley-id` rules); canonical bytes via `sley-mutate/src/object.rs:88-158` (`build_entity_object`/`import_entity_object`, preimage `MAGIC ++ uvar(1) ++ uvar(200) ++ epoch ++ uvar(len) ++ payload`, `ObjectId=BLAKE3(preimage)`, stored `preimage ++ digest`, `MAX_STANDALONE_BYTES`); errors `SCB_*` for framing + `SSMC_*` 20000-20015 for body (`SSMC1.md` §10) + `SCHEMA_*` for epoch/registry; responsibility: byte-exact framing + body structure, strict rejection, bounded traversal; owns NO type/CFG/effect/contract/test judgments (RW-100), NO lowering/image (RW-110), NO driver (RW-120) | STUB (trap; full 18-kind body + label/NFC + fingerprint verifier not yet wired; outer 2-field framing in-progress, see §9) |
| 1 `encode_program` | Inverse of leg 0: canonical `EntityObjectRecord` (`entity_id` 32B, `body` union bytes, optional label NFC ≤1024B, optional fingerprint 32B) via `encode_object_record` (`object.rs:160-175`: `encode_record([(1, entity_id),(2, body),...])`) to payload, then envelope preimage + digest as above; byte-exact, deterministic, `LIMIT` for resource exhaustion (never truncation) | STUB (trap; `encode_outer` 2-field + program envelope tag 200 land next) |
| 2 `decode_schema` | Schema epoch record + manifest: `SchemaEpochRecordV1` (`sley-schema/src/lib.rs:333`: epoch_number, scb_format 1, hash_tag 1, unicode NFC 16.0.0, limits, contracts sorted by canonical bytes, extensions, predecessor, migrations) via `encode_record` per `ContractDescriptor`/`ExtensionDescriptor`/`MigrationContractDescriptor` (`lib.rs:228-322`); plus `SSMC1_EPOCH1_SCHEMA.txt` ASCII LF manifest (closed `record`/`union`/`enum`/`entity`/`type`/`term`/`op` rows, ascending tags; field_schema_hash `1983bc8d...33ae` = BLAKE3(manifest), decoder_limits_hash `389791b1...6136a` = BLAKE3(limits preimage `SSMC1.md` §3)); errors `SCHEMA_*` (`EPOCH_MISMATCH`, `DOWNGRADE`, `CONTRACT_UNKNOWN`, `MIGRATION_UNSUPPORTED`, `EQUIVALENCE_FAILED`, `SELF_MODIFICATION`, `ROOT_OVERWRITE_FORBIDDEN`, `RECORD_INVALID`); responsibility: schema framing + registry binding, no program semantics | STUB (trap; exact input shape Bytes vs manifest ASCII vs registry entry closes via scoped contract procedure; preserves old encodings/epochs; no parallel format, no derived-image substitution, no identity alteration) |
| 3 `encode_schema` | Inverse of leg 2: canonical schema record/manifest bytes from typed registry entries; same hashes, ordering, limits; `LIMIT` for exhaustion | STUB (trap; same procedure as leg 2) |

Distinctions (no silent normative semantics):

- Existing encoding needing documented binding (not new format):
  `EntityObject` outer framing (`object.rs:88-222`, `encode_object_record`/
  `decode_object_record` for fields 1..4, `EntityId` fixed 32B,
  `Union` tag+len+value, `Option` 0/empty vs 1/payload, `Set` canonical
  order) and `SchemaEpochRecordV1`/`ContractDescriptor` encodings
  (`sley-schema/lib.rs:228-322`) already exist natively; this slice
  documents their binding to legs 0-3 (object kind, epoch, fields,
  ordering, references, bytes, errors, boundary above) without altering
  identities, epochs, or bytes. No parallel format created.
- Implemented behavior conflicting with governing contract: none found.
  Slice-1 symmetric `encode_uvar` width rule remains the sole new
  codec-level contract (labeled, deterministic refusal, not parity).
- Genuinely unspecified: `codec_main` input shapes for schema legs
  (exact `Bytes` vs manifest ASCII vs registry entry) and full 18-kind
  body + label/NFC + fingerprint verifier details for program legs.
  Closed via existing scoped contract/review procedure (this record +
  scoped native review, no broad audit; incompatible semantic change
  would require its migration/authority process per SCB1 §11 / SSMC1 §9;
  old encodings/epochs preserved).

Selected next useful complete unit from the real dependency graph
(not contrived for size): SCB1 record framing engine, first
instantiation `FixtureEmptyObject` (payload `00`, 1B; stored 76B
envelope `534c...9586b`, tag 1, epoch `00*31 ++ 01`, domain
`sley2.object.v1`; `conformance/scb1/v1/accepted.json`
`standalone-empty-object`, `rejected.json` envelope-* + opaque 2).
Record framing (count/tags/lens/trailing/bool) is the leaf every
program entity needs (outer Record fields 1..4, `NamespaceBody` fields
1..2, `WorkspaceBody` 5 fields, etc.; `SSMC1_EPOCH1_SCHEMA.txt:6,26,28`).
Empty is the frozen fixture (not invented), minimal record (0 fields),
prerequisite for all records; program outer fixtures measured natively
(`sley-mutate build_entity_object`, epoch `09*32`, entity `01*32`, no
label/fingerprint): `ns-empty` (parent `None`, members empty) stored
123B / payload 47B / preimage 91B; `ns-one` (one member) 156B;
`const-bool` 128B; `param-bool` 160B; `ws-min` 162B; `pkg-min` 190B.
All <204B opaque sample (see §8) and <1 MiB bridge; outer 47B payload
reuses this exact framing plus `EntityId`/body loops (land next, no
repair by size + envelope bounds). No derived executable image
substituted for canonical program state; no object identities altered.

## 3. Contract decisions (explicit, reviewable)

- **Error-code preservation.** Refusals carry exact `SCB_*`/`SSMC_*`
  registry strings as `Bytes`. No mapping to 7-code program vocabulary.
  Uvar `Err` from `CallDirect` forwarded unchanged. Vocabulary mapping
  belongs to leg wiring.
- **Opaque scope for outer in-progress (labeled).** Outer
  `build_outer_decode` parks count==2 as `UNKNOWN` and 3/4-field as
  `SSMC_RESERVED_FIELD_PRESENT` (scope-first for 3/4: label needs NFC
  tables + S20-250 verifier, body 18 kinds need body slices; explicit
  divergence vs reference `Ok` for valid 3/4-field objects, pinned, never
  misreported as format success). No success path claimed for outer;
  `codec_main` legs stay stubs. Empty scope is complete (0 fields, no
  scope exclusion; `FIELD_UNKNOWN` for count>0 is format, not scope).
- **Capacity restriction (AR-05, same class as slices 1-2).** `B2V1`
  converts inputs up to 1 MiB bridge cap; larger refuse
  `SCB_RESOURCE_LIMIT` at conversion. Epoch allows 64 MiB
  (67,108,864 bytes) standalone (corrected from 67 MiB; `SCB1.md` §9,
  `lib.rs:18`). Sley-executed 204 stored bytes (128 opaque payload) is
  the demonstrated successful sample under `codec_limits` (not an exact
  maximum; larger may succeed with larger budgets or refuse on value
  units). Empty 1B payload (76B stored envelope) far inside with 50x
  headroom (see §8). No chunk scheme, substitute digest, domain change,
  silent budget raise, or streaming API.
- **InternalInvariant traps.** Checked-arithmetic overflow on indices
  below converted length, `Get None` after explicit remaining checks,
  32B digest conversion target `Trap(InternalInvariant)` with proof
  sketch per site (same standard as slices 1-2). Reachable resource
  edges (`B2V1`/`PSH1`/`V2B1` caps) return typed errors. Asymmetric
  defensive style intentional and recorded (see envelope §3). No
  malformed input reaches a trap.
- **Upfront `input.len > MAX` subsumed.** Reference checks
  `input.len > MAX_STANDALONE_BYTES` first (`lib.rs:431,533`,
  `object.rs:122`); every Sley-accepted input is below 1 MiB bridge cap
  (far below 64 MiB / 67,108,864 bytes), over-cap refuses
  resource-loudly. No dedicated Sley block; observation only, same
  resource code.

## 4. Substrate findings (from building; F1/F2 reused, F5 refined, no new workaround)

- **F1/F2 reused.** Shift amounts/widths `UInt32` (shift opcodes demand
  `u32`); no int conversion opcode, so reused `decode_uvar` keeps 8-bit
  ladder and empty/outer thread `UInt64` positions/values with `UInt32`
  widths. `ZigZag` still deferred (all framing reads unsigned; no
  envelope/outer/record impact).
- **F5 refined: `ValueUnits` are cumulative charges, not live usage.**
  `execute.rs:2279` `initial_value_units = input_units (1+len for `Bytes`)
  + types len + bytes len`; `2285-2399` `value_units_const` saturating
  semantic units per constant (e.g. `Bytes` 1+len, `Bool` 2+1+1,
  `Vector` 1+len+elements, `Record` 33+len+32/elem); `2030`
  `charge_value` only adds (`live += amount`, `peak = max(peak, live)`,
  never subtracts; no freeing); `1561-1584` `prepare_frame` charges
  call args; `1703` `execute_extended` charges full result +
  cell-contents; `1340`/`1369` charge call results. Peak is max
  cumulative, not live. Persistent-vector pushes (`PSH1` via
  `AdapterInvoke`, `extended.rs` bridge) charge full new-vector units
  each push, so `n` pushes accumulate `O(n^2)` (`envelope.md` §8:
  512B payload (588 stored) peak 998957 vs 1M cap while fuel ~20K,
  instr ~1.8K stay inside, proving value-units bind first). Empty
  decode/encode are loop-free (single uvar + comparisons for decode;
  `VectorNew` + 1×`PSH1` + `V2B1` for encode; no backedges), so peak
  is inputs + types + single values (18955/986), not pushes. Outer
  47B (32B `EntityId` + 10B body loops) will push 42B total, peak
  estimated 200-300K (<1M, 3-5x headroom; no repair by size + bounds).
- **OREF-1 retained and traced to envelope call paths (not irrelevant).**
  Eleven bare continuations (`80`×11, no terminator): Rust reference
  reports `INTEGER_OVERFLOW` (shift bound `lib.rs:1206-1230`, verified by
  direct probe); Python oracle reports `LENGTH_OVERFLOW` (no shift bound,
  `codec.py:61-78`). No frozen vector covers this input. Reused Sley
  `decode_uvar` mirrors reference (normative `sley-scb1`, S20-120) over
  oracle on shift-bound inputs; oracle file excludes this case. Trace to
  envelope: `validate_envelope` calls `decode_uvar` via `CallDirect`
  for version w64 (offset 8), tag w32, len w64 (`envelope.rs: v_call`,
  tag call, `l_call`); truncated `magic ++ 80*11` (19B) hits version
  w64 with no terminator, so Sley/reference `OVERFLOW` vs oracle
  `LENGTH_OVERFLOW` (same divergence, preserved, not claimed
  irrelevant). Envelope widths 64/32 are oracle-supported for terminated
  values; the divergence needs truncation (no terminator), explicitly
  preserved, never edited to green. Does not affect empty valid tags
  (count 0 single-byte `00`, terminated).
- **OREF-2 qualified: envelope tag uses `UInt32`, but gap is generic
  runner, not envelope runner.** Oracle `decode_declared_value` does not
  implement `UInt16`/`UInt32` rejected fixtures (2 of 8 fresh uvar
  rejected report unsupported-path; Sley covered vs Rust reference).
  Oracle envelope runner (`decode_standalone`, `codec.py:260`) DOES use
  `decode_uvar(cursor, 32)` for `contract_tag` (small values 1,2,3
  single-byte, all supported; frozen 23/26 PASS includes
  `envelope-contract-unknown` tag 3). Tag overflow (e.g. `8080808010`
  =2^32 at w32) refuses `OVERFLOW` on Sley/reference/oracle-envelope
  alike (width check, not unsupported). OREF-2 does not affect envelope
  valid tags (1,2) or empty count (w64 `00`). Protected-fixture
  compliance: no `conformance/` file added/removed/edited.

## 5. Residuals disposition

- AR-02 (streaming hash): not exercised. Empty needs no hash; outer
  preimage/body (47B) needs single-shot `RHW1` only when program
  envelope (tag 200) lands (all in-tree preimages ≤1 KiB for fixtures;
  47B outer + 15B domain far inside 1 MiB `RHW1` ceiling). No chunk
  scheme, substitute digest, domain change.
- AR-05 (capacity): measured §8. Format-valid to 64 MiB
  (67,108,864 bytes) per epoch; bridge/`RHW1`-typed to 1 MiB;
  Sley-executed empty 1B (76B stored envelope) far inside (peak 19K),
  envelope 204B sample peak 741K (<1M), 588/1100/1100077 resource rows
  preserved (see §8). Per-byte cost for loop-free record framing is
  single-uvar + comparisons (decode 285 fuel for 1B vs envelope 124
  fuel/byte with pushes; encode 20 fuel for 1B). Outer 47B loops add
  `O(n^2)` pushes but small (`n=42`, est. 200-300K peak, <1M).
- AR-06 (readiness vs evidence): no technical condition blocks this
  slice. Remaining work is evidence/acceptance (independent review,
  premium re-review, S20-780 separation), stated in §10.

## 6. Coverage by requirement (not just counts)

| Requirement | Evidence |
|---|---|
| Canonical accepted bytes, frozen envelope | `standalone-empty-object` (76B, payload `00`) Sley envelope `Ok` + reference `Ok` (slice 2, retained 9/9 green); empty payload `00` Sley `Ok(Unit)` + `ref_empty Ok` + oracle `_decode_empty_object(00) Ok` (direct probe, frozen envelope covers payload) |
| Distinct valid payloads + declared binding | Empty `00` only (0 fields); reqbool `01010101`/`01010100` + outer `ns-empty` 47B payload / 123B stored + `ns-one` 156B etc. measured natively (see §2), Sley envelope opaque `Ok` for 76/79B fixture envelopes (slice 2); program envelope tag 200 lands next (legs stay stubs, no invented framing) |
| Empty strict rejections | `""`/`80` `LENGTH_OVERFLOW`; `01`/`02`/`01010101` `FIELD_UNKNOWN`; `0000` `TRAILING`; `8000`/`8100` `NON_MINIMAL`; all code-for-code vs `ref_empty` (8 cases) |
| Canonical emission, byte-exact (round trip insufficient) | `encode_empty(Unit)` → `00` byte-exact vs `encode_uvar(0)`; `decode(encode)==Ok` plus independent byte comparison (not round trip alone) |
| Post-image inputs | 100 LCG mutations (seed `0xE11E_0003`, flip/truncate/append/prepend) drawn after admission; code-for-code vs `ref_empty` (no digests, no scope divergence on this stream; guard asserts no `FIELD_*` beyond `UNKNOWN`/`TRAILING`/`LENGTH`/`NONMINIMAL` here) |
| Independent oracle | Frozen 23/26 PASS via `sley2-scb1-oracle check` (retained, no edit); empty `00` covered by frozen `standalone-empty-object` payload; malformed bare payloads (`01`/`02` → `FIELD_UNKNOWN`, `0000` → `TRAILING`) verified by direct Python probe `_decode_empty_object` (agrees; see §4 OREF handling); fresh 2+11 envelope oracle CLI PASS retained (slice 2, not duplicated) |
| Opaque-scope pin (slice 2, retained) | 2 payload-in-envelope vectors assert Sley `Ok` + reference `FIELD_MISSING`/`FIELD_UNKNOWN`; `Ok` never implies program/schema validity |
| 20+8 corpus (slice 1, retained) | `rw080_codec_uvar.rs` 9/9 green (not duplicated; OREF-1/2 carried without omission, see §4) |
| Resource boundaries | Empty decode 1B + encode Unit→1B far inside (see §8); envelope 588/1100/1100077 resource rows preserved (see §8); over-bridge 1.1 MB refuses resource-loudly (typed or execution, never misread) |
| Three non-PASS envelope rows (explicit) | 1. `record-required-field-missing` (76B, tag 2, payload `00`): Sley opaque `Ok(00)` vs reference/oracle `FIELD_MISSING` (pinned divergence, scope: envelope valid, record invalid; `Ok` never implies program validity). 2. `record-unknown-field` (79B, tag 1, payload `01010101`): Sley opaque `Ok(01010101)` vs reference/oracle `FIELD_UNKNOWN` (same scope pin). 3. OREF-1 envelope-reachable edge (`magic ++ 80*11`, 19B truncated version w64, no terminator): Sley/reference `INTEGER_OVERFLOW` vs oracle `LENGTH_OVERFLOW` (preserved divergence, traced to `validate_envelope` version/tag/len `CallDirect` w64/32 paths; not in frozen, never edited to green). No frozen expectation edited. |
| Three resource non-success rows (explicit, capacity, not format) | 1. 588 stored (512 opaque payload): execution `ResourceLimit(ValueUnits)` peak 998957, fuel ~19909, instr ~1778 (value-units bind first; fuel/instr far inside). 2. 1100 stored (1024 payload): execution `ResourceLimit(ValueUnits)` peak 998349, fuel ~21493, instr ~1738 (same). 3. 1100077 stored (1.1 MB payload, >1 MiB bridge): execution `ResourceLimit(ValueUnits)` at input validation under `codec_limits` (fuel 0, instr 0, peak 1204790); under larger execution limits same bytes reach `B2V1` and return typed `SCB_RESOURCE_LIMIT`. Both preserve resource semantics (reported, never misread as format success). Original matched-budget results kept; supplementary empty/program-native runs labeled separately (see §8), never replacing baseline or weakening acceptance. |

Test functions (new, 4): valid+encode bytes, 8 rejections
code-for-code, 100 runtime mutations code-for-code, resources far
inside. Runtime suites compare Sley against in-test Rust reference per
input; oracle files + direct probes give second opinion. Outer
(`build_outer_decode`, 2-field count + field-1 chain, parked as
`UNKNOWN`, no success) and reqbool (`build_fixture_reqbool_decode`,
count + tag + len + bounds, parked) present as `dead_code`
in-progress framing (explicit scope, not claimed complete; lands next
with loops + field-2 + return + `encode_outer`); no test admits them
yet, so no success/failure claimed for those paths.

### 20+8 corpus accounting (slice-1 vectors, individually, reused)

Retained verbatim from `rw-080-codec-uvar.md` §6 (no duplication, no
omission, still green in `rw080_codec_uvar.rs` 9/9; OREF-1 11-cont
`OVERFLOW` vs oracle `LENGTH_OVERFLOW` + OREF-2 `UInt16`/`UInt32`
unsupported-oracle-path pinned there). No vector omitted; no
unsupported case folded into unqualified `PASS`.

## 7. Authoring slips caught by triangulation (honest record)

No hand-written fresh literal was wrong on first writing in this slice:
empty vectors are single-byte (`00` valid, `01`/`02`/`80`/`8000`/`8100`
edges, `0000` trailing) deterministic with no hand-encoding; reference
cross-check fired on first run for all 8 rejections (no expectation
rewritten to match candidate). One Sley-construction slip caught before
admission: `IntSubChecked` `Ok` payload is `u64` difference directly
(not `Tuple`; envelope `IntAddChecked` precedent `u1_sum` is `u64`);
fixed by correcting `el_unwrap` param type from `Tuple` to `u64`
before any test ran (no expectation rewritten). Outer field-1 chain
`cond` edges initially used `SwitchArgument` (`sav`) where `ValueRef`
(`pav`/`op_result`) required (`edge` takes `ValueRef`, `switch` cases
take `SwitchArgument`); caught by Rust type errors before lowering,
fixed per call site (no test expectation involved).

## 8. Resource observations (measured, `EMPTY_FUEL` + retained `ENVELOPE_*`)

Under `codec_limits` (100K instructions / 1M fuel / 1M value units /
100K output units):

- New (this slice, loop-free record framing, supplementary, labeled
  separately; does not replace envelope baseline or weaken acceptance):
  - Decode empty payload 1B (`00`): fuel 285, instr 70, peak value
    units 18955. Far inside all budgets; 50x headroom vs 1M value
    units, 5x vs envelope 76B peak 169067, 40x vs 204B peak 741117.
  - Encode `Unit` → 1B (`00`): fuel 20, instr 5, peak 986. Negligible;
    1000x headroom. No `O(n^2)` pushes (1×`PSH1`, no backedges).
  - Framing overhead included: `B2V1` conversion, one `CallDirect`
    uvar decode (count w64), `65535`/`0` comparisons, trailing
    equality, `ResultOk(Unit)` (decode) / `VectorNew` + 1×`PSH1` +
    `V2B1` (encode). Intermediate allocations counted in peak.
- Retained baseline (slice 2, matched budget, unchanged):
  - Success: stored 75B → 7031/863/167315; 76B → 7179/878/169067;
    79B → 7545/911/174275; 107B (32 payload) → 10961/1219/241979;
    204B (128 payload, demonstrated sample, not maximum) →
    23001/2330/741117. All inside fuel/instruction; value units bind
    first.
  - Execution `ResourceLimit(ValueUnits)`: 588 stored (512 payload,
    peak 998957) and 1100 stored (1024 payload, peak 998349); fuel
    (~20K) and instructions (~1.8K) far inside, proving persistent-vector
    value units bind first, not fuel. Reported, never misread.
  - Over-cap 1,100,077 stored (>1 MiB bridge): execution
    `ResourceLimit(ValueUnits)` at input validation under
    `codec_limits` (fuel 0, instr 0, peak 1204790); under larger
    execution limits same bytes reach `B2V1` and return typed
    `SCB_RESOURCE_LIMIT`. Both preserve resource semantics.
- Real program consumer (native, supplementary, labeled separately;
  Sley program envelope tag 200 lands next, no Sley execution claimed
  yet): `ns-empty` stored 123B / payload 47B / preimage 91B; `ns-one`
  156B; `const-bool` 128B; `param-bool` 160B; `ws-min` 162B; `pkg-min`
  190B (all via `sley-mutate build_entity_object`, epoch `09*32`,
  entity `01*32`, no label/fingerprint). All <204B opaque sample and
  <1 MiB bridge. Outer 47B payload reuses this record framing (count/
  tags/lens/trailing, single-byte uvars for small) plus 32B `EntityId`
  + 10B body loops (est. 200-300K peak, <1M, 3-5x headroom by size +
  envelope `O(n^2)` bounds + empty loop-free baseline; no repair).
  Larger compiler preimages stay refused loudly (typed or execution
  resource), never truncated, re-hashed, or chunked.
- Unmeasured quantities (explicit): available memory and
  native-operation counts have no separate instrument; fuel/
  instructions/peak-value-units are bounded-execution evidence. No
  streaming API introduced.

## 9. Remaining stubs and next dependency

Stubs: all four `codec_main` legs (pending full program/schema with 18
body kinds, label/NFC, fingerprint verifier, schema record/manifest
shapes; owning requirements RW-080 §1.1 legs 0-3); outer 2-field
success path (entity-ID 32B loop + field-2 tag/len/bounds + body loop +
trailing + `Tuple` return + `encode_outer`; in-progress code present,
parked, explicitly not claimed); reqbool bool-byte + trailing + `Ok`
+ count==2 second-field (in-progress, parked); `ZigZag` both directions
(pending int-conversion design); fixture-record semantic phase beyond
empty (`FIELD_*` for reqbool/outer bodies after digest: next slice with
program binding, see §2); Text/NFC/floats/maps/records (need unlanded
capabilities).

Next bounded dependency: outer 2-field completion (loops + field-2 +
`Tuple` return + `encode_outer` emission via `encode_uvar` for `len`
+ program envelope tag 200 via `CallDirect` envelope with program
tags/epoch) reusing this record engine plus digest ratio. Reuses
existing scope finding; no broad audit launched.

## 10. Review provenance and acceptance debt

Lint triage (repo zero-warning standard restored): `cargo fmt --check`
clean; `clippy -- -D warnings` clean (new builders carry
function-level allows for `many_single_char_names`/`similar_names`/
`too_many_lines` with repo precedent `bootstrap_closure.rs`/
`rw075_raw_callable.rs` since renaming green proof-code slots would
churn positional layout; in-progress outer/reqbool carry `dead_code`
with rationale (lands next, retained to avoid churn, not claimed
complete); `uvar` `build_exact`/`build_encode` retained as `dead_code`
verbatim from slice 1 for provenance/churn avoidance; `doc_markdown`
backticks via `clippy --fix`; casts use `try_from` with documented
bounds; hex helper removed (unused in empty scope; re-add when
reqbool/outer mutations need hex logging); `git diff --check` clean.

Author: same session/model as the candidate (self-review class under
the standing amendment; supports provisional development only, never
independent acceptance).

Second native opinion (separate session, read-only, no edits):
pending at record write time; one scoped read-only handoff attempt via
the available Council route (`vulcan`, read-only, no gateway/premium
probe, 300s budget) returned API rate-limited with no verdict (availability
failure, not a technical finding; no resubmit to route around, no model
switch to loosen validation, no gateway probe). To be obtained via the
available native reviewer under the standing amendment when capacity
allows (labeled with real model/session; self-review supports
provisional development only, never independent acceptance). Scope: empty
builders + tests, in-progress outer/reqbool parked scope, binding §2,
capacity §8 with `ValueUnits` cumulative + `O(n^2)` pushes + empty 19K/986
vs 1M + 76B 169K vs 1M + 204B 741K vs 1M + 588/1100/1100077 resource rows,
opaque 2 + OREF-1 edge 3 non-PASS rows, no frozen edit, 64 MiB correction,
204B sample qualification. No verdict claimed here; no gate input. Independent acceptance debt unchanged and
still required: Nabu round-12 review, premium round-2
`R2_ARCHITECTURE_PASS`, S20-780 independent acceptance. Retained premium
FAIL and `R2_EXIT: NOT_READY` preserved; S20-780 local audit (`83fe94a`)
stays separate (performed locally, independent review pending; GA
clean-room claim stays gated). No verdict files manufactured; no gate
edits; no promotion to runtime authority. No refused-gateway probe was
made and none is claimed.

Prior-nit confirmation (narrow, no gateway): slice-2 four prior nits
(manifest width-pair prose; two stale `UInt64` comments; encode-bridge
trap prose; exact-wrapper `pos` generality) repaired before 94a2a57
and re-confirmed repaired in reused builder; envelope four new nits
(preimage-bound overstatement, identical magic branches → repaired;
asymmetric `Get-None`, dead-code bloat → accepted variances with
rationale) re-checked and still repaired/accepted in current file.
Re-confirmation through refused gateway/premium route neither attempted
nor waited for; provisional status preserved as stated.

## 11. Identities

- Source: `crates/sley-vm/tests/rw080_codec_program_outer.rs` (this
  record's seed-assembler artifact; 4 empty tests green; outer/reqbool
  in-progress parked, no tests admit them yet).
- Graphs/images: admitted per-test at runtime (no checked-in image);
  profile `BOOTSTRAP_PROFILE_2
  (fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459)`;
  entries `decode_empty`, `encode_empty` (complete); callees
  `decode_uvar` (reused slice-1 verbatim); outer/reqbool entries
  present but unadmitted (in-progress, no image).
- Dependencies: `HOST_ABI_V2` imports `B2V1` (empty decode),
  `PSH1`/`V2B1` (empty encode) only
  (`bc564653302a73eb5f998427250a2bb7cd87f5685ef12619bd4ae1f1b2af70d5`);
  `EXEC_PACKAGE_V2` envelope via C0 seed route
  (`f4958c5e3d57762173b881288b008af17d45b5f07a431fcc442d9eec5770da94`);
  `RAW_BLAKE3_V1` (`785205fb...69f72`) unused here (envelope slice
  uses it; outer program envelope will when tag 200 lands); reference
  `sley-scb1` (test-oracle for fixtures: `decode_payload_exact`,
  `encode_uvar`) + `sley-mutate/object.rs` (native oracle for program
  outer: `build/import_entity_object` sizes 123-190B stored, 47B+ payloads);
  independent oracle `sley2-scb1-oracle` (second opinion: frozen 23/26
  PASS, empty `00` via frozen envelope payload, malformed bare payloads
  via direct `_decode_empty_object` probe, OREF-1/2 preserved).
- Prior checkpoint: 0470915
  (`0470915448016e2d049f9776d83cd095f8c84f3c`, clean worktree at
  start; no reset; history preserved; 64 MiB evidence reconciliation
  lands as separate commit before this slice, see §1 corrections).
- Validation tier: Tier 1 / targeted. Affected subsystems: `sley-vm`
  empty/uvar/scaffold/raw-callable tests, `sley-scb1` conformance,
  independent oracle, `sley-mutate` object sizes (native, no prod
  change). Checks: 4 empty tests, 9 uvar regression, 9 envelope
  regression, 2 scaffold, 16 raw-callable (reported as 18+2 across two
  binaries), 4 `sley-scb1`, oracle frozen CLI, `fmt`, `clippy`
  `-D warnings`, `git diff --check`. Result: all passed. Full gate:
  not run — not required for this development iteration (Tier 1 per
  tiered-validation policy). Known `make quick` baseline failure
  retained without re-running full suite: carried
  reproducibility-report staleness (attestation `84bfa9c9…`,
  3-surface class, release-lane owned; `rw-050-slice-1.md:165`,
  `rw-030-charter.md:180`); no full-suite `PASS` claimed and no
  unrelated baseline comparison rerun.
