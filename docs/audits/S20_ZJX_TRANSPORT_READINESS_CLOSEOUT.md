# S20-ZJX-READINESS Transport Readiness Closeout

Status: **amendment disposition `READY_EXISTING` (documentation-and-test-only closeout, no production repair); Sley 2.0.0 release disposition unchanged (BLOCKED, operator-held)**

Date: 2026-09-15

Amendment: `docs/audits/S20_ZJX_TRANSPORT_READINESS_AMENDMENT.md` (tracked copy
of the operator's `Sley-ZJX-Readiness.md`, revision 2, document ID
S20-ZJX-READINESS, activated by the operator's implementation instruction on
2026-09-15). Section numbers below are the amendment's.

Governing principle applied: preserve Sley's canonical contracts; make
transport replaceable outside them. This closeout adds no production code.

## 1. What this closeout states, and what it does not

- The byte-oriented boundary the amendment requires already exists in
  `sley-repo` for both supported artifact profiles (S20-170 repository packs,
  S20-540 repository exchanges). It is proven by production symbols, existing
  tests, and a new executable witness, not by a diagram.
- No ZJX release, edition, backend, profile, ABI, packaging, wire format,
  dictionary scheme, codec, FFI boundary, subprocess backend, or license tier
  was selected. No ZJX support is shipped. No `.sley.zjx` format, magic, MIME,
  envelope, protocol operation, command, registry, or provider loading was
  added. No publication, release, tag, mint, or GA authority was inferred or
  exercised.
- `READY_EXISTING` is an amendment-local report term. It does not imply
  `GOLD_PASS`, GA, benchmark completion, ZJX interoperability, compression
  quality, or performance. The Sley 2.0.0 release disposition is derived by the
  decision dossier and stays BLOCKED for the reasons already on record
  (succession trials, signing, release decision), none of which this amendment
  touches.

## 2. Baseline and candidate identities (ZR-01)

| Item | Value |
|---|---|
| Authoritative checkout | `/home/gfarch/Work/workspaces/sley2`, branch `main` |
| Baseline HEAD at activation | `4168332f34ba4ff4567fd60df8f2adc42083b1e1` (equal to `origin/main`); working tree clean except one in-flight review transcript (`evidence/review/verdicts/standards_sbom_and_provenance/nabu_architecture_review-70283ce.md`, untracked, owned by a concurrent read-only reviewer lane, not touched) |
| Audit commit named by the amendment | `70283ce1932d4182f206f0d09e6129af3976a4c2` (parent of the baseline; the only difference at HEAD is the 2026-09-15 handoff note, no code or contract change) |
| Concurrent worktrees | `/home/gfarch/Work/workspaces/sley2-campaign` at `70283ce` on `campaign/s20-640`, empty; three council reviewer sessions running read-only against `70283ce` (transcripts only). No writer conflict: this closeout touches no file those lanes write |
| Toolchain | `rust-toolchain.toml` channel 1.93.0; `rustc 1.93.0 (254b59607 2026-01-19)`, `cargo 1.93.0 (083ac5135 2025-12-15)` |
| Dependency lock digest | `Cargo.lock` SHA-256 `4b6af7f0f01b23db0bb44f23385ab82e1ed979c80bd57873243dd3e391d4eacc` (unchanged by this closeout) |
| Release feature set | every workspace crate declares no default features; the only declared features are `sley-repo/test-support` and `sley-txn/s20-530-test-hooks` (test-only); the witness enables neither |
| Contract digests (SHA-256) | `REPOSITORY_PACK_V1.md` `1e249e61…da594`; `REPOSITORY_EXCHANGE_V1.md` `42f31246…b359`; `SCB1.md` `a1422d63…0c86f`; `ERROR_CODES_V1.md` `0a5bff84…7dd9` (full values in `evidence/validation/zjx-transport-readiness-v1.json`) |
| Fixture digests | pack `accepted.json` `35685f78…6963`, `rejected.json` `9a08dc59…8b60`; exchange `accepted.json` `a0674fb8…e31a`, `rejected.json` `bbf81297…dbe0`; both `SHA256SUMS` verify |
| Attested release candidate | commit `7a94a4a`, artifact `6d970bf4…e159`, MULTI_HOST_REPRODUCIBLE; already stale versus HEAD before this closeout (attestation-bound changes landed at `f7df74f` and `70283ce`) |
| Qualification state at activation | finding register 375 obligations, 1 PENDING; dossier BLOCKED; GA report 48 EVIDENCED / 2 AWAITS_REVIEW / 2 GATED |
| Supported artifact profiles at this baseline | S20-170 pack (`S20_170_COMPLETE`) and S20-540 exchange (`S20_540_COMPLETE`, contract revision 7); both frozen at compression profile `0` |
| Amendment source digest | `Sley-ZJX-Readiness.md` SHA-256 `dde3ec640bb7eec76409d44e6aea2ed362135d797126b24c57641aee03178af0` |

The audit commit was not checked out, reset to, or rebased onto. No file
was cleaned, overwritten, or amended.

## 3. Discovered insertion point and architecture (ZR-04, ZR-07)

The future adapter inserts at exactly these production symbols, all of which
take bytes (a `&[u8]` bounded by the contract ceiling at entry) and none of
which interpret an input artifact pathname:

| Profile | Byte source (egress) | Preflight, no writes | Persistence-capable import (ingress) | Destination |
|---|---|---|---|---|
| S20-170 pack | `export_conformance_pack` (`crates/sley-repo/src/lib.rs:343`) returns `AcceptedRepositoryPack::stored_bytes` | `preflight_conformance_pack` (`lib.rs:480`, crate-private; entered by the importer) | `import_conformance_pack(store, &[u8], verifier)` (`lib.rs:413`) = preflight steps 1-5, then `promote_pack_objects` (`lib.rs:527`, step 6) | `ObjectStore` root path |
| S20-540 exchange | `export_repository_exchange` (`crates/sley-repo/src/exchange.rs:954`) returns `AcceptedRepositoryExchange::stored_bytes` | `preflight_repository_exchange(&[u8], verifier)` (`exchange.rs:1439`, public; import steps 1-6) | `import_repository_exchange(target, &[u8], verifier)` (`exchange.rs:1885`) = preflight, target classification, then persistence 8.1-8.7 | target directory path |

Conceptual flow, realized without any new symbol:

```text
native bytes (exporter, fixture, or future outer reconstruction)
    -> import_conformance_pack / preflight_repository_exchange / import_repository_exchange
    -> existing acceptance rules (S20-170 steps 1-7, S20-540 steps 1-9)
    -> existing checked state (AcceptedStateRoot, AcceptedHead, refs, objects)
```

Dependency direction: `sley-repo -> sley-txn -> sley-store` is unchanged. A
future composition layer would depend on `sley-repo` (and a selected backend);
nothing in `sley-id`, `sley-scb1`, `sley-schema`, `sley-state-root`,
`sley-store`, `sley-txn`, `sley-repo`, `sley-vm`, or `sley-protocol` depends
on it or on any transport crate (ZT-11 below). The existing execution context
(`ExecutionRequest`, policy roots, capability summaries) is untouched; no
`run(graph)` shortcut exists or was added.

The adapter cannot bypass the importer: no persistence or execution API
consumes an `AcceptedRepositoryPack`, `AcceptedRepositoryExchange`,
`ImportReport`, or `ExchangeImportReport` value (the public
`seal_mutated_conformance_pack_for_testing` builds a pack value, and the
report structs are constructible, but neither confers acceptance; the only
way bytes reach a store or a target is through the importers, which take
`&[u8]`), `PreflightedPack` and `promote_pack_objects` are crate-private, and
the transaction-owner clone API (`initialize_trusted_clone_*_with_maintenance`)
re-verifies every receipt under the importer's exclusive maintenance guard
and the incomplete-clone marker.

Streaming is not required and not claimed. The exchange contract's own text
places streamed or compressed profiles outside v1.

## 4. Existing versus added code

Added (this closeout):

- `crates/sley-repo/tests/zjx_readiness_witness.rs`: an integration test
  target (10 tests) using only the public API. It contains a deliberately
  test-only segmented framing (`ZR12-TEST-FRAME/NOT-FMT!` leader, big-endian
  fixed-width lengths, BLAKE3 finalizer, outer-only annotation). It is not
  SCB1, not a Sley contract, not ZJX, has no `sley2.` domain, no contract tag,
  no MIME, no public error symbol (its failures are a local enum), is compiled
  into no library or binary, and enables no feature.
- `docs/audits/S20_ZJX_TRANSPORT_READINESS_AMENDMENT.md` (tracked copy, one
  clean-room redaction noted in its header) and this closeout.
- `evidence/validation/zjx-transport-readiness-v1.json` and
  `evidence/validation/zjx-transport-readiness-logs-v1/` (commands, results,
  logs), `evidence/validation/test-inventory.json` (rebuilt: +10 tests),
  the `zjx_transport_readiness` section of the machine summary, and the
  derived register/dossier rebuilds.

Not changed: every file under `crates/*/src`, every `Cargo.toml`,
`Cargo.lock`, every contract under `docs/spec`, every conformance fixture,
every oracle. `git diff --stat HEAD -- 'crates/*/src' Cargo.toml Cargo.lock
'crates/*/Cargo.toml'` is empty (recorded in `baseline.txt`).

No production repair was proposed or made, so the ZR-02 pre-repair record
(obstruction, files, compatibility impact, rollback boundary) is empty by
construction and no repair review was needed. No stop condition of section 17
was reached.

## 5. Requirement matrix (ZR-01 to ZR-14)

Dispositions are the amendment's initial vocabulary; all are
`EXISTING_PROVEN` unless noted. "Commit" is the commit introducing this
closeout (the witness commit) unless a prior commit is named; reviewer
disposition is filled by the independent lanes in section 10.

| ID | Requirement (short) | Owning contract | Implementation symbol(s) | Existing test / executable witness | Disposition |
|---|---|---|---|---|---|
| ZR-01 | Evidence basis and activation baseline recorded | this closeout, `RESUME.md`, `docs/status/HANDOFF-2026-09-15-QUALIFICATION.md` | n/a | section 2 above; `evidence/validation/zjx-transport-readiness-v1.json` | EXISTING_PROVEN (records) |
| ZR-02 | Authority and controlled scope | `CONTRIBUTING.md`; `RESUME.md` push/mint discipline | n/a | no production repair; no commit/push/mint/publication authority inferred; independent review dispatched under the established council-lane workflow | EXISTING_PROVEN (records) |
| ZR-03 | No dependency, format, command, registry, or language added | `ARCHITECTURE.md` dependency law; `docs/ANTI_GOALS.md` | workspace manifests unchanged | ZT-11 dependency closure; `scripts/build_anti_goal_conformance.py` corpus (`greyforge_product` scan) | EXISTING_PROVEN |
| ZR-04 | Required architecture and dependency direction | `ARCHITECTURE.md`, ADR-0003, ADR-0025 | `import_conformance_pack`, `import_repository_exchange`, `preflight_repository_exchange` | section 3; witness `*_converge_through_the_production_importer` | EXISTING_PROVEN |
| ZR-05 | Reuse existing artifact contracts; profile 0 frozen | `REPOSITORY_PACK_V1.md`, `REPOSITORY_EXCHANGE_V1.md` | `validate_profile` (`lib.rs:838`), exchange profile check (`exchange.rs:804`), `PACK_COMPRESSION_UNSUPPORTED`, `EXCHANGE_COMPRESSION_UNSUPPORTED`, reserved `PACK_DECOMPRESSION_LIMIT` | `tests::later_profiles_fail_closed`, `tests::stable_reserved_error_symbol_exists`, exchange `exchange_codes_are_closed_and_contiguous`; `check_repository_pack_spec.py`, `check_repository_exchange_spec.py` | EXISTING_PROVEN |
| ZR-06 | Identity and exact preservation | `IDENTIFIERS_V1.md`, pack and exchange envelopes | `RepositoryPackId::derive`, `RepositoryExchangeId`, `StateRoot`, `TransactionId`, `ReceiptId`, `ObjectId`, `SchemaEpochId` | witness: `reconstruct(frame(native)) == native` under four segmentations and byte-identical destination trees; `check_repository_pack_vector.py`, `check_repository_exchange_vector.py` (independent Python) | EXISTING_PROVEN |
| ZR-07 | Byte-oriented boundary without mandatory streaming | pack and exchange import phases | `&[u8]` inputs bounded at `decode_envelope` (`lib.rs:689`, `exchange.rs:645`) | witness (all tests take bytes; destination path only); helper short-read/EOF/I/O rows in `test_frame_reader_bounds_output_and_fails_closed_on_every_outer_defect` (helper behavior only) | EXISTING_PROVEN |
| ZR-08 | Exact contract, epoch, version selection; no fallback | `SCB1.md`, `SCHEMA_EPOCH_V1.md`, pack/exchange envelopes | envelope magic, version, contract tag, epoch lookup via `SchemaEpochRegistry::lookup_contract` and `decode_contract`; nested-exchange rejection | pack corpus `magic`, `contract-tag`, `resealed-epoch`; exchange corpus `nested-exchange`; `wrong_version_and_epoch_set_fail_with_their_owned_symbols` | EXISTING_PROVEN |
| ZR-09 | Metadata and authority separation | `SCB1.md` closed fields and extension allowlist | `RecordReader::required`/`finish` (`SCB_FIELD_UNKNOWN`, `SCB_FIELD_ORDER`, `SCB_FIELD_DUPLICATE`), SCB1 `SCB_EXTENSION_UNKNOWN` | SCB1 corpus `record-unknown-field`, `unknown-extension`, `record-duplicate-field`, `record-field-order` (`conformance/scb1/v1/rejected.json`, oracle-checked in `make conformance`); witness `outer_annotations_change_the_frame_but_not_the_inner_identity` | EXISTING_PROVEN |
| ZR-10 | Verification, errors, atomicity | pack steps 1-7; exchange steps 1-9; `CRASH_RECOVERY_MATRIX_V1.md` | preflight/persistence split (`preflight_conformance_pack` then `promote_pack_objects`; `preflight` then 8.1-8.7 under the maintenance guard, head written last, marker as write guard) | pack `*_fails_before_promotion` (6 tests) and `object_verifier_failure_precedes_all_promotions`; exchange `target_rules_fail_closed_before_any_write`, `interruption_rows_x01_to_x07_converge_on_retry`, `frozen_write_paths_fail_closed_on_a_marked_root`; witness rejection and late-outer-failure tests | EXISTING_PROVEN |
| ZR-11 | Resource contracts | pack and exchange resource-limit sections; `check_declared_limits.py` | section 6 map | `decode_limits_bind_before_allocation_and_a_maximal_shape_decodes`; `bounded_pack_import_fuzz_smoke_rejects_rehashed_mutations`; S20-700 pack and exchange persistent slices | EXISTING_PROVEN |
| ZR-12 | Concrete integration witness | this closeout | production importers and exporters | `crates/sley-repo/tests/zjx_readiness_witness.rs`, 10/10 PASS | added test only |
| ZR-13 | Compatibility and performance | frozen contracts; `RELEASE_CANDIDATE_PACKAGING_V1.md` | no production byte changed | section 7 (executable non-effect) | EXISTING_PROVEN |
| ZR-14 | Release evidence and requalification | `REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md`, GOLD invalidation rules | n/a | section 8 (evidence-impact map) | EXISTING_PROVEN (records) |

## 6. Resource-boundary map (ZR-11)

Inner ceilings that bind at the Sley boundary, unchanged, with their
enforcement point and failure code:

| Ceiling | Value | Enforced at | Failure |
|---|---:|---|---|
| Pack stored bytes | 67,108,864 | `decode_envelope` entry (`lib.rs:689`) before any read; payload length claim bounded by `read_len(MAX_PACK_BYTES)` | `PACK_RESOURCE_LIMIT` |
| Pack expanded bytes | 67,108,864 | `check_expanded_bytes` (`lib.rs:1026`) during payload decode | `PACK_RESOURCE_LIMIT` |
| Pack epochs / roots / objects / leaves | 256 / 4,096 / 65,536 / 69,888 | `check_counts` (`lib.rs:1014`) | `PACK_RESOURCE_LIMIT` |
| Pack allocation budget | 134,217,728 | `check_expanded_bytes` (`lib.rs:1038`) | `PACK_RESOURCE_LIMIT` |
| Pack compression profile | must be 0 | `decode_payload` before any block interpretation; `validate_profile` | `PACK_COMPRESSION_UNSUPPORTED` (`PACK_DECOMPRESSION_LIMIT` reserved, never emitted) |
| Exchange stored bytes | 67,108,864 | `decode_envelope` entry (`exchange.rs:646`) | `EXCHANGE_RESOURCE_LIMIT` |
| Embedded pack bytes | 16,777,216 | `exchange.rs:800` (import) and `:1004` (export) | `EXCHANGE_RESOURCE_LIMIT` |
| Exchange receipts / branches / leaves | 4,096 / 4,096 / 8,194 | payload decode, before allocation | `EXCHANGE_RESOURCE_LIMIT` |
| Exchange allocation budget (shared top-down with the embedded pack decoder) | 134,217,728 | `exchange.rs:835` | `EXCHANGE_RESOURCE_LIMIT` |
| Preflight work ceilings (object verifications, binding visits, object bytes, receipt bytes) | 2,097,152 / 4,194,304 / 1,073,741,824 / 1,073,741,824 | single counters over the whole preflight (`exchange.rs:1337` and siblings) | `EXCHANGE_RESOURCE_LIMIT` |
| Exchange compression profile | must be 0 | `exchange.rs:804`, before block interpretation | `EXCHANGE_COMPRESSION_UNSUPPORTED` |
| Exchange expanded bytes | reserved | not bindable under profile 0 | (future profile must define) |

Nested accounting: the exchange charges its embedded pack against its own
budget rather than granting a fresh S20-170 budget (contract "inner
admissibility" invariant, proven by the maximal-legal-exchange decode test).
No limit is multiplied, reset, or weakened here.

Transport budgets left to the future integration specification (not invented
here): encoded-input ceiling, reconstructed-output ceiling (which must not
exceed the inner stored ceiling and must be enforced while output is
produced), nested frame/dictionary/metadata bounds, work and cancellation
budgets, and the simultaneous-liveness accounting of transport buffer plus
reconstructed bytes plus decoder allocations. The test-only helper shows the
shape (declared length validated against a caller-supplied ceiling before
allocation; running output checked per segment) and proves nothing about any
real backend.

Finding under existing owners: none. No missing validity-affecting safeguard
was observed in the audited production paths.

## 7. Compatibility and performance evidence (ZR-13)

- Accepted and rejected behavior frozen before and after: the pinned pack and
  exchange fixtures, their `SHA256SUMS`, both independent Python vector
  checks, and both fixture drift gates pass unchanged
  (`evidence/validation/zjx-transport-readiness-logs-v1/zt13-vectors.log`).
  No golden data was regenerated.
- Executable non-effect: no file under any crate's `src/`, no manifest, and
  the lock file changed; `cargo build -p sley-cli --locked` reports nothing
  to rebuild and `target/debug/sley` hashes identically before and after
  (`baseline.txt`, `non-effect.txt`). Hot paths are untouched; no provider
  dispatch was introduced. A performance campaign is therefore inapplicable
  under the amendment's own rule for docs/test-only changes; the existing
  benchmark baseline check (`check_benchmark_baseline.py`, in `make quick`)
  still passes.
- Public error symbols, numeric codes, phase ordering, and precedence:
  unchanged (`check_error_symbol_registration.py --check` and
  `check_declared_limits.py --check` in `make quick`; witness asserts the same
  symbol/code from the direct and reconstructed routes for all 16 corpus
  rejections).

## 8. Release-impact map (ZR-14)

| Evidence | Status after this closeout |
|---|---|
| Candidate `7a94a4a` / artifact `6d970bf4…e159` and its dual-host attestation | preserved, not overwritten, not reused; already stale versus HEAD before this closeout; still stale (this closeout adds attestation-bound files under `crates/` and `docs/`) |
| Records-only exemption | not claimed: `crates/sley-repo/tests/` and `docs/audits/` are outside `evidence/` and `machineresearch/`, so the next mint re-binds as `RESUME.md` already requires |
| Prior council verdicts for S20-170, S20-540, S20-700 pack/exchange slices | remain valid: no contract, fixture, decoder, or symbol changed |
| Reproducibility report, SBOM, provenance, checksums | unchanged files; the reproducibility and standards checkers keep reporting stale/ineligible until the planned re-mint (expected, pre-existing) |
| Finding register / decision dossier | rebuilt from the machine summary with the new `zjx_transport_readiness` section; the register's obligation count grows by this section's lane fields; GA states unaffected |
| Test inventory | rebuilt (`build_test_inventory.py`): sley-repo 396 -> 406 test attributes, total 1,425 -> 1,435 |
| Benchmark results | not affected (no executable change) |

Blocks: none new. (The first review round found that the filed `make-quick.log` carried a clean-room sentinel and that the T54 scan had been regenerated before the last files were staged; both are corrected at the revision commit, where the T54 scan is regenerated as the last staged step.) Re-mint requires the existing procedure (detached
worktree, lab attestation, merge with `--attest`) and is already queued after
the running review cycle; it is not this amendment's obligation.

## 9. Validation matrix (ZT-01 to ZT-14)

Baseline identity for every row: `4168332` (HEAD at activation) plus the
working-tree witness; candidate identity: the witness commit. Logs live under
`evidence/validation/zjx-transport-readiness-logs-v1/`.

| Row | Observation required | Command(s) | Result | Evidence |
|---|---|---|---|---|
| ZT-01 | Positive fixtures and production exports keep exact bytes, IDs, epoch, accepted facts | `cargo test -p sley-repo --locked --test zjx_readiness_witness` (`pinned_pack_direct_and_reconstructed_inputs_converge…`, `pinned_exchange_direct_and_reconstructed_inputs_converge…`, `production_pack_exporter_bytes_survive…`, `production_exchange_exporter_bytes_survive…`); `uv run … check_repository_pack_vector.py`; `uv run … check_repository_exchange_vector.py` | PASS | `witness-run.log`, `zt13-vectors.log` |
| ZT-02 | Direct and reconstructed input converge through the same production importer | same witness tests: equal `ImportReport` fields, equal `ExchangePreflightReport`, equal clone trees under two destination paths, re-export byte-identical | PASS (4 segmentations x 2 profiles) | `witness-run.log` |
| ZT-03 | Malformed fixtures retain owning symbols; no normalization | witness `pinned_pack_rejections_keep_their_owning_symbol…` (10 corpus inputs; observed symbols `SCB_MAGIC_INVALID`, `SCB_CONTRACT_UNKNOWN`, `SCB_LENGTH_OVERFLOW`, `SCB_TRAILING_BYTES`, `PACK_DIGEST_MISMATCH` x3, `PACK_SCHEMA_UNSUPPORTED`, `PACK_DIGEST_TREE_MISMATCH`, `SCHEMA_EPOCH_MISMATCH`) and `pinned_exchange_rejections_keep_their_owning_code…` (6 corpus inputs, codes equal to the corpus `expected_code`) | PASS | `witness-run.log` |
| ZT-04 | Truncation, trailing bytes, wrong contract/version/epoch, unsupported compression reject | corpus rows `truncated`, `trailing-byte`, `contract-tag`, `resealed-epoch`, `flip-epoch` via ZT-03; `cargo test -p sley-repo --locked` (`later_profiles_fail_closed`, `wrong_version_and_epoch_set_fail_with_their_owned_symbols`, `outer_trailing_byte_fails_before_promotion`, `nonminimal_outer_varint_is_rejected`) | PASS | `witness-run.log`, `cargo-test-sley-repo.log` |
| ZT-05 | Missing, surplus, duplicate, reordered, substituted contents keep closure rejection | `cargo test -p sley-repo --locked` (`missing_object_fails_before_promotion`, `surplus_object_fails_before_promotion`, `duplicate_root_is_rejected`, `reordered_inventory_fails_before_promotion`, `substituted_object_fails_before_promotion`, `dependency_requires_included_root`, `altered_tree_with_valid_outer_id_is_rejected`; exchange `reversed-branches`, `foreign-head`, `open-ancestry`, `nested-exchange` corpus rows through the witness) | PASS | `cargo-test-sley-repo.log`, `witness-run.log` |
| ZT-06 | Unknown canonical fields/extensions rejected; annotations grant nothing | `cargo test -p sley-scb1 --locked`; SCB1 corpus `record-unknown-field` (`SCB_FIELD_UNKNOWN`), `unknown-extension` (`SCB_EXTENSION_UNKNOWN`), `record-duplicate-field`, `record-field-order` (oracle-checked by `make conformance`, drift-gated by `check_scb1_spec.py` in `make quick`); witness `outer_annotations_change_the_frame_but_not_the_inner_identity` | PASS | `cargo-test-sley-scb1.log`, `make-quick.log` |
| ZT-07 | Invalid preflight leaves state untouched; persistence faults keep the owning recovery model | witness no-write assertions (empty store tree; nonexistent target) for all 16 rejections and 3 outer failures; `cargo test -p sley-repo --locked` (`interruption_rows_x01_to_x07_converge_on_retry`, `interrupted_clones_converge_on_retry_with_identical_bytes`, `target_rules_fail_closed_before_any_write`, `owned_re_classification_aborts_a_target_changed_after_the_advisory_pass`) | PASS | `witness-run.log`, `cargo-test-sley-repo.log` |
| ZT-08 | Limits enforced at the correct layer before excess allocation/output; nested accounting documented | section 6; `cargo test -p sley-repo --locked` (`decode_limits_bind_before_allocation_and_a_maximal_shape_decodes`, `bounded_pack_import_fuzz_smoke_rejects_rehashed_mutations`); helper-only: witness `test_frame_reader_bounds_output_and_fails_closed_on_every_outer_defect` (ceiling before allocation, overrun per segment, underrun, truncation at 8 cut points, injected I/O fault, wrong leader) | PASS (helper rows prove the helper only) | `cargo-test-sley-repo.log`, `witness-run.log` |
| ZT-09 | Outer failure, including late finalization, cannot cause early import/promotion/success | witness `late_outer_failure_after_a_complete_inner_pack_payload_persists_nothing` (finalizer flip, trailing byte, truncation: store tree empty, then a clean frame imports normally) and `…_exchange_payload_persists_nothing` (target does not exist after the outer failure; a clean frame then clones) | PASS | `witness-run.log` |
| ZT-10 | Destination path and outer annotations do not change inner identity; canonical metadata stays bound | witness: identical object-store and clone trees under different destination paths; annotation changes the frame, not the payload, pack ID, roots, or objects | PASS | `witness-run.log` |
| ZT-11 | No ZJX/backend/plugin/network requirement in default or release features | `cargo metadata --locked` closure: 27 external crates in the normal closure, 30 with dev/build kinds; none matching zjx, zstd, flate, lz4, brotli, snap, xz, lzma, reqwest, hyper, tokio, libloading, dlopen, openssl, rustls, curl, ureq, wasm, ffi; no default features anywhere; `build_anti_goal_conformance.py` corpus (`greyforge_product` sentinel scan) via `make evidence-refresh` history | PASS | `dependency-closure.json`, `baseline.txt` |
| ZT-12 | Machine protocol, CLI, diagnostics, execution unchanged | `make quick` (all contract checkers, `cargo check --workspace`, `cargo test --workspace`); `make lint` (`cargo fmt --check`, workspace clippy `-D warnings` with pedantic) | `make lint` PASS. `make quick` stops at the pre-existing staleness tripwire (expected until the re-mint); the 34 steps after it were run individually: 31 PASS, the standards checker's pre-existing `closure:ineligible` (expected), and two pre-existing drifts repaired here (the 2026-09-15 handoff note spelled the renamed origin, one clause reworded; the T54 secret scan regenerated); `cargo test --workspace`: 295 passed, 0 failed. Correction at the revision commit (Nabu P2 at `161a2f3`): the filed `make-quick.log` itself carried the S20-600 legacy artifact digest printed by `check_legacy_runner.py`, so `check_clean_room_boundary.py` failed once the log was tracked; the digest is redacted in the log and the checker passes again at the revision | `make-quick.log`, `make-quick-remaining-steps.log`, `make-lint.log`, `evidence/build/lint-report.json` |
| ZT-13 | Independent vectors/oracle results unchanged; no golden regeneration | `sha256sum -c SHA256SUMS` (pack, exchange); `check_repository_pack_vector.py`; `check_repository_exchange_vector.py`; `generate_repository_pack_rejections.py --check`; `generate_repository_exchange_fixtures.py --check` | PASS (all four OK, drift `[]`) | `zt13-vectors.log` |
| ZT-14 | Qualification impact, artifacts, performance evidence, independent review bound to the actual candidate | sections 2, 7, 8, 10; witness commit `161a2f3480dfca3df22019a9df4e3e3e4a38674f` (reviewed scope); the revision commit answers the lane findings; the records-only follow-up pins `witness_commit`, `review_scope_sha`, and the lane verdicts in the machine-summary section `zjx_transport_readiness` | see section 10 | `zjx-transport-readiness-v1.json`, `evidence/review/verdicts/zjx_transport_readiness/` |

Inapplicable rows: none. Skipped checks: none. Failing baselines carried
forward unchanged: the pre-existing candidate-staleness tripwire in
`check_reproducibility_and_independent_conformance.py` and the standards
checker's `closure:ineligible` branch (both expected until the re-mint, both
recorded in the 2026-09-15 handoff and unrelated to this amendment).

## 10. Independent review (ZR-02, ZR-12, ZR-14)

Three read-only council lanes (Ariadne contract, Nabu architecture, Vulcan
surface/security) were dispatched against the witness commit `161a2f3` with
the standing brief. Round 1 (transcripts `<lane>-161a2f3.md`): all three
upheld the transport-readiness claim and the insertion-point analysis and
returned REVISE on records-only items (Ariadne `REVISE_0_P0_0_P1_1_P2_2_P3_2_P4`,
Nabu `REVISE_0_P0_0_P1_1_P2_0_P3_2_P4`, Vulcan `REVISE_0_P0_0_P1_1_P2_1_P3_1_P4`):
the filed `make-quick.log` carried the legacy artifact digest sentinel, the
T54 scan predated the final staging, section 3's "produced only by the
owning functions" sentence was inexact, the ZR-10 test count was off by one,
and ZT-14 pointed at a not-yet-pinned commit. Every item is answered at the
revision commit; the delta round reviews that commit. Each lane must state the adapter's exact insertion point,
explain why it is neither a kernel dependency nor a validation bypass, and
judge this closeout's matrix. Transcripts:
`evidence/review/verdicts/zjx_transport_readiness/<lane>-<scope_sha>.md`;
lane fields: `ariadne_contract_review`, `nabu_architecture_review`,
`vulcan_surface_review` in the `zjx_transport_readiness` machine-summary
section. The section's status stays `REVIEW_PENDING` until all three lanes
carry the bare `PASS` token; a REVISE or FAIL rotates per the register rules
and reopens this closeout.

## 11. Unresolved findings and deferred decisions

- None open against this amendment at closeout (subject to section 10).
- Section 19 of the amendment lists every decision the later integration
  specification owns (ZJX release/edition, embedding and distribution rights,
  library/FFI/process boundary, supported inner kinds, outer envelope and IDs,
  recognition, profile and dictionary rules, streaming/random access, budgets,
  deterministic outer encoding, trust/signature policy, update compatibility,
  distribution behavior, tooling, structure-aware transforms). None was
  decided here.
- No claim is made that transport compression reduces model context-window
  usage, nor any speed, size, adoption, security, or enterprise-boundary claim.

## 12. Separate Sley 2.0.0 release disposition

Unchanged: the decision dossier derives BLOCKED; GA is not claimed; the
succession trials, signing/transparency, history re-anchor, re-mint, and the
release/publication decision remain operator-held as recorded in `RESUME.md`
and the 2026-09-15 handoff. This amendment's `READY_EXISTING` is a
readiness statement about the transport boundary only.
