# Sley 2.0.0 release notes

**Release:** Sley 2.0 (version `2.0.0`)
**Decision:** operator release of what exists, recorded 2026-09-24 in the
machine summary's `release_decision` (`RELEASE_APPROVED`, authority: operator).
**Publication:** authorized by the operator on 2026-09-24 (the machine
summary's `publication_decision`). The source is public at
<https://github.com/sley-lang/sley> as a single-commit export of the release
tree, and the artifact is published as the GitHub release `v2.0.0`.
**Not a GA claim.** `ga_claimed` stays `false` in every record. The unmet GA
acceptance criteria are listed in full below.

The artifact identity (commit, SHA-256, byte size, manifest digest) is fixed by
the release-candidate build of the tagged commit. It is recorded in
`evidence/release/reproducibility-report.json` and in the machine summary's
`release_candidate_packaging` section, not in this document.

The `v2.0.0` tag points at the source commit the artifact was built from. The
release records for the published artifact (the reproducibility report, the
SBOMs, the provenance, and the second-host attestation) were committed after
the tag, in records-only commits that change nothing outside `evidence/` and
`machineresearch/`. Read those records from the records-only commits that
directly follow the tag on `main`, not from the tagged tree.

## What Sley 2.0 is

Sley 2 is a new, incompatible programming-system lineage. Programs are created,
stored, changed, executed, tested, versioned and exchanged as typed semantic
state rather than as source text. The canonical program form is SSMC1, the
canonical encoding is SCB1 and the machine interface is SMP1. The governing
doctrine is that machines do not write source; they mutate verified program
state. There is no Sley source parser, canonical text format, formatter or
compatibility promise for Sley 1.x. Sley 1.2.0 is preserved and
checksum-verified as the frozen legacy lineage.

## What is in it

These are the GA acceptance criteria that `evidence/release/ga-acceptance-report.json`
derives as `EVIDENCED` from tracked evidence. The counts are those in the
report when this release was decided.

- **Lineage (26.1):** Sley 1.2.0 preserved and checksum-verified; no Sley 1.2.1
  implementation was required; independent repository history; no legacy
  source tree copied wholesale; every reused concept has a disposition and
  evidence.
- **Canonical format (26.2):** SCB1 frozen as version 1; the Rust
  implementation and the independent Python oracle produce byte-identical
  output; every non-canonical fixture is rejected; hashes are domain separated;
  schema epochs are explicit; pack import reconstructs exact roots; corruption
  is detected before a ref advances.
- **Semantics (26.3):** SSMC1 is the only canonical program representation; no
  source parser exists in the GA dependency graph; type, CFG, effect, contract
  and identity checks are deterministic; VM semantics agree with the
  conformance fixtures.
- **Agent loop (26.4):** query capsules are bounded; session handles are
  stale-safe; machine outputs use stable codes and contracts.
- **Repository (26.5):** state versions are content addressed; disjoint merge
  is deterministic; ambiguous merge creates conflict objects; GC preserves all
  retained roots; crash recovery produces only the old state or the complete
  new state.
- **Policy and security (26.6):** adapters are bounded; no arbitrary shell
  exists.
- **Packaging (26.8):** the artifact is named `sley-2.0.0-linux-x86_64.tar.gz`;
  it contains no secrets, local paths, caches or debug files; the manifest,
  SHA-256, size, SBOM, license inventory and provenance are recorded; a second
  clean build establishes reproducibility; the source working tree is clean.
- **Review (26.9):** publication happens only under a separate grant. The
  operator's recorded publication decision is that grant.

### Reproducible two-host artifact pipeline

- `make release-candidate-build` runs two clean, self-contained musl builds
  (`x86_64-unknown-linux-musl`, pinned toolchain `1.93.0`, ambient C compiler
  scrubbed) and compares them member by member. It then writes the S20-720
  evidence record, the reproducibility report, the content checks,
  CycloneDX 1.6 and SPDX 2.3 SBOMs, unsigned build provenance, the finding
  register, the GA acceptance report and the decision dossier. The T54 secret
  scan runs last.
- A second host rebuilds the exact candidate from a transferred Git bundle and
  emits an attestation that carries no host identity. `scripts/second_host_attest.sh`
  (phases `check`, `build` and `merge`) handles this, and the attestation is
  merged with `--attest`. Each exercise files a custody receipt in
  `evidence/release/second-host-lane-records.json`. The report reads
  `MULTI_HOST_REPRODUCIBLE` only when both hosts attest the same commit with the
  same artifact digest.
- The packaged binary runs a 12-step demo outside the source tree (S20-720
  unpacked demo).

### Succession harness

The `sley_2_0` live arm of the frozen 15-task succession corpus is integrated
(`bench/live/`, coverage in `bench/live/SUCCESSION-COVERAGE.md`). This covers
the mediated agent tool surface, the task judges, hash-chained tool-boundary
transcripts and the S20-630 accounting. Scripted witnesses drive the frozen
tool surface to accepted results for 13 of the 15 tasks, with two caveats:
CORRUPT's obligation is accepted on judge-side evidence under a recorded owner
ruling, and TYPE's fixture revision still records its own fresh-review gate.
EFFECT and CAP have no positive result (see below). Scripted acceptance is
harness evidence, not a succession result.

## Build and verify

The build needs a clean checkout at the release commit, the pinned Rust
toolchain (`rust-toolchain.toml`, 1.93.0) with the `x86_64-unknown-linux-musl`
target installed, Python 3 and `uv`. On a fresh cargo cache, run
`cargo fetch --locked` first.

```sh
make release-candidate-smoke        # two clean builds + all release records, then verify
make release-candidate-verify       # re-check the records against the tree
sha256sum dist/sley-2.0.0-linux-x86_64.tar.gz
python3 -c 'import json; r = json.load(open("evidence/release/reproducibility-report.json")); print(r["result"], r["commits"])'
make quick                          # the maintainers' routine gate (see below)
```

The digest printed by `sha256sum` must equal the `artifact_sha256` that the
reproducibility report records for the release commit. `make quick` reads the
outputs of the build above and the Sley 2.0 master goal, which lives outside
the repository (`SLEY2_MASTER_GOAL`), so it passes only on a maintainer's
machine. [QUICKSTART](../QUICKSTART.md#6-run-the-test-gates) lists the gates
anyone can run.

To reproduce the build on a second host, run `scripts/second_host_attest.sh --phase check`, then
`--phase build` and `--phase merge`, as `docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md`
section 5.1 describes.

## Known limits and what is not yet claimed

**GA acceptance report (at the release decision):** 52 criteria, of which 32
are `EVIDENCED`, 17 are `AWAITS_REVIEW` and 3 are `GATED`. No combination of
states is a GA claim. The 3 gated criteria are:

1. `26.6 policy and security - no P0/P1/P2 finding remains open`
2. `26.7 succession - every section 22 threshold passes`
3. `26.8 packaging - artifact is built from the final candidate commit`. This
   row evidences once the final mint's commit is recorded as
   `release_decision.final_commit`.

The 17 criteria awaiting review wait on independent review judgments. The
operator has decided that Council reviews are no longer used, so these rows
stay `AWAITS_REVIEW`. They are listed in `release_decision.unmet_ga_criteria_at_decision`.

**Open P1/P2 findings:**

- **rw075 correction (4 P1, 4 P2):** AR-02 is the self-hosted path's
  large-preimage hashing gap. It is being closed in the 2.1 self-hosting
  work, with an exact streaming BLAKE3 primitive for the self-hosted path. No
  identity-preimage ceiling is imposed, and native identity semantics and
  limits are unchanged in 2.0. The AR-06 review transcripts are not bound.
  Both belong to the self-hosting work in 2.1. By code trace (not yet pinned by a test), the raw-hash
  path they concern is not reachable from the 2.0 `sley` binary.
- **Finding-ledger mechanism P2s:** `release_candidate_packaging` (4),
  `reproducibility_and_independent_conformance` (3) and
  `standards_sbom_and_provenance` (2). They concern the claim-identity and
  retirement mechanics in `scripts/build_finding_register.py` and
  `scripts/retire_review_claims.py`, not the product.

**Succession benchmark:** the campaign is in progress. No accounting report is
tracked yet (`evidence/release/succession-accounting-report.json` is absent),
so no section 22 threshold result is claimed. Results will be published when
the campaign completes.

**CREATE:** a single candidate that carries both new functions and tests
targeting them is refused at commit (`TXN_TEST_EVIDENCE_UNSUPPORTED`). CREATE-style
code and tests currently reach the committed state in two commits. Admitting
them together in one candidate needs the native test supervisor.

**EFFECT and CAP:** the E7 opcodes stay excluded. Programs that need them get
the deterministic refusal `VM_LOWER_OPCODE_UNSUPPORTED`, so the EFFECT and CAP
succession tasks have no positive result.

**Self-hosting (SH2)** is the 2.1 goal. The REWEAVE seed artifacts (canonical
S, C0 and the C1 candidate) are preserved, but the official C1 is not claimed.

**Also not claimed:** signed provenance or a transparency-log entry
(provenance is recorded unsigned).
