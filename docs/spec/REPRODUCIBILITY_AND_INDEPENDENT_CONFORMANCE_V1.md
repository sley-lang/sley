# Reproducibility and Independent Conformance v1

Status: S20-730 contract draft, revision 2 (2026-09-03); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review). Revision 2 records the independent oracles that closed the two
native-only families (section 5). The mechanics are `scripts/build_reproducibility_report.py` and
`scripts/build_independent_conformance_report.py`; implementation state is
tracked in the machine summary.

## Boundary

S20-730 freezes how the Sley 2 candidate's reproducibility is attested across
hosts and how the independent conformance coverage of every fixture family is
recorded (master goal sections 6.5, 16.7 "reproducibility report" and
"final independent PASS"; dossier 21). It composes the S20-720 candidate
mechanics (`RELEASE_CANDIDATE_PACKAGING_V1.md`) and the S20-130 oracle
independence rule; it does not build the candidate itself, does not claim GA,
and does not open `release-check` or `v2`, which stay fail-closed. A second
host attestation is an operator-gated lane: the report records its absence as
`GATED_OPERATOR_LANE` rather than pretending to a multi-host result.

## 1. Reproducibility attestations

An attestation is the evidence one host produces after running the S20-720
mechanics to completion:

```text
attestation = {
  "contract": "sley2.reproducibility-attestation.v1",
  "host_label": non-empty string chosen by the operator ("primary" by default),
  "commit": 40 lowercase hex characters of the built commit,
  "artifact_name": "sley-2.0.0-linux-x86_64.tar.gz",
  "artifact_sha256": 64 lowercase hex characters,
  "artifact_size_bytes": integer,
  "manifest_digest": 64 lowercase hex characters,
  "member_count": integer,
  "toolchain": { "cargo": string, "rustc": string },
  "reproducibility": "REPRODUCIBLE",
  "working_tree_clean": true,
  "differing_members": []
}
```

`scripts/build_reproducibility_report.py --emit-attestation <path>` derives
it from the local S20-720 evidence record
(`evidence/runtime/s20-720-release-candidate/evidence.json`). An evidence
record whose result is not `PASS` or whose two builds were not
`REPRODUCIBLE` cannot become an attestation (`REPRO_EVIDENCE_INVALID`); a
missing record is `REPRO_EVIDENCE_MISSING`. No host name, user name, path, or
time enters an attestation; the host label is the operator's.

## 2. Reproducibility report

`scripts/build_reproducibility_report.py` merges the local attestation with
any number of `--attest <path>` attestations from other hosts into
`evidence/release/reproducibility-report.json`:

```text
report = {
  "contract": "sley2.reproducibility-report.v1",
  "work_package": "S20-730",
  "required_hosts": 2,
  "distinct_hosts": integer,
  "attestations": [attestation, ...] sorted by host_label,
  "commits": { commit: { "artifact_sha256": hex, "hosts": [host_label, ...] } },
  "result": "SINGLE_HOST_REPRODUCIBLE" | "MULTI_HOST_REPRODUCIBLE",
  "second_host": { "status": "GATED_OPERATOR_LANE" | "ATTESTED", "note": string },
  "ga_claimed": false,
  "publication_authorized": false,
  "blockers": [string, ...],
  "report_digest": SHA-256 of the canonical report without this field
}
```

Rules:

- two attestations with the same `host_label` are `REPRO_ATTESTATION_INVALID`;
  an attestation that fails the section 1 shape is the same code;
- two attestations for the same commit with different `artifact_sha256` are
  `REPRO_ATTESTATION_CONFLICT`, and no report is written;
- the result is `MULTI_HOST_REPRODUCIBLE` exactly when some commit carries
  at least `required_hosts` agreeing attestations; otherwise it is
  `SINGLE_HOST_REPRODUCIBLE` and `second_host.status` is
  `GATED_OPERATOR_LANE`;
- the report contains no timestamp, so equal inputs give equal bytes; the
  canonical form is JSON with sorted keys, two-space indentation, and a
  trailing newline.

## 3. Independent conformance report

`scripts/build_independent_conformance_report.py` derives
`evidence/conformance/independent-conformance-report.json` from tracked files
only, so the report is reproducible from the commit and `--check` detects
drift:

```text
report = {
  "contract": "sley2.independent-conformance-report.v1",
  "work_package": "S20-730",
  "make_target": "conformance",
  "fixture_directories": integer,
  "independently_checked": integer,
  "native_only": [directory, ...],
  "fixtures": [fixture, ...] sorted by directory,
  "oracle_independence": { "python_sources": integer,
                           "forbidden_markers": [string, ...],
                           "problems": [] },
  "result": "INDEPENDENT_CONFORMANCE_COMPLETE" | "INDEPENDENT_CONFORMANCE_PARTIAL",
  "report_digest": SHA-256 of the canonical report without this field
}
fixture = {
  "directory": "conformance/<name>/v1",
  "files": [{ "name": string, "sha256": hex, "bytes": integer }, ...],
  "sums_file": bool,
  "sums_consistent": true | null,
  "contract": string | null,
  "claim": string | null,
  "shape": { file: { list-valued key: length } },
  "coverage": { "kind": "independent_oracle", "runner": string, "command": string }
            | { "kind": "native_only", "note": string }
}
```

Rules:

- every directory under `conformance/` is a fixture family; an unmapped
  family is `CONFORMANCE_ORACLE_DRIFT`, so adding a fixture family requires
  declaring its coverage;
- a family mapped to an independent oracle names the exact command of the
  `make conformance` recipe; a command absent from the recipe is
  `CONFORMANCE_ORACLE_DRIFT`;
- a `SHA256SUMS` file must name every JSON file of its family with the
  correct digest (`CONFORMANCE_SUMS_MISMATCH`); a family without one records
  `sums_consistent: null`;
- an unreadable or non-JSON fixture is `CONFORMANCE_FIXTURE_UNREADABLE`;
- the result is `INDEPENDENT_CONFORMANCE_COMPLETE` exactly when no family is
  native-only. Revision 2 closed the two families that were native-only at
  revision 1: the extended VM vectors are checked by
  `sley2_scb1_oracle.vm_extended`, which decodes the `SLEYBC02` container and
  re-derives every cache key from the frozen preimage, and the release demo by
  `scripts/check_release_demo_vector.py`, which re-derives the `RootQueryId`
  and the `ExecutionReportId` from the recorded preimages. Both are codec and
  identity oracles written from the contracts; neither judges semantics, so
  the oracle does not become a second semantic kernel;
- `--check` recomputes the report and fails with `CONFORMANCE_REPORT_DRIFT`
  when the tracked file differs.

## 4. Oracle independence

The independent oracle (`oracle/scb1`) must not acquire a Rust
implementation dependency (S20-130). The report re-applies the forbidden
marker scan of `scripts/check_oracle_independence.py` and records the
source count and the marker list; any problem is `CONFORMANCE_ORACLE_DRIFT`.
The oracle is for conformance only and must not become a second semantic
kernel (master goal section 6.5).

## 5. Coverage classes

- `independent_oracle`: the family's vectors are checked by the Python
  oracle package or by a Python vector checker that decodes with the oracle,
  without the Rust implementation. Such a checker verifies containers,
  identities, and declared bindings; semantic judgment stays with the owner
  crate, so the oracle never becomes a second kernel (master goal section
  6.5).
- `native_only`: the family is exercised only through Rust code or through
  the packaged binary; it counts against the independent PASS. No family is
  native-only at revision 2.

## 6. Evidence files

- `evidence/release/reproducibility-report.json` (tracked): rebuilt by
  `make release-candidate-smoke` after the candidate build, so its
  `commit` names the last locally attested build.
- `evidence/conformance/independent-conformance-report.json` (tracked):
  rebuilt by the generator and verified by `--check` under `make quick`.

## 7. Codes

S20-730 reserves 73000 through 73007: `REPRO_EVIDENCE_MISSING` (73000),
`REPRO_EVIDENCE_INVALID` (73001), `REPRO_ATTESTATION_INVALID` (73002),
`REPRO_ATTESTATION_CONFLICT` (73003), `CONFORMANCE_FIXTURE_UNREADABLE`
(73004), `CONFORMANCE_SUMS_MISMATCH` (73005), `CONFORMANCE_ORACLE_DRIFT`
(73006), `CONFORMANCE_REPORT_DRIFT` (73007). Each script exits 1 and prints
one JSON object naming the code on failure.

## 8. Staging

`scripts/check_reproducibility_and_independent_conformance.py` runs under
`make quick`. Statuses: `S20_730_CONTRACT_DRAFT_REVIEW_PENDING`,
`S20_730_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS`,
`S20_730_MECHANICS_IMPLEMENTED_REVIEW_PENDING`, and `S20_730_COMPLETE`, the
last requiring the three Council reviews to read `PASS`. In every
implementation status the checker verifies both reports exist with their
contract tags, that the independent conformance report passes `--check`,
that the reproducibility report has at least one attestation and claims
neither GA nor publication, that the unit tests pass, and that
`release-check` and `v2` stay `NOT_IMPLEMENTED`.

## 9. Explicit exclusions

- No second-host build, transfer, or dispatch: the laptop lane is gated by
  the operator posture and stays gated until reopened for this purpose.
- No independent VM oracle: the extended VM vectors stay native-only until
  an independent lowering and execution oracle is commissioned (a separate
  package).
- No GA claim, release decision, publication, root license, standards SBOM,
  or provenance statement; `release-check` and `v2` stay fail-closed.
- No timestamps, host names, user names, or paths in either report.

## 10. Clarifications

Revision 1 carries none.
