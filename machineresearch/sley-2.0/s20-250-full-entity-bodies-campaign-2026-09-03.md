# Full S20-250 entity bodies: campaign record (opened 2026-09-03)

Status: contract draft revision 1 committed; Council review pending.

## Why now

After S20-540 closed, the local completion frontier named the full S20-250
entity bodies as the next dependency-complete work (S20-510 and S20-520 are
blocked by them). The operator directed the work to continue to the end of
the Sley 2 goal without further hand-backs.

## Council availability at the draft (evidence)

Three dispatch attempts were made before the draft, all through
`forge agent --bounded --thinking high --timeout 3000`:

| Time (UTC) | Model | Result |
|---|---|---|
| 10:50:06 | `claude-cli/claude-opus-5` | `HTTP 401 ... OAuth access token has been revoked.` (exit 1 after 54 s) |
| 10:52:55 | `openai/gpt-5.6-sol` | `API rate limit reached. Please try again later.` (exit 1 after 52 s) |
| probe | `anthropic/claude-opus-5` | `Requested agent harness "claude-cli" is not registered.` |

The gateway's `claude-cli:local` provider runs in token mode against the
Claude CLI credentials (`~/.claude/.credentials.json`, written 05:13Z); the
gateway started 06:52 local, after that file, so a restart would not repair
a revoked token. Repair needs an operator re-login of the Claude CLI lane or
a cleared OpenAI usage limit. Per the Council doctrine the front-line
integrator proceeded with the design from the frozen constraints and records
the review as pending; every review lands as a contract revision before the
freeze, exactly as S20-540 did.

## Design decisions taken by the integrator (pending review)

1. Six definitions in `sley-ssmc` (single normative model); `EntryExposure`
   moves to `sley-ssmc` with a `sley-mutate` re-export so the generated
   codec is byte-identical.
2. No fingerprints for the six kinds (SSMC1 section 8 requires field 4 only
   on GA-valid TypeDef and Function). The design brief's proposal of six new
   projections is withdrawn as contradicting that sentence.
3. Edge vocabulary mirrors the frozen S20-360 private reference graph in
   `crates/sley-policy/src/candidate_program.rs` (Ownership 1, Capability 7,
   Contract 8, TestTarget 10; `external_package` and `dependency_root` make
   no edge). An edge-agreement test is required evidence.
4. Complete-root request from one StateRoot record plus its objects; the
   pure judgment (eleven rules C1 to C11, codes 25013 to 25024) lives in
   `sley-query` and takes borrowed definitions plus the record's
   `entry_points` and `dependency_roots`; the extraction adapter lives with
   object loading and projection. The frozen S20-360 judgments
   `STATE_ROOT_ENTRY_POINT_KIND_INVALID` and the validator's
   `dependency_roots()` derivation fix the readings of C9 and C10.
5. Restricted S20-300/S20-310/S20-320 consumers stay bound to kinds 4 to 15
   and fail closed with `INDEX_SNAPSHOT_COMPLETENESS_UNSUPPORTED`.
6. New document plus ADR-0026 rather than an in-place edit of the restricted
   profile, whose checker binds its status line.

## Open questions for the reviews

- Whether Ownership is the right frozen kind for EntryPoint exposure and
  PolicyBinding subject binding (mirror rule) or a narrower reading is
  required.
- Whether C5's distinctness of the workspace root namespace from package
  root namespaces is too strict for the intended workspace model.
- Whether C7's forbidden member kinds are exactly right (Constant is
  allowed; DependencyBinding is not).
- Extraction adapter ownership: `sley-repo` (owns the store) calling a public
  projection in `sley-policy`, or a public projection consumed elsewhere.

## Commits

- `4701733` contract draft revision 1, ADR-0026, stage checker, this record.
- `e78a1ab` implementation: six definitions, eighteen-kind impact,
  complete-root judgment, sley-repo adapter, fixture, oracle, fuzz slice.
- closeout docs, revision 2 status, Tier 2 record: the commit after `e78a1ab`.

Tier 2 at `e78a1ab` (2026-09-03): core 914 tests, conformance, adversarial,
fuzz-smoke, complete-root smoke, all exit 0 in 30 s. Reviews still pending;
the retry loop probes both lanes every 15 minutes.
