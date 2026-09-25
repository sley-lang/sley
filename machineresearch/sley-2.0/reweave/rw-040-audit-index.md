# RW-040 audit index — slice 1 (2026-09-06)

Package: RW-040 baseline defects and bootstrap capability audit.
Depends on: RW-020 COMPLETE (adoption record
`evidence/release/operator-decision-ADOPT-REWEAVE-2026-09-06.md`).
Minimum gate: exact failing workloads and dispositions.
Owner (implementation): the authorized Muse/OpenCode REWEAVE workflow under
operator authority. Independent review (Ariadne/Nabu lanes per the accepted
proposal) is separate and has not happened for this slice.

## Requirement references

- REQ-FAM-EXEC-SEMANTICS (MG 7), REQ-RUST-BOOTSTRAP (OVR-02),
  REQ-BYTECODE-TARGET (OVR-12), REWEAVE §7.2 (exercise list),
  REWEAVE §21.3 (change invalidation baseline), REWEAVE §13 (S/P/H/C0–C3).

## Slice 1 contents (this directory)

- `rw-040-m2-gap-list.json`: every M2-exit gap with a concrete failing
  workload or a VERIFIED-present disposition with evidence identity.
- `rw-040-bootstrap-exercises-slice-1.json`: the eleven §7.2 exercises,
  each PASS-exists / PARTIAL / FAIL-missing-workload with the executed
  workload or the exact missing piece. Workloads executed 2026-09-06 via
  `scripts/check_vm_extended_opcode_profile.py` → PASS (revision 12,
  slices E1–E6+E7a landed) over `conformance/vm-extended/v1/accepted.json`
  (22 vectors, each with bytecode_sha256 + observation identity).
- `bootstrap-manifest.json`: skeleton. S/P/C0–C3 null (nothing exists to
  hash; hashes never invented). H carries measured host facts with sources;
  its manifest digest stays null until RW-030 pins the host record.

## Method (evidence reuse, per §24.2)

No new semantics, crates, contracts, or toolchain changes in this slice.
Family semantics are cited from
`docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md` (revision 12); execution facts
from the fixture + checker run above; restriction facts from
`machineresearch/sley-2.0/machine-summary.json` (phase M2, status
IN_PROGRESS). Anything not executed is marked PARTIAL or FAIL-missing, never
upgraded by document reading.

## Slice 1 results (summary)

- Exercises: 8 PASS-exists, 2 PARTIAL (loops/recursion: bounded recursion
  only; deterministic traversal: ordering only), 1 FAIL-missing-workload
  (byte processing on Bytes/Text operands).
- Gaps: 9 enumerated (G-1..G-9); 3 owned by RW-040 slice 2 (new workloads,
  no code), 4 routed to RW-160/RW-200/Council lanes (proposed, Nabu to
  confirm), 2 recorded non-goals/skips (by-design absences; succession
  BLOCKED_EVIDENCE without spend authority).
- Full-vs-restricted per workstream: checker (E1/E2/E4/E7a landed; E7
  effects/capabilities/adapters excluded → RW-160), lowerer/VM (EXTENDED_V1
  lands E1–E6+E7a; restricted 52-opcode gap superseded for bootstrap scope;
  full GA gap → RW-160), adapter (E7 adapter handles excluded → RW-160),
  repository (impact semantics unfrozen → RW-160 proposed; full recovery
  open → RW-200 proposed).

## Open for slice 2

- Bytes/Text order-predicate workload (exercise 1) and a traversal workload
  (exercise 8): new fixture vectors through the existing generator, no
  semantic change.
- Loop-form mapping (exercise 7): which terminators express iteration.
- `scripts/check_bootstrap_capability.py` staged checker + fixtures under
  `conformance/bootstrap-capability/v1/` (proposal-owned paths).
- Nabu routing review for G-4/G-7; Ariadne coverage review for the exercise
  list (proposal items 55–56).
