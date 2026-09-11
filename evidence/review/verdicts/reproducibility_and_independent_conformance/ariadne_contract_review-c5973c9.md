**Baseline verified:** `git rev-parse HEAD` = `c5973c90180d03402f0d2e5d2d91e941ef5dc58d`. Working tree carries only the three untracked paths noted in the request (two `.forge/slices` files, `evidence/review/requests/`). No files were written.

## Method and assumptions

- Read-only. This session's tool policy refused execution of `python3` scripts and tests, so `build_independent_conformance_report.py --check`, the staged checker, and `test_reproducibility.py` were **not re-executed**. Verification is by static reading, hand-recomputed digests (`sha256sum`), and git queries. Where I say a checker "fires", that is derived from its code path applied to git facts I did verify, not from a run.
- Prior lane verdict superseded: register entry `ariadne_contract_review` = `FAIL_1_P0_8_P1_6_P2_3_P3` at `9dc78fe` (`machineresearch/sley-2.0/reviews/s20-730-ariadne-contract-review-2026-09-04.log`). I do not rewrite it; findings below are current-tree only, with lineage noted.

## Verified facts (evidence)

| Check | Result |
|---|---|
| Delta `5b70052..79a1209` | exactly one commit; touches contract, builder, tests, `entity-read/v2/SHA256SUMS` |
| `conformance/entity-read/` | contains only `v2`; `git log --all --diff-filter=A -- conformance/entity-read/v1` is empty: **no v1 corpus ever existed** |
| Why v2 | `ENTITY_READ_PROFILE_V2.md` §2: methods 306/307 exist only in protocol version 2; fixture contract tag `sley2-entity-read-v2`; corpus authored at v2 from `9183db4`/`5f5c2dd` |
| `entity-read/v2/SHA256SUMS` | names all three JSON files; recomputed digests match (`85e270…`, `b88fcd…`, `abe861…`); evidence report records the same digests and `sums_consistent: true` |
| Builder command in `make conformance` | `Makefile:144` matches `COVERAGE["entity-read"]` exactly |
| Independence markers | 0 hits in `entity_read.py` (3175 lines) and the shim script |
| Semantic depth of the checker | `compute_work`/`check_work` recompute the §5 work bound; `negotiate_versioned` re-derives selection; `validate_rejected` runs layer decoders in order and derives the failing layer; `build_success_case` rebuilds response bytes. Same class as `check_root_backed_query_vector.py` |
| Prior P0 mechanism | closed at rev-3: `check_reproducibility_and_independent_conformance.py:163-186` verifies digest, ancestry, artifact-surface diff, uncommitted surface, and toolchain |

**Answer to the lane question:** the rev-4 `CORPUS_VERSION` pin of entity-read at `v2` is legitimate, no duplicate v1 corpus was invented, and the entity-read checker meets the semantic-depth bar applied to the other four semantic families. The defects below are elsewhere.

## Findings

**P1 [record]** `evidence/release/reproducibility-report.json` attests `84bfa9c` (2026-09-05, 177 commits behind HEAD). `git diff --name-only 84bfa9c HEAD -- <ARTIFACT_INPUT_PATHS>` lists 42 files (`crates/…`, `evidence/security/T52/pre-release-inventory.json`). By contract §2/§6/§8 the report is stale, and the staged checker's `history_problems` (lines 163-177) appends `reproducibility-report:stale:84bfa9c9c5d9:42-surface-files-changed:…`, making `make quick` (Makefile:79) exit 1 at the scope SHA. `c5973c9`'s message concedes it ("stays stale by rule"). Root cause: the local S20-720 record at `5b70052` has `working_tree_clean: false` (untracked RW-080 slices), so §1 refuses a fresh attestation. The mechanism is sound (that is the closure of my prior P0); the evidence is not acceptable under it, and the cure is the §2 one: clean tree, `make release-candidate-smoke`. Note also that `c5973c9` itself edits `pre-release-inventory.json`, which is on the artifact surface, so any attestation of its parent goes stale in the same commit; the evidence-refresh sequence needs to attest last.

**P1 [contract]** `docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md` §3, versioned-corpus rule. Rev-4 pins exactly one version directory per family and says "today every family pins v1 except entity-read", but four families carry unpinned sibling corpora: `bootstrap-profile/v2`, `exec-package/v2`, `host-abi/v2`, `smp1-json-bridge/v2`. The report neither digests, sums-checks, nor coverage-declares them; `smp1-json-bridge/v2` has **no SHA256SUMS at all** (would be `CONFORMANCE_FIXTURE_UNREADABLE` if in scope) and is named a governed corpus by `REQUIRED_CONTRACT_INDEX_V1.md` row 17.12. Their checkers run under `make quick` (Makefile:30,55,59,62), not `make conformance`. So `INDEPENDENT_CONFORMANCE_COMPLETE` ("every family's vectors are checked") overreaches the tree, and a tampered `host-abi/v2/host-abi.json` produces no report drift. Rev-4 made this exclusion normative without naming it. Fix: either record every version directory per family (each with sums, digests, coverage), or state that unpinned siblings are out of the report's scope and list them (e.g. `unpinned_versions`) so the COMPLETE claim is bounded.

**P2 [contract]** §5 semantic entry for entity-read. Checker depth is equal to the bar, but entity-read is the one family whose expected vectors are **oracle-authored** (`claim: independent-expected`, `refresh` mode) with no automated Rust consumer of `conformance/entity-read/v2` (`crates/` grep: only a comment at `sley-query/src/entity_read.rs:1805`); Rust agreement is a manual note in `S20_410_SMP1_FRAME_CLOSEOUT.md:226-238`. Every other semantic family is Rust-emitted then oracle-re-derived (e.g. `root_query.rs:2798`). The record should disclose the emission direction; closing the binding is AT-MW-02/S20-310 scope, not S20-730.

**P2 [contract]** §3 SHA256SUMS rule vs `build_independent_conformance_report.py:243-260`. Carried forward unresolved from prior P1-8: contract says a family without a manifest records `sums_consistent: null`; the builder raises `CONFORMANCE_FIXTURE_UNREADABLE`, so `null` and `sums_file: false` are unreachable and the guard at line 252 is dead. Fail-closed direction, hence P2 now.

**P2 [record]** `machineresearch/sley-2.0/machine-summary.json:3113` `attested_commit: e20b9ad…` while the report attests `84bfa9c`; the staged checker cross-checks only the four count/result keys (lines 305-313), not this one.

**P3 [implementation]** `oracle/scb1/src/sley2_scb1_oracle/entity_read.py:3145-3149`. `refresh` emits a two-line SHA256SUMS (accepted, rejected); the tracked manifest was hand-extended in `79a1209` to add `inputs.json` per the every-JSON rule. The next refresh-and-copy regresses to `CONFORMANCE_SUMS_MISMATCH`. Fail-closed, but the family generator and the S20-730 rule disagree.

**P3 [contract]** §10 has no revision-4 clarification recording *why* entity-read pins v2 (protocol v2 methods 306/307, tag `sley2-entity-read-v2`, no v1 ever existed). The header gives the location, not the rationale; the builder comment cites "the entity-read contract" without a section.

**P3 [record]** `docs/adr/ADR-0040…md:3` still reads "draft at revision 3"; `ADR_MARKERS` does not bind the revision, so the prior P1-7 pattern recurs.

**P4 [implementation]** `scripts/build_independent_conformance_report.py:5` docstring still says "every `conformance/<family>/v1` directory".

**P4 [record]** Report `runner` for entity-read reads `scripts/ (Python…)` while the logic is the 3175-line oracle module behind an 89-line shim; the builder's own comment argues for the package label.

## Lineage against the prior lane verdict

Prior P0 (report unbound/unverified): mechanism closed at rev-3; the evidence now fails that mechanism (new P1 record). Prior P1-5/P1-6 (depth axis, §9 contradiction): closed. Prior P1-8: open, now P2. Prior P1-7 pattern: recurs as P3. Prior P1-1/2/3/4 (second-host semantics, label binding, toolchain agreement, declared-vs-achieved coverage) were outside the rev-4 delta and not re-adjudicated here; they remain with the register.

Handoffs (no files written, advisory only): P1 record and the attestation ordering to the integrator; sibling-version rule to nabu (architecture) with vulcan for the missing `smp1-json-bridge/v2` manifest; `refresh` SHA256SUMS to merlin.

---

VERDICT: REVISE
SECTION: reproducibility_and_independent_conformance
FIELD: ariadne_contract_review
SCOPE_SHA: c5973c90180d03402f0d2e5d2d91e941ef5dc58d
FINDINGS:
[P1] [record] evidence/release/reproducibility-report.json - attests 84bfa9c, 42 artifact-surface files changed to HEAD; stale by contract §2/§6, staged checker fires and make quick is red at scope SHA; local S20-720 record at 5b70052 is working_tree_clean:false so no fresh attestation possible; cure per §2 on a clean tree, attest after evidence refresh
[P1] [contract] docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md §3 - one-pinned-version rule leaves bootstrap-profile/v2, exec-package/v2, host-abi/v2, smp1-json-bridge/v2 undigested, unsummed, undeclared; smp1-json-bridge/v2 has no SHA256SUMS yet is index row 17.12; COMPLETE overreaches the tree
[P2] [contract] §5 entity-read semantic entry - checker depth meets the bar, but vectors are oracle-authored with no automated Rust consumer; emission direction undisclosed
[P2] [contract] §3 vs scripts/build_independent_conformance_report.py:243-260 - sums_consistent:null and sums_file:false unreachable, dead guard; prior P1-8 unresolved
[P2] [record] machineresearch/sley-2.0/machine-summary.json:3113 - attested_commit e20b9ad disagrees with report 84bfa9c; unchecked
[P3] [implementation] oracle/scb1/src/sley2_scb1_oracle/entity_read.py:3145-3149 - refresh emits two-line SHA256SUMS omitting inputs.json; next refresh regresses to CONFORMANCE_SUMS_MISMATCH
[P3] [contract] §10 - no revision-4 clarification recording why entity-read pins v2 (protocol v2 methods 306/307; no v1 corpus ever existed)
[P3] [record] docs/adr/ADR-0040 line 3 - states contract revision 3 while contract is revision 4; ADR_MARKERS do not bind revision
[P4] [implementation] scripts/build_independent_conformance_report.py:5 - docstring still says v1 directory
[P4] [record] independent-conformance-report.json entity-read runner label - names scripts/ while the oracle package does the checking
SUMMARY: The rev-4 versioned-corpus rule pinning entity-read at v2 is legitimate: the family is protocol-version-2 by its own contract, its corpus has been v2 since authoring, no v1 corpus ever existed, the manifest names every JSON with digests I recomputed, and the checker recomputes work accounting, selection, response bytes, and failure-layer precedence with independent logic at the same depth as root-backed-query. The lane question therefore resolves in favour of the delta, and my prior P0 is closed as a mechanism. The verdict is REVISE rather than PASS for two P1s on the current tree: the S20-730 reproducibility evidence at the scope SHA is stale under the contract's own freshness rule (acknowledged in c5973c9's message) and the tree's Tier-1 gate is red on it; and the rev-4 pin rule silently scopes four families' sibling v2 corpora out of the report while COMPLETE claims every family's vectors, one of those corpora carrying no manifest at all. Remaining P2-P4 are contract/record accuracy and one generator-vs-rule inconsistency, all fail-closed. Not re-executed in this session: scripts and tests (tool policy); all claims rest on static reading, sha256sum, and git.
