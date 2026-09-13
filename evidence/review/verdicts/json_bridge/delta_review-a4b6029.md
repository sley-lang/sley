# json_bridge current-delta review — bridge r8 (delta f1684a9) at scope a4b6029

Scope: `git rev-parse HEAD` = `a4b60294e4dd75133fa08f92bf16022fb05bb807` (verified).
Delta: `f1684a936309deaa7832cc897b5496767efb0224` (phase-2 static: bridge r8 with v2 method metadata).
Summary section `json_bridge.current_delta_review` = `{contract_revision: 8, ariadne: PENDING, nabu: PENDING, vulcan: PENDING}` (read directly from `machineresearch/sley-2.0/machine-summary.json`).
No prior council verdicts at this revision (no `evidence/review/verdicts/json_bridge/` existed before this file).

## Inputs read in full

- `docs/spec/SMP1_JSON_BRIDGE_V1.md` (246 lines, Status rev 8, SMP1 pin rev 12).
- `scripts/check_smp1_json_bridge_contract.py` (243 lines, SPEC_REVISION 8, SMP1_REVISION 12).
- `git show f1684a9 --stat` + diff for `docs/spec/SMP1_JSON_BRIDGE_V1.md`, `docs/adr/ADR-0034-json-bridge-boundary.md`, `conformance/smp1-json-bridge/v2/methods.json`, `scripts/check_smp1_json_bridge_contract.py`, `scripts/generate_smp1_json_bridge_table.py`.
- `scripts/generate_smp1_json_bridge_table.py` (full) + `test_smp1_json_bridge_table.py` run.
- `conformance/smp1-json-bridge/v1/methods.json`, `conformance/smp1-json-bridge/v2/methods.json`, `docs/spec/SMP1.md` v1/v2 table sections, `docs/spec/ERROR_CODES_V1.md:435`, `docs/WORK_PACKAGES.md`, `docs/adr/ADR-0034-json-bridge-boundary.md`.
- `git diff f1684a9..HEAD` for bridge files (spec unchanged since delta; checker gained only the `check_current_delta_review` gate; crate gained additive phase-3 `for_version` exports — outside this delta, defaults frozen).

## Tool results (executed, not trusted blindly)

- `python3 scripts/check_smp1_json_bridge_contract.py` → `{"result": "PASS", "revision": 8, "problems": [], "status": "S20_420_IMPLEMENTED_REVIEW_PENDING"}` exit 0.
- `python3 scripts/generate_smp1_json_bridge_table.py --check` → v1 PASS; with `--protocol-version 2` → v2 PASS.
- `python3 scripts/test_smp1_json_bridge_table.py` → 9/9 PASS.

## Independently re-derived claims

- v1 table: `method_count` 41, `reserved_count` 4, 37 dispatched; tags exactly the frozen inventory (100-104, 200-214, 300-305, 400-404, 500-504, 600-604); reserved tags [305, 503, 601, 602].
- v2 table: `method_count` 43, `reserved_count` 4, 39 dispatched; `v2_tags == sorted(set(v1_tags) | {306,307})`; additions exactly `[{306 entity.version S20-310}, {307 entity.signature S20-310}]`, non-reserved, sorted, matching `docs/spec/SMP1.md:352-365` (43 rows / 39 dispatched / same four reserved / S20-310 owned).
- v1 frozen byte-for-byte across the delta: `git show f1684a9^:.../v1/methods.json | sha256` == working-tree sha (`cf7cb9fb…1c139cf`); `git diff f1684a9^..f1684a9 -- v1/methods.json` empty.
- Generator explicit selection: `build(protocol_version=1)` default v1; `--protocol-version {1,2}` required for v2; out-of-section rows, duplicate sections, wrong v2 tags, missing rows all fail-closed (test script proves).
- Pins: `docs/spec/SMP1.md:3` status rev 12; bridge spec names `` `docs/spec/SMP1.md` (revision 12) `` (`SMP1_JSON_BRIDGE_V1.md:29`) and re-pin history (§rev 8 text, lines 16-19); error range `42000 through 42004` at `ERROR_CODES_V1.md:435` with all five `| <n> | <symbol> |` rows in spec §5; ADR-0034 rev-8 record + WORK_PACKAGES rev-8 markers present.
- Section 10 pending declaration matches the delta: at `f1684a9` the crate had zero `versioned` symbols (`git show f1684a9:.../lib.rs | grep -c versioned` = 0); vectors/oracle/fixture scripts carry no v2 references; checker requires no capable symbol. Post-delta HEAD crate adds additive `for_version`/`versioned` exports with v1 defaults unchanged — outside this delta, not a delta defect (recorded as prose note only, no formal finding).

## Per-lane analysis

- Ariadne (contract): rev-8 text, SMP1-rev-12 pin, 41/37 + 43/39 counts, 306/307 ownership, v1-frozen claim, explicit-selection claim, codes, ADR/WP sync all verified against primary files. No contract falsehood in the delta. PASS, 0 findings.
- Nabu (architecture): bounded-section drift gate (v1 region + v2 additions region + `## 5. Bounded context` anchor, exact inventories, no silent absorption), explicit-version selection with frozen default, union validation, checker cross-pins (own rev + SMP1 status line), phase-3 pending staging with no required capable symbol. All verified. PASS, 0 findings.
- Vulcan (surface): declared encodings, object shapes, failure table + declared-field-order precedence, operations, unknown/omission copying, v1-only vectors/oracle/fuzz slice consistent with declared staging; v1 bytes bit-identical so no surface regression in the delta. PASS, 0 findings.

Note (out-of-scope, non-blocking, not a formal finding): at scope HEAD the crate contains additive phase-3 `*_for_version`/`*versioned` exports while spec §10 still says "the crate … stay version 1-only". True at delta `f1684a9`, stale at HEAD; defaults (`frame_to_json`, `frame_from_json`, `METHOD_TABLE_JSON`) remain frozen v1. Needs a spec touch on the next bridge revision, not a delta block.

```
VERDICT: PASS
SECTION: json_bridge
FIELD: current_delta_review.ariadne
SCOPE_SHA: a4b60294e4dd75133fa08f92bf16022fb05bb807
FINDINGS:
```

```
VERDICT: PASS
SECTION: json_bridge
FIELD: current_delta_review.nabu
SCOPE_SHA: a4b60294e4dd75133fa08f92bf16022fb05bb807
FINDINGS:
```

```
VERDICT: PASS
SECTION: json_bridge
FIELD: current_delta_review.vulcan
SCOPE_SHA: a4b60294e4dd75133fa08f92bf16022fb05bb807
FINDINGS:
```
