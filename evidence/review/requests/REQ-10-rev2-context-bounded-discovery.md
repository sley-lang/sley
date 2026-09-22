# Review request REQ-10 rev2 — CONTEXT bounded discovery contract delta

- Supersedes: `REQ-10-context-bounded-discovery.md`
  (sha256 `9f953627...c917`, verdict `REVISE_0_P0_2_P1_1_P2_0_P3_1_P4`,
  transcript
  `evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-b0e52e00.md`).
- Baseline tree: `b0e52e00a2bacfd7a874f0b4a7973a352508cebe` on
  `work/succession-sley20-arm`. Lane: Nabu (architecture).
  Vulcan budget/continuation-abuse and Ariadne profile-text lanes remain
  deferred, as in rev1. F1/Bool and private impact-role lists stay retired.
- Correction owned: rev1's "no served impact-enumeration route" premise was
  wrong. The trace checked for a method NAME containing "impact" and missed
  the class-tag mechanism: `server.rs:3465` decodes class 14
  `RootQuery::ReverseImpactClosure{seeds}` on the served `query.root` route
  (via `query_root` → `root_query.rs:104` → `sley-query` reverse reachability,
  bounded with `Cursor::Entity` paging and truncated/next_after accounting),
  alongside sibling discovery classes 13 (`ListDirectDependents`) and 15-19
  (`ForwardDependencyClosure`, `ListContractsFor`, `ListTestsFor`,
  `ListDeclaredEffects`). Rev1 delta item 2 (a second served method) is
  WITHDRAWN in full. No new served method is proposed in rev2.
- Nabu's executed evidence reproduces independently: `cargo test -p sley-repo
  --locked --offline --test s3_g2_context` → 1 passed
  (`s3_context_fixture_conformance`), 2 ignored, in ~3s at this baseline.
  Recorded process note: the rev1 packet constrained the review to read-only;
  the verdict reports reviewer-side test execution. The finding stands on its
  source anchors regardless; the deviation is noted, not litigated.

## Respecified delta (bootstrap, not a new method)

- Verified gap (narrowed per the verdict, confirmed against code): the class-14
  route IS served, but the trial interface cannot form a FIRST valid call.
  Every root-backed request preimage binds `snapshot_id` first
  (`sley-query/src/root_query.rs:740-755`); the server ignores the caller's
  value on decode (`server.rs:3408`) and refuses `QUERY_SNAPSHOT_MISMATCH`
  (31_003) whenever the rebuilt preimage differs (`server.rs:2697-2699`,
  capsule `:2768-2770`, restricted `:2827-2831`). No allowed response
  discloses an index snapshot identity first: `revision_summary`
  (`server.rs:3244-3259`) carries tx/root/policy/workspace/epoch/counts/receipt
  only, and the capsule record (which does carry it, `capsule.rs:437`) is
  itself gated behind the same preimage equality. Trial Python builds no
  preimages today (zero `snapshot_id`/`preimage` references across
  `bench/live/*.py` and the judge), so class 13-19 are served-but-unusable
  from the trial interface — "reachable" in method-tag terms only.
- Rev2 delta (additive tooling + authorized-input binding, no protocol change):
  1. Disclose the starting binding on an already-allowed route: extend
     `revision.read` (field 9 carrying the verified index snapshot identity)
     or return it in the `session.open` response — enumerated options, reviewer
     to select; no new method either way. Until selected, no implementation.
  2. Task-input amendment (authorized inputs only): name the target record
     typedef and required-field semantics (task intent supplied, not
     discovered; manifest `entities`/`targets` remain judge-side).
  3. Trial tooling: a preimage encoder + `raw query.root` class-14 discovery
     through the existing route with bounded pages/continuations and cumulative
     budget accounting; trial allowlist name set unchanged (the method tags are
     already afforded).
- Ownership (P2): no third reverse-impact semantics. Before implementation,
  record which of `ImpactGraph::transitive_impact`
  (`sley-query/src/lib.rs:515-547`) and `reverse_closure`
  (`sley-query/src/query.rs:680-716`) is authoritative for served class 14,
  and converge or document the divergence with symbols enumerated. The
  class-14 route already executes one of them; rev2 adds no path.
- Acceptance criteria (unchanged shape, reachable scope): `execute_attempt` →
  discovery from the disclosed binding (actual continuation where needed) →
  agent mutation → production admission → real oracle → append → verify, with
  complete impact repair authored through the interface; rejection for
  incomplete discovery, inconsistent continuation, exceeded budgets, missing
  evidence; no private IDs as witness arguments (witness manifest reads at
  `succ_witness_context.py:65-67` replaced by interface discovery), no hidden
  manifest, no inventory dump.

- Constraints: read-only review. No file writes. Verdict in reply text only,
  in the REQ verdict format, against section `context_bounded_discovery`,
  field `nabu_architecture_review`.
