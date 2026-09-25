# Vulcan qualifying re-review — s20_700_vm_persistent_fuzz_slice (repair fd3293f, records follow-up a554a14)

Everything needed is on disk. Static trace complete; no repo files written, no binaries executed.

# Vulcan qualifying re-review — s20_700_vm_persistent_fuzz_slice (repair fd3293f, records follow-up a554a14)

**Scope confirmation.** `git show --stat fd3293f`: 8 files, all under `docs/`, `fuzz/`, `machineresearch/`, `scripts/` — **no `crates/` change**. `git diff --stat fd3293f..HEAD`: `machineresearch/sley-2.0/machine-summary.json` only. `git status`: dirty = the two retained `.forge/slices/rw-080-*` files only. Runner `scripts/run_vm_persistent_fuzz.py` is untouched by both commits, so the proof at fd3293f is under the current (7k) runner — the prior P3 proof-provenance item is closed by construction.

## (a) Success-termination oracle — both lanes

**Outer lane** `fuzz/targets/vm_canonical_inputs.rs:141-158`: `let ExecutionTermination::Success(value) = &outcome.termination else { panic!(...) }` then `assert!(sley_mutate::encode_const_value(value).is_ok(), "a succeeded VM fixture value is not result-canonical")`. **Family lane** `:384-401` identical shape. Both replace the prior `is_ok()`.

Soundness (no canonical draw can leave Success under the asserted profile):
- Path: `execute_function` `execute.rs:494-502` = `lower_function` → `validate_inputs` → `execute_core`. `execute_core` `:1153-1177` writes parameter registers (fault → `observed_invariant` → `InternalInvariant`, now caught); `:1179-1190` maps `RuntimeFault` → `InternalInvariant` (now caught).
- Outer profile 0 (`:1693-1699`: 100 instr / 100 fuel / 10 000 units / 10 000 output). Identity fixtures: 0 instructions + 1 terminator fuel (`charge_action` `:2007-2028`, `dispatch_terminator` `:1841`); Boolean fixtures: 1 instruction + 1 terminator = 2 fuel. Return `:1845-1855`: `&value.value_type != result_type` → `InternalInvariant` — identity returns the validated parameter (type equality enforced at `:1791`), Boolean returns `Bool`; output units of a 32-byte `Bytes`/`Text` = 1+1+33 = 35 (`value_units_const` `:2285`, `:2349-2354`) ≪ 10 000. Initial units `:2279-2283` = input units + register count + bytecode length; the designated seeds `[fixture,0,family,1,0,0,0]` (runner `:293`) execute every fixture at profile 0 under this oracle and the 1576-run proof is clean, so the bytecode term fits with ≥9 900 units of headroom.
- Family lane `generous_limits()` `:1681-1689` (1 000 / 100 000 / 10 M / 10 M). Every family's failure path is a Success value: E2 `arithmetic_value` `extended.rs:1660-1690` wraps `Checked::Failure` as `Result::Err` data; `IntDivChecked` `:1568-1595` returns `ARITHMETIC_DIVIDE_BY_ZERO` / overflow for `MIN / -1` as values; E4 `MapNew` `:1836-1866` duplicate → `Result::Err(DuplicateKey)` value; E7a `contract_assert_value` `execute.rs:1604-1623`; E8 capacity → `capacity()` `extended.rs:510-535` value. Bridge fuel surcharge `execute.rs:1649-1662` = elements × `BRIDGE_ELEMENT_FUEL`=1 (`extended.rs:150`) ≤ 32 for a 32-byte request. E6 depth 3 < `MAX_CALL_DEPTH` 256 (`:1405`). No Trap terminator exists in any fixture. Cancel is `None` in both profiles.
- Encode predicate is the right canonicality predicate: it is literally the engine's own `require_canonical_form` (`execute.rs:1808-1811` — `sley_mutate::encode_const_value(value).is_err()` → `InputNotCanonical`), applied to outputs. E3 results are canonicalised by `canonical_f64` `extended.rs:1399-1406,1453`; E4 results are sorted by canonical key bytes `sorted_map` `extended.rs:1699-1706` (same `encode_const_value` the codec uses). So Success payloads encode; a regression in either would now fire.

No reachable non-Success path found for a canonical draw under the asserted profiles. **P1 closed.**

## (b) Raw-lane must-reject vs `validate_inputs` precedence

`:161-189`. Count branch asserts `first.as_ref().err() == Some(&Exec(InputCountMismatch))`; type branch asserts `first.is_err()`.
- `validate_inputs` `execute.rs:1721-1722`: `if request.inputs.len() != input.function.parameters.len() { return Err(Exec(InputCountMismatch)) }` — unconditional and first. `fixture.expected_input_types.len()` equals `function.parameters.len()` for all 9 outer fixtures (`identity_fixture` `:1405/1423`; `boolean_fixture` `:1448/1475`). `lower_function` precedes it (`:498`) but is request-independent and Ok for the fixed fixtures (proved by the profile-0 canonical seeds). No false positive.
- Type branch: per input, `check_input_shape` `:1784-1795` runs `types.check_constant` (`:1789`, may yield `Type(...)`), `require_hashable`, then `&value.value_type != expected` → `Exec(InputTypeMismatch)` `:1791-1792`; `require_canonical_form` `:1738` only after. Any zip-mismatch therefore yields *some* `Err` — `is_err()` is exactly right, never an `Ok`. No false positive.

## (c) Two-layer float refusal — exact

Predicates, all bit-identical to harness `canonical_f64_bits` (`:1597-1606`: NaN→0x7ff8…, ±0→0, else raw):
- Type layer: `sley-check/src/lib.rs:1077-1085` `check_f64`: `bits == 0x8000_0000_0000_0000` → `FloatNonCanonical`; `is_nan && bits != CANONICAL_F64_NAN` → `FloatNonCanonical` (`"TYPE_FLOAT_NON_CANONICAL"` `:93`). Reached via `check_constant` `:402-408` → `check_constant_as` `:750` from `execute.rs:1789`.
- Codec layer: `sley-scb1/src/lib.rs:867-875` `validate_f64_bits` same two conditions → `ScbErrorCode::FloatNonCanonical` → `encode_const_value` Err → `execute.rs:1810` `Exec(InputNotCanonical)` (`"VM_EXEC_INPUT_NOT_CANONICAL"` `:332`).
- No third layer: fingerprint `sley-ssmc/src/fingerprint.rs:563` `ConstData::F64Bits(value) => encoder.u64(*value)` — raw bits, no refusal. `enforce_input_count`/`add_input_units` trivially pass for two floats. Non-Success `Ok` impossible under generous limits (a). So the probe's four arms `:481-513` are exhaustive and correct; success-on-non-canonical is impossible (type layer fires at `:1789` before any execution).

## (d) Records

- Audit `docs/audits/S20_700_VM_INPUT_PERSISTENT_SLICE.md:24` "788 seeds" ✓ (runner enumeration unchanged); `:30-36` loaded-image re-scope + E1–E8 lane text ✓; `:97-132` repair-round text matches code, and both cited strings `TYPE_FLOAT_NON_CANONICAL` / `VM_EXEC_INPUT_NOT_CANONICAL` exist verbatim (`lib.rs:93`, `execute.rs:332`). `:83` "default 1576 over 788" ✓.
- GAPS `25-evidence-gaps.md:28-32` re-scoped; both checker markers `"does not cover the loaded-image path"` / `"RW-070 owner obligation"` present; `load_image` exists at `host_abi.rs:234`, `execute_loaded_image` at `execute.rs:529`, neither appears in the target (checker forbids at `:82-90`).
- Checker `:212-232` validates PASS / floor / `new_crash_artifacts` / `owner_lib_sancov` / 40-char commit — see P4-2.
- Regressions: VM_002 (`20 45 6b`) ✓ replayed as a tracked seed (runner `:306`); VM_001 — see P4-3.
- ADR-0039 `:5-8` E8 line ✓. `restricted_profile_opcodes` at `machine-summary.json:2913` ✓ (`supported_opcodes` gone).

## (e) b55c33e9 episode

Artifact present, `sha1 = b55c33e9…` (71 bytes `00 00 02 00 01 01 03 ff×64`), minimized `ff ff 02` (`artifacts/minimized-3b82…`, `minimized/`). Hand trace of `ff ff 02`: fixture 3, `family_gate` 0xff %3=0 → lane on, selector 2 → E3, raw lane with 0 inputs; family draw yields canonical NaN + normal → `FloatAdd` → canonical NaN → Success; sub-draw raws contain `ffff02ff…` = negative NaN ≠ 0x7ff8… → `check_f64` fires → `Type(FloatNonCanonical)`. The first sub-draw pinned `Exec(InputNotCanonical)` only → `"wrong error"` panic. Harness-oracle error confirmed; production correct; retest `returncode 0, still_crashes false` (`evidence.json:112-118`). Filing text consistent.

## (f) Proof binding

`evidence.json:125` `source_commit fd3293f…`, `:134-138` dirty = two retained `.forge` slices, `:111` PASS, `:73` 1576 = floor, `:86` `new_crash_artifacts []`, `:87` `owner_lib_sancov 125`, `:105` `libsley_vm…rlib: 125`, `:119` `cargo-json`, `:133` no unexpected warnings. Register block `machine-summary.json:2871-2912` transcribes these faithfully (except P4-2). The build step reports `Finished … in 0.02s` (`:29-31`) — cached; cargo's fingerprint over the fuzz crate and path deps binds that binary to the on-disk sources, which at run time equalled fd3293f (tree clean). Stated as inference, not observed.

## (g) Verdict-field honesty

`machine-summary.json:2856` `"vulcan_review": "REVISE_0_P0_0_P1_1_P2_4_P3_4_P4"` = prior verdict, `:2918` "obligation stays REVISE until independent PASS" — honest for the pre-filing state. Checker `:179` now accepts any `PASS|REVISE|FAIL` prefix, so filing this verdict will not break `make quick`.

## FINDINGS

[P4] [checker-pin-specificity] scripts/check_vm_persistent_fuzz_slice.py:31-32,54-56 `"did not succeed", "is not result-canonical"` (listed twice) + `"failed to execute"` — the old per-lane pins (`"a valid fixed VM fixture under normal limits was rejected"`, `"a family fixture under its own canonical inputs failed to execute"`) were replaced by substrings common to both lanes, so reverting *one* lane to `is_ok()` would still pass the checker; the duplicate entries are dead. Pin the full messages `vm_canonical_inputs.rs:146,152,389,395` per lane.

[P4] [proof-record-binding] scripts/check_vm_persistent_fuzz_slice.py:231-232 `len(proof.get("source_commit", "")) != 40` + machineresearch/sley-2.0/machine-summary.json:2871-2912 — the prior P3 asked for source-commit ancestry against the slice files; the checker validates shape only, so a proof at an older commit than the last target/engine change still PASSes. Separately, the a554a14 transcription dropped `toolchain_versions` (`evidence.json:129-132`) and never carried `toolchain_overridden: true` (`:128`) into the register block, though the runner marker `toolchain_versions` is pinned at checker `:138`. Audit `:129` is honest ("source-commit shape"). Residual, not a reopen.

[P4] [regression-replay-uniformity] fuzz/regressions/S20_700_VM_001.json `"input_hex": "ffff02", "minimized_input_hex": "ffff02"` + scripts/run_vm_persistent_fuzz.py:296-306 — the original 71-byte crash input (see (e) above for the od-verified bytes; sha1 b55c33e9) is recorded only in the ignored `evidence/runtime/.../artifacts/` dir, and unlike VM_002 (`bytes([0x20, 0x45, 0x6B])` at `:306`) `ff ff 02` is not a tracked corpus seed; durable replay depends on an untracked artifact. Audit `:165-166` says "filed regressions replay as corpus seeds". Add the seed and record the original hex.

**[Filer note — not reviewer text: the reviewer's original P4-3 parenthetical naming the 71 bytes is omitted here because it could not be re-verified verbatim; the verified bytes are `00 00 02 00 01 01 03` followed by 64 `ff` bytes (od of the retained artifact), matching the reviewer's section (e), and are recorded as `original_crash_hex` in `fuzz/regressions/S20_700_VM_001.json`.]**

[P4] [e3-codec-arm-unreachable] fuzz/targets/vm_canonical_inputs.rs:482-487 `Err(sley_vm::ExecutionError::Exec(sley_vm::ExecutionErrorCode::InputNotCanonical)) => assert!(non_canonical, ...)` — through `execute_function` this arm is dead for F64: `check_constant` (`execute.rs:1789` → `check_f64` `sley-check/lib.rs:1077-1085`) always precedes `require_canonical_form` (`:1738`) with an identical predicate, so `validate_f64_bits` (`scb1/lib.rs:867`) remains unreached at the VM boundary for floats (the map-order lane reaches it for maps). Oracle is correct and the target comment `:474-478` says so; the audit `:115-118` should state plainly that the codec float layer is unreachable here rather than "exactly one of the two". Informational.

[P4] [e7a-binding-cosmetic] fuzz/targets/vm_canonical_inputs.rs:873-876 `source: sley_ssmc::ContractSource::Parameter(id(base + 1))` — `operation_fixture` assigns `id(base+1)` to the *block* (`:1277`) and `id(base+3)` to the parameter (`:1280`), so the binding row names a block. Harmless: the VM reads predicate operands from the instruction (`prepare_frame` `execute.rs:1570-1584`) and never consults `bindings` (no `check_contract`/`contracts::` use in `lower.rs`/`execute.rs`/`extended.rs`), and the audit `:88` records that the VM ignores `ContractSource`. Pre-existing, not introduced by the repair; fix the id for fixture hygiene.

**Assumptions:** read-only; no cargo/python/fuzz execution (memory: static trace only); the profile-0 value-unit bound relies on the designated seeds having Success-terminated in the clean proof for the bytecode-length term; build freshness at the proof inferred from cargo fingerprinting plus a clean tracked tree; corpus now 852 files vs 829 at run time (libFuzzer additions, not a concern). Suggested filing path for nabu/argus: `evidence/review/verdicts/s20_700_vm_persistent_fuzz_slice/vulcan_review-fd3293f.md`; update `machine-summary.json:2856` to the disposition below (checker `:179` tolerates it).

```
FIELD vulcan_review
ROUND final
VERDICT PASS
SEVERITY_LINE 0_P0_0_P1_0_P2_0_P3_5_P4
SCOPE repaired lane at fd3293f116b6cc174e382b392bcf4b6c9ad3d31c (records-only follow-up a554a14 to HEAD), fuzz/targets/vm_canonical_inputs.rs + scripts/check_vm_persistent_fuzz_slice.py + slice records, engine unchanged
Disposition = PASS_0_P0_0_P1_0_P2_0_P3_5_P4
```
