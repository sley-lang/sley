# RW-050 slice 2 — E8 adversarial lane, BOOTSTRAP_PROFILE_1 freeze (2026-09-06)

> STATUS: IN REVIEW. Implementation, freeze, and validation complete;
> independent semantic/boundary reviews pending (§8). No landing until
> all review lanes are green.

Package: RW-050 complete bootstrap-support semantic/VM profile (depends
on RW-030 COMPLETE `c6e4269`, RW-040 COMPLETE `f625350`, slice 1 landed
`b59ecd7` with G-10 CLOSED under owner amendment A1). This slice
completes RW-050 through: (1) the required E8/pure-primitive
fuzz/adversarial lane, (2) exact `BOOTSTRAP_PROFILE_1`
dependency-closure determination, (3) the freeze with its
permitted-import registry, (4) readiness evidence for the RW-050 close
decision. No self-hosted compiler is implemented here; the slice
establishes the exact executable substrate it may subsequently be built
on. Follow-ups from slice 1 §6/§9 consumed: E8 fuzz lane + seed counts;
whole-freeze record with permitted-import entries.

## 1. E8 / pure-primitive adversarial lane

Three lanes, one per enforcement layer; fuzz constructors generate
inputs, production authorities judge them; no second semantic
implementation anywhere.

### 1a. Execution side (repo-native, always runs)

`crates/sley-vm/src/bridge_adversarial.rs` (new, `#[cfg(test)]`): seeded
xorshift lane, 3 seeds × 128 draws = 384 adversarial programs across
entry (B2V1/V2B1/PSH1/unlanded-XXXX) × 12 row-tamper classes × 11
invocation-shape classes × value classes × 6 fuel budgets, judged by
`lower_function`, `judge_function_operations` (acceptance parity
asserted per draw — the §3.1 invariant held continuously), and
`execute_function`. Exact refusal codes from a decision table over the
draw; determinism per draw; exact success values; fuel monotonicity
around the measured charge (exact reruns identically modulo the
observation, which binds limits by design); decode/re-encode identity
chained both directions across fresh executions. Fixed tests: empty
inputs cross as empty; the asymmetric 2^20 boundary (B2V direct,
V2B/PSH via single-execution `VariantSwitch` chains — cap-sized vectors
cannot re-enter as inputs past the 1,000,000-element S20-210 constant
ceiling; V2B-over-cap unreachable by construction, recorded defense in
depth); oversized inputs fail first at input judgment with the S20-210
resource code.

### 1b. Declaration side (repo-native, always runs)

`crates/sley-check/src/effects.rs` `mod pure_declaration_adversarial`
(new): exhaustive single-field mutants of the frozen shapes plus the
effect-set dimension through `validate_effect_program`, each both
unused and used. Unused malformed rows fail exactly like used ones (the
Ariadne A1 repair, re-pinned); form-conforming rows pass statically
while staying unregistered downstream (layering rule); valid effectful
rows pass declaration but serve only declared-effect invocations
(closure comparison fails otherwise — the hidden-host-effect case);
duplicate adapter identities fail at index build. Exact verdicts from
literal expectation tables (effect codes, type codes incl. the
step-2/step-4 `ParameterOutOfScope` parity).

### 1c. Persistent libfuzzer lane (contract-required evidence)

`fuzz/targets/vm_canonical_inputs.rs` extended with the E8 family lane
(selector 8, all three entries varying per draw) plus the
`bridge_sublane` (12 row tampers × 8 shapes × fuel exactness), as
required by `VM_EXTENDED_OPCODE_PROFILE_V1.md` section 5 ("one lane per
landed family"). Seed grammar 769 → 788 (9 families, regression seed
below); `run_vm_persistent_fuzz.py` / checker / machine-summary
updated; smoke PASS at 1024 and 4096 runs over 788 corpus files, zero
crashes.

Lane finding (repaired, regression preserved): at breadth the lane
crashed on minimized input `20 45 6b` — tamper class 11
(response-rewrite) was a no-op for V2B1 (response already `Bytes`), so
a frozen row flowed through as "tampered" and the expected refusal
fired on an accepted program. Production was correct; the lane oracle
was wrong. Repaired entry-aware (replacement always differs) with a
structural guard asserting every tampered row differs from frozen; the
minimized input is now a permanent corpus seed. A second repair in the
same round: the shared lane hardcoded entry PSH1 (`bridge_fixture(8)`
folded the lane selector into the entry index); entries now vary per
draw.

## 2. Closure determination

Derived from the REWEAVE §10.2 responsibilities (master spec) and
landed semantics, per-item required/not-required with substitution
analysis (full table in `rw-050-sufficiency.md`). Included: E1 data +
base booleans, E2 checked integers, E4 records/variants/maps, E5 cells
+ `value_hash` + `constant_ref`, E6 calls, E8 bridge (3 rows). Excluded
with rationale: E3 floats (closure is integer/byte/map-indexed;
exclusion removes host float-environment dependence from the bootstrap
determinism claim), E7a assertions (CondBranch+Trap substitutes), E7
tests/effects/capabilities (G-2 full-MG obligation), E5 globals and
function references (covered needs; no dynamic dispatch), generic
specialization, recursion. Terminators all five (Return/Branch/
CondBranch/VariantSwitch/Trap); TrapCode all four. Types: all value
forms except floats/handles/tokens/function-refs/type-parameters;
cells execution-local only. Dependency-closed by construction: every
admitted operation's semantics depends only on admitted operations
(checked per family; the gate enforces the set).

Loop-form decision (revisiting RW-040): Form A (CFG backedge loops)
frozen as the canonical iteration mechanism; recursion excluded (not
independently necessary; only iteration handles arbitrarily deep
structures); multi-function calls admitted with an acyclic-graph rule
enforced by the gate; the 256-frame ceiling stays as VM-enforced
defense in depth. Block parameters are the canonical toolchain state
mechanism (three loop workloads thread state through edges); cells
stay admitted with RW-040's `cond-drain-loop` as their evidence.

## 3. Freeze

- Machine record `conformance/bootstrap-profile/v1/profile.json`,
  digest `4f2691504b5c756eae1f5ef01e6e998cc4cd628d4b524b038b10d583bfefd630`
  (raw-byte SHA-256; supersedes `e1c524…` after the self-review
  coverage-rule addition and `0a1c20…` after the Ariadne-delta
  cells-wording fix — each supersession disclosed to reviewers; frozen
  transcripts keep the digests they verified); contract doc `docs/spec/BOOTSTRAP_PROFILE_1.md`
  carries the same digest; manifest stage P binds
  `{value, digest, provenance}`; `scripts/check_bootstrap_profile_1.py`
  verifies doc↔record↔manifest agreement plus vectors, gate, lanes,
  summary, wiring, and review transcripts on every `make quick`.
- Permitted-import registry: exactly the three reviewed rows
  (B2V1/V2B1/P-PUSH-family with monomorphization rule; landed
  instantiations u8-representative and Unit-trail); unknown identities
  denied, no extensible registry. One resolution authority
  (`resolve_bridge_entry`, now `pub(crate)` with `BridgeKind`
  crate-visible) serves lowering, execution, surcharge, and gate
  admission.
- Production gate `crates/sley-vm/src/bootstrap.rs`:
  `judge_bootstrap_profile` (membership only — opcode table of 42,
  type forms, acyclicity, empty effects/contracts/type-parameters,
  registry; reference integrity and semantics stay with their owners;
  refusals reuse `LowerErrorCode`). Static admission, not an execution
  path: `EXTENDED_V1` unchanged. Admission covers exactly the reached
  closure (self-review addition: inventory functions no call reaches
  cannot ride an admission — callers narrow first, as lowering does;
  the report lists functions pre-order, entry first). Gate tests: 10
  (families, exact table, type forms, pure-closed/acyclic, mutual
  recursion, named definitions, cell elements, bridge row+result,
  unreached inventory, registry).
- The gate lane caught one real production bug pre-freeze: the
  call-closure walk checked `visited` before `visiting`, so
  self-recursion returned Ok. Repaired with three-color marking;
  self- and mutual-recursion tests pin it.
- RW-070 boundary preserved: the freeze binds current import
  identities/schemas for closure evidence only and freezes no external
  host ABI (stated in profile record, doc, and gate docs).

## 4. Closure proof

20 vectors in `conformance/bootstrap-profile/v1/accepted.json`
(SHA-256 `878f3009…`, integrity file binds the profile record too),
every one gate-admitted pre-emission (asserted in the emitter),
executing under the frozen reference budgets (100k instructions, 10M
fuel; measured maxima 34 / 124): byte round-trip, push-loop growth (5,
0), symbol table (hit/miss), set-as-map, record/variant walk,
multi-function pass (value/error), bounds-checked traversal
(hit/miss), content-hash chain, exhaustive two-level switch (4 —
closes the RW-040 `VariantSwitch` limitation with accepted vectors),
iterative graph worklist (chain 4/cycle 2 — record-on-arrival
semantics: the sink pushes before exiting), image emission (3/0).
Generator `scripts/generate_bootstrap_profile_fixtures.py` with
`--check` in `make quick`. Trap termination pinned in-crate
(`InternalInvariant`, tag 4, no payload); the emitted set stays
success-valued like the vm-extended family.

Negative evidence: gate tests refuse every excluded opcode family,
excluded type form, effectful/contracted/generic function, recursive
cycle (self + mutual), unknown callee, generic call, unregistered or
tampered row; the adversarial lanes refuse tampered rows/shapes/
budgets with exact codes; restricted profile refuses 161; profile
confusion (wrong identity/version/epoch) fails closed by construction
(cache-key and observation preimages bind profile, versions, epoch,
root, and limits — changing the registry changes the bound identity).

## 5. Sufficiency

`rw-050-sufficiency.md`: every §10.2 item mapped to profile mechanics
with workload pointers; all eleven questions answered YES with
evidence; no missing capability found; nothing added for convenience.
No compiler code written.

## 6. Validation (final, post-review)

- `cargo test --workspace --locked`: 1097 passed, 0 failed, 14
  ignored (2 emitters by design + 12 pre-existing).
- `cargo clippy --no-deps --workspace --all-targets --locked -- -D
  warnings`: clean. `cargo fmt --all --check`: clean.
- `make conformance`: exit 0 (includes the new freeze checker and the
  bootstrap-profile conformance registration).
- `make quick`: green through the freeze gate (generator `--check`,
  both checkers PASS) and the staged checker (R2 READY); halts at the
  release-candidate packaging check, whose release-test batch needs a
  built release candidate (absent in REWEAVE scope — `working_tree_clean`
  gated, 15 SBOM candidate-evidence errors). The rest of quick was
  verified check-by-check: all green except the carried
  reproducibility-report staleness (unchanged class, release-lane
  owned). Separately observed (not a quick gate): the
  `sley2-runner-smoke` trial handshake fails in its harness setup
  (`SLEY2_TRIAL_HANDSHAKE_FAILED`, no trial executed) — trial paths
  untouched by this slice; re-verify post-landing on a clean tree.
  Staged checker: PASS, R2 readiness READY, zero open blockers,
  S/C0–C3 correctly later-stage-unbound.
- Supply chain: regen via owned builder dead last; standalone T52
  PASS, T54 PASS (zero findings); global DEFERRED on the unrelated
  root-license state, as before.
- Persistent fuzz: 4096-run PASS over 788 corpus (plus the 1024-run
  smoke), evidence in
  `evidence/runtime/s20-700-vm-input-libfuzzer/evidence.json`.

## 7. Staged inputs updated (fact updates, history preserved)

- `rw-040-bootstrap-exercises-slice-2.json` exercise 1: PARTIAL →
  PASS-exists with the bridge + closure evidence (original open-part
  text retained as closed-history).
- `rw-040-m2-gap-list-slice-2.json` G-10: OPEN BOOTSTRAP_BLOCKER →
  CLOSED with `closed_how` (the file's own G-9 precedent); the sole
  bootstrap blocker is gone.
- `bootstrap-manifest.json` stage P: unbound → bound to the frozen
  profile digest with provenance.

## 8. Reviews (transcripts in `reviews/`)

- Ariadne (semantic/closure): FAIL round 1 (binding type-closure
  hole) → repaired → delta re-review PASS. Round logs:
  `reviews/reweave-rw050b2-ariadne-2026-09-06.log`,
  `reviews/reweave-rw050b2-ariadne-r2-2026-09-06.log`.
- Nabu (architecture/boundary): PASS with one recommended in-slice
  fix (C0 opcode-set cross-check) and three notes (C1 comment, C2
  single resolution, C3 constants sweep) — all four addressed in-tree
  before landing, plus a constants negative test. Round log:
  `reviews/reweave-rw050b2-nabu-2026-09-06.log`.
- Reviewers are not the slice owner; failed rounds preserved with
  focused repairs. Default-lane quota exhausted mid-slice; the delta
  and Nabu rounds ran over the claude-cli lane (recorded in the
  transcripts' dispositions).

### Round 1 repairs (Ariadne FAIL → repaired, delta re-review pending)

Ariadne FAIL with one blocking finding, accepted on the evidence: the
gate's type-closure was unsound — `Named` types checked arguments only
(a record hiding `F64` crossed boundaries), `LocalCell` ignored its
element type, the `AdapterInvoke` branch `continue`d past result-type
membership, and the push relationship pin admits non-bootstrap
instantiations (e.g. over `F64`) that lowering accepts under
`EXTENDED_V1`. Repairs in `bootstrap.rs`: the input carries
`types: &TypeEnvironment`; definition inspection (fields/payloads,
unknown fails closed, cycle-safe coinductive accept); cell element
checks; `admit_bridge_use` with row-schema membership (non-bootstrap
rows are not permitted imports) falling through to the shared result
check; three new tests (named/tainted/clean/unknown, cells, bridge
row+result). Other axes reviewed sound (exact default-deny matching,
shared resolver, representation-only arms, no `AdapterCall`
manufacture, sufficiency mapping, RW-070 unfrozen).

### In-flight integration repairs (owned scope)

- New fixture family registration: `build_independent_conformance_report.py`
  `COVERAGE`/`DEPTH` gained `bootstrap-profile` (checker command,
  `codec_and_identity`); the checker joined the `make conformance`
  recipe. This fixed release-batch `ORACLE_DRIFT` errors the new family
  introduced (22-test batch back to OK).
- Oracle-independence split: the coverage-mapped freeze checker must
  not read implementation sources, so code-marker pins moved to
  `scripts/check_bootstrap_gate_markers.py` (not coverage-mapped, same
  split as the vm-extended checker beside its oracle command).

## 9. Disposition — RW-050 status: COMPLETE (2026-09-06)

Close criteria (§7 of the task), all met: G-10 closed under A1 ✓;
fuzz lane passes (repo-native lanes + 4096-run persistent PASS) ✓;
profile frozen and dependency-closed ✓; imports exact and default-deny
✓; no native semantic judgment ✓; composed workloads pass (20
gate-admitted vectors) ✓; negatives fail closed ✓; staged checker
accepts (R2 READY) ✓; independent reviews pass (Ariadne FAIL→PASS,
Nabu PASS) ✓; no new bootstrap blocker found (two lane findings and
two review findings, all repaired with regressions pinned).

BOOTSTRAP_READY determination (actual gate, not the staged R2 proxy):
the staged checker reports R2 READY (frozen profile, zero open
blockers), but BOOTSTRAP_READY remains FALSE — RW-050 completion alone
does not imply it. Remaining prerequisites outside this package:
S/C0–C3 unbound (no Sley toolchain program exists; no images), RW-060
lifecycle demo, RW-070 ABI/image freeze, RW-080+ recovery closure, and
the release gate (`decision_state` BLOCKED on review lanes, succession
trials, single-host attestation, root license). Next: the next
dependency-ready package under the REWEAVE DAG (RW-060/RW-070 per
their contracts).
