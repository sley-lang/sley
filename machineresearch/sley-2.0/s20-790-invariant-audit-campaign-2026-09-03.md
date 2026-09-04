# Cross-cutting invariant audit

Date: 2026-09-03. Integrator: Claude (front line). Council reviews pending.

## Why

Every package checker verifies its own contract. Nothing verified the
properties that hold *across* packages, so a statement one contract made about
another could be false without any gate noticing. Five defects were found this
way, each in a public interface, and each now has a gate.

## Method

Two questions, applied to the whole tree:

1. **Does a stated invariant's check cover what the statement claims?**
   `IDENTIFIERS_V1.md` said the registry "carries all thirty-five domains the
   implementation derives" and that a checker "compares the two on every
   `make quick`". The checker read one file.
2. **What did a later package change that an earlier contract still describes
   the old way?** The extended VM profile added a second cache profile; the
   S20-290 report envelope still said other profiles fail closed.

## Findings

| # | Defect | Why it mattered | Fix |
|---:|---|---|---|
| 1 | The execution report envelope bound no profile | A rejected result carries no cache key, so one request rejected under both profiles derived one identity while the envelope asserted a single profile | The envelope binds `lowering_profile`; `REPORT_ENVELOPE_PROFILE_V1.md` revision 2 |
| 2 | Fourteen validator source symbols named in no document | A consumer reading `source_symbol` had nothing to resolve | `CANDIDATE_RESULT_V1.md` section 8.1, pinned both ways |
| 3 | `GRAPH_RESOURCE_LIMIT` carried the S20-220 owner's code 22020, whose table names it `CFG_RESOURCE_LIMIT` | One number, two symbols; the symbol was registered only by a threat-register row, not by any contract | The owner's symbol is preserved, and registration now requires `docs/spec/` |
| 4 | `CAP_EFFECT_MISMATCH` reported two retryabilities | A program-side failure borrowed the S20-380 token authority's code, so a consumer got contradictory retry guidance for one symbol | `CAPABILITY_REQUIREMENT_EFFECT_UNRESOLVED`, and one retryability per symbol is gated |
| 5 | Fifteen hash domains outside the registry that claimed completeness | Domain separation is a security property, and the registry is where a reviewer checks it | All fifty registered; the drift check reads every crate |

| 6 | `conformance/candidate-result/v1` carried no `SHA256SUMS` | It was the one corpus of nineteen whose bytes an independent party could not verify without running the generator | The generator emits the manifest, and the conformance report refuses a corpus without one |

| 7 | The adapter-responses fuzz slice checker demanded a sentence no document carried | The slice failed whenever anyone ran it, and it runs in no aggregate target, so nothing noticed | The evidence-gaps document states the absent persistent targets, and `make persistent-fuzz-all` now runs all nineteen slices in one command |

| 8 | The oracle independence scan read `oracle/scb1/src` only | Thirteen of the nineteen independent oracles live under `scripts/`, so the check that vouches for their independence never read twelve of them | Both the checker and the conformance report scan every oracle the coverage map names, and the report labels the runner that actually runs |

| 9 | Twelve named fairness invariants connected to nothing | The seventeen run-freeze controls do enforce them, but the mapping existed only in a reader's head, and an invariant with no control is a claim nothing keeps | Each invariant names its enforcing controls, and the baseline checker binds the two sets in both directions |

A tenth, smaller correction: a JSON bridge test paired the restricted query
profile's code 31004 with an invented symbol.

The first full run of all nineteen persistent slices took three minutes forty
seconds and produced no artifact.

## Gates added

- `scripts/check_error_symbol_registration.py`: every emitted symbol is
  assigned by a contract under `docs/spec/`, one numeric code carries one
  symbol, and every symbol is exercised by a test, corpus, fuzz target, or
  oracle, by its string or by its enum variant. 329 symbols, 302 numeric
  codes, none unregistered, ambiguous, or unexercised.
- `scripts/check_declared_limits.py`: every public `MAX_*` value appears in a
  contract, because a decoder limit that decides validity is an epoch-frozen
  fact. 151 limits, none undocumented.
- `make lint`: the workspace configured `clippy::all` and `clippy::pedantic`
  as warnings and no target enforced them; thirty-eight had accumulated.
- The candidate result checker compares the documented source symbols with the
  implementation's literals in both directions and requires one retryability
  per symbol.
- The S20-770 registry drift check reads every crate.

## What this campaign does not claim

It is not the independent security review, and a mechanical property is not a
mitigation. Every finding above was found by reading contracts against code;
none came from a specialist review, which remains pending on Council access.
