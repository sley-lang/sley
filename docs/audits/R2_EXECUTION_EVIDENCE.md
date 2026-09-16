# R2 current-source execution and review evidence

The R2 exit checker runs RW060 lifecycle tests and the RW075 raw-callable,
execution-closure, hydration-workload and admission-authority tests. A summary
status or a frozen identifier alone cannot satisfy execution evidence. The
checker requires successful completion, named behavioral tests and the actual
emitted lifecycle identifiers. Expected-panic tests count only when libtest
reports success. Filtered tests are admitted only for the deliberately selected
admission-authority unit group.

`scripts/r2_execution_evidence.py` binds the inventory of crate inputs, Cargo
configuration and lock/toolchain files, all conformance and specification files,
Python gate scripts, the Makefile, host boundary and runtime anti-goal evidence.
The inventory includes untracked source additions and refuses missing tracked
inputs. Before and after each native suite, and again after the complete gate,
the source digest must match. Evidence and review transcripts are excluded
except for the explicitly consumed anti-goal document, avoiding circular review
identities. This is local execution evidence, not a signed external attestation.

Current RW075 Ariadne, Nabu and premium transcripts must each contain exactly
one line `SOURCE_R2_SHA256: <64 lowercase hexadecimal characters>` matching
the current source digest. The gate reports that digest in its execution detail.
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
