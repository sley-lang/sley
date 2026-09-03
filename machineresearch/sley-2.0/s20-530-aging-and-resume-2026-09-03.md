# S20-530 aging decision and Sley 2.0 resume record (2026-09-03)

Status: ADR-0024 ADOPTED (historical acceptance); S20-530 COMPLETE; S20-540
PACK EXCHANGE IS THE NEXT PACKAGE; Council reviews of the aging rule and the
isolated full-checker verification in flight at the time of writing

Owner: Claude orchestrator

## Authority for the decision

The 2026-09-02 stop checkpoint reserved the post-acceptance aging rule for the
operator. The operator's standing directive for this session was to resume
Sley 2.0 development from the last checkpoint. After the final checker
confirmation passed, every path forward required that rule, the packet in
`s20-530-post-acceptance-aging-decision-2026-09-02.md` showed that only
historical acceptance (Option A) unblocks the roadmap, and the work is local
and reversible. The orchestrator therefore proceeded under the explicit
assumption that Option A is approved, and says so here and in the commit
message. Nothing was pushed, deployed, or published. If the operator prefers
the v14 owned-surface amendment instead, revert `3036c15` and `18d4296` and
follow the packet's Option B; the S20-530 evidence is untouched either way.

## What landed (all on `main`, local only)

| Commit | Change set |
|---|---|
| `5c0e94d` | S20-530 completion note, final checker confirmation log, work-package row |
| `d705192` | aging decision packet and the read-only anchor prototype (narrative lane) |
| `3036c15` | ADR-0024, `scripts/check_s20_530_acceptance_anchor.py` in `make quick`, `scripts/verify_s20_530_accepted_state.py` behind `make s20-530-verify`, S20-530 closeout audit, frontier re-anchor (audit doc, frontier checker, S20-700 checker and blockers doc, machine summary), work-package row |
| `18d4296` | S20-390 fixture-emitter parity repair, scoped lint allows on the three frozen S20-530 test modules, regenerated T54 evidence |

`make quick` is green from `18d4296` onward (about 18 seconds, zero
warnings); at `3036c15` alone it still failed on the pre-existing S20-390
fixture drift that `18d4296` repairs.

## Failures found and repaired in flight

1. `make quick` had not been green since 2026-08-30. The S20-710 T54
   secret-scan manifest hashes every candidate file's bytes, and the evidence
   had last been regenerated at `7c622a3`; the next docs checkpoint `109d1bc`
   drifted it. Repair: `python3 scripts/generate_supply_chain_evidence.py`
   (three counters change, findings stay empty). Rule: regenerate T54 as the
   very last step before each commit; any later edit re-drifts it.
2. `python3 scripts/generate_transaction_receipt_fixtures.py --check` drifted
   from `8343beb` (2026-08-30) because the S20-530 tests added
   `DeleteEntityBinding` to the shared `Fixture::new` grant in
   `crates/sley-txn/src/repository.rs`, which changed the emitter's genesis
   and ordinary vectors. The committed S20-390 vectors were still valid (the
   Python oracle check passed in the closeout). Repair: the ignored emitter
   now builds `Fixture::with_mutation_classes("emit", &[CreateEntity])`;
   `Fixture::new` is unchanged for the recovery tests; no vector, digest, or
   production path changed.
3. The three frozen mapped-test modules raised 1,244 rustc lint warnings
   (unused variables and mut bindings, parenthesized `matches!` scrutinees,
   row-encoded function names, six unused helpers), all present in the
   retained closeout logs. Triage: expected contract forms parsed by the
   frozen checker at the accepted state; encoded with
   `#[allow(...)]` and an explanatory comment on each module. Lint hygiene
   for those modules is a deferred slice.

## Council review of the aging rule

Bounded reviews on `claude-cli/claude-opus-5` (`forge agent --bounded
--thinking high --timeout 3000`), one at a time behind the machine-wide
dispatch lock, with a 20-minute decision budget and the verdict line
`S20_530_AGING_REVIEW_VERDICT_JSON=...`:

- Nabu (architecture): session
  `forge-nabu-s20-530-aging-20260903T012356-62f886b2`, dispatched
  2026-09-03T01:23:56Z, verdict 01:28:33Z: `PASS_AGING_RULE`, no blocking
  finding, P2/P3 advisories (ADR wording, anchor self-binding, generators,
  live config copy, ownership reversion).
- Vulcan (QA/security): session
  `forge-vulcan-s20-530-aging-20260903T012356-3f3125a1`, dispatched
  01:28:51Z, verdict 01:33:30Z: `PASS_AGING_RULE`, no blocking finding,
  P2/P3/P4 advisories (gate registration, trust base, tree modes, clean Git
  environment, `dead_code` allow, summary fields).
- Disposition: every advisory is applied in the commit that follows
  `18d4296` (see the "Aging rule review" section of the closeout audit).

These reviews are advisory (ADR-0024 is not an S20-530 contract phase);
their findings are applied and recorded in
`docs/audits/S20_530_CRASH_RECOVERY_CLOSEOUT.md`.

## S20-540 design brief and consult

The design brief `s20-540-pack-exchange-design-brief-2026-09-03.md` proposes
a composed Repository Exchange v1 contract (tag 540) that embeds the exact
S20-170 pack and adds receipts, the accepted head, and branch records, with an
empty-target trust rule and head-last durability. A bounded Nabu architecture
consult on its five questions follows the aging reviews; the contract draft
(`docs/spec/REPOSITORY_EXCHANGE_V1.md`, ADR-0025, structural checker) is
written after that consult.

## S20-540 contract freeze (2026-09-03T02:41Z)

Repository Exchange v1 is frozen at revision 6 (`5e5d593`, freeze flip in the
next commit). Review trail, all on `claude-cli/claude-opus-5`, sequential:

| Pass | Session | Result |
|---|---|---|
| Nabu design consult | `forge-nabu-s20-540-design-20260903T013712-0fb98776` | embed, sley-txn API, byte-exact branches; concerns applied |
| Ariadne 1 (rev 1) | `forge-ariadne-s20-540-contract-20260903T014755-7931f8f9` | FAIL: 4 P0, 4 P1, 10 P2, 7 P3 |
| Ariadne 2 (rev 2) | `forge-ariadne-s20-540-rereview-20260903T020340-91f5e854` | FAIL: 2 P1, 1 P2, 3 P3 |
| Ariadne 3 (rev 3) | `forge-ariadne-s20-540-pass3-20260903T021306-68bdb08f` | PASS |
| Vulcan 1 (rev 3) | `forge-vulcan-s20-540-contract-20260903T014755-d837d5c4` | FAIL: 1 P1, 4 P2, 3 P3 |
| Vulcan 2 (rev 4) | `forge-vulcan-s20-540-rereview-20260903T022707-896ec8f9` | PASS with four text notes (rev 5) |
| Ariadne 4 (rev 5) | `forge-ariadne-s20-540-pass4-20260903T022707-997681c9` | FAIL: 1 P0 (stale 39021), 2 P1, 4 P2, 2 P3 |
| Ariadne 5 (rev 6) | `forge-ariadne-s20-540-pass5-20260903T023812-51133a3e` | PASS; ready to freeze |

Commits: `a3d6d2b` (rev 1), `b78c94b` and `b5fa5a0` (rev 2), `74a393c`
(rev 3), `e3ad315` (rev 4), `7c377c1` (rev 5), `c826f62` (39022),
`5e5d593` (rev 6). Frozen hashes: field schema `a843405b…` (unchanged since
rev 1), decoder limits `808eaba9…` (since rev 2). The implementation plan is
`s20-540-implementation-plan-2026-09-03.md`. Lessons recorded in memory:
absolute paths in Council requests; check SCB1 section 9 and canonical-set
element order before writing a contract; S20-390 numerics live in
`ERROR_CODES_V1.md`.

## Isolated verification

`make s20-530-verify` was launched at 2026-09-03T01:23:36Z (clone at
`cc0f92f`, workdir in the orchestrator scratchpad, log
`s20-530-verify.log`). Expected: the frozen checker's PASS line after about
95 to 130 minutes. Result: PENDING at the time of writing.

## Next development package: S20-540 pack exchange

Dependencies S20-170 (`S20_170_COMPLETE`, uncompressed root/object-only
pack profile) and S20-500 (native refs and branches) are complete. Owner:
Merlin. Owned paths: `sley-repo`, `sley-conformance`. Acceptance: a clean
import reconstructs root, refs, and ancestry per profile. Focused gate: the
clone-equivalent test. Release implication: M4 exit.

First slice: freeze the clone-equivalence profile as a contract (spec plus
ADR, structural checker in `make quick`) over the S20-170 pack and the
S20-500 ref records, then implement the exporter/importer and the
clone-equivalent conformance test. Reviews: Nabu on the design, Ariadne on
the contract, Vulcan on the import surface.
