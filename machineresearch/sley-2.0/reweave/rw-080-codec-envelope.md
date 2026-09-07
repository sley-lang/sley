# RW-080 §1.1 codec dependency slice 2: standalone SCB1 envelope validation — provisional C0 construction record

Status: PROVISIONAL (2026-09-07, operator development override). One Sley
function with real strict envelope framing plus digest through the v2
admit/approve/execute boundary. Not accepted runtime authority; `rw080`
stays BLOCKED and the R2 gate stays NOT_READY pending independent Nabu
round-12 review plus premium round 2 plus S20-780 independent acceptance.

## 0. Package-name reconciliation (no new planning campaign)

Reuse of the slice-1 reconciliation (`rw-080-codec-uvar.md` §0, no new
campaign, no alias beyond that note):

- RW-080: the construction stage and contract (`rw-080-contract.md`):
  modules §1.1 codec, §1.2 checker, §1.3 lowerer/builder, §1.4 driver.
- RW-090: the codec byte/error parity acceptance gate ("Sley codec
  byte/error parity against `sley-scb1`").
- RW-100: the semantic checker/test-planner acceptance gate only.

## 1. What was built

Seed-authored Sley function (test file is the seed-assembler record):

- `validate_envelope(input: Bytes, declared: UInt64, unit: Unit)
  -> Result<Bytes, Bytes>` — exact `decode_standalone_fixture`
  envelope order (`crates/sley-scb1/src/lib.rs:427`) through digest
  verification, with opaque payload. Reuses the slice-1 `decode_uvar`
  Sley function via `CallDirect` for every uvar field (version width
  64, contract tag width 32, payload length width 64) so framing errors
  keep exact reference decision order and codes. Sley owns parsing,
  byte-consumption decisions, length/bounds checks, exact
  digest-preimage construction (`sley2.object.v1` 15-byte domain prefix
  plus exact stored prefix bytes as `Bytes`), and the validation
  decision. RHW1 is used only for its admitted primitive responsibility
  (raw BLAKE3-256 over Sley-built bytes, 1 MiB per-call ceiling).
  `Ok` carries the exact opaque payload slice; `Err` carries the exact
  `SCB_*` code.

Images: envelope-only (entry envelope, `CallDirect` callee decode).
Admitted under `BOOTSTRAP_PROFILE_2` and approved through
`admit_v2_package` (C0 seed route, declared). Bridge uses: B2V1 (entry
conversion plus digest-bytes conversion), PSH1 (UInt8 domain/preimage/
payload accumulation), V2B1 (hash-input and payload output), RHW1
(digest). Four imports, exactly the admitted set.

Source: `crates/sley-vm/tests/rw080_codec_envelope.rs` (this record's
seed-assembler artifact; `build_decode` reused verbatim from 94a2a57,
`build_exact`/`build_encode` retained as `dead_code` for churn
avoidance with rationale comments). Entity namespaces per image are
two-byte `(namespace, index)` ids under disjoint `Ns` (decode
31/32/33/34, envelope 31/36/37/38 sharing `k=31` for constants, same
pattern as the slice-1 exact image); deterministic and collision-free
by construction.

## 2. Leg status (no invented wire formats)

Unchanged from slice 1 (`rw-080-codec-uvar.md` §2, reused scope finding,
no broad audit):

| Leg (scaffold selector) | Status |
|---|---|
| 0 `decode_program` | STUB (trap; format undefined) |
| 1 `encode_program` | STUB (trap; format undefined) |
| 2 `decode_schema` | STUB (trap; format undefined) |
| 3 `encode_schema` | STUB (trap; format undefined) |

Wiring legs now would invent framing. This validator is an entry of its
own approved image through the same boundary, and is the digest-carrying
dependency every future leg needs. The missing program/schema format
definition stays recorded as the blocking prerequisite for leg wiring,
mapped to owning requirement RW-080 §1.1; next bounded resolution is a
format-definition slice, not another audit.

## 3. Contract decisions (explicit, reviewable)

- **Error-code preservation.** Refusals carry the exact `SCB_*`
  registry strings as `Bytes`. No code is mapped to the 7-code
  program-level vocabulary. Uvar `Err` bytes from `CallDirect` are
  forwarded unchanged. The vocabulary mapping belongs to leg wiring.
- **Opaque payload (new, labeled).** `Ok` means envelope framing plus
  digest are valid for the declared fixture tag; it does NOT mean the
  payload is a valid program, schema, executable, or even a valid
  fixture record. Fixture-record semantic errors (`SCB_FIELD_*`, etc.)
  belong to a later slice with the program/schema binding. Tests pin
  the divergence: envelope-valid but record-invalid inputs return `Ok`
  here while the reference reports the later-phase field code. No
  success path bypasses envelope-layer malformed handling: every
  envelope-malformed input refuses with its exact envelope code.
- **Capacity restriction (AR-05, same class as slice 1).** B2V1/RHW1
  refuse past the 1 MiB bridge caps with `SCB_RESOURCE_LIMIT` (typed)
  or execution `ResourceLimit` under tight test budgets; the epoch
  allows 67 MiB standalone. Any input this slice accepts is decided
  exactly; over-cap inputs are refused loudly with resource semantics,
  never misread, truncated, or re-hashed under a changed domain. No
  chunk scheme, no substitute digest, no domain change, no silent
  budget raise, no streaming API.
- **InternalInvariant traps.** Unreachable-by-construction edges
  (checked-arithmetic overflow on indices below the converted length,
  `Get None` after explicit remaining checks, 32-byte digest conversion)
  target `Trap(InternalInvariant)` with a proof sketch per site in code
  comments. Reachable resource edges (B2V1/V2B1/PSH1/RHW1 caps) return
  typed errors. Asymmetric defensive style is intentional and recorded:
  magic `Get None` stays typed `MAGIC_INVALID` (preserves the truncated-
  magic rule even if the length check were missed); epoch/digest `None`
  after bounds traps (construction-invariant). No malformed input
  reaches a trap.
- **Upfront `input.len > MAX` subsumed.** Reference checks
  `input.len > MAX_STANDALONE_BYTES` first (`lib.rs:431`); every
  Sley-accepted input is below the 1 MiB bridge cap (far below 67 MiB),
  and over-cap inputs refuse resource-loudly. No dedicated Sley block;
  observation only, behavior coincides with the same resource code.

## 4. Substrate findings (from building; F1/F2 reused, F4/F5 new)

- **F1/F2 reused from slice 1.** Shift amounts/widths are `UInt32`
  (shift opcodes demand u32); no int conversion opcode, so the reused
  `decode_uvar` keeps its 8-bit ladder and this validator threads
  `UInt64` positions/values with `UInt32` widths (no new workaround
  needed here). ZigZag still deferred (no envelope impact: all framing
  reads are unsigned).
- **F4: preimage bound is payload_end, not digest_end (caught before
  admission).** First draft threaded `digest_end` (`len`) as the stored-
  prefix loop bound, which would have hashed the trailer. Self-review
  against `lib.rs:468` (`input[..len-32]`) caught it; bound corrected
  to `payload_end` (`len-32`) before any test ran. No expectation was
  rewritten: the fix preceded the first green run.
- **F5: value-units bind first under tight budgets.** Measured §8 shows
  execution `ResourceLimit(ValueUnits)` (not fuel/instructions) ends the
  supported range: persistent-vector pushes accumulate O(n²) value
  units, so 512-byte payloads (588 stored) already hit ~999K peak
  against the 1M test cap while fuel (~20K) and instructions (~1.8K)
  stay far inside. Over-cap 1.1 MB inputs terminate on input value
  units before Sley conversion under `codec_limits`; under larger
  execution limits the same bytes would reach B2V1 and return typed
  `SCB_RESOURCE_LIMIT`. Both layers preserve resource semantics; second
  reviewer confirmed the distinction is documented, not silent.
- **OREF-1 retained (reference/oracle divergence, slice-1 regression).**
  Eleven bare continuations (`80` x 11): Rust reference reports
  `SCB_INTEGER_OVERFLOW` (shift bound `lib.rs:1206-1230`, verified by
  direct probe); independent Python oracle reports
  `SCB_LENGTH_OVERFLOW` (no shift bound in `codec.py:61-78`). No frozen
  vector covers this input. The reused Sley `decode_uvar` mirrors the
  reference (the normative implementation). Authority for precedence:
  the accepted local SCB1 implementation (`sley-scb1`, S20-120) over
  the independent oracle on shift-bound inputs; the oracle file
  excludes this case. Does not touch this envelope path (widths 64/32
  only, both oracle-supported).
- **OREF-2 retained (oracle coverage gap, no disagreement).** The
  oracle runner does not implement `UInt16`/`UInt32` rejected fixtures
  (2 of 8 fresh rejected vectors in slice 1 report unsupported-path;
  those Sley paths are covered against the Rust reference instead).
  Everything the oracle supports agrees (slice-1 20/20 accepted, 6/6
  supported rejected; this slice's fresh 2 accepted + 11 rejected all
  PASS on the oracle CLI). No frozen expectation was edited. OREF-2
  does not affect this envelope path (version/payload-length width 64,
  contract-tag width 32, all oracle-supported). Protected-fixture
  procedure compliance: no `conformance/` file was added, removed, or
  edited; UInt16/UInt32 expectations stay derived from the accepted
  contract (SCB1 §3 width rule plus `read_uvar_width` width gate
  `lib.rs:603-606`), never copied from candidate output.

## 5. Residuals disposition

- AR-02 (streaming hash): not exercised. This validator needs only
  single-shot RHW1 over Sley-built preimages (all in-tree preimages
  ≤ ~1 KiB for valid fixtures; capacity probes use opaque payloads to
  the measured value-units bound). No chunk scheme, no substitute
  digest, no domain change.
- AR-05 (capacity): measured §8. Format validity (67 MiB epoch) is
  distinguished from implementation/profile limits (1 MiB bridge/RHW1
  caps; ~204 stored bytes under `codec_limits` value units). Per-byte
  cost is dominated by the two persistent-vector copy loops (preimage
  plus payload), ~124 fuel/byte on the measured range.
- AR-06 (readiness vs evidence): no technical condition blocks this
  slice. Remaining work is evidence/acceptance (independent review,
  premium re-review, S20-780 separation), stated in §10.

## 6. Coverage by requirement (not just counts)

| Requirement | Evidence |
|---|---|
| Canonical accepted bytes, frozen envelope | `standalone-empty-object` (76 B, payload `00`) Sley Ok + reference Ok |
| Distinct valid payloads + declared binding | empty (tag 1, `00`), req-true (tag 2, `01010101`), req-false (tag 2, `01010100`); each with wrong-declared `CONTRACT_UNKNOWN` on both sides |
| Truncated fields/payload/digest | 0/7-byte `MAGIC_INVALID`; 8/9/10/26/42/43/44/60/75-byte `LENGTH_OVERFLOW`; trailing `TRAILING_BYTES`; all code-for-code with reference |
| Invalid magic/version/epoch/kind owned here | stale-digest flips plus recomputed-digest preimages (via `ObjectId::derive`): structure error wins in all four, both sides |
| Nonminimal/overflowing lengths + inconsistencies | version/tag `8100` nonminimal; tag `8080808010` overflow; payload-len MAX+1 `RESOURCE_LIMIT`; len-10-with-1-present `LENGTH_OVERFLOW`; all code-for-code |
| Trailing + digest corruption | valid+`00` trailing; flipped digest; payload flip without recompute; all `TRAILING`/`DIGEST` code-for-code; bad-digest+trailing proves trailing-first |
| Multi-fault precedence | magic+version→`MAGIC`; version+contract→`VERSION`; contract+epoch→`CONTRACT`; digest+trailing→`TRAILING`; length+digest→`LENGTH`; all code-for-code (SCB1 §10) |
| Structure vs digest isolation | malformed-with-recomputed vs valid-with-corrupted pairs; neither check substitutes (see §3) |
| Post-image inputs | 200 LCG mutations (seed `0xE11E_0002`, flip/truncate/append/prepend) drawn after admission; code-for-code (no recomputed digests on this stream, so no payload-phase divergence arises; guard asserts it) |
| Independent oracle | frozen 23/26 PASS; fresh 2 accepted + 11 rejected PASS via `sley2-scb1-oracle check` (byte+decode+code agreement) |
| Opaque-scope pin | 2 payload-in-envelope vectors assert Sley Ok + reference `FIELD_MISSING`/`FIELD_UNKNOWN`; Ok never implies program/schema validity |
| 20+8 corpus (slice 1) | retained via `rw080_codec_uvar.rs` 9/9 green (see §10 table); not duplicated here; OREF-1/2 carried without omission |

Test functions: 9 (frozen, distinct payloads, truncations,
invalid fields stale+recomputed, nonminimal/overflow/lengths,
trailing/digest, multi-fault precedence, 200 runtime mutations,
resources). The runtime suite compares Sley against the in-test Rust
reference per input; the oracle files give the second opinion.

### 20+8 corpus accounting (slice-1 vectors, individually)

Accepted 20 (all Sley Ok + reference Ok + oracle agree; widths 64
unless noted; still green in `rw080_codec_uvar.rs`):
`02`, `3f`, `40`, `ff3f`, `8040`, `ff7f`, `808001`, `c0843d`
(1,000,000), `ffffff07` (16,777,215), `80808008` (16,777,216),
`ffffffff0f` (u32::MAX), `8080808010` (2^32), `808080808020` (2^40),
`ffffffffffffff7f` (2^56-1), `808080808080808001` (2^56),
`ffffffffffffffff7f` (2^63-1), `80808080808080808001` (2^63),
`feffffffffffffffff01` (u64::MAX-1), `959aef3a` (123,456,789);
duplicate value 2^63-1 listed twice in the test array counts once
semantically (18 distinct magnitudes plus 2 boundary re-entries = 20
entries). Rejected 8: `818000`/`808000`/`ff8000`
(`VARINT_NON_MINIMAL` w64, oracle-supported); `ff02`
(`INTEGER_OVERFLOW` w8, supported); `808004` (`INTEGER_OVERFLOW`
w16, OREF-2 unsupported-oracle-path, reference-covered); `8080808010`
(`INTEGER_OVERFLOW` w32, OREF-2 unsupported-oracle-path,
reference-covered); `81`/`ac` (`LENGTH_OVERFLOW` w64, supported). No
vector omitted; no unsupported case folded into an unqualified PASS.

## 7. Authoring slips caught by triangulation (honest record)

Two Sley-construction slips, both caught by verification before any
expectation was set (no test expectation rewritten to match the
candidate):

- Operation-inventory omission (`h_c32` missing from its block's
  `operations` vec): lowering refused `GRAPH_ORDINAL_MISMATCH`.
  Fixed by listing the op; reran cleanly.
- Preimage-bound draft error (F4, §4): corrected from `digest_end` to
  `payload_end` on re-reading `lib.rs:468` before the first test run.

No hand-written fresh literal was wrong on first writing in this
slice: envelope vectors are either frozen bytes or computed via
`encode_uvar`/`ObjectId::derive` at test time (deterministic, no
hand-encoding).

## 8. Resource observations (measured, `ENVELOPE_FUEL`/`ENVELOPE_RESOURCE`)

Under `codec_limits` (100K instructions / 1M fuel / 1M value units /
100K output units), opaque payloads with correct digests:

- Success: stored 75 B → fuel 7031, instr 863, peak value 167315;
  76 B → 7179/878/169067; 79 B → 7545/911/174275; 107 B (32 payload)
  → 10961/1219/241979; 204 B (128 payload) → 23001/2330/741117.
  All stay inside fuel/instruction budgets; value units bind first.
- Execution `ResourceLimit(ValueUnits)`: 588 stored (512 payload,
  peak 998957) and 1100 stored (1024 payload, peak 998349); fuel
  (~20K) and instructions (~1.8K) stay far inside, proving the bound
  is persistent-vector value units, not fuel. Reported, never misread
  as format success.
- Over-cap 1,100,077 stored (> 1 MiB bridge): execution
  `ResourceLimit(ValueUnits)` at input validation under
  `codec_limits` (fuel 0, instr 0, peak 1204790); under larger
  execution limits the same bytes would reach B2V1 and return typed
  `SCB_RESOURCE_LIMIT`. Both preserve resource semantics.
- Framing/preimage overhead included: every measurement covers
  B2V1 conversion, three `CallDirect` uvar decodes, domain (15) plus
  stored-prefix pushes, V2B1, RHW1, 32 digest compares, and payload
  extraction (intermediate allocations counted in peak value units).
- Supported envelope range under the admitted profile with these
  test budgets: format-valid to 67 MiB per epoch; bridge/RHW1-typed to
  1 MiB; Sley-executed to ~204 stored bytes (128 opaque payload) with
  fixture-valid envelopes (76/79 B) far inside. Larger compiler
  preimages stay refused loudly (typed or execution resource), never
  truncated, re-hashed, or chunked.
- Unmeasured quantities (explicit): available memory and
  native-operation counts have no separate instrument in this slice;
  fuel/instructions/peak-value-units are the bounded-execution
  evidence. No streaming API introduced.

## 9. Remaining stubs and next dependency

Stubs: all four `codec_main` legs (pending program/schema format
definition, owning requirement RW-080 §1.1); ZigZag both directions
(pending int-conversion design); fixture-record semantic phase
(`FIELD_*` after digest: next codec slice once program/schema binding
exists); Text/NFC/floats/maps/records (need unlanded capabilities).

Next bounded dependency: program/schema wire-format definition slice,
which unblocks leg wiring on top of this framing core plus the digest
ратио. Reuses the existing scope finding; no broad audit launched.

## 10. Review provenance and acceptance debt

Lint triage (repo zero-warning standard restored): `cargo fmt --check`
clean; `clippy -- -D warnings` clean (one `too_many_lines` allow on
the resource test plus two `dead_code` allows on retained
`build_exact`/`build_encode`, each with justification comments citing
repo precedent `bootstrap_closure.rs`/`rw075_raw_callable.rs`; casts
use `try_from` with documented bounds; hex helper uses `write!`, not
`format!`-collect); `git diff --check` clean.

Author: same session/model as the candidate (self-review class under
the standing amendment; supports provisional development only, never
independent acceptance).

Second native opinion (separate session, read-only, no edits):
verdict PROVISIONAL-ACCEPT, no blockers. It re-ran nothing (read-only
by instruction) but traced the reference decision order in the Sley
blocks, confirmed all constraint checks (Sley-side parsing, exact
codes, opaque scope, untouched legs, bridge-only imports, OREF
handling, capacity documentation), verified the four prior nits remain
repaired in the reused builder, and raised 4 new nits (preimage-bound
comment overstatement, identical magic branches, asymmetric Get-None
defense, dead-code bloat): the first two repaired in this record
before commit; the latter two accepted as intentional with rationale
comments (defensive typing vs invariant trap; churn avoidance for
proven builders). Same-underlying-model permitted; authorship and
session separation reported honestly here: reviewer `muse-spark`
(`muse-spark-1.3-contributor`), read-only session, artifacts
`crates/sley-vm/tests/rw080_codec_envelope.rs` (new worktree file),
`crates/sley-scb1/src/lib.rs:427-481`, `docs/spec/SCB1.md` §§2/10,
`conformance/scb1/v1/*`, builder HEAD
`94a2a57c9fa4ca325c131931f31248b45bb0a486`; no edits, no writes, no
gateway/premium probe. Reuse of foundation review: exact-scope only
(gate mechanics, frozen digests, error-vocabulary approach); the new
graphs, builders, and envelope behavior are reviewed in the session
record, not covered by any prior verdict. Independent acceptance debt
unchanged and still required: Nabu round-12 review, premium round-2
`R2_ARCHITECTURE_PASS`, S20-780 independent acceptance. Retained
premium FAIL and `R2_EXIT: NOT_READY` preserved; S20-780 local audit
(`83fe94a`) stays separate (performed locally, independent review
pending; GA clean-room claim stays gated). No verdict files
manufactured; no gate edits; no promotion to runtime authority. No
refused-gateway probe was made and none is claimed.

Prior-nit confirmation (narrow, no gateway): the four slice-1 nits
(manifest width-pair prose; two stale `UInt64` comments; encode-bridge
trap prose; exact-wrapper `pos` generality) were raised by the slice-1
second opinion and repaired before 94a2a57; the envelope second
opinion re-checked the reused ranges and confirms none was
reintroduced. Re-confirmation through the refused gateway/premium
route was neither attempted nor waited for; provisional status is
preserved as stated.

## 11. Identities

- Source: `crates/sley-vm/tests/rw080_codec_envelope.rs` (this
  record's seed-assembler artifact; 9 tests).
- Graphs/images: admitted per-test at runtime (no checked-in image);
  profile `BOOTSTRAP_PROFILE_2
  (fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459)`;
  entry `validate_envelope`, callee `decode_uvar` (reused slice-1
  builder verbatim).
- Dependencies: `HOST_ABI_V2` imports B2V1/PSH1/V2B1/RHW1 only
  (`bc564653302a73eb5f998427250a2bb7cd87f5685ef12619bd4ae1f1b2af70d5`);
  `EXEC_PACKAGE_V2` envelope via the C0 seed route
  (`f4958c5e3d57762173b881288b008af17d45b5f07a431fcc442d9eec5770da94`);
  `RAW_BLAKE3_V1` (`785205fb...69f72`) used only as raw hash;
  reference `sley-scb1` (test-oracle role); independent oracle
  `sley2-scb1-oracle` (second opinion: frozen 23/26 PASS, fresh 2+11
  PASS).
- Prior checkpoint: 94a2a57
  (`94a2a57c9fa4ca325c131931f31248b45bb0a486`, clean worktree at
  start; no reset; history preserved).
- Validation tier: Tier 1 / targeted. Affected subsystems:
  `sley-vm` envelope/uvar/scaffold/raw-callable tests, `sley-scb1`
  conformance, independent oracle. Checks: 9 envelope tests, 9 uvar
  regression, 2 scaffold, 16 raw-callable (reported as 18+2 across
  two binaries), 4 sley-scb1, oracle frozen+fresh CLI, fmt, clippy
  `-D warnings`, `git diff --check`. Result: all passed. Full gate:
  not run — not required for this development iteration (Tier 1 per
  tiered-validation policy). Known `make quick` baseline failure
  retained without re-running the full suite: carried
  reproducibility-report staleness (attestation `84bfa9c9…`,
  3-surface class, release-lane owned; `rw-050-slice-1.md:165`,
  `rw-030-charter.md:180`); no full-suite PASS claimed and no
  unrelated baseline comparison rerun.
