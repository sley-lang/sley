# S20-730 reproducibility and conformance closeout (2026-09-05)

## Findings

| # | Reviewer | Finding | Verdict |
|---|----------|---------|---------|
| 1 | Ariadne | The reproducibility report has no integrity or freshness check: `report_digest` is never verified, no drift check runs, and no binding ties the attested commit to the filing tree; the tracked report attests `ebd40dc`, 65 commits stale, while `make quick` passes it | confirmed live; fixed |
| 2 | Nabu | Multi-host merge is not persistent: the smoke rebuilds the report with no `--attest` and never reads the tracked report, so a committed `MULTI_HOST_REPRODUCIBLE` silently reverts to `SINGLE_HOST_REPRODUCIBLE` at exit 0 | confirmed structurally; fixed |
| 3 | Nabu | Coverage taxonomy has no depth axis: `independent_oracle` flattens semantic re-derivation and codec-and-identity checks, so `INDEPENDENT_CONFORMANCE_COMPLETE` hides the acknowledged VM-extended semantic gap | confirmed; fixed with a declared depth axis |
| 4 | Vulcan | The report attests `ebd40dc`, 65 commits behind `HEAD` with 15 `crates/` files changed since; no freshness rule in the contract and no staleness check in the checker | confirmed live; fixed |
| 5 | Vulcan | The report is never re-derived or digest-verified: the checker runs `--check` only on the conformance report, `report_digest` is written but read by nothing, so a hand-edited `MULTI_HOST` report with a fabricated attestation passes `make quick` | confirmed live; fixed |

## Fixes

- Integrity (Ariadne-1, Vulcan-5): `verify_report()` recomputes the
  digest over the report body and validates every attestation against the
  section 1 shape, reading only the tracked file, so it runs hermetically
  on a clean checkout with no gitignored local evidence. The checker runs
  it on every `make quick`; a hand-edited result or a malformed
  attestation fails closed. The builder's own `--check` is deliberately
  *not* in the checker: it re-derives from the gitignored S20-720
  evidence record and would fail-open-or-fail on a clean tree.
- Freshness and binding (Ariadne-1, Vulcan-4): the contract requires every
  attested commit to be an ancestor of the filing `HEAD`, and no tracked
  file under the artifact input surface
  (`build_release_candidate.ARTIFACT_INPUT_PATHS`: workspace, toolchain
  pin, demo runner, SBOM inventory, conformance subset, packaging script)
  to differ between the attested commit and `HEAD`. The attested toolchain
  must match the filing toolchain. The cure for a stale report is the
  smoke, not an edit. Freshness is a surface diff rather than a commit
  count: a count bound would be arbitrary, while a changed surface file
  means the attested artifact is provably not what this tree builds.
- Persistence (Nabu-2): a rebuild carries the tracked report's
  attestations forward — the fresh local attestation wins its label, an
  explicit `--attest` file wins its label, everything else is
  re-validated and merged. A tracked file that is not a report, or that
  carries a malformed attestation, fails closed. `--check` assembles the
  same inputs, so a carried report does not read as drift.
- Depth (Nabu-3): every independent family declares `semantic` or
  `codec_and_identity` in a `DEPTH` map the builder enforces, each family
  record carries its depth, and the report carries the `coverage_depths`
  roll-up the checker re-derives. Four families recompute outcomes with
  independent logic (merge judgment, semantic deltas, entity-impact
  closure, root-backed-query results); the other fifteen reconstruct
  encodings, identities, or digest trees. The `COMPLETE` rule is
  unchanged, and the contract now states what it does not promise:
  independent semantic judgment stays with the owner crates by design
  (master goal 6.5), so the depth axis keeps a codec-only `COMPLETE` from
  reading as a semantically judged one. The stale section 9 exclusion
  (extended VM vectors "stay native-only") is corrected.

## Second-host lane record

An attestation is unsigned and carries no host binding by design (contract
section 1, ADR-0040 decision 5), so the report cannot distinguish a genuine
second-host reproduction from a same-host relabel; operator custody of the
transfer is the trust root (contract section 5.1). This record makes each
exercise of the lane auditable without putting identity in the report: one
row per lab mint, verified facts only.

| Candidate commit | Date (local) | Transport lane | Lab checkout | Lab candidate `evidence.json` sha256 | Artifact sha256 | Transferred attestation-file sha256 | Bundle commit | Merge commit |
|---|---|---|---|---|---|---|---|---|
| `7a94a4a31272a6dc7588aff902dcde81f7d10a4e` | 2026-09-14 (lab checkout created 23:58) | `forge-lab-connect` (`ssh greyforgelab`) | `~/sley2-repro-7a94a4a`, detached at `7a94a4a31272a6dc7588aff902dcde81f7d10a4e` | `00b28ee070b7139ea85e33f088d312ebf2a120e941dba6df305792bf56172842` | `6d970bf4d5da78b034a837109348e3d1e373c6566cfae67a4fd84b447160e159` | not retained (the file was consumed by the `--attest` merge before this record existed) | not recorded for this mint | `6a2eef7` (label `secondary`, `MULTI_HOST_REPRODUCIBLE`) |

The next mint fills its row completely: the operator records the
attestation-file sha256 and the bundle commit before running the `--attest`
merge on the primary, then adds the merge commit.

## Reconciliation

No re-review was needed: these fixes change mechanics and reported facts
without contradicting any recorded disposition. The round verdicts stand
as `FAIL` history; the five P0s close by fix, recorded in the machine
summary.

## Validation

Offline tests pass (22 reproducibility/conformance tests, including
carry-forward merge, tamper detection, and depth pins). Tier 1
(`make quick`, `make lint`) and Tier 2 (`make core`, `make conformance`,
`make adversarial`, `make fuzz-smoke`) pass at the evidence commit, and
the clean `release-candidate-smoke` re-attestation (single host at the
committed tree, `SINGLE_HOST_REPRODUCIBLE` with the second host still the
operator-gated lane) leaves the staleness, ancestry, toolchain, digest,
and depth gates green under `make quick`. The full `make v1` gate was
skipped because this is a subsystem handoff, not a release boundary.
`make v2` and `make release-check` remain intentionally fail-closed.
