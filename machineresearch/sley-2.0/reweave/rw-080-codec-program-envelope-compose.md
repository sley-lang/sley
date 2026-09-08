# RW-080 §1.1 program slice 6: program-object envelope in Sley + single-invocation EntryPoint composition — provisional C0 construction record

Status: PROVISIONAL (2026-09-08, operator development override
`rw075_correction.operator_override_2026_09_07`). Four new Sley
functions with real strict codec algorithms through the v2
admit/approve/execute boundary (`validate_program_envelope`,
`build_program_envelope`, `decode_program_entrypoint`,
`encode_program_entrypoint`), reusing the slice-1 uvar engines and the
slice-4 outer/entrypoint engines via `CallDirect`, plus native program
reference for stored objects. Not accepted runtime authority; `rw080`
stays BLOCKED and the R2 gate stays NOT_READY pending independent
Nabu round-12 review plus premium round 2 plus S20-780 independent
acceptance. Prior slices retained unchanged (`rw-080-codec-uvar.md`,
`rw-080-codec-envelope.md`, `rw-080-codec-program-outer.md`,
`rw-080-codec-program-body.md`,
`rw-080-codec-program-entrypoint-encode.md`); this record covers only
the slice-6 delta. No amendment duplication, no reconciled settlement
reopened, no production/gate/ledger-status change.

## 0. Scope (no new planning campaign)

Reuse of slices 1-5 reconciliation (no new campaign, no alias beyond
those notes): RW-080 construction stage and contract
(`rw-080-contract.md`); RW-090 codec parity gate; RW-100 semantic gate
only (never imported here).

## 1. What was built (seed-authored Sley, this file is not the assembler;
source is `crates/sley-vm/tests/rw080_codec_program_outer.rs`)

- `validate_program_envelope(input: Bytes, unit: Unit)`
  `-> Result<Bytes, Bytes>` is the exact `import_entity_object`
  envelope order (`crates/sley-mutate/src/object.rs:118-149`) through
  digest verification, with opaque payload: upfront `len < 32`
  LENGTH_OVERFLOW (reference `input.len() < 32` before magic), magic
  `SLEYSCB1`, version uvar64 == 1, tag uvar32 == 200 fixed (any other
  value refuses CONTRACT_UNKNOWN, whether unknown or a valid fixture
  tag; no declared-param comparison), epoch fixed `[9;32]` (explicit
  native test bound, not registry acceptance), payload length uvar64
  (errors first, then RESOURCE_LIMIT past MAX_STANDALONE_BYTES),
  payload/digest bounds (LENGTH_OVERFLOW), TRAILING (before digest),
  DIGEST via Sley-built `sley2.object.v1` domain ++ stored prefix
  through RHW1. Uvar `Err` forwarded unchanged. Bridge uses
  B2V1/PSH1/V2B1/RHW1. Returns the exact opaque payload slice.
- `build_program_envelope(payload: Bytes, lenvec: Vector<UInt8>,
  unit: Unit) -> Result<Bytes, Bytes>` constructs the complete
  canonical stored object (`build_preimage` order,
  `object.rs:244-262`): fixed header (magic8 + version `01` + tag200
  `C8 01`) + 32 epoch bytes (`09`) via counted loop + caller-supplied
  length encoding + payload copy loops, then digest =
  RHW1(`sley2.object.v1` ++ stored prefix) appended as the exact
  32-byte trailer. The length encoding arrives as a caller-supplied
  vector (see §4 design rule); no native-prebuilt envelope enters the
  Sley encoder. Payload length past MAX refuses RESOURCE_LIMIT.
  `LIMIT` for exhaustion, never truncation.
- `decode_program_entrypoint(stored: Bytes, unit: Unit)`
  `-> Result<Tuple<Bytes, Bytes, UInt64>, Bytes>` runs envelope
  validation -> outer decode -> EntryPoint decode in one Sley
  invocation via `CallDirect`, returning the structured
  `(entity_id, function, exposure)` triple. Callee refusals forward
  unchanged, so envelope codes precede outer codes precede body codes.
- `encode_program_entrypoint(entity_id: Bytes, function: Bytes,
  exposure: UInt64, unit: Unit) -> Result<Bytes, Bytes>` runs
  EntryPoint encode -> outer encode -> length-derive (B2V1 + length +
  `encode_uvar` + B2V1) -> envelope build in one Sley invocation,
  returning the complete canonical stored object. Callee refusals
  forward unchanged.

Images (all admitted under `BOOTSTRAP_PROFILE_2` through
`admit_v2_package`, C0 seed route, declared): validate-only (entry
validate + callee decode_uvar); build-only (entry build, no callee);
program-decode (entry + validate + outer-decode + entrypoint-decode +
decode_uvar); program-encode (entry + entrypoint-encode + outer-encode
+ build + encode_uvar). Entity namespaces are two-byte
`(namespace, index)` ids under disjoint `Ns` per builder
(validate 111-118, build 125-128, decode 131-150, encode 151-170;
entry fids `(9,41)`/`(9,42)` validate, `(9,44)` build, `(9,45-49)`
decode, `(9,50-54)` encode), deterministic and collision-free by
construction.

## 2. Program-context resolution (from authority, not assumption)

- Program object tag 200 (`ENTITY_OBJECT_CONTRACT_TAG`,
  `object.rs:21`; `SSMC1_EPOCH1_SCHEMA.txt:4-5` `contract 200`,
  `digest_domain 3 sley2.object.v1`, `kind 200 SemanticEntity`).
- Accepted program schema epoch for these fixtures `[9;32]`
  (explicit native test bound used by `object.rs` tests and the slice-4
  probe fixtures; not registry acceptance, not a production epoch).
- Envelope field ordering (reference `import_entity_object`):
  len>MAX RESOURCE_LIMIT; len<32 LENGTH_OVERFLOW; magic MAGIC_INVALID;
  version uvar64 (errors first) VERSION_UNSUPPORTED; tag uvar32
  (errors first) CONTRACT_UNKNOWN; epoch 32B (short LENGTH_OVERFLOW)
  EPOCH_MISMATCH; payload-len uvar64 (errors first, then
  RESOURCE_LIMIT past MAX); payload short LENGTH_OVERFLOW; digest
  short LENGTH_OVERFLOW; trailing TRAILING_BYTES; digest
  DIGEST_MISMATCH; then body-layer codes.
- Canonical payload length encoding: uvar64 (`read_len`
  nonminimal/overflow preserved via `CallDirect decode_uvar`).
- Digest domain and exact preimage: BLAKE3(`sley2.object.v1` ++ stored
  prefix), i.e. `ObjectId::derive(preimage)` over
  `input[..len-32]` after the trailing check (F4 bound discipline
  retained: preimage bound is payload_end, not digest_end).
- Trailer handling: exact 32-byte digest trailer; anything else is
  LENGTH_OVERFLOW (short) or TRAILING_BYTES (long), never a digest
  decision.
- Fixture-envelope context (slice 2: tags 1..2, fixture epoch,
  declared tag) vs program-object context (tag 200, epoch `[9;32]`,
  fixed constants) are separate entries; the fixture validator is
  preserved unchanged (9/9 green). No tag/epoch check was weakened to
  accept arbitrary tags or epochs.
- Envelope-valid is distinct from program-valid (labeled): validate
  `Ok(payload)` means framing plus digest only. A recomputed-digest
  exposure-3 object returns validate-`Ok` (77B payload) while composed
  decode and native import both report `SCB_UNION_INVALID` (pinned by
  test). Existing canonical bytes, schema identities, epochs, and
  object identity semantics preserved.

## 3. Contract decisions (explicit, reviewable)

- Error-code preservation: refusals carry exact `SCB_*` strings and
  `SSMC_RESERVED_FIELD_PRESENT` for scope exclusions. No mapping to
  the 7-code program vocabulary. Uvar `Err` forwarded unchanged.
- Opaque scope for both envelope stages (labeled): validate-`Ok`
  never means the payload is a valid program object; outer-`Ok`
  (slice-4 pin, retained) never means the body is valid. Body validity
  belongs to the entrypoint stage and RW-100, never inferred.
- Scope-first and scope preservation through composition: union tags
  1..15,17,18 (including a real transplanted Namespace body, §6),
  outer 3/4-field records and tags 3/4 report
  `SSMC_RESERVED_FIELD_PRESENT`, pinned divergence vs native `Ok`
  where native accepts. Valid-but-unimplemented stays an
  implementation limitation, never redefined as invalid and never
  reported as format success. OREF-1/OREF-2 and prior explicit scope
  divergences preserved (uvar 9/9, envelope 9/9 green, oracle frozen
  23/26 retained, no frozen edit).
- Capacity restriction (AR-05 class, same as slices 1-5): B2V1/RHW1
  convert/hash up to the 1 MiB bridge caps; larger refuses
  `SCB_RESOURCE_LIMIT` at conversion. Epoch allows 64 MiB
  (67,108,864 bytes) standalone; Sley executes what fits the admitted
  bridge/profile (all fixtures 153B stored, far inside). No chunk
  scheme, substitute digest, domain change, silent budget raise, or
  streaming API.
- `InternalInvariant` traps: checked-arithmetic overflow on indices
  below converted length, `Get None` after explicit remaining checks,
  32B digest conversion target, small B2V1 conversions. All per site
  with the same standard as slices 1-5. Reachable resource edges
  (B2V1/PSH1/V2B1/RHW1 caps) return typed errors. No malformed input
  reaches a trap (proven by 14 validate + 18 composed rejection cases
  plus 200 retained envelope mutations in the envelope suite).

## 4. Design rule from substrate findings (F6/F7; see §5)

- **F6 rule (applied): the length copy precedes the payload
  conversion, and the length encoding crosses `CallDirect` as a
  first-class vector.** Rationale: in full-build-graph context,
  small-vector reads (1-2 byte length vectors) deterministically
  gained a trailing byte equal to the payload vector's second byte
  (e.g. `[03]` read as `[03, BB]` with payload `[AA,BB,CC]`; tracked
  across varied payloads; the polluted prefix is digest-bound, proven
  by digest-oracle mismatch). Minimal contexts (solo uvar encode,
  B2V1/len/call/B2V1/V2B1 chains with and without 43 pushes, outer
  image) all read exactly. Reordering the build so the length copy
  runs before any payload vector exists, plus passing the encoding as
  a vector (never converting short `Bytes` beside digest code),
  yields byte-exact lengths in every image. Reproducer recipe (to
  re-trigger: pre-reorder build with in-graph `encode_uvar` +
  B2V1-of-result beside the digest phase, 3-byte `aabbcc` payload):
  expect stored prefix `...2B [03,BB]...` with digest mismatch.
- **F7 rule (applied): the digest trailer is appended by an unrolled
  32-step Get+push chain, not a counted loop.** Rationale: a counted
  digest-copy loop (constant-32 bound, +1 increment, both audited
  correct many times over) deterministically emitted 16 bytes equal
  to the even indices of the true digest, while an identical-shape
  prefix loop in the same image copied 121/121 exactly; single-const
  edits flipped multiple independent behaviors (bound=1 run emitted
  the bare correct 32B digest with no prefix); minimal contexts
  (200-push loop exact, outer 3-loop image exact, validate unrolled
  digest chain exact) all behave. Unrolling the 32 digest Gets (same
  shape as the proven slice-2 digest chain, appending instead of
  comparing) yields byte-exact 153B objects. The stride-2 datum and
  bound paradox are preserved below as the F7 reproducer recipe.
- Neither rule changes emitted bytes, limits, domains, or error
  codes; both are placement/shape choices inside the Sley graphs,
  recorded here (not silent), with the triggering shapes preserved in
  this record for VM follow-up.

## 5. Substrate findings (F6/F7 anomalous record; F1/F2/F5 retained)

F1/F2 (u32 shifts/widths, 8-bit ladder, deferred ZigZag) and F5
(`ValueUnits` cumulative charges with `O(n^2)`
persistent-vector pushes) retained verbatim from slices 1-5. New:

- **F6: small-vector read pollution beside a live payload vector
  (DODGED, reproducer preserved).** In the pre-reorder build graph
  (entry B2V1 payload, header/epoch pushes, in-graph `encode_uvar`
  + B2V1-of-result, copy loops, digest phase), the 1-2 byte length
  vector deterministically read with one trailing byte equal to the
  payload vector's second byte: `[03]` -> `[03, BB]` for payload
  `[AA,BB,CC]`; tracked to `[03, 00]`/`[03, FF]`/`[03, 11]` for
  payloads `[AA,00,CC]`/`[AA,FF,CC]`/`[AA,11,CC]`; `[4D, 01]` for the
  77B entrypoint payload; `[80, AB]` (replacement, not append) for a
  128B payload; `[00, 00]`/`[01, 01]` for empty/1B payloads. Six
  independent arity/order/type audits of the loop found no defect;
  `build_encode` in isolation returns exact `[03]`; B2V1/len/call/
  B2V1/V2B1 chains with and without 43 pushes all return exact;
  outer-encode's identical B2V1-of-length shape is exact in its own
  image. Status: dodged by construction (§4 F6 rule); Sley code
  proven correct by audit; VM-level root cause open (candidate
  class: stale-length/aliasing in small-vector materialization;
  NOT claimed proven).
- **F7: counted digest-copy loop emits even-indexed bytes (DODGED,
  reproducer preserved).** In the composer graph, the counted
  digest-copy loop (constant-32 bound, +1 increment, both verified;
  fresh-const substitution retested) deterministically emitted 16
  bytes exactly equal to the even indices of the true 32B digest
  (16/16 positional match; prefix 121/121 exact in the same output),
  total 137B vs 153B. Single-const edits flipped multiple
  independent behaviors (bound=1 run emitted the bare correct 32B
  digest with no prefix; TupleGet-swap experiment behaved exactly
  per swapped logic with 136+16 shape). Minimal contexts clean
  (200-push loop exact to 200B; outer 3-loop image exact; validate
  unrolled digest chain exact; solo RHW1 exact against raw BLAKE3).
  Unrolling the 32 digest Gets (no counter/bound/backedge) yields
  byte-exact objects. Status: dodged by construction (§4 F7 rule);
  Sley code proven correct by audit (incl. automated edge-type audit
  of the whole composer); VM-level root cause open (candidate
  class: loop-control/counter context sensitivity; NOT claimed
  proven). Neither finding RDCs any existing evidence; both are
  additive records in the F-series tradition.
- OREF-1/2 carried without omission (see §3). No frozen edit; oracle
  frozen 23/26 retained, not re-run here.

## 6. Coverage by requirement (not just counts)

| Requirement | Evidence |
|---|---|
| Canonical accepted bytes, program binding | EntryPoint stored 153B / payload 77B / body 40B (`ep-local` func `0a*32` exp Local, `ep-proto` func `0b*32` exp Protocol, plus post-admission variant entity `02*32` func `0c*32` exp Protocol) via native `build_entity_object` (epoch `09*32`, tag 200, domain `sley2.object.v1`); Sley composed decode returns `(eid, func, exp)` triples matching native imports; Sley composed encode from structured values matches canonical stored bytes byte-exact (not roundtrip alone; independent byte comparison vs native builder for 3 cases) |
| Distinct valid payloads + varying inputs | Decode: local vs protocol vs post-admission variant (entity, function, and exposure all vary after admission, with input-dependent structured results). Encode: same three from structured values. Validate/build probes cover the same three |
| Program envelope validation order | 14 rejection vectors, each code-for-code vs native `import_entity_object`: short10 LENGTH; bad magic/version/tag2/tag201/epoch; trunc LENGTH; trailing-pre/post TRAILING; trailing+digest TRAILING (precedence); bad digest DIGEST; nonminimal len NON_MINIMAL; len MAX+1 RESOURCE; len127 LENGTH |
| Composed precedence + scope | 16 vectors: envelope faults (magic/version/tag/epoch/trunc/trailing/digest) precede body faults; tag-beats-body, epoch-beats-body, digest-beats-body, trailing-beats-digest pins; recomputed exposure3 UNION, count1 MISSING, count5 UNKNOWN, dup11 DUPLICATE, order10 ORDER, funclen31 LENGTH, eidlen33 TRAILING (all agree native); union1-scope (Sley SCOPE vs native FIELD_MISSING, both independently correct); real namespace-body transplant (Sley SCOPE vs native OK, pinned divergence, same class as slice-4 scope pins) |
| Post-image inputs | All composed vectors built or mutated after admission (native-built fixtures per case; patches + recomputed digests via `ObjectId::derive` so digest never conceals envelope checks; varied entity/function/exposure triple) |
| Independent oracle/reference | Native `sley-mutate` build/import (tag 200, epoch `09*32`, domain `sley2.object.v1`) as primary reference for valid + envelope rejections and composed agreement; hand-derived `object.rs`/`codec.rs` order for malformed codes; `sley-scb1` primitives for canonical bytes; oracle frozen 23/26 retained (not re-run here, no frozen edit) |
| Envelope/digest agreement | Sley validate Ok payloads byte-exact vs native preimages (77B); Sley build emits prefix byte-exact vs native preimage (121B); Sley composed encode emits stored byte-exact vs native stored (153B); wrong epoch/digest natively and in Sley agree |
| Input-dependent behavior | Same admitted images return different structured results per input (triples vary; rejections vary by fault across 14 + 16 distinct codes). Opaque copying alone could not produce interpreted `(eid, func, exp)` triples |
| Resource evidence (complete invocations only) | Decode 153B: fuel 28921, instr 3478, peak 553862. Encode: fuel 21900/21910, instr 2728/2731, peak 837106/837152. Validate 153B: fuel 16264. Build 77B: fuel 9676. All inside `codec_limits` (100K instr / 1M fuel / 1M value units / 100K output); value-units bind first (F5). No stage-sum substitution; no protected-limit change |

Test inventory (this file, 22 tests): 15 retained (empty 4, outer
probe/encode/reencode/rejections/mutations/resources 6, entrypoint
probe/encode/rejections 3, native envelope reference 1, outer+
entrypoint resources 1) + 7 new (validate probe, validate 14
rejections, build probe, composed decode triples, composed encode
byte-exact + exp3, composed 16 rejections, composed resources). The
runtime suites compare Sley against native per input; no test decodes
the object for Sley, chooses its semantic result, or performs the
encoder's work (the build probe's length-vector input shaping is
documented harness shaping; binding encoder evidence is the composed
path where every byte is Sley-produced).

## 7. Authoring slips caught by triangulation (honest record)

Builder arity slips caught at lowering before any expectation was set
(`TargetArguments` on first validate admission: lp_done 8-vs-7 params;
len-phase plen threading; epoch/magic chain arities): each fixed by
re-audit, no expectation rewritten. `op_result` vs `oav` wrapper slip
(Rust E0308) and `pav`-of-op slip (`pav(h_ghim)`, found by a
sav/pav-vs-op audit; would have been `ValueUnresolved`) repaired.
`p_unit` shadowing slip (function param vs block param, caught as
`GraphOwnerMismatch`) repaired by rename. cp_next missing-param slip
(caught as inventory mismatch) repaired. One test-expectation
correction (no candidate involvement): `union1_scope` native
expectation corrected from `OK` to `SCB_FIELD_MISMATCH` after verifying
the native reference genuinely reports FIELD_MISSING for tag-patched
content (patched tag does not make a valid Namespace body); the Sley
expectation (scope) stood as designed. No test expectation was
rewritten to match candidate output at any point. Digest-domain
omission in the first split design (RHW1 over prefix without domain)
caught by byte comparison before any claim; repaired by returning
`(prefix, preimage)` then folding the digest back into the build graph
per §4 (no tuples cross into the digest composer now).

## 8. Resource observations (measured, complete invocations)

Under `codec_limits` (100K instructions / 1M fuel / 1M value units /
100K output units); decode and encode budgets distinct per invocation:

- Composed decode (stored 153B -> triple): fuel 28921, instr 3478,
  peak 553862, all three variants.
- Composed encode (triple -> stored 153B): fuel 21900/21910, instr
  2728/2731, peak 837106/837152.
- Validate-only (stored 153B -> payload 77B): fuel 16264.
- Build-only (payload 77B + lenvec -> stored 153B): fuel 9676.
- Slice-3 200-300K outer projection: stays replaced by slice-4/5
  measurements. No general optimization pass (consumer fits: peaks
  <840K vs 1M, 1.2x headroom at worst; fuel/instr <30K vs budgets).
  No protected-limit change, no value charging change, no hash
  preimage change, no native semantic codec, no streaming design.

## 9. Remaining stubs and next dependency

Stubs (explicit, none claimed): program envelope paths for 3/4-field
records (label/fingerprint tags stay `SSMC_RESERVED_FIELD_PRESENT`
scope-first); `codec_main` legs 0-3 (full 18-kind body + label/NFC +
fingerprint verifier + schema legs; owning requirements RW-080 §1.1
legs 0-3); Namespace Sley codec (Option + Set ordering; retained as
outer opaque inputs and as the transplanted scope pin only);
reqbool/outer in-progress remnants (`build_fixture_reqbool_decode`
parked, `build_exact`/`build_encode` verbatim provenance retained);
ZigZag, Text/NFC, floats/maps/records (need unlanded capabilities);
F6/F7 VM-level root causes (recorded §5 for VM follow-up; dodges hold
by construction and measurement, not by proof of mechanism). Next
bounded dependency (from the real codec dependency graph, not
automatic expansion): Namespace body Sley decode/encode (Option +
Set ordering proofs) on top of the now-complete single-invocation
EntryPoint path; then `codec_main` leg wiring for the supported kinds.
No other entity kind begun in this slice.

## 10. Review provenance and acceptance debt

Lint triage (repo zero-warning standard): `cargo fmt --all` clean
(applied, mechanical line-wrap only);
`cargo clippy --workspace --all-targets -- -D warnings` clean (new
`too_many_lines` allows on the two composed builders and two long
tests, same precedent as prior slices; no suppressions of behavior
lints); `git diff --check` clean. Author: same session/model as the
candidate (self-review class under the standing amendment; supports
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
(affected: `sley-vm` program-outer 22/22, uvar 9/9, envelope 9/9,
scaffold 2/2; `sley-scb1` lib 6/6; `sley-mutate` object 5/5;
fmt/clippy/diff-check clean). Full gate not run (not required for
this development iteration); known `make quick` baseline staleness
carried without re-running the full suite.

## 11. Identities

- Source: `crates/sley-vm/tests/rw080_codec_program_outer.rs`
  (seed-assembler artifact; 22 tests green); exact starting HEAD
  `9b7c52ded7a0e5841501854e697cd0081fe47201` (slice-5 record) through
  this slice's working tree.
- Graphs/images: admitted per-test at runtime (no checked-in image);
  profile `BOOTSTRAP_PROFILE_2`
  (`fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459`
  per `crates/sley-vm/src/exec_package.rs:123-126`); entries
  `validate_program_envelope` `(9,42)`, `build_program_envelope`
  `(9,44)`, `decode_program_entrypoint` `(9,49)`,
  `encode_program_entrypoint` `(9,54)` (+ callees `decode_uvar`,
  `encode_uvar`, `decode_outer`, `encode_outer`, `decode_entrypoint`,
  `encode_entrypoint`). The test path emits no graph-root hash and no
  image digest (in-memory `Image`, lowered and admitted per test,
  nothing printed or hashed); the binding identity this path produces
  is the per-test receipt assertion
  `receipt.profile_digest() == BOOTSTRAP_PROFILE_2_DIGEST`.
- Dependencies: `HOST_ABI_V2` imports B2V1/PSH1/V2B1/RHW1 as admitted
  per image (`bc564653302a73eb5f998427250a2bb7cd87f5685ef12619bd4ae1f1b2af70d5`
  per `conformance/host-abi/v2/SHA256SUMS` and
  `docs/spec/HOST_ABI_V2.md`); `EXEC_PACKAGE_V2` via C0 seed route
  (`f4958c5e3d57762173b881288b008af17d45b5f07a431fcc442d9eec5770da94`
  per `conformance/exec-package/v2/SHA256SUMS` and
  `docs/spec/EXEC_PACKAGE_V2.md`, unchanged by this slice);
  reference `sley-scb1` (uvar/record/union/encode primitives) +
  `sley-mutate` (`build/import_entity_object`,
  `EntityBodyValue::EntryPoint`, `SSMC1_EPOCH1_SCHEMA.txt` rows
  9/25/28) + `sley-id` (domain `sley2.object.v1`,
  `ObjectId::derive`); schema context `field_schema_hash
  1983bc8d...33ae`, `decoder_limits_hash 389791b1...6136a` retained
  from slice 3.
- Fixtures: ep-local/ep-proto stored 153B / payload 77B / preimage
  121B / body 40B (+ post-admission variant entity `[2;32]` func
  `[0c;32]`); epoch `[9;32]`, entity `[1;32]`, func `[0a;32]`/
  `[0b;32]`; limits `codec_limits`; commands
  `cargo test -p sley-vm --test rw080_codec_program_outer` (22/22),
  `--test rw080_codec_uvar` (9/9), `--test rw080_codec_envelope`
  (9/9), `--test rw080_codec_scaffold` (2/2),
  `-p sley-scb1 --locked --lib` (6/6),
  `-p sley-mutate --locked --lib object` (5/5),
  `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `git diff --check`.
- Validation tier: Tier 1 / targeted (listed above). Full gate: not run.

(End of file)
