<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2b0f1c9f4940c020565137891a49a3769cd02061; role: vulcan; field: vulcan_surface_review_revision_11; dispatched: 2026-09-23T12:15:46Z; duration_s: 454; process_exit_code: 0 -->
# Vulcan Council review — json_bridge

Harness: claude-code
Reviewed checkpoint: 2b0f1c9f4940c020565137891a49a3769cd02061

This is an independent Vulcan surface review of `docs/spec/SMP1_JSON_BRIDGE_V1.md` revision 11. I did not open the untracked Ariadne json_bridge verdict file in the worktree.

What I ran and read:
- **Scope.** `git rev-parse HEAD` returned `2b0f1c9f4940c020565137891a49a3769cd02061`, which matches the scope. Branch is `work/succ-context-impl`.
- **Git reads:**
  - `git diff ab42a3a9..HEAD -- docs/spec/SMP1_JSON_BRIDGE_V1.md`: +8/−4. It came from two commits. `a8b4cddb` added an unrevisioned r13 re-pin paragraph. `8357c243` replaced it with the revision-11 Status/history sentence and the SMP1 r14 pin at :44.
  - `git diff ab42a3a9..HEAD -- docs/spec/SMP1.md`: the r13/r14 delta.
  - `git diff ab42a3a9..HEAD -- crates/sley-protocol/src/server.rs`: the 201 body refusal and field-9 gate.
  - `git diff --quiet ab42a3a9 HEAD -- crates/sley-protocol/src/lib.rs crates/sley-json-bridge crates/sley-scb1 crates/sley-id`: no change. The codec and the bridge crate are unchanged.
  - `git diff --stat` over the bridge conformance data, oracle, fixture generator, fuzz-slice checker, ADR-0034 and closeout: the only change is `scripts/generate_smp1_json_bridge_table.py` (one comment line).
  - `git show 8357c243` for ADR-0033 and WORK_PACKAGES.
  - `git log -L28,28` on the generator.
  - In-process diff of the `json_bridge` machine-summary section against `ab42a3a9`.
- **Checkers and tests:**
  - `python3 scripts/check_smp1_json_bridge_contract.py`: exit 0, `"result": "PASS"`, `"revision": 11`, `"status": "S20_420_IMPLEMENTED_REVIEW_PENDING"`, `"problems": []`.
  - `scripts/check_smp1_json_bridge_vector.py`, run through `uv run --offline --frozen --project oracle/scb1` for blake3: exit 0, PASS (methods 41, vectors 5, rejections 36).
  - `scripts/check_smp1_vector.py`, same way: exit 0, PASS (frames 3, mutations 4).
  - `python3 scripts/generate_smp1_json_bridge_table.py --check --protocol-version 1`, `2` and `3`: PASS each.
  - `python3 scripts/test_smp1_json_bridge_table.py`: 17 cases, PASS.
  - `python3 scripts/check_smp1_json_bridge_persistent_fuzz_slice.py`: PASS.
  - In-process SHA256SUMS check of `conformance/smp1-json-bridge/{v1,v2,v3}`: all 5 files OK.
  - `cargo test --locked --offline -p sley-json-bridge --lib`: 11 passed, 0 failed, 1 ignored (the fixture-refresh emitter). Doctests: 0.
  - `cargo test --locked --offline -p sley-protocol --lib workspace_open`: 4 passed (v1 never carries field 9 and refuses a body; v2 discloses the snapshot; v3 answers the v2 `open_summary`; the probe does not wait).
  - In-memory negative probes of the bridge checker, patching `chk.read`:

    | Probe | Result |
    |---|---|
    | baseline | PASS |
    | SMP1 pin at :44 reverted to 12 | FAIL `smp1-pin-text` |
    | Status line reverted to r10 | FAIL `spec-revision` |
    | ADR-0034 status rewritten to "revision 3" | PASS |
    | :44 reverted to 12, r14 pin string appended as a history line | PASS |

- **Files read:**
  - `docs/spec/SMP1_JSON_BRIDGE_V1.md:1-312`
  - `crates/sley-json-bridge/src/lib.rs:1-320, 523-537, 641-667, 690-1197`
  - `crates/sley-json-bridge/src/tests.rs` (test list; ceiling tests :598-634)
  - `scripts/check_smp1_json_bridge_contract.py:1-326`
  - `scripts/generate_smp1_json_bridge_table.py:15-134`
  - `docs/adr/ADR-0034-json-bridge-boundary.md:1-62`
  - `docs/audits/S20_420_JSON_BRIDGE_CLOSEOUT.md:3`
  - `docs/WORK_PACKAGES.md:48` (S20-420 row)
  - `crates/sley-protocol/src/lib.rs:1300-1369, 1754-1794`
  - `crates/sley-protocol/src/server_tests.rs:612-640`
  - `docs/spec/NATIVE_TEST_ADMISSION_V1.md:350-575`
  - `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:270-290, 560-580, 725-745`

## Evidence checked

- **Bridge bytes did not change.** SMP1 r14's non-empty-201 refusal is in the server only: `server.rs` `workspace_open` returns `PROTOCOL_PAYLOAD_INVALID` before loading the head, under every version (`server_tests.rs:638` covers v1). The codec's `validate_for_version` (`sley-protocol/src/lib.rs:1328-1364`) has no per-method body rule, and `lib.rs` is unchanged in range.
  - `frame_from_value_with` (`bridge lib.rs:870-905`) and `frame_to_json`/`frame_to_json_with` (:914-959) treat `body` as opaque hex for every method.
  - So a 201 frame with a non-empty body still bridges byte-for-byte, and the server judges it.
  - This matches bridge §3 ("invalid on the wire" means codec-invalid) and the authority rule of no semantic validation. The r11 claim "no bridge clause, encoding, or behavior changes" holds.
- **`open_summary` does not reach any bridge-owned record.** The new optional-by-omission convention `[n: T]` appears only at `SMP1.md:719` (`open_summary` field 9). The bridge renders that body only as opaque hex.
  - The bounded context ("one item either way") is copied, never derived (bridge §4).
  - None of the bridge-owned records (Frame, Hello, SelectedProfile, LimitProfile, BoundedContext, Failure, StreamChunk) uses the new convention.
- **Method tables are unchanged.** The row 201 cells changed from "repository path digest | accepted head" to "none | accepted head summary (appendix A)". The owner column stays `S20-390`.
  - The generator derives `reserved` from those cells (`generate_smp1_json_bridge_table.py:106-111`). Neither new cell is `reserved`, so `workspace.open` stays `reserved: false`.
  - Drift checks for v1, v2 and v3 pass, SHA256SUMS match, and the v1 table is 41 rows byte-for-byte.
  - No round-trip or rejection vector involves `workspace.open` (checked in-process), so no vector depends on 201 body semantics.
- **Re-pin consistency:**
  - Spec Status line r11 at :3; composition pin r14 at :44; history sentence at :31-35.
  - Checker `SPEC_REVISION = 11`, `SMP1_REVISION = 14`.
  - Machine summary: exactly five keys changed (`contract_revision` 11; `current_delta_review` r11, all PENDING; `implementation_complete` false; `status` `S20_420_IMPLEMENTED_REVIEW_PENDING`; `status_note`). The r10 PASS round is kept in `current_delta_review_note`.
  - The WORK_PACKAGES row names revision 11 and the pending review. `SMP1.md:21-23` composes bridge r11.
  - NATIVE r6 appendix D re-pins SMP1 r14.
  - Not updated: ADR-0034 and the generator comment (see findings).
- **Fail-closed behavior.** Version-selected bridge entrypoints hand the version gate to the codec: an expected version outside 1/2/3 is `PROTOCOL_VERSION_UNSUPPORTED` after the bridge's own field checks, which is the §5 precedence. The versioned hello renderers are render-only: `hello_from_json` reads only the v1 four-key features object and v1 method names (`lib.rs:1161-1181`, `object(..., fields, &[])` at :654), so v2/v3 hello JSON cannot be read back and is refused, never mis-parsed.
- **Resource ceilings** are unchanged: `MAX_JSON_TEXT_BYTES` / `DEPTH` / `ELEMENTS` are anchored crate↔contract↔oracle by the checker, and the tests are at `tests.rs:598-634`.
- **Non-counted observation (pre-existing, outside the delta):** `SMP1_JSON_BRIDGE_V1.md:133` reads `}Failure {`, a missing line break between `SelectedProfile` and `Failure` inside the §2 record block. It has been there since `539bb9da` (2026-09-04).

## Findings

[P3] [repin-completeness] docs/adr/ADR-0034-json-bridge-boundary.md:3-18 - The ADR status still says "the S20-420 contract is a draft at revision 10" and has no revision-11 record. In the same re-pin commit `8357c243`, ADR-0033 got a "Current pin (2026-09-23): … revision 5 (re-pin to SMP1 revision 14 …)" record; ADR-0034 was not touched. The bridge checker's `ADR_MARKERS` do not check revisions (probe: ADR rewritten to "revision 3" still PASSes), so the gate cannot see this. The re-pin also missed `scripts/generate_smp1_json_bridge_table.py:28`, whose comment still says "SMP1.md stays revision 13": `a8b4cddb` bumped it 12→13 and `8357c243` did not bump it to 14. - Closure evidence: an ADR-0034 revision-11 record (re-pin to SMP1 r14, no clause change, delta review pending) and the generator comment set to 14 (or made revision-neutral). Better still, a checker assertion tying the ADR status revision to `SPEC_REVISION`, with a revert test.

[P3] [undeclared-surface] docs/spec/SMP1_JSON_BRIDGE_V1.md:148-158,297-311 - SMP1 r14, which r11 now composes, names protocol version 3 itself and admits it on the explicit frame entrypoints. The bridge's `*_for_version` exports wrap those entrypoints and accept version 3 (`crates/sley-json-bridge/src/lib.rs:279-300, 742-755, 808-819`, `METHOD_TABLE_V3_JSON` at :63-72). Yet the contract's §2 names only the v1 and v2 tables, and §10's list of crate exports omits `METHOD_TABLE_V3_JSON`, `hello_to_json_for_version`, `frame_value_for_version` and `frame_from_value_for_version`. The bridge's own checker requires two of those (`check_smp1_json_bridge_contract.py:109-110`), so the contract and its checker disagree about what the crate declares. `hello_to_json_for_version(_, 3)` also emits a fifth `features` key, `native_tests` (`lib.rs:1010-1026, 1142-1146`). That object shape conflicts with §1 ("exactly the fields of its record in this contract", :82-84) and §2's four-key Hello (:125). The key is named by no contract: NATIVE :359 names the bit `native_tests_v1`, and no spec under docs/spec names the JSON key. `hello_from_json` refuses it with `SHAPE_INVALID`, so the surface fails closed, but it is undeclared. It landed in `0c655fc1` on 2026-09-16, after the bridge's last reviewed revision, and r11 is the first bridge revision since. - Closure evidence: a bridge revision, or an r11 amendment, that declares selection 3. Either list the v3 exports, the `native_tests` features key and the render-only (no read-back) rule for versioned hellos, or state that NATIVE appendix C/D owns the v3 JSON shapes and have NATIVE name the key. Then align the §10 enumeration with the checker's `CRATE_MARKERS`.

[P4] [checker-anchoring] scripts/check_smp1_json_bridge_contract.py:308-309 - The SMP1 pin check is an unanchored substring test for "`docs/spec/SMP1.md` (revision 14)" anywhere in the spec. Probe: revert the normative composition sentence at :44 to revision 12, append the r14 pin string as a history line, and the checker PASSes. The spec is correct today, so the gap is latent. The same commit anchored the CLI checker's composition sentence but not the bridge's. - Closure evidence: anchor the check to the composition sentence "It composes, and never alters, `docs/spec/SMP1.md` (revision N)" and add a revert test.

[P4] [precision] docs/spec/SMP1_JSON_BRIDGE_V1.md:31-34 - The r11 justification "the method table has no body columns, so no bridge clause … changes" is true of `methods.json`, but the generator derives `reserved` from SMP1's request/response cells (`generate_smp1_json_bridge_table.py:106-111`). The accurate reason is that row 201's new cells are not `reserved`, and the v1/v2/v3 drift checks confirm this. The same sentence also widens SMP1's "every later selection whose method table includes version 2's row 201" to "every later selection". Line 32 is also an unwrapped ~330-character line. - Closure evidence: at the next bridge revision, reword the sentence to cite the drift gate (row 201's cells are not `reserved`), quote SMP1's selection condition exactly, and rewrap the line.

## Assessment

SMP1 revision 14 makes nothing false in what this package states about `workspace.open`, the method table, or the bytes it produces and consumes.
- **201 changes live in the server only.** The empty-body refusal and the version ≥ 2 `open_summary` with field 9 are both server-side (`server.rs`). The codec (`sley-protocol/src/lib.rs`) and the bridge crate are byte-for-byte unchanged in range, and the bridge treats every body, including 201's, as opaque hex.
- **Generated tables are unchanged.** The row 201 cell edits leave all three tables byte-identical (drift checks PASS, SHA256SUMS OK).
- **Vectors, oracle, fuzz slice and crate tests still pass.**

The re-pin is correct in the contract, the checker, the machine summary (status moved off COMPLETE; the current delta is bound to r11 with every lane PENDING), WORK_PACKAGES, and the SMP1 composition sentence.

The two P3s are documentation and completeness gaps, not behavior defects:
1. ADR-0034 and one generator comment were left at older revisions.
2. The bridge contract still does not declare its version-3 surface, even though its composed authority now names version 3.

Both fail closed and touch no canonical bytes. Neither blocks my lane accepting revision 11, but both should be closed at the next bridge revision.

VERDICT: PASS_0_P0_0_P1_0_P2_2_P3_2_P4
SECTION: json_bridge
FIELD: vulcan_surface_review_revision_11
SCOPE_SHA: 2b0f1c9f4940c020565137891a49a3769cd02061
FINDINGS: [P3] [repin-completeness] docs/adr/ADR-0034-json-bridge-boundary.md:3-18 - ADR status still "draft at revision 10" with no revision-11 record (ADR-0033 got its current-pin record in 8357c243); ADR_MARKERS revision-agnostic (probe: "revision 3" still PASS); generator comment scripts/generate_smp1_json_bridge_table.py:28 still "SMP1.md stays revision 13" - ADR-0034 r11 record, generator comment at 14 or revision-neutral, ideally a checker ADR-revision anchor with revert test | [P3] [undeclared-surface] docs/spec/SMP1_JSON_BRIDGE_V1.md:148-158,297-311 - SMP1 r14 names version 3 but the bridge contract declares no v3 surface: §10 omits METHOD_TABLE_V3_JSON/hello_to_json_for_version/frame_value_for_version/frame_from_value_for_version (two required by the checker at :109-110), and hello_to_json_for_version(_,3) emits an undeclared fifth features key `native_tests` (lib.rs:1010-1026,1142-1146) against §1/§2 exact shapes; hello_from_json refuses it (fail-closed) - declare selection 3 (exports, key, render-only rule) or delegate the v3 JSON shapes to NATIVE with the key named, and align §10 with CRATE_MARKERS | [P4] [checker-anchoring] scripts/check_smp1_json_bridge_contract.py:308-309 - SMP1 pin is an unanchored substring (probe: :44 reverted to 12 plus the r14 pin string as a history line still PASSes) - anchor to the composition sentence with a revert test | [P4] [precision] docs/spec/SMP1_JSON_BRIDGE_V1.md:31-34 - "the method table has no body columns" skips that the generator derives `reserved` from SMP1 body cells (generator :106-111); "every later selection" widens SMP1's row-201 condition; line 32 unwrapped - reword to cite the drift gate and quote SMP1's condition at the next revision
SUMMARY: SMP1 revision 14's workspace.open changes (the empty-body refusal and version ≥2 open_summary with field 9) live only in the server; the codec and bridge crate are byte-for-byte unchanged and carry 201 bodies as opaque hex. The v1/v2/v3 method tables are unchanged (drift checks, SHA256SUMS), and the contract checker, vector oracles, table tests, fuzz slice and crate tests all pass. The re-pin is consistent in the contract, checker, machine summary, WORK_PACKAGES and SMP1 composition. ADR-0034 and a generator comment were left stale, and the bridge contract still does not declare its version-3 surface; both are P3 and fail closed.
