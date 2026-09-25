# RW-030 charter — MPI-0 controls + host-boundary (2026-09-06)

Package: RW-030 (depends on RW-020 COMPLETE). Minimum gate:
boundary/constitution review. Slice 1 (entry inventory,
`rw-030-entry-inventory.md`) verified NOT_STARTED-but-ready; this slice
charters. Artifacts: `host-boundary.json` (repo root — the exact path
the campaign-declaration consumer reads), `rw-030-g10-admission.md`
(§18 record selecting the G-10 remedy).

## 1. MPI-0 control map (control -> enforcement -> review -> lifecycle)

MPI-0 is the BLACKGLASS constitution identifier (REWEAVE §1.2). Repo
carriers: `CONTRIBUTING.md` C-01, `docs/ANTI_GOALS.md` C-02, both as
amended at adoption `3fc2275`.

| Control | Enforcement (real, today) | Review lane | Later lifecycle evidence |
|---|---|---|---|
| Six-crate native-codegen denylist (C-02 matrix row) | `build_anti_goal_conformance.py`, unconditional FAIL, runs in Tier 1 (`make quick`) | Council review of matrix changes | Supply-chain gates RW-270; final review RW-280 |
| Campaign-declaration rule (declared SH2 items cite `host-boundary.json` path+digest + staged gate) | same checker, vacuous-pass while no registry exists (no items declared = nothing authorized) | Human review authorizes semantics (checker docstring states citation-only) | `sh2-work-items.json` items as declared; RW-280 |
| SH2 boundary content (`host-boundary.json`, this charter) | digest-citation binding on declaration | Boundary/constitution review this slice (Ariadne + Nabu, §5) | RW-070 ABI/import freeze; RW-080 closure designation |
| Prose bans (source syntax/LSP/native backend/marketplace/self-hosting outside campaign) | review lanes (checker cannot judge prose; boundary proposal §Governance preconditions says so explicitly) | Same lanes, now with a chartered boundary to enforce | RW-280 final candidate review |
| Gate-name set (chartered here: `host-boundary.json` `gate_set` from REWEAVE §19 phases, §21 targets, §20 package gates) | checker requires a non-blank gate per declared item; namespace fixed by this charter | Charter review this slice; set changes need charter amendment (§4) | item-to-gate closure at RW-130/RW-220 |
| Threat register (55-threat M0 map) | `build_threat_coverage_report.py` plan-vs-tree measurement (RW-020/Council owned; untouched here) | independent security review (owns the work list) | per-threat evidence paths; RW-230 |

Manifest-only vs runtime confinement (explicit non-claim): the
anti-goal checker constrains the dependency graph and declared-work
citations. It is NOT runtime import confinement. Runtime confinement
is the RW-070 positive import manifest plus compiler-service import
denial tests plus the frozen host ABI. No manifest-only test is
described as confinement anywhere in this charter.

## 2. Boundary summary (full map in `host-boundary.json`)

- §10.2 items 1–6 map to intended Sley owners (codec RW-090, semantic
  checker RW-100, Witness checker RW-170/contracts RW-150, lowerer +
  image builder RW-110, build driver RW-120, discharge RW-180), each
  with its native reference seed named as seed, never as owner.
- §10.3 remainder maps to explicit responsibilities over actual crates
  (`sley-vm` execution/accounting/loading; `sley-store`/`sley-txn`/
  `sley-state-root`/`sley-repo` persistence; `sley-protocol`/
  `sley-adapter` framing; `sley-repo`/`sley-query`/`sley-mutate`
  extra-closure repository work; `sley-conformance` oracles/drivers).
  Allocation/accounting/primitive-ops are `existing` over `sley-vm`;
  cancellation/process containment are split out as `later-qualified`
  (no in-tree mechanism; RW-070 confinement scope).
- G-10 remedy chartered: bounded lossless representation bridge
  (`Bytes` <-> `Vector<UInt(8)>`, exact element type) plus one admitted
  minimal bounded generic growth op (`vector-push`, generic over `T`,
  byte-unaware, carrier frozen at RW-050/RW-070 — not pre-frozen here).
- Surfaces are marked `existing` (in-tree), `proposed` (chartered
  intent), or `later-qualified` (needs a future freeze/review: Witness
  checker, discharge implementation, ABI freeze, crypto-library pin,
  containment/confinement rules, closure designation).
- No implemented import is invented: exact operation tags, schemas,
  and the authorized import set freeze at RW-070, which retains that
  obligation (charter states it; `host-boundary.json` `abi_freeze`).
- §10.2(6) carve-out retained verbatim in effect: crypto primitives
  and optional bounded solvers may stay explicit bounded deps; discharge
  judgment stays language-owned; no Sley hand-rolled crypto.
- §10.4 distinctions recorded: giant-opcode ban, compiler-service ban
  with RW-070 denial tests, verifier-vs-checker split.

## 3. Staged checker phase-correctness (repair owned by this slice)

Defect found: `check_bootstrap_capability.py` looked for the charter at
`reweave/` and `evidence/reweave/` while the enforcing consumer
(`evaluate_campaign_declarations`) reads repo-root `host-boundary.json`
— a landing at either checker path would read "chartered" while the
real gate still fails, and vice versa. Repaired within scope: charter
candidates = repo root only (the enforced path); RW-040 audit evidence
unchanged (its verdict never depended on charter detection).

Phase rule repaired: readiness previously required S/P/C0/C1/C2/C3 all
bound, making BOOTSTRAP_READY depend on R3 artifacts (S program root at
RW-080, C-images at RW-120+, fixed point/seed-absence at RW-130) that
can only exist after readiness. Now BOOTSTRAP_READY-proxy requires
exactly the R2 preconditions: audit rows complete, no open blockers,
P frozen, boundary chartered, H manifest pinned to the boundary bytes.
S/C0–C3 stay reported as explicit later-stage info, never as reasons.
Audit PASS coexists with documented blockers (unchanged). Corpus:
synthetic positive now proves READY with S/C-stages null (later
evidence not required early); new negative proves H-unpinned blocks
even when chartered.

## 4. Protected-control changes

Protected controls (denylist, prose bans, gate namespace, this
boundary) change only through explicit operator scope amendment with
review (REWEAVE §18: destination/remediation-scope changes need
amendment; routine choices inside frozen contracts do not). No
implementation package may weaken a gate to pass; this package passes
its criteria with gates intact (§6).

## 5. Reviews (boundary/constitution; transcripts in `reviews/`)

- Ariadne (constitution/contract-adjacent): boundary map fidelity to
  §§10–11, admission contract soundness, no invented imports, carve-out
  intact. Round log: `reviews/reweave-rw030-ariadne-2026-09-06.log`.
- Nabu (architecture/routing): §18 admission validity, remedy routing
  (RW-050 implement / RW-070 freeze / RW-090 parity), checker repair
  ownership/scope, no new gate framework. Round log:
  `reviews/reweave-rw030-nabu-2026-09-06.log`.
- Reviewers are not the slice owner; failed rounds preserved with
  focused repairs (round 1 below; re-review requested after repair).

Rounds:
- Ariadne round 1 (2026-09-06,
  `reviews/reweave-rw030-ariadne-2026-09-06.log`): FAIL with one HIGH
  and two MEDIUM blockers: (HIGH) bridge closes access but not
  construction — no path builds a variable-length output octet vector
  (`vector_new` arity-fixed, `vector_set` cannot grow); (MEDIUM)
  bridge type inexact (`Vector<SInt>`, width omitted, conflicts with
  `u8vector` naming); (MEDIUM) allocation/accounting/cancellation/
  containment collectively `existing` while cancellation/containment
  have no in-tree mechanism. One INFO accepted (boundary map, §10.4
  bans, carve-out, RW-070 freeze, import distinction all retained).
  Repairs: admission selects bridge + one minimal bounded generic
  growth op (`vector-push`, generic over `T`, byte-unaware, carrier
  frozen at RW-050/RW-070) with chunked-encode-import alternative
  considered and rejected on record; exact element type
  `Vector<UInt(8)>` in admission §2–§3 and `host-boundary.json`
  `bridge_g10.element_type`/`admitted_growth`; native remainder split
  into `existing` allocation/accounting/primitive-ops vs
  `later-qualified` cancellation/containment. Re-review requested.
- Ariadne round 2 (2026-09-06,
  `reviews/reweave-rw030-ariadne-r2-2026-09-06.log`): PASS, no
  blockers. All three round-1 defects confirmed closed; one LOW prose
  nit (`range 0..256` ambiguity) repaired in-tree post-verdict
  (admission §2(iii) now names the `UInt(8)` type authoritative; no
  semantic change).
- Nabu round 1 (2026-09-06,
  `reviews/reweave-rw030-nabu-2026-09-06.log`): FAIL with one MEDIUM —
  admission §3 routed `vector-push` into the RW-070 import manifest
  unconditionally, inexact if RW-050 selects an E-profile opcode (not
  a host import). Four INFOs accepted (admission sound, sequencing
  RW-050/RW-070/RW-090 correct, checker repair correctly scoped,
  gate_set creates no new framework). Repair: §3 now states the two
   conversions always enter the manifest while push enters it only if
   selected as a host import; boundary bytes untouched. Delta
   re-review requested.
 - Nabu round 2 delta (2026-09-06,
   `reviews/reweave-rw030-nabu-r2-2026-09-06.log`): PASS, no blockers.
   Amended §3 routing confirmed exact; supersedes round-1 FAIL.

## 6. Tier-1 carryover (no waiver)

Two `make quick` gates fail identically on pristine `b23c263`
(RW-040 evidence, retained): reproducibility-report staleness
(attestation `84bfa9c9…` predates S20-330 protocol changes; owner:
release lane, re-attestation) and supply-chain secret-scan drift
(owner: supply-chain lane). Both are outside RW-030 scope and
untouched. The full quick gate is NOT relabeled PASS; affected-scope
validation for this slice is green (§7). Release/supply obligations
stand.

## 7. Validation on the slice-2 tree (actual runs, 2026-09-06)

- `python3 scripts/check_bootstrap_capability.py`: audit PASS;
  readiness NOT_READY with exactly `exercises-incomplete:['ex1']`,
  `open-bootstrap-blockers:['G-10']`, `unbound-readiness-stages:['P']`
  (chartered true, H pinned; S/C0–C3 reported as later info).
- Corpus: all 9 cases match (positive READY with S/C null; H-unpinned
  negative NOT_READY; 7 audit-shape cases unchanged).
- `host-boundary.json` parses; raw-byte SHA-256 `d935d238…b18a` pinned
  in `bootstrap-manifest.json` H (anticipated by the RW-040 skeleton
  note; digest + path + pinned_by, no other manifest change).
- `make conformance` exit 0 (includes staged checker + oracle).
- `build_anti_goal_conformance.py`: PASS overall; campaign-declaration
  check still vacuous-pass (no registry); denylist unchanged.
- Derived regens via owned builders only: `independent-conformance-
  report.json` (corpus fixture rebinding; families stay 20,
  python_sources 23, decisions untouched), `anti-goal-conformance.json`
  (digest rebind; 9 HOLDS unchanged).
- In-flight failure repaired at source: the first SHA256SUMS regen wrote
  full relative paths, breaking the builder's bare-name contract (6
  packaging-test errors, `SUMS_MISMATCH`). Repaired to bare-name format
  in-tree; `bench/release/tests` 59 OK after repair. RW-040 audit
  evidence untouched by the repair (its verdict never depended on
  charter detection or corpus dashes).
- `make quick`: fails ONLY at the carried pre-existing
  reproducibility-report staleness (`84bfa9c9…`, same 3-surface list
  class); the carried secret-scan drift reproduces standalone with the
  identical drift file (`evidence/security/T54/secret-scan.json`).
  Packaging suite 59 OK post-repair. No new failure introduced.

## 8. Charter identity

- Record: `host-boundary.json`, contract `sley2.host-boundary.v1`,
  landed at repo root (enforced path).
- SHA-256 (raw bytes): `d935d238a4d75d154df128aad630411ca3fdcc18e4db084dcd2317ab73bdb18a` (pinned in manifest H;
  cited by future `sh2-work-items.json` items).
- Landing context: the RW-030 slice-2 commit on main, child of
  `040f3d1` (exact hash in `git log`); tree otherwise clean at commit
  time; `git log 040f3d1..HEAD --name-only` shows only the RW-030
  charter/checker/corpus/manifest/review paths (no `crates/` touch).

## Closeout — RW-030 COMPLETE (2026-09-06)

Acceptance against the charter criteria:
1. Boundary landed at enforced path (`host-boundary.json`, repo root),
   §§10.2/10.3 mapped to intended Sley owners / explicit native
   responsibilities, surfaces distinguished
   existing/proposed/later-qualified, no invented implemented imports,
   RW-070 freeze retained (`abi_freeze`).
2. G-10 admission complete (`rw-030-g10-admission.md`): bridge +
   admitted generic `vector-push` selected with constraints; no new
   capability implemented (nothing in `crates/` touched by this
   package — verified: `git log 040f3d1..HEAD --name-only` will show
   only charter/checker/corpus/manifest/review paths).
3. MPI-0 controls mapped (§1); gate namespace chartered from existing
   authority; manifest-only non-claim stated; protected-change
   mechanism named (§4).
4. Checker phase-correct with corpus +/- cases (9/9 match); RW-040
   audit evidence intact (its verdict never depended on charter
   detection).
5. Boundary/constitution review dual PASS — Ariadne round 2 PASS
   (supersedes round-1 FAIL), Nabu round-2 delta PASS (supersedes
   round-1 FAIL); all four transcripts in `reviews/`; no
   self-certification.
6. Tier-1 carryover recorded without waiver (§6).

Verdicts: Ariadne PASS; Nabu PASS. Status: COMPLETE. RW-050
prerequisites satisfied (RW-030 + accepted RW-040 closeout). Next:
RW-050 slice 1 implements the admitted bridge under §6 of the
admission record.
