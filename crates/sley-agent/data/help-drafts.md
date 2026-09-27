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

`import --on d1` replaces a draft test of the same name only when it is an
unchanged earlier import; it never replaces a test you wrote or changed.

A follow-up that is not JSON, or that cannot be layered, is kept as the
next revision; `fill` repairs it and layers it on its base again. A
follow-up refused before layering (its base is a text or unlayered
revision, or the head changed) is not recorded: the refusal says so and
how to send it again.
Obligations point at pointers that exist in the revision's frame.

`fill` needs the latest revision number (`AGENT_DRAFT_STALE` otherwise).
When the accepted head has changed since the revision, `fill` and
`try --on` refuse with `AGENT_DRAFT_HEAD_CHANGED` unless `--rebase` is
given. `submit d1` submits only a Valid candidate of that revision
(`AGENT_DRAFT_INCOMPLETE` otherwise); an older revision is never used
instead, but `submit d1@r2` names one explicitly. A bare `submit` takes
the revision recorded last, over all drafts, and only when it is valid.

`tests: X/Y passed [authored A, imported I, provided P]` counts every
TestCase that ran once, by where its entry comes from. A live test the
frame changes counts where its new entry comes from (`replaces provided`
names it); a live test a ripple `arity` intent restates stays provided.
