# Contributing to Sley 2

Sley 2 is developed by Greyforge Labs and published at
<https://github.com/sley-lang/sley> under the Apache License 2.0. Issues and
pull requests are welcome there. Contribution does not grant push, merge,
deploy, tag, release, publication, provider-spend, or public claim authority;
those stay with the maintainers.

## Slice contract

Each change must name one unblocked work package, inspect its governing spec and
invariants, classify reused ideas as copied/adapted/reimplemented/new, and add
contracts, positive and negative tests, property or fuzz coverage where the
boundary warrants it, documentation, and evidence in the same slice.

Importing a Sley 1.x implementation pattern requires a prior ADR and disposition
entry. Legacy code may be executed as an external oracle; it may not be copied
into the new kernel.

## Validation economy

Run the smallest meaningful check first: the targeted `cargo test -p <crate>`
for the crate you changed, then the `scripts/check_*.py` checkers that cover
the files you touched. Before you open a pull request, run the combined gate
from the repository root:

```sh
make contributor-check
```

It runs formatting, strict workspace Clippy, all workspace tests,
conformance, adversarial tests and bounded fuzz smoke sequentially and stops
on failure. Install the pinned Rust toolchain with rustfmt and Clippy,
Python 3.12+ and `uv`; the first run may download locked dependencies.
Only tracked fixtures are required: no master-goal document, generated
release evidence, credentials or native-supervisor installation. Build/cache
files are ignored and tracked evidence is left unchanged. This gate validates
source; it does not certify release artifacts or native host enforcement.

For targeted iteration, the existing `make conformance`, `make adversarial`
and `make fuzz-smoke` targets remain available. Maintainers use `make lint`
to record lint evidence; it rewrites `evidence/build/lint-report.json`, unlike
`make contributor-check`. Leave that record out of your pull request unless
the maintainers ask for it. The
[Quickstart](docs/QUICKSTART.md#6-run-the-test-gates) has the details.

`make quick` and `make check-changed` are the maintainers' gates. They don't
pass on a fresh clone: some checkers read the outputs of a local release
build under `evidence/runtime/`, which isn't tracked, and two read the Sley
2.0 master goal, which lives outside the repository (`SLEY2_MASTER_GOAL`
points at it). `make v2` is a placeholder for the full product gate and fails
closed with `NOT_IMPLEMENTED` until that gate exists.

Say in the pull request which checks you ran and what they returned. Never
weaken a check or a test to make a change pass.

## Commit discipline

Use one coherent purpose and a conventional commit message. Do not add
`Co-Authored-By`. Do not amend, rebase, force-push, or absorb unrelated
changes without explicit direction.

## Prohibited additions

Do not add Sley source syntax, parser, canonical text, formatter, Tree-sitter,
conventional LSP, REPL, human projection, source compatibility, unrestricted
shell, mandatory Greyforge dependency, native backend, marketplace, or self-
hosting work to the 2.0 GA path.

Self-hosting toolchain work adopted under REWEAVE-1.0 (SH2 campaign) is
governed by host-boundary.json, BOOTSTRAP_PROFILE_2 (the frozen successor of
BOOTSTRAP_PROFILE_1), and the staged SH2 gates
instead of this paragraph. Every other prohibition in this paragraph is
unchanged.

An optional, noncanonical authoring frontend adopted under ADR-0055 is
governed by that ADR instead of the words "Sley source syntax" and "parser"
in the first paragraph of this section: it may accept textual proposals that
lower deterministically through the ordinary typed candidate and validation
path, and its syntax, files, formatting and parser never become program
authority or a prerequisite of the graph-native lifecycle. Every other
prohibition in that paragraph is unchanged.
