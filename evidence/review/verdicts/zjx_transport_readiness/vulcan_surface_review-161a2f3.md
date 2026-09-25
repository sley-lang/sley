# zjx_transport_readiness / vulcan_surface_review / Vulcan (surface and security lane)

Council review of the S20-ZJX-READINESS closeout (`READY_EXISTING`, documentation-and-test-only) at the witness commit. Read-only lane; this transcript is the only file written.

## 1. Scope verification

- `git rev-parse HEAD` -> `161a2f3480dfca3df22019a9df4e3e3e4a38674f` (equals SCOPE_SHA; proceeded).
- Branch `main`; `origin/main` = `4168332f34ba4ff4567fd60df8f2adc42083b1e1`; `git log origin/main..HEAD` = exactly the one closeout commit. HEAD is NOT pushed.
- Working tree: clean except the untracked `evidence/review/verdicts/standards_sbom_and_provenance/nabu_architecture_review-70283ce.md` (concurrent lane, not touched).
- Environment for checkers: `SLEY2_MASTER_GOAL=/home/dev/machineresearch/Sley2.0mastergoal.md`.

## 2. Inputs read in full

- Brief: `/tmp/agent-scratch`
- `docs/audits/S20_ZJX_TRANSPORT_READINESS_CLOSEOUT.md` (283 lines)
- `docs/audits/S20_ZJX_TRANSPORT_READINESS_AMENDMENT.md` (282 lines) and its source `/home/dev/Downloads/Sley-ZJX-Readiness.md`
- `crates/sley-repo/tests/zjx_readiness_witness.rs` (1015 lines)
- `evidence/validation/zjx-transport-readiness-v1.json`; `evidence/validation/zjx-transport-readiness-logs-v1/witness-run.log`; the `zjx_transport_readiness` section of `machineresearch/sley-2.0/machine-summary.json`
- `git show 161a2f3` (full stat; full diffs of `RESUME.md`, `docs/status/HANDOFF-2026-09-15-QUALIFICATION.md`, `evidence/security/T54/secret-scan.json`, `evidence/release/decision-dossier.json`, `evidence/build/lint-report.json`, `evidence/review/finding-register.json`)
- `crates/sley-repo/src/lib.rs` lines 330-545 and 685-720 (exporter, importer, preflight, promotion, `decode_envelope`, `validate_epochs`); `crates/sley-repo/src/exchange.rs` lines 645-680, 1439-1487, 1885-1960 (`decode_envelope`, `preflight_repository_exchange`, `import_repository_exchange`); symbol inventories of both files
- `conformance/repository-pack/v1/rejected.json` and `conformance/repository-exchange/v1/rejected.json` (ids, expected reasons/codes, digests; payloads decoded by the witness)
- `scripts/check_clean_room_boundary.py` (sentinel list, inventory, scope), `scripts/check_supply_chain_audit.py`, `scripts/generate_supply_chain_evidence.py` (candidate manifest and `--check` logic), `scripts/generate_repository_pack_rejections.py` (mutation construction), `scripts/check_domain_tags_and_strings.py` (scope)
- `crates/sley-store/src/lib.rs` `ObjectStore::new` (no filesystem effect), `crates/sley-repo/Cargo.toml`

## 3. Tool results (commands run and exact results)

| Command | Result |
|---|---|
| `git rev-parse HEAD` | `161a2f3480dfca3df22019a9df4e3e3e4a38674f` |
| `cargo test -p sley-repo --locked --test zjx_readiness_witness -- --nocapture` | `test result: ok. 10 passed; 0 failed; 0 ignored` (0.07 s). Printed ZT-03 symbols: magic `SCB_MAGIC_INVALID`, contract-tag `SCB_CONTRACT_UNKNOWN`, truncated `SCB_LENGTH_OVERFLOW`, trailing-byte `SCB_TRAILING_BYTES`, flip-trailer / flip-epoch / flip-payload `PACK_DIGEST_MISMATCH`, resealed-payload-bit `PACK_SCHEMA_UNSUPPORTED`, resealed-late-bit `PACK_DIGEST_TREE_MISMATCH`, resealed-epoch `SCHEMA_EPOCH_MISMATCH`. Identical to `witness-run.log` and to `witness.pack_rejection_symbols_observed` in the evidence record. |
| `python3 scripts/check_error_symbol_registration.py --check` | `PASS`; 495 emitted symbols, 315 numeric codes, `unregistered: []`, `unexercised: []` (exit 0) |
| `python3 scripts/check_declared_limits.py --check` | `PASS`; 156 declared limits, 0 undocumented (exit 0) |
| `python3 scripts/check_domain_tags_and_strings.py` | `PASS`; 50 registered domains, `problems: []` (exit 0) |
| `python3 scripts/check_clean_room_boundary.py` | **`FAIL`** (exit 1): `clean-room-violation:legacy-reference:evidence/validation/zjx-transport-readiness-logs-v1/make-quick.log` |
| `python3 scripts/check_supply_chain_audit.py` | **`FAIL`** (exit 1): `generated audit evidence drifted: {"drift": ["evidence/security/T54/secret-scan.json"]}` |
| `python3 scripts/generate_supply_chain_evidence.py --check` | `{"drift": ["evidence/security/T54/secret-scan.json"], "result": "FAIL"}`; T52 inventory matches |
| In-memory `build_outputs()` diff (no write) | T54 committed vs would-be at HEAD: `candidate_files_scanned` 1268 vs 1272; `candidate_bytes_scanned` 47,952,261 vs 48,111,664; `candidate_file_manifest_sha256` `6345b185...` vs `28574d6c...`; untracked counts unchanged (1 file, 15,996 bytes) |
| `git ls-files \| wc -l` | 1274 at HEAD (1259 at `4168332`); the generator excludes its 2 output paths, so 1272 is the correct HEAD count |
| Sentinel scan of the new log files (checker's own `SENTINELS` tuple) | only `make-quick.log` line 823 carries sentinel index 4 (the frozen legacy artifact's SHA-256), echoed by `python3 scripts/check_legacy_runner.py` (`"artifact_sha256"` field) into the captured `make quick` stdout. No other new file (witness, closeout, amendment copy, evidence record, other logs) carries any sentinel. |
| `sha256sum /home/dev/Downloads/Sley-ZJX-Readiness.md` | `dde3ec640bb7eec76409d44e6aea2ed362135d797126b24c57641aee03178af0` (matches header, closeout, evidence, machine summary) |
| `diff Downloads/Sley-ZJX-Readiness.md docs/audits/...AMENDMENT.md` | exactly two hunks: the 9-line prepended HTML comment header (`0a1,9`) and the one-line rewording of the section-2 origin sentence (`23c32`). Nothing else differs. |
| `git show 161a2f3 -- docs/status/HANDOFF-2026-09-15-QUALIFICATION.md` | one line changed: the "Live checkout" row's parenthetical now says the remote "was renamed to `sley.git` under the same owner" instead of spelling the origin path. Nothing else. |
| `git show 161a2f3 -- evidence/security/T54/secret-scan.json` | 3 candidate fields + 2 untracked fields changed; `result` stays `PASS_NO_HIGH_CONFIDENCE_FINDINGS`, `findings: []`, `blockers: []`, anchor unchanged |
| `git diff --stat 4168332 HEAD -- 'crates/*/src' Cargo.toml Cargo.lock 'crates/*/Cargo.toml' docs/spec conformance scripts Makefile` | empty |
| `git grep import_conformance_pack\|import_repository_exchange\|preflight_repository_exchange -- 'crates/*/src'` outside sley-repo | no callers (no CLI/production consumer; the seam is library-level) |
| `git grep ZR12-TEST-FRAME\|TransportFailure\|AdapterFailure` outside the witness and evidence | only the closeout's description (line 99) |
| Witness surface grep (`sley2.`, `MAX_* =`, `mime`, `magic`, `feature`, `#[cfg`) | no declarations; the only hits are the doc comments stating the negatives |

## 4. Independently re-derived claims

### 4.1 Insertion point (required of every lane)

From the code at HEAD, the future adapter's only insertion points are the three public byte-taking functions of `sley-repo`:

- `import_conformance_pack(store: &ObjectStore, input: &[u8], verifier: &V) -> Result<ImportReport>` (`lib.rs:413`). It is literally `preflight_conformance_pack(input, verifier)?` then `promote_pack_objects(...)` (`lib.rs:421-423`). Both callees are `pub(crate)` (`lib.rs:480`, `:527`), and `PreflightedPack` is `pub(crate)` (`lib.rs:473`), so no external code can reach promotion without the complete preflight.
- `preflight_repository_exchange(input: &[u8], verifier: &V) -> Result<ExchangePreflightReport>` (`exchange.rs:1439`), a pure wrapper over the private `preflight` (`exchange.rs:1358`); it writes nothing.
- `import_repository_exchange(target: &Path, input: &[u8], verifier: &V)` (`exchange.rs:1885`): `preflight(input, verifier)?` is the first statement; the stage marker, maintenance guard, object promotion, receipts, branches, and head follow only after it succeeds.

Why it is not a kernel dependency: `sley-repo` gains no dependency (manifests and lock unchanged; blake3 and serde_json were already a dependency and dev-dependency respectively). A composition layer would depend on `sley-repo`; nothing in the workspace depends on the witness or on any transport crate (the witness is an integration-test target, compiled into no library or binary, and the `cargo metadata` closure recorded in `dependency-closure.json` has no compression/network/loader crate).

Why it cannot bypass validation: the first act of every entry point is the size bound `input.len() > MAX_PACK_BYTES` / `MAX_EXCHANGE_BYTES` in `decode_envelope` (`lib.rs:690`, `exchange.rs:646`), followed by magic, version, contract tag, epoch, `read_len`-bounded payload, trailer, trailing-bytes, and digest checks. The accepted types (`AcceptedRepositoryPack`, `ImportReport`, `ExchangeImportReport`, `ExchangePreflightReport`) are only produced by these owning functions. The transport layer hands over `&[u8]` and nothing else: no annotation, feature flag, or claim reaches the importer's signature.

### 4.2 Witness proves what the closeout claims (adversarial reading)

- **Malformed inputs keep their owning symbol/code after reconstruction.** Pack: `pinned_pack_rejections_keep_their_owning_symbol_after_reconstruction` asserts `corpus.len() == 10`, `rebuilt.payload == input`, `via_transport.symbol() == direct.symbol()`, and the symbol family (`PACK_`/`SCB_`/`SCHEMA_`); direct route is the production importer itself, so equality is a genuine "outer layer neither erased nor replaced" proof. Exchange: `pinned_exchange_rejections_keep_their_owning_code_after_reconstruction` asserts `corpus.len() == 6` and `code() == expected_code` (from the corpus) on the direct preflight, the reconstructed preflight, and the reconstructed persistence-capable import, plus `!target.exists()`. I checked the corpus: the six `expected_code` values are `EXCHANGE_DIGEST_MISMATCH`, `EXCHANGE_PACK_INVALID`, `EXCHANGE_CANONICAL_ORDER`, `EXCHANGE_HEAD_INVALID`, `EXCHANGE_ANCESTRY_OPEN`, `SCB_TRAILING_BYTES`. The pack corpus carries only the Python oracle's `expected_reason` (`envelope`, `truncated`, `trailing`, `pack-digest`, `content-id`, `tree`, `pack-epoch`), not symbols; the printed Rust symbols are consistent with each reason (e.g. `resealed-payload-bit` flips a byte inside the first section-1 epoch entry and reseals, so Rust fails in `validate_epochs` with `PACK_SCHEMA_UNSUPPORTED` while the oracle names the epoch content id). The crate has no other Rust-side consumer of this corpus (pre-existing gap noted in the tightening audit); the witness is the first. Observation, not a finding: the per-id symbol expectation for packs lives only in the evidence record, not in an assertion, so the witness proves preservation-across-transport, not a pinned symbol table (the latter is the S20-170 owner's, not this amendment's).
- **Late outer failure persists nothing.** `adapter_import_pack` / `adapter_import_exchange` apply `?` to `reconstruct(...)` before naming the importer, so the type system, not a runtime check, prevents the importer from being reached on `Outer(..)`. The pack test flips the last finalizer byte, appends a trailing byte, and cuts 40 bytes; each yields the expected `TransportFailure` variant and `tree_snapshot(&temp.0).is_empty()`. `ObjectStore::new` only stores the path (`sley-store/src/lib.rs:191-193`), so an empty snapshot means no object was written; the same store then imports normally (`promoted_objects == 2`), proving no residual state. The exchange test asserts `!target.exists()` after a finalizer flip, then clones (2 receipts, 2 branches). Assertions are not weakened.
- **Annotations grant nothing and never reach the importer.** `Reconstructed { annotation, payload }`; every importer call passes only `payload`. `outer_annotations_change_the_frame_but_not_the_inner_identity` shows differing frames and annotations, identical payloads, and the pinned pack id after import. Structural, not merely observed.
- **Helper validates length before allocation and bounds output while producing it.** `reconstruct`: `annotation_len > 4_096` refused before `vec![0u8; annotation_len]`; `declared` (`u64 -> usize`, then `> ceiling`) refused before `Vec::with_capacity(declared)`; per segment `payload.len() + len > declared` refused before `resize`/`fill`; `payload.len() != declared` -> `Underrun`; finalizer only after the full payload; one extra read -> `TrailingBytes`. `test_frame_reader_bounds_output_and_fails_closed_on_every_outer_defect` exercises ceiling (with `native.len() - 1`), wrong leader, 8 truncation points, injected I/O fault at byte 200, overrun in the first segment header, and underrun. Observations (helper-only, non-actionable, consistent with closeout section 6 deferring work budgets): capacity equals the declared claim (bounded by the caller's ceiling, so a 64 MiB claim allocates 64 MiB before the first payload byte); `segment_count` is a raw `u32` and zero-length segments are legal, so work is bounded only by source length, not by the declared payload.
- **No prohibited expansion.** No `Cargo.toml`/`Cargo.lock` change; no `MAX_*` declaration, `sley2.` string, magic, MIME, feature, or `#[cfg]` in the witness; `TransportFailure`/`AdapterFailure` are private test enums; the leader `ZR12-TEST-FRAME/NOT-FMT!` shares no prefix with `SLEYSCB1`; error-symbol registration, declared limits, and domain-tag checkers all PASS. The evidence record's `"contract": "sley2.zjx-transport-readiness.v1"` follows the existing evidence-label convention (16 tracked evidence files carry `"contract": "sley2...."`) and the domain checker classifies such labels as non-identity evidence labels; not a new BLAKE3 domain.
- **Amendment copy.** Source digest matches; the diff is exactly the header comment plus the one section-2 sentence. Header-noted redaction is the only edit.
- **Quick-gate repairs.** The handoff-note repair is exactly one clause on one line, as described. The T54 regeneration is as described in the diff, but see finding F2: the regenerated record does not correspond to HEAD's tree.
- **Authority.** HEAD is unpushed (`origin/main` = `4168332`). RESUME.md adds one section stating no ZJX selection, nothing ships, release disposition unchanged, and that the witness rides the queued re-mint. Machine summary: `zjx_*_selected: false`, `zjx_support_shipped: false`, `publication_authority_inferred: false`, `ga_claimed: false`, `release_disposition_changed: false`, three lanes `PENDING`. Dossier delta is only the register growth (376 -> 379 obligations, 1 -> 4 PENDING) and the commit pin; GA states unchanged. No authority beyond RESUME.md is claimed.

## 5. Per-item analysis

| Item | Judgment |
|---|---|
| ZR-03 / section 4 prohibited expansion | Clean. Verified by checkers and by diff. |
| ZR-08 / section 9 exact selection, no fallback | Production `decode_envelope` checks magic, version, contract tag, epoch in order; corpus rows `magic`, `contract-tag`, `resealed-epoch`, `nested-exchange` all reject with their owning symbols through the transport route. |
| ZR-09 / section 10 metadata separation | Annotation is structurally unreachable by the importer. SCB1 closed-field rejection is owned elsewhere and unchanged (spec digest recorded). |
| ZR-10 / section 11 verification, errors, atomicity | Direct == reconstructed on all 16 rejections; late outer failures reach no importer; no-write assertions are strong (`!target.exists()`, empty regular-file tree on a store whose constructor writes nothing). |
| ZR-11 / section 12 resource contracts | Inner ceilings bind at `decode_envelope` before any read; helper validates claims before allocation. Closeout section 6 honestly defers transport budgets. |
| ZR-12 / section 13 witness | Uses the production importers and exporters; no mock verifier for exchanges (`RepositoryObjectVerifier` under the conformance epoch, repository built through `TransactionRepository`/`BranchRepository`). Pack rows use the synthetic fixture verifier because the pinned pack's objects are fixture objects; the same pack decoder under the production verifier is exercised through the embedded pack in every exchange row. No bypass, weakened assertion, or wrong verifier found. |
| Section 14 matrix ZT-12 (gates) | **Not evidenced at HEAD**: two of the "31 PASS + 2 repaired" claims fail at the reviewed SHA (F1, F2). Everything else in the matrix that I re-ran (witness, symbol/limit/domain checkers, non-effect diff) holds. |
| Section 14 ZT-14 (binding to the candidate) | Machine summary `witness_commit` and `independent_review.scope_sha` are `PENDING_COMMIT` at HEAD; the closeout row says the section names them (F3). |
| Test-only framing promotable to a format? | No: unregistered leader, no contract tag, no domain, no MIME, no registry entry, private enums, test target only, documented as never-promote. |
| Release-impact and GA disposition honesty | Honest: candidate preserved, records-only exemption not claimed, re-mint acknowledged as pre-existing, GA not claimed. |

## 6. Findings

**F1 [P2] [evidence/gate] `evidence/validation/zjx-transport-readiness-logs-v1/make-quick.log:823`** — The closeout commit introduces a tracked file that fails `scripts/check_clean_room_boundary.py` (`clean-room-violation:legacy-reference:...make-quick.log`). The captured `make quick` stdout includes `check_legacy_runner.py`'s JSON, whose `artifact_sha256` is the frozen legacy artifact digest, a checker sentinel; the log path is not on `SENTINEL_INVENTORY` and not under the transcript tree. Closeout section 9 ZT-12 and `evidence/validation/zjx-transport-readiness-v1.json` (`gates."make quick".remaining_steps.failed_pre_existing_repaired_in_this_closeout`) state the clean-room drift was repaired; at HEAD the same gate fails on a file this commit added. Owner action: redact the digest from the captured log (or drop the log in favour of the per-step exit summary) in the records-only follow-up, then re-run the checker; do not extend the checker inventory to admit evidence logs without an owner decision.

**F2 [P3] [evidence/gate] `evidence/security/T54/secret-scan.json:3-5`** — `scripts/check_supply_chain_audit.py` fails at HEAD with drift on the very file the closeout says was regenerated to repair drift. The committed record manifests 1268 candidate files / 47,952,261 bytes; HEAD's tree yields 1272 / 48,111,664 (1274 tracked minus the generator's 2 outputs), so the scan was generated before the last four files were staged and before later edits to already-staged files. Scan substance is unaffected (no findings, no blockers, anchor unchanged), but the closeout's ZT-12 claim is not reproducible at the reviewed SHA. Owner action: regenerate T54 as the final staged step of the records-only follow-up (after F1's log edit and after pinning `witness_commit`), then run `check_supply_chain_audit.py`.

**F3 [P4] [editorial] `docs/audits/S20_ZJX_TRANSPORT_READINESS_CLOSEOUT.md:297`** — ZT-14 states the machine-summary section "names the witness commit and the reviewed scope SHA", but at HEAD `witness_commit` and `independent_review.scope_sha` are `PENDING_COMMIT` (the section's own note defers pinning to the records-only follow-up). Reword the row to say the pin is applied by the follow-up, or pin it in that follow-up and leave the row as is.

Non-actionable observations recorded in section 4.2 (helper capacity equals the bounded claim; unbounded zero-length segment count; pack corpus has no Rust-side per-id symbol table). None of the findings touches the transport-boundary claim itself: the insertion point, the no-bypass argument, and the witness are sound; the defects are in the gate evidence the closeout attaches to them.

```
VERDICT: REVISE_0_P0_0_P1_1_P2_1_P3_1_P4
SECTION: zjx_transport_readiness
FIELD: vulcan_surface_review
SCOPE_SHA: 161a2f3480dfca3df22019a9df4e3e3e4a38674f
FINDINGS:
[P2] [evidence/gate] evidence/validation/zjx-transport-readiness-logs-v1/make-quick.log:823 - New tracked log carries the legacy freeze-digest sentinel (echoed by check_legacy_runner.py); scripts/check_clean_room_boundary.py FAILS at HEAD while closeout ZT-12 and the evidence record claim the clean-room drift was repaired.
[P3] [evidence/gate] evidence/security/T54/secret-scan.json:3-5 - Regenerated T54 record was produced before the final four files were staged (1268 files/47,952,261 bytes recorded vs 1272/48,111,664 at HEAD); scripts/check_supply_chain_audit.py FAILS at HEAD, contradicting the closeout's "repaired" claim. Regenerate as the last staged step of the records-only follow-up.
[P4] [editorial] docs/audits/S20_ZJX_TRANSPORT_READINESS_CLOSEOUT.md:297 - ZT-14 says the machine-summary section names the witness commit and scope SHA; both are PENDING_COMMIT at HEAD. Reword or pin in the follow-up.
SUMMARY: The transport-readiness claim itself holds under adversarial reading: the adapter's only insertion points are the three public byte-taking sley-repo functions whose first act is the contract size bound in decode_envelope, promotion and preflight internals are crate-private, no dependency, symbol, limit, domain, magic, MIME, feature, or manifest changed, the witness uses the production importers, exporters, and (for exchanges) the production verifier, all 16 corpus rejections keep their owning symbol/code through reconstruction, late outer failures are type-level short-circuits that persist nothing, annotations are structurally unreachable by the importer, and no authority beyond RESUME.md is claimed (HEAD is unpushed). The closeout is not yet accurate about its gate evidence at the reviewed SHA: the commit adds a log carrying a clean-room sentinel (checker FAIL) and a T54 secret-scan record generated before the tree was final (checker FAIL), both of which it reports as repaired; plus one editorial ZT-14 row. All three are records-only fixes for the already-planned follow-up commit.
```
