# CORRUPT original obligation — surface decision, revision 2

Status 2026-09-23, branch `work/succ-corrupt-impl`. Revision 1
(2026-09-21, reviewed at `2c97c32f`) received Ariadne verdict
REVISE_0_P0_1_P1_1_P2_3_P3_1_P4, DECISION OPTION_3
(`evidence/review/verdicts/corrupt_surface_decision/ariadne_contract_review-2c97c32.md`).
This revision answers each finding by id and implements the verdict's
closure list items 1-4. Item 5 (who must attempt the import) stays open
for the owner (section 4). `ga_claimed=false`.

## 1. Corpus obligation (frozen, unchanged)

`bench/corpus/v1/tasks.json:134-150`, S2B-CORRUPT-001: "Alter one canonical
object byte in an exchange pack, attempt import, reject it with an exact
digest failure, and prove the destination ref remains unchanged."
`strict_oracle.expected_code = "PACK_DIGEST_MISMATCH"`, `ref_advances = 0`.
Required outcomes: import fails before ref movement, the failure code is
PACK_DIGEST_MISMATCH, the old root remains accepted, and recovery is
deterministic. The task names no actor for "attempt import".

## 2. Findings and closure

### [P1] factual-premise: CLOSED (revision 1's premise was wrong)

Revision 1 (at :29-34 and :50-59) said the corpus path "is not drivable today".
That was wrong. `exchange.import` is the only import method: server.rs:1284
dispatches to `exchange_import` (server.rs:3131-3146), which calls
`import_repository_exchange`. That function's `preflight`
(crates/sley-repo/src/exchange.rs:1890) does the following:

- It checks the exchange trailer. The trailer is
  `RepositoryExchangeId::derive(input[..len-32])` (exchange.rs:674-677),
  which is unkeyed `blake3(domain || preimage)` (crates/sley-id/src/lib.rs:167-171,501).
- It then runs the PACK owner's `preflight_conformance_pack` on the embedded
  pack (exchange.rs:1384).
- That preflight returns `PackErrorCode::DigestMismatch` =
  `"PACK_DIGEST_MISMATCH"` for a stale pack trailer (lib.rs:714-717, :129).
- `ExchangeError::code()` passes the code through verbatim (exchange.rs:273-281),
  as the spec requires (docs/spec/REPOSITORY_EXCHANGE_V1.md:518-519).
- All of this happens before `classify_target` and before any write (exchange.rs:1890-1895).

Closure list items 1-4 are implemented and were executed:

1. **Vector (Rust).** `crates/sley-repo/tests/s3_g2_corrupt.rs`, test
   `s3_corrupt_exchange_resealed_embedded_pack` (:563).
   - It reads the staged `bench/fixtures/sley2/S2B-CORRUPT-001/base.pack` (:476).
   - It locates the embedded tag-170 pack (exchange payload field 2). The
     pack's own trailer must equal `RepositoryPackId::derive`.
   - It finds the canonical object entries with the existing test-only
     decode `decode_conformance_pack_entries_for_testing` (lib.rs:446). No
     production API was added.
   - It flips one interior object byte and reseals the exchange trailer
     with the owner's `RepositoryExchangeId::derive` (:515).
   - Owner-digest equivalence: resealing the untouched exchange must
     reproduce it byte for byte (:568-576).
   - Mutation check (run, then reverted): with the reseal removed, the
     test fails with `left: "EXCHANGE_DIGEST_MISMATCH"`,
     `right: "PACK_DIGEST_MISMATCH"`.
2. **Two independent flips, exact code, destination unchanged.**
   - The two flips are object 0 at 3/8 of its bytes (absolute offset 1301)
     and the last object at 5/8 (offset 2637).
   - Every import returns exact `PACK_DIGEST_MISMATCH` as
     `ExchangeError::Pack(_)` (:553-560). That is the owner's own error,
     not a remap.
   - On a populated destination, each flip is imported twice. Head tx, live
     object count (5), and every destination file byte are unchanged.
   - On a fresh destination, the target is never created (:640).
   - The target-free `preflight_repository_exchange` reports the same code.
3. **Positive control and determinism.** The clean exchange imports into the
   same fresh destination after three refusals, with no cleanup, onto the
   same head tx (:643). Retries return the same code.
4. **Pin.** The test is green in `cargo test -p sley-repo --test s3_g2_corrupt`
   (2 passed, 3 ignored by design). Its existing assertions are unchanged.
   The misleading module comment ":5-7 trailer gate fires first on ANY
   byte flip" is corrected (:4-18): that holds only for an unresealed flip.

The judge is wired to the same vector. `bench/fixtures/sley2_live_judge.py`,
`_judge_corrupt_pack_resealed` (:1695), is called for CORRUPT beside the
regression (:394):

- `_resealed_pack_vectors` (:1619) produces the same offsets, 1301 and 2637.
  It reseals through the pinned oracle project's blake3 and fails as a
  harness error unless the resealed-unflipped control equals `base.pack`
  (:1639).
- Fresh destination (sessionless import while no head exists,
  server.rs:1132-1149; `_pre_head_import` :1654): each flip and a retry
  must be refused with exact PACK_DIGEST_MISMATCH, with repository files
  byte-identical. The resealed-unflipped control must then import into the
  same destination; otherwise the verdict is ORACLE_CORRUPT_UNRECOVERED.
- Populated destination: each flip is imported twice, the two failure
  bodies must be identical, files must be byte-identical, and head tx and
  live count must be unchanged on a fresh session.
- A wrong code rejects ORACLE_CORRUPT_UNREFUSED. An accepted import rejects
  ORACLE_CORRUPT_ACCEPTED.

The `_judge_corrupt_exchange` docstring at :1420-1422 claimed
"PACK_DIGEST_MISMATCH lives one layer up". It is corrected (:1414-1433).
The unresealed EXCHANGE_DIGEST_MISMATCH check stays, labelled as a
regression.

Owner observation, not decided here. The spec's import-phase list puts
"the complete digest tree" (step 2) before "the complete S20-170 preflight
... over the embedded pack" (step 3) (REPOSITORY_EXCHANGE_V1.md:348-352).
The code runs the pack preflight first (exchange.rs:1384, then the tree
check at :1386-1396), because the pack leaf is keyed by the pack's
`RepositoryPackId` (spec :209).

- Step 2 also verifies "every declared identity", and the embedded pack's
  declared identity is exactly the check that yields PACK_DIGEST_MISMATCH.
- A literal tree-first reading would instead yield
  EXCHANGE_DIGEST_TREE_MISMATCH.
- No frozen exchange conformance vector pins this ordering.
  `conformance/repository-exchange/v1/rejected.json` has 6 mutations, and
  none alters the embedded pack.
- The new Rust pin fixes the implemented behavior. It does not settle the
  spec reading.

### [P2] mislabeled-evidence: CLOSED

Revision 1 called `_judge_corrupt_exchange` the "trial-surface path" (at
:17-22 and :55-58). It is **judge-side**:

- The judge flips the bytes, seeds a temporary workspace, and imports
  through the privileged `Session._raw_request` (sley2_live_judge.py:1464).
  `_raw_request` is described at `bench/live/sley2_tool.py:300-302` as a
  call "the agent's allowlist never covers".
- The new `_judge_corrupt_pack_resealed` is judge-side too (:1754 and
  `_pre_head_import`).
- The trial agent never touches these bytes, and neither check depends on
  the agent's candidate.
- The agent-bound work in this task is only the constant restore through
  propose/finish, which `_judge_corrupt_value` checks as a smoke test.

"Trial surface" is dropped from every CORRUPT record in this revision and in
SUCCESSION-COVERAGE.md.

### [P3] unsupported-claim (TOOL_METHODS "pinned by test"): CLOSED by rewording

`bench/live/sley2_tool.py:10` and `:84-85` say TOOL_METHODS is "pinned equal
to the smoke runner's allowlist by test". On this branch that is not true:

- No test under `bench/` names `TOOL_METHODS`.
- `bench/sley2/tests/test_runner.py` pins only `ARM_AFFORDANCES` (:552).

What is true:

- At this branch's HEAD, `sley2_tool.TOOL_METHODS == runner.ARM_AFFORDANCES`
  (18 entries, checked by a python3 import), and
  `"exchange.import" in runner.ARM_DENIED_METHODS` (runner.py:126).
- The mediated gateway's `raw` is gated on `TOOL_METHODS`
  (mediated_sley.py:220).
- The equality is a fact at this checkpoint, not a test-enforced guarantee.
  A pin test is being added on a separate branch and is not merged here.
- The `sley2_tool.py` comment is left as is on this branch, to avoid a
  cross-branch conflict, and should be read with this correction.

### [P3] evidence-gap (negative): CLOSED

The revision 1 negative, `trial_corrupt_neg_recheck.log`, recorded only
`judge exit: 1` and exercised the constant smoke check. The new logs in
`bench/live/succ-trials-20260923/corrupt/` carry the judge's JSON verdict
and include negatives that target the rejection path:

| Log | Verdict (status / code / detail) |
|---|---|
| `trial_corrupt_pos.log` | accepted / null / "all flows held"; resealed-vector evidence: fresh destination 3 refusals + populated destination 2x2 refusals, all `PACK_DIGEST_MISMATCH`; control accepted; head `0a0a398c…`, live objects 5; exchange-trailer regression held |
| `trial_corrupt_neg_accepted.log` | rejected / `ORACLE_CORRUPT_ACCEPTED` / "accepted-standin: corrupted exchange imported" (accepted-corruption stand-in: the unflipped control offered as the corruption) |
| `trial_corrupt_neg_unresealed.log` | rejected / `ORACLE_CORRUPT_UNREFUSED` / "unresealed-standin: EXCHANGE_DIGEST_MISMATCH" (the exchange code is never accepted for the PACK code) |
| `trial_corrupt_neg.log` | rejected / `ORACLE_CORRUPT_UNRESTORED` / "true" (constant smoke negative, retained) |
| `rust_s3_g2_corrupt.log` | `S3_EVIDENCE ... route=exchange.import resealed=true symbol=PACK_DIGEST_MISMATCH flipped_offsets=1301,2637 head_objects=5 unresealed_symbol=EXCHANGE_DIGEST_MISMATCH`; 2 passed |

Each trial log carries env provenance:

- UTC time and git HEAD `72036a4e` (0 dirty paths outside the log directories).
- sha256 of the sley binary, which was built from this branch. The branch
  changes no production source.
- sha256 of the judge driver binary and of `base.pack`.
- The Python version.

The stand-in negatives are also unit tests in
`bench/live/tests/test_corrupt_resealed.py`:

- `:118` and `:127` cover the resealed refusal and the accepted-corruption
  stand-in.
- `:135` covers the unresealed stand-in.
- `:74` checks the vector layout: exactly one object byte and the trailer
  differ, the flips land in different objects, and the offsets equal the
  Rust pin's.
- `:95` checks that a diverging reseal is a harness error, not a verdict.

The witness (`bench/live/succ_witness_corrupt.py`) refuses to overwrite an
existing log.

### [P3] interpretive-constraint (actor): RESTATED as an open owner question

Revision 1 (at :41-44) asserted that the task "requires" the trial agent to
attempt the import. No owner-contract text supports that. The frozen task
names no actor. The claim is withdrawn and restated as section 4.

### [P4] note: ADDRESSED

- `outer_digest_tamper_fails_before_promotion` (lib.rs:1396-1408) flips a
  pack **trailer** byte, not a canonical object byte.
- The conformance-pack object-byte pin is `s3_g2_corrupt.rs`
  `flip_object_byte` / `s3_corrupt_fixture_conformance`.
- At the pack layer "ref advances 0" is vacuous: S20-170 writes no ref
  (REPOSITORY_PACK_V1.md:143-144). So library pack pins cannot demonstrate
  ref or head retention.
- Head retention is now demonstrated at the exchange layer, by
  `s3_corrupt_exchange_resealed_embedded_pack` (TransactionRepository
  accepted head and live objects) and by the judge's populated destination.

## 3. Decision state

OPTION_3 stands as ruled. No PACK-layer acceptance is claimed for the
`sley_2_0` arm, and the exchange-trailer check remains only a regression.

What changed is the evidence behind the open item:

- The corpus code, route, and ref behavior are now demonstrated on the
  existing protocol with no new method. That means:
  - one canonical object byte altered in the exchange's embedded pack;
  - import through `exchange.import`;
  - exact PACK_DIGEST_MISMATCH before any write;
  - head and live objects unchanged;
  - deterministic refusal;
  - clean re-import accepted.
- Options 1 and 2 are moot. No bundle-import method is needed, and no
  EXCHANGE code is counted toward the PACK oracle.
- The only item between this evidence and closing the obligation is the
  owner ruling in section 4.

## 4. Open owner question (verdict closure item 5): does a judge-side attempt satisfy "attempt import"?

Facts for the ruling:

- **Task text.** The frozen text (tasks.json:134-150) says "attempt import"
  and names no actor.
- **The sley2 arm cannot import.** The frozen run control denies
  `exchange.import` to every arm (`bench/sley2/runner.py:126`;
  `bench/sley2/tests/test_runner.py:552`).
  - `ARM_AFFORDANCES` is digest-recorded, so any widening is visible
    (runner.py:90-94).
  - The trial tool has no import path (sley2_tool.py:84-107), and neither
    does the mediated gateway (`mediated_sley.py:48-51`, `:220`).
  - So no `sley_2_0` agent can perform the import. The attempt shown here
    is the judge's.
- **Legacy-arm parity.** The legacy arm files
  `bench/fixtures/legacy/S2B-CORRUPT-001/arm_limitation.json`:
  - "the digest gate is demonstrated by oracle-side verification".
  - The legacy oracle itself hashes the staged payload, reads `ref.txt`,
    and reports the refusal (`legacy/.../oracle.py:30-66`).
- **Raw-arm parity.** In the raw arm's frozen fixture, the oracle's test
  program builds the corrupt pack and calls the arm's importer
  (`raw/.../fixture/test_program.py:10-45`).
- **Across all three arms,** the corruption and the import attempt are made
  by the oracle or judge, never by an agent tool interaction. The sley2
  judge-side attempt is the same pattern, driven against the real engine
  owner.

The owner is asked to rule one of:

- **(a)** A judge/oracle-side attempt satisfies "attempt import" for every
  arm, in parity with the legacy `arm_limitation.json` and the raw test
  program. The sley2 obligation then closes on the evidence in section 2
  [P1].
- **(b)** The corpus requires an agent-attempted import. Every arm then
  lacks it by run control. That needs an explicit run-control change
  (reviewed, digest-visible) or an `arm_limitation.json`-style record for
  `sley_2_0`, and the obligation stays open.

A related question for the owner is the spec phase-order observation under
[P1].

Until the owner rules, the CORRUPT record claims three things only:

- the resealed-vector evidence, labelled judge-side;
- the exchange-trailer regression;
- the constant-restore smoke check.

`ga_claimed=false`.
