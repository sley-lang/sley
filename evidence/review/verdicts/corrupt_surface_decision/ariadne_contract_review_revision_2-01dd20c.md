<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 01dd20cb3604643a0c833f2d9306ab4e365d272e; role: ariadne; field: ariadne_contract_review_revision_2; dispatched: 2026-09-23T03:48:37Z; duration_s: 257; process_exit_code: 0 -->
# Ariadne Council review — corrupt_surface_decision

Harness: claude-code
Reviewed checkpoint: 01dd20cb3604643a0c833f2d9306ab4e365d272e

What I verified myself:
- **Scope.** `git rev-parse HEAD` returned `01dd20cb3604643a0c833f2d9306ab4e365d272e`, so the scope matches. `git status --short` was clean, and I left the tree unmodified.
- **Diff.** `git diff --stat ab42a3a9..HEAD` shows 11 files: the packet, SUCCESSION-COVERAGE.md, the judge, the witness, the new unittest, the Rust test, and 5 logs. No production `crates/*/src` file changed.
- **Log provenance.** `git log --oneline ab42a3a9..HEAD` shows four commits: e91693ab, 72036a4e, 10939486 and 01dd20cb. `git diff --stat 72036a4e..HEAD` touches only the packet, the coverage file and the logs. So the code at the logs' recorded `git_head` 72036a4e is the code at HEAD.
- **Rust suite.** `cargo test -p sley-repo --test s3_g2_corrupt -- --nocapture` exited 0 with `test result: ok. 2 passed; 0 failed; 3 ignored`. It printed `S3_EVIDENCE task=S2B-CORRUPT-001 route=exchange.import resealed=true symbol=PACK_DIGEST_MISMATCH exchange_len=5825 flipped_offsets=1301,2637 head_objects=5 unresealed_symbol=EXCHANGE_DIGEST_MISMATCH`, and `s3_corrupt_exchange_resealed_embedded_pack ... ok`.
- **Python suite.** `python3 -m unittest bench.live.tests.test_corrupt_resealed -v` exited 0 with `Ran 6 tests ... OK` and none skipped. So both the layout class and the integration class ran, the integration class against the sley binary in the preset CARGO_TARGET_DIR.
- **Allowlist check.** A python3 heredoc returned `TOOL_METHODS == ARM_AFFORDANCES` True with 18 entries, and `"exchange.import" in ARM_DENIED_METHODS` True.
- **Frozen exchange corpus.** A python3 read of `conformance/repository-exchange/v1/rejected.json` found 6 mutations: flip-trailer, nested-exchange, reversed-branches, foreign-head, open-ancestry and trailing-byte.
- **Not independently reproduced.** I did not re-run the packet's reseal-removed mutation check, because it needs a tree edit. The in-test unresealed assertion at the same offset (`s3_g2_corrupt.rs`, after the populated loop) covers the same distinction.

Files read, with line ranges:
- `evidence/review/verdicts/corrupt_surface_decision/ariadne_contract_review-2c97c32.md` (whole file)
- `bench/live/CORRUPT-SURFACE-DECISION.md:1-282` (whole file)
- `git diff ab42a3a9..HEAD` for `crates/sley-repo/tests/s3_g2_corrupt.rs` (new :399-652), `bench/fixtures/sley2_live_judge.py` (:394, :1414-1433, :1490-1779), `bench/live/succ_witness_corrupt.py` and `bench/live/SUCCESSION-COVERAGE.md`
- `bench/live/tests/test_corrupt_resealed.py:1-149`
- all five logs under `bench/live/succ-trials-20260923/corrupt/`, in full
- `crates/sley-repo/src/exchange.rs:1355-1404`, `:1880-1934`, `:3225-3300`
- `crates/sley-repo/src/lib.rs:443-446`, `:700-814`, `:1395-1396`
- `docs/spec/REPOSITORY_EXCHANGE_V1.md:120-229`, `:330-369`, `:486-528`, `:600-634`
- `bench/corpus/v1/tasks.json:132-143`
- `bench/sley2/runner.py:85-134`
- `bench/fixtures/legacy/S2B-CORRUPT-001/arm_limitation.json` and `oracle.py:1-80`
- `bench/fixtures/raw/S2B-CORRUPT-001/fixture/test_program.py:1-47`
- a grep for `trial[- ]surface` under `bench/`

## Evidence checked

1. **Engine route.**
   - `import_repository_exchange` calls `preflight` first (`exchange.rs:1890`), and `classify_target` only after it (`:1891`). The first write, `install_stage_marker`, is at `:1896`.
   - Inside `preflight`, the order is:
     - envelope and exchange trailer (`:1359`);
     - epoch, payload and registry decode (`:1360-1379`);
     - tag-170 header check, which returns `EXCHANGE_PACK_INVALID` (`:1381-1383`);
     - `preflight_conformance_pack` (`:1384`);
     - the digest tree, with the section-1 leaf keyed by the pack's `pack.pack_id` from that preflight (`:1386-1397`).
   - The pack envelope decode returns `PackErrorCode::DigestMismatch` when the trailer is not `RepositoryPackId::derive` (`lib.rs:714-717`).
   - Object entries are field 1 id, field 2 declared length and field 3 stored bytes (`lib.rs:800-811`). The judge's `_embedded_objects` parser matches this.
   - The packet's §2 [P1] trace is accurate.

2. **Rust pin (`s3_g2_corrupt.rs:563-652`).**
   - It resolves the staged `base.pack` (`:414`, `:476-477`).
   - It asserts the embedded pack trailer equals `RepositoryPackId::derive`.
   - It locates each object's stored bytes uniquely.
   - It flips one interior byte, object 0 at 3/8 and the last object at 5/8, and reseals with the owner's own `RepositoryExchangeId::derive` (`:515`).
   - Owner equivalence: resealing the untouched exchange reproduces it byte for byte (`:568-576`).
   - It checks that only the object byte and the trailer bytes differ.
   - `import_code` requires `ExchangeError::Pack(_)`, the owner error and not a remap (`:553-560`).
   - Populated destination: each flip twice, with head tx, live count and a whole-file byte snapshot unchanged.
   - Unresealed control: `EXCHANGE_DIGEST_MISMATCH`.
   - Fresh destination: three refusals and the target is never created (`:640`). The clean exchange then imports onto the same head (`:643`).
   - I ran it and it is green. The packet's line citations (:476, :515, :553-560, :563, :568-576, :640, :643; `lib.rs:446`) all resolve correctly.

3. **Judge.**
   - `_judge_corrupt_pack_resealed` (`:1695`) is called after the regression (`:394`).
   - `_resealed_pack_vectors` (`:1619`) fails as a harness error unless the recomputed control equals `base.pack` (`:1639-1640`).
   - The fresh destination goes through `_pre_head_import` (`:1654`), with sha256 file snapshots around each refusal. The control must import, otherwise the verdict is `ORACLE_CORRUPT_UNRECOVERED`.
   - The populated destination goes through `_raw_request` (`:1754`) with identical retry bodies. Head tx and live count are rechecked on a fresh session.
   - A wrong code gives `ORACLE_CORRUPT_UNREFUSED` and an accepted import gives `ORACLE_CORRUPT_ACCEPTED`.
   - The docstrings at `:1414-1433` and `:1695+` label the path judge-side and correct the "one layer up" error.

4. **Unit tests (`test_corrupt_resealed.py`).**
   - `:74` pins offsets `(1301, 2637)`, equal to the Rust pin's printed offsets. It also checks exactly one object byte plus the trailer differ, and that the flips land in distinct objects.
   - `:95` checks that a diverging reseal is a harness error.
   - `:118`, `:127` and `:135` cover the refusal, the accepted stand-in (`ORACLE_CORRUPT_ACCEPTED`) and the unresealed stand-in (`ORACLE_CORRUPT_UNREFUSED` with `EXCHANGE_DIGEST_MISMATCH`).
   - All six ran and passed.

5. **Logs.**
   - `trial_corrupt_pos.log` holds verdict `accepted` with this evidence: fresh destination 3 × `PACK_DIGEST_MISMATCH` at offsets 1301/2637/1301, control accepted, populated destination 2×2 `PACK_DIGEST_MISMATCH`, head `0a0a398c…`, live_objects 5.
   - `neg_accepted` gives `ORACLE_CORRUPT_ACCEPTED`, `neg_unresealed` gives `ORACLE_CORRUPT_UNREFUSED` with detail `unresealed-standin: EXCHANGE_DIGEST_MISMATCH`, and `neg` gives `ORACLE_CORRUPT_UNRESTORED`.
   - Every trial log carries provenance: git_head 72036a4e, dirty_paths 0, and sha256 of the sley binary, the judge driver and `base.pack`.
   - `rust_s3_g2_corrupt.log` shows exit 0 and 2 passed.
   - The witness refuses to overwrite an existing log.
   - These match the packet table at `:164-170`.

6. **Frozen task and parity.**
   - `tasks.json:137-139` says "attempt import" with no actor, and requires `expected_code = PACK_DIGEST_MISMATCH` and `ref_advances = 0`.
   - `runner.py:90-94` records the allowlist as a digest-recorded run control and excludes bulk import as a whole-store move (`:115-117`). `exchange.import` is denied at `:126`.
   - Legacy: `arm_limitation.json` records the oracle-side sha256 gate. The oracle hashes the payload and reads `ref.txt` before any verdict (`oracle.py:30-66`).
   - Raw: the frozen `test_program.py:10-43` builds `corrupt_pack()` and calls the arm's `import_pack` itself, including a retry-determinism check.
   - In all three arms, the corruption and the import attempt are made by the oracle or test harness, never by an agent tool call. The packet's §4 parity facts hold.

7. **Spec phase order.**
   - `REPOSITORY_EXCHANGE_V1.md:349-354` lists step 2 ("every declared identity, and the complete digest tree") before step 3 (the S20-170 preflight). Only step 8, persistence, is stated to run "in this exact order" (`:362`). Preflight precedence is otherwise not stated.
   - The section-1 leaf identifier is the `RepositoryPackId` of the embedded pack (`:209`).
   - `EXCHANGE_PACK_INVALID` "is returned exactly when the embedded bytes are not a tag-170 version-1 pack" (`:522-524`).
   - Lower-layer `PACK_*` codes are "preserved, never remapped" (`:518-519`).
   - The required test list demands "a nested-exchange test expecting `EXCHANGE_PACK_INVALID`" (`:628-629`).
   - The frozen `nested-exchange` mutation is built with `object_pack = exchange.stored_bytes` (`exchange.rs:3243-3250`, emitted at `:3291`). It replaces the embedded pack and pins `EXCHANGE_PACK_INVALID`.

## Findings

[P3] [factual-error] bench/live/CORRUPT-SURFACE-DECISION.md:112-114 - The packet says `conformance/repository-exchange/v1/rejected.json` "has 6 mutations, and none alters the embedded pack". The frozen `nested-exchange` mutation replaces the embedded pack with a whole tag-540 exchange (`crates/sley-repo/src/exchange.rs:3243-3250,3291`) and pins `EXCHANGE_PACK_INVALID`. That is an embedded-pack check the code runs before the digest tree (`exchange.rs:1381-1383` vs `:1386-1396`), and the spec requires that code (`docs/spec/REPOSITORY_EXCHANGE_V1.md:522-524,628-629`). The narrower claim is true: no frozen vector pins S20-170 preflight (PACK_DIGEST_MISMATCH) against the digest tree. But the sentence as written is false, and it omits the one frozen vector that already constrains the ordering question. - Closure evidence: reword it to "no mutation alters embedded-pack content under a valid tag-170 header; `nested-exchange` pins the tag-170 shape check (`EXCHANGE_PACK_INVALID`) ahead of the tree".

[P4] [record-consistency] bench/live/SUCCESSION-COVERAGE.md:31 - The CORRUPT row opens with "ACCEPTED `succ-trials-20260923/corrupt/trial_corrupt_pos.log`". The packet (`bench/live/CORRUPT-SURFACE-DECISION.md:215-216,275-281`) and the same file's prose entry ("No acceptance is claimed until the owner rules") say the obligation is not accepted. The word refers to the judge's JSON verdict, but a reader can take it as obligation acceptance. - Closure evidence: label it "judge verdict accepted; obligation pending owner ruling", or update the row when the actor ruling below is recorded.

Prior findings (from `ariadne_contract_review-2c97c32.md`):

PRIOR [P1] factual-premise (PACK_DIGEST_MISMATCH "not drivable"): CLOSED. The packet §2 corrects the premise. The executed Rust pin `s3_corrupt_exchange_resealed_embedded_pack` (2 passed, re-run by me) and the judge-path log `trial_corrupt_pos.log` show exact `PACK_DIGEST_MISMATCH` through `import_repository_exchange` / `exchange.import`, with:
- two independent object-byte flips, each refused twice;
- head tx, live count (5) and file bytes unchanged;
- the clean exchange accepted afterwards.

The false comments at `s3_g2_corrupt.rs:4-18` and `sley2_live_judge.py:1414-1433` are corrected.

PRIOR [P2] mislabeled-evidence ("trial-surface path"): CLOSED. The packet (`:118-135`), the judge docstrings, the witness docstring and SUCCESSION-COVERAGE all label both exchange paths judge-side. The remaining "trial surface" phrase in `succ_witness_corrupt.py:2` correctly describes the propose/finish constant restore.

PRIOR [P3] unsupported-claim (TOOL_METHODS "pinned by test"): CLOSED by rewording, which was an allowed closure. The packet (`:137-155`) and the coverage entry now call it "equal at this checkpoint, not test-pinned", and my python3 check confirms equality (18 entries). The `sley2_tool.py:10,84` comment is still inaccurate on this branch, and the packet acknowledges that.

PRIOR [P3] evidence-gap (negative with no reject code): CLOSED. The new logs carry JSON verdicts with exact codes. The negatives that target the rejection path (`ORACLE_CORRUPT_ACCEPTED`; `ORACLE_CORRUPT_UNREFUSED` with `EXCHANGE_DIGEST_MISMATCH`) are logged and unit-pinned. I re-ran the unit pins and they pass.

PRIOR [P3] interpretive-constraint (actor): CLOSED. The packet withdraws the claim at `:194-198` and restates it as an open owner question at §4. It is ruled below.

PRIOR [P4] note (trailer-byte pin, vacuous ref at the pack layer): CLOSED. The packet (`:200-211`) cites `s3_g2_corrupt.rs` as the object-byte pin, states that library pack pins cannot demonstrate ref or head retention, and moves head retention to the exchange-layer pin.

## Assessment

- **Implementation and evidence.** The executed Rust pin, the judge path, the unit tests and the provenance-bearing logs agree. Together they show:
  - one canonical object byte altered inside the staged exchange's embedded pack;
  - the unkeyed exchange trailer resealed, with owner equality proven;
  - import through the existing `exchange.import` / `import_repository_exchange`;
  - the exact PACK owner code, never remapped;
  - no write and no head or store movement;
  - deterministic retry;
  - the clean exchange then accepted in the same destination.

  No production source changed, and the logs' commit (72036a4e) has code identical to HEAD.
- **Packet accuracy.** Its facts are correct except the P3 sentence about the conformance corpus. That sentence does not bear on the evidence or on either ruling's outcome.
- **Limitations.** I did not rebuild the sley binary that the unittest integration class used. Its correctness is supported by the unchanged production source and by the Rust pin, which exercises the same function in-process.

**Actor ruling.** A judge-side corruption plus import attempt satisfies "attempt import":
- The frozen text (`bench/corpus/v1/tasks.json:137`) names no actor.
- In every arm's frozen fixture, the attempt is made by the oracle or test harness: `bench/fixtures/legacy/S2B-CORRUPT-001/oracle.py:30-66` with `arm_limitation.json`, and `bench/fixtures/raw/S2B-CORRUPT-001/fixture/test_program.py:10-43`.
- The sley2 run control deliberately excludes bulk import from the agent, digest-recorded (`bench/sley2/runner.py:90-94,115-117,126`).
- Neither alternative under B is warranted:
  - Widening that control would contradict its stated purpose.
  - A `sley_2_0` arm-limitation record would misstate a capability the engine demonstrably has. The legacy record exists because that engine lacks the symbol; the sley2 judge drives the real engine owner.
- Two conditions apply:
  - The CORRUPT record must keep stating that this evidence is agent-independent (judge-side), and that the agent-bound part is only the constant-restore smoke check.
  - The acceptance claims no agent-driven import.

**Order ruling.** The code is authoritative (`crates/sley-repo/src/exchange.rs:1381-1397`: tag-170 check, then `preflight_conformance_pack`, then digest tree). The spec phase list must be amended to match; the code needs no change. The reasons:
- The spec's preflight list states no precedence; only persistence is "in this exact order" (`REPOSITORY_EXCHANGE_V1.md:362`).
- The section-1 leaf identifier is the embedded pack's `RepositoryPackId` (`:209`), which only the S20-170 preflight establishes, and the embedded pack's declared identity is part of step 2's own "every declared identity".
- `EXCHANGE_PACK_INVALID` must be returned "exactly when" the embedded bytes are not a tag-170 pack (`:522-524`), and the frozen `nested-exchange` vector pins that ahead of the tree (`:628-629`; `exchange.rs:3243-3250,3291`).
- `PACK_*` codes are preserved, never remapped (`:518-519`). A tree-first reading would turn this corpus vector into `EXCHANGE_DIGEST_TREE_MISMATCH`, contradicting the frozen corpus `expected_code`.

The amendment to `docs/spec/REPOSITORY_EXCHANGE_V1.md`:
1. Strike "and the complete digest tree" from step 2 (`:351-352`).
2. Insert, after step 3 (`:353-354`), a step: "verify the complete digest tree, whose section-1 identifier is the `RepositoryPackId` verified by step 3".
3. Add to step 3 that the tag-170 version-1 check (`EXCHANGE_PACK_INVALID`) precedes the S20-170 preflight.
4. State that preflight steps 1-7 are a precedence order in which the first failing step's code is returned.
5. Add a frozen `rejected.json` mutation for this vector (a resealed embedded-pack object-byte flip returning `PACK_DIGEST_MISMATCH`), emitted by the existing generator path at `exchange.rs:3289-3299`, so the order is conformance-pinned rather than pinned only by `s3_g2_corrupt.rs`.

This amendment goes through normal spec review.

RULING_ACTOR: A
RULING_ORDER: CODE

VERDICT: PASS_0_P0_0_P1_0_P2_1_P3_1_P4_PRIOR_P3_P4_CLOSED
SECTION: corrupt_surface_decision
FIELD: ariadne_contract_review_revision_2
SCOPE_SHA: 01dd20cb3604643a0c833f2d9306ab4e365d272e
FINDINGS: [P3] [factual-error] bench/live/CORRUPT-SURFACE-DECISION.md:112-114 - The packet says rejected.json's 6 mutations include none that "alters the embedded pack", but the frozen `nested-exchange` mutation replaces the embedded pack (crates/sley-repo/src/exchange.rs:3243-3250,3291) and pins EXCHANGE_PACK_INVALID ahead of the digest tree (exchange.rs:1381-1383 vs :1386-1396; docs/spec/REPOSITORY_EXCHANGE_V1.md:522-524,628-629). Only the narrower claim is true: no vector pins S20-170 preflight against the tree. - Closure evidence: reword to name nested-exchange as pinning the tag-170 shape check before the tree, and say no mutation alters embedded-pack content under a valid header. | [P4] [record-consistency] bench/live/SUCCESSION-COVERAGE.md:31 - The CORRUPT row opens "ACCEPTED" while the packet (:215-216,275-281) and the coverage prose say no obligation acceptance is claimed until the owner rules. The label means the judge verdict but reads as obligation acceptance. - Closure evidence: relabel it "judge verdict accepted; obligation pending owner ruling", or update the row once RULING_ACTOR A is recorded.
SUMMARY: All six prior findings are closed. I re-ran `s3_corrupt_exchange_resealed_embedded_pack` (2 passed) and `test_corrupt_resealed` (6 OK), and they agree with the provenance-bearing logs: a resealed one-object-byte flip in the staged exchange's embedded pack gets exact PACK_DIGEST_MISMATCH through `exchange.import`, twice per flip, with head, live count and files unchanged, and the clean exchange is then accepted. The packet's facts are correct except one P3 misstatement about the frozen nested-exchange vector, plus a P4 labelling inconsistency in the coverage table. Ruling A: judge-side attempts satisfy "attempt import", in parity with the legacy and raw oracle-side attempts. Ruling CODE: the spec's import-phase list (REPOSITORY_EXCHANGE_V1.md:349-354) must move the digest-tree check after the embedded-pack preflight, state preflight precedence explicitly, and gain a frozen resealed-pack mutation.
