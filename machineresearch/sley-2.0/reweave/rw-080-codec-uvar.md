# RW-080 §1.1 primitive slice 1: strict uvar framing core — provisional C0 construction record

Status: PROVISIONAL (2026-09-07, operator development override). Three Sley
functions with real strict algorithms through the v2 admit/approve/execute
boundary. Not accepted runtime authority; `rw080` stays BLOCKED and the R2
gate stays NOT_READY pending independent Nabu round-12 review plus premium
round 2 plus S20-780 independent acceptance.

## 0. Package-name reconciliation (no new planning campaign)

The local DAG assigns work packages as follows (rw-030-charter, rw-070,
rw-075, premium/nabu round logs):

- RW-080: the construction stage and contract (this file's authority,
  `rw-080-contract.md`): modules §1.1 codec, §1.2 checker, §1.3
  lowerer/builder, §1.4 driver.
- RW-090: the codec byte/error parity acceptance gate ("Sley codec
  byte/error parity against `sley-scb1`").
- RW-100: the semantic checker/test-planner acceptance gate ("the
  required semantic corpus green").

The two prior slice records (`rw-080-codec-scaffold.md`, commit messages)
said "RW-100 gate" for the codec parity corpus. That was loose phrasing.
Adopted mapping from here on: work happens under the RW-080 construction
contract; codec completion is proven at the **RW-090** parity gate; RW-100
names the checker gate only. No new campaign, no alias beyond this note.

## 1. What was built

Seed-authored Sley functions (test file is the seed-assembler record):

- `decode_uvar(input: Bytes, pos: UInt64, width: UInt32)
  -> Result<(value: UInt64, new_pos: UInt64), Bytes>` — exact
  `Reader::read_uvar_width` behavior (`crates/sley-scb1/src/lib.rs:1198`):
  width gate (0/>64 refuse), LE7 accumulation, shift>=64 nonzero-payload
  refusal, shift==63 payload>1 refusal, shift>=71 refusal after increment,
  trailing-zero-group non-minimality, width range check, truncation as
  `LengthOverflow`. Returns the value AND the new cursor so callers (and
  the exact wrapper) decide trailing.
- `decode_uvar_exact(input, pos, width) -> same` — calls `decode_uvar`
  via `CallDirect`, then requires new_pos == input length else
  `SCB_TRAILING_BYTES`. Mirrors `decode_payload_exact` (`lib.rs:532`)
  with one honest generalization: the reference always starts at 0
  while the Sley wrapper takes a caller `pos` (cursor generality for
  future length-delimited composition); all tests use pos 0, where
  behavior coincides exactly.
- `encode_uvar(value: UInt64, width: UInt32) -> Result<Bytes, Bytes>` —
  canonical LE7 emission (Div/Rem loop, PSH1 accumulation, V2B1 output).

Images: decode-only, exact+decode (entry exact, `CallDirect` callee),
encode-only. Each admitted under `BOOTSTRAP_PROFILE_2` and approved
through `admit_v2_package` (C0 seed route, declared). Bridge uses:
decode 1 (B2V1), exact 2 (B2V1 x2), encode 2 (PSH1, V2B1).

Source: `crates/sley-vm/tests/rw080_codec_uvar.rs`. Entity namespaces per
image are two-byte `(namespace, index)` ids, deterministic and
collision-free by construction (documented here because the historic
single-byte test convention cannot hold graphs of this size; no identity
rule requires single-byte ids).

## 2. Leg status (no invented wire formats)

Contract §1.1 names the four operations symbolically
(`decode_program | encode_program | decode_schema | encode_schema`) but no
program/schema wire format, result schema, framing, tag, or field layout
exists anywhere in-tree (`sley-scb1` covers primitives and fixture
contracts only; there is no program codec reference). Therefore:

| Leg (scaffold selector) | Status |
|---|---|
| 0 `decode_program` | STUB (trap; format undefined) |
| 1 `encode_program` | STUB (trap; format undefined) |
| 2 `decode_schema` | STUB (trap; format undefined) |
| 3 `encode_schema` | STUB (trap; format undefined) |

Wiring legs now would invent framing. The uvar units are entries of their
own approved images through the same boundary, and are the root framing
dependency every future leg needs (lengths, tags, counts, versions). The
missing program/schema format definition is recorded as the blocking
prerequisite for leg wiring, not as license to invent it.

## 3. Contract decisions (explicit, reviewable)

- **Error-code preservation.** Refusals carry the exact `SCB_*` registry
  strings as `Bytes` (e.g. `SCB_VARINT_NON_MINIMAL`). No code is mapped
  to the 7-code program-level vocabulary: inventing equivalences is
  forbidden, and exact strings preserve all reference information. The
  vocabulary mapping belongs to leg wiring once program formats exist.
- **Symmetric encode width rule (new, labeled).** The reference encoder
  takes no width, so `encode_uvar(value, width)` refusing
  `width<64 && value>=2^width` with `SCB_INTEGER_OVERFLOW` is a new
  codec-level contract decision, not reference parity. Rationale: it
  applies the decode-side width rule symmetrically; the alternative
  (emitting bytes the decoder must refuse) would break round-trip
  coherence. Tested as deterministic refusal, not as parity.
- **Width gate parity.** `width==0 || width>64 -> SCB_INTEGER_OVERFLOW`
  mirrors `read_uvar` (`lib.rs:603`) exactly on both paths.
- **Capacity restriction (AR-05).** B2V1 converts inputs up to the 1 MiB
  bridge cap; larger inputs refuse `SCB_RESOURCE_LIMIT` at conversion.
  The epoch allows 67 MiB standalone. This is a documented restriction
  of the current slice, not a silent domain reduction: any input the
  slice accepts is decided exactly; inputs past 1 MiB are refused loudly
  with a distinct code, never misread.
- **InternalInvariant traps.** Checked-arithmetic failure edges that are
  unreachable by construction (disjoint-bit accumulation, bounded
  increments, loop-variant decreases) target `Trap(InternalInvariant)`
  with a proof sketch per site in code comments. Reachable resource
  edges (B2V1 cap on decode) return typed errors. On encode, the PSH1
  (accumulator push) and V2B1 (final conversion) failure edges also
  target `InternalInvariant`: the accumulator never exceeds 10 elements
  (u64 needs at most 10 LE7 bytes; the loop variant `q = v/128`
  strictly decreases to the `q == 0` exit), so the 1 MiB bridge caps
  are unreachable by construction there. No malformed input can reach a
  trap: every input-dependent failure has a typed-error edge.

## 4. Substrate findings (from building)

- **F1: shift amounts are u32.** `IntShlChecked/IntShrChecked` demand a
  `UInt32` amount (`extended.rs:928`). Shift state and widths are
  therefore `UInt32`; values/positions stay `UInt64`. Found via lowering
  refusal, fixed, pinned by tests.
- **F2: no int conversion opcode (bootstrap-profile observation).**
  Nothing widens `UInt8` to `UInt64`, narrows back, or converts signed.
  The slice works around widening with an explicit 8-bit selection
  ladder (7 test + 7 strip + 7 accumulate blocks per unit, reused per
  byte/iteration) and narrowing with the symmetric ladder. This is a
  workaround inside Sley, not a new primitive: all parsing, canonicality
  decisions, traversal, and emission are Sley operations. A `ConvertInt`
  opcode (or an explicit exclusion with codec consequences) belongs to a
  future RW-070 follow-up lane; until then every int-domain crossing
  pays ladder blocks.
- **F3: ZigZag SInt is deferred both directions.** Decoding needs
  uint->sint for the result; encoding needs signed multiply/negate at
  i64 extremes plus sint->uint. No conversion exists (F2), so the slice
  covers unsigned framing only. All SCB1 framing reads (lengths, tags,
  counts, versions) are unsigned, so nothing in scope is blocked. The
  three frozen sint vectors are pinned reference-side for the future
  slice; no Sley sint behavior is claimed.
- **OREF-1 (reference/oracle divergence, recorded).** Eleven bare
  continuations (`80` x 11): the Rust reference reports
  `SCB_INTEGER_OVERFLOW` (shift bound, verified by direct probe); the
  independent Python oracle reports `SCB_LENGTH_OVERFLOW`. No frozen
  vector covers this input. The Sley image mirrors the reference (the
  normative implementation). The oracle file excludes this case; the
  Sley-vs-reference test pins it.
- **OREF-2 (oracle coverage gap, no disagreement).** The oracle runner
  does not implement `UInt16`/`UInt32` rejected fixtures (2 of 8 fresh
  rejected vectors report unsupported-path). Those Sley paths are
  covered against the Rust reference instead. Everything the oracle
  supports agrees (20/20 accepted byte+decode, 6/6 supported rejected).

## 5. Residuals disposition

- AR-02 (streaming hash): not exercised. This unit needs no hash
  capability; envelope digest verification (RHW1 single-shot over a
  Sley-built preimage including the domain prefix) belongs to the
  envelope slice. No chunk scheme, no substitute digest, no domain
  change.
- AR-05 (capacity): observed fuel below (no compiler-scale claims; the
  1 MiB conversion restriction is explicit in §3). Per-byte decode cost
  is dominated by the F2 ladder, ~430 fuel/byte; encode ~260/byte.
- AR-06 (readiness vs evidence): no technical condition blocks this
  slice. Remaining work is evidence/acceptance (independent review,
  premium re-review, S20-780 separation), stated in §8.

## 6. Coverage by requirement (not just counts)

| Requirement | Evidence |
|---|---|
| Canonical accepted bytes, fixed vectors | 5 frozen uvar + 20 fresh oracle-confirmed + 18-value canonical sweep, Sley decode and emit byte-exact |
| Full u64 range incl. max | `ff..01` decodes to 2^64-1, encodes back identically |
| Empty / truncated input | `""`, `"80"`, `"81"`, `"ac"` -> `SCB_LENGTH_OVERFLOW` |
| Non-minimal integer encodings | `8000`, `8100`, `808000`, `818000`, `ff8000` -> `SCB_VARINT_NON_MINIMAL` |
| Width gate + range | w0/w65 refuse; w8 accept/refuse pair (255 ok, 256 refuses); w16/w32 refusal singles (65536@w16, 2^32@w32 refuse); w1 pairs |
| Shift/overflow edges | shift63-payload2, 11-byte nonzero-payload, 11 bare continuations -> `SCB_INTEGER_OVERFLOW` |
| Trailing bytes | exact wrapper: `0100`, `ac0200` -> `SCB_TRAILING_BYTES`; bare decode returns new_pos for the caller check |
| Error precedence | width gate before conversion; truncation before framing; overflow before minimality before width (reference order); decode errors before trailing check |
| Specified failure paths | every refusal is a typed `Result::Err(Bytes)`; no input reaches `Unreachable` legs or traps (traps are construction-invariant only) |
| Round-trip necessity + insufficiency | decode(encode(v))==v AND encode/decode checked independently against reference bytes/codes on 218 runtime-drawn values |
| Post-image inputs | 200 LCG round-trips + 200 LCG mutations drawn at test time after image admission; 28 fresh oracle-file vectors authored after 72fff72 |
| Independent oracle | Python oracle agrees on all supported fresh cases (§4 OREF-1/2); frozen corpus green on both sides |

Test functions: 9 (frozen accepted, frozen rejected, edges, encode
rules, fresh vectors, runtime round-trip x218, runtime mutations x200
code-for-code, exact wrapper, resources). The runtime suites compare
Sley against the in-test Rust reference per input; the oracle file
gives the second opinion.

## 7. Authoring slips caught by triangulation (honest record)

Six hand-written fresh literals were wrong on first writing (byte-length
miscounts: `8040`/`ffffff07`/`80808008`/8-byte `..8001` forms; two
sub-2^width "overflow" vectors that were valid). In every case the Sley
image decoded the given bytes correctly and the reference cross-check
fired, i.e. the triangulation worked as designed. No test expectation
was rewritten to match the candidate: each literal was re-derived from
the encoding rule and re-verified against both oracles.

## 8. Resource observations (measured, `--nocapture UVAL_FUEL`)

Decode fuel by input length (width 64): 1B 244, 2B 614, 5B 2218, 10B
4302. Encode fuel by output length: 1B 130, 2B 345, 4B 1015, 10B 2625.
Exact wrapper on 2B: 639. Budgets: 100k instructions / 1M fuel; all
observations stay under 5% of fuel budget. No scale claims beyond the
measured range. Bridge conversion charges 1 fuel/byte up front; ladder
blocks dominate per-byte cost.

## 9. Remaining stubs and next dependency

Stubs: all four `codec_main` legs (pending program/schema format
definition); ZigZag both directions (pending int conversion design);
envelope validation (magic/version/tag/epoch/len/trailing/digest: next
bounded slice, needs RHW1 + bytewise compare, both available);
length-delimited skip/extract composed via `CallDirect` on this unit;
Text/NFC/floats/maps/records (need unlanded capabilities).

Next bounded dependency: standalone envelope validation
(`decode_standalone_fixture` shape: envelope order precedence per
`lib.rs:427`, digest via Sley-built `domain ++ preimage` through RHW1),
which turns this framing core into a complete fixture-codec path
against `conformance/scb1/v1` (23 accepted / 26 rejected).

## 10. Review provenance and acceptance debt

Lint triage (repo zero-warning standard restored): all casts use
`try_from`/`From` with documented bounds (the mutation-test byte draws
take `% 256` first, making the truncation explicit); doc identifiers
carry backticks; the three builders carry function-level allows for
positional-name and line-count style lints with justification comments
(repo precedent: `bootstrap_closure.rs`, `rw075_raw_callable.rs`), since
renaming green proof-code slots would churn the layout the checker
verifies positionally. `cargo fmt --check` clean, clippy zero warnings,
`git diff --check` clean.

Author: same session/model as the candidate (self-review class under the
standing amendment; supports provisional development only, never
independent acceptance).

Second native opinion (separate session, read-only, no edits): verdict
PROVISIONAL-ACCEPT, no blockers. It re-ran 9/9 green, traced the full
reference decision order in the Sley blocks, confirmed all constraint
checks (Sley-side parsing, exact codes, labeled symmetric rule,
untouched legs, bridge-only imports), independently confirmed OREF-1
from the oracle source (no shift bound there) and OREF-2 from the oracle
dispatch table, verified fuel numbers and coverage counts, and raised 4
nits (manifest width-pair prose, two stale `UInt64` comments,
encode-bridge trap prose, exact-wrapper `pos` generality) - all four
repaired in this record before commit. Same-underlying-model permitted;
authorship and session separation reported honestly here. Reuse of
foundation review: exact-scope only (gate mechanics, frozen digests,
error vocabulary approach); the new graphs, builders, and codec
behavior are reviewed in the session record, not covered by any prior
verdict. Independent acceptance debt unchanged and still required:
Nabu round-12 review, premium round-2 `R2_ARCHITECTURE_PASS`, S20-780
independent acceptance. Retained premium FAIL and `R2_EXIT: NOT_READY`
preserved; no verdict files manufactured; no gate edits; no promotion
to runtime authority.

## 11. Identities

- Source: `crates/sley-vm/tests/rw080_codec_uvar.rs` (this record's
  seed-assembler artifact).
- Graphs/images: admitted per-test at runtime (no checked-in image);
  profile `BOOTSTRAP_PROFILE_2
  (fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459)`;
  entry functions per image (decode/exact/encode).
- Dependencies: `HOST_ABI_V2` imports B2V1/PSH1/V2B1 only;
  `EXEC_PACKAGE_V2` envelope via the C0 seed route;
  `RAW_BLAKE3_V1` unused here; reference `sley-scb1` (test-oracle
  role); independent oracle `sley2-scb1-oracle` (second opinion).
- Prior checkpoint: 72fff72 (scaffold; legs unchanged and still
  trapping).
