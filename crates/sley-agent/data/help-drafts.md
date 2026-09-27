# Drafts and repair

Every `try` records a draft revision, `d1@r1`, and keeps it even when the
frame does not parse, expand or validate. `d1` means its latest revision.

    sley-agent draft                       list drafts
    sley-agent draft d1                    state, candidate, tests, obligations
    sley-agent draft d1 --obligations      every open obligation
    sley-agent draft d1 --expanded         the plain AF1 derived from an AF1-X frame
    sley-agent draft d1@r1 --input         the exact text given
    sley-agent try --on d1 more.json       layer a frame on d1: next revision
    sley-agent fill d1 fix.json --revision 2   replace subtrees of revision 2
    sley-agent submit d1                   submit d1's latest revision

A delta is `{"set": [{"at": pointer, "value": subtree}, ...]}`: each `at`
is a JSON pointer that exists in that revision's frame (`""` replaces the
whole frame), and each `value` replaces that subtree. Targets may not
repeat or contain each other; the replacements apply together or not at
all. To insert into or delete from an array, replace the array. A frame
that did not parse can only be replaced whole.

`fill` needs the latest revision number (`AGENT_DRAFT_STALE` otherwise).
When the accepted head has changed since the revision, `fill` and
`try --on` refuse with `AGENT_DRAFT_HEAD_CHANGED` unless `--rebase` is
given. `submit d1` submits only a Valid candidate of that revision
(`AGENT_DRAFT_INCOMPLETE` otherwise); an older revision is never used
instead, but `submit d1@r2` names one explicitly.
