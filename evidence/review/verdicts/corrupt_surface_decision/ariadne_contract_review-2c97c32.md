<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2c97c32f4dd5d87040cffee2af4b7519ed0f5bdf; role: ariadne; field: ariadne_contract_review; dispatched: 2026-09-23T03:14:38Z; duration_s: 345; process_exit_code: 0 -->
# Ariadne Council review — corrupt_surface_decision

Harness: claude-code
Reviewed checkpoint: 2c97c32f4dd5d87040cffee2af4b7519ed0f5bdf

What I verified myself:
- `git rev-parse HEAD` returned `2c97c32f4dd5d87040cffee2af4b7519ed0f5bdf`, so the scope matches. `git status --short` was clean, and I left the tree unmodified.
- `cargo test -p sley-repo --lib outer_digest_tamper_fails_before_promotion` exited 0 with `test tests::outer_digest_tamper_fails_before_promotion ... ok` and `1 passed; 0 failed`.
- `cargo test -p sley-repo --test s3_g2_corrupt` exited 0 with `s3_corrupt_fixture_conformance ... ok` and `1 passed; 0 failed; 3 ignored` (the negative handshakes are ignored by design).
- `python3 -m unittest bench.sley2.tests.test_runner bench.live.tests.test_sley2_tool bench.live.tests.test_mediated_gateway` printed `Ran 51 tests`, `OK (skipped=28)`.
- A plain python3 heredoc imported both allowlists. `sley2_tool.TOOL_METHODS == runner.ARM_AFFORDANCES` is True and has 18 entries. `"exchange.import" in runner.ARM_DENIED_METHODS` is True. No `test*.py` under `bench/` names `TOOL_METHODS`, and no file names both `sley2_tool` and `ARM_AFFORDANCES`.
- A python3 heredoc on `bench/fixtures/sley2/S2B-CORRUPT-001/base.pack` (5825 bytes) found an outer `SLEYSCB1` envelope with tag varint `9c 04` (540), which makes it a v1 repository exchange. A second `SLEYSCB1` header, the embedded tag-170 pack, begins at offset 52.
- I tried to recompute the blake3 trailer with `uv run --frozen --project oracle/scb1`, but the sandbox refused the command. The system python3 has no blake3. The finding that PACK_DIGEST_MISMATCH is reachable (below) therefore rests on a line-by-line static trace, not an executed vector.

Files read, with line ranges:
- `bench/live/CORRUPT-SURFACE-DECISION.md:1-66` (the whole file)
- `bench/corpus/v1/tasks.json:134-150` (S2B-CORRUPT-001)
- `bench/live/sley2_tool.py:1-340`
- `bench/live/mediated_sley.py:48-62`, `:220`
- `bench/sley2/runner.py:85-145`
- `bench/sley2/tests/test_runner.py:551-552`
- `bench/fixtures/sley2_live_judge.py:370-400`, `:1413-1507`
- `crates/sley-protocol/src/lib.rs:630-720`
- `crates/sley-protocol/src/server.rs:1128-1149`, `:1284`, `:3131-3146`
- `crates/sley-repo/src/exchange.rs:100-335`, `:645-679`, `:930-942`, `:1358-1396`, `:1436-1450`, `:1885-1930`
- `crates/sley-repo/src/lib.rs:415-455`, `:700-718`, `:1395-1408`
- `crates/sley-repo/src/native_exchange.rs:1-30`
- `crates/sley-id/src/lib.rs:128-172`, `:453-501`
- `crates/sley-repo/tests/s3_g2_corrupt.rs:1-30`, and test function list `:251-389`
- `docs/spec/REPOSITORY_EXCHANGE_V1.md:340-395`, `:490-525`
- `docs/spec/REPOSITORY_PACK_V1.md:135-160`
- `bench/fixtures/README.md:172-180`
- `bench/live/SUCCESSION-COVERAGE.md:31`, `:455-470`
- `bench/live/succ-trials-20260921/trial_corrupt_recheck.log` and `trial_corrupt_neg_recheck.log` (175 bytes each, read in full)

## Evidence checked

1. **Frozen authority.** `tasks.json` S2B-CORRUPT-001 has this goal: "Alter one canonical object byte in an exchange pack, attempt import, reject it with an exact digest failure". It sets `strict_oracle.expected_code = "PACK_DIGEST_MISMATCH"` and `ref_advances = 0`, and its required outcomes include "old root remains accepted" and "recovery is deterministic". The task names no actor for "attempt import". The packet's restatement at lines 10-13 is faithful.

2. **Protocol method table.** `crates/sley-protocol/src/lib.rs:672-719` names exactly one import method, `exchange.import`. At `server.rs:1284` it dispatches to `exchange_import`, which at `:3136-3137` calls `import_repository_exchange` and maps the error to `owner(error.code(), …)`. No protocol method calls the symbol `import_conformance_pack`. The literal claim at packet line 33 is therefore true.

3. **`exchange.import` reaches the PACK owner and preserves its codes.** This contradicts the conclusion the packet draws at lines 29-34. The chain is:
   - `import_repository_exchange` (`exchange.rs:1890`) calls `preflight` first.
   - In `preflight`, `decode_envelope` (`exchange.rs:645-679`) checks the exchange trailer. The trailer is `RepositoryExchangeId::derive(input[..len-32])` (`:674-676`), which is `blake3("sley2.repository-exchange.v1" ‖ preimage)` with no key (`sley-id/src/lib.rs:144,167-171,501`). Anyone holding the bytes can recompute it.
   - After the header check at `:1381`, `preflight` runs `preflight_conformance_pack(&decoded.object_pack, verifier)?` (`exchange.rs:1384`). That is the PACK owner's own preflight. Its envelope decode rejects a stale pack trailer with `PackErrorCode::DigestMismatch`, i.e. `"PACK_DIGEST_MISMATCH"` (`lib.rs:714-717`, `:129`). This happens before the exchange digest-tree check (`exchange.rs:1386-1396`), before target classification (spec step 7), and before any write (spec step 8, `REPOSITORY_EXCHANGE_V1.md:346-356`).
   - `ExchangeError::Pack(e).code()` returns `e.symbol()` verbatim (`exchange.rs:273-281`). The spec requires this: "`PACK_*` … are preserved, never remapped" (`REPOSITORY_EXCHANGE_V1.md:518-519`).
   - `import_conformance_pack` is itself only `preflight_conformance_pack` plus `promote_pack_objects` (`lib.rs:415-425`). The exchange importer calls both (`exchange.rs:1384`, `:1929`).
   - Consequence: flip one canonical object byte inside the embedded pack at offset 52 of the staged `base.pack`, then recompute the unkeyed exchange trailer. `exchange.import` should then return exact `PACK_DIGEST_MISMATCH` before any write, over the existing protocol with no new method.
   - The same session-bound dispatch path already delivers `EXCHANGE_DIGEST_MISMATCH` to the judge (`server.rs:1138-1149`, then `:1284`).

4. **The "trial-surface path" is judge-side.** `_judge_corrupt_exchange` (`sley2_live_judge.py:1413-1480`):
   - flips bits in the staged `base.pack`;
   - seeds a fresh temporary workspace (`:1439`, `:1449`);
   - calls `session._raw_request("exchange.import", broken_hex)` (`:1456`). `_raw_request` is the privileged path described as "the agent's allowlist never covers these, and dispatch() cannot reach them" (`sley2_tool.py:300-302`).

   The trial agent never touches these bytes, and the check does not depend on the agent's candidate. The coverage record agrees: it says "judge-side … no agent path can attempt it" (`SUCCESSION-COVERAGE.md:466-469`).

5. **Allowlists.** `TOOL_METHODS` (`sley2_tool.py:88-107`) excludes every import method, and its stated exclusion is at `:84-87`. The runner lists `exchange.import` in `ARM_DENIED_METHODS` (`runner.py:126`), and `test_runner.py:552` asserts it is absent from `ARM_AFFORDANCES`. The gateway's `raw` is gated on `TOOL_METHODS` (`mediated_sley.py:220`), and `ALLOWED_COMMANDS` (`:48-51`) adds no import. The packet's claims about exclusion and the gateway mirror hold. `side` imports frozen side packs into a throwaway repo; agents cannot supply those bytes.

6. **S3 G2 pin.** `outer_digest_tamper_fails_before_promotion` (`lib.rs:1395-1408`) flips the pack's last byte, which is a trailer byte and not a canonical object byte. It asserts `PACK_DIGEST_MISMATCH` and that no `objects` directory exists. It is green. The object-byte variant is `s3_g2_corrupt.rs` (`flip_object_byte`, `:178`, `:274-290`), also green. The pack layer writes no ref (`REPOSITORY_PACK_V1.md:143-144`), so "ref advances 0" is vacuous at that layer.

7. **Recheck logs.** `trial_corrupt_recheck.log` contains only `propose valid True`, `finished True`, `judge exit: 0`. `trial_corrupt_neg_recheck.log` contains `judge exit: 1` and no reject code. The negative is a wrong-value constant, which exercises `_judge_corrupt_value` (the smoke check, `:1483-1507`), not the exchange-rejection path.

## Findings

[P1] [factual-premise] bench/live/CORRUPT-SURFACE-DECISION.md:29-34,50-59 - The packet says the corpus path "is not drivable today" because no protocol method reaches `import_conformance_pack`. That leaves out that `exchange.import` runs the PACK owner's `preflight_conformance_pack` on the embedded pack (`crates/sley-repo/src/exchange.rs:1384`) and returns `PACK_*` codes verbatim (`exchange.rs:273-281`; `docs/spec/REPOSITORY_EXCHANGE_V1.md:518-519`). The exchange trailer is an unkeyed digest that anyone can recompute (`exchange.rs:674-676`; `crates/sley-id/src/lib.rs:167-171`). An exchange with one embedded canonical object byte flipped and the trailer resealed therefore reaches exact `PACK_DIGEST_MISMATCH` before any write (`crates/sley-repo/src/lib.rs:714-717`), matching the corpus text "alter one canonical object byte in an exchange pack". All three options are framed on the false premise that PACK_DIGEST_MISMATCH needs a new surface. The same mistake appears in `crates/sley-repo/tests/s3_g2_corrupt.rs:5-7` ("trailer gate fires first on ANY byte flip") and `bench/fixtures/sley2_live_judge.py:1420-1422` ("PACK_DIGEST_MISMATCH lives one layer up"). - Closure evidence: a corrected packet, plus an executed vector showing `exchange.import` of the resealed `base.pack` returns exact `PACK_DIGEST_MISMATCH` twice with head tx and object count unchanged, while the clean pack still imports. Either an executed Rust test in `sley-repo`/`sley-protocol` or a judge-path run log is acceptable, and I did not execute this vector myself.

[P2] [mislabeled-evidence] bench/live/CORRUPT-SURFACE-DECISION.md:17-22,55-58 - The packet calls `_judge_corrupt_exchange` the "Trial-surface path", and option 2 relies on "EXCHANGE_DIGEST_MISMATCH on the trial surface". In fact the judge corrupts and imports the bytes itself through the privileged `_raw_request` (`bench/fixtures/sley2_live_judge.py:1439-1456`; `bench/live/sley2_tool.py:300-302`), independently of the agent. That contradicts `bench/live/SUCCESSION-COVERAGE.md:466-469` ("judge-side … no agent path can attempt it") and the packet's own standard at lines 41-44, under which a judge-side drive is "a judge self-test". - Closure evidence: relabel the path as judge-side in the packet and drop "trial surface" from option 2's wording, or supply a transcript showing an agent-attempted import.

[P3] [unsupported-claim] bench/live/CORRUPT-SURFACE-DECISION.md:35-36 - The packet says `TOOL_METHODS` is "pinned equal to the smoke runner's allowlist" (repeating `bench/live/sley2_tool.py:10,84`). The two tuples are equal at this checkpoint (checked by python3 above), but no test under `bench/` references `TOOL_METHODS`, and `bench/sley2/tests/test_runner.py` pins only `ARM_AFFORDANCES`. The "pinned by test" guarantee does not exist. - Closure evidence: a unittest asserting `sley2_tool.TOOL_METHODS == runner.ARM_AFFORDANCES`, or rewording the claim to "equal at 2c97c32f".

[P3] [evidence-gap] bench/live/CORRUPT-SURFACE-DECISION.md:21-22 - The packet says the negative (ORACLE_CORRUPT_UNRESTORED) was rechecked. `bench/live/succ-trials-20260921/trial_corrupt_neg_recheck.log` (175 bytes) records only `judge exit: 1` and no reject code. The negative also exercises the constant-value smoke check (`sley2_live_judge.py:1483-1507`), not the corruption-rejection path, so it gives no negative coverage for the exchange or PACK rejection. - Closure evidence: a log carrying the exact reject symbol, and a negative that targets the rejection path, such as an unflipped or accepted-corruption control.

[P3] [interpretive-constraint] bench/live/CORRUPT-SURFACE-DECISION.md:41-44 - The packet rules out a judge-side drive because the task "requires" an "attempt-bound interaction (the trial agent attempting the import through its surface)". The frozen task text (`bench/corpus/v1/tasks.json:134-150`) names no actor. The frozen run control denies `exchange.import` to every arm (`bench/sley2/runner.py:126`; `test_runner.py:552`), and the legacy arm records oracle-side verification (`bench/fixtures/legacy/S2B-CORRUPT-001/arm_limitation.json`). The packet's constraint is an unratified reading of the task, yet it decides which evidence may count. - Closure evidence: an owner-contract citation for the actor requirement, or restating it as an open question for the owner.

[P4] [note] bench/live/CORRUPT-SURFACE-DECISION.md:23-27 - The cited pin `outer_digest_tamper_fails_before_promotion` (`crates/sley-repo/src/lib.rs:1401-1402`) flips a trailer byte, not "one canonical object byte". The object-byte pin is `crates/sley-repo/tests/s3_g2_corrupt.rs:178,274-290`. At the pack layer "ref advances 0" is vacuous (`docs/spec/REPOSITORY_PACK_V1.md:143-144`). - Closure evidence: cite `s3_g2_corrupt.rs` as the object-byte pin and state that library pins cannot demonstrate ref or head retention.

## Assessment

- **Literal claims that check out:**
  - The protocol has exactly one import method.
  - No method calls the `import_conformance_pack` symbol.
  - `TOOL_METHODS` and `ALLOWED_COMMANDS` exclude import.
  - The EXCHANGE_DIGEST_MISMATCH regression and the S3 G2 library pins are green.
- **Where the packet is wrong:**
  - The central inference, that PACK_DIGEST_MISMATCH cannot be reached without a new surface, is wrong. `exchange.import` runs the PACK owner's preflight on the embedded pack and returns its codes unmapped. Resealing the unkeyed exchange trailer after an object-byte flip reaches the exact corpus code on the existing protocol (P1). This is shown by static trace only; the reseal vector was not run.
  - The existing exchange path is judge-side, not "trial-surface" (P2), and the packet's own self-test standard rules it out as agent-attempted evidence.
- **Consequence:** because factual claims are wrong, the verdict is REVISE.

The decision still has to be one of the three options as offered:

- **Option 1 is rejected.** It would widen the frozen run-control allowlist. `ARM_AFFORDANCES` is digest-recorded as a run control (`runner.py:90-94`), and `test_runner.py:552` asserts `exchange.import` is absent. P1 also shows a new PACK-routing method is unnecessary.
- **Option 2 is rejected.** As worded, it counts EXCHANGE_DIGEST_MISMATCH toward an oracle whose exact code is `PACK_DIGEST_MISMATCH` (`tasks.json` `strict_oracle.expected_code`). That contradicts the exact-code requirement and the spec's never-remap rule (`REPOSITORY_EXCHANGE_V1.md:518-519`). It also rests on the P2 mislabel.
- **Option 3 is chosen.** It is the only offered option consistent with the frozen contract: no PACK-layer acceptance is claimed, the exchange regression stands only as a regression, and `ga_claimed=false` is retained.

**What closes the open obligation** (to be proposed in a revised packet, not assumed here):
1. A judge or fixture vector that flips one canonical object byte inside the embedded pack of the staged `base.pack` and reseals the exchange trailer.
2. Driving that vector through `exchange.import` must return exact `PACK_DIGEST_MISMATCH` on two independent flips, with destination head tx and live object count unchanged.
3. A clean re-import must be accepted as the positive control, and the result must be deterministic on retry.
4. The vector must be pinned by a Rust test.
5. An owner ruling is needed on whether a judge-side attempt satisfies "attempt import", given that every arm is denied `exchange.import`.

DECISION: OPTION_3

VERDICT: REVISE_0_P0_1_P1_1_P2_3_P3_1_P4
SECTION: corrupt_surface_decision
FIELD: ariadne_contract_review
SCOPE_SHA: 2c97c32f4dd5d87040cffee2af4b7519ed0f5bdf
FINDINGS: [P1] [factual-premise] bench/live/CORRUPT-SURFACE-DECISION.md:29-34,50-59 - The packet says the corpus path "is not drivable today" because no protocol method reaches `import_conformance_pack`. That leaves out that `exchange.import` runs the PACK owner's `preflight_conformance_pack` on the embedded pack (`crates/sley-repo/src/exchange.rs:1384`) and returns `PACK_*` codes verbatim (`exchange.rs:273-281`; `docs/spec/REPOSITORY_EXCHANGE_V1.md:518-519`). The exchange trailer is an unkeyed digest that anyone can recompute (`exchange.rs:674-676`; `crates/sley-id/src/lib.rs:167-171`). An exchange with one embedded canonical object byte flipped and the trailer resealed therefore reaches exact `PACK_DIGEST_MISMATCH` before any write (`crates/sley-repo/src/lib.rs:714-717`), matching the corpus text "alter one canonical object byte in an exchange pack". All three options are framed on the false premise that PACK_DIGEST_MISMATCH needs a new surface. The same mistake appears in `crates/sley-repo/tests/s3_g2_corrupt.rs:5-7` ("trailer gate fires first on ANY byte flip") and `bench/fixtures/sley2_live_judge.py:1420-1422` ("PACK_DIGEST_MISMATCH lives one layer up"). - Closure evidence: a corrected packet, plus an executed vector showing `exchange.import` of the resealed `base.pack` returns exact `PACK_DIGEST_MISMATCH` twice with head tx and object count unchanged, while the clean pack still imports. Either an executed Rust test in `sley-repo`/`sley-protocol` or a judge-path run log is acceptable, and I did not execute this vector myself. | [P2] [mislabeled-evidence] bench/live/CORRUPT-SURFACE-DECISION.md:17-22,55-58 - The packet calls `_judge_corrupt_exchange` the "Trial-surface path", and option 2 relies on "EXCHANGE_DIGEST_MISMATCH on the trial surface". In fact the judge corrupts and imports the bytes itself through the privileged `_raw_request` (`bench/fixtures/sley2_live_judge.py:1439-1456`; `bench/live/sley2_tool.py:300-302`), independently of the agent. That contradicts `bench/live/SUCCESSION-COVERAGE.md:466-469` ("judge-side … no agent path can attempt it") and the packet's own standard at lines 41-44, under which a judge-side drive is "a judge self-test". - Closure evidence: relabel the path as judge-side in the packet and drop "trial surface" from option 2's wording, or supply a transcript showing an agent-attempted import. | [P3] [unsupported-claim] bench/live/CORRUPT-SURFACE-DECISION.md:35-36 - The packet says `TOOL_METHODS` is "pinned equal to the smoke runner's allowlist" (repeating `bench/live/sley2_tool.py:10,84`). The two tuples are equal at this checkpoint (checked by python3 above), but no test under `bench/` references `TOOL_METHODS`, and `bench/sley2/tests/test_runner.py` pins only `ARM_AFFORDANCES`. The "pinned by test" guarantee does not exist. - Closure evidence: a unittest asserting `sley2_tool.TOOL_METHODS == runner.ARM_AFFORDANCES`, or rewording the claim to "equal at 2c97c32f". | [P3] [evidence-gap] bench/live/CORRUPT-SURFACE-DECISION.md:21-22 - The packet says the negative (ORACLE_CORRUPT_UNRESTORED) was rechecked. `bench/live/succ-trials-20260921/trial_corrupt_neg_recheck.log` (175 bytes) records only `judge exit: 1` and no reject code. The negative also exercises the constant-value smoke check (`sley2_live_judge.py:1483-1507`), not the corruption-rejection path, so it gives no negative coverage for the exchange or PACK rejection. - Closure evidence: a log carrying the exact reject symbol, and a negative that targets the rejection path, such as an unflipped or accepted-corruption control. | [P3] [interpretive-constraint] bench/live/CORRUPT-SURFACE-DECISION.md:41-44 - The packet rules out a judge-side drive because the task "requires" an "attempt-bound interaction (the trial agent attempting the import through its surface)". The frozen task text (`bench/corpus/v1/tasks.json:134-150`) names no actor. The frozen run control denies `exchange.import` to every arm (`bench/sley2/runner.py:126`; `test_runner.py:552`), and the legacy arm records oracle-side verification (`bench/fixtures/legacy/S2B-CORRUPT-001/arm_limitation.json`). The packet's constraint is an unratified reading of the task, yet it decides which evidence may count. - Closure evidence: an owner-contract citation for the actor requirement, or restating it as an open question for the owner. | [P4] [note] bench/live/CORRUPT-SURFACE-DECISION.md:23-27 - The cited pin `outer_digest_tamper_fails_before_promotion` (`crates/sley-repo/src/lib.rs:1401-1402`) flips a trailer byte, not "one canonical object byte". The object-byte pin is `crates/sley-repo/tests/s3_g2_corrupt.rs:178,274-290`. At the pack layer "ref advances 0" is vacuous (`docs/spec/REPOSITORY_PACK_V1.md:143-144`). - Closure evidence: cite `s3_g2_corrupt.rs` as the object-byte pin and state that library pins cannot demonstrate ref or head retention.
SUMMARY: The packet's literal claims about the method table, the allowlist exclusions and the green pins hold, but its central inference is wrong. `exchange.import` already runs the PACK owner's preflight on the embedded pack and returns `PACK_*` codes verbatim, so resealing the unkeyed exchange trailer after an object-byte flip should reach exact `PACK_DIGEST_MISMATCH` on the existing protocol; this is a static trace, not yet an executed vector. The existing exchange regression is also judge-side, not "trial-surface". Of the offered options only OPTION_3 conforms to the frozen exact-code oracle and allowlist, and closure needs an executed resealed-embedded-pack vector with head and store unchanged, a Rust pin, and an owner ruling on who must attempt the import.
