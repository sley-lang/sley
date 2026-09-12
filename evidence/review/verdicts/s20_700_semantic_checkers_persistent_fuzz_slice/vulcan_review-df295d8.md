# Vulcan qualifying re-review — s20_700_semantic_checkers_persistent_fuzz_slice (round-4 rescope, lane at df295d8)

**Vulcan qualifying re-review — s20_700_semantic_checkers_persistent_fuzz_slice (round-4 rescope, lane at df295d8)**

Read-only; nothing written in-repo. HEAD = `5966d83`; `git diff df295d8..HEAD --stat` touches only two transcripts and the `last_local_proof`/note block of `machine-summary.json` (`:2711-2762`), so the lane at HEAD is the lane at `df295d8`. `git diff 67ad3a3..df295d8 -- crates/` is 0 bytes: no engine change.

**Assumptions (stated up front):**
- The sandbox denied execution of the built `ssmc_graph_cfg_checker`, `python3`, and scratch writes (three attempts). Every (c) trace below is a static byte-by-byte walk through `Cursor`/`apply_mutation` and the engine phase order; the owner's proof run is the only execution evidence, and it is not independent.
- `docs/spec/ERROR_CODES_V1.md` does not contain the `GRAPH_*`/`CFG_*` strings (they live in `docs/spec/CFG_VALIDATION_V1.md`); every code cited below is grounded on `crates/sley-check/src/cfg.rs:84-104` (`CfgErrorCode::as_str`) or `crates/sley-check/src/lib.rs:77-97` (`TypeErrorCode::as_str`).
- Engine phase order used for all derivations (`cfg.rs:261-304`, then `:321-336`): top-level limits → `validate_sorted_unique(effects/contracts)` → `index_unique` params/blocks/ops (`GRAPH_DUPLICATE_ENTITY`) → block inventory (`:275-279`) → entry (`:280-286`) → parameter inventory (`:662-711`) → operation inventory (`:713-746`) → `check_type(result_type)` (`:298`) → `build_successors` (`:748-785`, entry `Required` at `:753`, `TARGET_INVALID` at `:780`) → reachability (`:802-816`) → dominators → operand uses → terminators (`:338-371`).

## VERIFIED

**(a) Single branch is exact (no under-wide set).** Per-arm single-mutation outcomes re-derived on the four templates (T0 `:126-151`, T1 `:153-198`, T2 `:200-250`, T3 `:252-282`):

| arm | engine-reachable under one mutation | set status |
|---|---|---|
| 0 | DUPLICATE (`:268/:646`), ENTRY_INVALID (`:284`) | exact |
| 1 | DUPLICATE (`:655`), UNRESOLVED (`:676`), OWNER (`:678` — T1/T2 block-param push) | exact |
| 2 | ORDINAL (`:681`, T2 only) | exact |
| 3 | TYPE_PARAMETER_OUT_OF_SCOPE (`lib.rs:520-522`), RETURN_TYPE (`:345`) | over-wide (TYPE_CODES) |
| 4 | ENTRY_INVALID (`:283`), REACHABILITY (`:812`) | exact |
| 5 | DUPLICATE (`:656`), INVENTORY (`:278`) | exact |
| 6/31 | none (reorder re-indexes consistently; ≤1 op) | exact |
| 7/8 | INVENTORY (`:633`) — effects/contracts used nowhere else in cfg.rs | exact |
| 9 | DUPLICATE (`:646`), UNRESOLVED (`:676/:696`) | exact |
| 10/11 | OWNER (`:678/:698`) | exact |
| 12/25 | ORDINAL (`:681/:701`, `:734`) | exact |
| 13 | TYPE_PARAMETER_OUT_OF_SCOPE (`:683`), BOOL_REQUIRED (`:352`), RETURN_TYPE, TARGET_ARGUMENTS (`:389`) | over-wide (TYPE_CODES) |
| 14 | DUPLICATE, INVENTORY (`:276-278`) | exact |
| 15 | ENTRY_INVALID (`:285`), OWNER (`:688`) | exact |
| 16 | DUPLICATE (`:690`), UNRESOLVED (`:696`), OWNER (`:698`) | over-wide: INVENTORY unreachable (`:696` precedes `:706`) |
| 17 | DUPLICATE (`:723`), UNRESOLVED (`:729`) | over-wide: OWNER (only op 32 in only block 31 → duplicate first), INVENTORY (`:729` precedes `:741`) |
| 18 | ENTRY_INVALID (`:754`), REACHABILITY (`:812`) | exact — UNREACHABLE_VALUE removal correct (a flipped reachable block fails at `:809` before any terminator) |
| 19 | REACHABILITY, VALUE_UNRESOLVED (`:498/:515`), RETURN_TYPE, RESULT_INDEX (`:523`) | over-wide: TARGET group + DOMINANCE (T1/T2 entry→Return orphans the target → `:812` first; non-entry Return uses only self-owned/function params) |
| 20 | TARGET_INVALID (`:780`), REACHABILITY, TARGET_ARGUMENTS (`:383/:389`), VALUE_UNRESOLVED, RESULT_INDEX, DOMINANCE (`:507`, T1 block 12 → `Branch(13,[Parameter(14)])`) | over-wide: ENTRY_INVALID, BOOL_REQUIRED, SWITCH_*, TRAP_PAYLOAD, RETURN_TYPE |
| 21 | as 20 + BOOL_REQUIRED (`:352`) | over-wide: ENTRY_INVALID, SWITCH_*, TRAP_PAYLOAD, RETURN_TYPE |
| 22 | REACHABILITY, VALUE_UNRESOLVED, RESULT_INDEX | over-wide: TRAP_PAYLOAD (payload types are Unit/Bool: `contains_type_parameter` false, `traits().persistable` true at `lib.rs:604`), DOMINANCE (non-entry Trap payloads are self-owned or Function-role) |
| 23 | DUPLICATE, UNRESOLVED (`:729`) | exact |
| 24 | OWNER (`:731`) | exact |
| 26 | VALUE_UNRESOLVED, RESULT_INDEX, USE_BEFORE_DEFINITION (`:526`, ordinal 0 ≥ 0) | exact |
| 27 | TYPE_PARAMETER_OUT_OF_SCOPE (`:737`) | exact |
| 28/29/30 | DUPLICATE (`:646`) | exact |
| 32 | TARGET_INVALID (`:780`), REACHABILITY, VALUE_UNRESOLVED, RESULT_INDEX, DOMINANCE, SWITCH_TYPE (`:479`) | over-wide: SWITCH_CASES, SWITCH_PAYLOAD, TARGET_ARGUMENTS — selector is always Unit/Bool, so `expected_switch_cases` fails at `:479` before any case is examined |

UNREACHABLE_VALUE is unreachable under any single arm (needs a use block that is unreachable *and* declared so — no template has one, and no single arm both creates and uses it), so its removal from the six sets is correct. No under-wide set found; over-wide entries are all engine-emittable (sound) → VS-R4-003 (P4).

**(b) Multi branch closed by construction.** `fuzz_one` (`ssmc_graph_cfg_checker.rs:32-83`): the only early returns are `len == 0` (`:24`) and `len > 4096` (`:33`), both before any validation. `applied.len() == mutation_count` (one push per iteration, `:41-43`, regardless of no-op arms). `assert_eq!(first, second)` at `:48` is unconditional. Then `mutation_count == 0` → base-template assert; else `Err` → `applied.len() == 1` → `expected_for` (`:65-73`); else → `CODE_UNIVERSE.contains` (`:74-81`). No `Err` path skips both. `CODE_UNIVERSE` (`:560-603`, 42 entries) equals, in order and character-for-character, the 21 `CfgErrorCode::as_str` literals (`cfg.rs:84-104`) + the 21 `TypeErrorCode::as_str` literals (`lib.rs:77-97`); both enums have exactly 21 variants (`cfg.rs:34-77`, `lib.rs:27-70`), both `as_str` matches are exhaustive. `TYPE_CODES` (`:529-551`) equals the 21 type literals. `failure_class` (`:500-505`) is exhaustive over `CfgValidationError`. Hence for the current engine the membership assert can never fail — it only bites on a new enum variant. Ground of `check_type = check_type_inner + check_map_keys` (`lib.rs:302-305`) verified: `small_type` (`:836-853`) yields only Unit/Bool/SInt(8)/UInt(64)/Option(Bool)/Result{Unit,BuiltinFailure}/TypeParameter/AdapterHandle; `check_width` (`lib.rs:903-909`) accepts 8/64, `AdapterHandle`/`BuiltinFailure` are `Ok` at `lib.rs:529-532`, `check_map_keys` falls to `_ => Ok(())` (`lib.rs:591`).

**(c) All five (plus the sixth) inputs pass the new oracle** (static; execution blocked):
1. `01 02 12 01 16 00 00 00 00 00 02` → T1, count 2; arm 18 idx 1 → block 13 `ExplicitlyUnreachable`; arm 22 idx 0 → block 12 `Trap(Unreachable, Some(Parameter(known_ids[2]=14)))`. Engine: `reachable=[true,false]`, `:809-810` pass, terminator 12 → `:507` **CFG_DOMINANCE**. `applied=[18,22]` → multi branch → ∈ universe (`:577`) → pass.
2. `00 02 09 00 01 00 00 00 00 01 00 01` → T0; arm 9 renames param 2→`id(0)`; arm 1 pushes `known_ids[1]=id(0)` → `function.parameters=[2,0]`; BTreeMap visits `id(0)` first: ordinal 0 ≠ 1 → `:681` **GRAPH_ORDINAL_MISMATCH** → `[9,1]` multi → pass.
3. `01 02 09 01 01 00 00 00 00 10 01 00 02` → T1; arm 9 renames 14→`id(0)`; arm 16 pushes `id(0)` onto block 13 → `:701` **GRAPH_ORDINAL_MISMATCH** → `[9,16]` multi → pass. (Not seeded — VS-R4-005.)
4. `03 02 17 00 01 00 00 00 11 11 00 00 02` → T3; arm 23 renames op 32→`id(17)`; arm 17 pushes `id(17)` onto block 31 → `:734` **GRAPH_ORDINAL_MISMATCH** → `[23,17]` multi → pass.
5. `00 03 1d 00 0e 01 01 00 00 00 07 05 00 03` → T0; arm 29 clones block 3; arm 14 idx 1 renames clone→`id(7)`; arm 5 pushes `known_ids[3]=id(7)` → `function.blocks=[3,7]`; inventory passes, `reachable=[true,false]`, block 7 `Required` → `:812` **CFG_REACHABILITY** → `[29,14,5]` multi → pass.
6. `01 03 01 00 02 0a 01 00 00 0b 01` → T1; arm 1 pushes 14; arm 10 idx 1 owner→`id(10)`; arm 11 idx 1 role→Function → 14 visited second, ordinal 0 ≠ 1 → `:681` **GRAPH_ORDINAL_MISMATCH** → `[1,10,11]` multi → pass.
Pass signature: exit 0. The only possible panic text would be `mutation classes [...] escaped with unregistered failure class <code>`, which requires a code outside the 42-entry universe — none of these codes is.

**(d) Seeds match and execute.** `graph-cfg-corpus/seed-0008-77f782cbfd082786` … `seed-0012-522be65e27c409c0` od-dump exactly to inputs (1), (2), (4), (5), (6) above; filename suffix = sha256[:16] of the bytes (matches `write_corpus` `:412-413`). Runner blocks `:379-383` carry the same five byte lists; 4 + 3 + 1 + 5 + 4×(66+32) = **405** unique seeds (`:339-405`), 405 `seed-*` files on disk (644 total). libFuzzer is invoked with the corpus directory (`:311`) and the proof tail shows `634 files found … #635 INITED … #949 DONE` — every corpus file, including the five seeds (mtime 03:44:50.38, before the run), was executed.

**(e) Checker pins are present but weak** — see VS-R4-004. Substantive parts: `:78-80` requires every `Self::X => "LITERAL"` in `cfg.rs` (exactly 21 matches) to appear quoted-with-comma in the target; `:82-85` requires every quoted literal inside `expected_for` to appear in the `CODE_UNIVERSE` array body; `:88-94` requires the seven regression-seed prefixes; `:69-70` requires `applied.len() == 1` and the multi-branch message.

**(f) Proof binds df295d8 clean.** `machine-summary.json:2711-2761` `last_local_proof`: `source_commit` `df295d86…`, `source_tree_clean_except_retained_slices: true`, `worktree_dirty_files` = four `??` entries only (two retained `.forge` slices, two transcripts later committed in `5966d83`); corpus 405/385; 949/949; coverage 6485/3919/3879 + 3929/6020/5942; seeds 1615212316/1615103066; `new_crash_artifacts: []`; `crash-47e105f9…` retested `still_crashes: false`; `regression_seeds` names `seed-0008..0012`. `evidence.json` mirrors every field. Timeline: target edited 03:44:02 → binary 03:44:25 → commit df295d8 03:44:45 → proof run 03:44:50 (cargo 0.036 s = cached, i.e. source unchanged since build; tree clean vs HEAD) → summary 03:45:00 → commit 5966d83 03:45:05. So the proven binary is the committed content. VS-R3-003 closed. (Override toolchain `clang`/`clang/22` is recorded as such, unchanged from prior rounds.)

**(g) Verdict fields honest.** `machine-summary.json:2707` = `REVISE_0_P0_2_P1_1_P2_1_P3_2_P4` (round-3 string, unchanged until an independent PASS); `:2762` "obligation stays REVISE until independent PASS". Register rows (`finding-register.json:3164-3178`, `:4714-4723`) carry the same string, state `PENDING`. Dossier `open_reviews` 97→96 stems from the pack lane PASS, not this slice. Audit fourth-round section (`:218-247`) correctly records the arm-22 correction, the six-set removal, seeds 400→405 and option (ii) — with the inaccuracies below.

## FINDINGS

- **[P3] VS-R4-001 [record] — audit carries stale counts.** `docs/audits/S20_700_SEMANTIC_CHECKERS_PERSISTENT_SLICE.md:23-24`: "385 type-checker seeds and 400 graph/CFG seeds" — on disk 405 (runner `:339-405`, `machine-summary.json:2696`, 405 files). `:246`: "Fresh PASS proof 945/945" — the bound proof at HEAD is 949/949 (`machine-summary.json:2725-2728`); the records-only commit refreshed the summary but not the audit. Fix: one-line edits in the next records-only commit.

- **[P3] VS-R4-002 [record] — "VS-R3-004 refuted" is false at the reviewed lane.** `docs/audits/…:229-230` ("the register-staleness claim (VS-R3-004) was refuted on disk — the row already carried the round-2 string") and the `56ef0c6` commit message. On disk: `git show 67ad3a3:evidence/review/finding-register.json` and `git show 40feff2:…` both give `"disposition": "REVISE_0_P0_1_P1_2_P2_2_P3_1_P4"` (round-1), while `machine-summary.json` at those commits gives `"REVISE_0_P0_1_P1_1_P2_2_P3_3_P4"` (round-2). The finding was correct; it was **closed by regeneration** in `56ef0c6` (row now `REVISE_0_P0_2_P1_1_P2_1_P3_2_P4`, equal to `:2707`), not refuted. Fix: amend the audit sentence to "closed by regeneration in 56ef0c6".

- **[P4] VS-R4-003 [implementation] — over-wide-but-sound single entries remain; "single branch is exact again" (audit `:236-237`) is overstated.** `ssmc_graph_cfg_checker.rs`: arms 3/13 full `TYPE_CODES` (`:639, :660-668`; exact sets are `{TYPE_PARAMETER_OUT_OF_SCOPE, CFG_RETURN_TYPE}` and `{TYPE_PARAMETER_OUT_OF_SCOPE, CFG_BOOL_REQUIRED, CFG_RETURN_TYPE, CFG_TARGET_ARGUMENTS}`); arm 16 `GRAPH_INVENTORY_MISMATCH` (`:681`); arm 17 `GRAPH_INVENTORY_MISMATCH`, `GRAPH_OWNER_MISMATCH` (`:681, :683`); arm 19 the whole `TARGET` group plus `CFG_DOMINANCE` (`:700-710`); arms 20/21 `CFG_ENTRY_INVALID`, `CFG_SWITCH_*`, `CFG_TRAP_PAYLOAD`, `CFG_RETURN_TYPE`; arm 22 `CFG_TRAP_PAYLOAD`, `CFG_DOMINANCE` (`:721, :725`) — the round-4 restoration reason at `:714-718` ("whose payload owner was declared unreachable by another arm") is a two-arm scenario, which the single-only scope at `:608-609` excludes, so the restored entry is dead in the branch that consults it (the `[18,22]` input is handled by the universe branch); arm 32 `CFG_SWITCH_CASES`, `CFG_SWITCH_PAYLOAD`, `CFG_TARGET_ARGUMENTS` (`:754-758`; selector always Unit/Bool → `cfg.rs:479`). All are engine-emittable, so none is a P0; each reduces single-mutation precision only.

- **[P4] VS-R4-004 [record] — checker pins do not discriminate set content.** `scripts/check_semantic_checkers_persistent_fuzz_slice.py:62-65` pins `"CFG_RESULT_INDEX",`, `"CFG_DOMINANCE",`, `"CFG_UNREACHABLE_VALUE",`, `"TYPE_PARAMETER_OUT_OF_SCOPE",` — all four are satisfied by the `TYPE_CODES`/`CODE_UNIVERSE` lines (`:532, :575, :577, :579, :584`), so reverting any per-arm set entry (e.g. arm-22 `CFG_DOMINANCE`) passes. `:78-80` matches `f'"{literal}",'` anywhere in the file, not inside the universe block (removing `CFG_DOMINANCE` from the universe passes because arm sets still contain it), and scans `cfg.rs` only — the 21 `TypeErrorCode` literals in `lib.rs` are not pinned against `TYPE_CODES`/`CODE_UNIVERSE`. Seed pins `:88-94` are 3-byte prefixes. Every bypass I could construct fails *loud* (a missing code makes the harness panic → gate FAIL) rather than silently accepting, hence P4. Fix: scope the universe check to the `CODE_UNIVERSE` array body, add the `lib.rs` `Self::X => "TYPE_…"` scan, and pin the `expected_for` body hash or full seed byte lists.

- **[P4] VS-R4-005 [record] — the sixth transcript input is not seeded.** `01 02 09 01 01 00 00 00 00 10 01 00 02` (`[9,16]` variant of VS-R3-001.2) has no block in `run_semantic_checkers_persistent_fuzz.py:379-383` and no `seed-*` file. It passes by construction (trace (c).3), so this is a completeness note only.

**No P0/P1/P2:** no set is under-wide; the universe is exactly the engine's 42 emittable strings; no `Err` path escapes both asserts; the five inputs are pinned and executed; the proof binds the fix commit with a clean tracked tree; every verdict field states REVISE/PENDING honestly. Rounds 1–3 closures (UNREACHABLE_VALUE derivation, seed-0007, arm-1 OWNER, arms 6/31 empty, VS-R2-006/007, VS-R3-003/004) were re-checked and none is reopened by the rescope.

## SUMMARY

The rescope does exactly what option (ii) asked: single mutations keep exact-or-wider sound sets, everything else asserts determinism plus membership in a universe that is verified character-for-character equal to the engine's two exhaustive `as_str` matches, the five falsifying inputs are on disk, in the runner, and in the executed corpus, and the proof is bound to `df295d8` with a clean tracked tree. What remains is documentary: two stale numbers and one wrong "refuted" sentence in the audit, non-discriminating checker pins, and a set of over-wide single entries (including the arm-22 restoration, which the new scope makes moot). None affects the gate's soundness.

```
FIELD: vulcan_review
ROUND: final
VERDICT: PASS
SEVERITY_LINE: 0_P0_0_P1_0_P2_2_P3_3_P4
DISPOSITION: PASS_0_P0_0_P1_0_P2_2_P3_3_P4
SCOPE: round-4 lane at df295d8 (HEAD 5966d83, lane diff records-only): fuzz/targets/ssmc_graph_cfg_checker.rs, crates/sley-check/src/cfg.rs, crates/sley-check/src/lib.rs, crates/sley-ssmc/src/lib.rs, scripts/run_semantic_checkers_persistent_fuzz.py, scripts/check_semantic_checkers_persistent_fuzz_slice.py, docs/audits/S20_700_SEMANTIC_CHECKERS_PERSISTENT_SLICE.md, machineresearch/sley-2.0/machine-summary.json, evidence/runtime/s20-700-semantic-checkers-libfuzzer/evidence.json (+ graph-cfg-corpus/seed-0007..0012, graph-cfg-artifacts, minimized-graph-cfg, target/release mtimes), evidence/review/finding-register.json (HEAD, 67ad3a3, 40feff2, 56ef0c6), evidence/release/decision-dossier.json, evidence/review/verdicts/s20_700_semantic_checkers_persistent_fuzz_slice/vulcan_review-67ad3a3.md
```

**Evidence artifacts:** none written in-repo (read-only mandate). This transcript is suitable for filing verbatim as `evidence/review/verdicts/s20_700_semantic_checkers_persistent_fuzz_slice/vulcan_review-df295d8.md`. Handoff to the fix owner (merlin): the two P3 audit edits and the P4 checker-pin tightening can ride the same records-only commit that flips `machine-summary.json:2707` and the register row to `PASS_0_P0_0_P1_0_P2_2_P3_3_P4`; a retest of inputs (c).1–6 against the built target with exit 0 confirms the static traces, a non-zero exit refutes this review.
