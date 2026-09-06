# ADR-0049: REWEAVE-1.0 scope adoption (SH2 campaign path)

Status: accepted 2026-09-06 by operator decision ADOPT-REWEAVE-2026-09-06.
Contract review (Ariadne) PASS, architecture review (Nabu) PASS round 2,
security-surface review (Vulcan) PASS round 2; transcripts in
`machineresearch/sley-2.0/reviews/reweave-adoption-*-2026-09-06.log`.
Implementation under this ADR is the governance amendment only; engine
promotion stays a separate future authorization (REWEAVE section 14).

Date: 2026-09-06

## Context

`CONTRIBUTING.md` prohibited adding self-hosting work to the 2.0 GA path,
and `docs/ANTI_GOALS.md:30` denied native/JIT/AOT/marketplace/self-hosting
before GA. The mechanical checker (`scripts/build_anti_goal_conformance.py`)
scanned `Cargo.lock` only for six codegen crates
(`cranelift, inkwell, llvm-sys, wasmtime, wasmer, cranelift-jit`), so it
never enforced the prose self-hosting ban. Both texts blocked the REWEAVE
SH2 campaign path (RW-030 and all R2+ implementation).

The operator adopted REWEAVE-1.0 as the dispatch goal for the sley2 lineage
under the previous controlling policy (decision ADOPT-REWEAVE-2026-09-06,
recorded verbatim in
`evidence/release/operator-decision-ADOPT-REWEAVE-2026-09-06.md`). This ADR
is the append-only record of that adoption. It supersedes no prior ADR; it
narrows the prior policy texts named below, whose pre-adoption wording is
preserved in git history at `9b7054c`. The adoption binds to the exact
reviewed request and diff:

- request `reweave-r1/adoption-request.json` sha256
  `f6348b25fd65ede6cbff76adbae2ffeb418a4fff0d8843091117610f14e6f103`
- reviewed diff `independent-review/proposed-governance-diff.md` sha256
  `d2ce79d5a8a90517c3fbee2d466a4fc2295cb64e6d0cdb002969846ee0f0f937`
- delta review `independent-review/review-addendum-rev2-2026-09-06.md`
  sha256 `52fd47c51914f892f972ebe513af2dfbba7070979cc6a2fa59a5e49decd0c52c`

No revised request or expanded diff was substituted.

## Decision

1. **C-01 narrowed to the frozen line.** The CONTRIBUTING prohibition keeps
   every clause and now ends at the 2.0 GA path: self-hosting toolchain work
   adopted under REWEAVE-1.0 (SH2 campaign) is governed by
   `host-boundary.json`, `BOOTSTRAP_PROFILE_1`, and the staged SH2 gates
   instead of that paragraph. Every other prohibition in the paragraph is
   unchanged (Ariadne-verified byte-identical).
2. **C-02 re-scoped to unauthorized campaigns.** The anti-goal row now reads
   "native/JIT/AOT/marketplace/self-hosting outside an authorized campaign",
   evidenced by absence from the GA dependency graph unless declared in the
   authorized campaign boundary record (`host-boundary.json`) and gated by
   staged SH2 proofs.
3. **The checker is additive only.** The six-crate denylist still fails
   unconditionally. A review-gated campaign-declaration check
   (`evaluate_campaign_declarations`) additionally requires every declared
   SH2 work item (`evidence/reweave/sh2-work-items.json`) to cite the
   boundary record by exact path and SHA-256 digest and to name a staged SH2
   gate as a non-blank string; every failure mode (missing, unreadable,
   wrong-contract, or miscited registry; absent or unreadable boundary;
   digest mismatch; unnamed gate) violates. No SH2 work is declared today,
   so the check holds vacuously. Semantic authorization of SH2 work stays
   with human review; the checker verifies citation, not permission.
4. **C-03 recorded.** REWEAVE-1.0 is the internal campaign identity; 2.1.0
   is the naming proposal with availability unverified. Only
   version/publishing operations are affected; no version or release byte
   changes under this ADR.
5. **What this ADR does not do.** No engine promotion, no semantic or
   production change, no checker weakening, no provider spend, no
   publication, deployment, release, tag, or push, no SH3, no full MCR1
   (OVR-07 stands), no compiler rewrite, no retroactive approval, no blanket
   permission. The current implementation stays the bootstrap/reference
   foundation. SH2_SELF_HOSTED_TOOLCHAIN and amended WITNESS are required
   campaign outcomes; the RW-240 Coherence design-only audit stays required.

## Consequences

- RW-010 closeout is unblocked: the protected adoption record is the
  operator-decision file named above. RW-030 may start; RW-040 is
  dependency-ready once RW-020 finalizes. RW-050 still requires RW-030 and
  RW-040 both.
- RW-030 charters `host-boundary.json`, `BOOTSTRAP_PROFILE_1` references,
   and the staged SH2 gate set; until then any declared SH2 work item fails
   the campaign check. Validating a gate name against the authorized gate
   set stays review-owned until RW-030 makes the set mechanical.
- The dossier stays derived-not-decided (ADR-0043): this adoption changes no
   GA release-decision input, and the regenerated dossier is byte-identical.
- Ownership and residuals are bound in the operator-decision record:
   accountable operator authority, implementation in the authorized
   Muse/OpenCode workflow under that authority, independent review kept
   separate; R-1/R-2 assigned future work, R-4 recorded external condition,
   R-5 discharged by the delta addendum.
