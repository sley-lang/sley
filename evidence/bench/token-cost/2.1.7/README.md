# Token-cost records for the Sley 2.1.7 release notes

The `bench/token-cost` harness run on the sley-agent from the 2.1.7 release archive over 14 tasks:
the 9 tasks of the 2.1.5 notes plus five new ones (tak, triples, digitsum, isqrt, inversions),
whose sources and test cases are in `tasks/`. Forms: Python with pytest, the Sley Python dialect
with inline tests, C, Rust, Go and TypeScript. `loop.json` has one row per task, form, scenario
and tokenizer; `transcripts/` has every command and output verbatim. See
docs/release/SLEY-2.1.7.md.
