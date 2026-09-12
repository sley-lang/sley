# ADR-0018: Frozen legacy artifact adapter

Status: accepted for the scoped S20-600 artifact/version-smoke slice

## Decision

Treat the frozen Sley 1.2.0 release-candidate archive as an external, pinned
oracle artifact. Admit it only after exact outer identity, archive-safety,
embedded authority, and per-file payload verification. Copy it into private
temporary storage, extract it manually, strip write bits from the staged payload, and
allow only the exact `bin/sley --version` smoke with bounded execution and
create-only retained evidence.

Mutable scratch paths are private to the stage. The artifact-defined source
cache may use that scratch space, but no inherited HOME, TMPDIR, cache, or
environment enters the command. The adapter does not use the live Sley 1.2
checkout and does not import legacy source or semantics into the Sley 2 kernel.

## Consequences

- S20-600 now has verified artifact, staging, failure-retention, and runner
  smoke mechanics.
- On the origin host as of 2026-09-11, two 10-second timeouts, one
  30-second timeout, and a successful longer smoke are retained as runtime
  evidence; the slow legacy startup is not hidden.
  Retained smoke records are host-local create-only files under the ignored
  `evidence/runtime/s20-600-legacy-smoke/` directory: they are reproducible
  on any host holding the pinned artifact (rerun the smoke with a short
  `--timeout-seconds` for a timeout record, the default 90 s for success),
  but they are not reproducible from the tree alone and no digest index of
  them is tracked. The register's retained-evidence claim is therefore a
  host-local claim, not a tree claim.
- The stage is mode-hardened but not mounted read-only. That is sufficient only
  for the exact trusted version command, not trial containment.
- The routine quick gate verifies artifact bytes and adversarial synthetic
  behavior without spending about one minute on the real executable smoke.
- The explicit `legacy-runner-smoke` target reruns the real command when that
  release-boundary evidence is needed.
- No benchmark trial, oracle result, fairness result, or succession metric is
  established. Full S20-600 remains pending the trial-specific surfaces named
  in the normative spec.
