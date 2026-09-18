# Reproducibility and Independent Conformance v1

Status: S20-730 contract draft, revision 10 (2026-09-18); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review). Revision 2 records the independent oracles that closed the two
native-only families (section 5). Revision 3 carries previously merged
attestations across rebuilds, binds every attested commit to the filing
history with an artifact-surface freshness rule, verifies the report digest
in the checker, and gives the coverage taxonomy its depth axis. Revision 4
covers the versioned entity-read corpus (sections 3 and 5): the family's
pinned S20-310 vectors live under `conformance/entity-read/v2`, so the
report records each family's versioned corpus directory, and the
entity-read checker joins the semantic depth. Revision 5 enumerates every
tracked corpus version (section 3): a family may carry several `v<N>`
corpora, and the report digests, sums, and declares each one, claiming
depth only for the pinned version; a tracked version without a manifest or
a pin outside the coverage map fails closed. Revision 6 tightens the
version rules (section 3): version directories match `^v\d+$` with any
other family-root or nested entry failing closed, conflicting duplicate manifest
lines are `CONFORMANCE_SUMS_MISMATCH`, shapes record mapping sizes, and the
report carries an auditable `tracked_corpus_directories` count; section 2
records that minting requires a whole-tree clean checkout including
untracked files, deliberately stricter than the surface-scoped check.
Revision 7 (2026-09-15, the a809906 Council round) changes no rule: it
records that the second-host lane was exercised through the section 5.1
runbook (section 9), completes the section 2 surface enumeration with the
root license files and the compile-time embedded inputs, states that the
report's `blockers` list is the wave-time list and not a live gate, names
the build-environment scrub, points section 1 `working_tree_clean` at the
whole-tree rule, exports the attestation admissibility and candidate
selection helpers every consumer imports, and binds the machine summary's
restated reproducibility facts to the report in the checker. The
mechanics are `scripts/build_reproducibility_report.py` and
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
`working_tree_clean` is the whole-tree rule of sections 2 (minting bullet)
and 8: the S20-720 predicate over tracked and untracked files at the mint,
deliberately stricter than the surface-scoped uncommitted check the checker
runs against the filing tree.

An attestation is *admissible* exactly when it passes this section's shape;
`build_reproducibility_report.admissible_attestation` owns that predicate,
`admissible_attestations(report, commit=None)` applies it to a report, and
`select_attestation(report, candidate=None, commit=None)` names the one
attestation of the current candidate (with a candidate evidence record, the
attestation binding its commit, artifact digest, manifest digest, and size;
without one, the attested commit carrying the most agreeing hosts, and
`None` when two commits tie). Every consumer of the tracked report (the
provenance and SBOM builders, the standards and packaging checkers, the
dossier and GA builders) imports these instead of restating the filter or
selecting `attestations[0]`.

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
  "superseded_attestations": [ { "host_label": string, "commit": hex[40],
                                 "artifact_sha256": hex[64], "reason": string }, ... ],
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
- a rebuild carries the tracked report's attestations forward: every
  previously merged attestation whose label is not re-attested in the run
  (neither as the fresh local attestation nor by an explicit `--attest`
  file) and whose commit is the commit the fresh local attestation names is
  re-validated and merged, so a plain rebuild never silently drops another
  host of the same candidate. A tracked attestation of another commit
  describes a superseded candidate: a re-mint does not carry it onto the
  new candidate and does not drop it silently either; the report lists it
  under `superseded_attestations` (`host_label`, `commit`,
  `artifact_sha256`, `reason`), and the superseded host re-attests the new
  candidate through section 5.1 (revision 10). A tracked file that is not a
  report, or that carries a malformed attestation, is
  `REPRO_ATTESTATION_INVALID`;
- the result is `MULTI_HOST_REPRODUCIBLE` exactly when some commit carries
  at least `required_hosts` agreeing attestations; otherwise it is
  `SINGLE_HOST_REPRODUCIBLE` and `second_host.status` is
  `GATED_OPERATOR_LANE`;
- an attested commit must be an ancestor of the filing `HEAD`: the report
  attests this history, and a commit outside it is
  `REPRO_ATTESTATION_INVALID`. Ancestry alone does not make an attestation
  current: no tracked file under the artifact input surface
  (`build_release_candidate.ARTIFACT_INPUT_PATHS`: the Rust workspace, the
  workspace manifest and lockfile, the toolchain pin, the root license
  files `LICENSE` and `NOTICE`, the demo runner, the SBOM inventory, the
  packaging script whose flags stage the binary, the tracked files the
  binary embeds at compile time, today `docs/spec/SSMC1_EPOCH1_SCHEMA.txt`,
  `conformance/smp1-json-bridge/v2/methods.json` and
  `conformance/smp1-json-bridge/v3/methods.json`, and the conformance
  subset) may differ between the attested commit and `HEAD`, else the
  artifact the report describes is not the artifact this tree builds, and
  the report is stale. The unit lane scans every `include_str!` and
  `include_bytes!` under `crates/` and refuses a non-test embed outside
  the surface, so the enumeration cannot silently lag the binary. The
  attested toolchain must also match the filing toolchain: a compiler
  upgrade changes the bytes without touching the tree. The cure for a
  stale report is `make release-candidate-smoke`, not an edit;
- the release build runs with every link and build override scrubbed from
  its environment (`build_release_candidate.SCRUBBED_LINK_ENV`: `CC`,
  `CXX`, `CFLAGS`, `CXXFLAGS`, `CPPFLAGS`, `LDFLAGS`, `LD`;
  `SCRUBBED_BUILD_ENV`: `CARGO_ENCODED_RUSTFLAGS`, `RUSTC`, `RUSTC_WRAPPER`;
  and every `CARGO_PROFILE_RELEASE_*` variable), because an attestation
  binds toolchain version strings only and an ambient override would attest
  `REPRODUCIBLE` for a non-canonical binary that only a second host's
  `REPRO_ATTESTATION_CONFLICT` could catch;
- `blockers` records the wave-time list: the packages that were open when
  the candidate was minted, as the builder's constant names them. The
  report is digest-bound and re-minted only by the two-host smoke, so the
  list is not a live gate and is not re-derived from package statuses; the
  live gates are `release-check`, `v2`, and the package checkers. Only
  `second_host_attestation_operator_lane` is derived, dropping exactly when
  the merge reaches `required_hosts`;
- minting while the operator working tree carries retained untracked
  material that must not be moved uses the canonical detached linked
  worktree procedure (operator decision, S20-720 wave): start from the exact
  candidate commit, create a detached linked worktree, prove that worktree
  clean, mint release and reproducibility evidence there, bind the evidence
  to the exact candidate commit and resulting artifact identity, and file
  refreshed evidence as a records-only descendant where required. The
  retained material stays in the operator tree untouched; the whole-tree
  clean-tree gate semantics are unchanged, only the checkout the mint runs
  in is made clean by construction;
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
  "tracked_corpus_directories": integer,
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
  "directory": "conformance/<name>/<pinned version>",
  "files": [{ "name": string, "sha256": hex, "bytes": integer }, ...],
  "sums_file": true,
  "sums_consistent": true,
  "contract": string | null,
  "claim": string | null,
  "shape": { file: { list-valued key: length, mapping key: size, except manifest } },
  "coverage": { "kind": "independent_oracle", "depth": "semantic" | "codec_and_identity", "runner": string, "command": string }
            | { "kind": "native_only", "note": string },
  "tracked_versions": ["v1", "v2", ...],
  "siblings": { version: { <version record>, "coverage": { "kind": "tracked_sibling", "pinned_version": string, "note": string } } }
}
```

The report also carries `coverage_depths`, the ascending directories at each
depth, so the depth distribution is readable without walking the fixtures.
`fixture_directories` counts families; `tracked_corpus_directories` counts
every tracked `v<N>` corpus across families, so the enumeration is auditable
from the report alone.

Rules:

- every directory under `conformance/` is a fixture family; an unmapped
  family is `CONFORMANCE_ORACLE_DRIFT`, so adding a fixture family requires
  declaring its coverage;
- each family names its pinned corpus version in the builder's
  `CORPUS_VERSION` map (default `v1`); today every family pins `v1` except
  `entity-read`, whose S20-310 vectors pin `v2` (section 10), and
  `exec-package-envelope`, whose only corpus is the `EXEC_PACKAGE_V2`
  framing vectors under `v2` (section 12); a pin naming a family outside
  the coverage map is `CONFORMANCE_ORACLE_DRIFT`;
- a family may carry several tracked corpus versions (`conformance/<name>/v<N>/`,
  `N` digits; any other family-root entry, and any nested entry inside a
  version directory, is `CONFORMANCE_FIXTURE_UNREADABLE`);
  the report records every one of them in `tracked_versions`, with a
  digested, summed, and declared record per version; depth is claimed only
  for the pinned version, and siblings carry `tracked_sibling` coverage that
  names the pinned version instead of a depth, so a tracked corpus is never
  silently outside the report and a sibling never reads as independently
  judged;
- a family mapped to an independent oracle names the exact command of the
  `make conformance` recipe; a command absent from the recipe is
  `CONFORMANCE_ORACLE_DRIFT`;
- a `SHA256SUMS` file must name every JSON file of its version directory
  with the correct digest (`CONFORMANCE_SUMS_MISMATCH`, including
  conflicting duplicate lines for one file); every tracked
  version directory carries one, so a version without a manifest is
  `CONFORMANCE_FIXTURE_UNREADABLE`, never a silent gap;
- an unreadable or non-JSON fixture is `CONFORMANCE_FIXTURE_UNREADABLE`
  (non-JSON files are digested but neither parsed nor manifest-covered);
- the result is `INDEPENDENT_CONFORMANCE_COMPLETE` exactly when no family is
  native-only. `COMPLETE` promises that every family's vectors are checked
  by an independent oracle at the recorded depth; it does not promise
  independent semantic judgment, which stays with the owner crate by design
  (master goal section 6.5, section 4 below). The depth axis keeps that
  limit visible: a `COMPLETE` report whose `codec_and_identity` list is
  non-empty names exactly where no independent oracle judges semantics.
  Revision 2 closed the two families that were native-only at revision 1: the extended VM vectors are checked by
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
- `independent_oracle` at `semantic` depth: the checker recomputes an
  outcome or judgment from frozen inputs with independent logic. Today that
  is the merge checker (recomputes both deltas and applies the merge
  judgment), the semantic-comparison checker (re-derives change classes and
  deltas), the complete-entity-impact checker (re-derives the edge set and
  its closure), the root-backed-query checker (re-derives result pages,
  work accounting, and failure precedence), and the entity-read checker
  (rebuilds expected response bytes, selection order, work accounting, and
  failure precedence from hand-authored semantic inputs).
- `independent_oracle` at `codec_and_identity` depth: the checker decodes
  containers and re-derives records, identities, keys, or digest trees
  without judging semantics. Today that is every other family, including
  the extended VM vectors and the release demo, whose checkers re-derive
  cache keys and request identities from frozen preimages.
- `native_only`: the family is exercised only through Rust code or through
  the packaged binary; it counts against the independent PASS. No family is
  native-only at revision 7 (unchanged since revision 2).

## 5.1 Second-host runbook

The second host is an operator-gated lane, so the procedure is written here
rather than automated:

1. On the second host, with the same commit checked out and a clean tree, run
   `make release-candidate-build`. It builds the candidate twice and writes the
   local S20-720 evidence record. When the checkout that must mint carries
   retained untracked material, use the canonical detached linked worktree
   procedure instead: `git worktree add --detach <path> <commit>`, prove the
   worktree clean, mint there, and file refreshed evidence as a records-only
   descendant; the gate stays whole-tree clean, the worktree is simply clean
   by construction.
2. Run
   `python3 scripts/build_reproducibility_report.py --host-label <label> --emit-attestation /tmp/<label>-attestation.json`.
   The attestation carries only the commit, artifact name, digest, size,
   manifest digest, member count, toolchain versions, and the clean-tree flag:
   no host name, user name, path, or time.
3. Copy that one JSON file to the primary host by any means the operator
   authorizes.
4. On the primary host run
   `python3 scripts/build_reproducibility_report.py --attest /tmp/<label>-attestation.json`,
   commit only the merged report and retained attestation as a provisional
   local checkpoint. The other derived documents may still name the previous
   candidate at this point. File the lane record naming that actual
   merge commit. Run `make evidence-refresh` because the receipt changes
   the files covered by the T54 scan, then `make release-candidate-verify`
   and `make quick`.
   Commit the receipt as a records-only descendant after those checks pass.
   The provisional merge is not a validated closure until its receipt and
   checks are complete.

The merged report reads `MULTI_HOST_REPRODUCIBLE` exactly when both hosts
attest the same commit with the same artifact digest; a disagreement is
`REPRO_ATTESTATION_CONFLICT` and writes no report, which is the point of the
exercise. Nothing in this runbook requires the second host to run any Sley
service, expose a port, or share a filesystem.

An attestation is unsigned and carries no host binding by design, so a
second-host attestation is indistinguishable in the report from a same-host
relabel; the trust root is operator custody of the transfer. Each exercise
of this runbook therefore files one row of the second-host lane record in
`evidence/release/second-host-lane-records.json` (date, transport lane,
lab checkout and commit, lab-side evidence record digest, transferred
attestation-file digest, bundle commit, merge commit), keeping identity out
of the report while the `MULTI_HOST_REPRODUCIBLE` claim stays auditable.

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
that every independent family carries a declared `semantic` or
`codec_and_identity` depth and the report's depth roll-up matches, that the
reproducibility report's digest recomputes and every attestation passes the
section 1 shape, that every attested commit is an ancestor of the filing
`HEAD` with no artifact-surface file changed since and the attested
toolchain current, that the
reproducibility report has at least one attestation and claims
neither GA nor publication, that the unit tests pass, and that
`release-check` and `v2` stay `NOT_IMPLEMENTED`. Minting requires a
whole-tree clean checkout including untracked files (section 1
`working_tree_clean`), deliberately stricter than the surface-scoped
uncommitted check: the attestation must bind the exact tree the artifact
builds from, not just its surface. Where the operator tree carries
retained untracked material, the binding is produced with the canonical
detached linked worktree procedure (section 2): the mint runs in a
worktree that is whole-tree clean, so the gate semantics are not weakened
to surface-only cleanliness.

## 9. Explicit exclusions

- No automated second-host build, transfer, or dispatch: the mechanics
  never build on, copy to, or dispatch a second host. Revision 7 records
  that the operator exercised the lane through the section 5.1 runbook
  (the tracked report carries a `secondary` attestation of the candidate
  merged at `6a2eef7` in that historical wave; subsequent candidate records
  live in the tracked report and records-eligible lane ledger); the exclusion is
  unchanged and names the mechanics, not the operator's lane.
- No independent VM semantic oracle: the extended VM vectors are checked at
  `codec_and_identity` depth, and an independent lowering and execution
  oracle that judges VM semantics would be a separate package. Revision 2
  closed the family's native-only state; this exclusion now names the
  remaining depth gap instead of a coverage gap.
- No GA claim, release decision, publication, root license, standards SBOM,
  or provenance statement; `release-check` and `v2` stay fail-closed.
- No timestamps, host names, user names, or paths in either report.

## 10. Clarifications

Revision 1 carries none.

Revision 3 records why `COMPLETE` keeps its no-native-only rule while the
depth axis exists: independent semantic judgment of every family was never
the bar, because the oracle must not become a second semantic kernel
(section 4, master goal 6.5); the bar is independent checking at a declared
depth, and the report's `coverage_depths` stop a codec-only family from
reading as a semantically judged one. It also records why freshness is an
artifact-surface diff rather than a commit count: a count bound would be
arbitrary, while a changed surface file means the attested artifact is
provably not what this tree builds.

Revision 5 records why `entity-read` pins `v2`: the S20-310 read methods
are protocol v2 methods (`entity.version` 306, `entity.signature` 307), so
no v1 entity-read corpus ever existed and none is invented; the pin names
the first and only corpus. It records the vector emission direction for the
same family: hand-authored semantic inputs (`inputs.json`, carrying the
authored frame-scenario inventory) flow into the oracle refresh, which
derives `accepted.json` and `rejected.json` plus the manifest; the Python
vector checker consumes all three, and the Rust owner test
`accepted_corpus_vectors_match_owner_and_encoder` reproduces every accepted
vector's response bytes, work charge, and object count from the same frozen
files through `include_str!`, the same pattern the scb1 and mutation corpora
already use. Reading frozen committed vectors in tests is not an S20-130
dependence: independence forbids the oracle from depending on the Rust
implementation, not the reverse. It
records why tracked siblings carry no depth: a sibling corpus (today
`bootstrap-profile`, `exec-package`, `host-abi`, and `smp1-json-bridge` at
`v2`) is digested, summed, and declared so it can never be a silent gap,
but depth is claimed only for the pinned version the mapped checker
actually runs against; promoting a sibling to pinned is a contract change,
not a builder default.

## 10. Revision 8 (2026-09-15)

The section 5.1 lane-record home is records-eligible, so documenting the
actual merge does not invalidate the candidate it attests. The historical
7a94a4a row remains in the closeout; subsequent rows are in the JSON ledger
with contract `sley2.second-host-lane-records.v1` and a `records` array.
Each row names `candidate_commit`, `date`, `transport_lane`, `lab_checkout`,
`lab_commit`, `lab_evidence_sha256`, `artifact_sha256`, `attestation_path`,
`attestation_file_sha256`, `bundle_commit`, and `merge_commit`. Optional
`note` explains unavailable historical transport detail without guessing.
`attestation_path` points to the transferred JSON retained under
`evidence/release/attestations/`; its byte digest must match the receipt.
For each selected multi-host candidate the checker requires a row binding
its commit/artifact, equal lab and bundle commits, a reachable merge commit,
and an admissible retained secondary attestation with the same full identity.
The lab evidence digest and transport details are custody receipts, not
cryptographic host authentication. The checker does not claim remote access.

The `artifact_name` field is exactly S20-720's `ARTIFACT_NAME`; a foreign or
non-string value is `REPRO_ATTESTATION_INVALID`. Freshness tests exercise
real isolated histories for record-only and source-changing descendants,
uncommitted source changes, absent history and unavailable/changed toolchains.

The revision-7 paragraph below its historical heading describes that wave,
including its then-current mint. Current candidate identity is always the
tracked report and summary, with the matching lane receipt in the ledger.

## 11. Revision 9 runbook correction (2026-09-15)

The 400895e review identified a redundant pre-commit refresh that was not
performed in that mint. Section 5.1 now matches the recorded and required
sequence: commit the merged report and retained secondary, file the receipt
against that commit, refresh all derived evidence, verify, and run quick.
The provisional merge is never claimed as a validated closure.

## 12. Revision 10 (2026-09-18): re-mint supersession and the envelope pin

The 178873d7 Council round (Ariadne P2, Nabu P2, Vulcan P2) found that the
carry-forward rule had no supersession path: at a re-mint the documented
`make release-candidate-build` carried a secondary attestation of the
superseded commit onto the new candidate (distinct hosts across two
commits, no selectable attestation, a stale-report checker failure), and
the primary-only reports at 75ad17aa and 9316df19 were produced by an
undocumented step. Section 2 now carries only attestations of the commit
the fresh local attestation names; attestations of another commit are
listed as `superseded_attestations` rather than dropped silently or kept
wrongly. `build_reproducibility_report.carried_attestations` implements
the rule (`bench/release/tests/test_reproducibility.py::
test_a_re_mint_supersedes_attestations_of_another_commit`).

The same round (Ariadne P3, Nabu P3, Vulcan P3) found the
`exec-package-envelope` corpus pinned at `v2` by the builder alone.
Section 3 now names the pin: the family's only corpus is the
`EXEC_PACKAGE_V2` framing vectors (`conformance/exec-package-envelope/v2`,
checked by `scripts/check_exec_package_envelope_v2.py` at
`codec_and_identity` depth); a `v1` directory never existed because the v1
package format had no standalone envelope corpus. The fixture's
`status: PROVISIONAL_RW_080_CONSTRUCTION_REVIEW_PENDING` names the
RW-080 review state of the construction that emitted it, not the coverage
class, which is complete for the family (Nabu/Vulcan P4 note).
