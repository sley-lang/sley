# RW-050 slice 1 — G-10 bridge implementation (2026-09-06)

> STATUS: POSITIVE on re-landing (§9). The slice first closed as a
> NEGATIVE RESULT (prototype fully implemented but review FAILED at
> owner level, §§7–8 preserved below as permanent evidence). Under the
> granted owner amendment (i) the work was reconciled (not merged),
> repaired, re-reviewed to all-green, and landed. §§1–5 describe the
> LANDED implementation (reconciled from the parked prototype branch
> `rw050-slice1-prototype`, which remains untouched history).

Package: RW-050 complete bootstrap-support semantic/VM profile (depends on
RW-030 COMPLETE `c6e4269`, RW-040 COMPLETE `f625350`). This slice implements
the admitted remedy under `rw-030-g10-admission.md` §6: the two conversions
with cap/fuel/failure, the admitted generic push, frozen cap and charges,
byte-level workloads both directions, negative corpus, permitted-import
freeze input. No other profile work; BOOTSTRAP_PROFILE_1 freeze as a whole
and the E8 fuzz lane stay follow-ups.

## 1. Carrier decision (recorded, reviewed)

All three primitives ride `adapter_invoke` (tag 161) as versioned entries,
in its frozen invocation shape: two operands (`scope`, `request`) and an
`Entity` immediate naming an import with declared request/response/failure
types. The admission left push's carrier open (E-profile opcode or host
import); slice 1 selects host import for all three because the E-profile-
opcode carrier is foreclosed without epoch work: the SSMC1 epoch-1 opcode
table is frozen and test-pinned (`sley-ssmc` tag/from_tag round-trip test
pins the exact 55-tag list; manifest `SSMC1_EPOCH1_SCHEMA.txt` closes the
rows; `BuiltinFailureKind` closes kinds 1–5). A first-round contract review
(Ariadne FAIL, transcript preserved) corrected two design errors, both
repaired in-tree (see §7): the initial single-operand shape violated the
frozen scope/request arity (the `call_direct operands=N` analogy invoked
for it was invalid and is withdrawn), and fuel was charged after the arm
ran. No new opcode, no new failure kind, no `SLEYBC02` layout change
(161 with an `Entity` immediate already encodes), so `lowerer_version`
stays `[2, 0, 0]` and no schema epoch is required — the same determination
class as slice E7a. `adapter_abi_entries` stays 0 (entries are
profile-pinned, take no adapter configuration).

Entries are genuine epoch-1 `AdapterImport` values (identity, adapter
identity, ABI version 1, exact scope/request/response/failure types,
empty effect list — entries are pure value functions, so no `AdapterCall`
effect applies), resolved from the supplied import inventory:
`LoweringInput` carries `adapters` (S20-230 requests carry their real
import sets; the SMP1 path carries none, staying closed), and judgment,
execution, and fuel accounting all resolve the immediate against that
inventory, pinning every frozen field. A frozen identity with no carried
row, a foreign carried row, or a row with a tampered adapter identity,
ABI, failure type, effect list, or (for conversions) request/response
stays `VM_LOWER_OPCODE_UNSUPPORTED`. Scope mirrors the reference-adapter
convention (state acted upon; `Unit` where stateless): B2V
`[Unit, Bytes]`, V2B `[Unit, Vector<UInt(8)>]`, push `[Vector<E>, E]`
generic over `E` (the carried push row pins identity and purity while
types derive per use from the operands). The identities are REWEAVE
host-ABI import identities (`SLY1/BRIDGE/` + code, RW-070 freeze records
them), not reference-adapter identities — stated in contract, not
smuggled. [Superseded permission basis: the profile-local permission
note below was replaced at re-landing by owner amendment A1 (S20-230
§1.5); the push relationship pin below is now enforced at all layers
(see §9). Preserved as investigation history.] Permission note
(profile-local, reviewed): pure effectless
imports are permitted exactly these three frozen rows (S20-230 §1.4's
zero-effects rationale does not attach to authority-free value functions;
E7a predicate precedent); effectful adapters keep their owners and the
profile's effectful-Function refusal is untouched. S20-240 full / S20-280
full / S20-380 full keep every other adapter use.

## 2. Frozen values (profile section E8, contract revision 13)

- Entries: `SLY1/BRIDGE/` + `B2V1` / `V2B1` / `PSH1`, zero-padded to 32
  bytes; exact schemas in the E8 table (`Vector<UInt(8)>` exact — no
  other width; push generic over `T`, byte-unaware).
- `BRIDGE_MAX_ITEMS` = 1,048,576 (2^20, symmetric with
  `MAX_EXECUTION_CELLS`); single values past 1 MiB cannot cross in one
  call — profile bound, raise needs contract review.
- Capacity refusal: `Err(BuiltinFailure(Index, 2))`. The admission's
  "typed Limit failure" is realized as `Index` code 2 (kinds epoch-closed,
  codes per-kind values; capacity belongs to the collection-bounds family;
  code 1 stays index-out-of-range per the `VectorSet` precedent).
- `BRIDGE_ELEMENT_FUEL` = 1 per request element (conversions) or push,
  charged UP FRONT through `charge_action` before the arm allocates or
  converts, so a starved budget terminates without the work (the E6
  call-fuel precedent). Refusal paths allocate nothing (cap checked
  first). Per-instruction, per-terminator, and call-site value-unit
  charges unchanged.
- Threat posture: T25 impossible (identity = frozen table; unapproved
  `Entity` → `OpcodeUnsupported` at judgment); T26 impossible (schemas
  pinned at judgment, data shapes rechecked at execution,
  mismatch = internal fault). No new stable error codes
  (`new_stable_error_codes` stays 0).

## 3. Workloads (production path, 29 vectors)

Emitter `emit_vm_extended_vectors_for_fixture_refresh` + generator +
`--check` + oracle over 29 vectors (26 + 3, subject op 161):
- `bridge-bytes-to-vector`: SCB1-`encode_bytes` payload crosses to a u8
  vector (decode-read path; real codec bytes in, representation out).
- `bridge-vector-push`: 2-vector + octet → 3-vector at runtime (length
  decided by execution, not lowering arity).
- `bridge-vector-to-bytes`: u8 vector → bytes (emission path).
Chaining composes across executions (Rust-side two-execution test grows
1 → 2 elements); no unwrap op exists by design, so straight-line fixtures
cannot consume a `Result` — stated, not worked around.

Unit tests (6 fns): happy paths + judgment acceptance all entries,
18-case rejection matrix (genuine resolution: frozen-id-without-row,
foreign carried row, tampered adapter-id/request/effects rows, and
unapproved→`OpcodeUnsupported` with resolution before arity pinned;
non-Entity→`ImmediateMismatch`; scope/request mistypes→`SignatureMismatch`;
restricted profile refuses 161), over-cap `Err(Index, 2)` with at-cap
success boundary, exact fuel (1 + 100 + 1 terminator = 102) with starved
`ResourceLimit(Fuel)` that now terminates before any conversion work,
program-bytes-cross-as-vectors-only negative (SCB1 bytes in, `Sequence`
out, lengths equal, no verdict shape), 128× repeat determinism.

## 4. In-flight repairs (owned scope, diagnosed in-flight)

- `lint` target was red on pristine `c6e4269` (nested `fn operation`
  after statements in `drain_loop_fixture`; RW-040 gated lib-scoped
  clippy only): pure move above the statements, no semantics.
- Independent oracle family set +161 (diversity arm kept independent:
  container/identity only, still no semantic judgment).
- Checker `check_vm_extended_opcode_profile.py`: E8 slice, `[161]`,
  section + pin markers, `e7_excluded` → `PARTIAL_E7A_E8_LANDED`.
- Generator EXPECTED 29 + claim `e1-e6-e7a-and-e8`.
- machine-summary: E8 IMPLEMENTED (3 vectors, 6 tests), rev 13,
  `e8_epoch_determination`, SBOM `dependency_relationships` 120→121
  (dev-dep edge).
- Supply-chain regen via owned builder (required: T52 inventory gained
  the workspace-internal sley-vm→sley-scb1 edge + lock hash): T54
  regenerated clean, findings `[]`, blockers `[]` — the carried drift
  was stale counters (806→846 files), verified, not waived. Closing
  order honored (supply-chain last; provenance/dossier/SBOM rebound
  after; namespace re-bound to final T52).
- `Cargo.lock` +1 (path dev-dep `sley-scb1` for authentic SCB1 payload
  bytes in tests; no external version changes; `--locked` green).
- Genuine inventory resolution (Ariadne round-2 HIGH, accepted):
  `LoweringInput` + `LoweringContext` + `ExecutionContext` carry
  `adapters`; judgment/execution/fuel all resolve against the supplied
  inventory; frozen rows are real `AdapterImport` values; fixtures carry
  them (SMP1 path carries none, closed; candidate-validation passes its
  real S20-230 sets, still refused unless frozen). The +16-byte struct
  growth tripped `large_types_passed_by_value` (pre-existing 248-byte
  struct sat 8 bytes under the lint cliff): 16 private fns converted to
  borrow (public signatures untouched; `LoweringInput` already `Copy`).
  Pre-existing `lint` red on pristine `c6e4269` repaired by pure move.
- Supply-chain mirror rebinding (Nabu round-1 MEDIUM, accepted): the
  `s20_710_pre_release_audit` mirror carried a hash/edge count already
  stale by one at `c6e4269` (120 T52 edges vs pinned 119); the lock
  change extended it. Repaired: mirrors rebound to the true T52 values
  (hash `887d1460…`, 121 edges = SBOM DEPENDS_ON), checker constants
  advanced (lock-change maintenance, not a waiver), checker hardened
  with explicit T52↔summary reconciliation (live combo reconciles; both
  stale combinations caught standalone). Closing builder order honored
  (supply-chain regen dead last; SBOM/provenance/dossier rebound
  after each content change).

## 5. Validation on the landed tree (reconciled; see §9)

- `cargo test -p sley-vm` 45/0 (1 ignored emitter, by design);
  `cargo test --workspace --locked` 39 binaries ok, 0 failures.
- `cargo clippy --workspace --all-targets -- -D warnings` clean
  (includes the pre-existing repair above); `cargo fmt --check` clean.
- `make conformance` exit 0 (oracle 29/29, staged checker, all families).
- `make quick`: exactly ONE failure — the carried reproducibility-report
  staleness (attestation `84bfa9c9…`, same 3-surface class; release-lane
  owned, untouched). The carried secret-scan drift is RESOLVED by the
  owned-builder regen above (standalone `check_supply_chain_audit`
  PASS: T52 PASS, T54 PASS with zero findings); `bench/release/tests`
  59 OK; SBOM/provenance PASS; decision_state stays BLOCKED.
- RW-030/RW-040 evidence untouched (checker detections unchanged;
  readiness still NOT_READY with G-10 open pending this slice's review).

## 6. Follow-ups (not claimed)

- E8 lane in the `vm_canonical_inputs` persistent fuzz slice + seed
  counts (S20-700 harness): RW-050 slice 2.
- BOOTSTRAP_PROFILE_1 whole-freeze record with permitted-import entries:
  RW-050 slice 2 (this slice is its G-10 input).
- RW-070: entry ABI/import freeze + compiler-service denial tests.
- RW-090: Sley codec byte/error parity against `sley-scb1`.

## 7. Reviews (transcripts in `reviews/`)

- Ariadne (contract-adjacent): E8 contract fidelity (schemas, cap,
  charges, failure mapping, anti-shortcut, manifest relationship),
  no invented imports, frozen-table reasoning sound.
  Round log: `reviews/reweave-rw050-ariadne-2026-09-06.log`.
- Nabu (architecture/routing): carrier selection, remedy routing
  (RW-070/RW-090 follow-ups recorded), checker/oracle repair scope,
  no new gate framework.
  Round log: `reviews/reweave-rw050-nabu-2026-09-06.log`.
- Reviewers are not the slice owner; failed rounds preserved with
  focused repairs (round 1 below; re-review requested after repair).

Rounds:
- Ariadne round 1 (2026-09-06,
  `reviews/reweave-rw050-ariadne-2026-09-06.log`): FAIL with one HIGH
  and one MEDIUM, both accepted on the evidence. HIGH: the initial
  single-operand shape violated frozen op-161 scope/request arity and
  bypassed `AdapterImport` resolution (the `call_direct` analogy
  withdrawn). MEDIUM: per-element fuel charged after the arm ran, so
  starved budgets did the work before terminating. Repairs: frozen
  `[scope, request]` invocation shape with `AdapterImport`-structured
  frozen entries (identity/ABI-1/types, empty effects for pure entries;
  push generic over `E`); surcharge moved before arm execution (E6
  up-front precedent), refusal paths allocate nothing; contract section
  E8 rewritten; vectors, tests, and 14-case matrix regenerated to the
  new shape. Re-review requested.
- Nabu round 1 (2026-09-06,
  `reviews/reweave-rw050-nabu-2026-09-06.log`): FAIL with one MEDIUM —
  supply-chain evidence internally inconsistent (summary mirror stale
  hash/count vs T52; checker pinned constants without reconciling).
  Three INFOs accepted (carrier sound, routing intact, Ariadne FAIL
  handling honest). Repair: mirrors rebound, constants advanced,
  reconciliation added (§4). Delta re-review requested.
- Ariadne round 2 (2026-09-06,
  `reviews/reweave-rw050-ariadne-r2-2026-09-06.log`): FAIL, still HIGH —
  effectless imports need genuine `AdapterImport` modeling from a
  supplied inventory (private table + hardcoded identities insufficient;
  push outside even that table), or an explicit reviewed
  contract/epoch change permitting profile-local effectless imports.
  MEDIUM (pre-charge) confirmed repaired in-tree. Repairs: genuine
  inventory resolution above + explicit profile-local permission note
  (§1) + 18-case matrix with tamper rows. Round 3 requested.
- Ariadne round 3 (2026-09-06,
  `reviews/reweave-rw050-ariadne-r3-2026-09-06.log`): FAIL with two
  HIGHs, both accepted as binding. HIGH 1: effectless rows are invalid
  epoch-1 imports — verified in code (`validate_adapters` rejects
  zero-effect rows with `ADAPTER_EFFECT_CARDINALITY`; lowering refuses
  every effectful Function, so `AdapterCall`-carrying rows are
  uncallable without S20-230-full effectful lowering). The profile-local
  permission cannot override the owner validator: owner-level decision
  required (see §8). HIGH 2: push rows do not pin request/response —
  analyzed (relationship pin: require response==Vector<request>) but not
  implemented, moot once HIGH 1 bars landing. MEDIUM stays repaired.
  No round 4: the remaining blocker is not repairable in-tree.
- Nabu round 2 delta: NOT REQUESTED — the supply-chain repair it would
  re-review is reverted with the slice surface (§8); the MEDIUM's
  substance (mirror staleness incl. the pre-existing 119-vs-120, checker
  non-reconciliation) is recorded as findings for the supply-chain lane
  instead. Nabu round-1 FAIL stands as issued, dispositioned here, not
  silently closed.

## Closeout — RW-050 slice 1 status: POSITIVE on re-landing (§9)

(Original negative-result closeout, preserved:) The slice was complete
as an investigation and closed as a landing: the admitted bridge was
implemented end to end, but contract review FAILED at owner level —
effectless imports contradicted the S20-230 owner validator, and no
in-tree repair existed. The disputed surface was NOT landed: prototype
parked, `main` reverted plus record and transcripts.

Verdicts: Ariadne FAIL (binding, owner-level); Nabu FAIL (MEDIUM,
dispositioned via revert + lane findings).

## 8. Owner-decision packet — G-10 remedy blocked (2026-09-06)

Question for the operator (with S20-230 owner as needed): authorize one
of the following, or redirect the campaign. Facts established this slice
(all verified in-tree, transcripts preserved):

F1. Exactly one new Sley-observable primitive family is irreducible for
    the encode path (runtime-length vector growth; cf. §1): no existing
    op grows a vector, and multi-site `VectorNew` cannot cover unbounded
    lengths. Byte transport additionally needs a crossing mechanism.
F2. Every in-VM carrier is blocked without owner/epoch work: new opcodes
    need epoch work (frozen 55-tag table + manifest + S20-210 checker);
    effectless 161-imports fail the S20-230 validator
    (`ADAPTER_EFFECT_CARDINALITY`, `effects.rs:538`); effectful
    161-imports are uncallable (lowering refuses every effectful
    Function) without S20-230-full effectful lowering (R4 scope).
F3. The RW-030 §18 admission selected imports without checking
    owner-contract compatibility — the admission has a gap, not just the
    implementation. Any reselection needs an admission amendment, not a
    quiet edit (per the admission's own §5 rule).
F4. A complete prototype exists (branch `rw050-slice1-prototype`):
    genuine inventory resolution, pre-charge fuel, 2^20 cap, Index code
    2, 29 vectors, 45/0 unit tests, Tier-1 green except carried repro.
    It re-lands nearly as-is if option (i) is granted (plus the push-row
    relationship pin: require response==Vector<request>).

Options and costs:
(i)  S20-230 amendment permitting pure effectless imports (narrow,
     rationale: no authority to hide + E7a precedent). Cost: owner
     review cycle. Unlocks the prototype.
(ii) Full E7 adapter enablement (effectful lowering + registry +
     capability judgment). Cost: R4-scale, multi-owner. Not R2.
(iii) Epoch change for new opcodes. Cost: manifest/checker/epoch
      migration across lanes. Not R2.
(iv) Driver-side marshaling for conversions (native byte-I/O remainder,
     zero profile/owner friction) + still-unresolved push carrier
     (needs (i) or (iii) for the one growth op). Partial.
(v)  Accept G-10 open through R2 (BOOTSTRAP_READY stays barred) and
     replan the bootstrap closure around the decision. Schedule cost.

Recommendation: (i), then re-review. The packet, prototype branch, and
transcripts are the complete handoff; nothing here preempts the owner.

Findings for lanes (no action taken on main):
- Supply-chain lane: summary mirror went stale twice (119 vs 120 T52
  edges at `c6e4269`; checker pins constants without reconciling
  summary↔T52). Recommended: T52↔summary reconciliation in
  `check_supply_chain_audit.py` (prototype implementation existed on
  the parked branch) + mirror-advance discipline on lock changes.
- S20-230 lane: §1.4/§3.3 assume all imports touch host state; pure
  value imports have no lawful form. Options (i)/(ii) above.

## 9. Re-landing under owner amendment (i) — slice POSITIVE (2026-09-06)

The operator / S20-230 owner granted option (i) narrowly (decision on
§8). The negative result above stands as permanent evidence; this
section turns the slice positive on NEW reviewed implementation
evidence only.

Method: reconciliation, not merge. The 13 source/tooling paths were
checked out from `rw050-slice1-prototype` (zero path overlap with the
amendment commit `fd94402`), then adapted; evidence was regenerated via
owned builders in closing order, never copied; vectors regenerate
byte-identical (29). The prototype branch is untouched history.

Amendment basis: S20-230 §1.5 (owner amendment A1, G-10-tied) + §3.3
pure arm + §§4/6/7/8 conformance; admission §7 (A1); checker pins.
Accepted: Ariadne FAIL/HIGH→PASS on repair (row-level declaration
enforcement), Nabu FAIL/MEDIUM→PASS on repair (order wording), commits
in `fd94402`.

Re-landing repairs beyond the prototype:
- Push-row binding (Ariadne HIGH-2) at three layers: S20-230
  declaration relationship + invocation binding (amendment); lowering
  resolve relationship + judgment operands-equal-row; execution
  revalidation of operand/result value types against the carried row
  (Ariadne Phase-2 HIGH-1 repair — the public helper cannot serve a
  registered identity with off-row types).
- `Index` code 2 validity end to end (Ariadne Phase-2 HIGH-2 repair):
  `check_builtin_failure` admits Index 1..=2 (kinds epoch-closed,
  codes per-kind; S20-210 delegates validity to kind; only widens);
  unit test (1+2 valid, 3 invalid) + capacity-value `check_constant`
  assertion.
- Profile E8 text: permission re-based on A1, push paragraph to row
  binding (revision 13 still unlanded, no bump).
- Phase-2 negatives: relationship-violating row, operands-outside-row,
  foreign ABI, foreign failure type, fourth bridge code (lowering);
  direct public-helper calls over empty/unknown/wrong-ABI/effectful/
  relationship-violating inventories fault, off-row operand/result
  types fault, frozen rows execute.

Reviews (transcripts in `reviews/`, all preserved, none
self-certified): `reweave-rw050a1-ariadne-*.log` (FAIL→r2 PASS),
`reweave-rw050a1-nabu-*.log` (FAIL→r2 PASS), `reweave-rw050b1-nabu`
(PASS), `reweave-rw050b1-ariadne` (FAIL 2 HIGHs→r2 PASS),
`reweave-rw050b1-nabu-r2` (delta PASS). Landing requires all green:
satisfied.

Validation on the landed tree: `cargo test --workspace --locked`
1076/0; clippy `-D warnings` clean; `cargo fmt --check` clean; `make
conformance` exit 0 (oracle 29/29, staged checker, all families);
`make quick` exactly ONE failure — the carried reproducibility-report
staleness (release-lane owned; secret-scan drift resolved by regen,
standalone T52/T54 PASS). G-10 CLOSED as the bootstrap blocker
(admitted bridge landed under A1); BOOTSTRAP_READY still barred by P
(unfrozen) and remaining RW-050 work, unchanged.

Follow-ups (unchanged from §6): slice 2 (E8 fuzz lane + seed counts,
BOOTSTRAP_PROFILE_1 whole-freeze record with permitted-import
entries); RW-070 (entry ABI/import freeze + compiler-service denial
tests); RW-090 (codec parity).
