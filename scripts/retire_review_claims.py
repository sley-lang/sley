#!/usr/bin/env python3
"""Retire per-package open claims a later review verified closed.

Finding register contract section 3 (revision 6). An entry of a section's
`pN_open` list is `"<field>[@<scope7>]: <finding>"` — the lane field that
raised it and, from revision 6, the seven-hex scope it was recorded at. It
is retired to `pN_closed_claims` as `{claim, verified_by}` when either

- automatic rule: a verdict field of the SAME lane in the section carries
  `PRIOR_..._PN_..._CLOSED` (the lane re-verified its carried findings item
  by item) and was recorded at a scope that is a strict descendant of the
  entry's scope; `verified_by` is that verdict's transcript
  (`<section>/<lane>*-<scope7>.md`, resolved by scope; closers are read
  from the live verdict fields and from every filed transcript's VERDICT
  line) with the cited closure lines that speak about the claim; an entry
  without a scope tag is never retired automatically; or
- an explicit retirement in `evidence/review/claim-retirements.json`
  naming `section`, `severities`, `prefixes` and the `verified_by`
  transcript path (with a reason), for entries the automatic rule cannot
  see (untagged entries, lane-less rows, closure reviews whose verdict token
  omitted the `PRIOR` suffix but whose transcript records the closure).

Nothing vanishes: a retired entry stays in the section under
`pN_closed_claims`, and `build_finding_register.py` refuses a retirement
whose transcript does not exist. Usage: retire_review_claims.py [--check]
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from build_finding_register import (  # noqa: E402
    claim_relation_problem, claim_scope, closed_severities, cited_closure_lines, finding_key, is_carry,
    is_closure_line, is_tracked, line_speaks_about, named_carry_sha, open_lines_about, raising_scope, refusing_open_lines, same_finding,
    scope_generation, shared_vocabulary, shares_its_kind, transcript_path,
)

SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
RETIREMENTS = ROOT / "evidence/review/claim-retirements.json"
REVIEWERS = ("ariadne", "nabu", "vulcan", "merlin", "codex")
PRIOR = re.compile(r"_PRIOR_((?:P[0-4]_)*P[0-4])_CLOSED")
SCOPE = re.compile(r"\bon ([0-9a-f]{40})\b")
TRANSCRIPT = re.compile(r"transcript ((?:evidence/review/verdicts|machineresearch/sley-2\.0/reviews)/\S+?\.(?:md|log))")
TAG = re.compile(r"^([A-Za-z0-9_.]+?)(?:@([0-9a-f]{7,40}))?: ")
TRANSCRIPT_SCOPE = re.compile(r"[-@]([0-9a-f]{7,40})\.md$")


def transcript_scope(path: str) -> str | None:
    """The scope a transcript filename records (`…-<sha7>.md`); None when
    the path names no scope (Nabu/Vulcan P2 at 8966da2e: a closing line can
    only cover claims raised at or before its transcript's scope)."""
    match = TRANSCRIPT_SCOPE.search((path or "").split("#")[0])
    return match.group(1) if match else None


def role(field: str) -> str | None:
    """The lane a field, claim prefix or transcript name belongs to; dotted
    fields (`current_delta_review.<lane>`) split too (Ariadne P4 at 76ae15ab)."""
    for token in re.split(r"[._-]", field):
        if token in REVIEWERS:
            return token
    return None


def note_of(section: dict, field: str) -> str | None:
    if "." in field:
        parent, lane = field.split(".", 1)
        live = section.get(parent)
        text = live.get(lane + "_note") if isinstance(live, dict) else None
    else:
        text = section.get(field + "_note")
    return text if isinstance(text, str) else None


_ancestry: dict[tuple[str, str], bool] = {}


def strictly_later(later: str | None, earlier: str | None) -> bool:
    """`later` is a strict descendant of `earlier` in this repository."""
    if not later or not earlier or later.startswith(earlier) or earlier.startswith(later):
        return False
    key = (earlier, later)
    if key not in _ancestry:
        _ancestry[key] = (
            subprocess.run(
                ["git", "merge-base", "--is-ancestor", earlier, later],
                cwd=ROOT, check=False, capture_output=True,
            ).returncode
            == 0
        )
    return _ancestry[key]


def transcript_for(section_name: str, lane: str, scope: str | None, fallback: str | None) -> str | None:
    """The lane's transcript at `scope` (`<section>/<lane>*-<scope7>.md`), else the note's."""
    if scope:
        matches = sorted((ROOT / "evidence/review/verdicts" / section_name).glob(f"{lane}*-{scope[:7]}.md"))
        if len(matches) == 1:
            return matches[0].relative_to(ROOT).as_posix()
        # Zero or ambiguous matches never fall through to the note's path
        # silently (Vulcan P4 at 76ae15ab): a closer whose transcript cannot
        # be resolved is refused unless the note's path is one of the matches.
        if fallback and (ROOT / fallback) in matches:
            return fallback
        raise SystemExit(
            f"{section_name}: {len(matches)} transcripts match {lane}*-{scope[:7]}.md (note names {fallback})"
        )
    return fallback


VERDICT_LINE = re.compile(r"^VERDICT:\s*([A-Z0-9_]+)\s*$", re.M)
_resolved: dict[str, str | None] = {}


def resolve_scope(short: str) -> str | None:
    """The full commit id a transcript's scope stamp names (None when unknown)."""
    if short not in _resolved:
        done = subprocess.run(["git", "rev-parse", "--verify", "--quiet", f"{short}^{{commit}}"],
                              cwd=ROOT, capture_output=True, text=True, check=False)
        _resolved[short] = done.stdout.strip() if done.returncode == 0 and done.stdout.strip() else None
    return _resolved[short]


def transcript_closers(section_name: str) -> list[tuple[str, str, set[str], str | None, str | None]]:
    """Every filed transcript of the section that records a per-finding
    closure line or a PRIOR verdict: (file stem, lane, closed severities,
    scope, path). Transcripts
    are the durable record, so the closers do not depend on which round's
    verdict currently occupies the live field (76ae15ab round: reading live
    fields only made the regeneration non-monotone)."""
    found = []
    directory = ROOT / "evidence/review/verdicts" / section_name
    if not directory.is_dir():
        return found
    for path in sorted(directory.glob("*.md")):
        stamp = re.search(r"-([0-9a-f]{7,40})\.md$", path.name)
        lane = role(path.name)
        # Only filed transcripts (in the git index) are closers: an untracked
        # in-flight file must not retire anything (Vulcan P3 at 1a9f0aab). A
        # lane-less closure transcript (`<field>_closure-<scope>.md`) closes
        # for its own field, matched by stem in `retire()`.
        if not stamp or not is_tracked(path.relative_to(ROOT).as_posix()):
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        verdicts = VERDICT_LINE.findall(text)
        if not verdicts:
            continue
        # A filed transcript closes every severity it records a per-finding
        # closure line for (named lines, never the PRIOR fallback), whether or
        # not its verdict token carries a PRIOR group — a lane may close some
        # carried findings and raise new ones in one verdict (Ariadne P3 at
        # 6589c6ec: five 1a9f0aab closures had no closer because the tokens
        # omitted the suffix). Each claim is still bound to its own speaking
        # line under the identity relation.
        lines = text.splitlines()
        severities = {f"P{n}" for n in range(5) if any(is_closure_line(line, f"P{n}") for line in lines)}
        severities |= closed_severities(verdicts[-1]) if PRIOR.search(verdicts[-1]) else set()
        if not severities:
            continue
        found.append((path.stem, lane, severities, resolve_scope(stamp.group(1)), path.relative_to(ROOT).as_posix()))
    return found


def closers(section: dict, section_name: str = "") -> list[tuple[str, str, set[str], str | None, str | None]]:
    """(field, lane, closed severities, scope, transcript) per PRIOR verdict."""
    found = []
    candidates: list[tuple[str, object]] = [
        (field, value) for field, value in section.items() if not field.endswith("_note")
    ]
    live = section.get("current_delta_review")
    if isinstance(live, dict):
        candidates.extend((f"current_delta_review.{lane}", live.get(lane)) for lane in REVIEWERS)
    for field, value in candidates:
        if not isinstance(value, str) or role(field) is None:
            continue
        if not PRIOR.search(value):
            continue
        # One grammar (Nabu P3 at 76227765): the closed set is the register's
        # `closed_severities`, never a second parse of the token.
        severities = closed_severities(value)
        note = note_of(section, field) or ""
        scope = SCOPE.search(note)
        transcript = TRANSCRIPT.search(note)
        scope_sha = scope.group(1) if scope else None
        found.append((
            field,
            role(field),
            severities,
            scope_sha,
            transcript_for(section_name, role(field), scope_sha, transcript.group(1) if transcript else None),
        ))
    # A live PRIOR field's closer carries the union of its token's closed
    # severities and the severities its transcript's status lines record
    # (Nabu P3 at b58ac1e0: the line-derived entry had been dropped for a
    # transcript the live field named, narrowing it to the token).
    by_path = {item[4]: item for item in transcript_closers(section_name)}
    found = [
        (field, lane, severities | by_path[transcript][2], scope, transcript) if transcript in by_path else (field, lane, severities, scope, transcript)
        for field, lane, severities, scope, transcript in found
    ]
    seen = {item[4] for item in found if item[4]}
    found.extend(item for item in by_path.values() if item[4] not in seen)
    # Earliest closing transcript first, by git ancestry (Nabu P3 at
    # 79fdcc63: filename order had rebound closures to later rounds).
    found.sort(key=lambda item: (scope_generation(item[3][:7]) if item[3] else 10**9, item[4] or ""))
    return found


def relation_problem(section_name: str, claim: str, transcript: str, section: dict) -> str | None:
    """The builder's section/lane/scope relation for a transcript path, plus
    the tracked-transcript rule (an untracked file is not a filed transcript)."""
    resolved = transcript_path(transcript)
    if resolved is None:
        return f"{transcript} is not a tracked transcript"
    return claim_relation_problem(section_name, claim, resolved, section)


from build_finding_register import closure_head  # noqa: E402


def speaking_lines(path: Path, severity: str, claim: str, section: dict | None = None, section_name: str | None = None) -> list[int]:
    """Closure lines of `severity` that name the claim's finding identity —
    a strong identity when the lane files several findings under the
    claim's kind — and none when the same transcript records the claim OPEN
    on another line."""
    if not path.is_file() or open_lines_about(path, claim, severity, section, section_name):
        return []
    strong = shares_its_kind(section, severity, claim, section_name) if section is not None else False
    shared = shared_vocabulary(section, severity, claim, section_name) if section is not None else set()
    text = path.read_text(encoding="utf-8", errors="replace").splitlines()
    speaking = [n for n in cited_closure_lines(path, severity) if line_speaks_about(text[n - 1], claim, strong=strong, shared=shared)]
    # A line whose own head (before the CLOSED marker) names the finding is
    # that finding's closure; a line naming it only in trailing evidence
    # prose is used only when no head names it (Nabu P3 at 79fdcc63).
    heads = [n for n in speaking if line_speaks_about(closure_head(text[n - 1]), claim, strong=strong, shared=shared)]
    return heads or speaking


def retire(summary: dict, retirements: list[dict]) -> int:
    retired = 0
    for name, section in summary.items():
        if not isinstance(section, dict):
            continue
        lane_closers = closers(section, name)
        for severity in range(5):
            key = f"p{severity}_open"
            entries = section.get(key)
            if not isinstance(entries, list) or not entries:
                continue
            keep: list[str] = []
            closed = list(section.get(f"p{severity}_closed_claims", []))
            for entry in entries:
                verified = None
                binding = None
                tag = TAG.match(entry)
                prefix = tag.group(1) if tag else entry.split(":", 1)[0]
                # The claim's raising scope: its tag, else the transcript that
                # raised it, else its field's note (the builder's rule); a
                # claim with no raising scope never retires automatically.
                scope = raising_scope(name, entry, section)
                for field, lane, severities, closer_scope, transcript in lane_closers:
                    lane_match = lane == role(prefix) if role(prefix) else (
                        lane is None and Path(transcript or "").name.startswith(prefix.removesuffix("_note"))
                    )
                    if (
                        lane_match
                        and f"P{severity}" in severities
                        and transcript
                        and strictly_later(closer_scope, scope)
                    ):
                        lines = speaking_lines(ROOT / transcript, f"P{severity}", entry, section, name)
                        if not lines or relation_problem(name, entry, transcript, section):
                            continue
                        verified = f"{transcript}#L{','.join(map(str, lines))} — {field} at {closer_scope[:8]} verified the carried P{severity} closed"
                        break
                # Exact-claim bindings first: the whole claim string and its
                # lines, the reviewer's per-finding closure line bound by hand
                # where the identity relation cannot see it.
                for item in retirements:
                    for exact in item.get("claims") or []:
                        if item["section"] == name and exact.get("claim") == entry:
                            closure = cited_closure_lines(ROOT / item["verified_by"], f"P{severity}")
                            lines = exact.get("lines") or []
                            relation = relation_problem(name, entry, item["verified_by"], section)
                            if refusing_open_lines(name, entry, ROOT / item["verified_by"], f"P{severity}", True):
                                relation = relation or "the transcript records the claim OPEN"
                            if relation or not lines or not set(lines) <= set(closure):
                                raise SystemExit(
                                    f"exact-claim retirement for {name} P{severity} cites {item['verified_by']}#L{lines} "
                                    f"which records a closure of P{severity} at {closure} :: {entry[:90]}"
                                )
                            verified = f"{item['verified_by']}#L{','.join(map(str, lines))} — {exact.get('reason') or item.get('reason', '')}"
                            binding = "exact-claim"
                for item in retirements:
                    if (
                        binding is None
                        and item["section"] == name
                        and severity in item.get("severities", [])
                        and item.get("prefixes")
                        and entry.startswith(tuple(item["prefixes"]))
                    ):
                        # A prefix names one whole field of the section
                        # (`<field>[@scope]:`, the field or its `_note`
                        # present in the section); a bare-lane or empty
                        # prefix would sweep the section (Vulcan P4 at
                        # 76ae15ab; lane-less fields qualify — Vulcan P3 at
                        # 1a9f0aab). Each matched claim is still bound to
                        # its own speaking line.
                        for prefix in item["prefixes"]:
                            head = re.match(r"^([A-Za-z0-9_.]+)(?:@[0-9a-f]{7,40})?:", prefix)
                            field_name = head.group(1) if head else ""
                            known = field_name in section or field_name.removesuffix("_note") in section or (
                                "." in field_name and isinstance(section.get(field_name.split(".")[0]), dict)
                            )
                            if not head or field_name in REVIEWERS or not known:
                                raise SystemExit(f"retirement for {name}: prefix names no field of the section: {prefix!r}")
                        recorded = speaking_lines(ROOT / item["verified_by"], f"P{severity}", entry, section, name)
                        closure = cited_closure_lines(ROOT / item["verified_by"], f"P{severity}")
                        lines = item.get("lines") or recorded
                        # Every cited line records a closure of the severity, at
                        # least one speaks about the claim, and the transcript
                        # stands in the section/lane/scope relation (the
                        # builder's rules; Vulcan P3 at 1a9f0aab: the explicit
                        # path had checked no relation).
                        relation = relation_problem(name, entry, item["verified_by"], section)
                        if relation or not lines or not set(lines) <= set(closure) or not set(lines) & set(recorded):
                            raise SystemExit(
                                f"retirement for {name} P{severity} cites {item['verified_by']}#L{lines} "
                                f"which records a closure of P{severity} at {closure}, naming the claim only at {recorded} :: {entry[:90]}"
                            )
                        verified = f"{item['verified_by']}#L{','.join(map(str, lines))} — {item['reason']}"
                if verified:
                    record = {"claim": entry, "verified_by": verified}
                    if binding:
                        record["binding"] = binding
                    closed.append(record)
                    retired += 1
                else:
                    keep.append(entry)
            section[key] = keep
            section[f"{key}_count"] = len(keep)
            if closed:
                section[f"p{severity}_closed_claims"] = closed
    return retired


def fold_restatements(summary: dict) -> int:
    """Fold a lane's later re-statement of a carried open finding into the
    claim at the round it names (Nabu/Vulcan/Ariadne P4 at 76ae15ab: carried
    findings were listed once per round). The re-statement moves to
    `pN_restated_claims` as `{claim, restates}`; nothing vanishes and the
    named root stays open.

    One rule, no closure path (d158d26b round: every ambiguity fails
    closed): a named carry folds only into the one OPEN same-key claim at the
    round it names, from a strictly later round. A fold never transfers a
    closure — a carry whose named root is retired stays open, `retire`
    judges it on its own closing lines, and a carry the root's closing round
    could have judged but did not close is a split `--check` refuses until
    an explicit exact-claim binding reconciles it (Nabu P2, Vulcan P3,
    Nabu P3, Vulcan P3 at d158d26b: the exact-key inheritance and the
    retired-root guard had handed one finding's closure to another). Two
    same-key claims at the named round are ambiguous and fold nothing."""
    folded = 0
    for name, section in summary.items():
        if not isinstance(section, dict):
            continue
        for severity in range(5):
            entries = section.get(f"p{severity}_open")
            if not isinstance(entries, list) or not entries:
                continue
            keys = {entry: finding_key(entry, name, section) for entry in entries}
            keep: list[str] = []
            restated = list(section.get(f"p{severity}_restated_claims", []))

            def scope_of(claim: str) -> str | None:
                scope = claim_scope(claim) or raising_scope(name, claim, section)
                return scope[:7] if scope else None

            for entry in entries:
                named_sha = named_carry_sha(entry)
                if keys[entry][0] is None or not is_carry(entry) or not named_sha:
                    keep.append(entry)
                    continue
                matching = list(dict.fromkeys(
                    other for other in entries
                    if other != entry and same_finding(keys[other], keys[entry]) and scope_of(other) == named_sha
                ))
                if len(matching) != 1:
                    keep.append(entry)
                    continue
                root = matching[0]
                later, first = claim_scope(entry), claim_scope(root)
                # Only a strictly later round's re-statement folds; an
                # untagged root is the earliest by construction.
                if later is not None and (first is None or strictly_later(later, first)):
                    restated.append({"claim": entry, "restates": root})
                    folded += 1
                else:
                    keep.append(entry)
            section[f"p{severity}_open"] = keep
            section[f"p{severity}_open_count"] = len(keep)
            if restated:
                section[f"p{severity}_restated_claims"] = restated
    return folded


def reopen_all(summary: dict) -> None:
    """Return every closed claim to its open list (regeneration from scratch;
    the same transcripts must close it again, or it stays open)."""
    for section in summary.values():
        if not isinstance(section, dict):
            continue
        for severity in range(5):
            closed = section.pop(f"p{severity}_closed_claims", None) or []
            restated = section.pop(f"p{severity}_restated_claims", None) or []
            if not closed and not restated:
                continue
            entries = list(section.get(f"p{severity}_open") or [])
            # Re-statements return too (Vulcan P2 at 8f774d0c: a regeneration
            # had kept recorded folds verbatim), so the fold is re-derived.
            for item in closed + restated:
                if item["claim"] not in entries:
                    entries.append(item["claim"])
            section[f"p{severity}_open"] = entries
            section[f"p{severity}_open_count"] = len(entries)


def closer_disagreements(summary: dict) -> list[str]:
    """Live PRIOR fields whose transcript's status lines record severities
    the token does not close (Vulcan P4 at 8966da2e: ten fields widened
    beyond their token with no live effect): reported in `--check`, never
    silent. Report-only — the union still closes, per the b58ac1e0 rule."""
    problems: list[str] = []
    for name, section in summary.items():
        if not isinstance(section, dict):
            continue
        by_path = {item[4]: item for item in transcript_closers(name)}
        for field, value in section.items():
            if not isinstance(value, str) or field.endswith("_note") or role(field) is None:
                continue
            if not PRIOR.search(value):
                continue
            note = note_of(section, field) or ""
            transcript = TRANSCRIPT.search(note)
            path = transcript.group(1) if transcript else None
            if path not in by_path:
                continue
            token_sevs = closed_severities(value)
            extra = sorted(by_path[path][2] - token_sevs)
            if extra:
                problems.append(f"{name}.{field}: token closes {sorted(token_sevs)} but {path} records {','.join(extra)}")
    return problems


def split_status_problems(summary: dict) -> list[str]:
    """Keys listed open in one copy and closed in another (Ariadne P2 at
    8966da2e: 8 exact-key and 19 wildcard pairs had passed `--check`):
    one finding receives one status — a split forces an explicit
    reconciliation, never a silent dual listing. Only an identity counts:
    an identifier-bearing key (an absent identifier is no identity), and
    an open copy the closing transcript could have judged — its scope
    strictly predates the closer's. A copy raised at or after the closing
    round is that round's own statement (8966da2e round, second batch: the
    s20_700 closure lane closed its 8f774d0 P3 and raised a new P3 on the
    same identifier in one transcript)."""
    problems: list[str] = []
    for name, section in summary.items():
        if not isinstance(section, dict):
            continue
        for severity in range(5):
            closed_by_key: dict[tuple, list[str]] = {}
            for item in section.get(f"p{severity}_closed_claims") or []:
                if not isinstance(item, dict):
                    continue
                key = finding_key(item["claim"], name, section)
                closer = transcript_scope(item.get("verified_by", ""))
                if key[3] and closer:
                    closed_by_key.setdefault(key, []).append(closer)
            reported: set[tuple] = set()
            for claim in section.get(f"p{severity}_open") or []:
                key = finding_key(claim, name, section)
                if key not in closed_by_key or key in reported:
                    continue
                scope = claim_scope(claim) or raising_scope(name, claim, section)
                if scope is None or any(strictly_later(closer, scope[:7]) for closer in closed_by_key[key]):
                    reported.add(key)
                    problems.append(f"{name}.p{severity}: one finding open and closed: {key[1]!r} {key[2]!r} {key[3]!r}")
            problems.extend(named_carry_splits(name, section, severity))
    return problems


def named_carry_splits(name: str, section: dict, severity: int) -> list[str]:
    """A named carry and its named root — same lane, kind and file/origin,
    the root raised at the round the carry names, whatever either
    identifier — are one finding by the reviewer's own statement (Ariadne P3
    at d158d26b: 18 named-carry/named-root pairs kept two statuses because
    the identifiers differed or were absent). One open and the other closed
    is a split when the closing transcript strictly postdates the open
    member (it could have judged it); `--check` refuses it until an
    explicit exact-claim binding reconciles it. Detection only: the link
    never folds or closes anything."""
    def scope_of(claim: str) -> str | None:
        scope = claim_scope(claim) or raising_scope(name, claim, section)
        return scope[:7] if scope else None

    open_claims = [c for c in section.get(f"p{severity}_open") or [] if isinstance(c, str)]
    closed = [item for item in section.get(f"p{severity}_closed_claims") or [] if isinstance(item, dict)]
    # A folded carry already shares its open root's status; only open and
    # retired claims are linked here.
    members = [(c, None) for c in open_claims] + [(item["claim"], item) for item in closed]
    problems: list[str] = []
    for carry, carry_item in members:
        sha = named_carry_sha(carry) if is_carry(carry) else None
        if not sha:
            continue
        carry_key = finding_key(carry, name, section)
        at_round = [
            (root, root_item) for root, root_item in members
            if root != carry and scope_of(root) == sha and finding_key(root, name, section)[:3] == carry_key[:3]
        ]
        # The named root: the one claim at that round with the carry's exact
        # key, else the one claim of its lane, kind and file there. Several
        # candidates name no single root, and no link is drawn.
        exact = [pair for pair in at_round if finding_key(pair[0], name, section) == carry_key]
        chosen = exact if exact else at_round
        if len(chosen) != 1:
            continue
        root, root_item = chosen[0]
        if (carry_item is None) == (root_item is None):
            continue
        if carry_key[3] and finding_key(root, name, section) == carry_key:
            continue  # the identity rule above already reports an equal key
        open_member, closed_item = (carry, root_item) if carry_item is None else (root, carry_item)
        closer = transcript_scope(closed_item.get("verified_by", ""))
        opened = scope_of(open_member)
        if closer and opened and strictly_later(closer, opened):
            problems.append(
                f"{name}.p{severity}: named carry and named root split: {carry[:70]!r} / {root[:70]!r}"
            )
    return sorted(set(problems))


def regeneration_divergence(tracked: dict, retirements: list[dict]) -> list[str]:
    """Sections and severities whose closed or re-stated claim set differs
    between the tracked ledger and a from-scratch regeneration (reopen →
    retire → fold), or whose closures cite different transcripts."""
    fresh = json.loads(json.dumps(tracked))
    reopen_all(fresh)
    retire(fresh, retirements)
    fold_restatements(fresh)
    problems: list[str] = []
    for name, section in tracked.items():
        if not isinstance(section, dict):
            continue
        for severity in range(5):
            for key in (f"p{severity}_closed_claims", f"p{severity}_restated_claims"):
                # The full reference (path and `#L` lines): two transcripts,
                # or two line sets, closing one claim are different closures
                # (Nabu P4 at 79fdcc63: the gate had compared paths only).
                recorded = {(item["claim"], item.get("verified_by", item.get("restates", ""))) for item in section.get(key) or []}
                regenerated = {(item["claim"], item.get("verified_by", item.get("restates", ""))) for item in fresh.get(name, {}).get(key) or []}
                if recorded != regenerated:
                    problems.append(f"{name}.{key}: tracked {len(recorded)} vs regenerated {len(regenerated)} ({len(recorded ^ regenerated)} differ)")
    return problems


def replay_problems(summary: dict) -> list[str]:
    """Closed claims whose cited lines no longer record a closure that speaks
    about them, or that are also listed open."""
    problems: list[str] = []
    for name, section in summary.items():
        if not isinstance(section, dict):
            continue
        for severity in range(5):
            open_entries = set(section.get(f"p{severity}_open") or [])
            for item in section.get(f"p{severity}_closed_claims") or []:
                claim, verified_by = item.get("claim", ""), item.get("verified_by", "")
                path = verified_by.split("#")[0]
                cited = re.search(r"#L([0-9,]+)", verified_by)
                wanted = {int(n) for n in cited.group(1).split(",") if n} if cited else set()
                relation = relation_problem(name, claim, path, section) if path else "no transcript"
                if relation:
                    problems.append(f"{name}.p{severity}_closed_claims: {relation} :: {claim[:60]}")
                    continue
                recorded = set(speaking_lines(ROOT / path, f"P{severity}", claim, section, name)) if path else set()
                closure = set(cited_closure_lines(ROOT / path, f"P{severity}")) if path else set()
                if item.get("binding") == "exact-claim":
                    from build_finding_register import exact_claim_binding
                    if (
                        not wanted
                        or not wanted <= closure
                        or not exact_claim_binding(name, claim, ROOT / path, sorted(wanted))
                        or refusing_open_lines(name, claim, ROOT / path, f"P{severity}", True)
                    ):
                        problems.append(f"{name}.p{severity}_closed_claims (exact-claim): {verified_by[:100]} :: {claim[:60]}")
                elif not wanted or not wanted <= closure or not wanted & recorded:
                    problems.append(f"{name}.p{severity}_closed_claims: {verified_by[:100]} :: {claim[:60]}")
                if claim in open_entries:
                    problems.append(f"{name}.p{severity}: open and closed: {claim[:60]}")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if any claim is still retirable")
    parser.add_argument("--regenerate", action="store_true",
                        help="return every closed claim to its open list first, then retire from scratch")
    parser.add_argument("--fold-restatements", action="store_true",
                        help="fold later re-statements of a carried open finding into its earliest claim")
    args = parser.parse_args()
    summary = json.loads(SUMMARY.read_text(encoding="utf-8"))
    # Canonical order (Ariadne P3 at 6589c6ec; Nabu P2 at 8f774d0c: a fold
    # run first had hidden a closure): reopen every closure, retire, then
    # fold the re-statements that no closer closed. `--regenerate`
    # therefore always folds.
    if args.regenerate:
        reopen_all(summary)
    retirements = json.loads(RETIREMENTS.read_text(encoding="utf-8")) if RETIREMENTS.exists() else []
    retired = retire(summary, retirements)
    folded = fold_restatements(summary) if (args.fold_restatements or args.regenerate) else 0
    if args.check:
        # Replay (Vulcan/Nabu P4 at 76ae15ab): every recorded closed claim
        # must still verify against its transcript at the cited lines, and
        # no claim may be listed open and closed at once. Regeneration
        # (Ariadne P3 at 6589c6ec): reopening every closure, folding and
        # retiring from scratch must reproduce the tracked ledger — the same
        # closed and re-stated claim sets per section and severity. Split
        # status (Ariadne P2 at 8966da2e): no key may be open in one copy
        # and closed in another.
        stale = replay_problems(summary)
        divergence = regeneration_divergence(json.loads(SUMMARY.read_text(encoding="utf-8")), retirements)
        splits = split_status_problems(summary)
        closers_note = closer_disagreements(summary)
        result = "PASS" if retired == 0 and not stale and not divergence and not splits else "FAIL"
        print(json.dumps({"retirable": retired, "stale_closures": stale, "regeneration_divergence": divergence, "split_status": splits, "closer_disagreements": closers_note, "result": result}, indent=2))
        return 0 if result == "PASS" else 1
    SUMMARY.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"retired": retired, "folded": folded, "result": "PASS"}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
