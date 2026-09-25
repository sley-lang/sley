# AT-MW-02 consumer acceptance clarification

Status: REVIEWED_ACCEPTED, 2026-09-08. Ariadne and Nabu independently
approved candidate `5e2ff61cc3e6dd6e437881d20ba7458f4896a1b9`. This changes only the
consumer demonstration definition in ENTITY_READ_PROFILE_V2 section 7;
the reviewed runtime and remaining acceptance requirements stay in force.

The retained architecture specification, section 9, explicitly lists local
expression replacement among eight required edit categories. The
architecture audit, AT-MW-02, calls its required demonstration EC1a but
does not define that label. Its ENCODED-table reference points to a section
containing activation and exclusions, and its `measure_mw.py` and raw JSON
are attributed to an unnamed campaign scratch area. Bounded inspection of
the specification, audit, tracked files and architecture review records did
not recover the mapping or a scratch locator. The historical audit and
measurements remain unchanged; no historical EC1a reproduction is claimed.

The active profile prospectively requires a bounded local expression
replacement and a parameter-type-dependent signature edit. Each uses the
actual runner/stdio candidate pipeline. The expression edit must preserve
response-derived existing facts across varied fixtures, and an independent
judgment checks the changed and preserved fields. These conditions test
AT-MW-02's operative requirement: the protocol supplies the current typed
facts needed for a canonical edit within bounded root and session context.
They do not claim coverage of all eight edit categories or model benchmark
success. No store dump or fixture-provided current body may enter the agent
context.

Source anchors: `SLEY-2.0-ARCHITECTURE-TIGHTENING.md` section 9 in the
canonical machineresearch authority; this repository's
`docs/audits/SLEY-2.0-ARCHITECTURE-TIGHTENING-AUDIT.md` sections 2, 3.7 and 4;
retained Machine Genesis section 8.2. The integrator retains the bounded
search and source-hash receipt as `ec1a-source-resolution.json` in the
private AT-MW-02 evidence directory. The amendment authorizes its defined
consumer implementation; successful demonstration evidence remains required.
