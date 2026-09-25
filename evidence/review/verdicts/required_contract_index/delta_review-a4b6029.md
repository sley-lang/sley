# Required contract index — current-delta review (index r2, delta f1684a9)

Scope SHA: `a4b60294e4dd75133fa08f92bf16022fb05bb807` (verified: `git rev-parse HEAD`
returns exactly this SHA; read-only review otherwise).
Delta under review: commit `f1684a936309deaa7832cc897b5496767efb0224` ("index r2 with
v2 method metadata"), which moved `docs/spec/REQUIRED_CONTRACT_INDEX_V1.md` Status
revision 1 → 2 and extended row 17.12 with `ENTITY_READ_PROFILE_V2.md` plus
`conformance/smp1-json-bridge/v2`.
Summary state at scope: `machineresearch/sley-2.0/machine-summary.json`
`required_contract_index` has `contract_revision: 2`, status
`S20_770_CONTRACT_DRAFT_REVIEW_PENDING`, and
`current_delta_review: {contract_revision: 2, ariadne: PENDING, nabu: PENDING,
vulcan: PENDING}`. No prior council verdicts exist at this revision (no
`evidence/review/verdicts/required_contract_index/` directory existed before this
file; no verdict commits found).

## Sources read (full)

- `docs/spec/REQUIRED_CONTRACT_INDEX_V1.md` (88 lines, Status revision 2)
- `scripts/check_required_contract_index.py` (203 lines, incl. lines 127–141
  derived-vs-registry subset check)
- `git show f1684a9 --stat` and the diff of the index, ADR-0047, and machine-summary
- `machineresearch/sley-2.0/machine-summary.json` `required_contract_index` section

## Checker result (run, not trusted blindly)

`python3 scripts/check_required_contract_index.py` → `result: PASS`, `problems: []`,
`exit 0`, with `required_contracts: 12`, `documents_named: 20`, `domains_named: 14`,
`checkers_named: 18`, `derived_identifier_domains: 50`.

## Independent re-derivation (all lanes)

- Table parse: 12 `| 17.x` rows; required names unchanged r1→r2 (diff touches only
  the row-17.12 document and corpus cells plus Status/history text).
- Every backticked `*.md` in document cells exists under `docs/spec` (20/20),
  including the delta-added `ENTITY_READ_PROFILE_V2.md`.
- Every backticked digest domain is frozen backticked in `IDENTIFIERS_V1.md`
  (14/14); domain cell of row 17.12 is byte-identical r1→r2.
- Every backticked `*.py` checker exists under `scripts/` and is referenced as
  `scripts/<checker>` in the `Makefile` (18/18), including `check_smp1_contract.py`.
- Every `conformance/...` corpus is an existing directory and every `crates/...`
  entry is an existing directory, including delta-added
  `conformance/smp1-json-bridge/v2` (contains `methods.json` + `SHA256SUMS`;
  `sha256sum -c SHA256SUMS` run inside the directory → `methods.json: OK`).
- Derived-vs-registry re-derived independently: 50 `sley2.*` string literals in
  `crates/**/*.rs` (excluding `target/`), 0 unregistered in `IDENTIFIERS_V1.md` —
  matches checker. Note on lines 127–141: the comment claims registry and derived
  "must be the same set" but the code enforces only derived ⊆ registry
  (unregistered-domain failures); stale registry entries would not fail. No delta
  impact (0 drift either way); checker-fidelity note only, not a delta finding.
- v2 metadata: `conformance/smp1-json-bridge/v2/methods.json` has
  `method_count: 43, reserved_count: 4`, tag set = v1 set ∪ `{306, 307}`
  (v1 file: 41/4; diff exactly `{306, 307}`), matching `SMP1.md` r12
  ("41 rows total, 37 dispatched" frozen v1; "43 rows total, 39 dispatched" union)
  and summary `protocol.version_2` (43 methods, reserved 305/503/601/602).
- No-13th-identity claim is grounded in the defining document, not just the index:
  `ENTITY_READ_PROFILE_V2.md` §2 lines 42–46 ("uses the existing frame envelope,
  tag 400, frame epoch, field schema and digest domain… No additional persistent
  query identity or object format is introduced").
- SMP1 §§2–3 headings exist; §4 v1/v2 table sections and Appendix D reference
  `ENTITY_READ_PROFILE_V2.md` §§3–5 for tags 306/307.
- `check_smp1_contract.py` (PASS, rev 12) pins v1 tags, v2 additions `{306,307}`,
  union, appendix-D tags, and `entity.version`/`entity.signature` markers;
  `check_smp1_json_bridge_contract.py` (PASS, rev 8) pins both methods files
  including v2 counts/union/additions. `make quick` runs the index checker, both
  checkers, and `generate_smp1_json_bridge_table.py --protocol-version 2 --check`.
- Revision binding: index Status line revision 2 across one `^Status:.*revision (\d+)`
  hit; summary `contract_revision: 2`; `current_delta_review.contract_revision: 2`
  with shape `{contract_revision, ariadne, nabu, vulcan}` and triple PENDING under
  DRAFT status — exactly the pre-review staging the checker accepts.
- Codes/WORK_PACKAGES/ADR: 77000 `CONTRACT_INDEX_UNSATISFIED` / 77001
  `CONTRACT_INDEX_DRIFT` present in `ERROR_CODES_V1.md`; WORK_PACKAGES S20-770 row
  cites the index with the revision-2 record; ADR-0047 carries the revision-2 record.

## Non-blocking notes (all lanes, explicitly sub-report-grade, no P-grade assigned)

- Row 17.12 lists only `check_smp1_contract.py`, while the delta-added v2
  `methods.json` is directly validated by `check_smp1_json_bridge_contract.py` and
  the v2 generator `--check`. The index rules require "at least one" checker per
  name, so no rule is violated, and the artifact is validated in `make quick`
  (both checkers PASS); this under-scoping predates the delta (r1 had the same
  shape for the v1 bridge corpus). Future precision could list the bridge checker.
- The table corpus cell lists the v2 metadata directory flatly alongside full
  corpora, but the Status paragraph and §6 revision history both explicitly call it
  the "additive version 2 metadata directory", and the Extraction commit message
  states independent vectors remain outstanding — thinness is disclosed, not hidden.
- "Accepted entity-read extension" reads as design-accepted (doc header:
  independent reviews passed on `4807037`, integrator acceptance under the active
  development directive, scoped authority with consumer-sync still required), and the
  index simultaneously disclaims acceptance/freeze/completeness (§3) — no contract
  overstatement found.

## Lane verdicts

### Ariadne (contract)
Traceability holds for the delta: twelve versioned names, document-wins rule intact,
no contract defined, revision binding correct, all named artifacts exist and the
listed checkers run. No report-grade findings on the delta. Verdict: PASS (0 findings).

### Nabu (architecture)
Additive-coverage claim holds: same `sley-protocol-handshake-v1` name, digest
domains unchanged, no thirteenth identity — grounded in ENTITY_READ §2 and
re-derived registry/tag evidence; v1 freeze preserved with explicit v2 union;
staging state correct. No report-grade findings on the delta. Verdict: PASS
(0 findings).

### Vulcan (surface)
All named surfaces exist and pin: documents, corpus/metadata dirs with working
SHA256SUMS, Makefile-run checkers plus v2 generator check, all re-run PASS; v2
metadata-only thinness is disclosed in-index. No report-grade findings on the
delta. Verdict: PASS (0 findings).

## Machine-readable footers

VERDICT: PASS / SECTION: required_contract_index / FIELD: current_delta_review.ariadne / SCOPE_SHA: a4b60294e4dd75133fa08f92bf16022fb05bb807 / FINDINGS: none
VERDICT: PASS / SECTION: required_contract_index / FIELD: current_delta_review.nabu / SCOPE_SHA: a4b60294e4dd75133fa08f92bf16022fb05bb807 / FINDINGS: none
VERDICT: PASS / SECTION: required_contract_index / FIELD: current_delta_review.vulcan / SCOPE_SHA: a4b60294e4dd75133fa08f92bf16022fb05bb807 / FINDINGS: none
