I've verified all five rev9 items against the source, and they are closed correctly. I'm still returning REVISE, because three things I hadn't raised before turned up. The most serious is that the command guide the live model reads was never on any edit list: it still shows the old `revision` usage and never mentions `open`.

```
VERDICT: REVISE_0_P0_1_P1_1_P2_3_P3_2_P4
SECTION: context_bounded_discovery
FIELD: nabu_architecture_review
SCOPE_SHA: 95d42fda691bc5d8ec468478529b7b174d3a8759 (read at HEAD 2c97c32f; delta verified records-only — 8 files, all evidence/gate-record, zero source)
PACKET_SHA256: dbfe12f3ac5429e1187eb315c81ac2d6539df51f000cc4d79de292eeca1a9aa5 (sha256 of the committed request file at 2c97c32f; see assumption 2)
SUPERSEDES: 011535124cc30fd7d865eef718d6eda5fa5e15f96c928108d2d5b4c8cf8dcaa6 (rev8, REVISE_0_P0_0_P1_2_P2_2_P3_1_P4)
FINDINGS:

[P1] [surface] The agent-facing tool contract is missing from every edit list,
so the live model's documented surface lags the implementation both ways.
`bench/live/tooling.py:67-145` (`SLEY2_TOOLING`) is the TOOLING.md staged to
the live model on both routes: direct (`tooling.py:186-189`) and mediated
production (`mediated_attempt.py:140-142`; `mediated_transport.py:4-6`: "the
live model drives the documented sley-tool surface directly"). Its command
block (`:84-98`) documents `revision` with no argument (`:87`) and has no
`open` line. Two changes adopted earlier leave that text wrong:
(a) rev7 Fix 4 changes `revision` to require one argument, so the only
documented route to the head fails closed on every call as written;
(b) Fix 2 plus edit 4.1 add the `open` discovery command, which the model is
never told about. The model could reach it only by guessing
`raw workspace.open ""`. `raw` is documented, but the permitted method names
are not listed anywhere in TOOLING.md.
The first acceptance step (`workspace.open` → head binding) is therefore
unreachable through the documented surface on the route the trials measure.
No gate catches this: `test_tooling.py:37` checks only that the digest is
deterministic, and `tooling_digest` is not pinned anywhere (verified).
Pre-existing defect on the same line: `:87` has a missing newline, which
fuses `sig ENTITY_HEX` and `revision` into one line of the agent's contract.
Remedy — edit 4.2, in the same commit:
- Add `.sley-live/sley-tool open` and `.sley-live/sley-tool revision TX_HEX`
  as separate lines, fixing the `:87` fusion.
- Add one sentence: `open` reports the accepted-head summary; `revision`
  takes a tx from a prior `open`, because no state survives across
  invocations (`:72`).
- Pin the coupling with a test in `test_tooling.py`: every
  `mediated_sley.ALLOWED_COMMANDS` name appears as `sley-tool <name>` in
  `SLEY2_TOOLING`. That holds for all 14 names today and fails on the
  missing `open`. This makes a fifth enumeration mechanical.

[P2] [completeness] Rev7 Fix 4's arity change has callers outside every
inventory. This is my miss too: I ratified option (a) at rev7 without
enumerating the callers. `revision` with no argument is invoked by:
- `mediated_client.py:113,298,381,438,595,727,750,760,771`. This is the
  deterministic adapter that `stage_test_adapter`
  (`mediated_attempt.py:194-206`) runs through the real confinement,
  mediation and capture stack.
- The witness producers `succ_witness_sig.py:139` and
  `succ_witness_type_full.py:165`.
- Tests: `test_sley2_tool.py:68,72,92,177,186,236,259,277` and
  `test_agent_access.py:67,88,100,101`.
The `eighteen` grep floor (C4) cannot see any of these. The test sites fail
loudly under `make quick`; the witness scripts fail only when run. Nothing
yet tests the fail-closed condition Fix 4 names (a missing or invalid tx is
refused).
Remedy:
- Add these sites to the C1 active list. Disposition: the adapter and the
  witness scripts call `open`, then `revision <tx>`; the tests are updated
  the same way.
- Add one negative test: `revision` with no argument, and with a
  wrong-length tx, is refused.
- `capture_demo.py:373` only sets a capture label; no change needed.

[P3] [record] Item 4 lists three load-bearing checker/spec lines; there is
a fourth. `_spec_allowlist` ends its parse region at the literal
`region.find("Those two entity names")` (checker `:139-140`), which matches
spec `:297`. Appending `workspace.open` last puts that sentence directly
after it, so "those two" then appears to include `workspace.open`, while
the actual antecedent is the two `entity.*` names. An implementer who
rewords the sentence for clarity trips `spec-allowlist:missing`. That fails
closed, but it is exactly the kind of surprise Item 4 exists to prevent.
Remedy: either keep the literal verbatim (clarify the antecedent without
touching the phrase), or co-edit `:139` in the same commit. Say which.
The 2000-character region bound is not at risk: the terminator currently
sits about 400 characters past the anchor, by inspection.

[P3] [surface] Remaining enumerations and one implementation constraint:
- `bench/live/mediated_shim.py:23-29` maps commands to phases in
  `READ_COMMANDS`/`COMPOSE_COMMANDS`. `open` lands in "read" only through
  the `_phase` default (`:42`), and `trusted_capture` does not validate
  phase per command. The result is correct by fallback, not by
  enumeration. Remedy: add `"open"` to `READ_COMMANDS`.
- The dedicated `open` command also adds a new capture label, `open` (from
  the `mediated_sley.py:224-225` else-branch). Item 3 names only the raw
  label flip.
- Constraint: the judge rejects any `omitted>0` or `truncated` response on a
  method outside `BOUNDED_QUERY_METHODS` — unmediated audit
  `sley2_live_judge.py:3162-3165`, mediated audit `:3465-3466`. Neither
  `open` nor `workspace.open` is in that set (`:2990-2991`). Rev3's
  "field 9 absent" omission must therefore be a structural absence in the
  response body, never a `bounds.omitted` signal. Otherwise every
  cold-snapshot trial rejects as "hidden truncation on open".
  State this in the implementation spec.

[P3] [record] The packet cites a malformed hash for the rev8 transcript.
Line 9 gives `e617e0c1db180d38b71df46d8b2965dfe50e373335c0e9`: 46 hex
characters, so not a sha256. It is also not a prefix of the actual value,
`e617e0c1db180d38db658cf288b49804ebd360a21da89dc914e5c8d539b4b13b`; the two
diverge at character 17. The gate record (`GATE-RECORD-...:51`) uses
`e617e0c1...`, which is consistent with the real value. This is the same
class of fault Item 5 guards against. Remedy: the next packet cites the
full value and corrects this one in the open; the committed rev9 packet
stays as history.

[P4] [record] `sley2_tool.py:19-34`, the module docstring's command list,
shows `revision` with no argument (`:24`) and no `open`. It is not visible
to the agent: `main` never prints `__doc__` (verified no reference). Fold
the correction into the `:10`/`:84` prose edit.

[P4] [record] Transcript per the convention:
`evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-rev9-95d42fda.md`.
This session wrote nothing; the writing lane owns it. The Fix 1 A4
demonstration is still owed to the implementation gate record.

SUMMARY: REVISE. The architecture is still settled; nothing above is design.
All five rev9 items are verified closed:
- Item 1: the `:51-57` import holds; `test_sley2_tool.py:17` imports only
  `bench.live`; the checker has zero `sley2_tool`/`bench/live` references;
  both tuples match in order today, and `workspace.open` also sorts last,
  so appending it keeps the pin assertion true.
- Item 2: `ALLOWED_COMMANDS` `:48-51` (14 names), `:62` inheritance, the
  `:173` gate and the `:187` forward are sufficient.
- Item 3: `:220-223` is correct as stated.
- Item 4: `:135` and `:257` are exact.
- Item 5: the rev6/rev7 artifacts are now committed at 10becd4d, and all
  four cited hash prefixes match the committed files.

The P1 and P2 continue the pattern I named at rev8, and my own framing of it
was incomplete. The command surface is not four coupled enumerations but at
least seven: `ARM_AFFORDANCES`, `TOOL_METHODS`, `ALLOWED_COMMANDS`, the
checker and spec literals, the agent's TOOLING.md, the shim's phase sets,
and the dispatch arity each caller depends on. The most important is the
one the model actually reads. To end the rev-by-rev chase, the
implementation gate record should include a mechanical sweep: every command
name, plus every `"revision"` call site, each given a disposition. The two
pin tests (TOOL_METHODS↔ARM_AFFORDANCES, ALLOWED_COMMANDS↔TOOLING.md) make
the load-bearing couplings mechanical. A clean rev10 needs only edit 4.2,
the P2 inventory and negative test, the terminator statement, the shim
line, the field-9 omission constraint, and the corrected hash.
```

**Assumptions**
1. **Read-only.** I wrote no files and ran no tests, checker or `make`. Two local measurement commands were refused by the sandbox, so the terminator distance above comes from reading spec `:288-302`, not from a computed value. All citations are to source at 2c97c32f. That tree's delta over 95d42fda is records-only (verified: 8 files, none of them source), so line numbers are identical to the baseline.
2. **PACKET_SHA256** is the hash of the committed `REQ-10-rev9-context-bounded-discovery.md`, whose text matches the dispatched packet as I received it. If the dispatched bytes differ, the writing lane should substitute the dispatch hash.
3. **P1 severity is my judgement.** The `raw` escape hatch technically still reaches `workspace.open`. I rate it above rev8's P2 anyway: here the documented command (`revision`) breaks outright, the new affordance cannot be found from the model's own contract, and no gate catches either.
4. **The field-9 constraint in P3-2 is conditional.** I did not inspect the Rust response encoder. It becomes a defect only if the implementation reports field-9 absence through `bounds.omitted`.
5. **Allowlist ratification** stays with the allowlist owner. The follow-on lanes (`vulcan_surface_review`, `ariadne_contract_review`) remain deferred and were not evaluated here.

Separately: the Google Calendar and Google Drive connectors need authorizing in claude.ai connector settings before they can be used. This review didn't need them.

---
Writing-lane record (not reviewer text): the rev9 packet landed at
evidence/review/requests/REQ-10-rev9-context-bounded-discovery.md;
sha256sum
dbfe12f3ac5429e1187eb315c81ac2d6539df51f000cc4d79de292eeca1a9aa5.
Reviewer text above the delimiter is byte-verbatim from
resume-20260918/review-REQ10r9-dispatch4-nabu.log (dispatch 4, exit 0,
engine claude-code from the forge-council review pool, run at tip
2c97c32f; dispatches 1-2 timed out on the hermes engine and dispatch 3
failed on a provider quota refusal "HTTP 400: You're out of extra usage",
log review-REQ10r9-dispatch3-nabu.log).
Reviewer verdict: REVISE_0_P0_1_P1_1_P2_3_P3_2_P4.
