# R2 current-source execution and review evidence

The R2 exit checker runs RW060 lifecycle tests and the RW075 raw-callable,
execution-closure, hydration-workload and admission-authority tests. A summary
status or a frozen identifier alone cannot satisfy execution evidence. The
checker requires successful completion, named behavioral tests and the actual
emitted lifecycle identifiers. Expected-panic tests count only when libtest
reports success. Filtered runs are admitted only for the two deliberately
selected library groups named in `LIB_SUITES`: the admission-authority unit
group and the Sley-owned AR-05 replay
(`bootstrap_closure::closure_workloads_replay_through_v2_with_attribution`,
bound at 7426bc0b). Every suite runs under a 180-second timeout that includes
compilation, so the gate presupposes a warm build of the `sley-repo` and
`sley-vm` test targets (`cargo test -p sley-repo --test rw060_source_free_lifecycle --no-run`
and the `rw075_*` targets); a cold build reports `lifecycle-command-failed`
rather than a false pass.

`scripts/r2_execution_evidence.py` binds the inventory of crate inputs, Cargo
configuration and lock/toolchain files, all conformance and specification files,
Python gate scripts, the Makefile, host boundary and runtime anti-goal evidence.
The inventory includes untracked source additions and refuses missing tracked
inputs. Before and after each native suite, and again after the complete gate,
the source digest must match. Evidence and review transcripts are excluded
except for the explicitly consumed anti-goal document, avoiding circular review
identities. This is local execution evidence, not a signed external attestation.

The digest binds the working tree, not a commit. The gate therefore prints
`R2_SOURCE_SHA256`, `R2_SOURCE_HEAD` (`git rev-parse HEAD`) and
`R2_SOURCE_TREE` (`clean`, or `dirty` with the porcelain entries of the
source inventory) and fails the predicate `R2_source_tree_clean` when any
tracked or untracked source-inventory entry is modified, added, deleted or
renamed; changes outside the inventory (evidence, summaries) do not affect it.

Current RW075 Ariadne, Nabu and premium transcripts must each contain exactly
one line `SOURCE_R2_SHA256: <64 lowercase hexadecimal characters>` matching
the current source digest, and must record the same two facts the gate
prints: the HEAD commit the digest was taken at and that the source
inventory was clean (`R2_SOURCE_TREE: clean`). A transcript whose digest was
taken on a dirty tree binds no commit and does not qualify, whatever its
verdict. The gate reports the digest in its execution detail.
The latest round is still authoritative; an unbound newer round cannot fall
back to an older bound PASS. Existing exact verdict-token and within-file FAIL
precedence rules remain in force. Historical RW070 reviews continue to refer to
their separately checked frozen profile identities.

The September 15 correction passed independent implementation review and all
live native suites. The aggregate deliberately remained `NOT_READY` because
current qualifying review transcripts were absent. The original architecture
FAIL and all historical transcripts remain preserved. See
[`r2-binding-correction-2026-09-15.md`](../../evidence/review/r2-binding-correction-2026-09-15.md)
for review scope and executed checks.
