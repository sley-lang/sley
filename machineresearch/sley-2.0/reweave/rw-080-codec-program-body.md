# RW-080 §1.1 program slice 4: outer 2-field completion + EntryPoint kind 16 in Sley — provisional C0 construction record

Status: PROVISIONAL (2026-09-07, operator development override
`rw075_correction.operator_override_2026_09_07`). Three Sley functions with
real strict codec algorithms through the v2 admit/approve/execute boundary
(`decode_outer`, `encode_outer`, `decode_entrypoint`), plus native program
envelope reference for stored objects. Not accepted runtime authority;
`rw080` stays BLOCKED and the R2 gate stays NOT_READY pending independent
Nabu round-12 review plus premium round 2 plus S20-780 independent acceptance.
Prior slices retained unchanged (`rw-080-codec-uvar.md`,
`rw-080-codec-envelope.md`, `rw-080-codec-program-outer.md`); this record
covers only the slice-4 delta. No amendment duplication, no reconciled
settlement reopened, no production/gate/ledger-status change.

## 0. Scope (no new planning campaign)

Reuse of slices 1-3 reconciliation (no new campaign, no alias beyond those
notes): RW-080 construction stage and contract (`rw-080-contract.md`);
RW-090 codec parity gate; RW-100 semantic gate only (never imported here).

## 1. What was built (seed-authored Sley, this file is not the assembler;
source is `crates/sley-vm/tests/rw080_codec_program_outer.rs`)

- `decode_outer(payload: Bytes, unit: Unit) -> Result<Tuple<Bytes, Bytes>, Bytes>`
  exact 2-field `decode_object_record` opaque path (`sley-mutate/object.rs:177`):
  count via `CallDirect decode_uvar` w64 (`count > 65535` RES), `count < 2`
  MISSING, `count > 4` UNKNOWN, count 3/4 scope-first
  `SSMC_RESERVED_FIELD_PRESENT` (pinned divergence vs native Ok, never
  misreported as format success); field-1 tag w32 / len w64 / bounds
  (`pos+len <= ilen` else LENGTH, `len > MAX` RES) with match-after-read
  dispatch (tags 3/4 scope; tag 1 entity path with `len < 32` LENGTH /
  `len > 32` TRAILING mirror of `decode_fixed`, 32B PSH1 loop Form A;
  tag 2 body-first parks to prev=2 field-2 chain; tag 0/5+ UNKNOWN after
  bounds); field-2 tag w32 with DUP (`==prev`) / ORDER (`<prev`) before
  payload per reference, scope-first for 3/4, len w64 + bounds before UNKNOWN;
  opaque body PSH1 loop + V2B1; trailing (`end == ilen`) else TRAILING;
  returns `Tuple(entity_id 32B, body opaque)`. Uvar `Err` forwarded unchanged.
  Bridge uses B2V1/PSH1/V2B1 only. No loops beyond bounded byte copies.
- `encode_outer(entity_id: Bytes, body: Bytes, unit: Unit) -> Result<Bytes, Bytes>`
  canonical `encode_record([(1, entity_id), (2, body)])`: entity B2V1 +
  `len < 32` LENGTH / `> 32` TRAILING encode-side mirror (typed EntityId is
  always 32; this validates untyped Bytes callers); body B2V1 + `len > MAX`
  RES; body length via canonical `encode_uvar` (`CallDirect`, width 64) so
  multi-byte lengths stay canonical; fixed pushes 02/01/20/02 + 32B entity
  copy + length-bytes copy + body copy loops + V2B1. `LIMIT` for exhaustion,
  never truncation.
- `decode_entrypoint(body: Bytes, unit: Unit)`
  `-> Result<Tuple<Bytes, UInt64>, Bytes>` for closed kind 16
  (`SSMC1_EPOCH1_SCHEMA.txt:9,25`; `NamespaceBody` NOT implemented here, see
  §2 choice): union tag w32 (16 valid; 1..15,17,18 known-but-unimplemented
  `SSMC_RESERVED_FIELD_PRESENT` scope, 0/19+ `SCB_UNION_INVALID` matching
  reference `UnionInvalid` for unknown tags); union len w64 + bounds +
  whole-body consumption (union `end == ilen` else TRAILING); record count
  exactly 2 (`>65535` RES, `<2` MISSING, `>2` UNKNOWN); field 1 tag 1 else
  ORDER (tag 2, expected 1 first) / UNKNOWN; field 1 len + bounds within
  union end + fixed 32B (`<32` LENGTH / `>32` TRAILING) + 32B copy loop;
  field 2 DUP (`==1`) / ORDER (`<1`, i.e. 0) before payload, `==2` valid else
  UNKNOWN; field 2 payload copy + `decode_uvar` w32 at 0 on extracted bytes
  (preserves NON_MINIMAL/OVERFLOW) with exact-consumption trailing;
  exposure 1/2 else UNION_INVALID; record trailing (`end2 == union_end`);
  returns `Tuple(function 32B, exposure 1/2)` — interpreted values, not an
  opaque copy. Bridge uses B2V1/PSH1/V2B1 only.
- `build_entrypoint_encode` exists as `dead_code` parked after exposure
  validation + function B2V1 (honest no-success halt, never admitted, never
  claimed; see §9). `codec_main` legs 0-3 stay stubs
  (`rw080_codec_scaffold.rs`); no outer-only parser is wired behind a
  full-program-validity success.

Images: outer-decode (entry outer + callee decode_uvar), outer-encode (entry
outer-encode + callee encode_uvar), entrypoint-decode (entry + callee
decode_uvar). Each admitted under `BOOTSTRAP_PROFILE_2` through
`admit_v2_package` (C0 seed route, declared). Entity namespaces are two-byte
`(namespace, index)` ids under disjoint `Ns` per image (decode 71-74 +
outer 75-78; encode 81-84 + outer-encode 85-88; entry-decode 91-94 +
entrypoint 95-98), deterministic and collision-free by construction.

## 2. Body-kind choice (recorded briefly)

Preferred candidate was Namespace (already-measured ns-empty/one/two
fixtures, empty + nonempty). Chosen kind is EntryPoint 16 because it provides
the shorter useful complete path: fixed 32B function + 1/2 enum, fixed 40B
body (`1026...`), no `Option` union nesting and no `Set` ordering/duplicate
proofs. Namespace fixtures are retained as outer opaque varying inputs
(ns-empty 47B / ns-one 80B / ns-two 113B payloads through Sley outer decode +
encode), so varying body contents are exercised at the outer layer while the
interpreted kind is EntryPoint (local/protocol, varying function IDs).
Namespace Sley decoding/encoding remains future work, not redefined as
invalid (scope, see §9).

## 3. Program context (tag/epoch distinction resolved, bound explicitly)

- Fixture validator (slice 2) admits tags 1..2 and fixture epoch
  `00*31 ++ 01` (`sley-scb1` `EPOCH_ID`); it is preserved unchanged.
- Program binding reports contract tag 200 (`sley-mutate/object.rs:21`,
  `SSMC1_EPOCH1_SCHEMA.txt:4-5` `contract 200`, `digest_domain 3
  sley2.object.v1`, `kind 200 SemanticEntity`) and test epoch `[9;32]`
  (object.rs tests; probe fixtures in §8). Sley outer/entrypoint builders
  take no caller-declared tag/epoch (no `declared` parameter anywhere in
  this file); tag checks are fixed constants (outer has no tag — it parses
  payload records; entrypoint pins union 16; envelope tag 200 belongs to the
  envelope layer, next). Epoch `[9;32]` in this slice is an explicit native
  test bound for stored objects (see §8 envelope-native test), not registry
  acceptance and not a production epoch; registered/accepted production
  epochs remain a registry obligation outside this codec. No tag/epoch check
  was removed and no caller declaration is accepted.

## 4. Contract decisions (explicit, reviewable)

- Error-code preservation: refusals carry exact `SCB_*` strings
  (`FIELD_MISSING/UNKNOWN/DUPLICATE/ORDER`, `LENGTH_OVERFLOW`,
  `TRAILING_BYTES`, `VARINT_NON_MINIMAL`, `INTEGER_OVERFLOW`,
  `RESOURCE_LIMIT`, `UNION_INVALID`, `MAGIC_INVALID` etc. natively) and
  `SSMC_RESERVED_FIELD_PRESENT` for scope exclusions. No mapping to the
  7-code program vocabulary. Uvar `Err` forwarded unchanged.
- Opaque scope for outer (labeled): outer `Ok(entity, body)` means outer
  framing valid with exact trailing; it does NOT mean the body is a valid
  program object. Malformed bodies with valid outer framing return outer `Ok`
  (pinned, see §6 mutations opaque=8); body validity belongs to the body
  slice (entrypoint here) and RW-100, never inferred from outer success.
- Scope-first for 3/4-field and tags 3/4 and non-EntryPoint union tags
  (1..15,17,18): `SSMC_RESERVED_FIELD_PRESENT`, pinned divergence vs native
  `Ok`/body-code. Valid-but-unimplemented stays an implementation limitation,
  never redefined as invalid canonical input and never reported as format
  success.
- Capacity restriction (AR-05 class, same as slices 1-3): B2V1 converts
  inputs up to the 1 MiB bridge cap; larger refuses `SCB_RESOURCE_LIMIT` at
  conversion. Epoch allows 64 MiB (67,108,864 bytes) standalone; Sley
  executes what fits the admitted bridge/profile (all fixtures <200B stored,
  far inside). No chunk scheme, substitute digest, domain change, silent
  budget raise, or streaming API.
- `InternalInvariant` traps: checked-arithmetic overflow on indices below
  converted length, `Get None` after explicit remaining checks, 32B digest
  conversion target, small B2V1 conversions (e.g. ≤10B length bytes). All per
  site with the same standard as slices 1-3. Reachable resource edges
  (B2V1/PSH1/V2B1 caps) return typed errors. No malformed input reaches a trap
  (proven by 18+12 rejection cases + 100 outer mutations + 100 retained empty
  mutations).

## 5. Substrate findings (no new workaround; F1/F2/F5 retained)

F1/F2 (u32 shifts/widths, 8-bit ladder, deferred ZigZag) and F5 (`ValueUnits`
cumulative charges with `O(n^2)` persistent-vector pushes) retained verbatim
from slices 1-3. New evidence: outer 47B→113B fuel 4875→10181 and peak
67216→148500 (push-dominated, value-units bind first; fuel/instr far inside);
encode 47B 2848/323/51234 (cheaper than decode, one uvar call + pushes);
entrypoint 40B 5354/827/69727 (uvar calls + fixed 32B loop + 1B payload loop).
The 200-300K outer projection from slice 3 is replaced by these measurements
(see §8). OREF-1/2 carried without omission (uvar 9/9 green, envelope 9/9
green, oracle frozen 23/26 retained, no frozen edit; OREF-1 11-cont edge and
OREF-2 UInt16/32 gap remain pinned in slice-1/2 records and are unaffected
here since all executed uvars are terminated single-byte widths 32/64).

## 6. Coverage by requirement (not just counts)

| Requirement | Evidence |
|---|---|
| Canonical accepted bytes, program binding | EntryPoint stored 153B / payload 77B / body 40B (`ep-local` func `0a*32` exp Local, `ep-proto` func `0b*32` exp Protocol) via native `build_entity_object` (epoch `09*32`, entity `01*32`); Sley outer decodes payloads to `(eid, body)` byte-exact; Sley entrypoint decodes bodies to `(func, 1/2)` matching independent native records; Sley outer encodes `(eid, body)` to canonical payload bytes byte-exact (not roundtrip alone; independent byte comparison vs native payloads for 4 cases: ns-empty/one + ep-local/proto) |
| Distinct valid payloads + varying inputs | Outer: ns-empty 47B, ep-local/proto 77B, ns-one 80B, ns-two 113B (Namespace empty/nonempty + EntryPoint kinds, varying EntityIds via func bytes `0a`/`0b` and Namespace members 0/1/2). Entrypoint: local vs protocol (exposure 1 vs 2, func `0a` vs `0b`) with input-dependent structured results |
| Strict rejections + precedence | Outer 18 cases: count 0/1 MISSING, count 5 UNKNOWN, count-3 SCOPE, [1,1] DUP, [1,0] ORDER, [1,5] UNKNOWN, [2,2] DUP, [2,1]/[2,0] ORDER, [2,3] SCOPE, 31B LENGTH, 33B TRAILING, truncated LENGTH, trailing TRAILING, nonminimal NON_MINIMAL, tag-overflow INTEGER_OVERFLOW, [1,1]-DUP-before-truncation precedence. Entrypoint 12 cases: ns-union SCOPE, tag 0/19 UNION, count 1 MISSING, count 3 UNKNOWN, [2,1] ORDER, [1,1] DUP, [1,3] UNKNOWN, 31B func LENGTH, exp 3 UNION, trailing TRAILING, truncated LENGTH. All code-for-code vs hand-derived reference order (object.rs + codec.rs); 3 entrypoint hex slips caught by triangulation (union-length mistakes returning TRAILING instead of expected; hexes corrected, codes unchanged, no expectation rewritten) |
| Canonical emission from structured values | Sley outer encode from `(eid Bytes, body Bytes)` matches canonical payload bytes for 4 cases (independent byte comparison). Sley entrypoint encode from `(func, exposure)` is parked (no success claimed; §9). Full structured-to-stored (func/exp → stored) remains next (needs entrypoint encode + program envelope in Sley) |
| Post-image inputs | 100 LCG outer mutations (seed `0xE11E_0007`, flip/truncate/append/prepend) drawn after admission, each with recomputed digest (preimage + `ObjectId::derive`) so digest never conceals payload checks; Sley outer vs native `import_entity_object` per policy: agree 92 (same outer code), opaque divergence 8 (Sley `Ok`, native body-layer `Err`), scope pins 0. Retained empty 100-mutation suite still green |
| Independent oracle/reference | Native `sley-mutate` build/import (tag 200, epoch `09*32`, domain `sley2.object.v1`) as primary reference for valid + envelope rejections (epoch/digest); hand-derived `object.rs`/`codec.rs` order for malformed codes; `sley-scb1` primitives (`encode_uvar`, `encode_record` framing) for canonical bytes; oracle frozen 23/26 retained (not re-run here, no frozen edit). Reference comparison vs independently derived expectations vs reviewer judgments kept separate; OREF findings preserved in slices 1-2 |
| Envelope/digest agreement | Natively: stored 153B imports `Ok` with full digest agreement; wrong epoch → `SCB_EPOCH_MISMATCH`; tampered digest → `SCB_DIGEST_MISMATCH`. Sley envelope validation is NOT claimed (no Sley envelope image admits tag 200 yet; §9). Mutations use recomputed digests precisely so payload checks are isolated |
| Input-dependent behavior | Same admitted images return different structured results for different inputs: outer `(eid, body)` varies across ns-empty/one/two + ep-local/proto; entrypoint `(func, exp)` is `([0a;32],1)` vs `([0b;32],2)`; rejections vary by fault (18 + 12 distinct codes). Opaque copying alone could not produce interpreted `(func, exp)` values |

Test inventory (this file, 13 tests): 4 retained empty (valid+encode, 8
rejections, 100 mutations, resources) + 9 new (outer probe 3 payloads, outer
encode roundtrip, outer re-encode 4 cases, outer 18 rejections, entrypoint
probe 2 bodies, entrypoint 12 rejections, native envelope reference,
outer 100 mutations with recomputed digests, outer+entrypoint resources).
Runtime suites compare Sley against native per input; no test decodes the
object for Sley, chooses its semantic result, or performs the encoder's work.

## 7. Authoring slips caught by triangulation (honest record)

Three entrypoint hex-construction slips (union-length bytes wrong: count1
`1004` → `1023`, count3 `1028` → `1029`, order21 field layout, dup11 union
length `1026` → `1045`, func31 payload length): each fired as TRAILING vs the
expected ORDER/DUP/MISSING/UNKNOWN/LENGTH on first run; hexes corrected to
the reference layout, expected codes unchanged, no expectation rewritten to
match the candidate. One outer-encode shadowing slip (`p_in`/`p_unit` block
params shadowing function params → `GraphOwnerMismatch`) caught at lowering
before admission; renamed to `bp_*`. One outer-encode threading slip
(dropped source vec in 32B copy loop → `TargetArguments`) caught at lowering;
re-threaded. One entrypoint ordinal slip (`f1_tag_ok` params vec order vs
creation order → `GraphOrdinalMismatch`) caught at lowering; vec reordered.
No test expectation was rewritten to match candidate output at any point.

## 8. Resource observations (measured; projection replaced)

Under `codec_limits` (100K instructions / 1M fuel / 1M value units / 100K
output units); decode and encode budgets distinct per invocation (no counter
resets between helpers; per-stage Sley invocations with explicit
two-invocation boundary for outer+entrypoint — no single composed Sley
invocation claimed):

- Outer decode: 47B 4875/677/67216; 77B (ep) 7275/857/92236; 80B 7567/883/96035;
  113B 10181/1077/148500. Cost grows with body length (push-dominated body
  copy loop); value-units bind first.
- Outer encode (eid+body → payload): 47B 2848/323/51234; 80B (43B body)
  4406 peak 102064. Cheaper than decode (one `encode_uvar` call + pushes).
- Entrypoint decode (body → func/exp): 40B 5354/827/69727 both variants
  (fixed shape; input-dependent values, size-independent cost here).
- Empty retained: decode 285/70/18955; encode 20/5/986.
- Native envelope reference (not Sley cost): stored 153B / payload 77B /
  preimage 121B for EntryPoint objects; import agrees with full digest.
- Slice-3 200-300K outer projection: REPLACED by the measurements above.
  No general optimization pass (consumer fits: all peaks <150K vs 1M, 7-20x
  headroom; fuel/instr <11K vs budgets). No protected-limit change, no value
  charging change, no hash preimage change, no native semantic codec, no
  streaming design.

## 9. Remaining stubs and next dependency

Stubs (explicit, none claimed): `build_entrypoint_encode` parked after
exposure check + function B2V1 (fixed emission lands next; `dead_code`, never
admitted); program envelope in Sley (tag-200/epoch-`09*32`/domain/RHW1
validate + build; native reference only in §6 test, no Sley envelope image
yet); composed `decode_program`/`encode_program` Sley entries wiring
envelope→outer→entrypoint via `CallDirect` (needs the two stubs above);
`codec_main` legs 0-3 (full 18-kind body + label/NFC + fingerprint verifier +
schema legs; owning requirements RW-080 §1.1 legs 0-3); Namespace Sley
codec (Option + Set ordering; retained as outer opaque inputs only);
reqbool/outer in-progress remnants (`build_fixture_reqbool_decode` parked,
`build_exact`/`build_encode` verbatim provenance retains); ZigZag, Text/NFC,
floats/maps/records (need unlanded capabilities). Next bounded dependency:
entrypoint encode emission (fixed pushes + 32B copy + V2B1, ~100 lines,
reuses this record engine) then program envelope validate/build in Sley
(fixed tag/epoch, loops for magic/epoch/digest/preimage/payload, RHW1), then
composed entries with single-invocation measurement. Reuses existing scope
findings; no broad audit launched. Session-resource limit reported directly:
envelope-in-Sley plus single-invocation composition remain unexecuted in this
session after outer + entrypoint-decode completion and full payload-layer
proofs; the deliverable definition is unchanged (no redefinition of success).

## 10. Review provenance and acceptance debt

Lint triage (repo zero-warning standard): `cargo fmt --all` clean (one long
hex line wrapped); `cargo clippy --no-deps --workspace --all-targets
--locked -- -D warnings` clean (entrypoint-encode `dead_code` allow with
rationale; `needless_borrow`, `useless_format`, `format_collect` in new
tests repaired, not suppressed); `git diff --check` clean. Author:
same session/model as the candidate (self-review class under the standing
amendment; supports provisional development only, never independent
acceptance). Independent native reviewer: not obtained in this session; one
scoped read-only Council handoff was available in principle but gateway/
premium outage persists per the amendment and no rerun of a known-refused
Nabu/Vulcan route was attempted without changed availability evidence and
without specific instruction. Missing independent acceptance stays visible
here and does not stop this authorized implementation. No verdict files
manufactured; no gate edits; no promotion to runtime authority. Acceptance
debt unchanged and still required: Nabu round-12 review, premium round-2
`R2_ARCHITECTURE_PASS`, S20-780 independent acceptance. Retained premium FAIL
and `R2_EXIT: NOT_READY` preserved; `rw080` stays BLOCKED; S20-780 local audit
stays separate; S20-780 debt and R2/retained FAIL untouched. No fabricated
verdict or full-suite claim. Tier 1 / targeted validation per
tiered-validation policy (affected: `sley-vm` program-outer 13/13, uvar 9/9,
envelope 9/9, scaffold 2/2; `sley-scb1` lib 6/6; `sley-mutate` object 5/5;
fmt/clippy/diff-check clean). Full gate not run (not required for this
development iteration); known `make quick` baseline staleness carried without
re-running the full suite.

## 11. Identities

- Source: `crates/sley-vm/tests/rw080_codec_program_outer.rs` (seed-assembler
  artifact; 13 tests green); base `b52d1c6` (slice-3 checkpoint: outer decode
  + outer encode + entrypoint-decode probe) through this slice's working tree
  (outer/entrypoint proofs + native envelope reference + resources).
- Graphs/images: admitted per-test at runtime (no checked-in image); profile
  `BOOTSTRAP_PROFILE_2` (`fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459`);
  entries `decode_outer`/`encode_outer`/`decode_entrypoint` (+ callees
  `decode_uvar`/`encode_uvar`); `decode_entrypoint` images bind entries
  `(9,36)`/`(9,35)` under `Ns` 91-98; outer images `(9,31)/(9,32)` under
  71-78 and `(9,33)/(9,34)` under 81-88. Runtime-generated identities are
  execution evidence (admit receipts assert the profile digest per test);
  "generated and admitted per test" is construction, not a substitute for
  these identities.
- Dependencies: `HOST_ABI_V2` imports B2V1/PSH1/V2B1 only
  (`bc564653302a73eb5f998427250a2bb7cd87f5685ef12619bd4ae1f1b2af70d5` per
  prior slices; RHW1 unused in Sley here, used natively via `ObjectId::derive`
  for recomputed digests); `EXEC_PACKAGE_V2` via C0 seed route
  (`f4958c5e3d57762173b881288b008af17d45b5f07a431fcc442d9eec5770da94`
  retained); reference `sley-scb1` (uvar/record/union/encode primitives) +
  `sley-mutate` (`build/import_entity_object`, `EntityBodyValue::EntryPoint`,
  `SSMC1_EPOCH1_SCHEMA.txt` rows 9/25/28) + `sley-id` (domain
  `sley2.object.v1`, `ObjectId::derive`); schema context
  `field_schema_hash 1983bc8d...33ae`, `decoder_limits_hash 389791b1...6136a`
  retained from slice 3.
- Fixtures: ns-empty payload 47B / stored 123B / preimage 91B; ns-one 80B /
  156B; ns-two 113B / 189B; ep-local/proto payload 77B / stored 153B /
  preimage 121B / body 40B; epoch `[9;32]`, entity `[1;32]`, func
  `[0a;32]`/`[0b;32]`; limits `codec_limits`; seeds `0xE11E_0007` (outer 100
  mutations, flip/truncate/append/prepend) + retained `0xE11E_0003` (empty
  100); commands `cargo test -p sley-vm --test rw080_codec_program_outer`
  (13/13), `-p sley-scb1 --locked --lib` (6/6), `-p sley-mutate --locked
  --lib object` (5/5), uvar/envelope/scaffold regressions, `cargo fmt`,
  `cargo clippy --no-deps --workspace --all-targets --locked -- -D warnings`,
  `git diff --check`.
- Validation tier: Tier 1 / targeted (listed above). Full gate: not run.
