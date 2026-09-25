# Review request REQ-10 rev5 — CONTEXT bounded discovery record completion

- Supersedes rev4 (sha256 `f3b64457...2002`; verdict REVISE 0P1/3P2/2P3,
  architecture settled, record completion specified; transcript
  `evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-rev4-b0e52e00.md`).
  Rev4's remedy-(a) adoption, option-B withdrawal, omission-not-refusal shape,
  head-pinned opener placement, and P3 anchor fix stand as reviewed.
- Baseline tree: `c1e8db56b15b41c9c90aaa8ec927015b6bcbe204` on
  `work/succession-sley20-arm`. Sources identical to `b0e52e00` (rev4
  baseline): the sole delta is the records-only aggregate-review successor
  (`bench/live/GATE-RECORD-20260921-AGGREGATE-REVIEW.md`); no Rust sources,
  fuzz targets, or lint inputs changed. Lane: Nabu (architecture).
  Section `context_bounded_discovery`, field `nabu_architecture_review`.

## Recommended change (single record, three P2s + two P3s, no redesign)

P2-1 — allowlist/spec/checker revision bump (same commit, following the
exact precedent `docs/audits/PHASE3_V2_OFFER_DESIGN.md:76-83` which
admitted `entity.version`/`entity.signature` in tuple order and bumped
the contract to revision 4):
1. Move `workspace.open` from `ARM_DENIED_METHODS`
   (`bench/sley2/runner.py:143`) to `ARM_AFFORDANCES`, appended in tuple
   order after `session.capabilities` (alphabetical; 18 → 19 names). Move,
   not copy: preserves the disjointness invariant
   (`bench/sley2/tests/test_runner.py:552-555`) and the
   allowlist-plus-denylist-covers-SMP1 invariant (`:556-565`).
   `conformance/smp1-json-bridge/v2/methods.json:49` offers
   `workspace.open` and `Method::V2_ALL` includes it un-reserved
   (`crates/sley-protocol/src/lib.rs:517-524,733-744`), so the handshake
   missing-check (`runner.py:465-467`) passes.
2. Edit `docs/spec/SLEY2_TRIAL_RUNNER_V1.md` section 9 (`:291-297`) frozen
   order list to include `workspace.open` last; change `holds eighteen
   names` to `holds nineteen names`; update the revision preamble (`:5-12`)
   to revision 5 recording this admission. Update the nine load-bearing
   `eighteen-name` phrasings (`runner.py:148,446,931,936`;
   `bench/live/sley2_tool.py:8,84`; spec `:9,35` plus section 9) to
   nineteen in the same commit.
3. Update `scripts/check_sley2_trial_runner.py`: count gate `:257`
   (`len(allowlist) != 18` → `!= 19`) and the spec-order anchor `:135`
   (`holds eighteen names` → `holds nineteen names`). The claim digest
   rotates by construction (no digest literal pinned;
   `bench/sley2/tests/test_runner.py:579-587`); the count gate and the
   spec-order gate are the separate controls this item names.

P2-2 — encoder scoping (specification gap with silent failure mode, not a
wrong decision): `revision_summary`
(`crates/sley-protocol/src/server.rs:3244-3259`) is a free function over
`&VerifiedRevision` with no knowledge of headness, shared by exactly two
callers: `revision_read` (`:2499-2504`, arbitrary caller-supplied
revision) and `workspace_open` (`:2889-2892`, accepted head via
`self.head()`). Field 9 (accepted-head snapshot identity) is encoded on
the `workspace.open` path ONLY — via an explicit head-binding parameter
or a split encoder — and `revision_read` response bytes are left
byte-identical. An implementer writing field 9 inside the shared
`revision_summary` would leak it onto arbitrary-revision reads, reopening
the disclosure/discovery disagreement the omission-not-refusal shape
closed.

P2-3 — live-tool convergence (same commit): `bench/live/sley2_tool.py:261`
calls `workspace.open` as harness inside `_read_head` (`:256-267`),
before the agent runs; `workspace.open` is absent from `TOOL_METHODS`
(`:88-107`) and the dispatcher refuses outside methods (`:322`). The
agent-facing route is `revision` (`:717-718`), calling `revision.read`
with the harness-pre-supplied head tx — the harness-relay pattern rev3
withdrew as option B, live today. Remedy: extend `TOOL_METHODS` with
`workspace.open` (19 names, same tuple order as `ARM_AFFORDANCES`),
replace the harness relay with the afforded agent route, and either
author the pinning test (`TOOL_METHODS == ARM_AFFORDANCES` in tuple
order) or strike the claim at `sley2_tool.py:8,84` (`pinned equal to the
smoke runner's allowlist by test`; no such test exists — grep for
`TOOL_METHODS` across `bench/live/tests/` returns nothing).

P3-1 — probe ownership: the new `sley-repo` probe (`read_record` +
`accept_cached`, no `fresh_snapshot` fallthrough) is owned by Merlin
(`docs/WORK_PACKAGES.md:35` assigns S20-300, repository index cache in
`sley-repo`, to Merlin). Verified: `accept_cached` private
(`crates/sley-repo/src/index_cache.rs:108`, reads no object),
`read_record` private (`:242`), `fresh_snapshot` `pub(crate)` (`:131`),
sole public entry `complete_root_snapshot` (`:203`) falls through
unconditionally (`:221`), as does `verify_cached_snapshot` (`:288`).
With the probe, O(1)/`returned_entities = 1` holds truthfully
(`counted(summary, 1)` at `server.rs:2892`).

P3-2 — denial rationale rewrite (same commit): `runner.py:116-118`
justifies `ARM_DENIED_METHODS` as bulk-export/import plus repository/run
mutation. `workspace_open` mutates nothing: takes no body, reads
`self.head()`, encodes a summary, returns `counted(summary, 1)`
(`server.rs:2889-2892`). Rewrite the comment and the
`ArmAffordanceTests` docstring
(`bench/sley2/tests/test_runner.py:541-546`) which become false on the
move; the over-broad category (not risk) strengthens remedy (a).

## Acceptance (unchanged shape, now fully specified)

`workspace.open` (allowed, 19-name surface) → accepted-head snapshot
binding (field 9, head-pinned opener path only) → class-14 discovery
with actual continuation where needed → agent mutation → production
admission → real oracle → append → verify; rejection for incomplete
discovery, inconsistent continuation, exceeded budgets, missing evidence;
no private IDs as witness arguments, no hidden manifest, no inventory
dump. Follow-on lanes remain `vulcan_surface_review` and
`ariadne_contract_review`, deferred.

- Constraints: read-only review. No file writes, no test execution in the
  reviewer session. Verdict in reply text only, in the REQ verdict format,
  against section `context_bounded_discovery`, field
  `nabu_architecture_review`.
