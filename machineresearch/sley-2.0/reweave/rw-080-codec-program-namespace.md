# RW-080 §1.1 program slice 7: Namespace body Sley decode/encode + single-invocation program composition — provisional C0 construction record

Status: PROVISIONAL (2026-09-08, operator development override
`rw075_correction.operator_override_2026_09_07`). Six new Sley
functions with real strict codec algorithms through the v2
admit/approve/execute boundary (`decode_namespace`,
`encode_namespace`, `decode_program_namespace`,
`encode_program_namespace`, plus the shared `decode_uvar`/`encode_uvar`
callees), reusing the slice-1 uvar engines, the slice-4 outer engines,
and the slice-6 envelope engines via `CallDirect`. Not accepted
runtime authority; `rw080` stays BLOCKED and the R2 gate stays
NOT_READY pending independent Nabu round-12 review plus premium round
2 plus S20-780 independent acceptance. Prior slices retained
unchanged (`rw-080-codec-uvar.md`, `rw-080-codec-envelope.md`,
`rw-080-codec-program-outer.md`, `rw-080-codec-program-body.md`,
`rw-080-codec-program-entrypoint-encode.md`,
`rw-080-codec-program-envelope-compose.md`); this record covers only
the slice-7 delta. No amendment duplication, no reconciled settlement
reopened, no production/gate/ledger-status change.

## 0. Scope (no new planning campaign)

Slice-6 §9 next bounded dependency: "Namespace body Sley
decode/encode (Option + Set ordering proofs) on top of the
now-complete single-invocation EntryPoint path; then `codec_main` leg
wiring for the supported kinds." This slice does the Namespace body
plus its composed program paths. RW-080 construction stage and
contract (`rw-080-contract.md`); RW-090 codec parity gate; RW-100
semantic gate only (never imported here).

## 1. Identities and owned surface

- Baseline: `0a909e1d6ae9a63a12addbc87d5353351305530d` (slice-6
  implementation commit).
- Implementation: `5044168a448b46de22b29189d039bd1cc3b3056a`
  (`crates/sley-vm/tests/rw080_codec_program_outer.rs` only; +11949/−1171).
- This record: committed separately (see closeout HEAD).
- Owned files (explicit, closed):
  1. `crates/sley-vm/tests/rw080_codec_program_outer.rs` — extend only
     (builders, images, calls, asserts, 7 new tests). No existing
     function, test, fixture, or expectation altered (one repair class
     exception: none; the `utag_overflow`/`ns_wrap_union`/`mg_len`
     items are new-code lint repairs, not behavior changes).
  2. `machineresearch/sley-2.0/reweave/rw-080-codec-program-namespace.md`
     (this record).
- Scratch probe `crates/sley-vm/tests/rw080_ns_probe.rs` (native-only,
  used to pin §3 codes before implementation) was DELETED before
  closeout; it never shipped.
- Refused surface (untouched): other entity kinds, `codec_main` legs
  0-3, ZigZag, general Text/NFC, floats, maps, general records,
  3/4-field label/fingerprint scope, VM F6/F7/F8 root causes,
  production/gate/status promotion, unrelated cleanup. Existing
  EntryPoint observable behavior unchanged (22/22 prior program-outer
  tests green unmodified).

## 2. Canonical Namespace byte layout (from native authority)

Frozen contract: `NamespaceBody { parent: Option<EntityId>,
members: EntityIdSet }` (`value_generated.rs:43`, kind tag 3),
`impl_required_record_codec!(NamespaceBody, 1 => parent, 2 =>
members)` (`codec.rs:2496`), `Option` union 0/1
(`codec.rs:Option` impl), `EntityIdSet` sorted-strict list with
`MapDuplicate`/`MapOrder` (`codec.rs:541-587`), union tags 1-18 with
fallthrough `UnionInvalid` (`codec.rs:2800-2849`).

Wire form of a canonical body (all integers canonical uvar):

```text
union(3, record([
  (1, union(parent_tag, parent_payload)),
  (2, list(count, (sized(32B member))*))),
]))
parent_tag 0 with empty payload = None;
parent_tag 1 with 32B payload = Some(entity);
members ascending, strictly (dup -> SCB_MAP_DUPLICATE,
  descent -> SCB_MAP_ORDER).
```

Measured native vectors (epoch `[9;32]`, entity `[1;32]`,
`build_entity_object`):
- empty (None,[]): `03080201020000020100` (10B; stored 123B)
- one (None,[2]): `0329...` (43B; stored 156B)
- two (None,[2,3]): `034a...` (76B; stored 189B)
- parent_empty (Some(9),[]): `03280201220120<09*32>020100` (42B; stored 155B)
- parent_one (Some(9),[2]): 75B body; stored 188B
- parent_three (Some(9),[4,5,6]): `038b01...` (142B body,
  multi-byte union/record lengths; stored 257B)

## 3. What was built (seed-authored Sley; source is the owned test file)

- `decode_namespace(body: Bytes, unit: Unit)`
  `-> Result<Tuple<Bytes, Bytes, UInt64>, Bytes>` returns
  `(parent 0/32B, members n*32B concat, count)`: union tag 3 valid
  (1,2,4..18 `SSMC_RESERVED_FIELD_PRESENT` scope; 0/19+
  `SCB_UNION_INVALID`); union len (uvar64, RES past MAX, overrun
  LENGTH, underrun NOT trailing yet — record parses first per native
  `decode_nested_exact` order); record count ==2
  (MISSING/UNKNOWN/RES); field-1 tag dispatch BEFORE length read
  (exact `decode_record_fields` order: 2 ORDER else UNKNOWN);
  field-1 bounds within union end; parent union
  (`read_union`-then-match: bounds, tag dispatch, per-tag fit —
  0 demands empty+exact-fit else UNION/TRAILING; 1 demands exactly
  32B else LENGTH/TRAILING; 2+ UNION even with trailing); field-2
  DUP/ORDER/UNKNOWN dispatch before length read; member list
  (`read_count` 1M cap; per-member len RES/bounds-then-fixed-32;
  ordering proved only after successful element decode, fused into
  the 32B copy loop with member-0 self-compare and skipped verdict);
  list/record/union trailing AFTER inner parses succeed
  (`check_finished` order). Bridge B2V1/PSH1/V2B1 only.
- `encode_namespace(parent: Bytes 0/32B, members: Bytes n*32B,
  unit: Unit) -> Result<Bytes, Bytes>`: parent length mirror
  (0 None; 32 Some; <32 LENGTH; >32 TRAILING); 32-step remainder
  walk proving whole identities and counting (no division opcode);
  member order/dup enforced (MAP_ORDER/MAP_DUPLICATE; native encode
  has no counterpart since typed inputs cannot be unsorted — codes
  mirror the decode-side nested checks, documented); multi-byte
  lengths via canonical `encode_uvar` `CallDirect` (count, field-2
  length, union length); record prefix built EARLY (F6 discipline —
  no loop-built vector length is re-read late); parent 32B via
  unrolled Get+push chain (F8 dodge, see §5).
- `decode_program_namespace(stored: Bytes, unit: Unit)`
  `-> Result<Tuple<Bytes, Bytes, Bytes, UInt64>, Bytes>` runs
  envelope validation -> outer decode -> Namespace decode in one Sley
  invocation, returning `(entity_id, parent, members, count)`.
  Callee refusals forward unchanged (envelope > outer > body).
- `encode_program_namespace(entity_id: Bytes, parent: Bytes,
  members: Bytes, unit: Unit) -> Result<Bytes, Bytes>` runs
  Namespace encode -> outer encode -> length-derive (B2V1 + length +
  `encode_uvar` + B2V1) -> envelope build (prefix split + RHW1 digest
  + unrolled 32-step trailer append, same F6/F7 construction as
  slice-6) in one Sley invocation.

Images (all admitted under `BOOTSTRAP_PROFILE_2` through
`admit_v2_package`, C0 seed route, declared): ns-decode (entry +
`decode_namespace` + `decode_uvar`); ns-encode (entry +
`encode_namespace` + `encode_uvar`); program-ns-decode (entry +
validate + outer-decode + ns-decode + `decode_uvar`); program-ns-encode
(entry + ns-encode + outer-encode + build + `encode_uvar`). Entity
namespaces two-byte `(namespace, index)` ids under disjoint `Ns` per
builder (ns-decode 171-178, ns-encode 181-188, prog-decode 191-210,
prog-encode 211-230; entry fids `(9,55)`-`(9,68)`), deterministic and
collision-free by construction (plus a static duplicate-entity audit
in-record, §5).

## 4. Contract decisions (explicit, reviewable)

- Error-code preservation: exact `SCB_*` strings plus
  `SSMC_RESERVED_FIELD_PRESENT` for scope exclusions. Uvar `Err`
  forwarded unchanged. No mapping to the 7-code program vocabulary.
- Native-order precedence (verified against the native reference at
  runtime for every vector): union-tag dispatch before union-length
  read errors? NO — uvar errors first (tag/len decode failures
  precede dispatch); record tag dispatch before field-length reads;
  parent `read_union` bounds before tag dispatch, per-tag fit after;
  member bounds before fixed-32 checks; ordering only after a
  successful element decode; trailing only after successful inner
  parses. One deliberate improvement over slice-4: union/record
  trailing is checked AFTER the inner parse (native
  `check_finished` order), so a short union payload reports the
  inner LENGTH (proven: `ulen0` -> LENGTH both sides), where slice-4
  checks union trailing early (untested corner there; EntryPoint
  behavior untouched here).
- Scope pins (valid-but-unimplemented stays a limitation, never
  success, never invalid): body union tags 1,2,4,16,18 ->
  `SSMC_RESERVED_FIELD_PRESENT` vs native FIELD_MISSING (×4) /
  LENGTH_OVERFLOW (16); real transplanted EntryPoint body ->
  SCOPE vs native OK (mirror of the slice-6 Namespace transplant,
  reversed).
- Encode-side validation without native counterparts (Sley takes
  untyped Bytes where native takes typed values): parent 31/33 ->
  LENGTH/TRAILING (outer-encode eid mirror); members remainder ->
  LENGTH (nested short-read mirror); unsorted/dup members ->
  MAP_ORDER/MAP_DUPLICATE (decode-side mirrors). All documented as
  Sley-side canonicality enforcement.
- Capacity restriction (AR-05 class, as prior slices): B2V1/RHW1
  1 MiB bridge caps refuse `SCB_RESOURCE_LIMIT`; Sley executes what
  fits the admitted profile. No chunk/domain/budget change.
- `InternalInvariant` traps: checked-arithmetic overflow below
  converted length, `Get None` after explicit bounds, 32B digest
  target, small B2V1 conversions. Per site, same standard. No
  malformed input reaches a trap (proven by 40+16 rejection cases).

## 5. Substrate findings (F6/F7 retained; F8 new, both dodged)

F1/F2 (u32 shifts/widths, 8-bit ladder, deferred ZigZag) and F5
(`ValueUnits` cumulative charges with `O(n^2)`
persistent-vector pushes; value units bind first on every path
measured here) retained verbatim from slices 1-6. F6 (small-vector
read pollution beside a live payload vector + digest phase) and F7
(counted digest-copy loop emitting even-indexed bytes) dodges
retained and re-applied: length encodings are copied immediately
after conversion; the digest trailer is the same unrolled 32-step
chain; the record prefix is built EARLY so no loop-built vector
length is re-read late (§3 Rpre rule).

- **F8: counted-loop guard misroutes in one encode region;
  unrolled equivalent exact (DODGED, reproducer preserved).**
  In `build_namespace_encode`'s parent-copy counted loop
  (`pcopy_check`: `LessThan(idx, 32)` over a B2V1-converted 32B
  parent vec), the guard deterministically routed to done at entry
  (0 iterations; parent union emitted `[01, 20]` with no parent
  bytes; downstream lengths self-consistently 2/8), while
  structurally identical counted loops in the same image
  (remainder, count-copy, member copy/compare, R-prefix,
  f2len/Ml/ulen/final copies) execute exactly. Probe record (all
  observations stable across runs; trapped-wording probes used
  error-code routing, never expectations): `Equal(idx,k0)` TRUE and
  `Equal(idx,k32)` FALSE in the suspect block while `LessThan`
  reported FALSE (3/3 mutually consistent only under region-local
  const misread, yet the constant table is byte-exact and
  duplicate-free per static audit, and the same consts read
  correctly in `par_len`/member loops); `VectorLen(parvec)` reads 32
  in `par_len` but the pcopy guard still misroutes with NO consts
  involved (LessThan of two VectorLens); filler-byte observation
  proved Pu exactly `[01, 20]` (loop ran 0×, not short); fuel/instr
  deltas match 0 iterations. The unrolled 32× Get+push chain
  (F7-style: const indices, no counter/bound/backedge) over the
  SAME parent vector emits byte-exact parent bytes first try
  (42B body green, then the whole suite). Status: dodged by
  construction (unroll); Sley code proven correct by audit (plus a
  mechanical arity/duplicate audit of both builders: no dangling
  targets, no duplicate entities); VM-level root cause open
  (candidate class: loop-control/counter context sensitivity in the
  suspect region; NOT claimed proven). Neither finding RDCs any
  existing evidence; both are additive F-series records.
- OREF-1/2 carried without omission (uvar 9/9, envelope 9/9 green,
  oracle frozen 23/26 retained, no frozen edit).

## 6. Coverage by requirement (not just counts)

| Requirement | Evidence |
|---|---|
| Canonical accepted bytes, program binding | 6 native-built Namespace objects (empty/one/two + parent_empty/parent_one/parent_three, epoch `[9;32]`, tag 200, domain `sley2.object.v1`); Sley body-decode triples match native semantics; Sley body-encode matches canonical bodies byte-exact (not roundtrip alone); Sley composed decode returns `(eid, parent, members, count)` matching native imports; Sley composed encode matches canonical stored bytes byte-exact (independent byte comparison vs native builder for 4 in-envelope sizes) |
| Distinct valid payloads + varying inputs | Body: 6 shapes (parent None/Some × 0/1/2/3 members, incl. 142B multi-byte-length body); composed: 4 stored objects incl. post-admission-style variant (entity `[2;32]`, new membership) |
| Strict rejections + precedence | Body: ~45 vectors code-for-code vs runtime native (union tags 0/1/2/4/16/18/19; ulen 0/over/nonmin/res; count 0/1/3/5/nonmin/65536; f1t 0/2/3; f1len 0/5/nonmin/res; parent tags 2/3/7, tag0-nonempty, tag1 len 0/31/33; f2t 0/1/3/5; f2len 1/over/66; memcnt 0/1/3/nonmin/1M; memlen 31/33/nonmin/res; dup/swap members; truncations; empty; trailing (union/record/list); 5 scope pins; 2 precedence pins). Encode: 6 rejections (parent 31/33, remainder 17/33, dup, order) |
| Canonical emission from structured values | Body-encode from `(parent, members)` byte-exact for 6 shapes + determinism double-run + Sley-decode-back. Composed encode from `(eid, parent, members)` byte-exact for 4 stored sizes |
| Post-image inputs | All composed vectors built or patched after admission (native-built fixtures; patches + `ObjectId::derive` recompute so digest never conceals checks; varied entity/parent/members) |
| Independent oracle/reference | Native `sley-mutate` build/import as primary reference for valid + all malformed codes (computed at runtime in-test, never hand-typed); `sley-scb1` primitives for constructed vectors; oracle frozen 23/26 retained (not re-run, no frozen edit) |
| Envelope/digest agreement | Sley composed decode agrees with native imports (incl. digest); Sley composed encode emits stored byte-exact (incl. digest trailer); wrong epoch/digest natively and in Sley agree |
| Input-dependent behavior | Same admitted images return varying triples/quadruples per input and fault-specific codes across 45+2 vectors |
| Resource evidence (complete invocations only) | §8; value-units bind first everywhere (F5); over-limit behavior pinned by test, not hidden |

Test inventory (this slice, 7 new tests): body valid+encode (6
shapes), body encode rejections (6), body rejections (~45 vectors +
6 encode), composed valid (4 stored), composed rejections
(8 envelope + 6 body + transplant + encode-order), composed
resources (3 both-directions + 3 decode-only), bind-first pins
(2). Runtime suites compare Sley against native per input; no test
decodes the object for Sley, chooses its semantic result, or
performs the encoder's work (the build probe's length-vector input
shaping stays in the reused slice-6 build entry, unchanged).

## 7. Authoring slips caught by triangulation (honest record)

Builder arity/order slips caught at lowering before any expectation
was set (`TargetInvalid` from a missing encode-image block;
`TargetArguments` from a `u8vec`/`Bytes` transcription slip in the
composed-encode tail, found by mechanical diff against the slice-6
original); test-harness slips (no-op tag patch `0xc9-1`,
`read_exact_bytes` arity, closure borrow escapes, `Vec`/`&[]`
literal sizing) caught by compile/test before any claim; one
expectation correction with no candidate involvement (`f2len66`
predicted TRAILING, native reports LENGTH_OVERFLOW — member-loop
LENGTH precedes record trailing; both sides agree, Sley already
matched). The `Equal`-vs-`LessThan` inversion during F8 diagnosis
was a triage probe, never a claim. No test expectation was rewritten
to match candidate output at any point.

## 8. Resource observations (measured, complete invocations)

Under `codec_limits` (100K instructions / 1M fuel / 1M value units /
100K output); decode and encode budgets distinct per invocation.
Format: fuel / instructions / peak value units. Value units bind
first on every path (F5); fuel/instruments stay far inside.

Body decode (body -> triple): 10B 3330/707/62563; 43B
10805/1351/96048; 76B 17029/1801/150465; 42B 6240/956/87824; 75B
13721/1596/121941; 142B 26604/2550/257533.
Body encode (pair -> body): 10B 1203/237/59714; 43B
9124/1117/128697; 76B 16057/1834/267091; 42B 4518/705/119156; 75B
12419/1581/234499; 142B 26484/3057/680058.
Composed decode (stored -> quadruple): 123B 20984/2853/389903;
156B 34967/4054/598741; 155B 30256/3650/584356; 189B
47751/5069/900593; 188B 44245/4847/863522.
Composed encode (triple -> stored): 123B 16258/2146/557595; 156B
29901/3682/959146; 155B 25161/3258/938325.
Bind-first pins (ResourceLimit(ValueUnits) with fuel/instr inside):
enc-189B fuel 33569/instr 4018/peak 999962 (capped); dec-257B fuel
24525/instr 2553/peak 997595 (capped). Encode fits ≤156B stored;
decode fits ≤189B stored. No stage-sum substitution anywhere; no
protected-limit change; no value-charging change.

## 9. Remaining stubs and next dependency

Stubs (explicit, none claimed): `codec_main` legs 0-3 (full 18-kind
body + label/NFC + fingerprint verifier + schema legs; owning
requirements RW-080 §1.1 legs 0-3); 3/4-field outer records
(label/fingerprint stay `SSMC_RESERVED_FIELD_PRESENT` scope-first,
reused unchanged); reqbool/outer in-progress remnants (parked,
verbatim provenance retained); ZigZag, Text/NFC, floats/maps/records
(need unlanded capabilities); F6/F7/F8 VM-level root causes
(recorded §5 for VM follow-up; dodges hold by construction and
measurement). Over-envelope Namespace sizes (two-member+ encode,
parent-three decode/encode, big variant) are F5 value-unit
over-limit, pinned by test — larger envelopes need either cheaper
value charging (protected change, not proposed) or a fused emission
architecture (contradicts the required composition through proven
machinery, not proposed). The supported decode-dispatch dependency
landed later as slice 8 (`rw-080-codec-supported-dispatch.md`) for
EntryPoint 16 and Namespace 3. The remaining bounded dependency is the
canonical schema-bearing main path: other body kinds,
label/NFC/fingerprint handling and verification, followed by complete
decode and encode dispatch. No other entity kind began in this slice.

## 10. Review provenance and acceptance debt

Lint triage (repo zero-warning standard): `cargo fmt --all` clean
(applied); `cargo fmt --all -- --check` clean; `cargo clippy
--no-deps --workspace --all-targets --locked -- -D warnings` clean
(new `too_many_lines`/`type_complexity` allows on long
builders/tests, same precedent as prior slices; no behavior-lint
suppressions); `git diff --check` clean. Forge slice validate: not
applicable (no slice contract initialized for this path; targeted
validation run instead). Author: same session/model as the candidate
(self-review class under the standing amendment; supports
provisional development only, never independent acceptance).
Independent native reviewer: not obtained in this session; no rerun
of a known-refused Nabu/Vulcan route was attempted without changed
availability evidence and without specific instruction. Missing
independent acceptance stays visible here and does not stop this
authorized implementation. No verdict files manufactured; no gate
edits; no promotion to runtime authority. Acceptance debt unchanged
and still required: Nabu round-12 review, premium round-2
`R2_ARCHITECTURE_PASS`, S20-780 independent acceptance. Retained
premium FAIL and `R2_EXIT: NOT_READY` preserved; `rw080` stays
BLOCKED; S20-780 local audit stays separate; S20-780 debt and
R2/retained FAIL untouched. No fabricated verdict or full-suite
claim. Tier 1 / targeted validation per tiered-validation policy
(affected: `sley-vm` program-outer 29/29 — was 22/22, +7 new;
uvar 9/9; envelope 9/9; scaffold 2/2; `sley-scb1` lib 6/6;
`sley-mutate` object 5/5; fmt/clippy/diff-check clean). Full gate
not run (not required for this development iteration).

## 11. Identities

- Source: `crates/sley-vm/tests/rw080_codec_program_outer.rs`
  (seed-assembler artifact; 29 tests green); exact starting HEAD
  `0a909e1d6ae9a63a12addbc87d5353351305530d` through implementation
  `5044168a448b46de22b29189d039bd1cc3b3056a` (this record committed
  separately; see closeout HEAD).
- Graphs/images: admitted per-test at runtime (no checked-in image);
  profile `BOOTSTRAP_PROFILE_2`
  (`fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459`
  per `crates/sley-vm/src/exec_package.rs:123-126`); entries
  `decode_namespace`/`encode_namespace`/`decode_program_namespace`/
  `encode_program_namespace` (+ callees `decode_uvar`,
  `encode_uvar`, `decode_outer`, `encode_outer`,
  `validate_program_envelope`, `build_program_envelope`); entry fids
  `(9,55)`-`(9,68)` under disjoint `Ns` (see §3). The test path
  emits no graph-root hash and no image digest (in-memory `Image`,
  lowered and admitted per test); the binding identity this path
  produces is the per-test receipt assertion
  `receipt.profile_digest() == BOOTSTRAP_PROFILE_2_DIGEST`. No
  emitted receipt identities beyond these per-test admission
  receipts; no absent identities beyond the same (no minting path
  exercised).
- Dependencies: `HOST_ABI_V2` imports B2V1/PSH1/V2B1 (+RHW1 on
  program paths) as admitted per image
  (`bc564653302a73eb5f998427250a2bb7cd87f5685ef12619bd4ae1f1b2af70d5`
  per `conformance/host-abi/v2/SHA256SUMS` and
  `docs/spec/HOST_ABI_V2.md`); `EXEC_PACKAGE_V2` via C0 seed route
  (`f4958c5e3d57762173b881288b008af17d45b5f07a431fcc442d9eec5770da94`
  per `conformance/exec-package/v2/SHA256SUMS` and
  `docs/spec/EXEC_PACKAGE_V2.md`, unchanged by this slice);
  reference `sley-scb1` (uvar/record/union/encode primitives) +
  `sley-mutate` (`build/import_entity_object`,
  `EntityBodyValue::Namespace`, `SSMC1_EPOCH1_SCHEMA.txt` row 3) +
  `sley-id` (domain `sley2.object.v1`, `ObjectId::derive`); schema
  context `field_schema_hash 1983bc8d...33ae`,
  `decoder_limits_hash 389791b1...6136a` retained from slice 3.
- Fixtures: §2 vectors; epoch `[9;32]`, entity `[1;32]` (+ variant
  `[2;32]`), parent `[9;32]`/`[12;32]`/None, members
  `[02..06;32]`/`[13,14;32]`; limits `codec_limits`; commands
  `cargo test -p sley-vm --test rw080_codec_program_outer` (29/29),
  `--test rw080_codec_uvar` (9/9), `--test rw080_codec_envelope`
  (9/9), `--test rw080_codec_scaffold` (2/2),
  `-p sley-scb1 --locked --lib` (6/6),
  `-p sley-mutate --locked --lib object` (5/5),
  `cargo fmt --all -- --check`,
  `cargo clippy --no-deps --workspace --all-targets --locked -- -D warnings`,
  `git diff --check`.
- Validation tier: Tier 1 / targeted (listed above). Full gate: not run.
- Formal gate state (unchanged, retained): `rw080: BLOCKED`;
  `R2: NOT_READY`; retained premium result: FAIL; provisional
  qualification only; no production promotion; no gate/status
  promotion. This record claims IMPLEMENTATION COMPLETE for the
  Namespace slice only — never RW-080/R2/production qualification.

(End of file)
