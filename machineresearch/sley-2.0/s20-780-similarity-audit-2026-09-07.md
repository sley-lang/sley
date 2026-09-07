# S20-780 similarity/provenance audit — performed 2026-09-07 (local evidence, independent review pending)

Authority: `docs/spec/CLEAN_ROOM_DISPOSITION_REGISTER_V1.md` §3 gap,
`machine-summary.json` `remaining_gates.similarity_audit`
("Performing and recording the audit flips `similarity_audit_performed`
without failing the gate"). Scope is the audit the boundary checker cannot
mechanize: wholesale-copy detection beyond sentinels, dependencies,
touchpoints, and register hygiene. This record does not close the GA
clean-room claim (GA acceptance is release-package owned) and is not an
independent review.

## 1. Corpora (provenance anchor verified first)

- Legacy source: `/home/greyforge/archive/sley/1.2.0/sley-1.2.0-source-397fa28.tar.gz`
  (4,534,056 bytes). SHA-256 `1c866d360305d0b511dc2c33c4907b33544fc73bc6cb6fa4c0e1687df48eb90e`
  matches register §1.2 exactly. Unpacked to `/tmp` only; never into the
  repository; no `sley-1.2.0-source*` path tracked (checker still asserts this).
- Legacy inventory (1073 files, ~14 MiB): 392 `.sley`, 388 `.json`, 178 `.md`,
  58 shell (`bin/` + `scripts/`), 6 Python (`clients/`), 3 TS, TOML/YAML,
  **zero Rust files**. Implementation language of the predecessor corpus is
  `.sley`/shell; the Sley 2 implementation corpus is Rust (102 tracked `.rs`).
- Tree side: 961 tracked files at `f5fd457` (clean worktree; tracked == tree).

## 2. Method (all local, read-only, rerunnable)

Script: `/tmp/opencode/sley12audit/analyze.py` (session scratch, not tracked);
machine report: `/tmp/opencode/sley12audit/report.json` (scratch; figures
below are its output).

- A. Exact bytes: SHA-256 of every legacy file vs every tracked tree file.
- B. Same-basename text pairs: normalized lines (strip blanks, `#` comments,
  collapse whitespace), difflib ratio.
- C. Python cross: all 6 legacy `.py` vs all 204 tree `.py`, normalized-line
  ratio over all pairs; flag ratio >= 0.30 or any common block >= 10 lines.
- D. Shell cross: all 58 legacy shell files vs tracked tree `.sh`.
- E. Marker tokens: distinctive identifiers (>= 12 chars, stop-list filtered)
  from legacy code scanned against tree `*.py`/`*.rs`/`*.sh` via `git grep`;
  every hit adjudicated in §4.
- F. Embedded implementation: every normalized 5-line window of all 392
  legacy `.sley` files probed against the index of all tree text files
  (<= 500 KB); any hit reported.

## 3. Results

- A: **0 exact byte matches** (1073 legacy × 961 tracked, including
  `LICENSE`/`NOTICE`/`README.md`/`Makefile` — even those differ byte-wise).
- B: 122 same-basename pairs; maximum ratio **0.100** (`.gitignore`),
  READMEs/`ARCHITECTURE.md`/`Makefile` all <= 0.025 — generic-English level.
- C: **no pair >= 0.30, no common block >= 10 lines** across 6 × 204 files.
- D: tree tracks **zero `.sh` files** — the 58-file legacy shell corpus has
  no counterpart by construction.
- F: **0 embedded `.sley` windows** in any indexed tree text file; the tree
  tracks zero `.sley` files (`git ls-files '*.sley'` empty), so the entire
  392-file legacy implementation corpus has no tree counterpart.
- E: 317 raw token hits; all adjudicated in §4. No hit survives as copying
  evidence.

## 4. Adjudication of every distinctive shared token

- `SLEY_DISABLE_SOURCE_CACHE`, `SLEY_SOURCE_CACHE_DIR`, `sley_version`,
  `tracked_release_payload_excluding_release_metadata`, `expected_sley_digest`:
  tree occurrences are confined to `bench/legacy/runner.py`, its tests, and
  `scripts/check_legacy_runner.py` — the single authorized out-of-process
  touchpoint (register §1.1/§2.3). An adapter staging the frozen binary must
  name its env vars and parse its manifest keys; that is the dispositioned
  touchpoint, not source copying. Touchpoint inventory unchanged (still
  exactly the adapter + tests + boundary scripts).
- `CONTROL_MISMATCH`, `EVIDENCE_INVALID`, `DEPENDENCY_MISSING`,
  `DRY_RUN_REQUIRED`, `REQUIRED_METRICS`, `MAX_RECORD_BYTES`,
  `MAX_RESPONSE_BYTES`, `REFERENCE_MISMATCH`, `transaction_id_hex`:
  convergent generic compounds in independently-designed schemes — tree uses
  namespaced Rust enums with different values (`RawErrorCode::CONTROL_MISMATCH
  = 61_001`, `ProvenanceErrorCode::EVIDENCE_INVALID = 74005`,
  `GC_DEPENDENCY_MISSING`, `GC_DRY_RUN_REQUIRED`, `AUTHORITY_REFERENCE_MISMATCH`),
  legacy uses bare shell-string diagnostics and fixture keys with different
  meanings (`REFERENCE_MISMATCH` is a legacy shell filename variable).
- `REQUIRED_METRICS` (`check_benchmark_baseline.py`) reads the shared
  succession plan metric set — authorized shared corpus vocabulary (register
  §1.3: the corpus is shared by design; legacy evidence may inform fixtures,
  never semantics).
- Remaining ~300 hits are standard-library/programming vocabulary
  (`ArgumentParser`, `AssertionError`, `JSONDecodeError`, `expected_digest`,
  `manifest_path`, SPDX spec terms) shared by any two projects in this domain.

## 5. Provenance spot-checks (identity/hash schemes)

- Legacy source contains **zero mentions of BLAKE3**; the Sley 2 identity
  system (`EntityId`/`ObjectId`/`StateRoot`, `RAW_BLAKE3_V1`, `SLEYPOBS1`,
  domain-separated digests) has no legacy antecedent in-tree to derive from.
- Legacy "content address / domain separat*" mentions are fixture-contract
  vocabulary only, not an identity scheme.

## 6. Verdict and limits

- Verdict: **no wholesale copying, no copied implementation, no copied
  module structure found**. The predecessor implementation corpus (`.sley` +
  shell, zero Rust) and the Sley 2 implementation corpus (Rust + Python
  oracles) are disjoint by language; Python shows no near-match; every
  distinctive shared string traces to the authorized touchpoint, the shared
  corpus, or convergent naming. The authorized touchpoint remains exactly the
  dispositioned inventory.
- Limits (both directions): the method catches exact and near matches; a
  fully rewritten paraphrase of an idea is out of scope — ideas are governed
  by the register's three-part reimplementation test (§1, entries 1.4+),
  not by text similarity. The frozen binary artifact was not compared (binary
  arm, not source). Documentation overlap was measured at generic-English
  level only.
- Status: local implementation-session evidence at `f5fd457`-based tree;
  independent (Argus/Council) review of this audit is still pending; the GA
  clean-room claim stays gated on that review plus the release package.
