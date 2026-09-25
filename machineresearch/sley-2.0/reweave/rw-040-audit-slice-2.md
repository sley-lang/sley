# RW-040 audit — slice 2 (2026-09-06)

Package: RW-040 baseline defects and bootstrap capability audit (depends on
RW-020 COMPLETE). Minimum gate: exact failing workloads and dispositions.
Owner (implementation): the authorized Muse/OpenCode REWEAVE workflow under
operator authority. Independent review (Ariadne coverage, Nabu routing) is
separate; verdicts recorded below, transcripts in
`machineresearch/sley-2.0/reviews/`.

Slice 1 (`rw-040-audit-index.md`, retained as history) left 1
FAIL-missing-workload and 2 PARTIAL exercise rows plus the staged checker.
This slice resolves all three with executed workloads through the existing
production path, routes G-1..G-9 three ways, and lands the staged checker
with its negative corpus. No crate semantics, contracts, or toolchain
changed: test-emitter additions + fixture vectors are evidence.

## Workloads executed (ten PASS-exists, one PARTIAL-documented)

Production path for every vector: `cargo test -p sley-vm
emit_vm_extended_vectors_for_fixture_refresh -- --ignored` (each vector
lowers via `lower_function` and executes via `execute_function` under
`CacheProfile::EXTENDED_V1`, asserting `Success`) →
`scripts/generate_vm_extended_fixtures.py` (write) →
`--check` + `scripts/check_vm_extended_opcode_profile.py` → PASS
(revision 12, slices E1–E6+E7a, 26 vectors).

New in slice 2 (expected behavior confirmed, none invented):
- `bytes-less-than` / `text-less-than` (opcode 98, 1 instruction each):
  both observe Bool true, validated-value hash identical to the SInt true
  control (`db8444a3…`). Category: missing test supplied; the E1
  comparator path (`ordered()` + `compare_data()`) pre-existed.
- `vector-traverse` (VectorNew 3×u64 + VectorGet index 1, subject opcode
  34, 2 instructions): observes `Some(20)`. Category: missing test
  supplied. Recorded limitation: straight-line chains stop at
  Option/Result (no unwrap op); CFG-level deconstruction is the designed
  form (see loop mapping).
- `cond-drain-loop` (4-block SLEYBC02, bytecode_sha256 `3e080650…`, 8
  instructions): cell-threaded drain of `{7:"small"}` to `{}`, backedge
  taken once, `CondBranch` exit. Category: existing behavior demonstrated
  (backedge legality per `CFG_VALIDATION_V1.md`, fuel-boundedness per
  `execute.rs` `charge_action`).

Full rows: `rw-040-bootstrap-exercises-slice-2.json` (each row carries
`vectors` bindings + expected/actual + command + limits + category).
Loop forms/bounds: `rw-040-loop-form-mapping.md` (backedge loop and
bounded recursion both suffice; RW-050 need freeze only one; profile
limits to carry: 256-frame ceiling, fuel budgets, 1,048,576-cell cap, no
E7/unwrap dependence).
Gaps: `rw-040-m2-gap-list-slice-2.json` (G-9 CLOSED; G-10 OPEN as the sole
bootstrap-closure blocker — byte access/construction, RW-050 primary with
RW-030 bridge input, §18 admission barring BOOTSTRAP_READY; G-1/G-5/G-6
evidence gates; G-2/G-3/G-4/G-7 full-MG obligations with owners
RW-160/RW-200/Council/operator; G-8 non-goal with profile constraint). No
semantic/VM repair routed to RW-050 (zero defects demonstrated); every
final-product obligation keeps its owner. Nabu delta INFO recorded: if the
G-10 bridge alternative wins, RW-070 later freezes its ABI/import details.
Findings: `rw-040-findings-slice-2.json` (empty, findings.json field
shape; limitations recorded in exercise/mapping docs, not as findings).

## Staged checker (proposal-owned paths)

- `scripts/check_bootstrap_capability.py` (also in `make quick` and the
  `make conformance` recipe): bindings (vector-id membership + digest
  prefix match against `accepted.json`), coverage (all rows PASS-exists),
  gap routing (classification/owner/evidence enums), manifest structural
  honesty (no invented hashes, no stages without source, no H digest
  without charter), readiness computation. It never executes bytecode or
  judges opcodes. Live verdict: audit PASS, readiness NOT_READY with
  exact reasons (`unbound-stages:[S,P,C0,C1,C2,C3]`, `rw030-not-chartered`).
- Negative corpus `conformance/bootstrap-capability/v1/` (7 cases +
  SHA256SUMS, registered in `build_independent_conformance_report.py`
  COVERAGE/DEPTH as `codec_and_identity`): 4 structural FAIL shapes, 1
  bad binding, 1 unowned gap, 1 synthetic positive proving readiness is
  not rigged. All 7 match expected verdicts each run.
- Framework repairs the corpus registration required (all diagnosed as
  in-flight failures, fixed at source): COVERAGE + DEPTH entries,
  SHA256SUMS manifest (bare-name format), `make conformance` recipe line,
  regenerated derived `evidence/conformance/independent-conformance-
  report.json` (19→20 families, COMPLETE), `evidence/validation/test-
  inventory.json` (vm-extended 22→26 vectors, totals 360→364, new family
  row), `evidence/release/decision-dossier.json` (counts only:
  families 19→20, python_sources 22→23; decision_state stays BLOCKED,
  evidenced 24/gated 10 unchanged), `evidence/release/provenance.json`
  (conformance-report digest rebinding only; subject/commit untouched).
  Regeneration used only the owned builders; no ledger, requirement, or
  review disposition was edited.

## Validation on the slice-2 tree

- `make quick`: every line passes EXCEPT two failures proven pre-existing
  by pristine-`b23c263` reruns (stash evidence): reproducibility-report
  staleness (attestation `84bfa9c9…` predates S20-330 protocol changes;
  re-attestation is release-lane owned) and supply-chain secret-scan drift
  (identical drift file pristine). Untouched, out of audit scope.
- Affected Tier 2 green: `cargo test -p sley-vm` 39/0 (1 ignored emitter,
  by design); `cargo test --workspace` all 39 binaries ok, 0 failures;
  `cargo clippy -p sley-vm -- -D warnings` clean; `cargo fmt --check`
  clean; `make conformance` exit 0 (includes the new checker and the
  oracle over 26 vectors).

## Review requirements (proposal items 55–56)

- Ariadne (contract-adjacent): does the capability list cover the 10.2
  closure with nothing invented? Inputs: slice-2 exercises JSON, loop
  mapping, gap routing, boundary proposal §§1–3 + REWEAVE §§7.2/10.2.
- Nabu (architecture): are dispositions (repair vs profile work vs epoch
  question) routed to the right package? Inputs: slice-2 gap routing,
  work-package mapping, adoption record §§ ownership/residuals.
- Reviewer must not be the slice owner; failed rounds preserved with
  focused repairs (none yet — rounds below).

Rounds:
- Ariadne round 1 (2026-09-06, `reviews/reweave-rw040-ariadne-2026-09-06.log`):
  FAIL with one HIGH blocking finding: exercise 1 ordering vectors do not
  establish byte ACCESS/CONSTRUCTION for 10.2 items 1/4 (codec, image
  assembly). Three INFOs accepted (145 is `test_observe`; no-unwrap is fine
  via `VariantSwitch`+`CasePayload`; combos are unexercised, not missing).
  Repair: exercise 1 → PARTIAL-documented (ordering kept as demonstrated;
  access routed as gap G-10 to RW-050 with §18 admission path + RW-030
  bridge input); checker now accepts routed PARTIAL/FAIL rows (new corpus
  case `exercises-partial-routed.json`); G-2 refined (145=`test_observe`).
  Re-review requested.
- Nabu round 1 (2026-09-06, `reviews/reweave-rw040-nabu-2026-09-06.log`):
  PASS (high confidence), no blockers; one LOW note folded into G-4
  (RW-200 must integrate/revalidate the retained recovery foundation).
- Nabu round 2 delta (2026-09-06,
  `reviews/reweave-rw040-nabu-r2-2026-09-06.log`): PASS (high confidence).
  G-10 correctly classified BOOTSTRAP_BLOCKER routed RW-050 (+RW-030
  bridge input); G-2/G-4 refinements preserve round-1 PASS; nothing
  bootstrap-genuine deferred. INFO recorded: bridge selection later needs
  RW-070 ABI/import freeze (folded into the Gaps paragraph above).
- Ariadne round 2 (2026-09-06,
  `reviews/reweave-rw040-ariadne-r2-2026-09-06.log`): FAIL on one MEDIUM —
  this summary still claimed no §10.2 blocker (stale G-1..G-9 enumeration
  contradicting G-10). Exercise split, G-10 admission route, and
  exercises 2–11 all PASS/INFO. Repair: Gaps paragraph + RW-050 inputs
  corrected to name G-10 the sole open bootstrap-closure blocker.
  Round 3 confirmation requested.
- Ariadne round 3 (2026-09-06,
  `reviews/reweave-rw040-ariadne-r3-2026-09-06.log`): PASS, supersedes
  round-2 FAIL. Stale-claim repair confirmed; no new findings. RW-040 may
  close from Ariadne's scope.

## RW-050 inputs (evidence + one admission, not repair)

11-exercise evidence + loop mapping + constraints (no E7 test/effect ops,
no unwrap-op dependence, recursion/backedge as canonical iteration
candidate, 256-frame/fuel/cell bounds) + gap G-10 (§18 admission:
accessor-vs-bridge for byte access/construction). No defect reproducer
(no defect demonstrated).

## RW-030 note

RW-030 (MPI-0 controls + host-boundary charter) is dependency-ready
(RW-020 COMPLETE, C-01/C-02 resolved) but NOT commenced: no
`host-boundary.json` at either logical path, no charter, no
boundary/constitution review since adoption. Its R1 acceptance as a
proposal does not complete its control/charter obligations. RW-050
remains blocked until RW-030 and RW-040 both close. Next executable
slice after RW-040 close: RW-030 slice 1 (see closeout).

## Closeout — RW-040 COMPLETE (2026-09-06)

Acceptance against the accepted proposal:
1. Gaps: G-1..G-10 each carry a concrete executed workload, a
   VERIFIED-present disposition with evidence identity, an exact open
   state with owner, or a declared BLOCKED_EVIDENCE skip —
   `rw-040-m2-gap-list-slice-2.json`.
2. Exercises: ten PASS-exists with workloads, one PARTIAL-documented
   (ordering demonstrated, access routed as G-10) —
   `rw-040-bootstrap-exercises-slice-2.json` + `rw-040-loop-form-mapping.md`.
3. Full-vs-restricted per workstream with owners — slice-1 record sharpened
   by the slice-2 three-way routing (RW-050 vs RW-160 vs RW-200 vs
   Council/operator/non-goal).
4. `bootstrap-manifest.json` skeleton holds S/P/H/C0 identities with C1–C3
   null; the staged checker enforces the honesty invariants each run.
5. Tier 1 + affected Tier 2: green except two `make quick` gates proven
   pre-existing by pristine-`b23c263` reruns (reproducibility-attestation
   staleness, supply-chain secret-scan drift; both owned outside this
   audit and untouched). Affected Tier 2 green: sley-vm 39/0, workspace
   all-ok, clippy/fmt clean, `make conformance` exit 0. Findings in
   findings.json field shape (`rw-040-findings-slice-2.json`, empty: zero
   defects demonstrated).

Reviews: Ariadne PASS round 3 (supersedes round-2 FAIL; round-1 FAIL
historic, repaired in-tree). Nabu PASS round 1 + PASS round-2 delta.
Failed rounds preserved in `reviews/`; no self-certification.

RW-040 minimum gate (exact failing workloads and dispositions): MET.
Status: COMPLETE. G-10 stays open as RW-050/RW-030 forward work; that is
a dependency, not an audit remainder.
