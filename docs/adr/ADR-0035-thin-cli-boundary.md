# ADR-0035: the CLI as a transport endpoint with no semantics

Status: proposed; the S20-430 contract is a draft at revision 10 with
Council review pending; implemented at `crates/sley-cli` (2026-09-03)
with endpoint tests over a trusted genesis repository and the mechanical
rule audit in `make quick`. Revision 5 record (2026-09-08): command
defaults, version/report shapes, and legacy behavior are unchanged; the
prospective `--protocol-profile v2-capable` / `--expected-version`
surface with additive `sley2-cli-v2` / `sley2-cli-report-v2` contracts is
declared pending, not implemented; capable CLI runtime is phase 3.
Revision 6 record (2026-09-09): the section 9 version-aware surface is
implemented (profile flag, expected-version frame rule, capable
metadata/report, version-aware serve through `Server::new_versioned`)
with legacy defaults, v1 shapes, and legacy behavior unchanged;
the capable CLI runtime is implemented in revision 6 under the phase-3 slice.
Revision 7 record (2026-09-14): composition pin move to bridge revision 9.
Revision 8 record (2026-09-14): composition pin move to bridge revision 10;
section 8 states the end-of-input rule for every bridge ceiling; the
cited gate tests derive their revision from the checker and run under
`make quick`.
Revision 9 record (2026-09-23): composition pin move to SMP1 revision 14
and bridge revision 11 (it replaced an in-place revision 8 note that had
pinned SMP1 revision 13 without a CLI revision); through the composed
server a legacy serve now fails a non-empty 201 body.
Revision 10 record (2026-09-23): admits the shipped version 3 capable
surface (`v3-capable`, `[1,2,3]`, `sley2-cli-v3`, `sley2-cli-report-v3`)
and, as decision 8, the private native-test worker entry; re-pins SMP1
revision 15 and bridge revision 12.

Date: 2026-09-03; revision 5 record 2026-09-08; revision 6 record 2026-09-09; revision 7 and 8 records 2026-09-14; revision 9 and 10 records 2026-09-23

## Context

The master goal names `sley-cli` a thin machine-oriented command wrapper
that the semantic kernel never imports and that contains no private
validation rules (sections 14.2, 14.3, 22.6). The S20-410 server answers
frames deterministically, S20-440 defines batch admission, and S20-420
renders frames as JSON. A CLI that parsed, validated, or interpreted
anything itself would be a second semantic authority.

The Council lanes were still unavailable at this draft (see ADR-0026); the
design is the integrator's and is submitted to Ariadne, Nabu, and Vulcan as
soon as a lane returns.

## Decision

1. **Endpoint only.** `sley serve` moves frames between standard input,
   standard output, and `Server::answer` or `answer_batch`; the only
   frames it builds itself are its hello and the negotiation-failure
   response, both through the frozen codec.
2. **Two orderings, both explicit.** Per-frame answering is the default;
   `--batch` reads to end of input and answers together so S20-440
   cancellation can precede execution. Neither mode reorders or drops.
3. **Two representations, one canonical.** Byte mode hands the codec's
   frames through unchanged; JSON mode converts each line with the bridge
   and answers bridge rejections as failure frames.
4. **Counting report.** The report copies counts and codes and interprets
   nothing.
5. **Four codes, four exit statuses.** `CLI_*` codes 43000 through 43003
   name only the endpoint's own failures; a failed answer is never a CLI
   failure.
6. **Mechanical rule audit.** `scripts/check_cli_rules.py` fails closed on
   any kernel dependency, reverse dependency, hand-built failure or frame
   literal, judgment function, or method match arm in the CLI source.
7. **Staging.** `scripts/check_cli_contract.py` binds the contract, ADR,
   work-package row, and summary section, and fails closed if the CLI crate
   appears before the summary allows it.
8. **One bounded exception: the native-test worker entry (revision 10).**
   `sley __native-test-worker <input_path>` lets the native test
   supervisor run its worker from the installed binary. It is not a user
   command or a protocol method; it writes raw refusal words and exits with
   the worker's own statuses (1, 6, 7, 8), disjoint from decision 5. It is
   the only reason `sley-cli` depends on `sley-test-runner` (and so links
   `sley-vm`); the rule audit admits exactly one worker call and one
   command word. Moving the worker to its own binary would remove the
   exception; revision 10 records the shipped arrangement instead of
   moving it.

## Consequences

- Every judgment a script observes through the CLI is the server's, so a
  CLI trace is protocol evidence.
- The benchmark harness (S20-600 series) can drive Sley 2 through one
  endpoint without a second code path.
