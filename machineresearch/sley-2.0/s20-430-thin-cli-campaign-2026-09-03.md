# S20-430 Thin CLI Campaign (2026-09-03)

Status: contract draft revision 1 written by the integrator with every
Council lane unavailable; Ariadne contract review, Nabu architecture
review, and Vulcan surface review queued.

## Frontier at start

- The S20-410 server answers frames deterministically (`Server::answer`,
  `answer_batch` with S20-440 cancellation), S20-330 sessions are
  implemented, and the S20-420 bridge renders and parses frames as JSON.
- The master goal names `sley-cli` a thin machine-oriented command wrapper
  with no private validation rules, never imported by the kernel.
- The local completion frontier names S20-430 as the next authority-safe
  package and its guard blocks `crates/sley-cli` until the summary allows
  it.

## Design brief

Contract: `docs/spec/SLEY_CLI_V1.md`, ADR-0035, stage checker
`scripts/check_cli_contract.py`, rule audit `scripts/check_cli_rules.py`
(in `make quick`).

- `sley serve` is a transport endpoint over standard input and output in
  byte or JSON form, per-frame or `--batch`, over one repository path; it
  builds only its hello and the negotiation-failure response.
- A counting report and four `CLI_*` codes 43000 through 43003 mapped to
  exit statuses 2 through 5; a failed answer is never a CLI failure.
- `frame decode`, `frame encode`, `methods`, `hello`, and `version` expose
  the bridge and the endpoint's offer without any judgment.
- The rule audit fails closed on kernel dependencies, reverse
  dependencies, hand-built failure or frame literals, judgment functions,
  and method match arms.

## Open questions for the reviews

- Whether per-frame answering should be the default or `--batch` should
  be the only mode, given that cancellation only takes effect in a batch.
- Whether the endpoint should offer the `json_bridge` feature only under
  `--json` or always.
- Whether `Server::offered_hello` belongs to S20-410's contract surface or
  stays an implementation convenience.

## Records

| Stage | Commit | Tier 1 | Notes |
|---|---|---|---|
| Contract draft revision 1 | pending | pending | ADR-0035, stage checker, rule audit |
