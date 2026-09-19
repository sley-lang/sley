# Release Candidate Packaging v1

Status: S20-720 contract draft, revision 6 (2026-09-18), with round-7
clarifications (2026-09-11, section 13); Council review pending (Ariadne
contract review, Nabu architecture review, Vulcan surface review). Revision
2 records the clarifications found while implementing
revision 1 (section 11). Revision 3 orders the remaps most-general-first,
describes the manifest's non-release status, requires a clean tree for
tracked evidence, and enumerates the demo's 20.12 verbs honestly
(section 12). Revision 4 ships the operator-approved root license
(section 14). The mechanics are `scripts/build_release_candidate.py`
and `bench/release/run_demo.py`; implementation state is tracked in the
machine summary.

## Boundary

S20-720 freezes the mechanics of the clean-room release candidate: how the
artifact `sley-2.0.0-linux-x86_64.tar.gz` is built twice from the working
tree, what it contains, how it is unpacked and exercised with no source
tree, how its checksums, manifest, inventory, and scans are recorded, and
how reproducibility is established or its nondeterminism named (master
goal sections 18.2 `release-check`, 20.12, 26.8; dossier 21). It does not
claim GA, a release decision, publication, or the operator approvals it
records as blockers: `make release-check` and `make v2` stay fail-closed
(`NOT_IMPLEMENTED`) until the GA gates exist, and the candidate mechanics
run under `make release-candidate-smoke`.

## 1. Build

`scripts/build_release_candidate.py` performs, in order:

1. a clean release build of `sley-cli` (`cargo build --release --locked
   -p sley-cli`) in a fresh target directory under `dist/`, with
   `--remap-path-prefix` mapping the working tree to `/sley2` so no local
   absolute path enters the binary. rustc applies the last matching rule,
   so the flags run most-general-first: the home directory to
   `/home-remapped`, the cargo registry sources to `/cargo/registry/src`,
   and the working tree to `/sley2` last. Any other order lets the home
   rule shadow the tree rule and moves the leak where the scan cannot see
   it (section 5);
2. packaging (section 2) into `dist/sley-2.0.0-linux-x86_64.tar.gz`;
3. unpacking the artifact into a private directory outside the working
   tree and running the conformance subset and the canonical demo there
   (sections 3 and 4) with the working directory inside the unpacked
   artifact and no environment variable naming the source tree;
4. verifying every manifest entry's SHA-256 and size and scanning every
   member for the source tree path, any `/home/` path, the
   `/home-remapped` residue, the build username, and the bounded
   secret patterns (section 5);
5. a second clean build in a second fresh target directory and a second
   package, compared byte for byte with the first (section 6);
6. writing `evidence/runtime/s20-720-release-candidate/evidence.json`.

Any step that fails stops the run with its code; nothing is published.

## 2. Contents

```text
sley-2.0.0-linux-x86_64/
  bin/sley                          the S20-430 endpoint binary
  MANIFEST.json                     contract, commit, toolchain, files with sha256 and size
  SBOM.json                         the S20-710 pre-release inventory, verbatim
  LICENSES.json                     declared licenses per package and the approved root license
  LICENSE                         the operator-approved root license text, installed verbatim
  NOTICE                            the approved ownership and license notice
  conformance/smp1/v1/              the S20-410 fixture
  conformance/smp1-json-bridge/v1/  the S20-420 fixture and method table
  conformance/release-demo/v1/      the demo fixture (section 4)
  demo/run_demo.py                  the canonical demo, self-contained
```

The tar archive is deterministic: members sorted by path, mtime zero,
uid and gid zero, modes normalized (0644 files, 0755 executables and
directories), gzip with mtime zero and no name. `MANIFEST.json` is
canonical JSON (S20-610 rules) and names the artifact, the contract
`sley2.release-candidate-manifest.v1`, the exact commit, `rustc` and
`cargo` versions, the target triple, and every member's path, size, and
SHA-256; its own digest is the SHA-256 of its canonical bytes. The
manifest also carries the artifact's non-release status inside that
digest: `ga_claimed: false`, `publication_authorized: false`, the
blockers that keep `release-check` fail-closed, and the `working_tree_clean`
flag of the built tree, so a dirty build can never carry a bare commit and
the artifact is self-describing once it leaves `dist/`. A manifest that
omits any of these fields is `PACKAGE_MANIFEST_INVALID`.

## 3. Conformance subset

From the unpacked artifact only: `bin/sley methods` equals the packaged
method table; `bin/sley frame decode` over every packaged SMP1 fixture
frame equals the packaged bridge JSON; `bin/sley frame encode` reproduces
the frame bytes; `bin/sley version` names the CLI and protocol versions.

## 4. Canonical demo (source-independence proof)

`demo/run_demo.py` drives `bin/sley serve --json` over two empty
directories inside the unpacked artifact and nothing else. The fixture
`conformance/release-demo/v1/demo.json`, emitted from the executable test
genesis and drift-gated in `make quick`, carries the exchange bytes of a
root holding one executable Function, a `query.root` summary request bound
to that root with its expected response record, an `execute` request with
its expected report identity, and the head transaction identity. The demo:

1. imports the exchange into the first directory (`exchange.import`),
   opens a session, and answers the summary query byte-identically to the
   fixture;
2. executes the Function (`execute`) and reads the report back (`report`)
   with the fixture's identity;
3. creates a branch at the head (`branch.create`) and exports the
   repository (`exchange.export`);
4. imports the export into the second directory, opens a session there,
   and answers the same summary query byte-identically;
5. runs GC (`gc.dry_run`) on both and requires no deletion candidate.

The environment holds no `.sley` source, parser, Sley 1.x artifact,
Tree-sitter grammar, LSP, or projection; the demo reads only the unpacked
files. The demo exercises nine of the forty-one dispatched methods:
`exchange.import`, `session.open`, `query.root`, `execute`, `report`,
`branch.create`, `exchange.export`, `gc.dry_run`, and `session.close`.
Against master goal section 20.12's eight verbs (create, modify, execute,
test, branch, merge, export, import) that is four covered (execute,
branch, export, import) and four residual (create, modify, test, merge);
query, report, session, and GC steps are plumbing, not 20.12 verbs. The
demo proves source-independence for the verbs it covers, not for every
operation the protocol dispatches.
Candidate construction, commit, test selection, and merge through
the demo wait for the public candidate builder (S20-350 is proposal-only);
creation waits for a `workspace.create` step from a packaged trusted
genesis, which is the recorded yes to the campaign's open question 1 and
lands as a demo extension. Until then create stays in the residual above,
and all of it is recorded as the demo's explicit gap.

## 5. Forbidden content

A member containing the source tree's absolute path, any `/home/` path,
the remap residue `/home-remapped`, the build username, or a bounded
secret pattern (private-key headers, cloud and token prefixes)
is `PACKAGE_CONTENT_FORBIDDEN`. The `/home-remapped` needle exists because
the home remap rewrites a leaked tree path into a shape the tree-path and
`/home/` needles can never match; the username needle catches
non-path-shaped leakage the remaps do not reach. Caches, target
directories, and debug files are never packaged: the member list is the
section 2 list exactly.

## 6. Reproducibility

The second package must equal the first byte for byte. When it does not,
the run records `NOT_REPRODUCIBLE` with the exact differing member paths
and whether the difference is archive-only (identical member bytes,
differing archive metadata) or content; the result is never rounded to a
pass.

## 7. Evidence and blockers

`evidence.json` records the contract `s20-720-release-candidate-v1`, the
commit, both builds' toolchains, the artifact name, size, and SHA-256, the
manifest digest, the member count, the conformance and demo results, the
scan result, the reproducibility result with any differing members, the
working tree cleanliness, and the blockers that keep `release-check`
fail-closed: root license text approval (operator), standards SBOM and
provenance (S20-710 full), succession thresholds (S20-640), and the
Council reviews. Tracked evidence requires a clean tree: the smoke passes
`--require-clean` (opt out only with `--allow-dirty`, which no tracked
target uses), a dirty tree stops with `PACKAGE_TREE_DIRTY`, and the
manifest's `working_tree_clean` flag names what was built.

Where the operator tree carries retained untracked material that must not
be moved, minting uses the canonical detached linked worktree procedure
(`REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md` section 2): the mint
runs in a worktree that is whole-tree clean by construction, and the
`--require-clean` gate semantics are unchanged.

The register's `candidate_*` fields must name the tracked attestation:
commit, artifact digest, manifest digest, size, member count, cleanliness,
reproducibility, and toolchain, with the attestation clean and
`REPRODUCIBLE`; the checker enforces the binding as
`candidate-attestation-mismatch`, so the register cannot name
a candidate no attestation describes. The lint report
`evidence/build/lint-report.json` (`sley2.lint-report.v1`) carries eleven
fields: `contract`, `commit`, `fmt_clean`, `fmt_detail`, `clippy_clean`,
`clippy_warnings`, `result` (recorded since the `make lint` gate landed at
`bedd4220`, 2026-09-13), `working_tree_clean`, `lint_inputs_clean` and
`dirty_lint_inputs` (revision 6, 2026-09-18, the candidate binding), and
`dirty_paths` (2026-09-19,
the literal `git status --porcelain -z` paths behind
`working_tree_clean`, so a `false` names what was dirty — the records-only
files of the mint, never a lint input). It is bound to the candidate: its
`commit` must equal `candidate_commit`, `lint_inputs_clean` must be true
and `result` `PASS`; the checker enforces this as
`lint-report:commit-differs-from-candidate` and
`lint-report:not-a-clean-pass` (revision 6, section 16).
`machine-summary.json` `artifact` stays null until an operator-approved
release candidate exists.

## 8. Stable failures

| Numeric | Symbolic code |
|---:|---|
| 72000 | `PACKAGE_BUILD_FAILED` |
| 72001 | `PACKAGE_MANIFEST_INVALID` |
| 72002 | `PACKAGE_CONTENT_FORBIDDEN` |
| 72003 | `PACKAGE_CONFORMANCE_FAILED` |
| 72004 | `PACKAGE_DEMO_FAILED` |
| 72005 | `PACKAGE_NOT_REPRODUCIBLE` |
| 72006 | `PACKAGE_TREE_DIRTY` |
| 72007 | `PACKAGE_INTERNAL_INVARIANT` |

## 9. Required evidence

- the demo fixture generated from the executable genesis and drift-gated;
- offline tests of the packaging primitives (deterministic tar, manifest
  digest, content scan, reproducibility comparison);
- `make release-candidate-smoke` producing the evidence file with the
  demo, conformance subset, scan, and two-build comparison;
- `scripts/check_release_candidate_packaging.py` in `make quick`, which
  also requires `release-check` to stay `NOT_IMPLEMENTED`;
- Tier 1 plus Tier 2 validation, and the Ariadne, Nabu, and Vulcan
  reviews with every report-grade finding closed.

## 10. Explicit exclusions

### Smoke outputs and their owners

The shared Make target orchestrates several owners. An output emitted by
`release-candidate-build` or checked by `release-candidate-verify` does not
become an S20-720 acceptance decision merely because it runs in that target.

| Output | Owning package / authority |
|---|---|
| Candidate archive, manifest and `evidence/runtime/s20-720-release-candidate/evidence.json` | S20-720 packaging, build and demo evidence |
| `evidence/release/candidate-content-checks.json` | S20-720 artifact-content evidence, section 15 |
| `evidence/release/reproducibility-report.json` and its transferred host attestations | S20-730 reproducibility and independent conformance |
| `evidence/release/sbom/{cyclonedx-1.6,spdx-2.3}.json` and `evidence/release/provenance.json` | S20-710 standards SBOM and provenance |
| `evidence/review/finding-register.json` | S20-740 finding disposition and clearance |
| `evidence/release/ga-acceptance-report.json` and `evidence/release/decision-dossier.json` | S20-750 GA criteria and decision assembly; neither is itself an operator release decision |
| `evidence/security/T52/pre-release-inventory.json` and `evidence/security/T54/secret-scan.json` | Threat-register T52/T54 supply-chain evidence, consumed by S20-710 and security review |
| `evidence/build/lint-report.json` (`sley2.lint-report.v1`) | `make lint` / `scripts/record_lint_report.py`; bound to `candidate_commit` by this package's checker (revision 6) |

`sync_evidence_counters.py` synchronizes derived summary counters; it grants
no acceptance. The final supply-chain refresh accounts for generated file
changes. `release-candidate-verify` reads these owners' evidence through
their checkers; it does not replace their independent review obligations.

### Scope limit

This contract does not claim: GA or a release decision (S20-750);
publication, push, tag, upload, or deployment; the standards SBOM,
provenance, and root license (S20-710 full); independent conformance
(S20-730); the succession benchmark; or any change to the fail-closed
`release-check` and `v2` gates.

## 11. Revision 2 clarifications

- The binary is built with three remaps: the working tree to `/sley2`, the
  cargo registry sources (`$CARGO_HOME/registry/src`) to
  `/cargo/registry/src`, and the home directory to `/home-remapped`; the
  first build without the registry remap left ten registry source paths in
  the binary and the content scan refused the artifact, which is the scan
  working as intended.
- Working-tree cleanliness is recorded in the evidence and enforced only
  under `--require-clean` (`PACKAGE_TREE_DIRTY`); the smoke runs during
  development on a dirty tree and says so.
- The demo also executes the Function on the clone and requires the same
  report record, and runs `gc.dry_run` on both repositories requiring no
  deletion candidate; the runner's environment is exactly `PATH` and
  `LANG`, and every `sley` invocation runs from inside the unpacked
  artifact.
- The artifact holds fourteen members; the stages and second artifact are
  removed after comparison unless `--keep` is given, and only
  `dist/sley-2.0.0-linux-x86_64.tar.gz` remains under the ignored `dist/`.
- Reproducibility was established byte for byte on the first clean run
  after the remap fix; a later archive-only difference is recorded, never
  rounded.

## 12. Revision 3 clarifications

- The three remaps run most-general-first (home, registry, tree) because
  rustc applies the last matching rule: the revision 2 order let the home
  rule shadow the other two, and the binary carried `/home-remapped` paths
  the scan's needles could never match. The scan now names the residue and
  the username, and offline tests pin the order.
- Cleanliness is no longer recorded-but-optional: tracked evidence
  requires `--require-clean`, the manifest carries `working_tree_clean`
  inside its digest, and the revision 2 sentence blessing dirty smokes is
  superseded.
- The manifest carries `ga_claimed: false`, `publication_authorized:
  false`, and the blockers inside its digest, so the artifact states its
  non-release status by inspection; `LICENSE-PENDING.txt` remains the
  human-readable statement.
- Section 4 now enumerates the demo's nine methods and its 20.12 verbs
  (four covered, four residual) instead of claiming every dispatched
  operation; open question 1 is answered yes, with the mechanism
  (`workspace.create` from a packaged trusted genesis) named as the demo
  extension that moves create out of the residual.

## 13. Round-7 clarifications (2026-09-11; gate semantics unchanged)

- Section 7 names the canonical detached linked worktree procedure for
  minting while the operator tree carries retained untracked material:
  the procedure changes where the mint runs, not what `--require-clean`
  requires.
- Section 7 states the register binding rule the checker enforces:
  `candidate_*` must name the tracked attestation
  (`candidate-attestation-mismatch`).
- Section 1 step 4 names all five content needles (section 5 was already
  exact); failures record the full partial evidence with the failure
  attached, the invocation is recorded on every path with accepted flags
  only, both builds' toolchains are recorded, and the version check names
  the CLI and protocol versions.

## 14. Licensed-candidate revision (2026-09-14; gate semantics unchanged)

- Under the S20-710 license decision (Apache License, Version 2.0), the
  staged artifact ships the installed `LICENSE` and `NOTICE` as members
  (section 2) and no `LICENSE-PENDING.txt` placeholder is staged; the
  revision 3 sentence naming the placeholder as the human-readable
  statement is superseded for candidates minted after the decision.
- Staging derives the member set from the T52 inventory and refuses
  (`PACKAGE_INTERNAL_INVARIANT`) unless the inventory names exactly
  `LICENSE` and `NOTICE`, every workspace package carries the approved
  disposition, and the staged bytes match the inventoried digests.
- `LICENSES.json` records the approved root-license status with the
  SPDX identifier and member digests instead of the pending blocker.
- The mint blockers drop `root_license_text_operator_approval`; the
  remaining blockers still ride inside the manifest digest.

## 15. Artifact-content evidence (revision 5, 2026-09-15)

S20-720 owns `scripts/build_candidate_content_report.py` and the tracked
`evidence/release/candidate-content-checks.json`. The report contract is
`sley2.candidate-content-checks.v1`. It has exactly these fields:

- `contract`: the tag above;
- `commit`, `artifact_sha256`, `manifest_digest`, `artifact_size_bytes`: the
  selected admissible attestation's candidate identity;
- `checks`: exactly `manifest` and `forbidden_content`, both booleans;
- `result`: `PASS` iff both checks are true, otherwise `FAIL`;
- `report_digest`: SHA-256 of the JSON body excluding this field, serialized
  with sorted keys, two-space indentation and a trailing newline.

Selection binds the S20-730 owner predicate to the local S20-720 build
record, including during the old-secondary/new-primary handoff. The
archive's SHA-256 and size must match. The exact regular-file set is the
packaging module's `expected_artifact_members`: its owned fixed files plus
all tracked files under the section 3 subset at the candidate's Git commit.
Staging and inspection consume this same owner. Duplicate regular files,
unexpected/missing directory entries, and any special member (including
symlinks and hardlinks) refuse before extraction. The unpacked manifest
must verify all members and bind the same commit and manifest digest.
Section 5's forbidden-content scan runs on the unpacked bytes.

Input, archive-shape, manifest/binding and tracked-report-drift failures
return exit 1 and `72001 PACKAGE_MANIFEST_INVALID`, without replacing the
tracked report. A completed scan with forbidden content records `FAIL`,
returns exit 1 and `72002 PACKAGE_CONTENT_FORBIDDEN`. Both CLI failure
forms include `contract`, `result`, numeric `code`, and `symbol`; input
failures also include `error`. Success returns exit 0. No new codes are
reserved. A failed or stale report cannot evidence the GA content criterion.

`make release-candidate-build` creates the archive and report.
`make release-candidate-verify` and Tier 1 `make quick` run the report's
`--check`: both require the matching local `dist/` archive, the gitignored
S20-720 build evidence, and the tracked S20-730 report. A clean clone first
builds the candidate and completes the second-host merge before final
verification. The split preserves build-before-verify ordering even under
parallel make. The machine summary's `candidate_content_report` and
`candidate_content_checker` point to the owned files; the packaging checker
binds those pointers and this contract section.

## 16. Lint-report binding (revision 6, 2026-09-18; gate semantics unchanged)

The c04539b9 Council round (Nabu P3) found the checker enforcing a lint
report binding the contract named nowhere. Revision 6 records it: the lint
report names the tree it linted (`commit`, and `lint_inputs_clean` — the
porcelain state of `crates/`, `.cargo/`, `Cargo.toml`, `Cargo.lock`,
`rust-toolchain.toml`, `clippy.toml`, `rustfmt.toml` when recorded, with
`dirty_lint_inputs` listing any dirty entry and `working_tree_clean` the
whole-tree state), and the packaging checker requires `commit ==
candidate_commit`, `lint_inputs_clean == true` and `result == PASS`
(`lint-report:commit-differs-from-candidate`, `lint-report:not-a-clean-pass`).
The report is therefore recorded at the candidate commit with only
records dirty, in the records-only descendant, after `make lint`. Section 10
names its owner.
