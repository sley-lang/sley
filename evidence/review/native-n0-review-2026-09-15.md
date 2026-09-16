# Native-test N0 owner-contract review

Date: 2026-09-15. Scope: `NATIVE_TEST_EXECUTION_V1.md`, `NATIVE_TEST_ADMISSION_V1.md`, `NATIVE_TEST_RESERVATIONS_V1.md`, ADR-0050, and additive identifier/ADR-index pointers, compared with the existing VM, value codec, policy context, SCB1, exchange and protocol owners. Read-only static review; no source edits, builds, runtime suites or exploit reproductions.

## Verdict

**REVISE: two implementation-blocking contract ambiguities and one normative wording inconsistency.** The overall ownership, measured-versus-deterministic evidence split, transaction acceptance boundary and compatibility strategy remain sound within this review. Resolve the exact N2 request/validation boundary and deterministic output-count definition before treating the wire contract as frozen. This is N0 contract review only; no native execution, supervision qualification, runtime acceptance, Council or release verdict is supplied.

## Findings

### N0-01 — Required: specify an implementable VM request and TestCase validation boundary

- **File:** `docs/spec/NATIVE_TEST_EXECUTION_V1.md:81`; `:92`.
- **Description:** The request names only existing `LoweringInput` inventories, target/root/epoch, values, limits and profile. Step 3 nevertheless requires the VM to validate the TestCase replay and observation forms. Actual `LoweringInput` (`crates/sley-vm/src/lower.rs:167`) has function/parameter/block/operation/constant/global/contract/adapter inventories but no TestCase or its replay/expected-observation fields. Ordered values and limits cannot establish those omitted forms. The new request also has no exact Rust-level field/type and result/error signature, leaving N2 to invent how this required validation reaches the VM while N1 is explicitly downstream of N2.
- **Required correction:** Freeze the N2 API/type boundary. Either supply the canonical SSMC TestCase data needed for this VM validation and define its bindings, or explicitly place TestCase replay/observation validation in the test owner and give the VM only its independently checkable pure function/contract/effect obligations. Name exact input, limits, observation/outcome and error types and their ownership without a reverse VM dependency on tests/conformance. Preserve the specified old lowering/input error order and describe where any new profile checks enter it.
- **Status:** addressed (author response; independent rereview pending).
- **Response:** Addressed in contract revision 2 §3: exact NativeExecutionInput/Request/Outcome/Error API and ownership, explicit effect/requirement inventories with VM-owned FunctionUnit projection, and TestCase replay/observation validation assigned to sley-tests using canonical checker. Existing lowering/input ordering is preserved; new outer/profile/pure closure checks are located explicitly.

### N0-02 — Required: one deterministic over-budget output count and exact codec owner

- **File:** `docs/spec/NATIVE_TEST_EXECUTION_V1.md:118`; `:122`.
- **Description:** The observation hashes `output_bytes_counted`, but the refusal rule permits the first proven count above budget and says the counter **may** saturate at cap+1. A counting sink may add a whole encoded field/payload length at once, while another may advance byte by byte or saturate. These are different permitted counters for the same value and limit, hence different native observation bytes/IDs. The text also references an unqualified `encode_const_value`; both mutation-value and fingerprint owners have functions with that name. The existing public encoding used by extended VM is `sley_mutate::encode_const_value` (`crates/sley-mutate/src/codec.rs:66`, referenced by `crates/sley-vm/src/extended.rs:1695`), distinct from the private fingerprint traversal.
- **Required correction:** Pin the exact canonical byte representation and owner path. Specify one observable counter rule, such as mandatory cap+1 on proven overflow and exact length on success, with an explicit u64::MAX case. Define the counter on terminations before output sizing and on absent trap payload; define any extra counting work/limit charge and refusal precedence. The bounded sizing helper should be owned by the existing pure value-codec layer and share the encoder's canonical structure, not reproduce its encoding in tests/conformance. Independent golden vectors must then have exactly one expected observation for each limit case.
- **Status:** addressed (author response; independent rereview pending).
- **Response:** Addressed in revision 2 §4: canonical bytes are exactly sley_mutate::encode_const_value; its owner must add a shared bounded sizing path before allocation. Overflow is always cap+1, exact within-cap size on success, zero before sizing/absent payload, and explicit u64::MAX/codec-ceiling behavior. Sizing adds no fuel/instruction/unit charge, reuses hard codec bounds, and has a distinct native OutputEncoding resource tag for codec safety refusal. Unique golden observations are required.

### N0-03 — Suggestion: reconcile the identifier registry's preimage rule

- **File:** `docs/spec/IDENTIFIERS_V1.md:91`; `docs/spec/NATIVE_TEST_RESERVATIONS_V1.md:48`.
- **Description:** The registry says a domain cannot be reused for another preimage, while the proposal intentionally adds disjoint magic/preimage variants under five existing typed identity families. The reservation document explains the intended distinction, but the normative registry still states the broader prohibition without that qualification. This is a documentation inconsistency, not a demonstrated hash collision or an objection to the chosen versioned-family design.
- **Suggestion:** Add a narrow explicit rule permitting reviewed, magic-disjoint versions within the same typed identity family while forbidding reinterpretation of old bytes, cross-purpose reuse and implicit acceptance. Keep live registry rows unchanged until implementation promotion. Cite the new variant ledger/ADR so readers and future drift tooling agree on the rule.
- **Status:** addressed (author response; independent rereview pending).
- **Response:** Addressed in IDENTIFIERS_V1.md: narrowly permits reviewed magic-disjoint variants within the same typed identity family, preserving old byte IDs and decoder behavior; still forbids cross-purpose reuse, aliasing and reinterpretation. Reservation ledger cites that live rule; no live domain table rows were added.

## Verified strengths and boundaries

- New custom envelopes specify magic, canonical version/length, record, hash preimage and stored trailer. Listed record tags and signature field counts are coherent: measurement signs fields 1–20 and stores signature 21; acceptance signs fields 1–18 and stores signature 19. Signing contexts are correctly distinguished from BLAKE3 domains.
- Traced plan → execution report → measurement → approval → transaction → acceptance statement/receipt dependencies. The statement binds a previously computed transaction ID; no circular transaction/signature dependency was found. Empty selection still has explicit report, historical context and acceptance signature requirements.
- Stronger native selection separately preserves the old static selected set and rechecks all six literal limits, selected count and aggregate limits. Memory/time measurements do not masquerade as deterministic traps. Raw imported keys/configuration IDs are not trust grants; receiver-established roles/scopes remain required.
- The eleven-field historical context names the existing `candidate_validation.rs::encode_context_projection` owner, whose actual fields agree with the proposed projection description. A future owner accessor is explicitly required instead of duplicating its encoder.
- The direct bundle record avoids encoding an up-to-48MiB bundle as one SCB1 Bytes field. Each embedded report/config/plan remains subject to the 16MiB Bytes bound. Canonical 65,536-byte receipt chunks also remain below that bound; at most 1,024 chunks reconstruct at most 64MiB. Receipt and exchange total bounds still apply: a maximum-size standalone receipt is not promised to fit inside a 64MiB exchange once transport overhead is added. The chunking does not waive total/allocation limits or raise the old decoder's cap.
- Exchange section-5 binding is additive to the old leaf construction; the count changes from 2 + receipts + branches to 3 + receipts + branches, maximum 8,195. Mixed ancestry uses explicit version dispatch. Old transaction/genesis/receipt/exchange bytes and decoder acceptance are preserved.
- Protocol v3 activation requires negotiated feature 32 and profile-specific methods/frames; old v1/v2 reserved/unknown behavior and entity handle/report meanings remain explicitly unchanged. Paging, attempt status and unknown-outcome reconciliation do not grant semantic or commit authority.

## Read-only reservation checks

A local parser over the reservation ledger confirmed 13 unique new domains, 13 unique eight-byte magics and descriptor tags 23–35. None of those domain/magic strings occurs in current Rust source. All 26 error numbers 29200–29225 are distinct and do not collide with current crate numeric literals. Existing registry descriptor tags end at 22; the actual protocol feature constants are 1, 2, 4, 8, 16, leaving 32 available, and method additions 605–607 follow the existing 604 boundary. Existing-family domains match their registered identity types. These checks establish present reservation consistency, not future implementation or vector acceptance.

## Acceptance limit

The initial contracts correctly remain proposals and the live registry is not expanded with unimplemented domains. N0 acceptance should wait for the two required definitions above and resolution of the small registry wording inconsistency. N2/N1 implementation, independent byte/hash vectors, actual N3 enforcement on both hosts and later native transaction/transport qualification remain separate work.

## Author revision summary

All three findings are addressed in place. The original REVISE verdict is preserved pending independent rereview. No Rust implementation or admitted vectors were changed.


## Revision 2 delta re-review — 2026-09-15

Current verdict: **REVISE for two remaining exact refusal-mapping/precedence definitions.** Revision 2 substantially resolves the original findings. The original review above remains historical evidence; this section states the current scope.

### Original finding disposition

- **N0-01 closed:** execution section 3 now specifies the owned N2 Rust input/request/outcome/error API. TestCase replay/expected-observation validation explicitly belongs to the test owner; VM does not pretend its LoweringInput carries TestCases. `EffectDefinition`, `CapabilityRequirement`, `FunctionUnit` and `validate_effect_program` were checked against their actual existing owner types/signature. The supplied inventories can implement this boundary without a VM dependency on tests/conformance.
- **N0-02 partly closed:** exact `sley_mutate::encode_const_value` ownership, a pure-owner bounded measurement API, Exact/OverLimit result, mandatory cap+1 observable overflow, prior-failure zero, absent-payload zero, u64::MAX handling and no extra VM fuel charge now remove the original chunk-size-dependent counter choice. New native resource tag 7 is an appropriate distinct encoding-safety refusal and does not reuse an old VM tag or expected trap. The overlap precedence below remains to be fixed.
- **N0-03 closed:** `IDENTIFIERS_V1.md:91` now explicitly permits reviewed, magic-disjoint versions inside the same typed family while preserving old IDs and decoder behavior and forbidding cross-purpose reuse. Live registry rows remain unchanged.

### N0-04 — Required: encode the newly introduced effect-validation error unambiguously

- **File:** `docs/spec/NATIVE_TEST_EXECUTION_V1.md:114` and `:277`.
- **Description:** `NativeExecutionError` now includes `Effect(sley_check::effects::EffectValidationError)`, but the report's closed Rejected phase list still has only type1, CFG2, lowering3, fingerprint4, execution5 and native-profile6. No phase/mapping is defined for the true S20-230 Effect case. The existing EffectValidationError itself also carries preserved Type and Cfg variants (`crates/sley-check/src/effects.rs:170`), so implementers must otherwise invent how each nested failure becomes canonical report bytes.
- **Required correction:** Define the complete NativeExecutionError-to-Rejected conversion, preserving nested earlier owner numeric/symbol codes and their phases. Add a distinct native effect phase (for example 7) if needed without renumbering existing phase tags. Explicitly map Profile and all preserved variants too.
- **Status:** addressed (author response; independent rereview pending).
- **Response:** Revision 3 execution contract §5 supplies the exhaustive NativeExecutionError conversion table. Nested Type/Cfg errors preserve phases1/2 and exact owner numeric/symbol accessors; true S20-230 errors use new phase7; all Preserved and Profile variants are explicit. Tags1..6 remain unchanged.

### N0-05 — Required: decide codec-hard-limit versus declared-output overlap

- **File:** `docs/spec/NATIVE_TEST_EXECUTION_V1.md:189` and `:198`.
- **Description:** The bounded measure may return OverLimit or ScbError::ResourceLimit. The contract maps these to different deterministic terminations/counters: OutputBytes/cap+1 versus OutputEncoding/0. It explicitly makes hard limits first at u64::MAX, but does not fix the general precedence when encoded output exceeds both a small declared cap and a codec depth/collection/standalone bound. One implementation can stop once it proves the declared cap exceeded; another can complete bounded codec validation and discover the hard refusal. Both meet the current measurement description but yield different observations. The new tag makes this distinction observable, so mandatory cap+1 alone does not close it.
- **Required correction:** Specify one universal precedence and measurement contract. For example, perform bounded codec validity/hard-limit validation first and return a codec refusal before classifying Exact/OverLimit; or specify an exact traversal/refusal rule that gives declared-cap precedence consistently. Describe the hard-work cutoff and require a shared owner implementation so the rule remains bounded without materializing an oversized buffer. Independent vectors must pin an overlap case as well as u64::MAX.
- **Status:** addressed (author response; independent rereview pending).
- **Response:** Revision 3 §4 universally validates/counts canonical encoding under shared codec hard bounds before classifying Exact/OverLimit, for every cap. Partial declared overflow cannot waive later map/order/depth/collection errors. First hard refusal stops work with the shared owner error; only fully valid oversized encodings produce OutputBytes/cap+1. §10 requires overlap and u64::MAX vectors.

The field grammars, signature counts, direct bundle/chunking strategy and present numeric/domain reservations remain as assessed in the initial review. No native code was implemented or executed during this re-review. Fixing these two definitions is a contract completion task, not evidence of runtime correctness or qualified supervision.

## Author revision 3 summary

N0-04 and N0-05 are addressed in execution sections4/5/10. The independent
REVISE verdict remains historical pending rereview. No accepting Rust path or
admitted vector changed in this contract correction.


## Revision 3 delta re-review — 2026-09-15: current verdict

**N0 contract review accepted: zero remaining findings in the reviewed contract scope.** This current verdict supersedes the earlier REVISE conclusions while preserving their findings and correction history above. It permits proceeding to the planned N2/N1 implementation against these contracts; it does not claim implemented/native runtime acceptance, qualified supervision, new Council verdicts or release readiness.

- **N0-04 closed.** Execution contract section 5 (`:293`) now enumerates every NativeExecutionError leaf and fixes the canonical report phase/code/symbol conversion. Compared directly with current `ExecutionError` (`execute.rs:355`), `LoweringError` (`lower.rs:207`), `CfgValidationError` (`cfg.rs:173`) and `EffectValidationError` (`effects.rs:170`): all current variants are accounted for. Nested type/CFG errors retain phases 1/2; true effect failures use additive phase 7; prior phase tags and owner codes/symbols remain unchanged. The new exact mapping requires vectors for each row.
- **N0-05 and N0-02 closed.** Execution section 4 (`:188`) universally requires bounded complete codec validation/counting before declared-cap classification. Crossing a small declared cap does not skip later canonical/hard-bound checks; a shared owner hard refusal terminates at the owner's fixed first-error point. Only completely valid encodings yield Exact/OverLimit. Thus codec ResourceLimit is OutputEncoding/0, while a valid over-cap encoding is OutputBytes/cap+1; u64::MAX and absent/prior-output cases remain explicit. This gives one deterministic observable result without oversized intermediate allocation or an invented complete length after a hard refusal.
- **N0-01 and N0-03 remain closed.** The exact N2 ownership/API boundary and narrow identifier-family versioning clarification are retained. OutputEncoding tag 7 remains confined to the new native enum; old VM observations/resource tags and runtime acceptance are not silently changed.

Revision 3's required nested-error and overlap vectors are implementation acceptance obligations, not existing evidence. No code, build, runtime test or supervisor operation was performed for this contract re-review.

### Reviewed contract fingerprints

- `docs/spec/NATIVE_TEST_EXECUTION_V1.md`: `bd9820aa8aca4a2421dac68b17df9eed7f4edc5a3e8c1da42d853b00d84e7a6b`
- `docs/spec/NATIVE_TEST_ADMISSION_V1.md`: `b4ab9cd4b00364b4a3ef59bf1cc7f9f421a5acbe057623d4770b7946b23d09b0`
- `docs/spec/NATIVE_TEST_RESERVATIONS_V1.md`: `cd99f878a11e31b0384f98ee7fe1b448fa1ee2f1b8bcad280ed89c66a7c8337f`
- `docs/adr/ADR-0050-native-test-evidence-and-admission-boundary.md`: `b6b0cfc3bcf2e887e60684bbe93e91ce81342a39df04176e44f93986d70f4b82`
- `docs/spec/IDENTIFIERS_V1.md`: `5c598246ea51067db73d782d6f977af47b1fbef05ebaecc092f63db28e83f5d3`


## Focused observation/report-capacity clarification — 2026-09-15

**Accepted; N0 remains at zero open scoped findings.** Execution section 3 step 5 now reserves stored-observation capacity before frame admission using the actual hash count/exact limits, fixed identity widths, longest permitted termination (Trap with present hash) and maximum-width encodings of all five dynamic counters. SCB length prefixes are part of the exact stored grammar, so the conservative bound also includes their growth and the trailer. Equality passes; an insufficient literal observation cap, including zero, preserves pre-execution ResourceLimit/phase 5.

The clarification explicitly separates the outer report: before invoking N2, N1 must reserve the full NativeExecutionReport, using the observation reservation as its Embedded payload and counting all wrapper/Bytes/union/record/envelope/trailer overhead. Exceeding the report hard cap yields 29207 before execution. This closes the boundary identified during this focused review: fitting an observation alone cannot guarantee that its wrapper fits. The final encoder's exact size check remains defensive; it is not a normal post-run capacity refusal or matching expected trap.

This is acceptance of the sizing contract, not verification of its forthcoming N2/N1 implementations. Reviewed execution-spec SHA256: `94f5c39dfec814ba77e3bf0aa0b45944c18243f8ef80067acd75161e97de5b7c`. Prior fingerprints above identify the earlier revision-3 snapshot, and are retained as history.
