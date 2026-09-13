# Nabu architecture review — finding register (contract rev 4)

Subject: S20-740 finding-register package (docs/spec/FINDING_REGISTER_V1.md rev 4,
scripts/build_finding_register.py CONTRACT_REVISION 4, scripts/check_finding_register.py,
bench/review/tests/test_finding_register.py, evidence/review/finding-register.json)
Reviewer: Nabu (architecture lens — derivation integrity, fixed-point stability, layering)
Commit: db1bc623d01e838d49c153feb0be05a7502b8794 (HEAD; working tree carries the
uncommitted rev-4 package under review — HEAD blobs still read rev 3, live files read rev 4)
Scope: full package (first Nabu review of this section; prior Vulcan re-review
vulcan_surface_review-a4b6029.md read REVISE with 2×P3)

## Verification transcript (all commands run, no file modified except this verdict)

- `git rev-parse HEAD` → `db1bc623d01e838d49c153feb0be05a7502b8794`. Working tree dirty
  (7 files: spec, builder, checker, tests, machine-summary, finding-register.json, dossier) —
  that dirty tree IS the rev-4 review target. `git show HEAD:scripts/build_finding_register.py`
  reads `CONTRACT_REVISION = 3`; live file reads `4`. HEAD commit message itself says
  "register 290/63 OPEN" (rev-3 artifact); live artifact reads 291 obligations / 52 open.
- `python3 scripts/build_finding_register.py --check` → mode check PASS, obligations 291,
  open_reviews 52, result FINDING_REGISTER_OPEN, exit 0. Write mode deliberately NOT executed:
  it would rewrite the tracked artifact (forbidden by the read-only constraint); `--check`
  exit 0 proves a write would be byte-identical, i.e. write-idempotence without writing.
- `python3 scripts/check_finding_register.py` → result PASS, problems [], status
  S20_740_REGISTER_IMPLEMENTED_REVIEW_PENDING, exit 0.
- `python3 -m unittest discover -s bench/review/tests -t .` → Ran 48 tests, OK (rev 3 ran 44;
  +4 are the rev-4 conjunction pins).
- Targeted paths (6/6 OK): fixed-point counter-stability, no-host-path, unclaimed-blocks,
  claimed-does-not-block, closed-does-not-block, mid-string-named-with-counts.
- Live artifact probe: contract_revision 4, obligation_count 291, states
  {HISTORICAL_ROUND 53, OTHER 3, PASS 183, PENDING 52}, severity mentions
  {P0 57, P1 101, P2 113, P3 131, P4 21}, unclaimed_carried_findings 39,
  mid_string_complete_packages 10, complete_packages 25,
  complete_packages_with_open_reviews [], result FINDING_REGISTER_OPEN.
- Host/timestamp probe of the artifact: `/home/`, `/greyforge/`, `file://` all absent;
  zero `20\d\d-\d\d-\d\dT` matches; hostname and user absent. New-field shapes exact:
  unclaimed rows {section, field, disposition, unclaimed_severities}; mid-string rows
  {section, status, open_obligations}.
- Prior-P3 repair closure on live data: reproducibility
  nabu_architecture_review `PASS_WITH_P1_P3_P4_FOLLOWUPS_NO_P0_P2` now sits in
  unclaimed_carried_findings (with the two ariadne siblings); s20_360 (3 open),
  s20_390 (3 open), mutation_value_profile (1 open) all named in
  mid_string_complete_packages with counts while violations stays [].
- Import/write-surface probe: builder imports only argparse/hashlib/json/re/sys/enum/pathlib
  (no time/socket/os-host/datetime); the sole write call targets the register artifact —
  the summary is never written. Checker's only live derivation call is
  `builder.collect(summary)` for equality/digest cross-check; classify/severity/supersede
  names appear there solely as presence-marker strings.

## Task item 1 — drift detection still covers the new fields

`--check` rebuilds the register in memory, canonicalizes the WHOLE dict
(`json.dumps(sort_keys=True)` + trailing newline) and fails with REGISTER_DRIFT unless the
tracked bytes are identical (builder lines 572-587). The two new keys are ordinary members
of that dict, so any drift in them flips the comparison — no per-field allowlist exists to
go stale. The stage checker adds defense in depth: it re-derives `collect(summary)`,
requires list equality (`obligations-drift`), digest equality (`obligations-digest`), exact
key-shape checks for both new lists (`carried-shape`, `mid-complete-shape`), and shells
`--check` itself (checker lines 213-242). Live: `--check` exit 0 at 291/52/OPEN. Covered.

## Task item 2 — fixed-point property survives

The digest covers the derived obligation list, not the summary bytes (builder line 521;
`register_digest` covers the canonical register minus itself), because the summary records
the register's own counts. Counter-type fields (`open_reviews`, `obligations`,
`register_result`, per-package counts) are ints or review-free names, so `collect()` never
yields obligations from them — counter updates cannot perturb the derivation. Proven live by
`test_recording_the_registers_own_counts_does_not_change_it` (mutates open_reviews to 4242,
rebuild identical — passed). Both rollups are pure functions of (summary, obligations):
`unclaimed_carried` scans PASS rows + `section_claimed_severities` (top-level `pN_open`
lists/counts); `mid_string_complete` scans open-state rows + top-level statuses. No I/O, no
clock, no host lookup; artifact grep confirms no timestamps/paths/host data. Stable.

## Task item 3 — layering

Builder derives, never judges or edits: `collect()` copies each disposition verbatim into
the obligation record and derives state/severities/supersession alongside — no summary
write path exists, no disposition string is ever rewritten. Checker asserts without
deriving judgments: it re-invokes the builder's own `collect` (same layer, same function)
and compares, pins markers/shapes/counters/statuses, and delegates drift and tests to
subprocesses — it classifies nothing itself. Tests pin the new CLEAR conjunction both ways:
`test_an_unclaimed_carried_finding_blocks_clearance` (negative), plus
`test_a_claimed_carried_finding_does_not_block_clearance` and
`test_a_closed_carried_finding_does_not_block_clearance` (positives), and
`test_mid_string_complete_packages_are_named_with_open_counts` (visibility-only: named,
never a violation, open review still blocks). Conjunction itself (builder lines 505-512)
adds `not carried` while correctly excluding mid-string (open reviews there already block).
Layering holds.

## Findings

No architecture-level findings. The two prior Vulcan P3s are closed on live data with
code, artifact, and test evidence each; the full-scope pass over derivation (single source,
verbatim dispositions, deterministic ordering, lane- and round-guarded supersession),
fixed-point (digest-over-derived, counter-stable, pure rollups, no ambient data), and
layering (derive / assert / pin) surfaces no residual defect. (`not violations` in the
CLEAR conjunction is unreachable-but-harmless defense in depth, not a finding.)

VERDICT: PASS / SECTION: finding_register / FIELD: nabu_architecture_review / SCOPE_SHA: db1bc623d01e838d49c153feb0be05a7502b8794 / FINDINGS: 0
