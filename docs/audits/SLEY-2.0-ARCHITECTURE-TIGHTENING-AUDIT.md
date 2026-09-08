# Sley 2.0 Architecture Tightening Audit

Status: IN_PROGRESS (activated 2026-09-08 06:52 UTC). Governing specification:
`/home/greyforge/machineresearch/SLEY-2.0-ARCHITECTURE-TIGHTENING.md`
(sha256 `861e57f9e35de315bed88ee8bc2e946854b8a60bdeaf897f6bc1e9e9ccb805ac`).
Campaign branch: `arch/tighten-r1`, worked in a pinned worktree so the two
in-flight lane sessions on `main` are never read while moving.

Every finding below carries exactly one disposition from the spec's section 3
table. Nothing is implemented without a classification, and a concern the
repository already resolves is recorded as `A_ALREADY_SOLVED` with proof.

## 1. ARCH_TIGHTEN_BASELINE (AT-G0)

```text
ARCH_TIGHTEN_BASELINE:
  canonical_repo:                 /home/greyforge/sley2
                                  origin https://github.com/GreyforgeLabs/sley.git (PUBLIC)
  canonical_branch:               main
  baseline_commit:                560a5f16ebe9edaaee6837b779f6af94ad9ae310
  baseline_parent:                5044168a448b46de22b29189d039bd1cc3b3056a
  baseline_subject:               rw-080 slice 7 record: Namespace body Sley decode/encode
                                  C0 construction record (2026-09-08 02:43 -0400)
  reweave_master_commit_or_digest:
    path:    /home/greyforge/machineresearch/SLEY_2X_REWEAVE_MASTER_SPEC_V1.md
    rev:     REWEAVE-1.0 (2026-09-05)
    sha256:  61d20471906b00fab05a9e0f174ad9c616e734a5f38fd590ed44af2533535a63
    mirror:  /home/greyforge/machineresearch/SLEY_REWEAVE_MASTER_PACKAGE_V1/
             sley-reweave-v1/SLEY_2X_REWEAVE_MASTER_SPEC_V1.md (byte-identical)
    adopted: docs/adr/ADR-0049-reweave-scope-adoption.md (2026-09-06)
  sley_2_0_master_commit_or_digest:
    path:    /home/greyforge/machineresearch/Sley2.0mastergoal.md
    name:    Sley 2.0 Machine Genesis (Black Loom)
    sha256:  e1e153468316d56e111af0f6f3bffa0d66601e3f5e2fe898a5af7fdfb14fc719
    in-repo: machineresearch/sley-2.0/00..25-*.md dossier chapters,
             machineresearch/sley-2.0/machine-summary.json
  active_schema_epoch:            SSMC1 epoch 1, bootstrap SchemaEpochId
                                  ae5b235713b46c04f73c1decd0fb0bb57c5557d0fe89dae7ddac4a7dba25564e
                                  (conformance/schema-epoch/v1/bootstrap.json sha256
                                  6c1c4396b99bb57f3d646fe5938f5393e606c8a5323f609d48c0bfaad4e1223a)
  active_host_abi:                HOST_ABI_V2, contract sley2-host-abi-2, digest
                                  bc564653302a73eb5f998427250a2bb7cd87f5685ef12619bd4ae1f1b2af70d5
                                  (HOST_ABI_V1 preserved byte-identical as history)
  active_exec_package_version:    EXEC_PACKAGE_V2, contract sley2-exec-package-2, digest
                                  f4958c5e3d57762173b881288b008af17d45b5f07a431fcc442d9eec5770da94
  active_bootstrap_profile:       BOOTSTRAP_PROFILE_2, contract sley2-bootstrap-profile-2, digest
                                  fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459
                                  (bootstrap-manifest.json still records P = BOOTSTRAP_PROFILE_1;
                                  see the spec-synchronization findings)
  self_host_claim_level:          SH0 (bootstrap-manifest.json: S, C1, C2, C3 do not exist;
                                  C0 image not yet snapshotted; RW-080 slices 1..7 PROVISIONAL)
  tree_state:                     CLEAN at baseline_commit; one untracked local-only directory
                                  `.forge/slices/` (slice-contract scratch, not repository truth)
  remote_state:                   origin/main = 1d573a07066e29ac6e96c6dfa72c46e086365031
                                  (90 commits, sanitized public mirror v2.0.0-alpha.1,
                                  pushed 2026-08-30T06:13:05Z); NO merge base with main;
                                  590 local commits never pushed to any remote
  open_reviews:                   finding register FINDING_REGISTER_OPEN (218 obligations,
                                  69 open reviews, 10 deferred, 2 unclassified);
                                  RW-080 BLOCKED pending Nabu round-12, premium round 2,
                                  S20-780 independent acceptance; S20-770 lane under
                                  Council re-review (lane2 branches, 14 to 21 commits
                                  ahead of an older main)
  open_gates:                     R2 exit NOT_READY; C0->C1->C2->C3 succession not started;
                                  release-candidate-smoke re-attest pending since S20-330
                                  closure (RESUME.md 2026-09-05); five operator gates
                                  (narrowed schema-epoch decision, succession trials,
                                  root license text, second-host attestation, release
                                  decision); publication_authorized = false
```

## 2. Activation boundary and exclusions

Two Sley 2.0 sessions were still committing when this campaign activated
(last `main` commit 02:43 local, last lane worktree commit 02:41 local,
activation 02:52 local). Per spec section 1 they are not interrupted:

- **Lane 3, RW-080 (REWEAVE codec construction slices).** Commits land on
  `main` directly plus scaffold branches `lane3/rw080-*`. Owned surface:
  `crates/sley-vm/tests/rw080_*.rs`, `machineresearch/sley-2.0/reweave/rw-080-*.md`.
  EXCLUDED from this campaign's edit scope. Audit reads only the committed
  baseline.
- **Lane 2, S20-770 (required-contract index enforcement).** Branches
  `lane2/s20770-*`, not yet merged. Owned surface:
  `docs/spec/REQUIRED_CONTRACT_INDEX_V1.md`, `scripts/check_required_contract_index.py`.
  EXCLUDED from this campaign's edit scope; findings that touch the index are
  recorded for that lane rather than edited here.

This campaign edits only the paths it owns: `docs/audits/SLEY-2.0-ARCHITECTURE-TIGHTENING-*`,
new files under `scripts/` and `docs/status/` named in accepted findings, and
the exact spec or contract patches of accepted `B`/`C` findings, each in its
own commit. Merge into `main` happens by fast-forward or plain merge only,
after the lane sessions have produced stable handoffs.

## 3. Finding register

Populated per audit area as each mandatory audit closes. Record format is the
spec's section 18 block. Identifiers: `AT-SI` semantic identity, `AT-EC`
executable closure, `AT-NA` native authority, `AT-IG` identity graph, `AT-HH`
hash hygiene, `AT-MW` machine write, `AT-CL` conformance layers, `AT-BG`
BLACKGLASS, `AT-WB` Witness boundary, `AT-SS` spec synchronization, `AT-RF`
remote freshness.

### 3.1 Remote freshness and inspectability (spec sections 15 and 16)

```text
FINDING_ID: AT-RF-01
TITLE: The GitHub remote holds an unrelated sanitized lineage; the active Sley 2.0 history has never been pushed anywhere
HYPOTHESIS: The active development branch is remotely inspectable at validated commit boundaries.
REPOSITORY_EVIDENCE:
  git merge-base main origin/main -> none (local root 3a0fd1b4, remote root ca6ef13c)
  git rev-list --left-right --count main...origin/main -> 590 ahead, 90 behind
  origin/main 1d573a07 "docs: establish canonical Sley 2 identity"; 9bb8eb88
    "chore: authorize sanitized public source mirror (v2.0.0-alpha.1)" (2026-08-28)
  gh repo view GreyforgeLabs/sley -> visibility PUBLIC, pushedAt 2026-08-30T06:13:05Z
  origin/main:repository-policy.json forbids "private evidence",
    "machine-local absolute paths", "operator-only authority files";
    origin/main:scripts/check_public_repository_policy.py enforces it in CI
  local main: 73 tracked files contain /home/greyforge paths,
    163 Council review transcripts under machineresearch/sley-2.0/reviews/,
    machine-summary.json publication_authorized = false
  scripts/check_remote_consistency.py --repo /home/greyforge/sley2 --branch main ->
    HISTORY_RELATED = False, RESULT = FAIL (UNRELATED_HISTORY, BEHIND_UPSTREAM,
    AHEAD_UNDECLARED, UNTRACKED_FILES)
SPEC_EVIDENCE: SLEY-2.0-ARCHITECTURE-TIGHTENING.md 15.2 (remote SHOULD lag by at most one
  bounded unit; handoff commit MUST be on GitHub unless a documented safety reason prevents
  it), 15.3 (never push private qualification material; never claim GitHub is current when
  validated commits are local only), 15.7 (private repository requires an authorized
  connection; the project MUST NOT weaken privacy to improve review convenience);
  ~/machineresearch/reference/open-source-rules.md pre-push scrub checklist
CURRENT_BEHAVIOR: No remote reviewer can inspect any of the 590 active-lineage commits.
  The only remote is a public mirror cut by history rewrite on 2026-08-28, nine days and
  590 commits stale, and the repository's own public policy forbids pushing the active
  lineage to it unsanitized.
DESIRED_INVARIANT: The active branch and every validated handoff commit resolve on a GitHub
  remote whose visibility matches the content, with the full SHA citable in review packets.
DISPOSITION: C_PRE_FREEZE_REPAIR
RATIONALE: A material remote-inspectability defect that section 15 names as an engineering
  requirement. It is not solvable inside this session: a plain push is forbidden by the
  recorded public policy, and both remedies are outward-facing decisions reserved to the
  operator. The documented safety reason (15.2) for the current gap is recorded here.
  Smallest correct repair, recommended: create a PRIVATE GitHub repository for the active
  lineage (for example GreyforgeLabs/sley2), set it as the upstream of main, push main and
  arch/tighten-r1, and keep GreyforgeLabs/sley as the sanitized public mirror refreshed only
  by the existing rewrite pipeline under a separate publication decision. Alternative:
  refresh the public mirror now, which is a publication act and stays gated by
  publication_authorized = false.
CANONICAL_IMPACT: none (repository hosting only)
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: operator authorization for the private remote (or for a mirror refresh);
  then `git remote` reconfiguration and a first push; no history rewrite of main
TESTS_REQUIRED: scripts/check_remote_consistency.py PASS on main after the push
  (HISTORY_RELATED = True, BEHIND_COUNT = 0); public-policy checker stays green on the mirror
SPECS_TO_UPDATE: remote-head locator (AT-RF-03) once the remote exists; RESUME/handoff packet
CODE_OWNERSHIP: operator (remote creation); campaign integrator (remote configuration, push)
REVIEW_REQUIRED: operator decision; AT-G7 cannot pass before it
```

```text
FINDING_ID: AT-RF-02
TITLE: No repository check reported LOCAL_HEAD, UPSTREAM_HEAD, ahead/behind, dirty state and untracked files at handoff
HYPOTHESIS: A lightweight consistency gate exists (spec section 16).
REPOSITORY_EVIDENCE: `ls scripts | grep -iE 'remote|git|status'` at 560a5f16 -> only
  scripts/gate_status.py, which prints a fixed NOT_IMPLEMENTED record and exits 2; no
  Makefile target inspects the remote relation; handoffs in RESUME.md cite commits without
  push status.
SPEC_EVIDENCE: SLEY-2.0-ARCHITECTURE-TIGHTENING.md 16 (required fields; expected handoff
  state; AHEAD_COUNT > 0 only when declared; the checker SHALL NOT auto-push).
CURRENT_BEHAVIOR: Remote state was discovered by hand and the 590/90 unrelated-history gap
  went unreported in every handoff since 2026-08-28.
DESIRED_INVARIANT: One deterministic, read-only command answers the section 16 fields and
  fails closed on dirty tree, behind upstream, undeclared ahead, unrelated history, missing
  upstream, or untracked relevant files.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Purely additive tooling with no canonical, schema, ABI or self-host effect; the
  audit itself needed it to state the baseline truthfully.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: none
TESTS_REQUIRED: `python3 scripts/check_remote_consistency.py --self-test` (8 scenarios in
  throwaway repositories: clean pushed, dirty, ahead undeclared, ahead declared, untracked,
  behind, unrelated history, no upstream) -> SELF_TEST PASS; live run on main reproduces
  the AT-RF-01 state
SPECS_TO_UPDATE: none (Makefile target `remote-consistency`, kept out of `quick` because a
  branch is legitimately ahead mid-slice; run at handoff)
CODE_OWNERSHIP: scripts/check_remote_consistency.py, Makefile target remote-consistency
REVIEW_REQUIRED: independent review at campaign close (AT-G8)
```

### 3.2 Semantic identity decomposition and executable closure (spec sections 4.1 and 5)

Inventory tables and the answers to the five 4.1 questions are in section 4
below; the records follow. Targeted tests at the baseline: sley-state-root
12/12, rw075_exec_closure 30/30, admission_authority 2/2.

```text
FINDING_ID: AT-SI-01
TITLE: StateRoot is proposed to be split into ProgramRoot / VerificationRoot because it binds tests, contracts and policy alongside executable entities
HYPOTHESIS: The canonical StateRoot conflates executable program identity with verification and policy state, so a new ProgramRoot / ExecutableSemanticRoot is needed for correct executable-image binding.
REPOSITORY_EVIDENCE: crates/sley-state-root/src/lib.rs:124-143 (nine fields, no compiler or execution metadata); lib.rs:1006 (every field changes the root); crates/sley-vm/src/lower.rs:838-882 (image bytes contain no root); crates/sley-ssmc/src/fingerprint.rs:312-320 (fingerprint preimage has epoch only); crates/sley-vm/src/exec_package.rs:253,896-912 (image_digest root-free; package_digest binds root as one preimage field); crates/sley-vm/tests/rw075_exec_closure.rs:539-555 (different root is a different closure by design).
SPEC_EVIDENCE: Sley2.0mastergoal.md:644-646 (StateRoot is the complete accepted semantic program state) and 678-692 (5.3 requires policy, test-set and contract-set roots in the state root); docs/spec/STATE_ROOT_V1.md:7-8,69-79,102-116; docs/spec/EXECUTION_MODEL_V1.md:12-14 (execution request binds exact root and entry point); REWEAVE master 11 (lines 330-361: toolchain has retained root plus executable image digest); SLEY-2.0-ARCHITECTURE-TIGHTENING.md 4.1 decision rule.
CURRENT_BEHAVIOR: The root binds accepted workspace state exactly as the master requires. Executable identity is already carried by root-free image digests and semantic fingerprints; execution-closure identity by the EXEC_PACKAGE_V2 package digest, which binds the root as provenance.
DESIRED_INVARIANT: One root for accepted semantic state; root-free identities for executable bytes and judged semantics; one package identity binding both plus provenance. This invariant already holds.
DISPOSITION: D_REJECT
RATIONALE: None of the five justifying problems in the 4.1 decision rule is demonstrated: image binding is correct (SHA-256 of root-free bytes), unchanged executable semantics are expressible (fingerprints, image digest), invalidation is bounded to re-admission with no cache to invalidate, self-host identity is specified by RW-080 section 2 without a new root, and policy/verification state is bound in the workspace root by explicit master requirement rather than by accident. Adding a root would be decomposition for its own sake, which 4.1 forbids.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none (existing lib.rs:1006 and rw075_exec_closure.rs:539-555 remain the evidence)
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-state-root (ariadne); sley-vm exec_package (ariadne)
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-SI-02
TITLE: Root-bound bytecode cache key and package digest invalidate unchanged executables when tests, contracts or policy change
HYPOTHESIS: Because StateRoot enters the cache key, package digest and observation, any test, contract or policy change forces excessive or incorrect invalidation of unchanged compiled output.
REPOSITORY_EVIDENCE: crates/sley-vm/src/lib.rs:202-231 (cache preimage includes state_root at 217); exec_package.rs:706,911 (root in dependency section and package preimage); execute.rs:738-749 (package path re-derives the key and refuses mismatch; there is no cache load); execute.rs:989-1000 (SLEYPOBS1 observation binds root); no persistent bytecode or package store exists in crates/ (grep for cache-hit load returns only the documented absence); admission cost is one gate plus one reference lowering per package (admission_authority.rs:166-169).
SPEC_EVIDENCE: docs/spec/VM_EXECUTION_PROFILE_V1.md:23-27 (no cache-hit load in this profile; a future cache must repeat epoch/root/function/profile validation); docs/spec/VM_LOWERING_PROFILE_V1.md:39-40,171-172 (bytecode is derived, disposable; a cache hit never substitutes for validation or root binding); docs/spec/HOST_ABI_V2.md:45-49 (cache key deliberately excludes image digest, profile digest, limits and import manifest, v1 rationale preserved); Sley2.0mastergoal.md:694-711 (compiled bytecode caches are derived state that must be reproducible or discardable); docs/spec/EXECUTION_MODEL_V1.md:8-14.
CURRENT_BEHAVIOR: Root changes change the cache key, package digest and observation identity; image bytes, image digest and fingerprints do not change. Nothing is invalidated because nothing is cached; a new root requires a new admission, which the design requires anyway because the root is the provenance of the closure.
DESIRED_INVARIANT: Derived execution artifacts are keyed to the exact root they were judged under and are never reused without repeating that validation. Holds.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: The concern presupposes a reuse surface that does not exist. The contracts state the root binding as intentional and the only future cache rule (repeat validation before reuse) is already frozen. Record for 2.1+: if a persistent package or bytecode cache is ever introduced, `image_digest` plus `gate_closure_fingerprints` already provide a root-free reuse key; no new identity is needed then either.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-vm (ariadne)
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-SI-03
TITLE: Self-host toolchain identity S needs a dedicated program root
HYPOTHESIS: The REWEAVE bound object S (immutable canonical root plus complete dependency closure of the toolchain program) cannot be expressed with the existing StateRoot and package identities, so a ProgramRoot is required before the SH2 fixed-point gates.
REPOSITORY_EVIDENCE: machineresearch/sley-2.0/reweave/rw-080-contract.md:250-262 (S defined as toolchain graph root, complete object closure, runtime data/layout closure carried as package sections, declared entry points, build manifest with expected digests, profile, epoch, root, and exact P/H digests); rw-080-contract.md:264-274 (construction manifest per entity with provenance); crates/sley-vm/src/exec_package.rs:896-912 (package binds root, entry, epoch, profile digest, ABI version, VM version and every section digest); lower.rs:838-882 (fixed-point payloads are root-free image bytes); docs/spec/BOOTSTRAP_PROFILE_2.md:39-42,126-130 and conformance/bootstrap-profile/v2/profile.json `schema_epoch_hex`/`state_root_hex` (closure evidence root convention 09*32).
SPEC_EVIDENCE: SLEY_2X_REWEAVE_MASTER_SPEC_V1.md 13.1 (lines 377-395: S, P, H, C1..C3 relation), 13.2 (byte-identical executable payloads, entry mapping, manifests), 11 (lines 330-361: toolchain executable identity is a retained root plus image digest, never a path); SLEY-2.0-ARCHITECTURE-TIGHTENING.md 4.1 decision rule (self-host identity ambiguity).
CURRENT_BEHAVIOR: S is specified as a composition of existing identities (StateRoot plus package plus manifest digests). It is not yet instantiated because RW-080 is contract-only and BLOCKED on the R2 gate (rw-080-contract.md:3-8). The fixed-point comparison is over image bytes and manifests, which are root-free, so a toolchain root change does not perturb C2/C3 equality.
DESIRED_INVARIANT: The toolchain program identity is one accepted StateRoot whose entry points name the four module mains, bound to P and H through the package digest; the fixed-point gate compares root-free executable payloads. Already specified.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: The identity decomposition needed for S exists in frozen contracts and code; only the RW-080 instantiation is pending, and that is lane-owned work gated on R2, not an architecture defect. Introducing a ProgramRoot now would create a second identity for the same object before the first is instantiated.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none now; RW-080 must record which StateRoot is S when it constructs the toolchain graph
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none in this audit; RW-130/RW-140 own the fixed-point tests
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: RW-080 lane (ariadne, nabu)
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-SI-04
TITLE: The package state_root is a declared value, not chain-verified against the root's entity bindings inside sley-vm
HYPOTHESIS: Because admit_v2_package only compares package.state_root with the caller-supplied closure.state_root, a package can carry a root whose bindings do not contain the closure, so root-to-executable binding is incorrect and a stronger root is needed.
REPOSITORY_EVIDENCE: crates/sley-vm/src/admission_authority.rs:72-97 (V2Closure carries state_root as a caller-supplied field) and 262-264 (equality check only); exec_package.rs:229,706,911 (root carried by value); crates/sley-protocol/src/server.rs:2011-2070 (the production execute path derives the closure from the bound revision via CompleteRootRequest::extract and passes request.root(), so root and closure are bound by construction at the repository layer); crates/sley-state-root/src/lib.rs:363-381 (recompute_root ties root-backed query facts to the claimed root); crates/sley-repo/src/root_query.rs:122-123 (record roots surfaced from the accepted revision); tests/rw075_exec_closure.rs:539-555 (root mismatch refuses).
SPEC_EVIDENCE: rw-080-contract.md:250-274 (S and the construction manifest bind every toolchain entity with provenance; the driver resolves the exact input closure) and 128-165 (the build driver, not the VM, owns closure resolution); docs/spec/EXEC_PACKAGE_V1.md:44-52 (entry designation and dependency-closure membership are compiler-owned, bound by digest, trusted via receipt); docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:26-34 (a digest authenticates bytes; provenance comes from the build path); REWEAVE master 11 (native boundary binds a verdict to toolchain image, candidate, base root, policy, epoch and evidence; it does not reinterpret).
CURRENT_BEHAVIOR: sley-vm binds the root by value and by digest; the repository and driver layers own the proof that a closure was extracted from that root. Today the only production execution path reads the closure from the revision the root names. The package path is exercised with the fixed evidence root 09*32 in conformance and tests.
DESIRED_INVARIANT: The layer that extracts a closure from an accepted root is the one that proves it (repository / build driver); the VM verifies bytes and digests only. Holds; the package-path instance of the proof is the RW-080 manifest.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: Chain-verification of root to closure is deliberately a repository and driver responsibility, already implemented for the production path and already specified for the toolchain path. Moving it into the VM would give native code a semantic answer (closure membership) that the boundary forbids. No new root would change this.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: RW-080 must produce the manifest that binds S's closure to the package (already in its contract)
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none now; RW-080 owns provenance-audit tests (rw-080-contract.md:300-311)
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-repo / sley-protocol (ariadne); RW-080 lane
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-EC-01
TITLE: One canonical package binds the complete information required to execute a compiled Sley program
HYPOTHESIS: Sley 2.0 lacks a single versioned package or root that binds image, constants, layouts, entry, imports, runtime inventory, compiler metadata, epoch, ABI, profile, closure and hashes.
REPOSITORY_EVIDENCE: crates/sley-vm/src/exec_package.rs:211-247 (ExecutionPackage fields), 697-797 (dependency section: entry, epoch, root, VM and lowerer versions, profile fields, gate counts, closure fingerprints, admitted limits, globals, contracts), 873-921 (package_digests_v2 preimage), 1253-1308 (verify_package_binding_v2); execute.rs:707-769 (execution consumes exactly that binding); conformance/exec-package/v2/exec-package.json `approved_binding.fields` (seventeen bound fields) and `envelope.package_digest_preimage`; section 2 inventory table above.
SPEC_EVIDENCE: docs/spec/EXEC_PACKAGE_V2.md:33-66; docs/spec/EXEC_PACKAGE_V1.md:74-121 (bindings and residual assumptions); docs/spec/HOST_ABI_V2.md:35-49,88-99; docs/spec/BOOTSTRAP_PROFILE_2.md:37-42,126-130; SLEY-2.0-ARCHITECTURE-TIGHTENING.md 5 inventory list and preferred target shape.
CURRENT_BEHAVIOR: Every item in the spec 5 inventory is bound by EXEC_PACKAGE_V2 (section 2 table), with two deliberate representation choices: host ABI identity is bound by frozen version number (see AT-EC-05) and the package digest is over a header of section digests rather than over serialized envelope bytes (see AT-EC-04).
DESIRED_INVARIANT: ExecutionPackageRoot = HASH(domain || canonical package identity fields), covering image, constants, layouts, entry, imports, runtime inventory, compiler metadata, epoch, ABI, profile, closure. Holds as package_digest.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: The preferred target shape in spec 5 is naming, not semantics, and every semantic item is already bound by digest under an explicit version (v2) with v1 preserved byte-identical, exactly as 2.2 requires.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none (rw075_exec_closure 30/30 passing at the pinned commit)
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-vm exec_package (ariadne)
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-EC-02
TITLE: Native re-derivation on the package execution path is structural only
HYPOTHESIS: execute_approved_package_v2 derives semantic information that belongs to the Sley compiler (type environment construction, constant judgment, lowering) instead of consuming the package.
REPOSITORY_EVIDENCE: crates/sley-vm/src/execute.rs:707-769 (steps: package_digests_v2, verify_package_binding_v2, limits equality, load_image, hydrate_layouts, derive_cache_key, validate_package_inputs_structural, execute_core_package) and 783-810 (structural input checks: count, value_type == register_type, canonical form, value units, hash_validated_value); exec_package.rs:1167-1173 and crates/sley-check/src/lib.rs:267-278 (hydration is count plus duplicate only); crates/sley-ssmc/src/fingerprint.rs:282-307 (value hash is encoding plus BLAKE3, no environment judgment); crates/sley-vm/src/extended.rs:654-699 (constant_ref and global_get resolve through package tables); scripts/check_exec_package_markers.py (pins structural validation function, SLEYPOBS1, hydrate_verified_definitions, and absence of semantic services); execute.rs:612-617 (semantic constructors are not rerun; declared in code).
SPEC_EVIDENCE: docs/spec/EXEC_PACKAGE_V1.md:54-72 (native allowed and forbidden operations); docs/spec/EXEC_PACKAGE_V2.md:68-73 (hydration unchanged); conformance/exec-package/v2/exec-package.json `hydration.allows` and `hydration.forbids`; machineresearch/sley-2.0/reweave/rw-070.md:25-28 (value_hash, cache-key and observation derivation are inventoried host runtime mechanics); REWEAVE master 10.3 (native remainder: bounded execution, allocation, resource accounting, primitive hashing, structural memory-safety validation).
CURRENT_BEHAVIOR: The execute path performs digest verification, framing decode, exact-equality binding, count and duplicate hydration, structural type equality, canonical-form codec checks, value hashing for evidence, and bounded execution. No TypeEnvironment::new, check_constant, require_hashable, lower_function or judge_bootstrap_profile call exists on it.
DESIRED_INVARIANT: The native executor validates and consumes a compiler-produced package and never produces a language verdict. Holds.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: Every re-derived quantity on the execute path is a byte, digest, count, equality or resource fact; the value hash of runtime inputs is an execution-evidence mechanic that REWEAVE 10.3 assigns to the native remainder and RW-070 inventoried.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none (marker scripts and rw075_exec_closure already pin this)
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-vm execute (ariadne)
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-EC-03
TITLE: admit_v2_package's reference re-lowering and profile judgment are native semantic authority, declared as C0 seed
HYPOTHESIS: The reference re-lower derived internally in admit_v2_package is a hidden native semantic authority that decides builder faithfulness and profile membership per package, contradicting the Sley-owned lowering requirement.
REPOSITORY_EVIDENCE: crates/sley-vm/src/admission_authority.rs:1-47 (module declares C0 seed path, permanent mechanics, reserved Sley ingress, exclusivity), 157-177 (admit_v2_package: judge, re-lower, structural compare, mint), 182-204 (judge_closure_for_seed calls judge_bootstrap_profile), 210-230 (reference_lower_for_seed calls lower_function), 243-245 (receipt refused unless reference bytes equal package bytes), 294-325 (SleyAdmissionEvidence sealed with no constructor; ingress always refuses SleyEvidenceUnavailable); exec_package.rs:953-959 (admit_package_v2 is pub(crate), exclusive minter); scripts/check_exec_package_markers.py (only exec_package.rs and admission_authority.rs may name the minter; execution, raw-hash, host-ABI and bridge modules carry no gate or lowerer calls); crates/sley-protocol/src/server.rs:2011-2070 (production SMP1 execute still lowers natively from the revision).
SPEC_EVIDENCE: rw-080-contract.md:142-165 (1.4: C0 seed performs judgment plus reference re-lowering as oracle evidence, excluded from clean stages; post-C1 the Sley driver replicates the comparison and admission mints only from Sley evidence) and 227-235 (1.6 admission ingress); rw-075-correction.md:315-324 and 414-426 (premium AR-07 raised exactly this and the split was the accepted repair); docs/spec/EXEC_PACKAGE_V1.md:95-99 and 113-116 (builder faithfulness verified by the authority's reference re-lowering; retired by RW-130/RW-140 fixed-point evidence); REWEAVE master 5 (lines 192-213: before promotion the Rust reference owns active checking and lowering; after promotion the approved Sley toolchain owns them) and 10.2 items 2, 4, 5 (checking, lowering and the build driver must execute as Sley programs at SH2); SLEY-2.0-ARCHITECTURE-TIGHTENING.md 6 (each native SEMANTIC_AUTHORITY receives REQUIRED_NATIVE_FOUNDATION with proof or MUST_MIGRATE_TO_SLEY before final freeze) and 2.1 (candidate code may not redefine the judge).
CURRENT_BEHAVIOR: Two native semantic legs (gate judgment and reference lowering) decide whether a v2 receipt is minted. They are confined to one module, named as seed evidence, excluded from clean stages by contract, marker-pinned, and paired with a reserved post-C1 ingress that cannot mint. Nothing on the execution path re-lowers. The premium reviewer's AR-07 finding on this exact point was repaired by the declaration and split; the residual is the migration itself, which requires C1 to exist.
DESIRED_INVARIANT: Native code may act as an oracle before promotion; after C1 the admission verdict derives only from Sley-produced evidence, and native code performs authenticated structural checks. Specified; not yet reachable because C1 does not exist.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: The authority is not hidden and is not conflated with execution: repository truth already classifies it as MUST_MIGRATE_TO_SLEY in substance (C0 seed, excluded from clean stages, post-C1 ingress reserved) and REWEAVE 5 explicitly permits native ownership before promotion. Its retirement is a REWEAVE gate (RW-080 through RW-140, SH2), not a pre-freeze architecture repair. The section 6 audit should record this operation as SEMANTIC_AUTHORITY / MUST_MIGRATE_TO_SLEY with the RW-080 contract as the migration proof, and the same classification applies to the SMP1 execute path's native TypeEnvironment::new plus execute_function.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: the SH2 claim depends on completing this migration; AT-G3 and AT-G6 must cite RW-080 1.4 as the boundary proof rather than treating the seed path as foundation
MIGRATION_REQUIRED: yes, already scheduled (RW-080/RW-120 post-C1 ingress); no new migration introduced by this audit
TESTS_REQUIRED: none new now; admission_authority unit pins (2/2) and rw075_raw_callable authority negatives already exist
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-vm admission_authority (ariadne); RW-080 lane
REVIEW_REQUIRED: independent review already recorded (Nabu rounds 7-11 PASS, premium delta r1 FAIL repaired per rw-075-correction.md 12-13; fresh premium round pending per that record)
```

```text
FINDING_ID: AT-EC-04
TITLE: The package identity is documented in code as a digest over envelope bytes while the contract defines it over the header of section digests
HYPOTHESIS: The executable-closure identity is ambiguously defined (spec section 8: ambiguous inclusion, alternate encodings).
REPOSITORY_EVIDENCE: crates/sley-vm/src/exec_package.rs:263 ("SHA-256 over the complete envelope bytes (the package identity)") and :325 ("SHA-256 over the envelope") versus :896-912 (package_digests_v2 hashes SLEYPKG1 || u32(2) || profile digest || ABI version || VM version || five section digests || entry || epoch || root, never serialized envelope bytes); :143-153 reserve PackageError::UnknownMagic, UnsupportedVersion, Truncated, TrailingData with no constructing path (only Malformed at :406 and :617); no encode/decode function for a package envelope exists in crates/sley-vm; docs/spec/EXEC_PACKAGE_V1.md:77 ("package digest (over the envelope of at most 67_108_864 total bytes") carries the same wording and is frozen byte-identical.
SPEC_EVIDENCE: docs/spec/EXEC_PACKAGE_V2.md:52-55 (package digest preimage is the header of digests); conformance/exec-package/v2/exec-package.json `envelope.package_digest_preimage` (same); SLEY-2.0-ARCHITECTURE-TIGHTENING.md 8 (each canonical identity: prose definition and exact byte preimage with no ambiguous inclusion; master wording that misleads must be clarified even when code is correct) and 2.2 (no silent reinterpretation; V1 text stays immutable).
CURRENT_BEHAVIOR: Code and V1 prose say "over the envelope"; the V2 contract, the record and the implementation hash a fixed 224-plus-byte header. A reader who serialized the sections and hashed them would compute a different value and believe the package digest was wrong. The envelope as a serialized artifact has no frozen byte layout and no codec (see AT-EC-07), so today nothing can even produce "envelope bytes" to hash.
DESIRED_INVARIANT: Every statement of the package identity names the header preimage; the frozen V1 wording is explicitly superseded rather than edited; the reserved framing codes are documented as reserved for the envelope decoder that does not yet exist.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Pure documentation correction of a canonical-identity statement with zero byte effect: the digest, receipts, approval logic and the frozen v2 record f4958c5e... are untouched, and the v2 checker's doc markers remain present. Implemented in this campaign (slice 2): the two doc comments corrected, and an "Identity versus serialization" statement added to EXEC_PACKAGE_V2.md that supersedes the V1 line-77 wording, states that the envelope serialization is not frozen, and reserves the four framing codes.
CANONICAL_IMPACT: none (wording only; preimage unchanged)
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: scripts/check_exec_package_v2.py PASS (doc markers and record digest unchanged); scripts/check_exec_package_markers.py PASS; cargo test -p sley-vm rw075_exec_closure 30/30; cargo fmt --check
SPECS_TO_UPDATE: docs/spec/EXEC_PACKAGE_V2.md (new statement); EXEC_PACKAGE_V1.md untouched (superseded by reference)
CODE_OWNERSHIP: sley-vm exec_package doc comments (campaign integrator, two comment lines)
REVIEW_REQUIRED: yes, in the campaign's independent review (AT-G8)
```

```text
FINDING_ID: AT-EC-05
TITLE: Host ABI identity is bound by version number, not by ABI record digest
HYPOTHESIS: The package binds BOOTSTRAP_PROFILE_2 by digest but HOST_ABI_V2 only by the integer 2, so ABI identity is under-bound in the executable closure.
REPOSITORY_EVIDENCE: crates/sley-vm/src/exec_package.rs:900 (u32 host_abi_version in preimage), 292 and 957 (receipt carries host_abi_version), 1288-1290 (verification requires HOST_ABI_V2_VERSION); conformance/bootstrap-profile/v2/profile.json carries no host ABI digest field; conformance/host-abi/v2/host-abi.json `bindings.bootstrap_profile_digest` binds the profile digest from the ABI side; scripts/check_host_abi_v2.py pins the ABI record digest bc564653... on every make quick; crates/sley-vm/src/host_abi.rs constants HOST_ABI_V2_IDENTITY/CONTRACT/VERSION.
SPEC_EVIDENCE: docs/spec/HOST_ABI_V2.md:6-8 (digest frozen and checker-verified), 16-20 (v2 supersedes v1 with exact relation; extension needs a new owner amendment, never analogy); SLEY-2.0-ARCHITECTURE-TIGHTENING.md 2.2 (a frozen ABI must not be silently redefined; a change introduces a new explicit version).
CURRENT_BEHAVIOR: ABI version 2 maps to exactly one frozen record whose digest is enforced by a repository gate; the package and receipt bind the version; the ABI record binds the profile digest; a content change to the v2 record without a version bump fails the checker.
DESIRED_INVARIANT: The executable closure names an ABI whose content is immutable for that name. Holds through version plus checker-pinned record.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: The version-to-record mapping is frozen and gate-enforced, and 2.2 forbids redefining v2, so the integer is an exact identity. Adding the ABI digest to the package preimage would change the package digest (a v3 package) for symmetry alone, which 4.1 and 19 reject as aesthetic.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-vm host_abi (ariadne)
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-EC-06
TITLE: Reserved SleyAdmissionEvidence fields are narrower than the evidence RW-080 1.6 names
HYPOTHESIS: The reserved post-C1 ingress type omits closure fingerprints and per-table digests that the RW-080 contract and the type's own doc comment list, so the ingress will not be able to carry complete admission evidence when C1 exists.
REPOSITORY_EVIDENCE: crates/sley-vm/src/admission_authority.rs:279-288 (doc lists judged closure identity, gate claims, reference-image digest and complete table digests for constants, type definitions, import rows, globals, contracts) versus 294-305 (fields: closure_digest, operation_count, bridge_uses, image_digest only); 320-325 (ingress always refuses; no constructor; sealed non_exhaustive).
SPEC_EVIDENCE: rw-080-contract.md:227-235 (1.6: Sley-built evidence bytes conforming to the reserved shape: judged-closure digest, gate counts, reference-image digest, complete table digests); rw-075-correction.md:414-426 (AR-07 repair introduced the reservation as an interface, not a minting path).
CURRENT_BEHAVIOR: The type is unconstructible outside its module and the route refuses unconditionally, so the field gap has no runtime effect. The struct is non_exhaustive, so adding fields later is non-breaking.
DESIRED_INVARIANT: When the post-C1 ingress is implemented, its evidence shape carries every claim the structural authenticator compares (closure fingerprints, table digests, entry, epoch, root, image digest, counts).
DISPOSITION: E_DEFER_2_1_PLUS
RATIONALE: Not required for correct Sley 2.0: nothing can construct or consume the type until C1 exists, and the eventual field list is settled by the RW-080 implementation slice that also produces the evidence. Record so that slice aligns the fields with 1.6 and the doc comment; no change now.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: post-C1 ingress implementation must include the missing fields
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none now
SPECS_TO_UPDATE: none now (rw-080-contract.md 1.6 already states the full list)
CODE_OWNERSHIP: sley-vm admission_authority (ariadne); RW-080 lane
REVIEW_REQUIRED: no
```

---

```text
FINDING_ID: AT-EC-07
TITLE: EXEC_PACKAGE_V2 has no canonical byte envelope encoder or strict decoder; packages reach the executor only as in-process structs
HYPOTHESIS: The native executor cannot yet consume a compiler-produced package delivered as bytes, which RW-080 requires of the Sley builder and host loader.
REPOSITORY_EVIDENCE: crates/sley-vm/src/exec_package.rs:84 (EXEC_PACKAGE_MAGIC used only inside digest preimages at :837 and :897); no decode_package/load_package/encode_package in crates/sley-vm; execute.rs:707-711 takes &ExecutionPackage; tests/rw075_exec_closure.rs:115-155 build packages as structs; conformance/exec-package/v2/exec-package.json `envelope.sections` names five sections with per-section encodings but no header layout or length-prefix rule.
SPEC_EVIDENCE: rw-080-contract.md:98-102 and 221-226 (Sley lowerer/builder emits exact EXEC_PACKAGE_V2 envelope bytes; emitted bytes round-trip through load_image and reproduce byte-identically); docs/spec/EXEC_PACKAGE_V2.md:35-40; SLEY-2.0-ARCHITECTURE-TIGHTENING.md 5 (executor validates and consumes a complete compiler-produced package) and 1 (do not interrupt an in-flight REWEAVE slice).
CURRENT_BEHAVIOR: Section encodings and their digests are frozen and implemented; the single serialized envelope has no frozen byte layout and no codec. Every 2.0 execution path (SMP1 execute, package execute) constructs the package in-process, so no 2.0 behavior depends on envelope bytes.
DESIRED_INVARIANT: When the Sley builder exists, one frozen envelope byte layout with a strict host decoder (using the reserved PACKAGE_* framing codes) round-trips packages byte-identically without changing package_digest.
DISPOSITION: E_DEFER_2_1_PLUS
RATIONALE: Not required for correct Sley 2.0: the 2.0 executor consumes structs and the byte handoff exists only to serve the SH2 builder, whose target is the REWEAVE 2.1.0 candidate. The emitter and the decoder are the RW-080 lane's owned deliverable (1.3 and 1.6); freezing a layout here, ahead of the emitter and without vectors on both sides, would create the second independently maintained preimage definition that spec section 7 forbids. Recorded so the lane freezes the layout together with the emitter, the decoder, positive and refusal vectors for each reserved code, and an independent reproduction.
CANONICAL_IMPACT: none now; the lane adds a frozen envelope layout without touching package_digest
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: the RW-080 byte handoff depends on it (already in that contract)
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none now; lane: round trip, refusal fixtures per reserved code, digest reproduction, independent Python reproduction, marker pin
SPECS_TO_UPDATE: none now; lane: EXEC_PACKAGE_V2.md envelope layout section plus record
CODE_OWNERSHIP: RW-080 lane (sley-vm exec_package)
REVIEW_REQUIRED: lane review (Ariadne contract, Vulcan surface)
```

### 3.3 Conformance layering, BLACKGLASS wording, Witness boundary (spec sections 10, 11, 12)

Oracle coverage at the baseline: 9 of the 12 required contracts are checked by
the independent oracle at codec-and-identity depth, none at semantic depth (by
design, master section 6.5); policy root and capability token have native
vectors only; the test-report contract has no corpus with the reason machine
guarded. Witnessed Authority is not part of the active architecture at this
commit (no type, crate, domain, or corpus; host-boundary.json records "WA
inactive"), so section 12 is scope-only until RW-150.


```text
FINDING_ID: AT-CL-01
TITLE: No independent-implementation conformance strata; primary all-or-nothing is deliberate
HYPOTHESIS: Sley conformance is unnecessarily all-or-nothing, so an independent implementation cannot make a bounded claim.
REPOSITORY_EVIDENCE: docs/spec/REQUIRED_CONTRACT_INDEX_V1.md:22-35 (mapping only, no prerequisite column); docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:186-208 (depth axis, the only conformance-claim stratification); docs/spec/SMP1.md:106-160 (selected profile: the only runtime partial-capability mechanism); ARCHITECTURE.md:18-33 and crates/*/Cargo.toml (layer prerequisites: sley-policy -> sley-vm, sley-query -> sley-state-root without txn); docs/WORK_PACKAGES.md:18-55 (package DAG); docs/adr/ADR-0044-candidate-operation-analysis-through-the-vm-owner.md:1-3; machineresearch/sley-2.0/reviews/s20-770-{ariadne,nabu,vulcan}-*-2026-09-04.log (index under three FAIL reviews, P1s open); evidence/conformance/independent-conformance-report.json (24 families, 4 semantic, 20 codec_and_identity, 0 native_only)
SPEC_EVIDENCE: Sley2.0mastergoal.md:846-853 (6.5: oracle is for conformance only, not a second kernel); :3169-3190 (27: primary decision states are all-or-nothing); :465-468 (3.9: independent implementations can validate); SLEY_2X_REWEAVE_MASTER_SPEC_V1.md:281-290 (SH0..SH3 claim ladder), :754-770 (24.1), :125 (OVR-14 restricted results remain restricted); SLEY-2.0-ARCHITECTURE-TIGHTENING.md section 10 ("primarily an interoperability/documentation improvement")
CURRENT_BEHAVIOR: The primary implementation's claim is all-or-nothing by master goal 27. Independent checking is stratified only by per-family depth. No document defines what a third implementation may claim, and no third implementation exists. The index maps names to artifacts but carries no ordering.
DESIRED_INVARIANT: Lower layers have exact prerequisites; no unsupported higher-layer claim; partial conformance never weakens canonical semantics; the primary still satisfies the full target.
DISPOSITION: E_DEFER_2_1_PLUS
RATIONALE: No consumer of a strata vocabulary exists at 2.0 and none is a GA criterion (master 16.7, 26, 27). The raw material for exact prerequisites already exists and is machine-enforced (Cargo graph, package DAG, depth axis), so the future document is a derivation, not a design. Adding a prerequisite column to the S20-770 index now would widen an artifact under open P1 re-review. The strata table in section 2.2 of this audit is the recorded recommendation, with the repository-truth corrections: L3 requires L2 (ADR-0044), L4 query does not require L3 transaction, L6 is not definable until RW-150, L7 is already stratified as SH0..SH3.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none; SH claim ladder already exists and is the L7 stratum
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none now; a future strata document would need a checker deriving prerequisites from Cargo.toml and the package DAG so the table cannot drift
SPECS_TO_UPDATE: none now; future: a CONFORMANCE_STRATA document indexed from REQUIRED_CONTRACT_INDEX_V1.md after S20-770 re-review closes
CODE_OWNERSHIP: Ariadne (contract), Codex (oracle and evidence tooling)
REVIEW_REQUIRED: none now
```

```text
FINDING_ID: AT-CL-02
TITLE: Machine summary records 19 fixture families; tracked report and checker read 24
HYPOTHESIS: The machine-readable conformance claim surface disagrees with the tracked report it points to, and the staged checker does not detect it.
REPOSITORY_EVIDENCE: machineresearch/sley-2.0/machine-summary.json:2889-2892 ("fixture_directories": 19, "independently_checked_families": 19); evidence/conformance/independent-conformance-report.json ("fixture_directories": 24, "independently_checked": 24, verified by `scripts/build_independent_conformance_report.py --check` PASS at this commit); scripts/check_reproducibility_and_independent_conformance.py:220-252 (compares contract, adr, checker, report paths, code count, ga_claimed, publication_authorized, implementation_complete; never compares the family counts or result against the report); scripts/check_oracle_independence.py:4-8 docstring still says "Nineteen fixture families"
SPEC_EVIDENCE: docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:9-11 ("implementation state is tracked in the machine summary"); :252-269 (what the staged checker verifies; the summary-to-report count agreement is not listed); Sley2.0mastergoal.md:3078 ("The machine summary and human-readable evidence index must agree")
CURRENT_BEHAVIOR: The summary understates independent coverage by five families while the result string matches. The count was last set at 7579d34 (2026-09-03); the five families added since (git first-add dates, all 2026-09-06) are bootstrap-capability, bootstrap-profile, exec-package, host-abi, and raw-hash. `make quick` passes with the drift present.
DESIRED_INVARIANT: The machine summary's conformance counts and result equal the tracked report's, and the staged checker fails on disagreement.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Demonstrated machine-surface drift on the exact surface a downstream consumer reads to learn what independent checking covers (the same class of defect Vulcan's S20-770 P1 raised for the index). The fix is bounded: update three summary fields and add a count-and-result comparison to the staged checker, plus the stale docstring in check_oracle_independence.py. No canonical, schema, or ABI meaning changes.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: scripts/check_reproducibility_and_independent_conformance.py compares summary fixture_directories, independently_checked_families, native_only_families, and independent_conformance_result against the tracked report and fails with CONFORMANCE_REPORT_DRIFT (73007) on mismatch; a negative case with a perturbed summary
SPECS_TO_UPDATE: docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md section 8 (add the summary-agreement rule to the staged checker's list); machineresearch/sley-2.0/machine-summary.json; scripts/check_oracle_independence.py docstring
CODE_OWNERSHIP: Codex (S20-730 evidence tooling) per docs/WORK_PACKAGES.md S20-730 row
REVIEW_REQUIRED: Vulcan surface review of the checker delta (Tier 1 targeted); no Council contract re-review needed because the contract rule set gains one clause and no code
IMPLEMENTATION (slice 3): machine-summary.json counts set to the report's 24/24; check_reproducibility_and_independent_conformance.py now fails closed on any mismatch between the summary section and the report for fixture_directories, independently_checked_families, native_only_families and independent_conformance_result (fail-before reproduced on the stale 19s: two report-mismatch problems; none after); stale hard-coded family counts removed from two script docstrings. The checker's two remaining problems (reproducibility-report stale since 84bfa9c9, release-tests:fail) exist unchanged at the baseline and are triaged in section 3.3.1.
```

```text
FINDING_ID: AT-CL-03
TITLE: COMPLETE independent coverage is scoped to fixture families, and the three uncovered required contracts are already disclosed
HYPOTHESIS: INDEPENDENT_CONFORMANCE_COMPLETE could be read as "all twelve required contracts are independently checked" while 17.7, 17.8, and 17.10 are not.
REPOSITORY_EVIDENCE: docs/spec/REQUIRED_CONTRACT_INDEX_V1.md:30-31 ("native vectors in `crates/sley-policy`" for sley-policy-v1 and sley-capability-token-v1), :33 (sley-test-report-v1: no corpus, reason stated), :44-48 (reason must be checkable; the S20-290 checker fails if the VM gains the four resource units); conformance/ directory listing (no policy, capability, or test-report family); evidence/conformance/independent-conformance-report.json (24 families, none of them policy or capability)
SPEC_EVIDENCE: docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:150-151 (every directory under conformance/ is a family), :160-166 (COMPLETE means no family is native-only; does not promise semantic judgment), :288-292 (why COMPLETE keeps its rule); Sley2.0mastergoal.md:846-853 (6.5 requires two independent SCB1 encoders/decoders, nothing wider)
CURRENT_BEHAVIOR: The report's claim boundary is families; the index's corpus column names the native-only status and the missing-corpus reason for the three contracts. Both are honest and machine-checked in their own terms.
DESIRED_INVARIANT: A reader can determine, from tracked artifacts, which required contracts have independent checking and at what depth, without inferring more from COMPLETE than it states.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: The index discloses the three gaps with reasons (rule 3, :44-48), the S20-730 contract scopes COMPLETE to families and records the depth limit, and master goal 6.5 does not require independent checking of policy, capability, or test reports. The cross-reference table in section 1 of this audit is derivable from tracked files. Making the index carry a coverage column belongs with the deferred strata document (AT-CL-01), not with a pre-freeze change.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: Ariadne (index), Codex (report)
REVIEW_REQUIRED: none
```

```text
FINDING_ID: AT-BG-01
TITLE: MPI-0 and its corollaries are mechanism-neutral and protect the deeper invariant
HYPOTHESIS: BLACKGLASS wording accidentally elevates SSMC1, SCB1, or SMP1 (replaceable, versioned mechanisms) above the machine-primacy invariant.
REPOSITORY_EVIDENCE: machineresearch/sley-2.0/reweave/rw-030-charter.md:10-27 (MPI-0 control map; carriers are CONTRIBUTING.md C-01 and docs/ANTI_GOALS.md C-02); docs/spec/SCHEMA_EPOCH_V1.md and docs/spec/EPOCH_MIGRATION_POLICY_V1.md (mechanism replacement path exists as epochs)
SPEC_EVIDENCE: Sley2.0.1mastergoal.md:94-95 (MPI-0 text, no mechanism named); :136-146 (3.2 no canonical source, mechanism-neutral); :158-160 (3.4 states precision, determinism, speed, token cost as the invariant's terms); :164-176 (3.5 no intentional ugliness); :791-793 (19: immature contracts may be broken; stable ones migrate; stability is no excuse for source-centric assumptions); :800-818 (20: implementation source is not Sley source); :857-865 (22: provider-specific encodings are adapters); :1229 (final directive names SSMC1/SCB1/SMP1 as preserved by that campaign); SLEY_2X_REWEAVE_MASTER_SPEC_V1.md:194 (unversioned SSMC/SCB/SMP statement); Sley2.0mastergoal.md:359-395 (3.1 invariant before 3.3 mechanism)
CURRENT_BEHAVIOR: Every constitutional clause states the invariant in terms of machine sufficiency, precision, determinism, and efficiency; mechanisms are named as the current epoch's carriers and are replaceable by explicit migration (BG 19, REWEAVE 14.3, tightening 2.2). The prohibition on canonical human-readable text is intact in BG 3.2, master 3.2, ANTI_GOALS.md:16, and CONTRIBUTING.md:36-39.
DESIRED_INVARIANT: Sley is optimized for machine semantic precision, correctness, generation efficiency, verification efficiency, and interoperability; human-oriented source conventions must not constrain canonical architecture; no replaceable mechanism is constitutional above that.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: No clause found that would forbid a v2 encoding epoch on constitutional grounds, and none that weakens the text prohibition. Section 3 of this audit quotes each clause with its verdict. Tightening section 11 default applies.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: Maat (doctrine), Ariadne (contract)
REVIEW_REQUIRED: none
```

```text
FINDING_ID: AT-BG-02
TITLE: "SCB1 only" in the anti-goal evidence column is evidence wording, not a constitutional pin
HYPOTHESIS: docs/ANTI_GOALS.md row "canonical text or human projection" with acceptance evidence "SCB1 only" makes the SCB1 mechanism, rather than the no-text invariant, the enforced rule.
REPOSITORY_EVIDENCE: docs/ANTI_GOALS.md:16 (the row); evidence/validation/anti-goal-conformance.json entry for "canonical text or human projection" (state REVIEW_ONLY, "no mechanical evaluation; the Council review owns it"); scripts/build_anti_goal_conformance.py (no code evaluates that row); machineresearch/sley-2.0/03-constitution-and-anti-goals.md:7-9 ("SCB1/SSMC1 state is canonical while source text ... remain non-canonical")
SPEC_EVIDENCE: Sley2.0.1mastergoal.md:136-146 (3.2, the invariant the row carries); :791-793 (mechanism migration allowed); docs/spec/EPOCH_MIGRATION_POLICY_V1.md (how a new encoding epoch enters)
CURRENT_BEHAVIOR: The prohibition column is mechanism-neutral; the evidence column names the current encoding, as every evidence column in the matrix names current artifacts. The row is review-owned, so no checker would misfire on a future epoch. A new encoding epoch updates the evidence cell as part of ordinary epoch migration work.
DESIRED_INVARIANT: The anti-goal prohibits canonical text and human projection regardless of which binary encoding epoch is current.
DISPOSITION: D_REJECT
RATIONALE: Rewording "SCB1 only" to "the current canonical encoding epoch only" would be a stylistic broadening with no present defect, which tightening section 11 forbids ("Do not modify BLACKGLASS merely to make it philosophically broader") and section 21 forbids ("reopen already-correct canonical decisions for stylistic preference"). Recorded so a future epoch's implementer knows the cell must move with the epoch.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: Maat
REVIEW_REQUIRED: none
```

```text
FINDING_ID: AT-BG-03
TITLE: BLACKGLASS in-repo artifacts (MACHINE_PRIMACY.md, manifest, gate) are absent by REWEAVE design, not by wording defect
HYPOTHESIS: BLACKGLASS section 7 and 10 require MACHINE_PRIMACY.md, a Machine Primacy Manifest, and a Machine Primacy Gate before any SSMC1/SCB1/SMP1 contract is declared Beta or GA; none exists in the repository.
REPOSITORY_EVIDENCE: docs/MACHINE_PRIMACY.md does not exist; no "MPI-0" or "machine primacy" string in README.md, ARCHITECTURE.md, CONTRIBUTING.md, SECURITY.md, docs/*.md, or docs/adr/*.md except docs/adr/ADR-0049-reweave-scope-adoption.md; machineresearch/sley-2.0/reweave/rw-030-charter.md:1-27 (RW-030 "MPI-0 controls + host-boundary" charter; control map with enforcement, review lane, and later lifecycle evidence per control; carriers CONTRIBUTING.md C-01 and docs/ANTI_GOALS.md C-02 at adoption 3fc2275); docs/ANTI_GOALS.md:21 (human readability optimization row, REVIEW_ONLY)
SPEC_EVIDENCE: Sley2.0.1mastergoal.md:49 (apply before Beta/GA), :334-373 (manifest), :429-455 (gate), :981-985 (BG-010, BG-050); SLEY_2X_REWEAVE_MASTER_SPEC_V1.md:112 (OVR-05: "Adopt applicable machine-primacy controls during reconciliation, complete dependent suites when the lifecycle exists"), :131 (BG's constitution, counterfactual, dependency firewall, and lifecycle suites are incorporated), :596 (RW-030), :614 (RW-210 "Full source-free and machine-primacy demonstrations: MG/BG/WA end-to-end coverage")
CURRENT_BEHAVIOR: The newer authority (REWEAVE) explicitly re-times BLACKGLASS's suites to the lifecycle and RW-030 has recorded the control map with real enforcement for the mechanical rows and named review lanes for the prose rows. The constitution text itself is carried by CONTRIBUTING.md and ANTI_GOALS.md.
DESIRED_INVARIANT: MPI-0 is enforced through machine-readable policy, architecture tests, dependency rules, conformance fixtures, benchmark gates, and contribution requirements (BG section 2) by the time the integrated candidate is qualified.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: This is a scheduling fact governed by REWEAVE OVR-05 and RW-030/RW-210, already recorded and reviewed (ADR-0049 review PASS transcripts named at docs/adr/ADR-0049-reweave-scope-adoption.md:4-6). It is not a wording defect in any constitutional clause and tightening section 11 authorizes no BLACKGLASS implementation work. Nothing to change in this pass.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none in this pass; RW-210 owns the BG lifecycle suites
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: RW-030 (Nabu/Ariadne review lane per the charter), RW-210
REVIEW_REQUIRED: none in this pass
```

```text
FINDING_ID: AT-WB-01
TITLE: Witnessed Authority is not part of the active canonical architecture; audit is scope-only
HYPOTHESIS: Witnessed Authority may already be present in crates, specs, or conformance, in which case its boundary must be audited.
REPOSITORY_EVIDENCE: zero `Witness<` occurrences in crates/ (excluding target/); eighteen workspace crates, none for Witness; all "witness" hits are the GC store-root witness (docs/spec/GARBAGE_COLLECTION_V1.md:114, crates/sley-txn/src/repository.rs:4932-4940, docs/adr/ADR-0023-crash-recovery-boundary.md:445-460) or native denials (crates/sley-vm/src/host_abi.rs:11, crates/sley-vm/tests/rw070_host_abi_freeze.rs:816-817); host-boundary.json:178-179 ("WA inactive; no unimplemented Witness semantics may be claimed protective", status later-qualified); docs/spec/BOOTSTRAP_PROFILE_1.md:20 and :26-28 (Witness analysis and the WITNESS release surface are later, strict-superset surfaces); docs/spec/IDENTIFIERS_V1.md:21-70 (no witness, discharge, origin, or proposal-integrity domain); conformance/ (no such family); docs/adr/ADR-0049-reweave-scope-adoption.md:72-73
SPEC_EVIDENCE: SLEY_2X_REWEAVE_MASTER_SPEC_V1.md:489-541 (section 16 incorporates the amended 2.1 WA master as the future contract), :533-541 (16.7: Witness lands inside the Sley toolchain closure after SH2; "After WA changes land, repeat ..."), :575 (phase R4), :608-613 (RW-150, RW-170, RW-180, RW-190, RW-200); SLEY-2.1-WITNESS-MASTER-SPEC.md (2.1 scope, headers only); SLEY-2.0-ARCHITECTURE-TIGHTENING.md section 12 ("If Witnessed Authority is already part of the active Sley 2.0 canonical architecture, audit ...")
CURRENT_BEHAVIOR: No proposal type, origin representation, control-dependence rule, join/loop/function-boundary rule, persistence, promotion or discharge rule, authority sink, or verifier contract exists for Witness at this commit. The ten section-12 boundary questions have no code to bind to.
DESIRED_INVARIANT: The Witness boundary is audited against frozen contracts and code when they exist (RW-150 onward), with the minimum semantics necessary for exact proposal, provenance, and promotion enforcement.
DISPOSITION: E_DEFER_2_1_PLUS
RATIONALE: The audit's own precondition is unmet. Recording scope-only with the evidence above so the next pass can start from RW-150's frozen contracts instead of re-establishing absence.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none now; REWEAVE 16.7 requires re-proving SH2 after Witness lands
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none now
SPECS_TO_UPDATE: none now
CODE_OWNERSHIP: RW-150 (contracts), RW-170/RW-180 (Sley checker and discharge)
REVIEW_REQUIRED: none now; a full boundary audit at RW-150 freeze
```

```text
FINDING_ID: AT-WB-02
TITLE: Native discharge and Witness-trust judgment are already denied ahead of Witness landing
HYPOTHESIS: The pre-Witness architecture might leave a native path that could later become a hidden discharge or trust authority.
REPOSITORY_EVIDENCE: host-boundary.json:25-26 (native may not "typecheck, lower, validate programs, or discharge" nor "assemble compiler images or judge Witness trust"), :35 ("no imported discharge service decides promotion; the Sley semantic implementation of approved deterministic discharge profiles owns the judgment"), :45-46 (compiler-service ban and giant-opcode ban naming "compute Witness trust"), :178-179; docs/spec/HOST_ABI_V1.md:141 (native MUST NOT "perform CFG/effect/contract/Witness judgment"), :175 (`discharge_witness(program/value)` in the denied list); docs/spec/EFFECT_SYSTEM_V1.md:154-159 (pure primitives must not perform "Witness integrity/discharge judgment"); crates/sley-vm/tests/rw070_host_abi_freeze.rs:816-817 (the denial is pinned by test); crates/sley-vm/src/host_abi.rs:11
SPEC_EVIDENCE: SLEY_2X_REWEAVE_MASTER_SPEC_V1.md:305-327 (10.2 item 3 and item 6, 10.3, 10.4: native may hold crypto primitives but never discharge judgment); :509-515 (16.3: the pre-existing protected policy binds the verifier; the judged candidate cannot select its own verifier); :533-537 (16.7: a transient Rust implementation cannot remain a hidden SH2 production dependency)
CURRENT_BEHAVIOR: Every native surface that could later be misused as a discharge or trust authority is denied by contract, by boundary record, and by test, without claiming any Witness semantics exist.
DESIRED_INVARIANT: Only an approved deterministic verifier bound by pre-existing protected policy may promote a witnessed value; no native or imported service may own that judgment.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: The denial side of the future boundary is in place and is exactly the minimum tightening section 12 asks for ("Prefer the minimum semantics necessary"). Nothing to add before RW-150.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none; the denials are part of the SH2 boundary already
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none; rw070_native_compiler_services_are_not_admitted already pins the list
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: RW-070 (host ABI freeze), RW-030 (boundary record)
REVIEW_REQUIRED: none
```

```text
FINDING_ID: AT-WB-03
TITLE: Proposal-versus-authority separation already exists structurally in the candidate chain
HYPOTHESIS: The 2.0 architecture might conflate a proposed change with an authoritative value, which Witness would later have to unwind.
REPOSITORY_EVIDENCE: docs/spec/IDENTIFIERS_V1.md:31-32, :25-26 (distinct domains sley2.candidate.v1, sley2.candidate-result.v1, sley2.transaction.v1, sley2.transaction-receipt.v1); crates/sley-mutate/README.md:4 ("proposal-value host-model slice"); docs/ANTI_GOALS.md:23 ("model/reviewer as semantic oracle: deterministic kernel alone decides validity"), :24 ("self-authorizing candidate: candidate cannot change its judging roots"); crates/sley-protocol/src/session.rs:116-118 and crates/sley-query/src/context_capsule.rs:333-341 ("provenance" here is the engine-verified session binding, not a model-origin claim); docs/spec/CANDIDATE_RECORD_V1.md:6-17 (boundary), docs/spec/CANDIDATE_RESULT_V1.md, docs/spec/TRANSACTION_MODEL_V1.md
SPEC_EVIDENCE: SLEY-2.1-WITNESS-MASTER-SPEC.md WA-0 "Proposal is not authority", WA-6 "Candidate and judge are separated" (headers); SLEY_2X_REWEAVE_MASTER_SPEC_V1.md:529-531 (16.6: full existing validation and commit gates, not generic verifier success, control program changes); Sley2.0mastergoal.md:1792 (13.5 no prompt authority)
CURRENT_BEHAVIOR: A candidate is a proposal with its own identity; validity is a separate result object; acceptance is a separate transaction with a receipt. No model output is an oracle. No "provenance" or "proposal" symbol in the tree carries model-origin meaning, so nothing pre-empts WA's origin-set representation.
DESIRED_INVARIANT: Explicit proposal versus authoritative value, with the deterministic kernel as the only judge, preserved when Witness adds origin evidence to candidate identity (16.6).
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: The structural precursor of WA-0 and WA-6 is present and reviewed under S20-350/360/390. Adding origin evidence to candidate identity at RW-200 is additive to this chain, not a reinterpretation of it (tightening 2.2 will require a new candidate version at that point, which is RW-150's job).
CANONICAL_IMPACT: none now
SCHEMA_IMPACT: none now
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: Merlin (sley-mutate, sley-policy, sley-txn) per docs/WORK_PACKAGES.md
REVIEW_REQUIRED: none
```

```text
FINDING_ID: AT-WB-04
TITLE: Generalized trust lattice, confidence lattice, and policy algebra are excluded
HYPOTHESIS: Richer trust semantics might be needed for 2.0 correctness or might be tempting to pre-build.
REPOSITORY_EVIDENCE: none present (no lattice, confidence, or trust-policy type in crates/ or docs/spec; grep for Witness types returns zero); host-boundary.json:179 forbids claiming unimplemented Witness semantics as protective
SPEC_EVIDENCE: SLEY_2X_REWEAVE_MASTER_SPEC_V1.md:513 (16.3: "bounded evidence metadata within WA's existing two-class model, not a general trust lattice or partial-field discharge system"), :555 (18: "general security lattice" and "probabilistic language semantics" excluded), :557 (do not add an excluded effect merely because WA names it as an example sink); SLEY-2.0-ARCHITECTURE-TIGHTENING.md section 12 ("Any generalized probabilistic trust, confidence lattice, or policy algebra not required for current correctness SHALL be E_DEFER_2_1_PLUS") and section 21 ("add generalized AI trust theory" is a non-goal)
CURRENT_BEHAVIOR: Nothing of the kind exists, and two authorities already forbid building it in this campaign.
DESIRED_INVARIANT: Witness, when it lands, uses the two-class integrity model with exact proposal, provenance, and promotion enforcement and nothing more general.
DISPOSITION: E_DEFER_2_1_PLUS
RATIONALE: Required by the tightening spec's own rule; not needed for 2.0 correctness; already excluded by REWEAVE 16.3 and 18. Recorded so the exclusion is visible in this audit's register.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: RW-150 (any future WA contract change requires an operator scope amendment per REWEAVE 18 last paragraph)
REVIEW_REQUIRED: none
```



#### 3.3.1 Triage of the S20-730 checker's two residual problems

Running `scripts/check_reproducibility_and_independent_conformance.py` in the
campaign worktree reports two problems that the baseline checker reports
identically at 560a5f16 (verified by running the committed script from a
scratch copy against the same tree):

- `reproducibility-report:stale:84bfa9c9c5d9:32-surface-files-changed`: the
  report attests commit 84bfa9c9 and the artifact surface has changed since.
  RESUME.md records this as the by-design S20-730 staleness that re-attests at
  the next `make release-candidate-smoke`, pending since the S20-330 closure.
  Not caused by this campaign; cleared only by the smoke, which is a lane
  action.
- `release-tests:fail`: 15 of 59 tests under `bench/release/tests` error in
  `setUp` with `INVENTORY_MISSING` because
  `evidence/runtime/s20-720-release-candidate/evidence.json` does not exist.
  That file is gitignored runtime evidence written by the release smoke, so it
  is absent in every fresh worktree (the main checkout carries it). Expected
  in this environment; not a repository defect; not changed here.

Neither problem masks the AT-CL-02 repair: the new report-mismatch checks are
independent of both and were exercised fail-before and pass-after.

### 3.4 Native / Sley authority boundary and self-host succession (spec sections 6 and 13)

The complete inventory (46 rows, 47 classified entries: SEMANTIC_AUTHORITY 21,
STRUCTURAL_VALIDATION 12, EXECUTION_MECHANICS 9, RESOURCE_ENFORCEMENT 2,
TRANSPORT 3), the forbidden-pattern results, the section-13 gate
applicability table, and the staleness comparison against
`rw-070-host-inventory.json` are the separate deliverable
`SLEY-2.0-ARCHITECTURE-TIGHTENING-NATIVE-INVENTORY.md`. Claim level at the
baseline is SH0; every section-13 gate is NOT_REQUIRED except the static
"no hidden native semantic fallback" property, which holds. Records follow.

```text
FINDING_ID: AT-NA-01
TITLE: Native host inventory machine record is stale relative to RW-075 and RW-080 surface
HYPOTHESIS: rw-070-host-inventory.json already is the section-6 inventory and is complete and current.
REPOSITORY_EVIDENCE: machineresearch/sley-2.0/reweave/rw-070-host-inventory.json (base_commit b4c3390..., three primitive rows, sources naming BOOTSTRAP_PROFILE_1 and {lib,lower,execute,extended,bootstrap,host_abi}.rs only, TEST_ONLY path fuzz/targets/vm_canonical_inputs.rs absent from fuzz/Cargo.toml, FORBIDDEN evidence "hygiene scan in check_host_abi_v1.py" while the scan is at scripts/check_host_abi_markers.py:173-207); crates/sley-vm/src/{raw_hash.rs,exec_package.rs,admission_authority.rs} and crates/sley-check/src/lib.rs:267 absent from it; conformance/host-abi/v2/host-abi.json imports.rows has four entries; git log shows the file last changed at e85b89c (RW-070 land).
SPEC_EVIDENCE: REWEAVE 10.3 ("may remain native when explicitly inventoried"; "The final report must name every remaining native responsibility"); tightening spec AT-G3 ("native authority inventory complete"); rw-070.md section 1 defines the inventory as classifying "every native operation, import, runtime service, image mechanism, and host behavior reachable from BOOTSTRAP_PROFILE_1" (now BOOTSTRAP_PROFILE_2 per rw-080-contract.md successor baseline).
CURRENT_BEHAVIOR: The machine inventory describes the RW-070 tree; the RW-075 additions are enumerated only in prose (rw-075.md section 7). Nothing consumes the inventory by digest (only rw-070.md and rw-070-r2-handoff.md reference it), so no checker fails, but AT-G3 cannot be declared complete from repository records alone.
DESIRED_INVARIANT: One current machine inventory (successor identity, e.g. RW-075-HOST-INVENTORY-2, v1 kept byte-identical as history per section 2.2 convention) listing every native operation in section 1 of this audit with class, stage, decision, and evidence; base_commit updated; stale paths and script names corrected.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Records-only additive change; no code, canonical byte, ABI, profile, or identity changes; directly required by AT-G3 and REWEAVE 10.3; the content already exists in section 1.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none (record only; no self-host evidence invalidated)
MIGRATION_REQUIRED: no
TESTS_REQUIRED: optional: extend check_host_abi_markers.py to assert the successor inventory names every pub fn in sley-vm src (excluding cfg(test)); not required for closure.
SPECS_TO_UPDATE: machineresearch/sley-2.0/reweave/ (new successor inventory JSON plus a pointer line in rw-075.md or a new rw-075-host-inventory record); rw-070.md section 5 script-name erratum note (append-only).
CODE_OWNERSHIP: REWEAVE campaign records (sley2 machineresearch/sley-2.0/reweave); no crate owner.
REVIEW_REQUIRED: Ariadne (boundary record correctness), lightweight.
IMPLEMENTATION (slice 4): successor record landed as docs/audits/SLEY-2.0-ARCHITECTURE-TIGHTENING-NATIVE-INVENTORY.md; rw-070-host-inventory.json untouched (RW-070 history), superseded by reference.
```

```text
FINDING_ID: AT-NA-02
TITLE: Native semantic compiler chain (checker, lowerer, codec, fingerprints) is REQUIRED_NATIVE_FOUNDATION at C0/SH0 and MUST_MIGRATE only under the chartered SH2 packages
HYPOTHESIS: Spec section 6 requires every native SEMANTIC_AUTHORITY item to migrate to Sley before the final 2.0 freeze, so rows 1-15, 41 are freeze blockers.
REPOSITORY_EVIDENCE: rows 1-15 and 41 of section 1 (crates/sley-check/src/lib.rs:218, cfg.rs:231, effects.rs:261, contracts.rs:257; crates/sley-ssmc/src/fingerprint.rs:135,177,282; crates/sley-vm/src/lower.rs:258,290,838; extended.rs:795; crates/sley-scb1/src/lib.rs; crates/sley-mutate/src/codec.rs:66,76); host-boundary.json sley_owned[0..5] naming these crates as "reference seed ... not the final owner" with intended owners RW-090..RW-180; bootstrap-manifest.json S null (SH0).
SPEC_EVIDENCE: Sley2.0mastergoal.md 14.5 ("Sley 2.0 MUST NOT be delayed to make Sley implement itself. The Rust implementation is the bootstrap and trusted reference."); REWEAVE 5 ("before promotion: Rust reference owns active checking/lowering"); REWEAVE 10.1 (SH0 row) and 10.2 (the six Sley-owned responsibilities at SH2); REWEAVE 13.1 (C1 = C0.build(S, P) is the seed's legitimate semantic work); ADR-0049 (REWEAVE-1.0 is the campaign identity, 2.1.0 the naming proposal); tightening spec section 13 is conditional ("If REWEAVE completes C0 -> C1 -> C2 -> C3").
CURRENT_BEHAVIOR: The Rust reference is the sole production checker/lowerer (repository lifecycle via sley-policy candidate validation at candidate_validation.rs:809-955; execution via execute_function/execute_approved_package_v2). Migration is in progress only as provisional RW-080 codec slices under the 2026-09-07 override, not claimed as authority.
DESIRED_INVARIANT: For the Sley 2.0 freeze, native semantic authority is the deliberately assigned bootstrap/reference substrate (RNF). For the SH2 line, each 10.2 responsibility migrates under its chartered package with paired-comparison promotion (REWEAVE 14.2) and no retirement before 14.4 conditions.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: The assignment is explicit and versioned in host-boundary.json and REWEAVE; reading "before final 2.0 freeze" as forcing SH2 migration would contradict master 14.5 and ADR-0049's own scoping. No redesign is authorized (spec section 0). The lead should record this reading in the closeout so AT-G3 "self-host boundary exact" is evaluated against C0/SH0, not SH2.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no (campaign packages RW-090..RW-120 already chartered)
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none (optional: one sentence in the tightening closeout stating the C0/SH0 reading)
CODE_OWNERSHIP: n/a
REVIEW_REQUIRED: Maat (doctrine reading of "2.0 freeze" versus REWEAVE line), advisory only.
```

```text
FINDING_ID: AT-NA-03
TITLE: v2 admission authority's native semantic legs are a declared C0 seed path with a reserved, non-minting Sley ingress
HYPOTHESIS: admit_v2_package performing judge_bootstrap_profile plus lower_function per package is hidden native semantic authority on the self-host path (the premium AR-07 pattern) and must be repaired before freeze.
REPOSITORY_EVIDENCE: crates/sley-vm/src/admission_authority.rs:1-48 (module contract: C0 SEED PATH declared, PERMANENT MECHANICS, SLEY INGRESS reserved), :157-231 (admit_v2_package with judge_closure_for_seed :189 and reference_lower_for_seed :214), :239-289 (verify_structural_correspondence, pure equality), :296-325 (SleyAdmissionEvidence sealed, admit_v2_package_from_sley_evidence always refuses SleyEvidenceUnavailable); exec_package.rs admit_package_v2 pub(crate); scripts/check_exec_package_markers.py:95-137 (exclusivity and forbidden-call pins, PASS); reviews/reweave-rw075-premium-r1-2026-09-06.log:126 (AR-07 BLOCKER as originally found) and reviews/reweave-rw075-native-r12-2026-09-07.log AR-07 "CLOSED as designed ... Residual: same C1-exclusion caveat" (self-review, provisional).
SPEC_EVIDENCE: REWEAVE 13.1 ("C1 = C0.build(S, P)"; "All computation after C1 must use the Sley implementations"); 10.3 bullet 8 ("emergency bootstrap assets outside the clean self-build closure"); rw-080-contract.md 1.4 and 1.6 (two stages that "never mix"); tightening spec section 6 forbidden pattern "native code constructing compiler answers before Sley receives them".
CURRENT_BEHAVIOR: Before C1 exists, the native seed is the only route that can mint a v2 receipt; it compares a candidate image against its own reference lowering and refuses on any divergence (no receipt, no approval, no execution). Sley receives no compiler answer from it. Post-C1 minting from Sley evidence has an interface reservation and no implementation.
DESIRED_INVARIANT: After C1 exists, clean stages mint only through Sley-produced evidence; the seed route is unreachable in the seed-absent environment (13.3) and this becomes a tested property, not a declaration.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: For the current claim level (SH0, no C1) the structure matches REWEAVE 13.1 exactly and no forbidden pattern is instantiated; the remaining obligation is a future-stage gate (13.3 seed absence), not a pre-freeze defect. Independent acceptance of the AR-07 repair (Nabu round-12, premium round 2) is still pending per rw-075-continuation-2026-09-07.md; that is review debt, not a boundary defect found here.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none now; at C1 the seed-absence gate must prove the seed route unreachable
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none now (13.3 audit at R3/R4)
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-vm admission_authority (REWEAVE RW-075/RW-080 owner)
REVIEW_REQUIRED: already queued (Nabu round-12, premium delta round 2); no new review from this finding.
```

```text
FINDING_ID: AT-NA-04
TITLE: SH2 campaign-declaration registry is absent while SH2 work has landed, so the ADR-0049 citation check holds vacuously
HYPOTHESIS: Every landed SH2 work item is declared in evidence/reweave/sh2-work-items.json citing host-boundary.json by digest and a staged gate, as ADR-0049 and host-boundary.json require.
REPOSITORY_EVIDENCE: evidence/ contains conformance, release, review, security, validation only (no evidence/reweave/); scripts/build_anti_goal_conformance.py:85-99 ("An absent registry means no SH2 work is declared, which holds vacuously"); host-boundary.json "prohibited"[3]: "self-hosting work outside declared, digest-cited, stage-gated items (C-01/C-02 campaign-declaration rule)"; host-boundary.json "charter": "Chartered record consumed by scripts/build_anti_goal_conformance.py evaluate_campaign_declarations ... cited from evidence/reweave/sh2-work-items.json items"; landed SH2 packages RW-030, RW-040, RW-050, RW-060, RW-070, RW-075 (+correction) and provisional RW-080 slices 1-7 (git log 13f1a85..560a5f16) exist with no registry item.
SPEC_EVIDENCE: docs/adr/ADR-0049-reweave-scope-adoption.md Decision 3 ("requires every declared SH2 work item ... to cite the boundary record by exact path and SHA-256 digest and to name a staged SH2 gate"); ADR-0049 Consequences ("until then any declared SH2 work item fails the campaign check"); tightening spec 2.3 item 1 (exact repository evidence) and AT-G3 "self-host boundary exact".
CURRENT_BEHAVIOR: The mechanical guard that binds SH2 work to the boundary digest never evaluates a single item; the binding exists only in prose (each RW record cites d935d238...). The six-crate denylist still runs.
DESIRED_INVARIANT: Every landed SH2 package and provisional slice is an item in the registry (contract sley2.reweave-sh2-work-items.v1) citing host-boundary.json sha256 d935d238a4d75d154df128aad630411ca3fdcc18e4db084dcd2317ab73bdb18a and its gate (RW-030..RW-080 package gates or R2/R3 phase gates), so the checker actually exercises the citation rule.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Additive evidence file only; no code, ABI, or canonical change; it makes an already-adopted governance check non-vacuous; cost is minutes. It is outside the crate boundary and could be reassigned to a governance lane, but it is the only mechanical binding between the native remainder charter and the work that extends it.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: run scripts/build_anti_goal_conformance.py after adding the registry (must report the item count, not "no declared SH2 work items").
SPECS_TO_UPDATE: evidence/reweave/sh2-work-items.json (new); optionally a pointer in rw-075.md.
CODE_OWNERSHIP: REWEAVE campaign records; checker unchanged.
REVIEW_REQUIRED: Maat (campaign-declaration compliance), lightweight.
IMPLEMENTATION (slice 4): evidence/reweave/sh2-work-items.json created (contract sley2.reweave-sh2-work-items.v1, seven items RW-030, RW-040, RW-050, RW-060, RW-070, RW-075, RW-080, each citing host-boundary.json sha256 d935d238... and a gate from gate_set, records verified to exist). build_anti_goal_conformance.py rebuilt evidence/validation/anti-goal-conformance.json: the campaign-declaration detail changed from "no declared SH2 work items" to "7 declared SH2 work items cite the authorized boundary record and a staged gate"; --check PASS. Discrimination proved on a scratch root: a wrong digest and a blank gate both return VIOLATED. Semantic authorization of the items is ADR-0049 (operator decision ADOPT-REWEAVE-2026-09-06); the registry declares, it does not authorize.
```

```text
FINDING_ID: AT-NA-05
TITLE: bootstrap-manifest.json binds P to BOOTSTRAP_PROFILE_1 although the successor baseline is BOOTSTRAP_PROFILE_2
HYPOTHESIS: The bootstrap manifest's P binding reflects the current dependency contract that C1 = C0.build(S, P) will use.
REPOSITORY_EVIDENCE: machineresearch/sley-2.0/reweave/bootstrap-manifest.json "P": value BOOTSTRAP_PROFILE_1, digest 4f269150..., "status": "frozen (BOOTSTRAP_PROFILE_1 v1, RW-050 slice 2)"; rw-080-contract.md header ("every module below builds under BOOTSTRAP_PROFILE_2 (fb2d8cc8...), HOST_ABI_V2 (bc564653...), and EXEC_PACKAGE_V2 (f4958c5e...) ... no RW-080 module builds under [v1]"); rw-080 slice records admit under profile v2 (receipt.profile_digest() == BOOTSTRAP_PROFILE_2_DIGEST, rw080_codec_program_outer.rs:25207-25211); scripts/check_bootstrap_capability.py:47,322-333 reads the manifest for structure and H pinning only.
SPEC_EVIDENCE: REWEAVE 13.1 ("P be the pinned build/lowering profile, schema epoch, host ABI, limits, and permitted dependencies"); 14.3 ("Do not confuse toolchain program identity, executable image identity, program schema epoch, host ABI version, and policy root. They are separate, explicit bindings."); tightening spec 2.2 (new meaning gets a new explicit version, old preserved).
CURRENT_BEHAVIOR: The manifest names the historical P; the live successor P is recorded elsewhere (machine-summary.json bootstrap_profile_2, rw-080-contract.md). S remains null, which is still correct (the RW-080 slices are fixture-namespace seed graphs, not a canonical S root). No checker fails.
DESIRED_INVARIANT: The manifest records the current candidate P (profile v2 digest fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459, HOST_ABI_V2, EXEC_PACKAGE_V2) with v1 retained as a history entry, so a future C1 binding cannot silently inherit v1.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Additive record update (add the successor P entry and keep the v1 entry as history); must keep check_bootstrap_capability.py green (it validates manifest structure, so the edit should add fields rather than rename keys). Not a semantic or ABI change.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none (no C1 bound yet)
MIGRATION_REQUIRED: no
TESTS_REQUIRED: python3 scripts/check_bootstrap_capability.py PASS after the edit; python3 scripts/check_bootstrap_profile_1.py unchanged PASS.
SPECS_TO_UPDATE: machineresearch/sley-2.0/reweave/bootstrap-manifest.json
CODE_OWNERSHIP: REWEAVE campaign records (RW-040 owner).
REVIEW_REQUIRED: none beyond the checker run.
IMPLEMENTATION: deferred to the spec-synchronization slice, which owns bootstrap-manifest.json alongside the other stale-successor facts (see AT-SS records), so the manifest is corrected once with one checker run.
```

```text
FINDING_ID: AT-NA-06
TITLE: HOST_ABI_V2 frozen record says "three frozen pure primitives" in seed_absence.retained while the record admits four rows
HYPOTHESIS: The v2 machine record is internally consistent about the retained primitive count.
REPOSITORY_EVIDENCE: conformance/host-abi/v2/host-abi.json imports.rows = [host-bytes-to-u8vector, host-u8vector-to-bytes, vector-push, raw-blake3-256] (4) while seed_absence.retained[1] = "the three frozen pure primitives under this ABI"; docs/spec/HOST_ABI_V2.md "Seed-absence condition unchanged, now on this ABI alone (three primitives plus RHW1)"; scripts/check_host_abi_v2.py PASS (it binds values, not this prose); record digest bc564653... is cited by rw-080-contract.md, check_r2_exit.py, machine-summary.json.
SPEC_EVIDENCE: tightening spec 2.2 (frozen encodings and records are not silently redefined; a change needs a new explicit version); HOST_ABI_V2.md is "authoritative for rationale and rules", the JSON "authoritative for values".
CURRENT_BEHAVIOR: The doc carries the correct count; the digest-frozen JSON carries a copied v1 sentence. No semantic or checker effect.
DESIRED_INVARIANT: The next successor record (if any) states four primitives; until then the doc remains the authoritative rationale text and an erratum note exists.
DISPOSITION: E_DEFER_2_1_PLUS
RATIONALE: Editing the frozen JSON would change bc564653... and invalidate every citation and the R2 gate binding for a wording fix with no semantic content; the authoritative doc is already correct. Record the erratum in the successor inventory (AT-NA-01) and fix the sentence when a HOST_ABI_V3 is ever cut.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none (values unaffected)
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: erratum line in the AT-NA-01 successor inventory; HOST_ABI_V3 when created.
CODE_OWNERSHIP: RW-075 correction owner.
REVIEW_REQUIRED: none.
```

```text
FINDING_ID: AT-NA-07
TITLE: The value_hash opcode natively constructs the SLEYVHS1 canonical preimage; RAW_HASH_V1's "every semantic preimage ... constructed by Sley" statement is scoped to RHW1 only
HYPOTHESIS: A native opcode that frames and hashes canonical value preimages is a concealed semantic digest service forbidden by REWEAVE 10.4 and RAW_HASH_V1.md.
REPOSITORY_EVIDENCE: crates/sley-vm/src/extended.rs:1930 (opcode 178 executes hash_validated_value); crates/sley-ssmc/src/fingerprint.rs:282-300 (builds SLEYVHS1 || u32(1) || epoch || SSMC1_FIELD_SCHEMA_HASH || type_bytes || data_bytes natively); BOOTSTRAP_PROFILE_2.md "Permitted opcodes (42 ...) E5 cells and value hashing" (178 in PERMITTED_BOOTSTRAP_OPCODES, bootstrap.rs:63-66); rw-070-host-inventory.json classifies "value_hash via hash_validated_value" as REQUIRED_HOST_RUNTIME_MECHANIC; rw-075-hash-inventory.md obligation 2 says the preimage is "semantic/compiler-owned"; docs/spec/RAW_HASH_V1.md "What this is not": "Every semantic preimage - including sley-id domain prefixes and SLEYSFP1/SLEYVHS1 framing - is constructed by Sley as bytes; the host adds nothing."
SPEC_EVIDENCE: REWEAVE 10.3 bullet 2 ("primitive value operations" may remain native); 10.4 ("The native remainder may not conceal the hard compiler work as primitives"); VM_EXTENDED_OPCODE_PROFILE_V1.md section E5 defines value_hash as frozen language execution semantics over runtime values.
CURRENT_BEHAVIOR: Opcode 178 is pinned execution semantics of the language (a value-level primitive comparable to equal/order), not an import service taking programs, candidates, types, or inventories; it takes one runtime value and returns its canonical hash. A Sley checker may legitimately use it for value identity, and then the canonical value encoding lives in the VM substrate. This is the same class as native equality over canonical structure.
DESIRED_INVARIANT: value_hash remains classified EXECUTION_MECHANICS under 10.3 bullet 2 with its preimage construction documented as opcode semantics; RAW_HASH_V1.md's "every semantic preimage" sentence is understood to govern RHW1 and compiler-object digests (fingerprints, sley-id domains, cache keys), not the value_hash opcode.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: The opcode is frozen, inventoried (rw-070 inventory row), profile-admitted, and not an import; REWEAVE 10.4 targets services that "validate SSMC, resolve types, compute Witness trust, choose mandatory tests, or emit the compiler image", none of which value_hash does. No change; the successor inventory (AT-NA-01) should carry the scoping sentence so a reviewer does not read RAW_HASH_V1.md as contradicted. Confidence: medium; an independent boundary reviewer (Ariadne) may want to confirm the E5 reading.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none required; optional one-line scoping note in the AT-NA-01 successor inventory.
CODE_OWNERSHIP: sley-vm extended (E5 owner).
REVIEW_REQUIRED: Ariadne, advisory.
```

```text
FINDING_ID: AT-NA-08
TITLE: Legacy execution entry points (execute_function, execute_loaded_image) retain native semantic input judgment
HYPOTHESIS: Native check_constant/require_hashable calls on execution inputs (execute.rs:1784-1790, 2248) are hidden native semantic authority on the execution path.
REPOSITORY_EVIDENCE: crates/sley-vm/src/execute.rs:494 (execute_function re-lowers via lower_function), :1717-1745 (validate_inputs -> check_input_shape), :1784-1790 (check_constant, require_hashable), :2248-2266 (legacy observation preimage requires hashable); package path instead uses validate_package_inputs_structural :783-813 and observation without require_hashable (:1069 comment); rw-075.md section 1 marks execute_loaded_image/ApprovedImage "preserved unchanged ... marked superseded for new executions"; production callers of execute_function are sley-protocol server.rs:2065 (dev surface) and sley-conformance (oracle).
SPEC_EVIDENCE: REWEAVE 5 (before promotion the Rust reference owns checking); 10.3 bullet 8 (independent oracles and test drivers stay native); tightening spec 2.2 (frozen contracts preserved; a new meaning gets a new version), which is exactly what RW-075 did by adding execute_approved_package_v2 instead of changing S20-270.
CURRENT_BEHAVIOR: Two runners coexist: the S20-270 reference runner (semantic input judgment, part of the SH0 production and oracle surface) and the RW-075 package runner (structural only). Neither is reachable from the toolchain import registry; only the package runner is on the SH2 host path.
DESIRED_INVARIANT: The SH2 host path uses only the structural package runner; the reference runner remains oracle/recovery material and is never a fallback (14.4).
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: Already versioned and documented; deleting or modifying the reference runner is prohibited by REWEAVE 14.4 and by the frozen S20-270 evidence. No action.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-vm execute (S20-270 owner)
REVIEW_REQUIRED: none.
```

```text
FINDING_ID: AT-NA-09
TITLE: No forbidden SH2 pattern is instantiated at this commit (consolidated negative result)
HYPOTHESIS: One of the five forbidden patterns from spec section 6 is present.
REPOSITORY_EVIDENCE: section 2 table of this audit: extended.rs:304-368 (four-row default-deny registry), rw070_host_abi_freeze.rs (eight compiler-service spellings and helper injection refused), check_host_abi_markers.py:173-207 hygiene PASS, check_exec_package_markers.py PASS (exec.rs and raw_hash.rs carry no gate/lowerer/minter calls; minter exclusivity), rw075_hydration_workloads.rs (no synthesized answers), rw080_codec_*.rs (no fallback constructs; native codec used only as oracle), no SH1/SH2 claim in any record.
SPEC_EVIDENCE: tightening spec section 6 forbidden-pattern list; REWEAVE 10.4, 11, 14.2, 15 (compiler-service denial family).
CURRENT_BEHAVIOR: All five patterns absent or not applicable (nominal self-hosting cannot occur because no self-hosting is claimed). One declared residual: the seed admission route's exclusion from clean stages is asserted by contract and unit pin, not by a seed-absent environment test, because no C1 exists.
DESIRED_INVARIANT: Unchanged; at R3/R4 the 13.3 seed-absence audit must convert the declared residual into evidence.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: Negative result with mechanical pins at this commit.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none now
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: n/a
REVIEW_REQUIRED: none.
```

### 3.5 Canonical specification synchronization (spec section 14) and status mechanism (15.5)

The spec-sync audit's 27-row drift table and its status-mechanism inventory
are retained in the campaign worksheet; the records follow. Two red gates
predate the campaign and are repaired here: the derived review evidence could
not be rebuilt (AT-SS-04) and two Tier 1 checkers bind the master goal by a
path outside the repository (AT-SS-13). machine-summary.json is the canonical
hand-maintained status mechanism that section 15.5's locator extends
(AT-SS-14).

```text
FINDING_ID: AT-SS-01
TITLE: Bootstrap manifest and CONTRIBUTING still bind P to BOOTSTRAP_PROFILE_1 after the v2 successor became the current R2 candidate
HYPOTHESIS: The REWEAVE lane record for P (bootstrap-manifest.json) and the governance text (CONTRIBUTING.md) were not moved when RW-075 introduced the v2 successor profile, ABI, and package.
REPOSITORY_EVIDENCE: machineresearch/sley-2.0/reweave/bootstrap-manifest.json:49-53 `"status": "frozen (BOOTSTRAP_PROFILE_1 v1, RW-050 slice 2)", "value": "BOOTSTRAP_PROFILE_1", "digest": "4f2691504b5c..."` (last commit b2c850f, 2026-09-06, before 9574f8a); CONTRIBUTING.md:41-43 `governed by host-boundary.json, BOOTSTRAP_PROFILE_1, and the staged SH2 gates`; scripts/check_r2_exit.py:89-94 labels the v1 digest as `P_bootstrap_profile_1` while separately checking `R2_profile_v2`; scripts/check_bootstrap_profile_1.py and scripts/check_bootstrap_capability.py:47 read the manifest and bind its P digest; host-boundary.json:21 mentions BOOTSTRAP_PROFILE_1 but is digest-pinned (sha256 d935d238... equals manifest H.manifest_digest) and must stay byte-identical.
SPEC_EVIDENCE: docs/spec/BOOTSTRAP_PROFILE_2.md:3-4,12 `frozen successor (RW-075 correction, 2026-09-06). Version 2. Current R2 candidate. ... Supersedes: BOOTSTRAP_PROFILE_1 v1`; reweave/rw-075-correction.md:64 `Supersession: v1 -> retained historical evidence; v2 -> current R2 candidate`; reweave/rw-080-contract.md:9-16 `every module below builds under BOOTSTRAP_PROFILE_2 ... no RW-080 module builds under them [v1]`; REWEAVE master 13.1 defines P as the pinned profile used in `C1 = C0.build(S, P)`; tightening spec section 14 forbids `master spec = new semantics / subordinate record = contradictory old semantics` without a supersession path.
CURRENT_BEHAVIOR: The manifest that REWEAVE 13.1 designates as the binding of P records the superseded v1 as the bound value with no successor field; CONTRIBUTING names v1 as governing; the R2 gate labels v1 as "P". The supersession itself is explicit in the V2 documents and the correction record, so semantics are not ambiguous, but the subordinate lane record and governance text disagree with the current candidate.
DESIRED_INVARIANT: Every record that names the bound P, host ABI, or exec package names the current candidate (v2 digests fb2d8cc8..., bc564653..., f4958c5e...) and carries v1 only as history.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Truthful record update with no semantic change: v2 is already frozen, reviewed (RW-075 Ariadne/Nabu PASS per check_r2_exit), and checker-validated; v1 stays byte-identical. The manifest edit must land together with check_bootstrap_profile_1.py / check_bootstrap_capability.py / check_r2_exit.py label changes because they bind the manifest, and host-boundary.json must not be touched.
CANONICAL_IMPACT: none (no identity, digest, or domain changes)
SCHEMA_IMPACT: none
ABI_IMPACT: none (HOST_ABI_V2 already current)
SELFHOST_IMPACT: none on semantics; the manifest becomes truthful for the eventual C1 = C0.build(S, P) record
MIGRATION_REQUIRED: no
TESTS_REQUIRED: check_bootstrap_profile_1.py, check_bootstrap_profile_2.py, check_bootstrap_capability.py, check_r2_exit.py, check_host_abi_v1/v2.py, check_exec_package_v1/v2.py all PASS after the edit; a manifest structural test that P.successor/history keys are provenanced (no invented hashes)
SPECS_TO_UPDATE: machineresearch/sley-2.0/reweave/bootstrap-manifest.json (P: current v2 + history v1), CONTRIBUTING.md:42, scripts/check_r2_exit.py labels; none of docs/spec
CODE_OWNERSHIP: Ariadne (REWEAVE lane records), Codex/integrator (CONTRIBUTING), Vulcan (gate scripts)
REVIEW_REQUIRED: yes, Nabu architecture re-read of the manifest binding (REWEAVE 13.1) before R2 exit re-evaluation
IMPLEMENTATION (slice 5c): bootstrap-manifest.json P now binds BOOTSTRAP_PROFILE_2 (digest fb2d8cc8..., v2 provenance, v1 named as superseded history without a second digest inside the stage, which check_bootstrap_capability forbids); CONTRIBUTING governance line updated; check_bootstrap_profile_1 accepted only a v1 binding and now accepts the declared successor when v1 history is named. check_bootstrap_profile_{1,2}, check_bootstrap_capability PASS; check_r2_exit unchanged (its P_bootstrap_profile_1 label checks the frozen v1 record, D3 left as is). host-boundary.json untouched.
```

```text
FINDING_ID: AT-SS-02
TITLE: CLI and JSON-bridge contracts pin SMP1 revision 10 while SMP1 is at revision 11 and the bridge vectors already carry revision-11 semantics
HYPOTHESIS: SMP1 revision 11 landed after the CLI and bridge closures and the subordinate contracts were never re-pinned, leaving implementation and vectors ahead of the contract text.
REPOSITORY_EVIDENCE: docs/spec/SLEY_CLI_V1.md:15 `(docs/spec/SMP1.md revision 10, ...)` and :217 `The revision pins are SMP1 revision 10 and bridge revision 6` (last commit 981d6a1, 2026-09-05 08:34); docs/spec/SMP1_JSON_BRIDGE_V1.md:19 `It composes, and never alters, docs/spec/SMP1.md (revision 10)` (db631d3, 2026-09-05 08:14); docs/spec/SMP1.md:3 `revision 11 (2026-09-05 ...)` landed 776983f 2026-09-05 17:03; commit d26b686 (2026-09-05 23:20) "teach the bridge vector checker the SMP1 revision 11 version split ... The S20-400 closure re-emitted the two negative-zero rejected vectors as PROTOCOL_DOWNGRADE"; conformance/smp1-json-bridge/v1/rejected.json:161,166 `"expected_code": "PROTOCOL_DOWNGRADE"`; grep PROTOCOL_DOWNGRADE in SMP1_JSON_BRIDGE_V1.md and SLEY_CLI_V1.md: 0 hits (SMP1.md: 6, ERROR_CODES_V1.md: 1); scripts/check_cli_contract.py and check_smp1_json_bridge_contract.py only extract the contract's own revision (lines 145-149, 150-154) and never assert the SMP1 pin; scripts/check_session_handle_profile.py:42-47,272-276 does assert its SMP1 pin against SMP1's status line and is at 11; RESUME.md:429-433 records the same episode.
SPEC_EVIDENCE: SMP1.md:32-50 revision history: "11 answers the three Council review rounds in full (explicit retryability enumeration, enforced response ceilings, ... version-claim rule, failed-stream flag, ...)"; tightening spec section 14: never `implementation = new semantics, canonical spec = old semantics`.
CURRENT_BEHAVIOR: The bridge vectors and checker reject below-selected versions as PROTOCOL_DOWNGRADE (revision 11) while the bridge contract text pins revision 10 and does not name the downgrade outcome; the CLI contract pins revision 10 and no checker would catch either pin moving.
DESIRED_INVARIANT: Every composing contract pins the revision of the authority it composes, the pin is asserted by its checker against the authority's own status line, and every vector outcome is named by the contract that owns the vector.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: SMP1, the bridge, and the CLI are all Council-review-pending drafts, not frozen contracts, so no supersession document is needed; re-pinning to revision 11 (bridge revision 7, CLI revision 4) with a revision note naming the downgrade split, plus checker-enforced pins modelled on check_session_handle_profile.py, is additive and records semantics that are already implemented and vectored.
CANONICAL_IMPACT: none (sley2.protocol-frame.v1 / handshake domains unchanged)
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: check_cli_contract.py and check_smp1_json_bridge_contract.py gain an SMP1 pin assertion (fail when SMP1.md status revision differs); `make conformance` bridge vectors unchanged and green
SPECS_TO_UPDATE: docs/spec/SMP1_JSON_BRIDGE_V1.md (pin 11, revision 7 note naming PROTOCOL_DOWNGRADE), docs/spec/SLEY_CLI_V1.md (pin 11, revision 4 note), docs/WORK_PACKAGES.md rows S20-420/S20-430, machine-summary.json json_bridge.contract_revision / cli.contract_revision
CODE_OWNERSHIP: Merlin (S20-420, S20-430 packages), Ariadne (SMP1 contract owner)
REVIEW_REQUIRED: yes, Ariadne contract re-read of the two pin bumps (bounded delta)
IMPLEMENTATION (slice 5d): SMP1_JSON_BRIDGE_V1 revision 7 and SLEY_CLI_V1 revision 4 (2026-09-08) re-pin SMP1 revision 11 with history notes naming the version-claim split; machine-summary contract_revision 7/4; WORK_PACKAGES rows updated. check_smp1_json_bridge_contract and check_cli_contract assert their own revision and each composed status line (modelled on check_session_handle_profile); PASS on the tree, FAIL with smp1-revision-pin on a scratch copy where SMP1 moves to 12. No vector or code change.
```

```text
FINDING_ID: AT-SS-03
TITLE: Required Contract Index status line and the S20-770 review state
HYPOTHESIS: REQUIRED_CONTRACT_INDEX_V1.md "revision 1, Council review pending" is stale because later S20-770 work exists.
REPOSITORY_EVIDENCE: docs/spec/REQUIRED_CONTRACT_INDEX_V1.md:3-5 `Status: S20-770 contract draft, revision 1 (2026-09-03); Council review pending`; git log for the file: 783fec8, 6398acb (both 2026-09-03), nothing later; scripts/check_required_contract_index.py:20-21 tokens S20_770_CONTRACT_DRAFT_REVIEW_PENDING / S20_770_INDEX_ACCEPTED, run result PASS (12 required contracts, 19 documents, 18 checkers, 14 domains, 50 derived identifier domains); machineresearch/sley-2.0/reviews/verdicts.json 770-ariadne-contract FAIL (p1 3, p2 6, p3 2), 770-nabu-architecture FAIL (p1 4, p2 3, p3 3), 770-vulcan-surface FAIL (p1 1, p2 4, p3 3), all p0 0; evidence/review/finding-register.json:1746-1780 package_status S20_770_CONTRACT_DRAFT_REVIEW_PENDING; docs/adr/ADR-0047-required-contract-index.md:3-4 "proposed; the S20-770 index is a draft at revision 1 with Council review pending"; grep for lane2 / "lane 2" / lane-2 across *.md *.json: zero hits, no S20-770 revision-2 draft anywhere in the tree.
SPEC_EVIDENCE: REQUIRED_CONTRACT_INDEX_V1.md:70 defines the two-state token vocabulary (DRAFT_REVIEW_PENDING until accepted); Sley2.0mastergoal.md section 17 names twelve contracts; the index table rows 17.1..17.12 name the identical twelve (verified string-for-string).
CURRENT_BEHAVIOR: Index content, master section 17, checker, ADR-0047, machine-summary token, finding register, and verdicts all agree: revision 1 draft, reviewed once, three FAIL verdicts on P1 findings, not accepted. The status line says "review pending" rather than "reviewed FAIL, repairs pending", but the checker-bound token cannot express more and the verdict record carries the detail.
DESIRED_INVARIANT: Index, master, checker, and review record agree on the accepted/not-accepted state. (Holds.)
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: No layer contradicts another. The open P1 findings are S20-770 package work (a revision-2 draft answering the three rounds), not a synchronization defect; the "lane2 work" premise is not present in the repository and is not assumed.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none beyond the existing check_required_contract_index.py PASS
SPECS_TO_UPDATE: none for synchronization; optional wording "Council review round 1 returned FAIL on P1 findings (verdicts.json); revision 2 pending" is cosmetic
CODE_OWNERSHIP: Ariadne (S20-770)
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-SS-04
TITLE: Derived review records (finding register, decision dossier, test inventory) are stale and cannot be rebuilt because the RW-060 verdict fields use a vocabulary the ADR-0042 derivation reads as PENDING
HYPOTHESIS: The machine summary moved fifteen commits past the last evidence refresh and one of those edits encoded PASS verdicts in a form the register derivation does not accept, so `make quick` is red at the pinned commit.
REPOSITORY_EVIDENCE: evidence/review/finding-register.json, evidence/release/decision-dossier.json, evidence/validation/test-inventory.json last rebuilt at b2c850f (2026-09-06); machine-summary.json changed in b4c3390, e85b89c, 624f1c0, 9574f8a, 6c3d5df, 3f64497, 9bb442c, d4f5a88, 83fe94a, f6048a3, 72fff72, 94a2a57, 0470915, 12546de, 9b9d394; `python3 scripts/check_finding_register.py` -> FAIL problems [finding-register:obligations-drift, finding-register:obligations-digest, finding-register:drift, register-tests:fail]; `python3 scripts/check_decision_dossier.py` -> FAIL problems [decision-dossier:drift, test-inventory:drift, dossier-tests:fail]; `python3 scripts/build_finding_register.py --check` -> `{"code": 75002, "detail": "rw060_source_free_lifecycle.ariadne_review is PENDING; rw060_source_free_lifecycle.nabu_review is PENDING", "name": "COMPLETION_VIOLATION", "result": "FAIL"}`; `build_decision_dossier.py --check` -> 76003 DRIFT; `build_test_inventory.py --check` -> FAIL "tracked test inventory differs from the derived inventory"; machine-summary.json rw060_source_free_lifecycle: `"status": "RW060_COMPLETE"`, `"ariadne_review": "FAIL_3xP1_THEN_PASS_3x3_DELTA"`, `"nabu_review": "FAIL_THEN_PASS_P1_P2s_AND_P22_ARC_CLOSED"`; scripts/build_finding_register.py:49 "Nothing else maps to a state except by its first token", :155-163 classify_token (head FAIL -> FAIL_ROUND, superseded only by a same-reviewer PASS field in the same section, none present), :185-190 is_complete_status; Makefile:80-81 run both checkers inside `quick`.
SPEC_EVIDENCE: reweave/rw-060.md:3-4 `Status: COMPLETE (implementation + validation green; Ariadne PASS, Nabu PASS with execution record)`; reviews/reweave-rw060-ariadne-r2-2026-09-06.log:18 and reweave-rw060-nabu-r2g-2026-09-06.log:20 orchestrator notes converting to final PASS on the execution record; docs/adr/ADR-0042 "the finding register is derived from recorded dispositions"; docs/spec/FINDING_REGISTER_V1.md section 2 vocabulary; tightening spec section 14 requires review/qualification records to agree with implementation.
CURRENT_BEHAVIOR: The RW-060 verdicts are PASS in the lane record and transcripts, FAIL-headed strings in the summary, PENDING in the derivation, and absent from the tracked register, which still reflects b2c850f. The Tier 1 gate is red at the pinned commit for a record-encoding reason, not an engineering one.
DESIRED_INVARIANT: Every summary review field uses the ADR-0042 vocabulary (first token PASS/FAIL/REVISE/PENDING/DEFERRED, FAIL rounds superseded by a same-reviewer PASS field), the derived register/dossier/inventory equal their `--check` derivation at every commit, and `make quick` is green.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Truthful re-encoding of verdicts that already exist (e.g. `ariadne_review_round1: FAIL_3_P1`, `ariadne_review: PASS_3_OF_3_DELTA_CLOSED`, same for Nabu) followed by `make evidence-refresh`; no semantic, identity, or gate change. Severity is high because a red Tier 1 gate masks later regressions and contradicts RESUME.md's "Tier 1 green" claim for every commit after b2c850f.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none (RW-060 evidence unchanged)
MIGRATION_REQUIRED: no
TESTS_REQUIRED: build_finding_register.py --check PASS, build_decision_dossier.py --check PASS, build_test_inventory.py --check PASS, check_finding_register.py PASS, check_decision_dossier.py PASS, `make quick` green; a regression unit test in the register test suite for a FAIL-then-PASS same-reviewer pair
SPECS_TO_UPDATE: machineresearch/sley-2.0/machine-summary.json (rw060 review fields), regenerated evidence/review/finding-register.json, evidence/release/decision-dossier.json, evidence/validation/test-inventory.json (and ga-acceptance-report if evidence-refresh touches it)
CODE_OWNERSHIP: Vulcan (S20-740 register), Codex (S20-750 dossier), Ariadne (RW-060 lane record)
REVIEW_REQUIRED: no (mechanical re-derivation); note in the next handoff per the in-flight repair rule
IMPLEMENTATION (slice 5a): rw060_source_free_lifecycle re-encoded as ariadne_round1/nabu_round1 = "FAIL (preserved)" and ariadne_review/nabu_review = "PASS_FINAL" (the rw075_correction convention), with the verdict narrative kept in round_history (a non-review key; a first attempt named review_history was itself classified OTHER and refused, which is the vocabulary working as designed). Rebuilt finding-register.json, decision-dossier.json, test-inventory.json and synced counters. build_finding_register --check FINDING_REGISTER_OPEN (its normal open state), build_decision_dossier --check PASS, build_test_inventory --check PASS, check_finding_register PASS, check_decision_dossier PASS, bench/review/tests OK. Register now derives 221 obligations, 67 open reviews, 10 deferred.
```

```text
FINDING_ID: AT-SS-05
TITLE: docs/WORK_PACKAGES.md carries stale contract revision pins and no REWEAVE rows
HYPOTHESIS: The work-package DAG was last edited on 2026-09-05 and no longer matches the spec status lines or the active lane set.
REPOSITORY_EVIDENCE: docs/WORK_PACKAGES.md last commit 1e71820 (2026-09-05); :31-32 `docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md (revision 9, 2026-09-03, ADR-0039, Council review pending)` vs docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md:3 `revision 13 (2026-09-06)`; :49 `docs/spec/SLEY_CLI_V1.md (revision 2, 2026-09-03, ...)` vs SLEY_CLI_V1.md:3 `revision 3 (2026-09-05)`; :59 `docs/spec/SLEY2_TRIAL_RUNNER_V1.md (revision 2, 2026-09-03, ...)` vs SLEY2_TRIAL_RUNNER_V1.md:3 `revision 3 (2026-09-04)`; the other eight revision pins (S20-420, 630, 710, 720, 730, 740, 750, 400) match; grep `RW-` / reweave in WORK_PACKAGES.md: zero hits, while reweave/rw-030..rw-080 records and ADR-0049 are active; scripts/check_local_completion_frontier.py:15,378 reads the file but asserts no revision or row set (PASS).
SPEC_EVIDENCE: evidence/release/operator-decision-ADOPT-REWEAVE-2026-09-06.md:105-111 names `docs/WORK_PACKAGES.md owners` as one of the two role-assignment authorities and binds RW-010/RW-020; REWEAVE master table rows RW-020..RW-040 (lines 595-597) define the lane DAG; README.md:53 "Every next package must follow docs/WORK_PACKAGES.md".
CURRENT_BEHAVIOR: The document README names as the mandatory package authority is four revisions behind on S20-260/270, one behind on S20-430 and S20-620, and silent on the RW lane that has produced eleven landed packages/slices.
DESIRED_INVARIANT: Every revision pin in WORK_PACKAGES.md equals the pinned document's status-line revision, and every active lane (S20 and RW) has a row or an explicit pointer to its DAG authority.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Pure record refresh; add a revision-pin assertion to check_local_completion_frontier.py (or a small check_work_packages.py) so the rows cannot drift silently again, and add RW rows or a pointer row to the REWEAVE master table and reweave/ records.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: revision-pin check over every `docs/spec/X.md (revision N` fragment in WORK_PACKAGES.md against X.md line 3, run in `make quick`
SPECS_TO_UPDATE: docs/WORK_PACKAGES.md rows S20-260, S20-270, S20-430, S20-620 (and S20-420/S20-430 after AT-SS-02); RW rows RW-010..RW-080
CODE_OWNERSHIP: Codex/integrator (DAG file), Vulcan (frontier checker)
REVIEW_REQUIRED: no
IMPLEMENTATION (slice 5e): S20-260/270 rows pinned to VM_EXTENDED_OPCODE_PROFILE_V1 revision 13 (2026-09-06), S20-620 to trial-runner revision 3 (2026-09-04), S20-420/430 to the new revisions; a REWEAVE package table (RW-030..RW-080) added. The revision-pin assertion in the frontier checker was not added: the dossier checker pins WORK_PACKAGE_MARKERS by substring and a per-row revision assertion belongs to each package's own checker (S20-260/270's already reads its contract revision).
```

```text
FINDING_ID: AT-SS-06
TITLE: RESUME.md, README.md phase narrative, and the README Authority block predate REWEAVE adoption and contradict landed state
HYPOTHESIS: The human entry points are frozen at 2026-09-05 (RESUME) and 2026-08-28 (README) and state facts that later commits reversed.
REPOSITORY_EVIDENCE: RESUME.md:1 `# Resume state, 2026-09-05 (night, third push: S20-330 closed, checkpoint)` (9b7054c), :5-10 "the smoke attest of the S20-330 closure is the first action on resume", grep RW-0 in RESUME.md: zero hits; README.md:53-54 (39a6296, 2026-08-28) `S20-330 is deliberately deferred until negotiated session and verified workspace/root authority exist.` vs docs/audits/S20_330_NEGOTIATED_SESSION_CLOSEOUT.md:147-148 round-2 PASS x3 and commit 1e71820 "close S20-330"; README.md:156-163 Authority block lists the master only by its symlink path `/home/greyforge/machineresearch/sley/in-progress/2.0/Sley2.0mastergoal.md` and no REWEAVE master, machine-summary, or resume/locator; ARCHITECTURE.md:3 `Status: M1 normative baseline` (2026-08-27) vs machine-summary phase M2; later truth: 3fc2275 REWEAVE adoption (2026-09-06), reweave/rw-075-continuation-2026-09-07.md:1 `REVIEW_DEFERRED / IMPLEMENTATION_BLOCKED`, check_r2_exit `R2_EXIT: NOT_READY`, RW-080 slices 1-7 under operator override (machine-summary rw075_correction.operator_override_2026_09_07).
SPEC_EVIDENCE: tightening spec 15.5 requires a locator naming canonical branch, active lanes, spec paths, epoch, ABI, package version, profile, blocked gates, last-updated; 15.6 requires remote review packets; section 14 requires records to agree with implementation.
CURRENT_BEHAVIOR: A remote or resuming reader following README or RESUME is told S20-330 is deferred and that the next action is an S20-330 smoke attest; neither document mentions the self-hosting lane, the v2 successor contracts, the NOT_READY R2 gate, or the override under which the last nine commits landed.
DESIRED_INVARIANT: The first-read documents point at one derived, checker-validated locator (AT-SS-14) and contain no phase claim that a later commit reversed.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: README:53 and the Authority block are stale facts in an authority document (real defect, truthful edit); RESUME.md should be superseded by or point at the locator rather than be rewritten by hand each session; ARCHITECTURE.md and the M0 dossier chapters (00-executive-summary.md:3 "Status: M0 complete", 22-independent-review.md:3) are dated records and only need an "as of" pointer, not a rewrite.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: locator checker (AT-SS-14) asserts README Authority block names the locator and both master paths with sha256
SPECS_TO_UPDATE: README.md:53-54 and :156-163, RESUME.md (supersede or refresh with RW state), ARCHITECTURE.md:3 and machineresearch/sley-2.0/00-executive-summary.md:3 ("as of" pointer)
CODE_OWNERSHIP: Codex/integrator
REVIEW_REQUIRED: no
IMPLEMENTATION (slice 5e/5b): README no longer says S20-330 is deferred (closed 2026-09-05, closeout cited); README Authority block names the master-goal override and the locator; RESUME.md is rewritten at campaign closeout as the handoff. ARCHITECTURE.md and the M0 dossier chapter stay dated records (no edit).
```

```text
FINDING_ID: AT-SS-07
TITLE: V1 to V2 supersession of BOOTSTRAP_PROFILE, HOST_ABI, and EXEC_PACKAGE is explicit and the V1 files were never edited after the V2 landed
HYPOTHESIS: A V1 contract may have been edited after its V2 successor landed, or a V2 may lack a supersession statement.
REPOSITORY_EVIDENCE: V2 files all landed in 9574f8a (2026-09-06); `git log -- docs/spec/BOOTSTRAP_PROFILE_1.md` = b2c850f only; `HOST_ABI_V1.md` = e85b89c only; `EXEC_PACKAGE_V1.md` = 624f1c0 only (all at or before 9574f8a, none after); check_bootstrap_profile_1.py, check_host_abi_v1.py, check_exec_package_v1.py PASS; the only V1/V2 basename pairs in docs/spec are EXEC_PACKAGE and HOST_ABI (plus the BOOTSTRAP_PROFILE_1/2 naming).
SPEC_EVIDENCE: docs/spec/BOOTSTRAP_PROFILE_2.md:4,12 `Current R2 candidate. BOOTSTRAP_PROFILE_1 v1 preserved byte-identical as history.` / `Supersedes: BOOTSTRAP_PROFILE_1 v1`; HOST_ABI_V2.md:4,16 same pattern; EXEC_PACKAGE_V2.md:4,23 same pattern; reweave/rw-075-correction.md:64-68 `Supersession: v1 -> retained historical evidence; v2 -> current R2 candidate. V2 is a strict superset adding exactly one import row; no opcode/type/effect/capability broadening.`; tightening spec 2.2 GOOD pattern (`v1 preserved forever, v2 new meaning`).
CURRENT_BEHAVIOR: Exactly the 2.2 GOOD pattern: V1 immutable and still checker-validated, V2 declares supersession and the delta, correction record explains the path. V1 files intentionally carry no forward pointer (adding one would break byte-identity).
DESIRED_INVARIANT: Frozen V1 immutable; V2 names what it supersedes and why. (Holds.)
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: Repository evidence satisfies section 2.2 and 14 without any change; the only residual is the lane-record/governance binding covered by AT-SS-01.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none (existing v1/v2 checkers)
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: Ariadne
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-SS-08
TITLE: ADR index lists every ADR present but does not acknowledge the duplicated identifier ADR-0017
HYPOTHESIS: docs/adr/README.md may omit ADR files or list ADRs that do not exist.
REPOSITORY_EVIDENCE: docs/adr contains 50 ADR files; README.md lists numbers ADR-0001..ADR-0049 (each present); two files share ADR-0017: ADR-0017-candidate-contract-freeze.md and ADR-0017-offline-raw-baseline-runner.md; docs/adr/README.md:25-26 `- ADR-0017: candidate contract and identity freeze` / `- ADR-0017: offline raw baseline evidence runner` with no collision note; README last commit 3fc2275 (2026-09-06) added ADR-0049.
SPEC_EVIDENCE: tightening spec section 14 (records must agree) and 15.5 remote navigation; no repository rule assigns ADR numbers uniquely, but every closeout, WORK_PACKAGES row and machine-summary field cites ADRs by number.
CURRENT_BEHAVIOR: A citation "ADR-0017" is ambiguous between the S20-345 candidate freeze and the S20-610 offline runner; the index reproduces the ambiguity silently.
DESIRED_INVARIANT: Every ADR identifier resolves to exactly one file, or the index states the tolerated collision and how citations disambiguate (by slug).
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Renumbering would rewrite history references in closeouts and the machine summary (net negative); a one-line tolerated-collision note in the index plus slug-qualified citations is additive and truthful.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: optional index check that every docs/adr/ADR-*.md is listed and duplicate numbers are annotated
SPECS_TO_UPDATE: docs/adr/README.md:25-26
CODE_OWNERSHIP: Codex/integrator
REVIEW_REQUIRED: no
IMPLEMENTATION (slice 5e): docs/adr/README.md records the ADR-0017 identifier collision as tolerated with both filenames; no renumbering.
```

```text
FINDING_ID: AT-SS-09
TITLE: Master section 17 and the in-repo Required Contract Index name the same twelve contracts
HYPOTHESIS: The index might have drifted from the master's required-contract list.
REPOSITORY_EVIDENCE: docs/spec/REQUIRED_CONTRACT_INDEX_V1.md:20-31 rows 17.1 sley-scb-object-v1 ... 17.12 sley-protocol-handshake-v1; `python3 scripts/check_required_contract_index.py` -> PASS, required_contracts 12, problems [].
SPEC_EVIDENCE: /home/greyforge/machineresearch/Sley2.0mastergoal.md (sha256 e1e15346...) headings `## 17.1 sley-scb-object-v1` through `## 17.12 sley-protocol-handshake-v1`; string-for-string identical to the index rows in the same order.
CURRENT_BEHAVIOR: Exact agreement, checker-enforced for document, checker, domain, and corpus existence.
DESIRED_INVARIANT: Index equals master section 17. (Holds.)
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: Verified directly against the master file at the stated digest.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: Ariadne
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-SS-10
TITLE: REWEAVE requirements ledger entries reference only commits that exist; the ledger is an out-of-repo R1 draft that RW-020 accepted without regeneration
HYPOTHESIS: Ledger entries claiming landed status may cite commits absent from this repository.
REPOSITORY_EVIDENCE: /home/greyforge/machineresearch/SLEY_REWEAVE_MASTER_PACKAGE_V1/sley-reweave-v1/reweave-r1/requirements-ledger.json: 20 requirements (VERIFIED 1: REQ-LINEAGE; PARTIAL 11; NOT_STARTED 7; NOT_APPLICABLE_WITH_PROOF 1); the only commit-like token is `9b7054c` (REQ-RUST-BOOTSTRAP evidence "baseline commit 9b7054c") and `git cat-file -t 9b7054c` = commit; no entry claims VERIFIED at any other commit; reweave-r1/R1-README.json activation_status "PROPOSED ... R1 NOT passed, R2 NOT started"; reweave-r1/scope-adoption-record.json `adoption_mechanism: NOT YET RECORDED in-repo`; the repository has no copy of the ledger (grep requirements-ledger, REQ-SELFHOST: zero hits); evidence/release/operator-decision-ADOPT-REWEAVE-2026-09-06.md:138-142 `RW-020 ... COMPLETE using the technically accepted ledger draft (addendum section 3; not regenerated) ... sole VERIFIED obligation REQ-LINEAGE`, :118-119 "reconciliation itself remains open, owned by the RW-020 role assignments above ... required at a future RW-020 review"; reweave/rw-030-charter.md:3 "depends on RW-020 COMPLETE".
SPEC_EVIDENCE: REWEAVE master table line 595 `RW-020 | Requirement ledger and retained implementation map | RW-010 | No unclassified active obligations`; docs/adr/ADR-0049 accepted 2026-09-06.
CURRENT_BEHAVIOR: The ledger is a truthful R1 snapshot (its adoption line is stale only because it predates ADR-0049, and it says so); the operator record declares RW-020 complete on that draft and explicitly schedules regeneration for a future RW-020 review. Nothing points at a missing commit.
DESIRED_INVARIANT: No landed claim cites a non-existent commit (holds); regeneration of the ledger into the repository is an already-recorded future RW-020 obligation.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: The specific concern (phantom commits) is disproved; the staleness of the out-of-repo snapshot is already recorded with an owner and a trigger in the operator decision, so no new action is invented here. The locator (AT-SS-14) should name the ledger's canonical path and digest so remote readers can find it.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none now; ledger regeneration at the recorded RW-020 review
CODE_OWNERSHIP: Ariadne (RW-020 traceability/ledger per ADOPT:106)
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-SS-11
TITLE: Machine-summary REVIEW_PENDING tokens after PASS rounds are a documented, checker-bound convention, not drift
HYPOTHESIS: machine-summary.json status fields (e.g. S20_330_IMPLEMENTED_REVIEW_PENDING, S20_400_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING, S20_260_270_EXTENDED_IMPLEMENTED_REVIEW_PENDING) contradict closeouts that record PASS re-reviews.
REPOSITORY_EVIDENCE: machine-summary.json session_handle_profile.status = S20_330_IMPLEMENTED_REVIEW_PENDING; docs/audits/S20_330_NEGOTIATED_SESSION_CLOSEOUT.md:147-154 `Re-review round 2 ... Ariadne PASS, Nabu PASS, Vulcan PASS ... The contract freeze and package completion status remain a separate gate: the summary keeps S20_330_IMPLEMENTED_REVIEW_PENDING with the three PASS obligations superseding the FAIL rounds, matching the S20-400 closure.`; scripts/check_cli_contract.py:19-24 and eleven sibling checkers define DRAFT / REVIEW_PENDING / COMPLETE tokens and assert the summary value; docs/audits/S20_LOCAL_COMPLETION_FRONTIER.md:3 states the same for every implemented-with-reviews-pending package.
SPEC_EVIDENCE: FINDING_REGISTER_V1.md / ADR-0042 (verdict obligations carried per field, package status is a separate gate); tightening spec section 14.
CURRENT_BEHAVIOR: Summary tokens, closeouts, checkers, and the frontier agree on the meaning: implemented, reviewed PASS, freeze/completion gate not yet exercised.
DESIRED_INVARIANT: Summary token semantics documented and enforced. (Holds.)
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: The apparent contradiction is resolved by the closeout's explicit rule and by the checkers that pin the token.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: n/a
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-SS-12
TITLE: Machine summary indexes RW-080 provisional slices only through slice 4 while slices 5, 6, and 7 have landed records; rw-075.md lacks a forward pointer to its correction
HYPOTHESIS: The last three RW-080 slices were committed without the summary entry the earlier slices received.
REPOSITORY_EVIDENCE: machine-summary.json last commit 9b9d394 (slice 4); keys rw080_codec_scaffold_provisional, rw080_uvar_slice1_provisional, rw080_envelope_slice2_provisional, rw080_program_outer_slice3_provisional, rw080_program_body_slice4_provisional present, grep for slice 5/6/7: zero hits; records reweave/rw-080-codec-program-entrypoint-encode.md (`# RW-080 section 1.1 program slice 5 ... provisional C0 construction record`, 9b7c52d), rw-080-codec-program-envelope-compose.md (slice 6, 0a909e1), rw-080-codec-program-namespace.md (slice 7, 560a5f1) each `Status: PROVISIONAL (... operator development override ...)`; reweave/rw-075.md:3-4 `Status: IMPLEMENTED (validation green; independent Ariadne/Nabu reviews PENDING; premium delta re-review PENDING; RW-080 remains BLOCKED)` (624f1c0) vs check_r2_exit `RW075_ariadne_pass: PASS (latest-round verdict: PASS)`, `RW075_nabu_pass: PASS` and reweave/rw-075-correction.md carrying the later state.
SPEC_EVIDENCE: rw-080-contract.md:3-8 (RW-080 BLOCKED until premium PASS plus R2 READY) and machine-summary rw075_correction.operator_override_2026_09_07 (development continuation permitted, provisional labelling required); tightening spec section 14 (review records agree with implementation).
CURRENT_BEHAVIOR: The provisional slices are labelled consistently in their own records and in commit messages, but the summary, which the finding register and dossier derive from, stops at slice 4; a reader of rw-075.md alone sees reviews PENDING that later passed.
DESIRED_INVARIANT: Every landed lane record has a summary entry in the same commit; a superseded lane record names its successor record.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Add the three provisional keys (same shape and wording as slices 1-4, "rw080 stays BLOCKED, R2 NOT_READY") and one "Superseded state: see rw-075-correction.md" line in rw-075.md; no semantic change and no authority claim.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none (provisional status unchanged)
MIGRATION_REQUIRED: no
TESTS_REQUIRED: a summary/records parity check listing every reweave/rw-*.md record and asserting a summary key or explicit exclusion
SPECS_TO_UPDATE: machineresearch/sley-2.0/machine-summary.json rw075_correction section; reweave/rw-075.md:3-4
CODE_OWNERSHIP: Ariadne (REWEAVE lane records)
REVIEW_REQUIRED: no
IMPLEMENTATION (slice 5e): machine-summary rw075_correction indexes RW-080 slices 5, 6 and 7 in the existing provisional wording with commits and records; rw-075.md carries a successor pointer to rw-075-correction.md.
```

```text
FINDING_ID: AT-SS-13
TITLE: Two Tier 1 checkers bind the out-of-repo Sley 2.0 master by a parent-directory-relative path and fail in every worktree, clone, or remote checkout
HYPOTHESIS: Checkers that validate contracts against the canonical master assume the master's location relative to the repository's parent directory.
REPOSITORY_EVIDENCE: scripts/check_transaction_contract.py:33 `MASTER = ROOT.parent / "machineresearch/sley/in-progress/2.0/Sley2.0mastergoal.md"`; scripts/check_candidate_result_contract.py:31 same path; both listed in `make quick` (Makefile:91,95); executed from this worktree: `{"problems": ["missing:/home/greyforge/cache/worktrees/machineresearch/sley/in-progress/2.0/Sley2.0mastergoal.md"], "result": "FAIL"}` (exit 1) and the same "missing:" line from check_candidate_result_contract.py; `grep -l ROOT.parent scripts/*.py` returns only these two; /home/greyforge/machineresearch/sley/in-progress/2.0/Sley2.0mastergoal.md is a symlink to ../../../Sley2.0mastergoal.md (sha256 e1e15346... both ways); README.md:158 and machineresearch/sley-2.0/SPEC_INPUT_DOSSIER.md:3 cite the symlink path.
SPEC_EVIDENCE: tightening spec section 0 (independent reproducibility, remote inspectability), 14 (test vectors/checkers agree with canonical master), 15.6 (remote reviewer inspects exact code), 15.7 (no weakening of privacy to ease review); the master itself is outside the repository by design (task statement).
CURRENT_BEHAVIOR: `make quick` is green only on the single canonical checkout at /home/greyforge/sley2; a remote reviewer or any worktree cannot run the Tier 1 gate without recreating the operator's home-directory layout; the checkers give no override.
DESIRED_INVARIANT: Every Tier 1 checker runs from the repository alone, or reads the master from an explicit, documented override with a pinned digest recorded in-repo, and fails with a distinct SKIP/UNAVAILABLE code (never a silent pass) when the master is absent.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Additive: accept `SLEY2_MASTER_GOAL` (or a locator-declared path) with the expected sha256 pinned in the locator, keep the current default, and report `master:unavailable` distinctly so the failure is attributable; alternatively vendor the exact master sections these two checkers read as a digest-pinned extract. No semantic change to either contract.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: run both checkers from a fresh worktree with and without the override; `make quick` from a worktree
SPECS_TO_UPDATE: none of docs/spec; README.md Authority block and the locator (canonical master path plus sha256); CONTRIBUTING.md validation notes
CODE_OWNERSHIP: Merlin (S20-360/S20-390 checkers), Codex/integrator (locator)
REVIEW_REQUIRED: no
IMPLEMENTATION (slice 5b): SLEY2_MASTER_GOAL override in both checkers with the operator default kept and a distinct master:unavailable problem; verified from the campaign worktree with and without the override; CONTRIBUTING and README document it. The digest pin lives in the locator (canonical_spec.sha256), verified by check_remote_head.py.
```

```text
FINDING_ID: AT-SS-14
TITLE: No remote-head locator exists; machine-summary.json is the canonical machine-readable status mechanism to extend
HYPOTHESIS: The repository may already carry a machine-readable status/index that the section 15.5 locator can extend instead of a new docs/status file.
REPOSITORY_EVIDENCE: no docs/status directory at the pinned commit; inventory in section 2 of this audit: machine-summary.json (hand-maintained, bound by ~30 checkers, carries project/phase/status/m0_commit/final_commit and every package token, contract path, contract_revision, REWEAVE v1/v2 keys), evidence/release/decision-dossier.json (derived from the summary; decision_state BLOCKED; never decides), evidence/review/finding-register.json (derived), scripts/gate_status.py (static NOT_IMPLEMENTED stub, exit 2), scripts/check_r2_exit.py (run-time gate aggregation), docs/spec/REQUIRED_CONTRACT_INDEX_V1.md (traceability, not status), RESUME.md (hand narrative, stale, AT-SS-06), README Authority block (paths only). The lead's 5ff79ef adds scripts/check_remote_consistency.py (git-only freshness, no locator content).
SPEC_EVIDENCE: tightening spec 15.5 "If the repository already has a canonical machine-readable status/index mechanism, extend that mechanism instead of creating redundant status files" and "The locator MUST NOT become a second source of truth for commit identity"; ADR-0043 "decision dossier derived, not decided".
CURRENT_BEHAVIOR: The fields 15.5 requires are scattered: branch/commit in git, spec paths in README:158 and SPEC_INPUT_DOSSIER.md:3 (symlink form) and the operator decision, epoch in SCHEMA_EPOCH_V1.md, ABI/package/profile in the V2 status lines and rw-080-contract.md:9-16, blocked gates in check_r2_exit output and rw-075-continuation, last-updated nowhere.
DESIRED_INVARIANT: One derived, checker-validated locator whose non-commit fields live in machine-summary.json and whose commit fields are read from git at check time; human documents point at it.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Extending machine-summary.json (add a `locator` section: canonical_integration_branch, active_reweave_branches, canonical_spec_path plus sha256, reweave_master_path plus sha256, requirements_ledger_path plus sha256, schema_epoch, host_abi HOST_ABI_V2 bc564653..., exec_package EXEC_PACKAGE_V2 f4958c5e..., bootstrap_profile BOOTSTRAP_PROFILE_2 fb2d8cc8..., known_blocked_gates [R2_EXIT NOT_READY, premium delta FAIL, S20-710 root license, ...], last_updated_utc) and a `scripts/check_remote_head.py` that (a) verifies each sha256 and each version claim against the spec status lines and check_r2_exit, (b) reads commits from git only, and (c) optionally renders docs/status/SLEY2-REMOTE-HEAD.md as a derived view, satisfies 15.5 without a second hand-maintained truth. `make quick` runs the checker; RESUME.md and README point at it.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: check_remote_head.py self-test (stale sha, stale version, missing field each FAIL), run in `make quick` and `make remote-consistency`
SPECS_TO_UPDATE: machineresearch/sley-2.0/machine-summary.json (locator section), README.md Authority block, RESUME.md pointer, CONTRIBUTING.md (locator update rule: same or immediately following validated commit per 15.5)
CODE_OWNERSHIP: Codex/integrator (summary, docs), Vulcan (checker)
REVIEW_REQUIRED: yes, Nabu architecture read of the locator field set (bounded)
IMPLEMENTATION (slice 5f): machine-summary.json gains a top-level `locator` section (branch, latest validated integration commit, active lanes, canonical spec and REWEAVE master paths with sha256 and env overrides, schema epoch, HOST_ABI_V2, EXEC_PACKAGE_V2, BOOTSTRAP_PROFILE_2 with record paths and digests, known blocked gates, remote state, last_updated_utc). scripts/check_remote_head.py verifies every digest against the record it names, the epoch against the summary and fixture, the masters by sha256 when resolvable (UNAVAILABLE otherwise, never a pass), the validated commit's existence and ancestry in git, and lane branch existence; --render writes docs/status/SLEY2-REMOTE-HEAD.md, --check fails on a stale view. Self-test 5 cases (stale digest, missing field, stale epoch, ghost commit, clean). Wired into make remote-consistency, not quick, so the in-flight lanes' Tier 1 gate is untouched; README points at the view.
```

### 3.6 Identity and invalidation graph, hash and preimage hygiene (spec sections 7 and 8)

The full graph (66 identity records, edge list, duplicate-definition table
with its divergence checkers, hygiene matrix) is the separate deliverable
`SLEY-2.0-ARCHITECTURE-TIGHTENING-IDENTITY-MAP.md`. Records follow.


Disposition summary: 1 C_PRE_FREEZE_REPAIR, 8 B_ADDITIVE_NOW, 10 A_ALREADY_SOLVED, 3 D_REJECT, 2 E_DEFER_2_1_PLUS (24 records).

```text
FINDING_ID: AT-IG-01
TITLE: No single normative identity graph existed; rw-075 covers eight compiler-closure hash obligations out of more than sixty identities and sub-digests
HYPOTHESIS: The repository has no one document that lists every canonical identity with domain, preimage, dependencies, invalidation, owner and vector, so section 7 cannot be checked against a committed artifact.
REPOSITORY_EVIDENCE: machineresearch/sley-2.0/reweave/rw-075-hash-inventory.md lists 8 obligations scoped to the SH2 compiler closure and names SLEYPOBS1 without a layout; docs/spec/IDENTIFIERS_V1.md lists 50 domains with two exact preimages (workspace, entity) and delegates the rest to owning contracts; the preimages themselves live in 24 crate files and 7 Python oracles (section 1 records A1 to A18, B01 to B16, C01 to C25, D1 of this document); no in-tree file carries the dependency edge list of section 2.
SPEC_EVIDENCE: SLEY-2.0-ARCHITECTURE-TIGHTENING.md section 7 ("Create one normative dependency graph covering every canonical identity"); REWEAVE master section 6 ("All identifier preimages and hash domains remain explicit"); IDENTIFIERS_V1.md "Content-addressed identifiers" ("The owning contracts supply exact preimages").
CURRENT_BEHAVIOR: Each identity is fully specified by its owning contract and implemented once; the graph is recoverable only by reading all of them. rw-075 is accurate for what it lists but is a closure inventory, not an identity graph.
DESIRED_INVARIANT: One committed record enumerates every identity with the ten section 7 fields and the edge list, and is regenerated or re-verified when a domain is added.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: This document supplies the graph (sections 1 to 4). Committing it (for example under docs/audits next to the campaign record) is additive, changes no byte, and gives AT-G2 a checkable artifact. No redesign; no code.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none (optional: extend scripts/check_required_contract_index.py to require that every registry row is named in the committed graph)
SPECS_TO_UPDATE: docs/audits (new record); rw-075-hash-inventory.md gains one pointer line to the graph
CODE_OWNERSHIP: campaign integrator; ariadne for the pointer line
REVIEW_REQUIRED: campaign independent review (AT-G8)
```

```text
FINDING_ID: AT-IG-02
TITLE: Independently maintained preimage definitions exist for 27 identities and a gate fails on divergence in both directions
HYPOTHESIS: Rust and Python re-implement the same preimages with no checker that fails if they diverge (section 7 last sentence).
REPOSITORY_EVIDENCE: Duplicate definitions (section 3 tables): EntityId (sley-id lib.rs:360 / candidate.py:135), ObjectId (object.rs:94, scb1 lib.rs:412 / codec.py:18, transaction_receipt.py), StateRoot (state-root lib.rs:451 / check_state_root_vector.py:113, transaction_receipt.py:29), TransactionId and ReceiptId (txn codec.rs:1040,495 / transaction_receipt.py:22-25), SchemaEpochId (schema lib.rs:441 / check_schema_epoch_vector.py:61-66), PolicyRootId (policy lib.rs:1958 / transaction_receipt.py:30), CandidateId and ValidationProfileId (mutate codec.rs:4022,4010 / candidate.py:32-34,118), CandidateResultId (candidate_result.rs:584 / candidate_result.py:15-16), accepted-head checksum (txn repository.rs:2893, repo exchange.rs:61-64 / check_repository_exchange_vector.py:17,22), BytecodeCacheKey (vm lib.rs:199 / vm_extended.py:346), image SHA-256 (host_abi.rs:210 / vm_extended.py:390), ExecutionReportId domain step (conformance lib.rs:426 / check_release_demo_vector.py:115-124), RepositoryPackId with leaf and node (repo lib.rs:558,922-955 / check_repository_pack_vector.py:16-127), RepositoryExchangeId with leaf, node, name key (exchange.rs:628,849-932, refs.rs:473 / check_repository_exchange_vector.py:15-23,152-170), MergeConflictId (merge.rs:2123 / check_merge_vector.py), SemanticDeltaId (compare.rs:1245 / check_semantic_comparison_vector.py), RootQueryId (root_query.rs:711 / check_root_backed_query_vector.py), ContextCapsuleId (context_capsule.rs:567 / check_context_capsule_vector.py), IndexSnapshotId arm 2 (snapshot.rs:535 / check_complete_root_index_snapshot_vector.py), ProtocolFrameId and ProtocolHandshakeId (protocol lib.rs:1098,715 / check_smp1_vector.py). Divergence gates: Python side make conformance (Makefile:116-148) over the committed fixture; Rust side make quick regenerates the fixture from the crate emitter and diffs (scripts/generate_*_fixtures.py --check, Makefile:19-52,94-100, each invoking cargo test emit_*_for_fixture_refresh) or reads the fixture directly (sley-scb1 tests/conformance.rs:45,82; sley-mutate codec/fixture_tests.rs:11-18; sley-json-bridge tests.rs:10) or pins the hex in source (state-root lib.rs, schema lib.rs). The standalone pack vector is bound on the Rust side transitively: the exchange fixture embeds the stored pack bytes (exchange payload field 2) and is regenerated under --check. scripts/check_oracle_independence.py forbids the Python side from importing or invoking the Rust side (PASS, 26 sources).
SPEC_EVIDENCE: SLEY-2.0-ARCHITECTURE-TIGHTENING.md section 7 ("Prefer one canonical library/generated definition per preimage. The checker SHALL fail if independently maintained preimage definitions diverge"); Sley2.0mastergoal.md 6.5 ("Neither implementation may derive expected bytes from the other"); REWEAVE master section 6 ("independently check Rust/Sley/independent-oracle conformance over the applicable canonical corpus").
CURRENT_BEHAVIOR: A change to a Rust preimage regenerates different fixture bytes and fails make quick; a change to a Python oracle fails make conformance against the unchanged fixture; a change to the fixture fails both. The duplication is the intended independent-oracle design, not accidental reassembly.
DESIRED_INVARIANT: Every duplicated preimage is arbitrated by one committed fixture that both implementations must reproduce, under gates that run on every make quick and make conformance. Holds.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: The duplication is required by the master (independent oracle) and the divergence checker exists on both sides. The group files' caveat that Rust drift is caught only at fixture refresh was wrong: the refresh is executed and diffed by make quick. No consolidation is authorized; folding the Python oracle into the Rust definition would remove the independence the master requires.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: n/a
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-IG-03
TITLE: The accepted-head checksum codec is implemented in two Rust crates
HYPOTHESIS: sley-txn and sley-repo each hardcode SLEYHD01, the version and the sley2.accepted-head.v1 domain, so the two copies can drift.
REPOSITORY_EVIDENCE: crates/sley-txn/src/repository.rs:48-54 and :2893-2918 (encode_head, decode_head, owner); crates/sley-repo/src/exchange.rs:61-64 and :521-546 (second copy for exchange packaging); cross-crate equality test crates/sley-repo/src/exchange.rs:3149 fixed_head_bytes_match_the_transaction_model; Python re-derivation scripts/check_repository_exchange_vector.py:152-154 under make conformance; exchange import feeds the head back into sley-txn.
SPEC_EVIDENCE: docs/spec/TRANSACTION_MODEL_V1.md "Fixed accepted-head visibility primitive" ("S20-500 later owns those semantics and may reuse the transaction codec and compare-and-swap mechanism without creating a sley-txn -> sley-repo dependency"); section 7 preference for one definition per preimage.
CURRENT_BEHAVIOR: Two copies, bound by a Rust-to-Rust equality test and by the exchange fixture on both sides (AT-IG-02).
DESIRED_INVARIANT: Copies of a preimage definition across crates are pinned to each other by a test that fails on divergence. Holds.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: The duplication is deliberate to avoid a dependency inversion and is checked by a test and a frozen vector.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-txn, sley-repo (ariadne)
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-IG-04
TITLE: One registry domain sley2.observation.v1 covers two preimage framings (SLEYOBS1 and SLEYPOBS1)
HYPOTHESIS: Reused domain across incompatible objects (section 8).
REPOSITORY_EVIDENCE: crates/sley-vm/src/execute.rs:2185 (SLEYOBS1 || u32(1) ...) and :1021 (SLEYPOBS1 || u32(1) ...) both call ObservationId::derive; IDENTIFIERS_V1.md row "deterministic observation" names one domain; EXEC_PACKAGE_V1.md "Bindings" chose the SLEYPOBS1 magic so "package observations can never equal legacy SLEYOBS1 observations".
SPEC_EVIDENCE: IDENTIFIERS_V1.md ("A domain cannot be renamed, aliased, or reused for another preimage"); section 8 "reused domain separation across incompatible objects".
CURRENT_BEHAVIOR: The inner magic is the first preimage bytes and the two magics differ at byte 4 (O versus P) and in length (8 versus 9), so the two families are prefix-disjoint under one BLAKE3 domain; the observation binds the package digest, section digests, profile digest and ABI version in the SLEYPOBS1 arm and not in SLEYOBS1.
DESIRED_INVARIANT: No two objects under one domain can share a preimage. Holds by construction.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: The registry rule targets re-purposing a domain for a different object; here one identifier type has two framings distinguished inside the preimage, which is the same pattern as IndexSnapshotId arms (AT-HH-12). Optional: name both magics on the registry row (documentation only, folded into AT-IG-07).
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none required (see AT-IG-07 for the registry note)
CODE_OWNERSHIP: sley-vm (ariadne)
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-IG-05
TITLE: REWEAVE 21.2 "toolchain-program root" has no dedicated repository identity; the binding it requires is carried by StateRoot plus the package dependency section
HYPOTHESIS: Evidence cannot bind a toolchain-program root because none exists.
REPOSITORY_EVIDENCE: bootstrap-manifest.json S.value = null ("does not exist (SH0: no Sley toolchain program)"); scripts/check_bootstrap_capability.py reports later_stages_unbound [S, C0, C1, C2, C3] and passes by design (docstring lines 21-27); the program root of every executed closure is bound as StateRoot in the cache key (crates/sley-vm/src/lib.rs:213), the dependency section (exec_package.rs:706), the package digest (:911), both observations (execute.rs:1026, :2190) and the execution report (sley-conformance lib.rs:434), with the judged closure fingerprints digested into the dependency section (exec_package.rs:721-725).
SPEC_EVIDENCE: SLEY_2X_REWEAVE_MASTER_SPEC_V1.md 21.2 ("Every result binds applicable source commit, toolchain-program root, compiled image, host binary/ABI, schema epoch, policy root ...") and 13.1 (S is "the immutable canonical root and complete dependency closure of the Sley toolchain program"); docs/audits/SLEY-2.0-ARCHITECTURE-TIGHTENING-AUDIT.md AT-SI-03 (A_ALREADY_SOLVED, same conclusion from the identity-decomposition side).
CURRENT_BEHAVIOR: The binding mechanism exists end to end; the S identity itself is R3 evidence that cannot exist before a Sley toolchain program does.
DESIRED_INVARIANT: The root that execution evidence binds is the accepted StateRoot of the closure it judged, plus the closure fingerprints. Holds.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: Same disposition and evidence as AT-SI-03; recorded here so the identity graph names where the "toolchain-program root" edge lands. Do not invent a placeholder root.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: RW-080 must record which StateRoot is S when it constructs the toolchain graph (already in AT-SI-03)
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: RW-080 lane
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-IG-06
TITLE: Witness roots, provenance roots and discharge receipts are absent at this commit
HYPOTHESIS: Section 7 lists Witness/provenance roots and discharge receipts; they may exist unregistered.
REPOSITORY_EVIDENCE: No sley2.witness*, provenance or discharge domain in IDENTIFIERS_V1.md or any crate; the only mentions are prohibitions of a native discharge_witness(program/value) service (docs/spec/HOST_ABI_V1.md:175; crates/sley-vm/src/host_abi.rs:11; tests/rw070_host_abi_freeze.rs:816-817) and the RW-170 reservation in rw-075-hash-inventory.md "Method".
SPEC_EVIDENCE: REWEAVE master section 16 (Witness integration, future phase); docs/audits/SLEY-2.0-ARCHITECTURE-TIGHTENING-AUDIT.md AT-WB-01 (E_DEFER_2_1_PLUS, Witness not part of the active canonical architecture).
CURRENT_BEHAVIOR: Nothing to inventory; nothing to audit for hygiene.
DESIRED_INVARIANT: When Witness lands, its identities enter the registry with an ADR, fixtures and drift validation like every other domain.
DISPOSITION: D_REJECT
RATIONALE: A graph or hygiene finding against an identity that does not exist would be speculative. The scope question is already recorded as AT-WB-01.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: n/a
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-IG-07
TITLE: IDENTIFIERS_V1.md claims to carry "all fifty domains the implementation derives" while twenty-two sley2.* strings outside crates/ use different digest conventions and are not registry-checked
HYPOTHESIS: The registry's scope statement overclaims, and the shared sley2. namespace is used for BLAKE3 hash-domain prefixes (registry), NUL-terminated SHA-256 prefixes (bench evidence chains) and JSON contract labels (qualification records) with no rule keeping them apart.
REPOSITORY_EVIDENCE: docs/spec/IDENTIFIERS_V1.md paragraph after the table ("The registry now carries all fifty domains the implementation derives, scripts/check_required_contract_index.py compares the two over every crate"); scripts/check_required_contract_index.py:96-107 (scans crates/**/*.rs only); bench/sley2/runner.py:66-67 and bench/raw/runner.py:30-31 (b"sley2.sley2-trial-trace.v1\0", b"sley2.sley2-trial-digest-claim.v1\0", b"sley2.raw-run-manifest.v1\0", b"sley2.raw-trial-digest-claim.v1\0" as SHA-256 prefixes); scripts/build_decision_dossier.py:33, build_finding_register.py:32, build_release_provenance.py:30 and eleven more (JSON contract labels, record D1); host-boundary.json:57 (sley2.host-boundary.v1 label). Set difference computed this session: none of the 22 script-side strings equals a registered domain; scripts that hash with a registered domain (check_*_vector.py, oracle/) use the exact registered string.
SPEC_EVIDENCE: IDENTIFIERS_V1.md "Invariants" ("Domains are closed constants"); RAW_BASELINE_RUNNER_V1.md:85-90 (documents the NUL-terminated SHA-256 convention for the bench chain); SLEY-2.0-ARCHITECTURE-TIGHTENING.md section 8 ("missing domain separation; reused domain separation across incompatible objects") and section 14 (spec synchronization).
CURRENT_BEHAVIOR: No collision and no reuse today. The registry sentence is true for crates and false as written for the repository. A future script-side label that happened to equal a registry domain would not be caught by any checker.
DESIRED_INVARIANT: The registry states exactly what it governs (every domain any crate hashes with, BLAKE3, bare ASCII prefix), names the two other conventions as outside its scope, and a checker asserts that script-side and bench-side sley2.* strings are disjoint from the registry unless the script is the independent oracle for that domain.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Wording-only correction of an authority document plus a small checker assertion; no byte changes. The operator rule against leaving a stale fact in an authority document applies. The alternative of registering the evidence-chain prefixes as domains is rejected: they are SHA-256 with a different framing and are not canonical identities.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: scripts/check_required_contract_index.py gains a disjointness assertion over scripts/ and bench/ (excluding the oracle allowlist); make quick
SPECS_TO_UPDATE: docs/spec/IDENTIFIERS_V1.md (scope paragraph; optional note on the observation row naming SLEYOBS1 and SLEYPOBS1 per AT-IG-04)
CODE_OWNERSHIP: ariadne (registry), thoth or campaign integrator (checker)
REVIEW_REQUIRED: campaign independent review (AT-G8)
```

```text
FINDING_ID: AT-IG-08
TITLE: digest_domain_tag integers in contract descriptors have no global assignment table and follow two numbering conventions
HYPOTHESIS: The descriptor field that names a digest domain by integer is unregistered, so production-epoch assembly could collide.
REPOSITORY_EVIDENCE: Values 3, 4, 8 (docs/spec/SSMC1.md:32, STATE_ROOT_V1.md:49, POLICY_ROOT_V1.md:70) equal the sley-id Domain::ALL ordinals (crates/sley-id/src/lib.rs:46-82); values 18, 19, 20, 21, 22 (REPOSITORY_PACK_V1.md:26, REPOSITORY_EXCHANGE_V1.md:49, SEMANTIC_COMPARISON_V1.md:268, MERGE_V1.md:272, crates/sley-protocol/src/lib.rs:34) do not, and 18 to 22 are the ordinals of observation, execution-report, test-report, repository-pack and protocol-handshake. No code maps the integer to a domain string (grep digest_domain_tag: only descriptor encode/decode at crates/sley-schema/src/lib.rs:232,252,970 and per-epoch uniqueness at :877-893); every contract lives in its own single-descriptor conformance epoch (crates/sley-repo/src/lib.rs:318, exchange.rs:493, compare.rs:415, merge.rs:496, protocol lib.rs:973). Section 5 table of the Group C file lists all eight pairs; all distinct today.
SPEC_EVIDENCE: SCHEMA_EPOCH_V1.md section 4 ("Contract tags, domain tags, and kind tags are unique within one epoch"); POLICY_ROOT_V1.md section 3 ("not the final production epoch assembled from all contracts"); IDENTIFIERS_V1.md has no digest_domain_tag column.
CURRENT_BEHAVIOR: No collision; assembly of the production epoch has no single source for the integers. A future standalone contract for observation, execution-report, test-report or protocol-handshake choosing its ordinal would collide with 18, 19, 20 or 22 and the registry constructor would refuse the assembled epoch.
DESIRED_INVARIANT: One frozen table from digest_domain_tag to domain string (or an explicit statement that the integer is an epoch-local label with no global meaning) that production-epoch assembly reads.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Documentation-only table plus a checker comparison with the crate constants; prevents a real assembly hazard at zero byte cost. Renumbering existing tags would change five conformance epoch ids and every dependent fixture for no present defect (that variant is D_REJECT).
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none now; the table becomes the input to production-epoch assembly
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: scripts/check_required_contract_index.py compares the table with DIGEST_DOMAIN_TAG constants in crates
SPECS_TO_UPDATE: docs/spec/IDENTIFIERS_V1.md or REQUIRED_CONTRACT_INDEX_V1.md (table); SCHEMA_EPOCH_V1.md section 4 (pointer)
CODE_OWNERSHIP: ariadne
REVIEW_REQUIRED: campaign independent review (AT-G8)
```

```text
FINDING_ID: AT-IG-09
TITLE: EntityId does not bind the schema epoch
HYPOTHESIS: entity_kind is an epoch-frozen tag but SchemaEpochId is not in the EntityId preimage, so a later epoch that renumbers kinds could derive a colliding EntityId for a different kind in the same workspace.
REPOSITORY_EVIDENCE: crates/sley-id/src/lib.rs:358-372 (76-byte preimage: workspace, nonce, kind u32be, ordinal u64be); collision checks against live and tombstoned identities at validation phase 4 and at commit (crates/sley-txn/src/repository.rs tombstone ledger; IDENTIFIERS_V1.md "Entity identity").
SPEC_EVIDENCE: Sley2.0mastergoal.md 5.1 EntityId ("generated deterministically from the creating candidate nonce, workspace domain, entity kind, and creation ordinal; collision-checked before acceptance; never reused after deletion"); REWEAVE master section 6 ("Entity IDs are not regenerated during the pivot").
CURRENT_BEHAVIOR: EntityId is a stable logical identity that must survive epochs; kind tags are frozen by contract; collisions are rejected by the ledger.
DESIRED_INVARIANT: Logical identity is epoch-independent and collision-checked. Holds.
DISPOSITION: D_REJECT
RATIONALE: Binding the epoch would break the master's stability requirement; the collision ledger is the designed defence.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-id (ariadne)
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-IG-10
TITLE: Seventeen single-owner identities have no independent oracle
HYPOTHESIS: Section 8 item 6 ("independent conformance verification where required") is unmet for identities that have exactly one Rust owner.
REPOSITORY_EVIDENCE: Rust-only with no Python byte-layout re-derivation: WorkspaceId (fixed vector only), CapabilityTokenDigest and MAC (scripts/check_capability_token.py:126-128 compares machine-summary pins only), CapabilitySummaryDigest (check_candidate_contract_freeze.py prose), CandidateAttemptDigest, ValidationContextDigest and components, PhaseEvidenceDigest (opaque fixed32 in candidate_result.py), SemanticFingerprint and ValueHash (vm_extended.py:9-12 excludes them; check_fingerprint_impact_profile.py is a marker check), ObservationId both framings (check_vm_execution_profile.py marker check), section and package digests (check_exec_package_v*.py hash the JSON record), TestReportId (pin cross-check only), branch record and ref digests (check_ref_branch_contract.py pins strings), merge-plan nonce, QueryId, RestrictedQueryCapsuleId (pin only), SessionId (random nonce by design), ReferenceAdapterId, AdapterStateId, AdapterTranscriptId (pin only). Each has one owner function (section 1 records) and, except the cases in AT-HH-04, a frozen Rust vector.
SPEC_EVIDENCE: SLEY-2.0-ARCHITECTURE-TIGHTENING.md section 8 items 5 and 6; Sley2.0mastergoal.md 6.5 (independent oracle required for SCB1 encoders/decoders before GA; "The oracle is for conformance only. It must not become a second semantic kernel"); REWEAVE master section 6 (Rust/Sley/independent-oracle conformance "over the applicable canonical corpus" at final implementation); docs/audits AT-CL-03 (COMPLETE independent coverage is scoped to fixture families and the three uncovered required contracts are disclosed).
CURRENT_BEHAVIOR: One owner per preimage (section 7 satisfied); independent verification exists for every SCB1 envelope and every stored repository, protocol and query-corpus identity, and is absent for derived evidence digests, session-local identities and the capability family. No spec names which of these are "required".
DESIRED_INVARIANT: Independent oracles cover every identity that a second implementation (the Sley toolchain) must reproduce; derived evidence digests are covered when that implementation exists.
DISPOSITION: E_DEFER_2_1_PLUS
RATIONALE: Not required for correct 2.0: the master requires the independent oracle for canonical encoding and the required contracts, which are covered; the derived evidence digests are constructed only by the in-process validator or VM by design, and the Sley toolchain is the intended second implementation for fingerprints and value hashes. Record so the SH2 lane adds oracles for SemanticFingerprint and ValueHash first (they enter package identity). If the lead applies "where required" to CapabilityTokenDigest and CapabilitySummaryDigest because they enter TransactionId, the cheap path is the frozen-vector step in AT-HH-04, not a new oracle.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: the SH2 conformance corpus should include fingerprint and value-hash vectors
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none now
SPECS_TO_UPDATE: none now
CODE_OWNERSHIP: RW lane (ariadne)
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-IG-11
TITLE: NATIVE_REFS_BRANCHES_V1.md and MERGE_V1.md say their domains are not sley-id registry domains while IDENTIFIERS_V1.md lists them
HYPOTHESIS: Two owning contracts contradict the registry about whether their separators are registered.
REPOSITORY_EVIDENCE: docs/spec/NATIVE_REFS_BRANCHES_V1.md:99-101 ("branch_record_digest ... is not added to the sley-id domain registry"); docs/spec/MERGE_V1.md:210 ("sley2.merge-plan-nonce.v1 is a preimage separator, not a sley-id domain"); docs/spec/IDENTIFIERS_V1.md table rows for sley2.branch-record.v1, sley2.branch-ref.v1, sley2.branch-name-path.v1 (S20-500) and sley2.merge-plan-nonce.v1 (S20-520) added under ADR-0048; scripts/check_required_contract_index.py:104-107 fails if any crate-derived sley2.* string is absent from that file.
SPEC_EVIDENCE: IDENTIFIERS_V1.md ("a domain may be derived by any crate that hashes"); SLEY-2.0-ARCHITECTURE-TIGHTENING.md section 14 (canonical specification synchronization).
CURRENT_BEHAVIOR: Bytes and code are consistent; the prose in two owning contracts is stale relative to ADR-0048.
DESIRED_INVARIANT: Owning contracts state that every domain string a crate hashes with is in the registry and point to it.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Documentation-only synchronization of a stale fact in two authority documents; no byte impact.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none beyond the existing drift check
SPECS_TO_UPDATE: docs/spec/NATIVE_REFS_BRANCHES_V1.md section 4; docs/spec/MERGE_V1.md "Merge plan"
CODE_OWNERSHIP: ariadne
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-HH-01
TITLE: The Rust BOOTSTRAP_PROFILE_1_DIGEST literal is a mis-transcription of the frozen profile digest, and the only checker pins four bytes of it
HYPOTHESIS: A digest constant that enters the v1 package digest, v1 admission receipt and v1 SLEYPOBS1 observation does not equal the SHA-256 of the record it names, and no gate can detect it.
REPOSITORY_EVIDENCE: crates/sley-vm/src/exec_package.rs:112-115 decodes to 4f2691504b5c756e a1f5ef01e6e998cc 4cd628d4b524b038 b10d583bfefd6330; sha256sum of conformance/bootstrap-profile/v1/profile.json (recomputed this session) is 4f2691504b5c756e ae1f5ef01e6e998c c4cd628d4b524b03 8b10d583bfefd630 (the literal drops the "e" at hex offset 17 and inserts a "3" before the final "0"). The documented value appears in docs/spec/BOOTSTRAP_PROFILE_1.md:5, EXEC_PACKAGE_V1.md, HOST_ABI_V1.md, conformance/exec-package/v1/exec-package.json, conformance/host-abi/v1/host-abi.json, conformance/bootstrap-profile/v1/SHA256SUMS, machine-summary.json and bootstrap-manifest.json, and scripts/check_bootstrap_profile_1.py:43-46 verifies the file against it on every make quick. The v2 literal (:123-126) equals the SHA-256 of conformance/bootstrap-profile/v2/profile.json (fb2d8cc8...847459, recomputed). The only code-side pin is scripts/check_exec_package_markers.py:31 ("BOOTSTRAP_PROFILE_1_DIGEST", "0x4f, 0x26, 0x91, 0x50"), a four-byte prefix the drifted literal satisfies; no Rust test hashes profile.json (grep of crates/sley-vm for bootstrap-profile/v and profile.json: none). All v1 uses (:839 package preimage, :939 receipt, :1023 approval, :1060, :1222 verification) compare against the same wrong constant, so v1 is internally consistent but binds the digest of nothing. Re-verified at HEAD 197198d6 (exec_package.rs diff since baseline is two doc comments). No committed evidence carries a v1 package digest hex (grep of evidence/, machineresearch/sley-2.0, docs: none) and no fixture pins one.
SPEC_EVIDENCE: docs/spec/EXEC_PACKAGE_V1.md "Bindings" ("BOOTSTRAP_PROFILE_1 (4f26...d630 ...) is bound by digest"); BOOTSTRAP_PROFILE_1.md:5-9 ("the manifest stage P binds this digest"); SLEY-2.0-ARCHITECTURE-TIGHTENING.md section 7 ("The checker SHALL fail if independently maintained preimage definitions diverge") and section 8 ("omitted dependency binding; omitted schema/ABI/profile binding"); section 2.2 (no silent reinterpretation of a frozen preimage).
CURRENT_BEHAVIOR: Every v1 package digest and v1 receipt carries 32 bytes that are not the profile digest the contract says they are. v2 (the current R2 candidate) is correct. Two copies of the value are maintained (Rust literal, JSON record) with no full-width comparison between them.
DESIRED_INVARIANT: Every digest constant that enters a canonical preimage equals the SHA-256 of the record it names, and a test fails on any byte of divergence.
DISPOSITION: C_PRE_FREEZE_REPAIR
RATIONALE: This is a demonstrated canonical-identity defect (a preimage binds a value that identifies nothing) caught before freeze. Bounded repair, lead's choice between two variants under section 2.2: (a) correct the v1 literal to the documented digest and record the correction as a revision note in EXEC_PACKAGE_V1.md (no persisted v1 digest exists, so no frozen evidence is redefined), or (b) leave the v1 literal byte-identical as history and declare v1 packages non-evidence in EXEC_PACKAGE_V2.md. Either way add a Rust test that include_bytes! both profile.json records, hashes them with sha2, and compares all 32 bytes to both constants, and widen check_exec_package_markers.py to the full literal. Variant (a) is recommended because v1 is "preserved functional for legacy evidence only" and legacy evidence that binds a nonexistent digest is not evidence.
CANONICAL_IMPACT: v1 package digest, v1 receipt profile_digest and v1 SLEYPOBS1 observation values change under variant (a); v2 unaffected; no registry domain changes
SCHEMA_IMPACT: none
ABI_IMPACT: none (HOST_ABI_VERSION field unchanged; host-abi.json v1 already records the correct digest)
SELFHOST_IMPACT: none for R2 (v2 is the candidate); the bootstrap manifest stage P already binds the correct value
MIGRATION_REQUIRED: no (no persisted v1 package digest found; confirm before choosing variant a)
TESTS_REQUIRED: full 32-byte binding test for BOOTSTRAP_PROFILE_1_DIGEST and BOOTSTRAP_PROFILE_2_DIGEST against the profile.json bytes; check_exec_package_markers.py full-literal pin; cargo test -p sley-vm rw075_exec_closure; make quick
SPECS_TO_UPDATE: docs/spec/EXEC_PACKAGE_V1.md (revision note) or EXEC_PACKAGE_V2.md (non-evidence statement); machineresearch/sley-2.0/reweave/rw-075-hash-inventory.md item 4 or a correction record
CODE_OWNERSHIP: crates/sley-vm/src/exec_package.rs, scripts/check_exec_package_markers.py (ariadne); vulcan review of the test
REVIEW_REQUIRED: yes, campaign independent review (AT-G8) and Ariadne contract review
IMPLEMENTATION (slice 6a, variant a): the literal in crates/sley-vm/src/exec_package.rs now equals the raw-byte SHA-256 of conformance/bootstrap-profile/v1/profile.json (4f2691504b5c756eae1f5ef01e6e998cc4cd628d4b524b038b10d583bfefd630; the old bytes were a shifted hex transcription). No persisted v1 package digest exists in any record, fixture, or evidence file, so no frozen identity is reinterpreted; the V1 contract document already stated the correct digest, so this is the implementation catching up with its frozen spec. check_exec_package_markers.py replaced the 4-byte prefix pins with a full 32-byte comparison of both profile-digest literals against their records (a scratch copy with the old literal fails exec-rs-digest-drift). cargo test -p sley-vm all suites pass; check_exec_package_v1/v2 PASS.
```

```text
FINDING_ID: AT-HH-02
TITLE: Sley 2.0 master wording lists the digest as part of the hashed object for ObjectId, packs, capsules and candidates, which reads as a recursive definition
HYPOTHESIS: Master text suggests the digest is inside its own preimage.
REPOSITORY_EVIDENCE: Code and subordinate contracts are non-recursive everywhere: docs/spec/SCB1.md section 2 ("digest is outside its own preimage ... stored_bytes = envelope_preimage || digest"); crates/sley-mutate/src/object.rs:94-96; docs/spec/OBJECT_STORE_V1.md "Stored object record"; pack trailer outside preimage crates/sley-repo/src/lib.rs:566-567 and REPOSITORY_PACK_V1.md; capsule id derived over the response record which carries no capsule id (crates/sley-query/src/context_capsule.rs:567-640, capsule.rs:285-287); candidate trailer outside preimage crates/sley-mutate/src/codec.rs:4022-4038.
SPEC_EVIDENCE: Sley2.0mastergoal.md:631-640 (ObjectId = BLAKE3-256("sley2.object.v1" || canonical_object_bytes)) read with :660-669 (5.2: "A canonical object contains: ... canonical digest"); :742-752 (5.7 pack "MUST contain ... digest tree"); :1136-1152 (8.3 capsule "MUST contain ... capsule digest"); :1366-1382 (10.1 candidate "MUST bind to ... candidate digest"). SLEY-2.0-ARCHITECTURE-TIGHTENING.md section 8 last paragraph ("If current master wording suggests a recursive definition, the master spec MUST be clarified even if lower-level code is already correct").
CURRENT_BEHAVIOR: A reader of the master alone could conclude that canonical_object_bytes includes the digest. Every implementation and every subordinate contract excludes the trailer.
DESIRED_INVARIANT: The master states once that every content address is computed over the object's canonical bytes excluding the digest, that the digest is a trailer, and that "contains a digest" in the content lists means "is stored with".
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Required by section 8 even though the code is correct; one clarifying sentence in section 5.1 (and a cross-reference from 5.2, 5.7, 8.3, 10.1) with no canonical change.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: /home/greyforge/machineresearch/Sley2.0mastergoal.md 5.1 (ObjectId form), 5.2, 5.7, 8.3, 10.1
CODE_OWNERSHIP: campaign integrator (master edit under section 14 rules); maat doctrine check
REVIEW_REQUIRED: yes (master edit)
```

```text
FINDING_ID: AT-HH-03
TITLE: The master calls TransactionId the content address of a "transaction receipt" while the repository defines TransactionId over the transaction core and ReceiptId over the complete receipt
HYPOTHESIS: Master wording conflates two identities.
REPOSITORY_EVIDENCE: crates/sley-txn/src/codec.rs:452-464 (TransactionId over SLEYTXN1 core), :495-513 (ReceiptId over SLEYRCP1 receipt that embeds the stored transaction); docs/spec/TRANSACTION_MODEL_V1.md "Transaction identity" and "Complete persisted receipt"; docs/spec/IDENTIFIERS_V1.md "Content-addressed identifiers" ("ADR-0021 names its preimage the canonical parent-bound transaction receipt core. ReceiptId independently authenticates the complete persisted receipt").
SPEC_EVIDENCE: Sley2.0mastergoal.md:650 ("TransactionId: Content address of a canonical parent-bound transaction receipt"); SLEY-2.0-ARCHITECTURE-TIGHTENING.md section 14 (spec synchronization) and section 8 (prose definition must match the exact preimage).
CURRENT_BEHAVIOR: Consistent and non-cyclic in code and subordinate specs; the master names one identity where two exist.
DESIRED_INVARIANT: The master names both identities and their relation (receipt contains transaction; transaction excludes receipt).
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Wording synchronization of the master with ADR-0021 and TRANSACTION_MODEL_V1; no canonical change.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: /home/greyforge/machineresearch/Sley2.0mastergoal.md 5.1 TransactionId entry (add ReceiptId)
CODE_OWNERSHIP: campaign integrator; maat
REVIEW_REQUIRED: yes (master edit)
```

```text
FINDING_ID: AT-HH-04
TITLE: Some canonical evidence identities lack a spec byte layout or a frozen positive vector
HYPOTHESIS: Section 8 items 2 and 3 (exact byte preimage in prose, frozen positive vector) are unmet for a subset of identities.
REPOSITORY_EVIDENCE: SLEYPOBS1 package-bound observation: layout only at crates/sley-vm/src/execute.rs:1000-1066 (including the RESTRICTED_V1.vm_version constant pushed at :1038); EXEC_PACKAGE_V1.md and exec-package.json give a prose field list without order or widths; no preimage or id hex pinned (tests/rw075_exec_closure.rs:740 proves inequality and repeatability only). Package digest v1/v2 and the four section digests: no pinned 32-byte value anywhere (exec-package.json records carry only profile digests and the record's own SHA-256; Rust tests exec_package.rs:1335-1372 prove determinism and tamper sensitivity). Branch record and branch ref digests: no frozen hex (only perturbation tests refs.rs:9218-9264, :10320-10355; the exchange vector freezes one of each transitively as opaque stored bytes). CandidateAttemptDigest, ValidationContextDigest and components, PhaseEvidenceDigest: no standalone frozen hex; opaque fixed32 fields in the candidate-result and transaction-receipt corpora. CapabilityTokenDigest, authenticator and CapabilitySummaryDigest: frozen only as machine-summary pins (scripts/check_capability_token.py:126-128) and a Rust unit constant (capability_summary.rs:275), not in a conformance JSON. Contrast: SLEYOBS1 has both (VM_EXECUTION_PROFILE_V1.md section 6; execute.rs:3013 pins the 420-byte preimage).
SPEC_EVIDENCE: SLEY-2.0-ARCHITECTURE-TIGHTENING.md section 8 ("Each canonical identity MUST have: 1. prose definition; 2. exact byte-preimage definition; 3. frozen positive vector"); EXEC_PACKAGE_V1.md "Bindings" (prose only for SLEYPOBS1).
CURRENT_BEHAVIOR: Correct and deterministic in code; the listed identities are unfrozen (a silent layout change would pass every gate because no vector pins the bytes) and, for SLEYPOBS1, independently unimplementable from the specs.
DESIRED_INVARIANT: Each listed identity has its byte layout in the owning spec and one fixed preimage-plus-digest vector consumed by a test.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Tests and documentation only; the vectors freeze current bytes and change nothing canonical. Priority order: SLEYPOBS1 and package/section digests (they are the R2 execution identity), then branch record/ref, then the capability pair (they enter TransactionId), then attempt/context/phase.
CANONICAL_IMPACT: none (freezes current bytes)
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: pinned preimage and digest vectors in Rust tests or conformance JSON for each listed identity; make quick, make conformance
SPECS_TO_UPDATE: docs/spec/EXEC_PACKAGE_V2.md (SLEYPOBS1 byte layout; the v2 path is the live one), NATIVE_REFS_BRANCHES_V1.md (vectors), CAPABILITY_TOKEN_V1.md and CAPABILITY_SUMMARY_V1.md (vector pointers), CANDIDATE_RESULT_V1.md (vector pointers)
CODE_OWNERSHIP: sley-vm, sley-repo, sley-policy (ariadne); vulcan for test review
REVIEW_REQUIRED: campaign independent review (AT-G8)
```

```text
FINDING_ID: AT-HH-05
TITLE: Negative-vector coverage for outer trailing bytes is missing for the exchange envelope and Rust-side for the pack envelope
HYPOTHESIS: Section 8 item 4 (negative vectors where meaningful) is met unevenly across stored envelopes.
REPOSITORY_EVIDENCE: conformance/repository-exchange/v1/rejected.json ids: flip-trailer, nested-exchange, reversed-branches, foreign-head, open-ancestry (no trailing-byte); crates/sley-repo/src/exchange.rs tests cover only a nested receipt trailing byte (:3019); the decoder does reject (exchange.rs:668-673). conformance/repository-pack/v1/rejected.json has trailing-byte (id list includes it; generated by scripts/generate_repository_pack_rejections.py from the accepted bytes and verified by the Python oracle) but no Rust test exercises the Rust pack decoder with it and no crate reads that rejected corpus (readers: sley-mutate, sley-scb1 only). Every other stored envelope has both a fixture and a Rust test (state-root lib.rs:1127-1130, txn codec.rs:1361-1371, schema lib.rs:1444-1462, policy lib.rs:2956-2959, candidate_result.rs:1293-1304, merge.rs:2779-2782, compare.rs:2455-2459, protocol length check :1146-1149, image rw070_host_abi_freeze.rs:1065-1068).
SPEC_EVIDENCE: SLEY-2.0-ARCHITECTURE-TIGHTENING.md section 8 ("trailing-byte acceptance" and "malformed/negative vectors where meaningful"); SCB1.md section 2 ("No bytes follow digest"); REPOSITORY_EXCHANGE_V1.md and REPOSITORY_PACK_V1.md strict-import rules.
CURRENT_BEHAVIOR: Code paths correct by inspection; a regression in the Rust exchange trailing check would be caught by no test or fixture, and a regression in the Rust pack trailing check only by the Python-side corpus, which does not exercise Rust.
DESIRED_INVARIANT: Each stored envelope has a trailing-byte negative vector consumed by both implementations.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Tests and one fixture row only; no canonical change.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: add trailing-byte to the exchange rejected corpus (scripts/generate_repository_exchange_fixtures.py) and Rust assertions for pack and exchange outer trailing bytes; make quick, make conformance
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-repo (ariadne)
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-HH-06
TITLE: Execution-package "envelope" wording and unreachable framing codes (already repaired at HEAD as AT-EC-04)
HYPOTHESIS: The package identity prose says "over the envelope bytes" while the preimage is a header of digests, and four PACKAGE_* framing codes are declared but unreachable.
REPOSITORY_EVIDENCE: At 560a5f16: crates/sley-vm/src/exec_package.rs:263 and :325 ("SHA-256 over the complete envelope bytes"), :141-153 (UnknownMagic, UnsupportedVersion, Truncated, TrailingData never constructed), :836-850 and :896-910 (actual header preimage); EXEC_PACKAGE_V1.md:77. At HEAD 197198d6: commit 3e6939b5 corrected both doc comments (exec_package.rs:262-264, :327) and added "Identity versus serialization" to docs/spec/EXEC_PACKAGE_V2.md:68-85 stating the header preimage, the absence of a serialized envelope, and the reserved codes.
SPEC_EVIDENCE: docs/audits/SLEY-2.0-ARCHITECTURE-TIGHTENING-AUDIT.md AT-EC-04 (B_ADDITIVE_NOW, implemented in slice 2) and AT-EC-07 (E_DEFER for the envelope codec); SLEY-2.0-ARCHITECTURE-TIGHTENING.md section 8 items 1 and 2.
CURRENT_BEHAVIOR: Repaired at HEAD; B against the pinned baseline.
DESIRED_INVARIANT: Prose matches the exact preimage; dead framing codes are documented as reserved. Holds at HEAD.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: Duplicate of AT-EC-04, recorded here for hygiene-matrix traceability; when the RW-080 envelope codec lands it needs its own trailing and non-minimal vectors (AT-EC-07 already says so).
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none (AT-EC-04 ran check_exec_package_v2.py, check_exec_package_markers.py, rw075_exec_closure)
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-vm (ariadne)
REVIEW_REQUIRED: covered by AT-EC-04 review
```

```text
FINDING_ID: AT-HH-07
TITLE: Length-prefix conventions differ across evidence digests (u64be versus uvar)
HYPOTHESIS: Mixed conventions are a length-ambiguity risk.
REPOSITORY_EVIDENCE: crates/sley-policy/src/candidate_result.rs:481 (SLEYATT1 u64be len), :493 (phase evidence uvar len), crates/sley-policy/src/candidate_validation.rs:592 (u64be), crates/sley-policy/src/lib.rs:1799 (SLEYCAPD u64be), every SCB1 envelope uvar; query, adapter and VM families use u64be counts and lengths throughout (query.rs:897-901, adapter lib.rs:1267-1275, fingerprint.rs Encoder).
SPEC_EVIDENCE: Each owning contract states its own convention exactly (CANDIDATE_RESULT_V1.md sections 3 and 6, CAPABILITY_TOKEN_V1.md section 3, SCB1.md section 4); section 8 "length ambiguity".
CURRENT_BEHAVIOR: Each preimage is unambiguous on its own, domain-separated, and frozen; no two conventions meet inside one preimage.
DESIRED_INVARIANT: No preimage admits two parses. Holds.
DISPOSITION: D_REJECT
RATIONALE: Normalizing would change canonical bytes for no defect; the mix is a style difference, not an ambiguity.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: n/a
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-HH-08
TITLE: SCB1.md names EmbeddedContractBytes, a type no epoch, descriptor, oracle or crate declares
HYPOTHESIS: The nested-envelope rule in the encoding contract is stated in terms of a phantom type.
REPOSITORY_EVIDENCE: docs/spec/SCB1.md:55-58 is the only occurrence in docs, crates, scripts and oracle (grep this session). Every embedding is a plain Bytes field decoded separately by its owning contract: pack root_entry and object_entry stored_bytes (REPOSITORY_PACK_V1.md field schema), exchange object_pack and receipt stored_bytes (REPOSITORY_EXCHANGE_V1.md), receipt fields 3 to 7 (TRANSACTION_MODEL_V1.md), with nested-contract checks such as exchange.rs:958 embedded_pack_header_is_tag_170.
SPEC_EVIDENCE: SCB1.md section 2 sentence; Sley2.0mastergoal.md section 6 does not mention the type.
CURRENT_BEHAVIOR: Behavior matches the sentence's intent (opaque until separately decoded with explicit limits); an independent implementer reading SCB1.md expects a declared type and a decoder rule keyed on it that does not exist; the Python oracle passes without implementing it.
DESIRED_INVARIANT: SCB1.md states the realized rule: embedded envelopes travel as Bytes fields whose owning contract names the nested contract and decodes them separately.
DISPOSITION: E_DEFER_2_1_PLUS
RATIONALE: Useful clarification with no byte effect and no present ambiguity in any realized contract; fold into the next SCB1.md revision rather than a standalone pre-freeze edit.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: docs/spec/SCB1.md section 2 (future revision)
CODE_OWNERSHIP: ariadne
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-HH-09
TITLE: ValueHash canonical-form precondition is enforced by callers, not by the hash function
HYPOTHESIS: hash_validated_value could hash a non-canonical value (map order, NaN) and produce a second identity for equal values.
REPOSITORY_EVIDENCE: crates/sley-ssmc/src/fingerprint.rs:282-309 hashes the encoded type and data as given; VM_EXECUTION_PROFILE_V1.md section 1 requires VM_EXEC_INPUT_NOT_CANONICAL refusal before any hash; FINGERPRINT_IMPACT_PROFILE_V1.md section 6 ("Only a constant that passes S20-210 ... may be hashed"); SCB1.md section 7 (VM canonicalizes NaN and zero before a value can become canonical).
SPEC_EVIDENCE: Sley2.0mastergoal.md 7 ("NaN results must be canonicalized after every operation"; "map iteration must be canonical").
CURRENT_BEHAVIOR: The precondition is a documented caller contract and enforced at every VM input and result boundary.
DESIRED_INVARIANT: A ValueHash is computed only over canonical bytes. Holds by layered contract.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: The division (validate, then hash) is the stated design; moving validation into the hash function would duplicate S20-210 inside sley-ssmc.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-ssmc, sley-vm (ariadne)
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-HH-10
TITLE: The SMP1 handshake transcript concatenates three records without length words
HYPOTHESIS: client_hello || server_hello || selection without length framing could admit two parses.
REPOSITORY_EVIDENCE: crates/sley-protocol/src/lib.rs:715-727 (transcript), :661-681 (Hello::encode is one SCB1 record), :812-823 (SelectedProfile::preimage is one SCB1 record); a canonical record is prefix-delimited by its field count and per-field lengths; both peers hash the observed bytes; independent oracle scripts/check_smp1_vector.py rebuilds the transcript and passes; T45 downgrade test lib.rs:1976.
SPEC_EVIDENCE: SMP1.md section 2; SCB1.md sections 4 and 5 (record encoding is self-delimiting).
CURRENT_BEHAVIOR: Exactly one parse; no ambiguity demonstrable.
DESIRED_INVARIANT: The transcript preimage has one parse. Holds.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: Self-delimitation of canonical records is sufficient; a prose note in SMP1.md would be a nicety.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-protocol (ariadne)
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-HH-11
TITLE: Pack and exchange Merkle trees promote an unpaired final digest unchanged
HYPOTHESIS: Promotion could let a leaf stand in for a node.
REPOSITORY_EVIDENCE: Leaves and nodes use distinct domains (crates/sley-repo/src/lib.rs:59-60, exchange.rs:59-60); the payload stores the exact ordered leaf list and count, which the verifier recomputes before comparing the root (lib.rs:889-895 verify_digest_tree; exchange "Digest tree"); Python reproduces both trees (check_repository_pack_vector.py:111-127, check_repository_exchange_vector.py:153-170).
SPEC_EVIDENCE: REPOSITORY_PACK_V1.md and REPOSITORY_EXCHANGE_V1.md "Digest tree".
CURRENT_BEHAVIOR: Distinct domains plus a stored leaf list make a second-preimage substitution detectable.
DESIRED_INVARIANT: Tree root cannot be forged by re-labelling a leaf as a node. Holds.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: Proof above.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-repo (ariadne)
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-HH-12
TITLE: IndexSnapshotId uses one domain for the restricted arm and the complete-root arm
HYPOTHESIS: Arm 1 and arm 2 records could be confused under one domain.
REPOSITORY_EVIDENCE: The arm tag is inside the preimage (crates/sley-query/src/snapshot.rs:557); arm 2 requires a Some root (COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md lines 47-50); each consumer accepts exactly one arm (query.rs:412-416, root_query.rs:667-669); rejected fixture restricted-arm-tag in conformance/complete-root-index-snapshot/v1.
SPEC_EVIDENCE: COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md lines 60-64.
CURRENT_BEHAVIOR: Bound inside the preimage and refused on the wrong arm.
DESIRED_INVARIANT: One domain, arms disjoint by bound tag. Holds.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: Same pattern as AT-IG-04.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-query (ariadne)
REVIEW_REQUIRED: no
```

```text
FINDING_ID: AT-HH-13
TITLE: The deterministic merge-plan nonce feeds EntityId derivation
HYPOTHESIS: A deterministic nonce could make merge-created EntityIds collide across repositories or replays.
REPOSITORY_EVIDENCE: nonce = BLAKE3("sley2.merge-plan-nonce.v1" || A.root || judged_merged.root) (crates/sley-repo/src/merge.rs:1373-1377); EntityId also binds WorkspaceId, kind and ordinal (crates/sley-id/src/lib.rs:358-372); a derived identity that names a live entity fails MERGE_PLAN_UNSUPPORTED (MERGE_V1.md "Merge plan"); the transaction layer collision-checks against live and tombstoned ids.
SPEC_EVIDENCE: MERGE_V1.md "Merge plan" (determinism is the stated design: two peers computing the same merge derive the same plan); IDENTIFIERS_V1.md "Entity identity".
CURRENT_BEHAVIOR: Deterministic by design; collision is refused rather than possible.
DESIRED_INVARIANT: Identical merges derive identical plans; collisions never reach accepted state. Holds.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: Proof above.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: sley-repo (ariadne)
REVIEW_REQUIRED: no
```

### 3.7 Machine-write ergonomics (spec section 9)

Method: representative candidates for the eight edit classes were composed and
encoded through the repository's independent oracle after showing it
reproduces the committed mutation-candidate fixture byte for byte; every
candidate strictly re-imports. No trial data exists at this commit
(trials_executed 0), so round trips, validation failures and repair rounds are
derived minima with typical fail-closed causes, marked
NOT_MEASURABLE_AT_THIS_COMMIT. Primitive counts per class run from 1 to 18
and record bytes from 469 to 3819, inside every frozen ceiling; no material
machine-generation problem is demonstrated, so higher-order operations are
deferred. Records follow.


```text
FINDING_ID: AT-MW-01
TITLE: Higher-order typed mutation operations are not justified by a demonstrated machine-generation problem
HYPOTHESIS: Primitive-only mutation makes representative agent edits so long, large, or failure-prone that composite operations are needed before the 2.0 freeze.
REPOSITORY_EVIDENCE: ENCODED table in section 2 (measure_mw.py through oracle/scb1/src/sley2_scb1_oracle/candidate.py, oracle shown byte-identical to conformance/mutation-candidate/v1/accepted.json): 1 to 18 primitives, 469 B to 3819 B record bytes across the eight edit classes; ceilings docs/spec/VALIDATION_PROFILE_V1.md table (65,535 operations, 67,108,864 bytes); no trials: machineresearch/sley-2.0/machine-summary.json:2817, machineresearch/sley-2.0/18-legacy-succession-results.md:3, docs/spec/SLEY2_TRIAL_RUNNER_V1.md:259; composition surface crates/sley-protocol/src/server.rs:1284-1295; descriptor table crates/sley-mutate/src/generated.rs:682 (179 entries).
SPEC_EVIDENCE: SLEY-2.0-ARCHITECTURE-TIGHTENING.md:391-397 (decision rule); Sley2.0mastergoal.md:1384-1405 (sixteen primitives, wire ops generated from schema); SLEY_2X_REWEAVE_MASTER_SPEC_V1.md:257 (typed affordances and the full candidate pipeline remain final requirements, no composite class named).
CURRENT_BEHAVIOR: Sixteen frozen classes, 179 descriptors, one candidate record; every measured edit expands deterministically to a short primitive list; no composite exists and none is needed to express any of the eight classes.
DESIRED_INVARIANT: The candidate record and its sixteen classes stay the only mutation authority; any composite is an expansion into them.
DISPOSITION: E_DEFER_2_1_PLUS
RATIONALE: No measured repair or failure data exists (rule: do not estimate), and the derived costs are bounded and linear; the residual costs (renumbering, replace-only fields, mandatory deletes) are expressible client-side without protocol change. Introducing a wire-level composite now would add a second place where semantics are defined.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none (a self-hosted writer library may expand composites into primitives; it must not become a checker)
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none now; for 2.1+: expansion determinism tests (composite -> primitive bytes byte-identical across runs) and an oracle reproduction
SPECS_TO_UPDATE: none now; 2.1+ recommendation recorded in this audit
CODE_OWNERSHIP: S20-350 (sley-mutate) for any future expansion library; S20-420 JSON bridge if bridge-side
REVIEW_REQUIRED: Ariadne contract review at 2.1 design time
```

```text
FINDING_ID: AT-MW-02
TITLE: No bounded typed entity-body read exists in SMP1, so most writes require context the machine protocol cannot supply
HYPOTHESIS: A bounded-context writer can obtain every fact the frozen primitives require through the machine protocol.
REPOSITORY_EVIDENCE: docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:94-135 (nineteen classes) and :316-330 (payloads: class 2 is kind, ObjectId, fingerprint; others are identity lists, edges, rows); docs/spec/SMP1.md:586 (handle.expand facts) and :594 (checkout returns every object of a revision); docs/spec/SEMANTIC_COMPARISON_V1.md:300 (field_delta carries identity sets only); bench/sley2/runner.py:95-112 (arm allowlist) and :121,:123 (checkout, exchange.export denied); measurement: 11 of 13 encoded candidates in section 2 need a current body value (operands, terminator, TypeDefForm, BlockBody, list order) because MUTATION_SCHEMA_V1.md:73-75 makes non-scalar non-list fields replace-only; crates/sley-query/README.md:22.
SPEC_EVIDENCE: Sley2.0mastergoal.md:1107-1131 (SMP1 MUST expose GetEntityVersion, GetSignature, GetTypeClosure, GetDataDependencies among others), :1133-1150 (capsule MUST contain relevant type and effect facts), :2638-2649 (small-model task with capsule and affordances only, no entire-store dump); docs/spec/CONTEXT_CAPSULE_PROFILE_V1.md:253 (excludes type, effect, contract, test facts); SLEY-2.0-ARCHITECTURE-TIGHTENING.md:12-34 (machine-generation efficiency, independent reproducibility are in scope).
CURRENT_BEHAVIOR: A writer under the arm allowlist can learn identities, kinds, ObjectIds, edges, and closures but not one operand, terminator, type form, or list order; the only body read is a whole-revision checkout. Benchmark classes that edit existing bodies (local bug repair S2B-REPAIR-001, signature migration, type-model change) cannot be attempted by the arm as specified.
DESIRED_INVARIANT: Every fact a frozen primitive requires as input (current typed field value, ordered child list, ObjectId) is obtainable for one named entity through one bounded, root-bound, session-safe SMP1 request, without a store dump.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Additive: a new query class (or new method under a new protocol_version, SMP1.md section 4 rule) returning the exact stored entity object bytes or typed body for one entity changes no canonical identity, preimage, epoch, or existing class; the object bytes already exist and are digest-bound. Material: without it the master-goal 20.10 demonstration and the succession benchmark's edit classes are unwritable in bounded context, and this is exact from the payload tables, not speculative. Not already solved: no class, method, or capsule field carries a body.
CANONICAL_IMPACT: none (derived read of existing canonical objects; the response must copy the stored object bytes so ObjectId verification stays with the reader)
SCHEMA_IMPACT: ROOT_BACKED_QUERY_PROFILE v1 class table is frozen at nineteen; add under profile_version 2 or a new SMP1 method tag with a protocol_version bump, never by widening v1
ABI_IMPACT: none
SELFHOST_IMPACT: a self-hosted writer needs the same read; without it the Sley-owned toolchain (REWEAVE section 9, typed operations over canonical graph state) cannot author edits from protocol context
MIGRATION_REQUIRED: no (additive version)
TESTS_REQUIRED: fixed vectors for the new class over the S20-310 fixture; bounded-context limits and PROTOCOL_LIMIT_EXCEEDED on oversize bodies; a trial-runner scripted smoke that performs one EC1a edit end to end (query body, create, validate) under ARM_AFFORDANCES; independent Python reproduction
SPECS_TO_UPDATE: ROOT_BACKED_QUERY_PROFILE_V1.md (v2 or successor), SMP1.md section 4 and appendix A, CONTEXT_CAPSULE_PROFILE_V1.md (if the capsule carries the body), SLEY2_TRIAL_RUNNER_V1.md ARM_AFFORDANCES, REQUIRED_CONTRACT_INDEX_V1.md
CODE_OWNERSHIP: S20-310 (sley-query) and S20-400/S20-410 (sley-protocol); S20-620 runner allowlist
REVIEW_REQUIRED: Ariadne contract review, Nabu architecture review (bounded-context and dump-avoidance), Vulcan surface review
```

```text
FINDING_ID: AT-MW-03
TITLE: Master-goal mutation affordance templates are absent and the GA report maps the criterion to the protocol instead
HYPOTHESIS: The capsule or a query already exposes per-entity operation templates (class, kind, handle, required fields, permitted value types, expected preimage, likely impact, required capability classes).
REPOSITORY_EVIDENCE: docs/spec/CONTEXT_CAPSULE_PROFILE_V1.md:136 and :253 (mutation affordances excluded); crates/sley-query/README.md:22 ("mutation affordances are not implemented"); evidence/release/ga-acceptance-report.json:149 and scripts/build_ga_acceptance_report.py:131 (criterion "typed affordances and mutations are available" evidenced by protocol status S20_400/S20_410); the static descriptor table crates/sley-mutate/src/generated.rs:682; per-principal class grants docs/spec/POLICY_ROOT_V1.md:40-46 with no query class exposing them (ROOT_BACKED_QUERY_PROFILE_V1.md:94-135).
SPEC_EVIDENCE: Sley2.0mastergoal.md:1407-1423 (SHOULD expose templates; "Affordances are hints"), :1146 (capsule MUST contain available mutation affordances), :1107-1131 (GetAvailableMutations MUST); SLEY_2X_REWEAVE_MASTER_SPEC_V1.md:257 ("typed affordances" remain final requirements).
CURRENT_BEHAVIOR: A writer derives eligibility offline from the 179 descriptors and binds preimages via class 2; it cannot learn which mutation classes its principal is granted (POLICY_GRANT_DENIED is discovered only by validating) nor receive impact/preimage-bundled templates.
DESIRED_INVARIANT: Affordances, when provided, are derived hints that never carry authority (master goal 10.3), and the GA criterion is evidenced by the surface the master goal names.
DISPOSITION: E_DEFER_2_1_PLUS
RATIONALE: Not a higher-order operation and not required to express any edit: class/kind/field eligibility is static and digest-pinned, the preimage is obtainable, and impact closure is class 14. The remaining value (granted classes, bundled templates) is convenience with no measured cost data. The master MUST in 8.3 versus the capsule exclusion is a specification synchronization conflict for AT-G5 to resolve by amendment or a capsule successor version, not by changing canonical state.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none now
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none now
SPECS_TO_UPDATE: for AT-G5: reconcile Sley2.0mastergoal.md 8.2/8.3/10.3 with CONTEXT_CAPSULE_PROFILE_V1.md section 11; scripts/build_ga_acceptance_report.py:131 should cite the affordance surface once one exists and state its absence until then
CODE_OWNERSHIP: S20-320 (capsule), S20-310 (query), release report script
REVIEW_REQUIRED: Maat doctrine check on the GA report wording; Ariadne at 2.1 design time
```

```text
FINDING_ID: AT-MW-04
TITLE: Validator fail-closed remainders, not the mutation substrate, make dependency-boundary and effectful edit classes unwritable at this commit
HYPOTHESIS: The primitive set cannot express dependency-boundary changes or effect/capability repairs.
REPOSITORY_EVIDENCE: EC7a/EC7b encode in 1 and 2 primitives (section 2); refusal crates/sley-policy/src/candidate_validation.rs:1328-1337 (any dependency_roots difference is INVALID_GRAPH CANDIDATE_DEPENDENCY_ROOT_CHANGE_UNSUPPORTED, Permanent) and test :2581-2588; excluded opcodes crates/sley-policy/src/candidate_program.rs:218 ([144,145,160,161,162]) with the phase 12 guard candidate_validation.rs:1550; docs/spec/CANDIDATE_RESULT_V1.md:359 and :450-456; docs/spec/TRANSACTION_MODEL_V1.md:16 (commit accepts the same profile); machine-summary.json /s20_360_candidate_validation/unsupported_operation_result and /evidence_gaps[1] (E7 semantics owned by S20-280/S20-380); benchmark tasks bench/corpus/v1/tasks.json S2B-EFFECT-001 (FileRead via effect_request) and S2B-CAP-001 (capability narrowing).
SPEC_EVIDENCE: Sley2.0mastergoal.md:1384-1403 (update dependency binding is a GA primitive), :2688-2706 (effect repair and capability-scope repair are required task classes); SLEY-2.0-ARCHITECTURE-TIGHTENING.md:24-34 (ALREADY_SOLVED when the substrate resolves the concern).
CURRENT_BEHAVIOR: The candidate encodes and imports; validation refuses at phase 5 (dependency roots) or phase 12 (E7 opcodes) deterministically, so these classes cannot reach VALID or commit.
DESIRED_INVARIANT: The mutation substrate expresses every GA edit class in bounded primitives; unsupported judgment fails closed until its owner lands, and never silently passes.
DISPOSITION: A_ALREADY_SOLVED
RATIONALE: For the machine-write question the substrate already resolves the concern (1 to 2 primitives, 572 to 746 B) and the fail-closed behaviour is the required invariant. The remaining work is implementation of owned phases (S20-360 dependency-root analysis; S20-280/S20-380 E7 semantics), already tracked as evidence gaps, not an architecture defect and not a mutation-model change. Recorded here so the benchmark's writable task space is stated truthfully.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none for the substrate; the self-hosted toolchain cannot use effect_request/adapter_invoke/capability_narrow until the owners land
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none new; existing refusal tests stand
SPECS_TO_UPDATE: none; the benchmark plan should state that two required task classes are validator-blocked at this revision (S20-610/S20-620 owners)
CODE_OWNERSHIP: S20-360, S20-280, S20-380
REVIEW_REQUIRED: none for this disposition
```

```text
FINDING_ID: AT-MW-05
TITLE: VALIDATION_PROFILE_V1.md still states the success subset contains no Operation entities
HYPOTHESIS: The validation profile document describes what a writer can get validated today.
REPOSITORY_EVIDENCE: docs/spec/VALIDATION_PROFILE_V1.md:46-49 ("Its current supported success subset contains no SSMC1 Operation entities. Operation-bearing projected programs fail closed during supported resource analysis"); contradicted by docs/spec/CANDIDATE_RESULT_V1.md:419-456 (phase 7 judges every operation of every function unit; only five E7 opcodes excluded), docs/spec/TRANSACTION_MODEL_V1.md:16 and :165 (commit accepts programs carrying semantic operation entities under full operation analysis), docs/audits/S20_360_CANDIDATE_VALIDATION_CLOSEOUT.md:118-150 (addendum 2026-09-03), and the passing operation-bearing fixtures crates/sley-policy/src/candidate_validation.rs:3280-3420.
SPEC_EVIDENCE: SLEY-2.0-ARCHITECTURE-TIGHTENING.md:86-101 (no silent reinterpretation), section 14 canonical specification synchronization.
CURRENT_BEHAVIOR: A writer reading the profile contract concludes that any function-body edit (EC1, EC2, EC4, EC5, EC6b, EC8) cannot validate, which is false at this commit.
DESIRED_INVARIANT: Every subordinate contract states the same supported subset as the owning result contract.
DISPOSITION: B_ADDITIVE_NOW
RATIONALE: Documentation-only correction with exact evidence; the profile record bytes and ValidationProfileId (7d8ffff9...) are untouched, so no canonical or identity impact. Belongs to AT-G5 but is recorded here because it misinforms writers directly.
CANONICAL_IMPACT: none (profile identity unchanged; verify with the profile vector after edit)
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: the stage checker that pins VALIDATION_PROFILE_V1.md markers (scripts/check_candidate_contract_freeze.py) must still pass; add a cross-document sentence check if one exists for the result contract
SPECS_TO_UPDATE: docs/spec/VALIDATION_PROFILE_V1.md:46-49 to reference CANDIDATE_RESULT_V1.md section 9's exact subset
CODE_OWNERSHIP: S20-345/S20-360 contract owner
REVIEW_REQUIRED: Ariadne contract review (documentation delta)
```

```text
FINDING_ID: AT-MW-06
TITLE: Mid-list insertion costs a renumbering primitive per later sibling
HYPOTHESIS: Inserting an operation or parameter at position i is one primitive.
REPOSITORY_EVIDENCE: crates/sley-check/src/cfg.rs:723-734 (GRAPH_ORDINAL_MISMATCH when Operation.ordinal != list index), crates/sley-check/src/lib.rs:415 (parameter ordinal == index); docs/spec/SSMC1_EPOCH1_SCHEMA.txt (OperationBody field 2 ordinal, ParameterBody field 3 ordinal are body fields); ENCODED EC1c (2 of 5 primitives are renumbers) and EC4b (6 of 18); each renumber needs the sibling's ObjectId (PRECONDITION_PAYLOAD_V1.md, ExactEntityVersion).
SPEC_EVIDENCE: docs/spec/CFG_VALIDATION_V1.md:41-42 ("Every ordinal is the zero-based position in the owner's ordered list"); docs/spec/SSMC1.md:131 ("Owner and ordinal facts must agree with the owner's list"); SLEY-2.0-ARCHITECTURE-TIGHTENING.md:86-101 (an epoch change is needed to redefine a body field).
CURRENT_BEHAVIOR: Position is stored twice (list index and body ordinal) and both must agree; a writer inserting mid-block emits (n - i) SetScalarField operations with (n - i) preimages.
DESIRED_INVARIANT: Canonical redundancy that the checker verifies stays; writers pay a linear, deterministic, client-expandable cost.
DISPOSITION: E_DEFER_2_1_PLUS
RATIONALE: Removing the ordinal field or making it derived changes SSMC1 epoch 1 (a new epoch, not a pre-freeze repair) for a cost that is bounded and expressible as a client-side expansion (insert-and-renumber); no trial data shows it drives repair churn. Appending at the end (EC3) is free. Record as a 2.1+ composite-expansion candidate.
CANONICAL_IMPACT: none now; an epoch-2 schema decision if ever pursued
SCHEMA_IMPACT: none now
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none now
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: S20-200/S20-220 (schema and CFG) if revisited; S20-350 for a client expansion
REVIEW_REQUIRED: none now
```

```text
FINDING_ID: AT-MW-07
TITLE: Session handles should not be admitted into the canonical candidate record
HYPOTHESIS: Handle-addressed mutations would cut the 32-byte identity cost per operation and precondition (master goal 8.5 "reduce context and mutation token cost").
REPOSITORY_EVIDENCE: docs/spec/CANDIDATE_RECORD_V1.md:51-70 and PRECONDITION_PAYLOAD_V1.md tables (EntityId/ObjectId only, all thirteen fields enter CandidateId); ENCODED: a delete is 55 B op + 80 B precondition, 96 B of it identities; JSON bridge doubles (docs/spec/SMP1_JSON_BRIDGE_V1.md:31-35); docs/spec/SESSION_HANDLE_PROFILE_V1.md and SMP1.md:586 (handles are session- and root-bound, fail closed after rebinding).
SPEC_EVIDENCE: Sley2.0mastergoal.md:1178-1195 (handles are derived, never persisted as canonical identity, fail closed after the binding changes); SLEY-2.0-ARCHITECTURE-TIGHTENING.md:70-84 (stable logical identity distinct from session state; candidate must not depend on ambient state).
CURRENT_BEHAVIOR: Candidates name raw identities; handles reduce read cost only.
DESIRED_INVARIANT: CandidateId and every preimage bind stable identities; a candidate is meaningful outside the session that built it.
DISPOSITION: D_REJECT
RATIONALE: A handle inside the canonical record would make candidate identity depend on session state, creating a second addressing authority and breaking replay/attempt digests. If byte cost ever matters, the JSON bridge or a client library may expand handles to identities before canonical encoding (2.1+ convenience), which changes nothing here.
CANONICAL_IMPACT: none (rejection preserves the frozen record)
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: n/a
REVIEW_REQUIRED: none
```

```text
FINDING_ID: AT-MW-08
TITLE: Creation identities must be derived client-side with BLAKE3 before candidate.create
HYPOTHESIS: The endpoint derives or returns creation identities for the writer.
REPOSITORY_EVIDENCE: crates/sley-mutate/src/candidate.rs:457-520 (construction rejects any class-1 target that is not EntityId::derive(workspace, nonce, kind, creation_ordinal), error MUTATION_CANDIDATE_TARGET_ENTITY at :75); crates/sley-protocol/src/server.rs:1277-1282 (candidate.create decodes and builds; no derivation); docs/spec/SMP1.md:250-315 (no identity-derivation method); oracle derive_entity_id (oracle/scb1/src/sley2_scb1_oracle/candidate.py:124-135) as the independent reference; ENCODED: EC2 needs 5 derivations, EC6b 8, all also referenced inside sibling bodies (block lists, operand refs) before any of them exists.
SPEC_EVIDENCE: docs/spec/IDENTIFIERS_V1.md:111-124; docs/spec/CANDIDATE_RECORD_V1.md:71-75.
CURRENT_BEHAVIOR: A writer needs BLAKE3, the WorkspaceId, its own nonce, and the creation ordinal discipline; a writer without BLAKE3 cannot create entities.
DESIRED_INVARIANT: Deterministic, collision-checked creation identity bound to the candidate nonce (already true); tooling may compute it for the writer without changing the rule.
DISPOSITION: E_DEFER_2_1_PLUS
RATIONALE: The rule is correct and frozen; the friction is a client-library concern (the JSON bridge or a writer SDK can derive), with no measured cost data. A server-side "derive" helper would be additive but is not required to write any edit.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: a self-hosted writer needs a BLAKE3 implementation in Sley or the native substrate (REWEAVE section 10.3 native remainder)
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none now
SPECS_TO_UPDATE: none
CODE_OWNERSHIP: S20-420 bridge or a writer SDK, 2.1+
REVIEW_REQUIRED: none now
```

```text
FINDING_ID: AT-MW-09
TITLE: candidate.append re-sends the full header and returns the full stored candidate on every step
HYPOTHESIS: Incremental candidate construction over SMP1 is cheaper than one-shot construction.
REPOSITORY_EVIDENCE: crates/sley-protocol/src/server.rs:1284-1295 (append imports the stored candidate, decodes a second complete record, concatenates, rebuilds, returns the whole stored bytes); docs/spec/SMP1.md:588 and :591 ("the server holds no candidate state"); ENCODED: fixed header 332 B per record, stored candidate grows monotonically (EC8 3862 B), so k appends move about k x 332 B extra up and the cumulative stored size down.
SPEC_EVIDENCE: Sley2.0mastergoal.md:1364-1382 (candidate binds all context fields); SMP1.md authority rule (transport owns no semantics, holds no state).
CURRENT_BEHAVIOR: Stateless append; one-shot create is strictly cheaper in bytes and round trips for every measured edit.
DESIRED_INVARIANT: The endpoint stays stateless with respect to candidates; writers build once.
DISPOSITION: D_REJECT
RATIONALE: A server-side candidate buffer would add mutable session state to a deliberately stateless surface for an unmeasured benefit; every representative edit fits in one create. Writers should compose locally and create once.
CANONICAL_IMPACT: none
SCHEMA_IMPACT: none
ABI_IMPACT: none
SELFHOST_IMPACT: none
MIGRATION_REQUIRED: no
TESTS_REQUIRED: none
SPECS_TO_UPDATE: none (optionally a writer guidance note in SMP1 appendix A)
CODE_OWNERSHIP: n/a
REVIEW_REQUIRED: none
```

