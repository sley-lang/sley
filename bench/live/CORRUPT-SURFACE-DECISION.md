# CORRUPT original obligation — surface decision, revision 2

Status 2026-09-23, branch `work/succ-corrupt-impl`. Revision 1
(2026-09-21, reviewed at `2c97c32f`) received Ariadne verdict
REVISE_0_P0_1_P1_1_P2_3_P3_1_P4, DECISION OPTION_3
(`evidence/review/verdicts/corrupt_surface_decision/ariadne_contract_review-2c97c32.md`).
This revision answers each finding by id and implements the verdict's
closure list items 1-4. Item 5 (who must attempt the import) was left open
for the owner (section 4).

Revision 2 then passed Ariadne review on 2026-09-23, at scope `01dd20cb`.
That review issued two rulings, recorded in section 5:

- `RULING_ACTOR: A`: the obligation is accepted on the resealed PACK
  vector.
- `RULING_ORDER: CODE`: the ruling required the spec to be amended to the
  importer's order, and spec revision 9 does so.

That review also raised a P3 (section 2 [P1]) and a P4
(SUCCESSION-COVERAGE.md). This revision addresses both; their closure was
verified in the revision-3 review
(`evidence/review/verdicts/corrupt_surface_decision/ariadne_contract_review_revision_3-5d8106f.md`).
The revision-3 review's own spec findings (P2 precedence, P3 step 3.3) are
answered in section 5. `ga_claimed=false`.

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

Owner observation (revision 2; since ruled `RULING_ORDER: CODE`, see
section 5). The spec's import-phase list put
"the complete digest tree" (step 2) before "the complete S20-170 preflight
... over the embedded pack" (step 3) (REPOSITORY_EXCHANGE_V1.md:348-352).
The code runs the pack preflight first (exchange.rs:1384, then the tree
check at :1386-1396), because the pack leaf is keyed by the pack's
`RepositoryPackId` (spec :209).

- Step 2 also verifies "every declared identity", and the embedded pack's
  declared identity is exactly the check that yields PACK_DIGEST_MISMATCH.
- A literal tree-first reading would instead yield
  EXCHANGE_DIGEST_TREE_MISMATCH.
- `conformance/repository-exchange/v1/rejected.json` has 6 frozen
  mutations. One of them, `nested-exchange`, replaces the embedded pack with
  a whole tag-540 exchange (exchange.rs:3243-3250,3291). It pins the tag-170
  shape check (`EXCHANGE_PACK_INVALID`, exchange.rs:1381-1383) ahead of the
  digest tree (:1386-1396).
- No mutation alters embedded-pack content under a valid tag-170 header, so
  no frozen vector pins the S20-170 preflight (`PACK_DIGEST_MISMATCH`)
  against the digest tree.
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

This section is the revision 2 state. The owner rulings in section 5
supersede it: under `RULING_ACTOR: A` the obligation is accepted on the
resealed PACK vector.

OPTION_3 stood as ruled at revision 2. No PACK-layer acceptance was claimed
for the `sley_2_0` arm, and the exchange-trailer check remains only a
regression.

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

## 4. Owner question (verdict closure item 5, now ruled A in section 5): does a judge-side attempt satisfy "attempt import"?

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

Until the owner ruled, the CORRUPT record claimed three things only:

- the resealed-vector evidence, labelled judge-side;
- the exchange-trailer regression;
- the constant-restore smoke check.

## 5. Rulings (Ariadne revision 2 review, 2026-09-23)

Source: `evidence/review/verdicts/corrupt_surface_decision/ariadne_contract_review_revision_2-01dd20c.md`. The verdict is `PASS_0_P0_0_P1_0_P2_1_P3_1_P4_PRIOR_P3_P4_CLOSED`
at scope `01dd20cb`. The round index is
`evidence/review/rounds/corrupt-r2-01dd20c.json`.

### RULING_ACTOR: A

A judge-side corruption and import attempt satisfies "attempt import", by
parity across the arms:

- The frozen text names no actor (tasks.json:137).
- In the legacy and raw arms, the oracle or test harness makes the
  corruption and the import attempt.
- The sley2 run control deliberately denies bulk import to the agent
  (runner.py:90-94,115-117,126).

The ruling carries two conditions, both kept in this record and in
SUCCESSION-COVERAGE.md:

- The evidence is agent-independent (judge-side). The only agent-bound part
  is the constant-restore smoke check through propose/finish.
- The acceptance claims no agent-driven import.

**Consequence.** The S2B-CORRUPT-001 obligation for the `sley_2_0` arm is
ACCEPTED on the resealed PACK vector. The evidence is:

- the Rust pin `s3_corrupt_exchange_resealed_embedded_pack`;
- the judge path `_judge_corrupt_pack_resealed`;
- the log `succ-trials-20260923/corrupt/trial_corrupt_pos.log`;
- rejection-path negatives with exact reject symbols.

Two things must stay distinct:

- **Judge verdict.** The JSON `status` a trial receives. The judge accepts
  a trial whose candidate restores the constant, provided the judge-side
  vectors hold.
- **Obligation acceptance.** The owner ruling above that this judge-side
  evidence discharges the corpus requirement.

The unresealed EXCHANGE_DIGEST_MISMATCH check remains a regression only, and
no EXCHANGE code counts toward the PACK oracle. The live-model trial remains
outstanding. `ga_claimed=false`: this is an obligation record, not a GA
claim.

### RULING_ORDER: CODE

The importer's order is authoritative (exchange.rs:1381-1397: tag-170 check,
then `preflight_conformance_pack`, then the digest tree). The ruling required
the spec to be amended to match. Revision 9 does so, and no importer code
changed.

`docs/spec/REPOSITORY_EXCHANGE_V1.md` revision 9 makes these changes:

- It adds a status line citing the ruling.
- It rewrites import-phase steps 1 through 6 to list every check in the
  order `exchange.rs` runs it, and adds an exact precedence paragraph.
- The precedence paragraph names the checks that run ahead of a
  later-numbered step:
  - the empty receipt set (`EXCHANGE_ANCESTRY_OPEN`, step 2);
  - the tree-record profile and counts (`EXCHANGE_DIGEST_TREE_MISMATCH`,
    step 2);
  - the ungrammatical branch name (`EXCHANGE_BRANCH_INVALID`, step 3.3);
  - closure rule 6 for origin and ref records, and closure rule 2 (both
    step 5).
- Step 3 becomes ordered sub-steps:
  - 3.1, the tag-170 check (`EXCHANGE_PACK_INVALID`);
  - 3.2, the S20-170 preflight, with `PACK_*` preserved;
  - 3.3, leaf recomputation from the declared identifiers, with section-3
    name keys recomputed, then the leaf list and root, keyed by the
    `RepositoryPackId` from 3.2.
- The declared receipt and head identities are verified in step 4 (4.1 and
  4.4).
- It adds a dated "Revision 9 amendment note", corrected after the
  revision-3 review.

Revision-3 review answers:

- **[P2] precedence: CLOSED by exact enumeration, not narrowing.**
  - I re-read `exchange.rs` preflight end to end: `decode_envelope`,
    `decode_payload` and its field decoders, the registry decode,
    `compute_leaves`, the receipt loop, `verify_closure_rules`,
    `verify_branch_entry`, `verify_no_surplus` and
    `verify_receipts_against_pack`.
  - Beyond the reviewer's examples, this found that inside step 4 the head
    check (rule 5) runs before root closure (rule 3), and that the
    transaction workspace check interleaves per receipt with rule 1. Both
    are now stated.
  - The early exits are pinned pairwise by the new unit test
    `exchange::tests::preflight_precedence_early_exits_match_the_import_phase_text`,
    including both of the reviewer's failure scenarios:
    - zero receipts plus a nested exchange gives `EXCHANGE_ANCESTRY_OPEN`;
    - an earlier binding-broken branch plus a later foreign-workspace branch
      gives `EXCHANGE_BRANCH_INVALID`.
  - The test also pins a tree-algorithm failure over the nested pack,
    `BRANCH_INVALID` over `HEAD_INVALID`, `ANCESTRY_OPEN` over
    `HEAD_INVALID`, and `HEAD_INVALID` over `ROOT_CLOSURE`.
- **[P3] step 3.3 wording: CLOSED.** Step 3.3 is reworded as the closure
  evidence asks, and the note's "moves them into step 3.3" sentence is
  corrected.

The note records these deliberate choices against the ruling's list, all for
spec review:

- **Sub-steps, not a new top-level step.** This keeps the step-7 and step-8
  cross-references, rows X-01 through X-07, and the checker markers intact.
- **Precedence exact, as realized.** The code is not a plain 1-7 order, so
  the steps were rewritten to the realized order rather than asserting one.
- **Frozen conformance mutation deferred.** A frozen `rejected.json`
  mutation for the resealed vector is not added. It would re-freeze a
  digest-bound corpus, whose digests feed the independent-conformance, GA,
  release-provenance and decision-dossier evidence. It needs its own
  conformance review. Until then the order is pinned by `s3_g2_corrupt.rs`
  and the exchange unit test above.
