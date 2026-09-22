Verification complete against source at the rev8 baseline tree `95d42fda`. No files written, no tests run, no checker executed — per packet constraint.

```
VERDICT: REVISE_0_P0_0_P1_2_P2_2_P3_1_P4
SECTION: context_bounded_discovery
FIELD: nabu_architecture_review
SCOPE_SHA: 95d42fda691bc5d8ec468478529b7b174d3a8759
PACKET_SHA256: UNAVAILABLE_IN_THIS_SESSION (see assumption 2; not fabricated)
SUPERSEDES: 24d2b5056f14afe619737306a592c47071384ce11540439edddf47574f0bfe08 (REVISE_0_P0_1_P1_2_P2_1_P3_0_P4, asserted; see P3-1)
FINDINGS:

[P2] [architecture] The B4 STRIKE decision rests on a false cost premise,
and the strike leaves the invariant this packet newly makes load-bearing
unenforced. The stated reason — "a pin test would invent a new cross-package
test dependency" — is falsified by the import graph. `bench/live/sley2_tool.py:51`
is a module-level `from bench.sley2.runner import PROFILE_ARGS, Endpoint,
endpoint_offer, probe_handshake, request_frame`. Every live test that touches
the tool already pulls `bench.sley2.runner` transitively and at import time:
`bench/live/tests/test_sley2_tool.py:17`, `test_agent_access.py:19`,
`test_acceptance_repairs.py:17` all carry top-level `from bench.live import
... sley2_tool`. The cross-package dependency is not invented by a pin test;
it is already load-bearing and unavoidable. The rest of the rationale does
verify: no live test imports `bench.sley2` by name (checked every file under
`bench/live/tests/` by AST, not grep), and the checker genuinely never reads
`bench/live` — `scripts/check_sley2_trial_runner.py` contains no reference to
`sley2_tool` or `bench/live` at all, so the runner-side count (`:257`),
spec-order (`:252-256`) and digest gates pin one side only. That is the
problem, not the defence. After edit 4.0 both tuples read nineteen names and
nothing checks they are the same nineteen in the same order — while Fix 2's
own text makes tuple-order identity the requirement ("the same tuple order as
`ARM_AFFORDANCES` after the move"), and rev4/rev5 established divergence of
these two surfaces as the defect class this whole REQ chain exists to close.
Replacing a false claim with true prose is correct and I uphold that half.
Disposition, decided in-lane on corrected facts (the packet invites the
reviewer to judge): the pin test is now required, because its true cost is
one `assertEqual(sley2_tool.TOOL_METHODS, runner.ARM_AFFORDANCES)` in a file
that already imports both sides — not a new dependency. Keep the honest
prose; add the assertion it makes true. If the writing lane still prefers the
bare strike, the record must say the identity is maintained by review only
and name that as an accepted unenforced invariant, not present it as a cost
avoidance.

[P2] [architecture] Fix 2's enumeration claims completion of stated intent
and stops one file short — the same outward step rev7 caught at `:322`.
Adding an `open` command to `sley2_tool.dispatch` puts it outside the
mediated gateway's frozen command surface. `bench/live/mediated_sley.py:48-51`
defines `ALLOWED_COMMANDS` as fourteen names — `inventory, side, read, sig,
revision, caps, budgets, raw, inspect, validate, append, compose, propose,
finish` — under the comment "The frozen documented surface (same commands as
sley2_tool.dispatch)", and `:173-177` returns `GATEWAY_COMMAND_DENIED` for
anything outside `GATEWAY_COMMANDS`. `open` is not there (verified: the
dispatcher's twelve `command == "..."` sites plus the `inspect`/`validate`
pair, and no existing `open` command — `:225`'s `_fail_if(opened, "open")` is
a harness label, not a command). So after Fix 2 the direct agent route has
`open` and the mediated route denies it, and the file's own "same commands as
sley2_tool.dispatch" comment goes false in the same commit. This is not fully
dead — a mediated agent can still reach the method as `raw workspace.open
<body>`, since `raw` is allowed and `:723`/`:220` gate on `TOOL_METHODS`,
which edit 4.0 widens — so I rate it P2 rather than P1. But the mediated lane
is the captured, measured route, and a discovery affordance reachable only by
falling back to the raw escape hatch on that lane is exactly the surface
divergence Fix 2 is meant to end. Remedy: edit 4.1 — add `open` to
`ALLOWED_COMMANDS` in the same commit, or state explicitly and with reasons
that the mediated surface deliberately excludes the dedicated command and
correct the `:47` comment. Either is acceptable; silence is not.

[P3] [record] The packet applies the Fix 3 standard to rev4 and not to
itself. Fix 3 is right, and I verified its ground truth independently: no
`evidence/review/verdicts/sley2_trial_runner/` directory exists at
`95d42fda` (I enumerated all 51 verdict subdirectories; it is absent), and
`machine-summary.json.sley2_trial_runner` carries `status: S20_620_COMPLETE`,
`contract_revision: 4`, and all three lanes at
`PASS_0_P0_0_P1_0_P2_0_P3` with no transcript behind them. Refusing to mint
`_revision_4` fields pointing at a nonexistent path is the correct call. But
the same test applied to this packet's own chain: neither
`nabu_architecture_review-rev7-95d42fda.md` nor any rev6/rev7 verdict or
request artifact exists anywhere in the repository. I scanned every commit
reachable from every ref; the tree holds rev1–rev5 verdicts and requests
REQ-10 through rev5 only, and neither claimed sha256
(`24d2b505…f08` packet, `590df37e…243` transcript) appears in any tracked
object. This is consistent with the read-only constraint — the reviewer never
writes and the writing lane evidently has not committed rev6/rev7 — but it
means rev8's `SUPERSEDES` line will name a transcript path absent from the
baseline, which is the precise fault Fix 3 corrects for rev4. Remedy: the
writing lane commits the rev6 and rev7 transcripts in or before this round,
or the rev8 record marks them explicitly as not-yet-committed rather than
citing them as though resolvable. Note this cuts the other way too — the
packet's own precedent shows the intended convention works: sha256 of
`evidence/review/requests/REQ-10-rev5-context-bounded-discovery.md` is
`ace99e91…fbc3`, matching `PACKET_SHA256` in the rev5 transcript exactly, so
the chain is verifiable when the artifacts land.

[P3] [record] Edit 4.0 silently rewrites an audit vocabulary and the packet
does not say so. `bench/live/mediated_sley.py:220-223` labels captured
exchanges `raw:<method>` when `args[0] in sley2_tool.TOOL_METHODS` and
`raw:denied` otherwise, under a comment at `:217-219` asserting
"Schema-stable: the capture record keeps the same fields; only the method
vocabulary gains the `raw:` prefix for raw frames." Admitting `workspace.open`
to `TOOL_METHODS` flips any such call from `raw:denied` to
`raw:workspace.open` in the capture stream the judge audits bounded-query
discipline from. The change is correct by design — the method is allowed now,
so labelling it denied would be the bug — but it is a vocabulary change in an
evidence stream, it is the second such change that comment has absorbed, and
a record-completion packet should name it. No gate moves; no PASS/FAIL
affected. Fold a sentence into the Fix 2 edit-site list.

[P4] [record] Transcript per the adopted convention:
`evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-rev8-95d42fda.md`,
superseding `nabu_architecture_review-rev7-95d42fda.md` (see P3-1 on that
file's absence). Not written by this session — read-only constraint; the
writing lane owns creation, and this verdict text is the artifact. Nothing
was executed this round: the A4 demonstration Fix 1 calls for remains owed to
the implementation gate record, and I did not stand in for it. Follow-on
lanes remain `vulcan_surface_review` and `ariadne_contract_review`, deferred
and not evaluated here.

SUMMARY: REVISE, and the architecture remains settled — both P2s are
enumeration completeness at the edit-site level, not design. Two of the four
adopted remedies are correct, complete, and verified against source with
nothing left owed.

Fix 1 is right, and I confirmed it mechanically rather than by reading. I
reconstructed both candidate insertion points as real files and parsed them:
at rev7's withdrawn ~`:155` target the inserted `section.get("contract_revision")`
loads `section` at line 158 while its first binding in `main` is line 174 —
a live NameError, so the withdrawal is correct and the P1 was real. At the
adopted `:178` target every name resolves: `spec` bound `:155`, `section`
bound `:174`, `problems` bound `:147`, `re` imported module-level at `:8` as
claimed. Line `:178` is exactly `status = section.get("status")` and `:201`
is exactly `if status in IMPLEMENTATION_STATUSES:`, so the block lands
between them at indent 4, outside the block, feeding `:286-289` and the
`:291` payload as argued. The regex anchors: spec line 3 reads "Status:
S20-620 contract draft, revision 4 (2026-09-09); Council review", which
`^Status: S20-620 contract draft, revision (\d+)` matches under `re.M`. The
precedent is followed in full now, ordering included —
`check_root_backed_query_profile.py` binds section `:209`, status `:213`,
extracts `:214-215`, compares `:216-217`, reads summary `:218`, asserts
`:226`. Fix 1 needs nothing further.

Fix 4 is right, and I executed the register's own regex against the three
candidate forms to be sure rather than reasoning about it. `NOTE_SCOPE =
re.compile(r"\bon ([0-9a-f]{40})\b")` at `build_finding_register.py:540`,
consumed at `:606` inside the `:604-607` derivation. The proposed template
`Historical <lane> verdict on <40-hex commit>; transcript <path>.` yields
`scope = "95d42fd"`. Both rbqp-quoted forms yield `None` — the short-sha
variant (`at c1d4177`) and the full 40-hex variant (`at e050fe75…`) alike,
confirming the `at` spelling fails regardless of sha length, which is the
sharper version of the packet's claim. The `:604` prefix guard
`_revision_[0-9]+$` admits `nabu_architecture_review_revision_5` and rejects
the bare base field, so the revision-bound fields Fix 3 introduces are
exactly the ones that reach the note path. Scope attribution only; no gate
moves. Correct as written.

On Fix 3 I want the ownership call recorded plainly, because it is mine and
it is not obvious. Binding at revision 5 rather than minting revision-4
fields is the right trade: it preserves the rev4 approvals as what they are —
valid historical evidence in byte-exact base values and git history — while
refusing to manufacture a template an implementer cannot satisfy. The
`:286-289` extension with `completion-unbound-review:{key}` alongside the
existing base-field check is the correct shape, and gating the base-field
update on the same commit that returns status to `COMPLETE` closes the
window rev7's P1 opened. I am ratifying a record that will permanently show
three lanes cleared at revision 4 with no transcript. That is the honest
state of this tree, and inventing paper to cover it would be the worse
outcome.

What both P2s share is the pattern this chain keeps reproducing, and it is
worth naming once more rather than per-finding: each round traces the new
affordance one file further out and stops at the edge of the file it is
editing. Rev4 stopped at the two allowlist gates and missed the completion
gate. Rev5 inherited that. Rev7 caught `TOOL_METHODS` at `:322` and stopped
inside `sley2_tool.py`. Rev8 adopts that fix and stops before
`mediated_sley.py`, which consumes `TOOL_METHODS` at `:220` and enumerates
dispatch commands at `:48-51`. The surface is not one list; it is four
coupled enumerations — `ARM_AFFORDANCES`, `TOOL_METHODS`, `ALLOWED_COMMANDS`,
and the checker's literals — and only the first is gated. Fix both P2s, and
a pin test makes two of the four couplings mechanical instead of editorial.
That is what stops a rev9 on this same axis.
```

Assumptions, stated explicitly:

1. **No execution against the repository.** Every finding is source-anchored at the `95d42fda` tree object by inspection of `git show` output. I did not run the checker, `make quick`, or any test, and I made no writes. The one thing I did execute was in my own scratch space, outside the repository: I reconstructed the two candidate Fix 1 insertions as standalone files and parsed them with `ast` to resolve name bindings, and I ran the `NOTE_SCOPE` regex against the three candidate note forms. Those establish properties of the proposed text, not of the repository, and touched nothing under the working tree.

2. **PACKET_SHA256 is genuinely unavailable, not omitted.** The convention in this chain hashes the committed request file — I confirmed this by recomputing rev5's, which matches its transcript exactly. The rev8 request is not committed anywhere in the repository, and the packet reached me as prose whose byte-exact on-disk form I cannot observe. Emitting a hash here would be fabrication of the same class Fix 3 correctly refuses. The writing lane should fill it from the committed artifact.

3. **Rev7 is taken on assertion.** Neither the rev7 packet nor its transcript exists in any reachable commit (full scan of all refs), so the quoted verdict `REVISE_0_P0_1_P1_2_P2_1_P3_0_P4`, the two sha256 values, and the claim that Fixes 3 and 5 were confirmed complete are unverifiable from this tree. I checked internal consistency instead: four named transcription faults against 1×P1 + 2×P2 + 1×P3 reconciles exactly, and rev7's characterisation of the rev5 verdict matches the rev5 transcript I *can* read. This is P3-1's substance, not a separate complaint.

4. **Carried-forward Fix 3 is not re-derived, and one dependency inside it is load-bearing.** Rev7's Fix 3 (inventory floor, token set, `status_note`) is carried verbatim and I did not restate it. I flag one thing the writing lane must confirm still holds, because it is a gate and not prose: `check_sley2_trial_runner.py:135` anchors `_spec_allowlist` on the literal string `"holds eighteen names"` and `:257` asserts `len(allowlist) != 18`. If Fix 3's token set is prose-only and does not reach those two lines plus the spec text at `:291` they parse, the checker fails closed with `spec-allowlist:missing` or `allowlist-count:19`. I assume rev7 verified this. If it did not, that is a P0 and not a P2 packet.

5. **Affordance authority unchanged.** As in rev4/rev5, I continue to treat `ARM_AFFORDANCES`/`ARM_DENIED_METHODS` as the binding arm surface. I verified the move's mechanical preconditions still hold — `workspace.open` is present in `conformance/smp1-json-bridge/v2/methods.json` (43 methods), currently sits in `ARM_DENIED_METHODS` at `runner.py:143`, and the disjointness and SMP1-coverage tests at `test_runner.py:550-567` are move-safe — but ratifying the widened allowlist remains the allowlist owner's call, flagged rather than asserted as mine.

6. **The P2-2 severity is my judgement.** No gate fails on the `ALLOWED_COMMANDS` omission; the mediated agent retains a working `raw` path. I rated it P2 rather than P1 for that reason and rather than P3 because the mediated lane is the captured, measured route and because the file's own comment becomes false. Reasonable reviewers could place it either side.
---
Writing-lane record (not reviewer text): the rev8 packet landed at
evidence/review/requests/REQ-10-rev8-context-bounded-discovery.md;
sha256sum
011535124cc30fd7d865eef718d6eda5fa5e15f96c928108d2d5b4c8cf8dcaa6.
Reviewer text above the delimiter is byte-verbatim from
resume-20260918/review-REQ10r8-nabu.log (dispatch exit 0, engine hermes).
Reviewer verdict: REVISE_0_P0_0_P1_2_P2_2_P3_1_P4.
