# RW-030 entry state + control-surface inventory — slice 1 (2026-09-06)

Package: RW-030 MPI-0 controls and self-hosting/host-boundary charter.
Depends on: RW-020 COMPLETE. Minimum gate: boundary/constitution review.
Owner (implementation): the authorized Muse/OpenCode REWEAVE workflow under
operator authority. Independent review is separate and not yet requested
for this package.

## Completion check (independent, this date)

RW-030 is NOT_STARTED and NOT complete:
- No `host-boundary.json` at either logical path (`reweave/`,
  `evidence/reweave/`); staged checker reports `chartered_rw030: false`.
- No charter document, no MPI-0 control work, no boundary/constitution
  review recorded since adoption. All commits since `3fc2275` are RW-040
  audit evidence, fixture/checker additions, and owned-builder derived
  regens (verified via `git log 3fc2275..HEAD --name-only`).
- R1 accepted the SH2 boundary only as a proposal (`sh2-boundary-proposal.md`);
  the adoption record holds it as proposal and leaves gate-name-set
  validation review-owned until RW-030 charters the set.

Dependency-ready: YES (RW-020 COMPLETE; C-01/C-02 resolved by `3fc2275`).
RW-050 stays blocked until RW-030 closes (RW-040 COMPLETE as of `f625350`).

## Control surface inventoried (read-only; nothing chartered here)

1. Constitution source: MPI-0 = the BLACKGLASS constitution identifier
   (REWEAVE §1.2). Repo carriers: `CONTRIBUTING.md` (C-01 as amended),
   `docs/ANTI_GOALS.md` (C-02 as amended), `docs/THREAT_REGISTER.md`.
2. Amended prose controls: CONTRIBUTING "Prohibited additions" now routes
   adopted SH2 work to `host-boundary.json` + `BOOTSTRAP_PROFILE_1` + the
   staged SH2 gates — all three forward references are currently unbound
   (no file, no freeze, no gate set). The paragraph otherwise still bans
   the listed additions to the 2.0 GA path.
3. Mechanical control: `scripts/build_anti_goal_conformance.py`
   `evaluate_campaign_declarations` — six-crate denylist unconditional;
   declared SH2 items fail without `host-boundary.json` + digest match +
   non-blank gate set. Verified at adoption; untouched since.
4. Prose-ban enforcement: by review lanes, not by the checker (boundary
   proposal §Governance preconditions) — an RW-030 charter input, since
   the lanes need a chartered boundary to enforce.
5. Threat register: `docs/THREAT_REGISTER.md`, M0 planned-control map
   (55-threat scope per work-package mapping); evidence paths are future
   required outputs, not passing evidence. S20-740 register + threat
   coverage report are RW-020/Council owned; not touched here.
6. Gate-name-set remainder: validation stays review-owned until RW-030
   charters the set (adoption record). Open charter input, not a finding.
7. Native-remainder starting point: boundary proposal §§2–3 (permitted
   remainder list; anti-shortcut rule; semantic-ownership transition).
   Exact imports/owners/versions/semantics/limits/tests belong to the
   charter slice, not this inventory.

## Forward inputs already bound

- G-10 (RW-040): if the bridge alternative wins, RW-030 inventories the
  byte-access bridge in `host-boundary.json`; RW-070 later freezes its
  ABI/import details (Nabu round-2 INFO).
- RW-040 staged checker stays green while `host-boundary.json` is absent
  (its H-digest rule only constrains a claimed digest, and readiness
  correctly reports `rw030-not-chartered`).

## Slice 1 acceptance (this file only)

Entry state recorded with file-level evidence; control surface listed
without chartering anything; no enforcement added, no prose changed, no
`host-boundary.json` authored. Charter drafting + boundary/constitution
review are slice 2 under the package minimum gate, with independent
review (charter touches C-01/C-02 enforcement).
