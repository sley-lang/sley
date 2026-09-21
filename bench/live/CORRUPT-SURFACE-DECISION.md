# CORRUPT original obligation — explicit surface decision (review input)

Status 2026-09-21 (wt-succ branch). The exchange-layer regression is
implemented and green; the corpus's PACK-layer obligation is retained
open with the exact missing operation specified. This packet is the
decision the reviewer needs — not a silent substitution.

## Corpus obligation (S2B-CORRUPT-001, frozen)

- Alter one canonical object byte in an exchange pack, attempt
  import, reject with the exact failure code PACK_DIGEST_MISMATCH,
  destination ref advances 0, old root stays accepted,
  deterministic recovery.

## What exists and is green

- Trial-surface path (`_judge_corrupt_exchange`): two single-bit
  corruptions (middle, last byte) of the staged pack → attempt
  `exchange.import` → exact EXCHANGE_DIGEST_MISMATCH, destination
  head tx and live object count identical before/after
  (`trial_corrupt_recheck.log`). Regression + neg
  (ORACLE_CORRUPT_UNRESTORED) rechecked 2026-09-21.
- Library-layer pin: `import_conformance_pack` on a tampered
  conformance pack → exact PACK_DIGEST_MISMATCH before promotion
  (frozen S3 G2 `outer_digest_tamper_fails_before_promotion`;
  re-verified green this slice). Never equated with the exchange
  layer.

## Why the corpus path is not drivable today

- The serve protocol exposes exactly one import method:
  `exchange.import` → `import_repository_exchange` (exchange owner).
  No protocol method reaches `import_conformance_pack`
  (repository-bundle import owner).
- The trial tool allowlist (`TOOL_METHODS`, pinned equal to the
  smoke runner's allowlist) exposes no import/merge/commit/export
  operation by construction ("Commit, merge, execute, export,
  import, report, session management, and tests stay outside the
  agent's reach" — `bench/live/sley2_tool.py`). The mediated
  gateway (`ALLOWED_COMMANDS`) mirrors the same surface.
- A judge-side direct drive of `import_conformance_pack` would be a
  judge self-test, not the required attempt-bound interaction (the
  trial agent attempting the import through its surface). Deliberately
  not implemented.

## Decision requested

One of:

1. **Add a trial-surface bundle-import operation** (new TOOL_METHOD
   routing to `import_conformance_pack` semantics against a staged
   pack, with the exact PACK_DIGEST_MISMATCH + ref-unchanged
   judgment). Touches the frozen tooling contract + method table:
   needs fixture/tooling review before implementation or adoption.
2. **Adopt the exchange-layer path as the task's operationalization**
   (rule that EXCHANGE_DIGEST_MISMATCH on the trial surface +
   library-pinned PACK_DIGEST_MISMATCH satisfies the corpus
   requirement). Needs owner-contract review; this packet + the
   green S3 pin are the evidence.
3. **Defer with the obligation explicitly open** (current state):
   no acceptance is claimed for the PACK layer; the exchange
   regression stands as implemented (not as substitution).

Until the review decides, the CORRUPT record claims only the
exchange-layer regression plus the retained-open PACK obligation.
`ga_claimed=false`.
