# S20-700 Remaining-Surface Blockers

Status: **merge boundary absent; full S20-700 remains incomplete**

Section 18.5 names eleven required persistent-fuzz surfaces. Eleven landed
libFuzzer binaries exercise ten of those required surfaces because the single
`ssmc_graph_cfg_checker` target covers both graph and CFG judgments. Two
additional binaries cover the adjacent S20-360 candidate-result importer and
the S20-390 transaction/receipt importers. The only remaining required gap is
the merge engine.

## Mutation candidates

S20-350 exposes the production proposal-only record/envelope boundary with
complete native structural codecs, bound preconditions, digest verification,
and deterministic creation IDs. The `mutation_candidate` target drives the
real build/import and decode/encode APIs and passed its fixture-seeded smoke. It
exposes no validation authority, mutation application, or state transition.

## Candidate results

S20-360 adds a fixture-seeded persistent target over the production result
importer. Sixteen accepted seeds cover `VALID` plus every terminal decision;
four structural corruptions and five synthetic seeds exercise strict envelope
rejection. Successful imports must repeat byte-identically, rederive the exact
result ID, and preserve the fourteen-phase monotonic shape. This extra target
does not replace any Section 18.5 surface and grants no candidate or commit
authority.

## Transactions and receipts

S20-390 adds a fixture-seeded persistent target over both production importers.
Trusted-genesis and ordinary receipts, eight envelope corruptions, one
digest-valid wrong-manifest-length receipt, and synthetic boundary seeds drive
strict import. Successful imports must repeat, rederive exact transaction and
receipt identities, preserve trailers, and retain nested bindings. Repository
closure separately rejects the wrong authenticated object length. This is an
adjacent hardening surface rather than one of Section 18.5's eleven named
minimums.

## Merge engine

S20-520 merge is implemented (`docs/audits/S20_520_MERGE_CLOSEOUT.md`);
S20-510 semantic comparison is implemented
(`docs/audits/S20_510_SEMANTIC_COMPARISON_CLOSEOUT.md`); both sit on the full
S20-250 bodies (`e78a1ab`), all three under draft contracts awaiting Council
reviews and contract freeze. S20-530 crash recovery is complete
(`docs/audits/S20_530_CRASH_RECOVERY_CLOSEOUT.md`, aging under ADR-0024);
S20-540 pack exchange is complete
(`docs/audits/S20_540_REPOSITORY_EXCHANGE_CLOSEOUT.md`);
the full S20-300 complete-root snapshot is implemented
(`docs/audits/S20_300_FULL_COMPLETE_ROOT_SNAPSHOT_CLOSEOUT.md`) and
the full S20-310 root-backed queries are implemented
(`docs/audits/S20_310_FULL_ROOT_BACKED_QUERY_CLOSEOUT.md`), and
the full S20-320 context capsule is implemented
(`docs/audits/S20_320_FULL_CONTEXT_CAPSULE_CLOSEOUT.md`), each under its
draft contract with reviews pending; the S20-400 SMP1 contract is drafted
(`docs/spec/SMP1.md` revision 3, ADR-0032) and S20-410 is implemented
(`docs/audits/S20_410_SMP1_FRAME_CLOSEOUT.md`) with four methods deferred on
owner gaps. Now
the merge engine target is attached (`fuzz/targets/merge_conflict_decoder.rs`, the conflict
decoder in two lanes plus the common-ancestor rule;
`docs/audits/S20_700_MERGE_PERSISTENT_SLICE.md`), so every Section 18.5
required surface now has a landed target. The merge judgment over three
complete roots is exercised by the deterministic merge corpus and its
independent oracle rather than a synthetic-root harness, whose inputs are
the frozen S20-250 and S20-510 surfaces with their own targets.

Protocol remains an adjacent future fuzz gap, but it is not one of the eleven
minimum persistent surfaces listed in Section 18.5 and S20-410 is not landed.

S20-700 remains incomplete: every required surface is fuzzed, but the
complete finding register, the independent review, and the Vulcan receipts
for the S20-250, S20-510, and S20-520 slices remain deferred with the
Council lanes, and `make v2` remains a release-boundary gate. The scoped S20-600 and
S20-610 mechanics, S20-650 unavailable disposition, and bounded S20-710 audit
have since landed. Restricted S20-360 candidate validation, restricted S20-390
atomic commit, and S20-500 native refs are complete. S20-510 and S20-520 await
reviews only. S20-530 crash recovery and S20-540 pack exchange are complete;
the full S20-300 complete-root snapshot is the next dependency-complete
work; the current cross-lane result is
`docs/audits/S20_LOCAL_COMPLETION_FRONTIER.md`.

Focused validation:

```text
python3 scripts/check_s20_700_frontier.py
make transaction-receipt-persistent-fuzz-smoke
make check-changed
```
