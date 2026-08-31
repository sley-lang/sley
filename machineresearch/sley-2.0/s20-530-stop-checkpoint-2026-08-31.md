# S20-530 stop checkpoint

Status: PAUSED AT GOVERNED CLOSEOUT BOUNDARY

Owner: Codex orchestrator

Checkpoint time: 2026-08-31T19:19:19-04:00

## Repository state

- Repository: `/home/greyforge/sley2`
- Branch: `main`
- V9 archive-mode repair candidate commit:
  `3b1f1906643d23a518d29b4274d465debe75494f`
- Last authoritative v8-bound plan commit:
  `9068adc9b1c8b3035295649a592fa0725589a240`
- Nothing was pushed, deployed, published, or executed against an external
  runtime.
- All S20-530 Forge-agent review processes were stopped before this checkpoint.

## What is complete

The v8 closeout failure was reduced to a deterministic Git archive mode defect.
Default isolated `git archive` emitted regular and executable modes `0664` and
`0775`, while the validated Git tree requires `0644` and `0755`. The v9
candidate fixes only that boundary by invoking:

```text
/usr/bin/git -c tar.umask=0022 archive --format=tar <validated-commit>
```

The exact ordered arguments are bound into the execution profile. Hostile
controls reject omitted configuration, `tar.umask=0002`, reordered arguments,
and a changed archive format. Extracted bytes and modes must still equal the
validated commit descriptors; no post-extraction mode normalization is used.
The immutable v8 evidence was not changed.

## Validation captured before stopping

Validation tier: targeted Tier 2 contract-refreeze checks. The full release
gate was not run.

- Full v9 contract and hostile-control corpus: PASS.
- Runner self-test, including real commit materialization: PASS.
- Matrix compiler: PASS, 100 rows and 419 mapped tests;
  SHA-256 `3cd961d37db98862f72186038230db7cf6a3a3659839f2ce23e2475ec47b8c1d`.
- Exception reconciliation: PASS; entry `110 = 90 + 20`, control
  `5213 = 3547 + 1666`.
- Ruff formatting and lint: PASS.
- JSON parsing, `git diff --check`, and immutable-v8 comparison: PASS.
- Full `make v1`: skipped because this is not a release boundary.

## Explicitly skipped

Fresh v9 Nabu, Ariadne, and Vulcan freeze receipts were not obtained. The Nabu
lane timed out with exit 124. The remaining review processes were interrupted
after the operator directed Codex to stop using the failing Forge-agent path.
Those attempts are non-authoritative and no receipt from them was inserted.

Consequently, the v9 evidence remains a locally validated candidate rather
than an authoritative reviewed freeze. The machine summary continues to point
to the last authoritative v8 freeze. The v9-bound 12 MB test plan was not
rebuilt, the long captured closeout runner was not repeated, and implementation
receipts were not requested.

## Exact resume point

If S20-530 is resumed, start from
`3b1f1906643d23a518d29b4274d465debe75494f` and choose one explicit governance
path before further execution:

1. obtain fresh trusted v9 freeze receipts through a working review surface; or
2. approve and document a contract amendment that replaces the specialist
   receipt requirement with a local/operator-reviewed gate.

Then rebuild the test plan against contract set
`70c8b69e52817a42766b59fa857e475141c2116a1295fb67f53dcbfcf44a05a8`, run
the captured closeout once, collect or explicitly waive implementation
receipts, and update the machine summary. Do not reuse the timed-out sessions.

## Performance follow-up

Before another long closeout, split independent deterministic checker controls
into bounded worker shards and merge their results in stable contract order.
Cap workers below the host logical-CPU count, preserve per-shard logs and exit
statuses, and keep a serial reference mode for equivalence checks. Do not
change the frozen v9 contract merely to add concurrency; make this a separately
reviewed validation-harness change.

## Completion estimate at stop

- S20-530 governed closeout: 97%, high confidence.
- Overall Sley 2.0 roadmap: 51%, moderate confidence.

The active development run stops at this checkpoint. No next work package is
authorized by this record.
