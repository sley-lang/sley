# Council verdict: section `zjx_transport_readiness`, field `nabu_architecture_review`, role Nabu (architecture)

Reviewer lane: Nabu (architecture). Scope: the S20-ZJX-READINESS closeout
commit. Read-only review; this transcript is the only file written.

## 1. Scope verification

- `git rev-parse HEAD` -> `161a2f3480dfca3df22019a9df4e3e3e4a38674f` (equals SCOPE_SHA; branch `main`).
- `git status --short` -> one untracked file only,
  `evidence/review/verdicts/standards_sbom_and_provenance/nabu_architecture_review-70283ce.md`
  (another lane's transcript, not touched, not read).
- `git log --oneline 4168332..161a2f3` -> exactly one commit, `161a2f3
  Qualification: S20-ZJX-READINESS transport readiness closeout (READY_EXISTING, docs and witness only)`.

## 2. Inputs read in full

- `docs/audits/S20_ZJX_TRANSPORT_READINESS_AMENDMENT.md` (282 lines, sections 1-20).
- `docs/audits/S20_ZJX_TRANSPORT_READINESS_CLOSEOUT.md` (283 lines, sections 1-12).
- `crates/sley-repo/tests/zjx_readiness_witness.rs` (1,015 lines, 10 tests).
- `evidence/validation/zjx-transport-readiness-v1.json`; logs `witness-run.log`,
  `baseline.txt`, `non-effect.txt`, `make-quick.log` (structure and the
  flagged region), `make-quick-remaining-steps.log` (clean-room step).
- Machine-summary section `zjx_transport_readiness` of
  `machineresearch/sley-2.0/machine-summary.json`.
- `crates/sley-repo/src/lib.rs`: lines 1-80 (module layout, constants),
  330-545 (`export_conformance_pack`, `import_conformance_pack`, the two
  `*_for_testing` helpers, `PreflightedPack`, `preflight_conformance_pack`,
  `promote_pack_objects`), 685-848 (`decode_envelope`, `decode_payload`,
  list decoders, `validate_profile`), 1012-1044 (`check_counts`,
  `check_expanded_bytes`), 1087-1104 (`decode_list`), 1155-1170 (`read_len`).
- `crates/sley-repo/src/exchange.rs`: lines 1-110 (composition, ceilings),
  640-850 (`decode_envelope`, entry decoders, `decode_payload`), 945-1046
  (`export_repository_exchange`), 1300-1490 (`verify_receipts_against_pack`,
  `preflight`, `preflight_repository_exchange`), 1880-1980
  (`import_repository_exchange`), 2649-2700 and 3255-3275 (limit and
  nested-exchange tests).
- `crates/sley-txn/src/repository.rs` 2470-2600
  (`initialize_trusted_clone_receipts_with_maintenance`,
  `initialize_trusted_clone_head_with_maintenance`);
  `crates/sley-txn/src/maintenance.rs` symbol index.
- `crates/sley-repo/src/test_support.rs` header; `crates/sley-repo/Cargo.toml`.
- `ARCHITECTURE.md` "Dependency law"; `docs/adr/ADR-0003-crate-authority.md`;
  `docs/adr/ADR-0025-repository-exchange-boundary.md` (decisions 1-9).
- `scripts/check_clean_room_boundary.py` (sentinel rule, lines 34-48 and
  182-206, read with the sentinel values masked).
- `/home/gfarch/Downloads/Sley-ZJX-Readiness.md` (operator source, read
  only to diff against the tracked copy).

## 3. Tool results (commands run and exact results)

All checkers ran with
`SLEY2_MASTER_GOAL=/home/greyforge/machineresearch/Sley2.0mastergoal.md`.

| Command | Result |
|---|---|
| `git diff --stat 4168332 161a2f3` | 23 files, +5,603/-24: `RESUME.md`, the witness, two `docs/audits` files, the handoff note (1 line), `evidence/build/lint-report.json`, `evidence/release/decision-dossier.json`, `evidence/review/finding-register.json`, `evidence/security/T54/secret-scan.json`, `evidence/validation/test-inventory.json`, 11 files under `evidence/validation/zjx-transport-readiness-*`, `machineresearch/sley-2.0/machine-summary.json` |
| `git diff --stat 4168332 161a2f3 -- 'crates/*/src' Cargo.toml Cargo.lock 'crates/*/Cargo.toml' docs/spec conformance oracle scripts` | empty (no production source, manifest, lock, contract, fixture, oracle, or script change) |
| `cargo test -p sley-repo --locked --test zjx_readiness_witness -- --nocapture` | `10 passed; 0 failed; 0 ignored` in 0.08 s; ZT-03 symbols printed: `SCB_MAGIC_INVALID`, `SCB_CONTRACT_UNKNOWN`, `SCB_LENGTH_OVERFLOW`, `SCB_TRAILING_BYTES`, `PACK_DIGEST_MISMATCH` x3, `PACK_SCHEMA_UNSUPPORTED`, `PACK_DIGEST_TREE_MISMATCH`, `SCHEMA_EPOCH_MISMATCH` (identical to `witness-run.log`) |
| `cargo clippy -p sley-repo --all-targets --locked -- -D warnings` | clean, exit 0 |
| `cargo metadata --locked` closure (own script over `resolve`) | normal-kind external crates: 27; with dev/build kinds: 30; none matching zjx, zstd, flate, lz4, brotli, snap, xz, lzma, reqwest, hyper, tokio, libloading, dlopen, openssl, rustls, curl, ureq, wasm, ffi. Workspace dependents of `sley-repo`: `sley-protocol` (normal+dev), `sley-cli` (dev only). `sley-repo` features: `test-support` only; no defaults |
| `sha256sum crates/sley-repo/tests/zjx_readiness_witness.rs` | `912f52edee6529d84c18ad0f6f48443acf32f07a5848a4ca0d4c88ba7e5902c2` (equals `witness.file_sha256` in the evidence record) |
| `evidence/validation/test-inventory.json` | `rust_unit_tests: 1435`; `sley-repo: tests 406, ignored 4` (matches closeout section 8) |
| `sha256sum ~/Downloads/Sley-ZJX-Readiness.md` | `dde3ec64…78af0` (equals the header and evidence digests) |
| `diff <(tail -n +10 tracked copy) source` | exactly one differing line: section 2's origin-repository sentence (exit 1, one hunk). The header comment is the only other addition. Clean-room redaction claim holds |
| `python3 scripts/check_repository_pack_spec.py` | PASS, `problems: []`, `rust_unit_tests: 19` |
| `python3 scripts/check_repository_exchange_spec.py` | PASS, `problems: []`, `S20_540_COMPLETE` |
| `python3 scripts/check_error_symbol_registration.py --check` | PASS, `unregistered: []`, `unexercised: []` |
| `python3 scripts/check_declared_limits.py --check` | PASS, `undocumented_limits: 0` |
| `python3 scripts/build_anti_goal_conformance.py --check` | PASS, 25 anti-goals, `violations: []` |
| `python3 scripts/check_clean_room_boundary.py` | **FAIL**, exit 1, one problem: `clean-room-violation:legacy-reference:evidence/validation/zjx-transport-readiness-logs-v1/make-quick.log` |

Follow-up on the FAIL (sentinel values masked here on purpose; this
transcript must not carry one either):

- The checker scans every `git ls-files` entry outside
  `machineresearch/sley-2.0/reviews/` for six sentinels (the frozen archive
  path, two legacy artifact filenames, the origin name, two freeze digests)
  and flags any file not in its `SENTINEL_INVENTORY`.
- `make-quick.log` line 823 is `"artifact_sha256": "<64hex>"` inside the
  JSON that `python3 scripts/check_legacy_runner.py` printed at line 819 of
  the log (the S20-600 frozen legacy artifact digest). Committing the raw
  quick log therefore committed a sentinel into a non-inventory file.
- `make-quick-remaining-steps.log` shows the clean-room step run
  individually failing on the handoff note (which the commit then reworded);
  the log file itself was untracked at that time, so the checker never saw
  it. At `161a2f3` it is tracked and the gate fails. No other file added by
  the commit carries a sentinel (checked the closeout, the amendment copy,
  `baseline.txt`, the remaining-steps log).

## 4. Independently re-derived claims

### 4.1 Insertion point (from the code, not the closeout)

A future optional transport adapter is a composition layer that (a) obtains
native bytes from `sley_repo::export_conformance_pack(store, roots,
verifier) -> AcceptedRepositoryPack { stored_bytes, .. }` (`lib.rs:343`) or
`sley_repo::export_repository_exchange(root, verifier) ->
AcceptedRepositoryExchange { stored_bytes, .. }` (`exchange.rs:954`),
(b) fully reconstructs the exact bytes into a bounded buffer, and (c) hands
that `&[u8]` to exactly one of:

- `sley_repo::import_conformance_pack(&ObjectStore, &[u8], &V) -> Result<ImportReport>` (`lib.rs:413`),
- `sley_repo::preflight_repository_exchange(&[u8], &V) -> Result<ExchangePreflightReport>` (`exchange.rs:1439`, no writes),
- `sley_repo::import_repository_exchange(&Path, &[u8], &V) -> Result<ExchangeImportReport>` (`exchange.rs:1885`).

No signature takes an input artifact pathname; the only paths are the
destination store root and clone target. There is no other public entry
that turns bytes into promoted objects, receipts, branches, or a head.

### 4.2 Why it is not a kernel dependency

- `ARCHITECTURE.md` dependency law and ADR-0003 place transports and "ZJX"
  explicitly outside the kernel; the direction is `... -> txn -> repo/vm ->
  protocol -> bridge/cli`. A composition layer sits after `sley-repo` in
  that order (it imports `sley-repo`; nothing imports it).
- `cargo metadata --locked`: the only workspace crates depending on
  `sley-repo` are `sley-protocol` and, dev-only, `sley-cli`. Every kernel
  crate the amendment names (`sley-id`, `sley-scb1`, `sley-schema`,
  `sley-state-root`, `sley-store`, `sley-txn`, `sley-repo`, `sley-vm`,
  `sley-protocol`) has no path to any transport crate because no such crate
  exists in the closure (27 normal external crates, none compression,
  network, FFI, or loader related). `sley-repo` declares no default
  features and only `test-support`, which the witness does not enable.
- The witness is an integration test target under `crates/sley-repo/tests/`;
  Cargo compiles it into no library or binary, so its framing cannot become
  a dependency of anything.

### 4.3 Why it cannot bypass validation

- Pack: `import_conformance_pack` is literally
  `preflight_conformance_pack(input, verifier)?` then
  `promote_pack_objects(store, &preflighted.decoded.objects, verifier)?`
  (`lib.rs:419-421`). `PreflightedPack` (`:471`), `preflight_conformance_pack`
  (`:480`), and `promote_pack_objects` (`:527`) are `pub(crate)`. A caller
  outside the crate cannot construct a `PreflightedPack` or reach step 6
  without steps 1-5 (envelope, payload, profile, registry lookup and decode,
  epochs, digest tree, root import and identity, dependency and object
  closure, per-object canonical verification).
- Exchange: `import_repository_exchange` calls the private `preflight`
  (`exchange.rs:1358`, steps 1-6 including the embedded pack's full
  `preflight_conformance_pack`, digest tree, receipt import, closure rules,
  branch verification, surplus check, topological order, and the four
  preflight work counters) before `classify_target` and before the first
  write. Persistence then runs only under
  `acquire_exclusive_repository_maintenance_nonblocking(target)` and uses
  the `sley-txn` clone API, which itself re-verifies: `validate_exclusive_maintenance`,
  `require_incomplete_clone` (the stage-marker write guard), receipt decode,
  `verify_transaction_relationship`, `load_objects` against the durable
  store, `verify_manifest_lengths`, `validate_inventory`
  (`repository.rs:2493-2565`); the head is written last and only after every
  ancestor receipt is durable (`:2582`). So even a composition layer that
  acquired its own guard would be re-running the same verifications, not
  skipping them.
- Nothing consumes an `AcceptedRepositoryPack`, `AcceptedRepositoryExchange`,
  `ImportReport`, `ExchangePreflightReport`, or `ExchangeImportReport` as
  input to persistence or execution (grep over `crates/*/src`: no function
  outside test modules takes any of them by parameter). They are outputs,
  not grants. Transport success therefore cannot substitute for a Sley
  check, matching amendment sections 5 and 11.

### 4.4 Identity preservation (ZR-06, section 7)

The witness proves exact recovery by bytes, not by digest: `assert_eq!(rebuilt.payload,
native)` under four segmentations x two profiles (lines 607, 649, 703,
727, 767, 804). Accepted facts are compared as typed values from the
production importer: `pack_id`, `roots: Vec<AcceptedStateRoot>` (includes
`stored_bytes`), `promoted_objects`, `present_objects` (623-626);
`ExchangePreflightReport` whole-struct equality (653); exchange
`exchange_id`, head `transaction_id`, receipts, branches, object counts
(669-683); and `tree_snapshot`, a `BTreeMap<relative path, Vec<u8>>` of
every regular file under two different destination paths (629-631, 687,
741-743), which is a byte-exact tree comparison. Round trips through the
production exporters (`again.stored_bytes == exported.stored_bytes` at 716,
`re_exported.stored_bytes == exported.stored_bytes` and equal
`exchange_id` at 746-747) close the loop. The amendment's distinction
between identities is honored: `RepositoryPackId`/`RepositoryExchangeId`
are recomputed by `decode_envelope` from the bytes, `StateRoot` by
`import_state_root`, `TransactionId`/`ReceiptId` by
`import_transaction_receipt`; the outer BLAKE3 finalizer is checked only by
the helper and never reaches the importer (ZT-10 test at 993-1015).

### 4.5 Resource-boundary map (closeout section 6) against the code

| Closeout row | Code | Verdict |
|---|---|---|
| Pack stored bytes 67,108,864 at `decode_envelope` entry, payload claim bounded by `read_len(MAX_PACK_BYTES)` | `lib.rs:690` `input.len() > MAX_PACK_BYTES`; `:707` `read_len(MAX_PACK_BYTES)`; `read_len` at `:1159` rejects before `take_exact` | accurate |
| Pack expanded bytes and allocation budget at `check_expanded_bytes` (`:1026`, `:1038`) | `:1038` `total > MAX_EXPANDED_BYTES || total > MAX_PACK_ALLOCATION` | accurate as to place and code. Observation: `MAX_EXPANDED_BYTES == MAX_PACK_BYTES`, so under profile 0 the expanded total can never exceed the stored ceiling and the 134,217,728 arm is dominated; both are contract-frozen values, so this is a note, not a finding |
| Pack epochs/roots/objects/leaves at `check_counts` (`:1014`) | `check_counts` enforces the upper bounds and the non-zero rules; the first binding point is earlier: `decode_list(input, MAX_PACK_*)` (`:1087-1093`) rejects `count > maximum || count > remaining` before `Vec::with_capacity(count)`, and `decode_tree` bounds leaves the same way (`:825`) | values and code correct; the map names the second gate rather than the first; both exist |
| Pack compression profile 0 at `decode_payload` before block interpretation, and `validate_profile` | `:741-743` in `decode_payload` (before epochs/roots/objects are decoded); `validate_profile` `:838` | accurate |
| Exchange stored bytes at `decode_envelope` (`:646`) | `:646` | accurate |
| Embedded pack 16,777,216 at `:800` (import) and `:1004` (export) | exact lines | accurate |
| Exchange receipts/branches/leaves before allocation | `decode_list(.., MAX_EXCHANGE_RECEIPTS)` `:682`, `.., MAX_EXCHANGE_BRANCHES` `:731`, `.., MAX_EXCHANGE_LEAVES` `:769` via the same count-before-capacity rule | accurate |
| Exchange allocation budget shared top-down at `:835` | `:820-835` sums the embedded pack length plus receipt and branch bytes and rejects over `MAX_EXCHANGE_ALLOCATION` before `preflight_conformance_pack` runs its own ceilings on the embedded bytes | accurate; "shared" is realized by charging the whole embedded pack against the exchange budget, then letting the pack decoder's independent ceilings also apply, which is strictly tighter, not a reset |
| Preflight work ceilings at `:1337` and siblings | `verify_receipts_against_pack` `:1316-1345`: single `u64` counters over the whole preflight for receipt bytes, binding visits, distinct object verifications, object bytes | accurate |
| Exchange profile 0 at `:804` | `:803-804` | accurate |
| Nested accounting proven by the maximal-legal-exchange decode test | `decode_limits_bind_before_allocation_and_a_maximal_shape_decodes` `:2649` | present |

No limit is weakened, multiplied, or reset. Finding under existing owners:
none, agreeing with the closeout.

### 4.6 Witness authenticity and test-only framing

- Every accepted fact in the witness comes from `import_conformance_pack`,
  `preflight_repository_exchange`, `import_repository_exchange`,
  `export_conformance_pack`, or `export_repository_exchange` (imports at
  lines 56-60). No alternate validator, no mock decoder, no construction of
  an accepted root, pack, receipt, or graph outside those functions.
  Sources are built only through production authorities
  (`StateRootBuilder`, `TransactionRepository::initialize_trusted_genesis`,
  `commit`, `BranchRepository`).
- Framing: leader `ZR12-TEST-FRAME/NOT-FMT!` (shares no prefix with
  `SLEYSCB1`), big-endian fixed-width lengths, BLAKE3 finalizer, local
  `TransportFailure` enum. Not SCB1, no `sley2.` domain, no contract tag,
  no MIME, no error symbol registered (`check_error_symbol_registration.py`
  PASS with `unregistered: []`). It is under `tests/`, so it cannot leak
  into any library, binary, or feature; the anti-goal `--check` scan passes.
  The module doc says it must never be promoted. Mistaking it for a format
  would require someone to copy the file's private functions into `src/`,
  which the dependency law, the error-symbol registration gate, and the
  contract spec checkers would each catch. Risk judged negligible.
- Verifier context: exchanges (and the packs embedded in them) run under
  the production `RepositoryObjectVerifier`; the standalone pack tests use
  the S20-100 fixture-contract verifier because the pinned pack fixture's
  objects are synthetic fixtures. Acceptable and consistent with the
  crate's own pack tests; noted, not actionable.

### 4.7 Documentation-and-test-only claim

`git diff --stat 4168332 161a2f3` restricted to `crates/*/src`, all
`Cargo.toml`, `Cargo.lock`, `docs/spec`, `conformance`, `oracle`, and
`scripts` is empty. The Cargo.lock digest in `baseline.txt` matches. The
`non-effect.txt` binary hash comparison is the owner's record; I did not
rebuild `sley-cli`, but with no source, manifest, or lock change the
executable non-effect follows from Cargo's fingerprinting. Claim holds.

## 5. Per-item analysis

- Amendment section 5 (ZR-04): satisfied by 4.1-4.3. Dependency direction
  `sley-repo -> sley-txn -> sley-store` unchanged; no `run(graph)` shortcut.
- Section 7 (ZR-06): satisfied by 4.4; exact byte and tree comparisons, no
  digest-only equality.
- Section 8 (ZR-07): all three insertion points take `&[u8]` bounded at
  `decode_envelope`; no streaming claimed; helper short-read/EOF/I-O rows are
  correctly labeled helper-only in the closeout (ZT-08).
- Section 11 (ZR-10): preflight/persistence split verified in code; the
  witness proves no-write on every corpus rejection from both routes and
  after three late outer failures; owning symbols preserved (16 rows).
- Section 12 (ZR-11): map accurate with the two precision notes in 4.5.
- Section 13 (ZR-12): production importers and exporters exercised, both
  profiles, pinned fixtures plus exporters, malformed corpus, late outer
  failure. Satisfied.
- Section 4 prohibited expansion: none observed (no dependency, format,
  magic, MIME, domain, command, registry, plugin loading, public error
  code, contract, limit, or fixture change).
- Clean-room redaction: the only edit to the operator's text (diff shows
  one line).
- Release-impact map and separate GA disposition (sections 8, 12 of the
  closeout): stated honestly; candidate preserved, staleness acknowledged,
  BLOCKED unchanged, no records-only exemption claimed.
- Requirement and validation matrices: evidenced by production symbols and
  executable tests for ZR-03..ZR-13 and ZT-01..ZT-11, ZT-13. **ZT-12 is not
  accurate at scope**: the closeout says the individually run quick steps
  pass apart from the two pre-existing baselines and the two drifts it
  repaired, but the commit itself introduced a new clean-room violation by
  tracking `make-quick.log` with the legacy artifact digest in it, so
  `scripts/check_clean_room_boundary.py` (a `make quick` step) fails at
  `161a2f3`. This is an evidence-hygiene regression with no production or
  contract effect and a one-line remedy, but it is a mandatory gate and the
  row misreports it; P2.
- ZT-14 / machine summary: `witness_commit` is `PENDING_COMMIT` at scope
  and the section carries no reviewed scope SHA; the row says the section
  "names the witness commit". The note anticipates a records-only follow-up.
  Binding is currently achieved only by the transcript filenames. P4.
- Closeout section 3 wording "produced only by the owning functions":
  `AcceptedRepositoryPack` is also returned by the pre-existing, public,
  non-feature-gated `seal_mutated_conformance_pack_for_testing`
  (`lib.rs:461`, S20-700 fuzzer hook) from caller-supplied entries, and all
  four result structs have public fields. The property that actually blocks
  a bypass is the one in 4.3: none of these types is consumed by any
  persistence or execution API. The sentence should say that. P4.

## 6. Findings

- [P2] [evidence] `evidence/validation/zjx-transport-readiness-logs-v1/make-quick.log:823` carries a clean-room sentinel (the S20-600 legacy artifact digest printed by `check_legacy_runner.py`); `scripts/check_clean_room_boundary.py` FAILs at `161a2f3` with `clean-room-violation:legacy-reference:...make-quick.log`, and closeout section 9 row ZT-12 claims the quick steps pass except pre-existing baselines. Remedy: redact that line from the tracked log (or route the log through the S20-780 inventory rule the owner prefers), re-run the checker, and correct the ZT-12 row and `make-quick-remaining-steps.log` reference.
- [P4] [record] `docs/audits/S20_ZJX_TRANSPORT_READINESS_CLOSEOUT.md` section 9 row ZT-14 and `machineresearch/sley-2.0/machine-summary.json` `zjx_transport_readiness.witness_commit`: the row says the section names the witness commit; at scope the field is `PENDING_COMMIT` and no reviewed scope SHA is recorded. Pin `161a2f3480dfca3df22019a9df4e3e3e4a38674f` in the records-only follow-up and word the row as "pinned by the follow-up".
- [P4] [record] `docs/audits/S20_ZJX_TRANSPORT_READINESS_CLOSEOUT.md` section 3, "produced only by the owning functions": `AcceptedRepositoryPack` is also produced by the public `seal_mutated_conformance_pack_for_testing` (`crates/sley-repo/src/lib.rs:461`) and the report structs are publicly constructible; restate the barrier as "no persistence- or execution-capable API accepts these types; only `&[u8]` enters the importers and `PreflightedPack`/`promote_pack_objects` are crate-private".

Non-actionable observations (prose only): the pack allocation-budget arm at
`lib.rs:1038` is dominated by `MAX_EXPANDED_BYTES` under profile 0; the
first count gate is `decode_list` rather than `check_counts`; there is no
public no-write preflight for standalone packs (only for exchanges), which
the amendment does not require.

```
VERDICT: REVISE_0_P0_0_P1_1_P2_0_P3_2_P4
SECTION: zjx_transport_readiness
FIELD: nabu_architecture_review
SCOPE_SHA: 161a2f3480dfca3df22019a9df4e3e3e4a38674f
FINDINGS:
[P2] [evidence] evidence/validation/zjx-transport-readiness-logs-v1/make-quick.log:823 - tracked quick log carries the S20-600 legacy artifact digest sentinel; scripts/check_clean_room_boundary.py FAILs at 161a2f3 (clean-room-violation:legacy-reference), contradicting closeout section 9 row ZT-12; redact the line, re-run, correct the row
[P4] [record] docs/audits/S20_ZJX_TRANSPORT_READINESS_CLOSEOUT.md:ZT-14 - row says the machine-summary section names the witness commit; zjx_transport_readiness.witness_commit is PENDING_COMMIT at scope and no reviewed scope SHA is recorded; pin 161a2f3480dfca3df22019a9df4e3e3e4a38674f in the records-only follow-up
[P4] [record] docs/audits/S20_ZJX_TRANSPORT_READINESS_CLOSEOUT.md:section 3 - "produced only by the owning functions" is inexact (public seal_mutated_conformance_pack_for_testing at crates/sley-repo/src/lib.rs:461 returns AcceptedRepositoryPack; report structs are publicly constructible); restate the barrier as no persistence/execution API consuming these types
SUMMARY: The architecture claim is sound and proven from the code: the future adapter inserts at import_conformance_pack (lib.rs:413), preflight_repository_exchange (exchange.rs:1439), and import_repository_exchange (exchange.rs:1885) with bytes from export_conformance_pack (lib.rs:343) and export_repository_exchange (exchange.rs:954); a composition layer on those symbols sits after sley-repo in the ARCHITECTURE.md/ADR-0003 order, nothing in the 27-crate locked closure or any kernel crate depends on it, and it cannot bypass validation because only &[u8] enters the importers, PreflightedPack/promote_pack_objects are crate-private, and the sley-txn clone API re-verifies every receipt under the exclusive maintenance guard and the incomplete-clone marker. The witness (10/10, re-run here) exercises the production importers and exporters with exact byte and byte-exact tree comparisons, its framing is test-only and unregistrable, and no crates/*/src, Cargo.toml, Cargo.lock, contract, fixture, or script changed. One P2 blocks a bare PASS: the committed make-quick.log carries a legacy-digest sentinel so the clean-room gate fails at scope while ZT-12 reports it passing; two P4 record edits follow.
```
