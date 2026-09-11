# Release Candidate Packaging v1

Status: S20-720 contract draft, revision 3 (2026-09-05); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review). Revision 2 records the clarifications found while implementing
revision 1 (section 11). Revision 3 orders the remaps most-general-first,
describes the manifest's non-release status, requires a clean tree for
tracked evidence, and enumerates the demo's 20.12 verbs honestly
(section 12). The mechanics are `scripts/build_release_candidate.py`
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
   member for the source tree path, any `/home/` path, and the bounded
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
  LICENSES.json                     declared licenses per package and the root-license blocker
  LICENSE-PENDING.txt               the operator-approval blocker statement
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
   manifest's `working_tree_clean` flag names what was built. Where the
   operator tree carries retained untracked material that must not be moved,
   minting uses the canonical detached linked worktree procedure
   (`REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md` section 2): the mint
   runs in a worktree that is whole-tree clean by construction, and the
   `--require-clean` gate semantics are unchanged.
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
