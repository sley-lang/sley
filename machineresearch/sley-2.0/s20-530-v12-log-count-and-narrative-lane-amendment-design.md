# S20-530 v12 log-count and narrative-lane amendment

Status: candidate contract amendment

## Problem

The first captured closeout under the reviewed v11 contract (HEAD `de16207`,
2026-09-02T08:11Z to 09:26Z) passed every gate: source binding, three
test-list commands, all 25 Tier 2 commands including `cargo fmt --all --
--check`, 419 mapped tests, 28 retained logs. The implementation reviews that
followed found two contradictions inside the frozen contract that only an
accepted closeout could expose.

1. The specification's workspace-input closure paragraph declares "the exact
   twenty-seven runner-named S20-530 command logs" while the checker's
   `TEST_LIST_COMMANDS` (3) plus `TIER_2_COMMANDS` (25), the runner, and the
   captured evidence bind twenty-eight. The 25th Tier 2 command, the
   formatter gate, was added without updating that sentence. The spec digest
   is part of the contract set, so the evidence cannot exactly satisfy v11.
2. With `implementation_complete` true, `execution_binding_problem` (through
   `require_execution_evidence`) requires that `git diff <validated>..HEAD`
   touch only the three output paths and that the working tree carry no other
   change. The S20-530 checker runs inside `make quick`. The package's own
   completion steps, the stop checkpoint, the resume record, and the
   work-package row, are tracked non-output files, so the frozen v11 rule made
   completing the package and keeping the Tier 1 gate green mutually
   exclusive, and would re-open the gate on every later commit anywhere in the
   repository.

## Decision

V12 changes two things and nothing else.

1. The specification says twenty-eight runner-named command logs. No checker,
   runner, or matrix change is needed; the count was already 28 everywhere
   else.
2. Post-validation change tolerance for narrative lanes. `non_output_changes_since`
   and the current-versus-validated non-output comparison ignore paths under
   `machineresearch/` and the file `docs/WORK_PACKAGES.md`
   (`POST_VALIDATION_NARRATIVE_LANES`). Every other tracked path stays bound:
   crates, scripts, `Cargo.toml`, `Cargo.lock`, `docs/spec`, `docs/adr`,
   conformance vectors, oracle, bench, and the frozen evidence. The narrative
   lanes carry no validation authority: the checker requires only two literal
   substrings in `docs/WORK_PACKAGES.md`, which remain required, and reads
   nothing else from them. The runner still binds the complete closure at
   validation time, the working tree must still be clean, the validated commit
   must still be an ancestor of HEAD, and the recorded workspace inputs must
   still equal the validated commit's blobs.

## Rejected alternatives

- Accepting v11 as is: the frozen spec would state a false log count, and the
  first narrative commit after acceptance would turn `make quick` red.
- Treating narrative documents as validation outputs: outputs are exact paths
  written by the runner and are exempt from dirtiness and from the closure;
  documents are neither runner outputs nor safe to leave uncommitted.
- Re-running the closeout after every narrative commit: about 75 minutes per
  documentation change, and it does not solve later development commits.

## Re-freeze scope

The spec and ADR gain v12 paragraphs and the spec's log-count sentence changes;
the checker's freeze identity moves to v12 together with the narrative-lane
constant and the two exemptions; the frozen spec, ADR, and checker digests,
the aggregate contract set, the freeze evidence, and the test-plan binding are
rebound. Sources, runner, reconciler, scanner parity, and the exception
partition (fingerprint `867bd1d9…`) are unchanged. The immutable v8 through v11
freeze evidence and receipts remain historical authority. Fresh Nabu,
Ariadne, and Vulcan freeze receipts are required, followed by one captured
Tier 2 closeout and fresh implementation receipts.
