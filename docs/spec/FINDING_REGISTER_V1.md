# Finding Register v1

Status: S20-740 contract draft, revision 10 (2026-09-23); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review). The mechanics are `scripts/build_finding_register.py`;
implementation state is tracked in the machine summary.

Revision 7 (2026-09-19) tightens the claim-to-transcript relation the
76ae15ab round found lexical (Ariadne/Nabu/Vulcan P3/P4): a closure line
is a per-finding status line in one of three shapes — the item's leading
bold head (`- **[P1] … — CLOSED.** evidence`), a status alone in a table
cell (`| … | P3 | **CLOSED (P3)** |`), or a standalone bold status span
(`… — **CLOSED.** evidence`, `**Both CLOSED.**`); a marker inside quotes,
backticks or parentheses, a finding-raising `[Pn] [kind]` line, a
`VERDICT:`/`SUMMARY:`/`FINDINGS:` line, or any line carrying an unquoted
`OPEN` status is never a closure line. The cited line must name the
claim's finding identity (the relation stated for the 1a9f0aab and
6589c6ec rounds below supersedes the revision's first wording: a
specific kind phrase not shared within the lane, an identifier not shared
within the lane and not the ledger's own vocabulary, a finding id, a
file:line anchor, a quoted phrase, or a path with a tag word), the
transcript must be
the claim's lane at a strictly later scope by git ancestry (not filename
order), a claim tag must be `<field>@<7..40 lowercase hex>`, `verified_by`
must be `<path>#L<n>[,<n>...] — note` (one group, no empty items), a claim
may not be open and closed at once, and the `PRIOR` fallback never serves
P0–P2. Closers are read from the live verdict fields and from every filed
transcript's `VERDICT` line, so a later non-`PRIOR` verdict does not void
an earlier automatic closure on regeneration
(`retire_review_claims.py --regenerate` re-derives every closure; `--check`
replays every recorded closure and refuses a stale one). A lane's later
re-statement of a carried open finding (same lane, kind, path and
identifier, naming its carried root) folds into the claim at the round
it names as `pN_restated_claims` `{claim, restates}`
(`--fold-restatements`); the named root stays open, a closure must
still postdate its scope and speak about it, and the register reports
`package_restated_claims`. One finding receives one status: `--check`
refuses a split — an identifier-bearing key, or a named carry and its
named root, open in one copy and closed by a transcript that strictly
postdates the open copy (the scope of the refusal is stated in full in
the revision 10 paragraph below). After the 1a9f0aab round (Nabu P2, Ariadne
P2/P3, Vulcan P3): the severity a closure line records is the one its own
leading token names (`[P1]`, `Prior P3`, `P2/P3`), never a severity
mentioned later in the head; the claim-to-line relation is a finding
identity, not vocabulary — a specific kind phrase of the claim's bracketed
tag verbatim (`fail-closed-gap`, `list-depth creep`; hyphen and space
equivalent), an identifier the finding names (never a path's own words),
a commit id, a finding id (`RW090-DEV-01`, `V-02`), a file anchor with
its lines (`exchange.rs:2974-3001`), a quoted phrase repeated verbatim,
or a path together with a tag word (a commit id likewise only together
with a tag word or a path); a generic one-word tag, a shared path alone,
or the ledger's own files (`machine-summary.json`, `finding-register.json`,
`claim-retirements.json`, the GA report and dossier) never relate. A
transcript must be in the git index (tracked or staged): an untracked
file is not a filed transcript. The section/lane/scope relation is
applied on every path — automatic, explicit, exact-claim and `--check`
replay.

After the 6589c6ec round (Ariadne P2/P3): a kind phrase is not an
identity when the lane files several findings under that kind — when any
other claim of the same lane and severity in the section (open, retired
or re-stated) carries the claim's kind, only a strong identity relates
(identifier, finding id, file:line anchor, quoted phrase) or an
exact-claim binding is required; a lane's field name, a transcript stem,
a per-package section name and the ledger's own field names
(`p3_open`, `p4_closed_claims`, …) are never identifiers on either side.
**OPEN-line rule:** a transcript that records the claim's severity OPEN
— an item status line or a finding-raising `[Pn]` line whose own head
names the claim by a strong identity — cannot close that claim on
another line; this binds every closure, exact-claim bindings included.
Shared vocabulary is computed per claim: any kind phrase or identifier
another claim of the same lane and severity in the section carries is
excluded from that claim's identity. Every filed transcript of a lane
that records per-finding closure lines is a closer for the severities
those lines record, whether or not its verdict token carries a `PRIOR`
group (a lane may close carried findings and raise new ones in one
verdict); each claim is still bound to its own speaking line. A
re-statement's finding key uses the description when its anchor is a
ledger file. A claim's severity is bound to the `[Pn]` finding line its
raising transcript records for it (`raising_severity`; a mismatch is
`REGISTER_SUMMARY_INVALID`). Round folding consults the notes' `on <sha>`
scopes: a PASS whose scope is not strictly later than a FAIL/REVISE
round's scope by git ancestry never folds it, whatever the calendar day.
Equal scopes are never strictly later (Ariadne P4 at 8966da2e:
`merge-base --is-ancestor x x` exits 0); a same-scope PASS counts as
filed before-or-at the FAIL for scope ordering but never folds it.
`retire_review_claims.py --regenerate` runs reopen → retire → fold in
that order and `--check` also regenerates in memory and refuses a
tracked ledger the regeneration does not reproduce
(`regeneration_divergence`).

After the 79fdcc63 round (Ariadne/Nabu/Vulcan P3): every claim has a
raising scope — its tag, else the earliest filed transcript of its lane
(or, lane-less, of its field) whose `[Pn]` finding line begins with the
claim's description, else a frozen revision field's or closure-note
field's `on <sha>` — and the own-round and strictly-later rules apply to
it on every path; a claim with no raising scope never retires
automatically. Closers are ordered by git ancestry (the earliest closing
transcript binds) and a line whose own head names the finding is
preferred over one naming it only in trailing prose. Document ids
(`ADR-0040`, `S20-540`) are not finding ids; an anchor's line span
(`413-417,424-425`) relates only when the line also names the claim's
file basename, or the claim cites no file (revision 8, stated at revision
10), and a single line number never does; a re-statement's leading carry marker
(`(carried from <sha>, OPEN …)`) is stripped before its finding key is
taken. Finding ids are shared vocabulary like kinds and identifiers; a
finding id carried by a cited path is not the claim's. The strong read
(a kind the lane shares) is the item's own head: an identifier, finding
id, anchor, quoted phrase, or a path with a tag word standing there; the
OPEN-line refusal reads the head without the path rule. A table cell's
parenthetical is at most 24 characters and never carries OPEN
(`**CLOSED (leg 2 OPEN)**` is not a closure). The raising-severity
binding applies to open, retired and re-stated claims alike.

After the b58ac1e0 round (Ariadne/Nabu/Vulcan P3): a finding key is the
lane, the kind and — for a finding against a source or document file —
that file plus its first identifier (a carried re-statement keys on the
identifier it carries, never a wildcard); for a finding against
the ledger itself, the round it originates from (the `carried from
<sha>` scope a re-statement names, else the claim's raising scope, else
its description) plus the identifier. Two keys name one
finding when lane, kind and file/origin agree and their identifiers are
equal (`same_finding`); whether a claim is a
re-statement is decided by the 8f774d0c rule below, never by a bare carry
word. A shared kind's own
words are not tag words under the strong read. A live `PRIOR` field's
closer carries the union of its token's closed severities and the
severities its transcript's status lines record.

After the 8f774d0c round (Ariadne/Nabu/Vulcan P2): a re-statement is
only a claim that names its carried root — a leading
`(carried from <sha>, …)` clause or a `carried from <sha>` phrase — never
a claim whose prose merely says `prior`, `unchanged` or `again`; such a
claim keys on the identifier it carries and meets only the claim
carrying that identifier; an original without a backticked identifier
matches only another identifier-less original. Shared vocabulary counts
every other claim of the lane and severity except a same-finding carry
in either direction. Regeneration runs reopen (closures and
folds alike) → retire → fold: a re-statement folds into a retired root
only when the root's closing line names the re-statement too, else it
stays open. The change record
`evidence/review/rounds/revision-7-retirement-changes.json` is derived
from the tracked ledgers by a stated keying (see its `keying`). Where the reviewer's per-finding closure line
cannot be seen by that relation (a claim recorded truncated or phrased
differently), the tracked retirement file may bind the whole claim string
to its lines (`claims: [{claim, lines, reason}]`, recorded as
`binding: exact-claim`); the builder and `--check` refuse an exact-claim
closure the tracked file does not carry line for line. An untagged claim
of a frozen `_revision_N` field takes its scope from that field's `_note`;
an explicit prefix names one whole field of the section, lane-less fields
included.

After the 8966da2e round (Ariadne/Nabu/Vulcan P2): a leading carry clause
counts only when it names its root (`from <sha>` inside the parentheses);
a bare `(prior)`, `(carried, unchanged)` or any other sha-less
parenthetical leaves an original with its own identifier. A fold resolves
its root by the sha the re-statement names (tag or raising scope of a
same-finding claim), nearest by git ancestry when several match, and stays
open when nothing at that round matches — never by earliest scope or list
position. A fold into a retired root requires the root's own cited closing
lines to name the re-statement under the retirement read (strong identity
with shared vocabulary, OPEN-line refusal). The closing transcript must
strictly postdate the re-statement — a claim filed after the closure was
written inherits nothing. An open claim whose exact key matches a
retired claim inherits that status (it restates the closed member)
under the same postdating rule, unless the closing transcript records
it OPEN — one finding, one status; a remaining split is refused by
`--check` until explicitly reconciled. The ledger replays the fold's
rule: a re-statement naming a sha restates a claim at that scope,
except for an exact-key inheritance into a retired claim. Shared
vocabulary exempts a same-finding carry derived from the claims
themselves, never from the ledger's popped `restates` link, so the
incremental and regenerated reads agree; two originals sharing one key
still count toward each other's vocabulary. This is contract revision
8.

After the 8966da2e round's second batch (2026-09-22/23: Ariadne, Nabu
and Vulcan reproducibility P2/P3s, Vulcan standards P2/P3s, Vulcan
s20_700 closure P2/P3): identity by absence is no identity. An absent
identifier (`""`) never makes two claims one finding for the shared
vocabulary (two carries of distinct findings without backticked
identifiers still count toward each other), for exact-key inheritance or
for the split-status refusal. A `carried from <sha>` phrase counts only
outside backticked and quoted spans (`unquoted`): a claim quoting another
claim's carry is not a re-statement and does not take that round as its
origin. Exact-key inheritance into a retired claim requires a non-empty
identifier and a cited closing line of the retired claim that speaks
about the inheriting claim under the retirement read (strong identity,
with the lane's shared vocabulary less the inheriting pair itself, and
the OPEN-line refusal), from a closing transcript that strictly
postdates it; a named carry inherits only at the round it names or from
a retired carry of the same named root, and the ledger replays the same
named-sha rule. A named carry whose round holds two same-key claims is
ambiguous and stays open. The split-status refusal applies to an
identifier-bearing key whose open copy the closing round could have
judged (the closer strictly postdates it); a copy raised at or after the
closing round is that round's own statement. An uncarried copy never
folds by key alone — a lane may raise a distinct finding on an
identifier it used before — so `pN_open_count` is a claim count (one per
round that stated a finding) and the register reports
`package_open_findings`, the distinct identifier-bearing keys per open
list plus each identifier-less claim; the GA row gates on the claim
count, which can only over-count, and states both. A tagged claim whose
raising round records no finding line for it is refused in every list
(`SUMMARY_INVALID`), and a lane-less field reads its own
`<stem>*-<scope7>.md` transcript for its raising severity. `--check`
also reports, without failing, a live PRIOR token that closes fewer
severities than its transcript's status lines (`closer_disagreements`);
the union still closes, per the b58ac1e0 rule. This is contract
revision 9.

After the d158d26b round (2026-09-23: Nabu and Vulcan packaging P2s;
Ariadne packaging, Ariadne, Nabu and Vulcan reproducibility and Vulcan
standards P3s), every ambiguity fails closed and every rule the mechanics
apply is stated here; where this text and an earlier paragraph differ,
this paragraph governs.
- Folding. A named carry (a leading `(carried from <sha>, …)` clause or a
  `carried from <sha>` phrase outside quoted spans) folds into the one
  open claim of its exact finding key raised at the round it names, from
  a strictly later round; two such claims fold nothing. A fold never
  transfers a closure: there is no exact-key inheritance and no fold into
  a retired root, so a carry whose root is retired stays open until
  `retire` closes it on its own closing lines or an exact-claim binding
  does. The ledger refuses a re-statement that is not a named carry of a
  claim at the named round, that is not strictly later, or whose chain
  ends at a retired claim.
- OPEN refusal. The OPEN item's whole head is read under the strong read
  (identifier, finding id, file:line anchor, quoted phrase; no path rule,
  no shared-vocabulary exemption). It applies to automatic closures, the
  explicit rule, exact-claim bindings, the builder and the replay alike.
  An exact-claim binding alone may declare, line by line in
  `open_lines_of_other_findings` with a reason, an OPEN line that is
  another finding's; an undeclared matching line, or a declared line that
  is not such an OPEN line, refuses the closure.
- Split status. `--check` refuses (a) an identifier-bearing key open in
  one copy and closed in another by a transcript that strictly postdates
  the open copy, and (b) a named carry and its named root — the one claim
  at the named round with the carry's exact key, else the one claim there
  of its lane, kind and file, whatever the identifiers — one open and the
  other closed by a transcript that strictly postdates the open member.
  A copy raised at or after the closing round is that round's own
  statement. Every split is reconciled by an exact-claim binding, never
  by a broader rule.
- Identity. An absent identifier (`""`) is no identity; ledger field
  names (the `pN_*` lists, the register's `package_*` outputs, lane and
  verdict field names) are never identities, so a finding whose subject
  is such a name is closed by an exact-claim binding; the span rule above
  needs the claim's file basename on the line.
- Records. Every closure a rule change reopens, and every status a
  re-derivation changes, is listed per claim with the refusing rule in
  `evidence/review/rounds/revision-10-retirement-changes.json`; the eight
  reviewer-verified closures revisions 8 and 9 reopened are restored by
  exact-claim bindings there listed. This is contract revision 10.
Every closure recorded at 76ae15ab that revision 7 changed is listed per
claim, with the refusing rule where it stays open, in
`evidence/review/rounds/revision-7-retirement-changes.json` (the round's
first statement that "none reopened" was measured against the wrong
baseline and is corrected there).

Revision 6 (2026-09-18) names the per-package claim ledger's retirement
path (Nabu/Vulcan/Ariadne P3s at db53894e: the retirement had been done by
an untracked script and recorded in fields no contract named): an entry of
`pN_open` is `"<field>[@<scope7>]: <finding>"` and leaves the open list only
into `pN_closed_claims` as `{claim, verified_by}` where `verified_by` is
`<path>#L<n>[,<m>...] — <note>`: the repository path of an existing
transcript (literal, under `evidence/review/verdicts/` or
`machineresearch/sley-2.0/reviews/`, no `..`, no absolute path) and the
lines of it that record the closure — closure lines: the severity named
before a `CLOSED` status marker (`— CLOSED`, `: CLOSED`, `**CLOSED`) with
no `OPEN` status marker on the same line, so prose that merely contains the
word or a line recording a partial (`leg 1 CLOSED; leg 2 OPEN`) or open
status never counts; when the transcript names no such line for the
severity, every closure line of a verdict whose `PRIOR` clause closes that
severity. The transcript must belong to the claim's section directory and
the claim's lane, and a scope-tagged claim can never cite its own round
(c67b0729 round: the relation had been lexical only). The verifying transcript
is either the same lane's later verdict carrying `PRIOR_…_PN_…_CLOSED` at a
strictly later scope, resolved as `<section>/<lane>*-<scope7>.md`
(automatic rule), or a closure review named in the tracked
`evidence/review/claim-retirements.json` with the cited lines and a reason
(explicit rule). `scripts/retire_review_claims.py` applies both rules with
the register's own `closed_severities` grammar (one grammar); the register
builder refuses a `verified_by` whose transcript does not exist, is not a
transcript path, or records no closure of the severity at the cited lines,
and reports `package_closed_claims`. Retired claims are not open claims:
the CLEAR predicate and the GA open-findings row read only `pN_open`. The
same round's Vulcan/Nabu P3 corrected the closure grammar: a
count-prefixed severity (`2_P4`) is a fresh count and never a closure
claim, with or without a following `_PRIOR_…_CLOSED` clause; only the
severities inside the `PRIOR` clause, or in a run of severities ending at
the anchor, are closed.

Revision 5 (2026-09-18) adds chronology to round folding: a `PASS` whose
`_note` is dated before the `FAIL`/`REVISE` round's `_note` never folds
that round, whatever the round tokens say (Vulcan P4 at 92fa6646: a lane
could otherwise pre-file a `*_final_review` `PASS` and fold every later
`REVISE`); undated notes keep the token rule. No verdict is reclassified;
two reproducibility Ariadne revisions that an older base `PASS` had folded
now read `PENDING` until the lane's later `PASS` is filed.

Revision 4 closes the two precision gaps the Vulcan re-review of the live
register kept open as P3s: a `PASS` that still names findings blocks
clearance unless the review declares them closed or the section tracks them
(`unclaimed_carried_findings`), and sections whose status says `COMPLETE`
without satisfying the completion test are named with their open counts
(`mid_string_complete_packages`). Neither repair reclassifies a verdict.

Revision 2 answers the three Council reviews of the revision-1 draft (8
P0s); the answers are itemized in section 7. No register value changed
meaning silently: every tightened rule is stated here before it runs.

## Boundary

S20-740 needs one machine-readable answer to "which review obligations exist,
which are open, and which severity tokens their dispositions name" before an
independent reviewer can issue a complete PASS (master goal sections 16.7 and
26.9, dossier item "findings by severity and disposition"). This contract
freezes that register: how review obligations are derived from the machine
summary, how their dispositions are classified, and which invariant a
completed package must satisfy.

It reports per-obligation severity *tokens named in dispositions*, not
per-finding severity: a disposition that closes two P1s and a bare `PENDING`
both appear honestly for what they record, and a bare pending review carries
no severity because none was recorded. The per-finding record the dossier
item ultimately needs (id, title, severity, disposition, owning package,
closing commit) is future work named in section 7, not this register.

It is not the independent review itself, does not issue or accept findings,
and does not change any package's status. The review remains Vulcan's, and
`make release-check` and `make v2` stay fail-closed.

## 1. Source

`machineresearch/sley-2.0/machine-summary.json` is the only source: it is the
tracked record every package updates. The register walks it and collects every
string field whose name contains `review` or `disposition`, plus every
lane-named string leaf (`ariadne`, `nabu`, `vulcan`, `merlin`, `codex`)
directly under a `review`/`disposition` record such as
`current_delta_review`, except

- fields naming a role, session, actor, or instant, by anchored suffix:
  `reviewer_role`, `*_session_id`, `*_at`, `*_by`, `*_id`, `*_timestamp`,
  `*_note`, and `*reviews`. The anchors are exact: a field that merely
  contains one of these words elsewhere is collected, not dropped;
- values that are an ISO-8601 instant (`^\d{4}-\d{2}-\d{2}T`) or a Council
  session identifier (`^forge-` or `^[a-z]+-[a-z]+-s20-`), which name a
  review event rather than record its disposition;
- the register's own verdict field `independent_review`: it is the review's
  output, not an input obligation. Collecting it would make
  `S20_740_COMPLETE` unreachable, because the verdict must read `PENDING`
  until the review it awaits has happened. The stage checker asserts it
  separately per status instead.

A missing summary is `REGISTER_SUMMARY_MISSING`; a summary that is not an
object, or that carries no review obligation at all, is
`REGISTER_SUMMARY_INVALID`.

## 2. Obligations

Each collected field is one obligation:

```text
obligation = {
  "section": dotted path of the owning object ("(root)" at the top level),
  "field": field name,
  "disposition": the recorded string,
  "state": "PASS" | "PENDING" | "DEFERRED" | "HISTORICAL_ROUND" | "OTHER",
  "reviewer": the lane token the field name carries, or null,
  "severities": ascending list of the P0..P4 tokens the disposition names
    outside negations,
  "declares_closed_findings": bool,
  "declares_no_open_p0_p1_p2": bool,
  "package_status": the owning section's status, or null,
  "superseded_by": the superseding PASS field, or null,
  "round_date": the latest YYYY-MM-DD date in the field's `_note`, or null
}
```

States are exact over the first underscore-delimited token, matched against
the closed head set `PASS`, `PENDING`, `DEFERRED`, `FAIL`, `REVISE`, plus the
declared alias table (`VULCAN_PASS` reads `PASS`; the table lives in the
builder and this section names every entry, so no alias is ever silent):

- `PASS` when the head token is `PASS` or the disposition is a declared
  alias; except a `PASS` head that still claims an open finding outside a
  negation group (`P0_OPEN`, `OPEN_P1`) contradicts itself and is `OTHER`;
- `PENDING` when the head token is `PENDING`;
- `DEFERRED` when the head token is `DEFERRED` (a lane that was unavailable);
- `HISTORICAL_ROUND` when the head token is `FAIL` or `REVISE` **and** the
  same section records a `PASS` obligation carrying the same reviewer lane
  token that closes this round (`superseded_by` names that field); a
  `FAIL`/`REVISE` round with no closing `PASS` was never re-reviewed, so it
  stays `PENDING`. Closure runs from the qualified or early round toward the
  general or later review: an `initial`/`first`/`revision-N` round folds into
  the `final` or unmarked review, and a qualified subject round folds into
  the strictly less qualified review. A slice `PASS` never closes the
  overall `FAIL` it belongs to, and a scoped `PASS` never closes a round
  from another subject. Round ordering follows the field-name
  convention (`initial` before `final`, revision numbers ascending) and is
  enforced: a cross-core fold needs round evidence (an early token on the
  round or a late token on the `PASS`), so an older general `PASS` never
  closes a newer qualified `FAIL`, and two unmarked rounds never fold
  across cores. Chronology is enforced when both rounds' `_note` fields
  carry calendar dates (`YYYY-MM-DD`; the latest date in the note): a
  `PASS` dated before the round it would close does not close it
  (revision 5); `round_date` is recorded on every obligation;
- `OTHER` otherwise, which the register surfaces rather than silently
  normalizing.

`reviewer` is the first of `ariadne`, `nabu`, `vulcan`, `merlin`, `codex`
appearing in the field name, or null when the field names no lane; a
`FAIL`/`REVISE` round with no lane token can never match a superseder and
stays open.

`severities` strips every `NO_OPEN_P0...` and `NO_NEW_P0...` negation group
before scanning, then deduplicates: `PASS_NO_OPEN_P0_P1_P2` carries no
severity tokens, while `PASS_PRIOR_P2_P3_CLOSED_NO_NEW_P0_P1_P2_P3_P4`
carries `P2, P3`. Count-prefixed encodings name zero counts explicitly, and
a zero count is an absence claim like a negation: `FAIL_0_P0` names no `P0`
mention, and an all-zero `PASS_0_P0_0_P1_0_P2_0_P3` carries none at all. The list is distinct tokens per obligation, not a finding
count, and a negated or zero-count token is an absence claim, never a mention.

`declares_closed_findings` is true when the disposition names `CLOSED`;
`declares_no_open_p0_p1_p2` is true when it names `NO_OPEN_P0_P1_P2`.

## 3. Register

`evidence/review/finding-register.json`:

```text
register = {
  "contract": "sley2.finding-register.v1",
  "contract_revision": integer,
  "work_package": "S20-740",
  "source": "machineresearch/sley-2.0/machine-summary.json",
  "obligations_digest": SHA-256 of the canonical obligation list,
  "obligation_count": integer,
  "states": { state: count },
  "severity_mentions": { "P0".."P4": count },
  "obligations": [ obligation ... ] ascending, the full payload the
    obligations_digest covers,
  "open_reviews": [ {section, field, disposition, severities} ... ]
    ascending, the PENDING obligations with what each records,
  "unclaimed_carried_findings": [ {section, field, disposition,
    unclaimed_severities} ... ] ascending: PASS obligations that still name
    severities (after valid-negation strip and zero-count absence) where
    every named severity is unaccounted for: outside the review's own
    per-severity `CLOSED` scope (only lane words may stand between the
    severity and the `_CLOSED` anchor, a count-prefixed severity such as
    `2_P4` is a fresh count and never a closure claim, the anchor needs a right word
    boundary, and the claim must be terminal except for absence
    (`NO_...`) and followup (`WITH_...`) declarations carrying content
    (a bare trailing `NO`/`WITH` keyword is vacuous) — so a `P2`
    closure never covers carried `P3/P4` followups, `DISCLOSED`/
    `UNCLOSED` substrings never exempt, and `P1_CLOSED_CIRCUIT` word
    salad is not a closure claim),
    outside a valid negation group (a group stacked under `NO_` is void
    and declares nothing), and outside the section's per-package open
    claims. Visible in severity_mentions but able to survive into a CLEAR
    read, so they block clearance until claimed or closed — without
    reclassifying the verdict.
  "mid_string_complete_packages": [ {section, status, open_obligations} ... ]
    ascending: sections whose status contains `COMPLETE` but does not end
    `COMPLETE` (restricted / proposal / boundary language). Not a
    completion claim, so not a violation; named here with open counts so
    the precision gap lives in the artifact, not in prose.
  "deferred_reviews": [ {section, field, disposition} ... ] ascending,
  "unclassified": [ {section, field, disposition} ... ] ascending, state OTHER,
  "superseded_rounds": [ {section, field, disposition, superseded_by} ... ]
    ascending, the HISTORICAL_ROUND obligations and what superseded each,
  "complete_packages": [section, ...] ascending, sections whose status ends COMPLETE,
  "complete_packages_with_open_reviews": [ {section, field, state} ... ],
  "declared_open_findings": the summary's open_findings counters,
  "package_open_claims": { "section.field": open count } ascending, every
    per-package p0..p4 open list length and open count the summary carries,
  "package_restated_claims": { "section.pN_restated_claims": count } ascending,
    every folded re-statement list (revision 7); each entry is {claim,
    restates} with restates naming a claim of the same section and
    severity that is open, retired or itself re-stated, and the re-stated
    claim listed nowhere else,
  "package_closed_claims": { "section.pN_closed_claims": count } ascending,
    every retired per-package claim list (revision 6); each entry is
    {claim, verified_by[, binding: "exact-claim"]} with verified_by naming an existing transcript
    under evidence/review/verdicts/ or machineresearch/sley-2.0/reviews/
    and the cited lines that record the closure of that severity,
  "result": "FINDING_REGISTER_CLEAR" | "FINDING_REGISTER_OPEN",
  "register_digest": SHA-256 of the canonical register without this field
}
```

Rules:

- the summary's `open_findings` must carry exactly the non-negative int
  counters `p0` through `p4`; anything else is `REGISTER_SUMMARY_INVALID`,
  because a missing or non-numeric counter would let the register read
  clear vacuously. Every per-package open list must be a list and every
  per-package open count a non-negative int, or the summary is likewise
  invalid; every per-package closed-claim entry must be `{claim,
  verified_by}` with an existing transcript path (revision 6), or the
  summary is invalid — a claim leaves the open ledger only through a
  transcript. `verified_by` is `<transcript path>#L<lines> — <note>`; the
  path is repository-relative (no `..`, no absolute path) and must resolve
  under the section's own verdict directory; the cited lines must each be
  closure lines (the severity named before the item's own `CLOSED` status
  in a bold head, a table cell or a standalone bold status span, with no
  unquoted `OPEN` status on the line — revision 7) and at least one must
  name the retired claim's finding identity (a kind phrase or identifier
  not shared within the lane, a finding id, a file:line anchor, a quoted
  phrase, or a path with a tag word — never the ledger's own vocabulary);
  the verifying transcript must belong to the same lane as the retired
  claim's round, must not be that round's own transcript, and must record a
  verdict at a strictly later scope commit (git ancestry, not filename
  order). Closers are derived from the live verdict fields and from every
  section transcript's `VERDICT` line (`scripts/retire_review_claims.py`),
  so a closure a later transcript records is not lost when the live field
  is later normalized; `--check` replays every recorded closure against its
  transcript and refuses a stale one or a claim listed open and closed at
  once, and `make quick` / `release-candidate-verify` run it. A citation
  that fails any of these is refused and the claim stays open — prose that
  mentions the severity ("no P1 exists", "leg 1 CLOSED; leg 2 OPEN", a
  quoted "— CLOSED") is not a closure line. A lane's later re-statement of
  a carried open finding folds into the earliest claim
  (`pN_restated_claims`, revision 7) and is neither open nor closed;
- a retired claim's cited closure line must not be contradicted by the
  same transcript: if the transcript records the claim's severity OPEN
  in an item head or finding line that names the claim by a strong
  identity, the closure is `REGISTER_SUMMARY_INVALID` (exact-claim
  bindings included);
- the result is `FINDING_REGISTER_CLEAR` exactly when no obligation is
  `PENDING`, no obligation is `OTHER`, `unclaimed_carried_findings` is
  empty, `complete_packages_with_open_reviews`
  is empty, every top-level open-finding counter is zero, and every
  per-package open claim is zero; otherwise it is `FINDING_REGISTER_OPEN`
  and the register names what is open. A `DEFERRED` lane is recorded
  unavailability, not an open finding, so it does not block clearance by
  itself;
- **a completed package may not carry an open review**: a section whose status
  ends in `COMPLETE` (but not `INCOMPLETE` or `NOT_COMPLETE`) with any
  obligation in state `PENDING`, `DEFERRED`, or `OTHER` is
  `REGISTER_COMPLETION_VIOLATION`, and no register is written. A
  `HISTORICAL_ROUND` obligation is allowed there, because a package reaches
  completion by passing after earlier rounds failed, and `superseded_rounds`
  names the passing field that proves it;
- the register carries no timestamp, host name, user name, or path outside the
  repository, and is a pure function of the summary's obligations, so `--check`
  detects drift with `REGISTER_DRIFT`;
- the digest covers the derived obligation list rather than the summary bytes,
  because the summary also records this register's own counts: a byte digest
  would never reach a fixed point, while an obligation digest is stable under
  the counter updates the checker cross-checks;
- `FINDING_REGISTER_OPEN` is the expected state while Council lanes are down;
  it is not a failure of this package, and the checker does not require
  `FINDING_REGISTER_CLEAR`. S20-740 itself cannot complete until the register
  reads `FINDING_REGISTER_CLEAR` and an independent reviewer records a PASS.

## 4. Codes

S20-740 reserves 75000 through 75003: `REGISTER_SUMMARY_MISSING` (75000),
`REGISTER_SUMMARY_INVALID` (75001), `REGISTER_COMPLETION_VIOLATION` (75002),
`REGISTER_DRIFT` (75003). The script exits 1 and prints one JSON object naming
the code on failure.

## 5. Staging

`scripts/check_finding_register.py` runs under `make quick`. Statuses:
`S20_740_CONTRACT_DRAFT_REVIEW_PENDING`,
`S20_740_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS`,
`S20_740_REGISTER_IMPLEMENTED_REVIEW_PENDING`, and `S20_740_COMPLETE`, the last
requiring the three Council reviews to read `PASS`, the register to read
`FINDING_REGISTER_CLEAR`, and an independent review record. In every
implementation status the checker verifies the register exists with its
contract tag and revision, that its obligation payload matches the summary
(count and digest, not just the tallies), that no completed package carries
an open review, that the unit tests pass, and that `release-check` and `v2`
stay `NOT_IMPLEMENTED`. The checker pins the summary's `contract_revision`
to this document's revision, so a contract edit without its revision number
fails the gate.

## 6. Explicit exclusions

- No independent review, no finding acceptance, no severity judgment: the
  register reports what packages recorded, and never edits a disposition.
- No creation, closure, or reopening of findings, and no status change to any
  package.
- No Council dispatch: the register is derived offline from tracked evidence.
- No GA claim, release decision, or publication.

## 7. Clarifications

Revision 4 (2026-09-13) closes the two precision gaps the Vulcan re-review
of the live register kept open as P3s: a `PASS` that still names findings
blocks clearance unless the review declares them closed or the section
tracks them in its per-package open claims (`unclaimed_carried_findings`;
states unchanged, verdicts not reclassified), and mid-string-`COMPLETE`
statuses are named with open counts (`mid_string_complete_packages`).

Revision 3 (2026-09-11) enforces the round ordering section 7 already
required: a cross-core fold needs round evidence, so the older general
`PASS`es no longer close the newer qualified `FAIL`s they predate (the
live cases were six S20-360/S20-390 rounds reading closed), lane-named
leaves under `review`/`disposition` records are collected as obligations
(the live case was five `current_delta_review` records reading invisible),
and supersession requires lane-core compatibility so a scoped `PASS` can
never fold a foreign round.

Revision 2 (2026-09-05) answers the Council reviews of the revision-1 draft:
a `FAIL`/`REVISE` round needs a same-lane superseding `PASS` (Ariadne P0-1,
Vulcan P0-2; the two restricted-query Nabu `REVISE` records were re-reviewed
to `PASS` for exactly this rule); classification is first-token over a closed
head set with a declared alias table, and a self-contradicting `PASS` is
`OTHER` (Ariadne P0-2); open reviews carry disposition and severities while
the Boundary promises tokens, not per-finding severity (Nabu P0-1); the
register's own verdict is asserted per status instead of collected, so
completion is reachable (Nabu P0-2); clearance reads the per-package open
claims beside the top-level counters (Vulcan P0-1); the obligation payload is
part of the frozen shape and digest-checked (Vulcan P0-3); severity tokens
exclude negations (Vulcan P0-4).

Completion is tested by status suffix (`COMPLETE`, excluding `INCOMPLETE`
and `NOT_COMPLETE`); mid-string boundary statuses such as
`S20_340_COMPLETE_IMMUTABLE_DESCRIPTORS_ONLY` are not treated as complete
for the violation check, but their unsuperseded reviews still block
clearance as `PENDING`, so the gap is reporting precision, not a silent
pass. Since revision 4 the precision gap lives in the artifact: every
status containing `COMPLETE` without satisfying the suffix test is named
in `mid_string_complete_packages` with its open-obligation count.

Round ordering is a field-name convention the register reads
directionally: closure runs early-or-qualified toward late-or-general, so a
slice `PASS` beside an overall `FAIL` does not supersede (the live case is
the S20-700 surface audit, whose slice `PASS`es leave the audit `FAIL`
open), and neither would a `PASS` recorded before its `FAIL`.

The per-finding record (id, title, severity, disposition, owning package,
closing commit, cross-checked by this register) that an independent review
starts from is future S20-740 work, not this contract.
