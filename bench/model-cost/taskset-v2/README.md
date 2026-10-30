# Task set v2 (2.2 measurement, item 0): design, to be pre-registered

Status (30 Oct 2026): rules fixed and the 24 task statements authored
(`authored.json`, unedited). No oracle, hidden cases or reference programs
exist yet, the set is not hashed into `prereg.json`, and no model call has
used v2. Sley's 2.2 list work (#88, #89) landed before these rules were
written; the audit asked for the reverse order, and this is recorded here so
v2 results are read with that in mind.

## Why v1 is not enough (audit by Claude Code, 30 Oct 2026)

- v1 is integer-only: every task returns one integer (or an error) from ints
  and lists of ints. Agent code returns lists, counts things and handles text.
- v1's "held-out" tasks are spent. count_leq and clamp_sum were held out, and
  the #83 card lines targeted exactly those shapes (count_leq's thinking fell
  from 243 to 22 tokens per call). v1 has no held-out group any more.
- 18 tasks give a Sley/Python ratio CI half-width of about 0.15 to 0.17
  (#81: 1.04 to 1.41; #84: 1.10 to 1.45). The 2.2 gate is a half-width of 0.07
  or less.

## Rules

1. **Size.** At least 24 tasks. Full tier: 24 x 6 languages x 3 reps = 432
   trials per model
   (about $0.10 billed for Haiku 5.5 on the raw API; v1's 324 trials cost
   $0.052), plus the 6 sealed held-out tasks (rule 7).
2. **Mix.** At least a third (8 or more) return a list, need string handling or
   need counting by key (what Python writes with `.append`, `Counter`, `dict`
   or `str` methods). The rest are integer tasks as in v1. Up to 4 are edits of
   an existing file.
3. **Independent author.** Task statements are written by someone who has not
   seen the Sley card, its "instead of" lines, the 2.2 idiom list or the v1
   per-task costs: a fresh Claude Code session (`claude -p`, Opus 5.5, one
   turn, tools off, an empty directory, no access to the sley repository) given
   only `prompt.md`. `authoring.json` records the call. The output is committed
   unedited; any later fix (a wrong example, an ambiguous statement) is a dated
   amendment listed below, never a silent edit.
4. **Oracle first.** Each task gets a Python reference oracle and 128 seeded
   cases (seed 218, edge cases first) before any model sees it, and reference
   programs in all six languages that agree with the oracle (`verify_refs.py`).
5. **Hash before running.** `prereg.json` records the SHA-256 of `tasks.py`
   (v2), the card and this file. A task changed after the hash is an
   amendment, and the run reports both.
6. **Python arm first.** One Python-only pass (about $0.02) shows which idioms
   occur naturally before any Sley run. Its idiom counts are published.
7. **Held out for real.** The 24 tasks here are public, so none of them can be
   held out. 6 more tasks come from a second, separate authoring call with the
   same prompt, are kept off the repository (on greyarch), and only their
   SHA-256 is committed until the 2.2 candidate is frozen; nobody working on
   the card or the dialect reads them before then.
8. **Same-run arms.** A/B comparisons (card variants, dialect builds) run
   interleaved in one run, never as separate before and after runs.
9. **Kill criterion (from the audit).** If Sley's success rate on v2 is 10 or
   more points below Python's, the headline becomes success rate, and cost
   parity stops being the frame.

## Harness work this needs

- Return kinds beyond `int`: `list` (list of ints) and `str`; parameter kind
  `str`. Each language's signature, test driver and oracle driver in
  `langs.py` gain those kinds (C: out-pointer plus length; Rust `Vec<i64>` /
  `String`; Go `[]int64` / `string`; TypeScript `number[]` / `string`).
- Sley: a list return is refused by the 2.2 dialect today (a list local is
  never a value). v2 measures that honestly: such trials fail unless 2.2
  ships list returns, and the run reports them.
- `tasks.py` keeps v1 unchanged (`TASKS_V1`), adds `TASKS_V2`, and `run.py`
  takes `--task-set v1|v2` so the v1 evidence stays reproducible.

## The authored set

24 tasks (`authored.json`): 11 return an int, 8 a string and 5 a list; 11
have error inputs; parameters are 11 strings, 11 lists and 9 ints. The mix
rule (8 or more returning a list or string, or counting by key) holds with 13.
Three tasks resemble v1 tasks (`second_largest_distinct`,
`count_pairs_with_sum`, `bracket_depth`); they stay, since the author never saw
v1.

Next, in order: a Python oracle per task with 128 seeded cases (any example
that disagrees with the oracle is an amendment), harness kinds `str` and list
returns, reference programs in six languages (`verify_refs.py`), the 6
sealed held-out tasks authored and hashed, the `prereg.json` hash, then the
Python-only pass.

## Amendments

None yet.

## Open decisions for Magister

- Whether a Claude Code session is independent enough as the author, or a
  person should write or review the statements before the hash.
- Whether v2 adds `str` now or only `list` returns (smaller harness change).
