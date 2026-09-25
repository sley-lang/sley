# Semantic dispositions — closure pass 2026-09-21 (work branch only)

Supplements `SUCCESSION-COVERAGE.md` (which remains the predicate-to-evidence
record). No silent closure: each item states the evidence, the exact residual,
and the gate holding it. `ga_claimed=false`.

## MODULE (S2B-MODULE-001) — export-grant evidence retained; equivalence review obligation open

- Evidence retained: export grant on `new_package` (86*32) via the surface;
  6 fixed inputs observed, reference_count 6, no-duplicate-impl extras green;
  rechecked 2026-09-21 (`trial_module_recheck.log`); negative (target-
  respecting no-op) → `ORACLE_STALE_IMPORT`.
- Corpus clauses at issue (`bench/corpus/v1/tasks.json` S2B-MODULE-001):
  "Move a deterministic checksum function from a utility namespace into an
  integrity namespace", required "namespace binding changed", "all references
  resolve". State evidence: before/after package exports (integrity-namespace
  binding ∅→{checksum} under the entity model's export-set visibility);
  function identity preserved (collateral-enforced); 6 CallDirect callers
  resolve identically; `old_package` (85*32) retained because the frozen
  manifest `targets` name only `new_package` — removal is FORBIDDEN by the
  frozen collateral rule, so no authored candidate can exhibit literal
  old-package removal.
- Disposition: the export-grant operation is proven. Whether it constitutes
  the corpus move (given the fixture's restrictive targets forbid the
  old-package removal a literal move would exhibit) is NOT closed by coverage
  prose: retained as an explicit review obligation with the source clauses
  above and the before/after state evidence. The fixture's target
  restrictiveness is never cited as proof of corpus equivalence.

## MERGE (S2B-MERGE-001) — ancestry corrected; failure narrowed to exercised configurations

- Corrected diagnosis retained: sides SHARE ancestor transaction history
  (base head tx bytes present in both packs — identity evidence; the judge
  docstring's "independent geneses" claim was wrong and is fixed).
- Production-path proof retained (`prove_merge_production.py`,
  `trial_merge_production.log`, runner-owned, no fixture changes): branch
  pointers fork at the ancestor; side histories co-locate by content-
  addressed union (+1 object +1 tx each); `merge.judge` REACHED and returned
  `MERGE_COMPARE_FAILED` (`COMPARE_ROOT_INCOMPLETE` family).
- Narrowing (no overclaim): the observed failure covers exactly the
  trial-shaped revisions exercised (MERGE/CREATE/TYPE bases, seeded AND
  committed) while extraction succeeds on harness-built repos. "All tested
  trial-shaped revisions failed" does NOT establish that every
  repository-backed merge path fails. Missing/inconsistent root facts traced
  to root-bindings alignment vs program projection inside extraction
  (failing trial path vs successful harness-built path isolates
  fixture/extraction/product responsibility; the gap is product/fixture work
  under review, not trial harness).
- The live candidate-validation acceptance stands on its own evidence only.
  It does NOT substitute for the original task's branch-change, semantic-
  merge, or selected-test requirements: those remain behind the
  complete-root fixture/product review gate, with the minimal reproducer,
  concrete proposed repair, and regression matrix behind that gate. S3
  (`s3_g2_merge`) proves merge semantics on synthetic complete sides.

## CORRUPT (S2B-CORRUPT-001) — PACK obligation open; exchange regressions separate

- Original PACK-layer obligation (corpus: `PACK_DIGEST_MISMATCH` via bundle
  import of a one-byte-corrupted canonical object) stays OPEN pending the
  explicit surface decision in `CORRUPT-SURFACE-DECISION.md` (frozen trial
  surface exposes no bundle-import operation; three reviewer options; judge
  self-test deliberately refused). No silent code mapping, no task
  substitution, no unrestricted import authority.
- Exchange-layer regressions preserved separately: 2 bit-flips → exact
  `EXCHANGE_DIGEST_MISMATCH`, destination ref/store unchanged
  (`_judge_corrupt_exchange`; import excluded from the agent allowlist by
  construction); constant-restore smoke; rechecked 2026-09-21; S3 G2
  conformance green (PACK_DIGEST_MISMATCH pinned at the bundle-import
  owner, never equated with the exchange code).

## PERF (S2B-PERF-001) — resource measurement preserved with exact units

- Owner definition (`crates/sley-vm/src/execute.rs`, `ExecutionLimits`):
  `max_value_units` = "Maximum monotonic semantic value units". Observed
  quantity per case: `ExecutionOutcome.peak_value_units` (driver-reported;
  enforced ceiling `DRIVER_MAX_VALUE_UNITS = 100_000`; missing telemetry →
  harness failure, never zero). Semantic value units are NEVER described as
  physical bytes or process memory; provider RSS, fuel, and constant zeros
  are never substituted.
- Evidence reused unless affected code/bindings change: 5x5 governing inputs,
  46-op scan → 7-op map (58-op record), outputs identical, reduction ≥30%,
  effects empty, fuel non-regressing; rechecked 2026-09-21
  (`trial_perf_recheck.log`, `trial_perf_memory.log`); flipped probe →
  `ORACLE_OUTPUT_MISMATCH`.

## Held gates (unchanged, independent work continues)

- TYPE corrected closure lives on the work branch; fixture-design review
  retained before any main adoption (original preserved as
  `task_manifest.v1-frozen.json`; corpus v1 unchanged; retired choices in
  `TYPE-FIXTURE-REVIEW-PACKET.md` §6).
- DEAD tombstone semantics: review-gated production semantic change
  (tombstone-aware test-plan selection); provider unavailable; gate retained;
  positive stays explicitly blocked.
- EFFECT/CAP rev16 handle-model decision: design/review gate; adapter work
  pending.
- Live-model campaign prerequisites unsatisfied (seeds/budgets preregistered,
  90-attempt minimum, two-host package/dossier, current source-bound Council
  transcripts). No campaign started. No acceptance campaign or GA claim.
