# Changelog

This file starts with Sley 2.2.0. Earlier releases are recorded as full release
notes under `docs/release/` (2.0.0 through 2.1.8); they are not summarized here.

## 2.2.0 — DRAFT, unreleased

Draft status: version and changelog preparation only. Nothing below is tagged,
built, sealed, hashed (release artifact) or published. The draft release notes
are at `docs/release/SLEY-2.2.0.md`, which lists remaining release verification.

### Added

- Native Text operations in the VM: `slice` lowers to native `E9` Text entries
  over `adapter_invoke`, in extended profile revision 18 (#102).
- Python-dialect string operations over native Text: `len`, indexing,
  iteration, slices, `+` concatenation, `ord`, `chr`, `str(n)` and `join`
  (#103).
- `bisect_left` and `bisect_right` in the Python dialect (#105).
- Structured-dialect integer dict locals: `dict`, `dput`, `dget`, `dhas` and
  `len` (#108).
- Python-dialect integer dict syntax, with `dput` keys carried through value
  branches (#109); local integer sets with add, membership and length (#111).
  Both update the `help python` card.
- Task-set v2 reference programs for the 13 string tasks; Sley's 2.2 dialect
  can now express all 24 v2 tasks, and every reference agrees with the oracle
  on all visible tests and 128 seeded hidden cases (#104).

### Changed

- The non-negative `//` and `%` fast path (2.1.8, #73) now also proves names
  non-negative from `loop` and `if` guard conditions, so guarded digits/loops
  qualify (#107).
- `sley-agent call --fuel <n>` lowers the call fuel ceiling for a single call
  or a batch. It only ever lowers the default ceiling (it accepts `n` up to
  the default); execution past the lowered ceiling still ends in a `Fuel`
  resource limit (#110).

### Known regression (historical evidence, carried forward)

- `fib(30)` wall time regressed between 2.1.6 and 2.1.8 (about 73.0 ms to
  76.5 ms, approximately 4.3–4.8%) with an identical VM instruction count
  (14,808,950). This is evidence from the 2.1.6–2.1.8 measurement sessions,
  not a fresh 2.2 timing, and no 2.2 runtime measurement has been taken.

### Not claimed in 2.2

- No new token-cost, runtime or model-cost measurements; no fresh paid API
  runs. Measurement v2 (four-arm) is blocked; the held-out tasks are not
  sealed and no pre-registration hash was computed.
- Four local oracle regression tests and 2,343 exhaustive small-domain count_by_remainder cases passed; a full task-set exhaustive sweep is deferred.
- Open work is listed as pending or deferred in
  `docs/release/SLEY-2.2.0.md`, including T1 (#112), still pending. T6 (#113) is merged: explicit once-per-session with-card totals and legacy missing-card n/a reporting; no fresh token measurement.
