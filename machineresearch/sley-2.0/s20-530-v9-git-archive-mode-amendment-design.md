# S20-530 v9 Git archive mode amendment

Status: candidate contract amendment

## Problem

The v8 closeout runner bound workspace inputs to Git-tree descriptors with
regular modes `0644` and executable modes `0755`. Its per-command source
snapshot used default `git archive`, whose tar backend applies
`tar.umask=0002` and emits `0664` and `0775`. The runner copied those member
modes exactly and then rejected its own materialized snapshot. V8 therefore
cannot produce authoritative closeout evidence on the pinned host without
changing the frozen execution contract.

## Decision

V9 changes only the archive invocation to the exact ordered arguments:

```text
/usr/bin/git -c tar.umask=0022 archive --format=tar <validated-commit>
```

The setting is command-scoped. It does not change the frozen local Git config
or the wider Git subprocess environment. The ordered arguments join the
execution-profile payload, and the trust-boundary identity advances from v2 to
v3. Materialization still fails unless every extracted regular-file byte and
mode descriptor equals the validated commit descriptor.

## Rejected alternatives

- Adding `tar.umask` to the shared Git environment would affect every Git
  command and enlarge the environment contract.
- Adding `tar.umask` to `.git/config` would violate the exact local-authority
  bytes and make a repository-local mutation part of closeout.
- Normalizing modes after extraction would mask archive metadata drift and add
  a second interpretation layer.

## Controls

The runner accepts only the exact command-scoped argument tuple. Self-controls
reject omitted configuration, `0002`, reordered configuration, and a changed
archive format. A real self-test materializes the validated commit and compares
the complete excluded-output-aware descriptor map. The normal per-command path
repeats that comparison before and after every captured command.

## Re-freeze scope

V9 changes the specification, ADR, checker, runner, checker self-contract,
aggregate contract-set digest, freeze evidence, and test-plan binding. The v8
freeze evidence remains byte-for-byte unchanged. The matrix, production
recovery behavior, independent reconciler, exception ledgers, and exception
partition are expected to remain identical and must be compared before v9 is
accepted. Fresh Nabu, Ariadne, and Vulcan freeze receipts are required, followed
by fresh implementation receipts after captured Tier 2 validation.
