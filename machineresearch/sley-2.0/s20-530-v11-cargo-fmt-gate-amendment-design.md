# S20-530 v11 cargo fmt gate amendment

Status: candidate contract amendment

## Problem

The first captured v10 closeout execution (HEAD `d0dc478`, 2026-09-01T23:56Z
to 2026-09-02T01:12Z) passed source binding, all three test-list commands, and
Tier 2 commands 1 through 24: every mapped test passed and the independent
reconciler reproduced the frozen partition counts. It failed closed on the
25th and last frozen Tier 2 command, `cargo fmt --all -- --check`, with
`error: no such command: fmt` (exit 101). The frozen execution environment
pins `PATH` to the isolated tool directory, which provisions `cargo`, `rustc`,
and `rustdoc` but neither the `cargo-fmt` external subcommand nor `rustfmt`.
Like cor07's `mkfifo` in v10, the command had never executed inside the
captured environment because every earlier closeout stopped before test
execution.

Behind the provisioning gap sits a second defect. A developer-shell
`cargo fmt --all -- --check` with the pinned toolchain (1.93.0, rustfmt
1.8.0-stable) reports 1929 formatting hunks, all of them inside the
`#[cfg(test)] mod tests` modules of the three S20-530 owner sources
(`crates/sley-repo/src/refs.rs` 1174, `crates/sley-txn/src/repository.rs` 702,
`crates/sley-store/src/lib.rs` 53) and none anywhere else: every production
item, every test-only item outside those modules, and every other crate is
already rustfmt-clean. The identical hunks exist at the pre-v10 commit
`424126e`. The repository `Makefile` never runs `cargo fmt`; only the S20-530
Tier 2 list carries it.

## Decision

V11 changes nine things and nothing else.

1. Tool provisioning. `execution_tools()` in the runner resolves `cargo-fmt`
   and `rustfmt` through the same pinned `rustup which` path as `cargo`,
   `rustc`, and `rustdoc`, records them in the tool manifest between `rustdoc`
   and `python3`, and `create_tool_bin()` links both into the isolated tool
   directory. The checker's frozen `EXECUTION_TOOL_NAMES` names them in the
   same position. `cargo fmt` resolves `cargo-fmt` from the pinned `PATH`, and
   `cargo-fmt` resolves `rustfmt` from the same `PATH`; no environment
   variable is added.
2. Formatter exemption for checker-rendered test modules. Each of the three
   owner `mod tests` modules gains exactly one attribute line,
   `#[rustfmt::skip]`, directly after its `#[cfg(test)]`. The bodies of these
   modules are not free-form code: the checker renders and byte-verifies
   their statement text (recovery-provenance assertions, grouped multifault
   bodies, fixture plans, snapshot and corruption helpers, exact limit
   probes), the plan binds every mapped test body by digest, and the frozen
   exception partition binds every source byte. Two authorities cannot own the
   layout of the same bytes; the checker keeps it. `cargo fmt --all -- --check`
   passes on the result with zero hunks, and it still governs all production
   code, all test-only items outside the modules, and every other crate.
3. Checker recognition of the attribute chain. `exact_test_module_range`
   accepts the whitespace-stripped attribute chain from a frozen two-element
   set, `#[cfg(test)]` and `#[cfg(test)]#[rustfmt::skip]`, instead of the
   single literal. No other scanner, projection, or authority rule changes.
4. Implementation drift repair found while preparing v11. The checker froze
   the private transaction bridge `recovery_receipt_metadata` in the
   match-guard form on 2026-08-28 (`39a6296`); the implementation commit of
   2026-08-29 (`691e870`) rewrote the same logic as an `if` inside the `Ok`
   arm. Both map a symlink or non-file to `TransactionErrorCode::Io`,
   `NotFound` to `RecoveryReceiptIncomplete`, and any other error through
   `into()`. `require_public_recovery_api` compares token-normalized bodies
   and runs only on the implementation-complete path, which no closeout had
   reached, so the drift stayed latent and would have failed the final checker
   after a passing closeout. V11 restores the source body to the frozen form;
   the checker expectation does not change.
5. Checker input defect on the same latent path. `require_public_recovery_api`
   handed `recovery_ancestry_error_traits_problem` the `production_rust`
   projection, which blanks string-literal contents, so the frozen
   `Display::fmt` expectation containing the three message literals could
   never match any source. The check now receives the byte-preserving
   `normal_build_source` projection of the same file (test-only items removed,
   literals intact), and the unchanged production body matches the unchanged
   expectation token for token. No expectation text changes.
6. Checker signature pattern on the same latent path. The frozen regex for the
   public `recover_gc_witness` API demanded the spelling
   `Result<GcWitnessRecoveryStatus, GcError>`, but `gc.rs` has always written
   `Result<GcWitnessRecoveryStatus>` through its module alias
   `type Result<T> = core::result::Result<T, GcError>;`, the same type. The
   public API baseline is anchored at the freeze commit with the alias
   spelling, so respelling the source would break that anchor. The checker
   now requires the exact alias declaration at top level and accepts the
   return type with or without the explicit `GcError` argument; the spec's
   spelled signature remains the exact type.
7. Checker contradiction on the same latent path. `limit_events_problem`
   rejected every unresolved control entry (scope and dominating-exit values,
   callables, and edge liveness, plus collection mutations), yet the v5 to v8
   exception partition deliberately records exactly those static-resolver
   frontier cases as reviewed manual exceptions: the committed v10 plan
   carries 1,666 ledgered control exceptions across those seven atom kinds
   (914 exit authorities, 500 exit values, 96 scope authorities, 95 scope
   values, 31 exit liveness, 16 collection mutations, 14 scope liveness),
   every one matched to its manifest entry through `unresolved_record_sha256`,
   so the strict form could never pass on the real manifest. The unchanged
   main tree fails the same check. `limit_events_problem` now derives the
   control exception ledger from the same manifest and accepts an unresolved
   entry only when its canonical digest is one of those ledgered
   `UNRESOLVED_BY_V8_STATIC_RESOLVER` records, and only after the derived
   control ledger has reproduced the frozen reviewed `control_ledger_sha256`
   byte for byte; any other source, including every hostile negative
   control, receives the strict rule. The first draft of this change
   derived the allowlist from the manifest under test without that digest
   binding, and the main-tree self-test `checker self-test accepted a
   cross-crate qualified constant: constant-false` caught it before review.
8. Checker renderer on the same latent path. `expected_private_enum_body`
   rendered each payload variant of the four test-only durability-cut enums
   as `Name { field: Type, }`, with a trailing comma inside the braces, while
   every owner source writes the rustfmt single-line form
   `Name { field: Type }` (rustfmt removes that comma, and these enums sit
   outside the exempt test modules). The token-normalized comparison in
   `require_test_only_durability_cut_authority` therefore could never pass on
   the real sources. The renderer now emits the single-line form; the frozen
   variant tables and the source enums are unchanged.
9. Checker literal inconsistency on the same latent path. For every LIMIT
   row subcase the frozen-default assertion is checked twice:
   `require_exact_limit_default_assertion` demands the Rust
   underscore-grouped literal (`65_792`), which the mapped tests write, while
   the generic `exact_value_assertion_problem` demanded the JSON spelling
   (`65792`), so the same assertion could never satisfy both. The generic check
   now also accepts the underscore-grouped integer literal; string and
   boolean expectations are unchanged.

## Rejected alternatives

- Reformatting the three test modules with the pinned rustfmt: measured at
  44,251 insertions and 11,038 deletions; rustfmt inserts trailing commas and
  reorders imports, so the token streams change and the checker's exact
  renderings (snapshot type aliases, grouped M2 adapter signatures, and
  recovery-provenance assertion statements were the first three to fail)
  would all have to be rewritten in rustfmt's shape. That converts a
  formatting repair into a broad checker rewrite with no semantic gain.
- A `rustfmt.toml` that accepts the committed layout: measured candidates
  (`max_width` 110 or 120, `use_small_heuristics = "Max"`, `style_edition =
  "2021"`, per-width overrides) each leave 1,997 to 4,270 hunks and spread new
  diffs into every other crate. `ignore` and `trailing_comma` are unstable
  options unavailable on the pinned stable toolchain.
- Removing or waiving `cargo fmt --all -- --check` from the frozen Tier 2
  command list: weakens the frozen gate and requires operator approval that
  the resume instruction does not carry.
- Provisioning the tools without the exemption: the command still fails.

## Re-freeze scope

The three added attribute lines and the restored bridge body rebind the positional scanner parity (length
plus 17 bytes and code mask; literal counts and literal value digests are
unchanged) for the three owner sources, the limit source-set digest, the
scanner contract digest, every exception record whose `source_sha256` binds
one of the three files, both exception-ledger digests, and the ordered
exception-partition fingerprint. All entry and control counts, complete
manifests, inventories, static-pass sets, event ordering, and non-binding
record fields are expected to remain unchanged; the exact changed and
unchanged fields are recorded in the v11 freeze evidence. The runner digest
changes for the tool manifest and the checker for the tool names, the
attribute-chain set, the traits-check input, the GC-witness signature pattern with its alias check,
the ledgered-unresolved reconciliation of `limit_events_problem`, the
durability-cut enum renderer, and the grouped-integer literal acceptance in the
generic semantic assertion check. V11 refreezes the specification, ADR, checker, runner,
aggregate contract-set digest, freeze evidence, and test-plan binding. The
immutable v8, v9, and v10 freeze evidence and receipts remain byte-for-byte
unchanged as historical authority. Fresh Nabu, Ariadne, and Vulcan freeze
receipts are required, followed by a captured Tier 2 closeout and fresh
implementation receipts.

## Deferred, out of scope

The owner-crate test builds emit pre-existing rustc warnings (unused
variables, dead test helpers, non-snake-case grouped test names; 53 in the
`sley-store` lib test build alone). They predate v11, are not part of the
frozen Tier 2 gate, and are left untouched so this amendment stays a
provisioning and exemption repair; they are recorded as follow-up work.
