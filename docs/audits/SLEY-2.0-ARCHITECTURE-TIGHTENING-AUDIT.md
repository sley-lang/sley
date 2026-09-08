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
