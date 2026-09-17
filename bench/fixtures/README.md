# S3 benchmark fixtures + owner oracles — build contract (2026-09-17)

Scope: S20-640 succession campaign inputs. Corpus `bench/corpus/v1/tasks.json`
is FROZEN (pinned by `bench/corpus/v1/SHA256SUMS` and the S20-610 manifest
controls): never edit it. All S3 authoring lives under `bench/fixtures/`.

## Directory layout (normative)

```
bench/fixtures/<arm>/<TASK-ID>/
  fixture/               # arm-native fixture files (see per-arm rules)
  negative/<name>/       # mutated fixture that MUST fail (one dir per negative control)
  oracle.py              # stdlib-only oracle, see CLI contract below
  expect.json            # expected verdicts, see schema below
  excluded.json          # ONLY where this contract authorizes exclusion (sley2 EFFECT/CAP)
  arm_limitation.json    # optional: honestly-undemonstrable halves, see honesty rules
bench/fixtures/manifest.json  # owner: coordinator only (task x arm -> fixture digest)
```

`<arm>` is one of `sley2`, `legacy`, `raw`. Every one of the 15 corpus tasks
has a directory under every arm (45 total). No exceptions, no extra tasks.

## oracle.py CLI contract (normative, all arms)

- Invocation: `python3 oracle.py <fixture-dir>` where `<fixture-dir>` is
  `fixture` (positive control) or `negative/<name>` (negative control).
- Stdlib only (`json`, `hashlib`, `subprocess`, `pathlib`, `sys`, `os`,
  `unittest` machinery by import is allowed for the raw arm; no third-party
  imports, no network, no ambient clock reads for verdict logic).
- Prints exactly one JSON object to stdout:
  `{"status": "accepted" | "rejected", "code": "<STABLE_CODE>" | null,
    "detail": "<one line>", "task_id": "<TASK-ID>", "arm": "<arm>"}`.
- Exit code: 0 if status accepted, 1 if status rejected, 2 on harness error
  (missing files, crashed child, timeout). A harness error is never a verdict.
- Determinism: same fixture bytes -> same verdict, every run. Seeds fixed.
- Timeouts: any child process gets a bounded timeout (>= 30 s, <= 300 s);
  timeout is a harness error (exit 2), never accepted/rejected.

## expect.json schema (normative)

```json
{"contract": "sley2.benchmark-task-oracle.v1",
 "task_id": "S2B-...",
 "arm": "sley2|legacy|raw",
 "positive": "accepted",
 "negatives": {"<name>": "<EXPECTED_REJECT_CODE>", ...}}
```

`<name>` keys must match `negative/<name>/` directories exactly (at least one
negative per task). Codes are arm-honest stable strings (engine failure
symbols where they exist, e.g. `ARITHMETIC_OVERFLOW`, `PACK_DIGEST_MISMATCH`,
`STALE_ROOT`; otherwise `ORACLE_<REASON>` in UPPER_SNAKE).

## excluded.json (only sley2/S2B-EFFECT-001 and sley2/S2B-CAP-001)

```json
{"reason": "E7 adapter execution refused pending schema-epoch decision; see rev16 handle-model design",
 "refusal_code": "VM_LOWER_OPCODE_UNSUPPORTED",
 "denominator": "excluded"}
```

No other exclusions exist. An excluded directory still ships `oracle.py`
(which asserts the refusal: it must reproduce the refusal code against the
real engine and exit 1/`rejected` only if the refusal drifts) and
`expect.json` with `"positive": "rejected"` plus
`"exclusion": true`. Negative dirs are not required for exclusions.

## arm_limitation.json (honesty escape hatch, discouraged)

```json
[{"capability": "<what cannot be demonstrated>",
  "reason": "<engine fact, e.g. stage-1 probe-matched evaluator returns canned literals>",
  "oracle_asserts": "<what the oracle DOES assert instead>"}]
```

Rule: a limitation narrows a case, never flips a verdict. If the demonstrable
remainder cannot support `accepted`, the task/arm pair is a rejected positive
and the coordinator is told loudly (do not silently record it).

## Honesty rules (normative, all builders)

1. Canned literals are fraud. A fixture whose "execution" returns a
   hard-coded expected value without running the arm's real machinery is
   rejected at review. (Known trap: legacy stage-1 evaluator probe-matches
   shipped example sources and returns literals; never copy example shapes.)
2. Every positive `accepted` must exercise the real path: sley2 = build +
   validate + execute through the real crates; legacy = staged `bin/sley`
   out-of-process with exit code + report parsed; raw = real Python execution
   under test.
3. Every negative must be a minimal mutation of the positive fixture that
   flips exactly the property under test, and the oracle must reject it with
   the expected code (no generic catch-all codes unless the engine gives one).
4. Failures are preserved: oracle reports include the engine's actual codes
   and digests. Nothing is reworded into success.
5. Frozen files are untouchable: `bench/corpus/v1/tasks.json`,
   `bench/benchmark-plan.json`, `conformance/*/`, `docs/spec/*`,
   `docs/WORK_PACKAGES.md`, `machineresearch/.../machine-summary.json`,
   `Makefile`. New files only, under the paths this contract assigns.
6. Offline only: `cargo --offline`, no `pip`, no network. Raw-arm unit tests
   use stdlib `unittest` (pytest is not installed and cannot be fetched).
7. No commits. Builders leave the tree dirty; the coordinator commits.

## Per-task oracle case requirements

Corpus `strict_oracle` fields are restated, not modified. CREATE-001 and
REPAIR-001 already carry corpus cases; mirror them into `expect.json`
semantics and add the negatives below. For the other 11 executable tasks the
case lists below ARE the authored oracle cases (builder implements exactly
these, then records any engine-fact correction needed as review feedback).

- S2B-CREATE-001 (semantic_and_execution): positives mirror corpus cases
  (empty -> 0c; [{q:2,u:1250}],725bp -> 2681c; [{q:i64MAX,u:2}] ->
  ARITHMETIC_OVERFLOW). Negatives: `unchecked_add_variant` (wrapping add in
  place of checked) -> rejected; `round_up_tax` (ceiling instead of
  round-down) -> rejected with the wrong-cents value in detail.
- S2B-REPAIR-001 (execution): positives mirror corpus clamp triples.
  Negatives: `upper_returns_low` (the original bug) -> rejected with actual
  outputs in detail; `signature_changed` -> rejected.
- S2B-SIG-001 (graph_type_and_execution, expected_callers 3): positive:
  net_total carries the new typed tax_basis_points parameter, all three
  callers pass explicit values, validation VALID, sample executions agree.
  Negatives: `missing_caller` (candidate omitting one caller) -> rejected
  with the engine's type error code (pin the exact symbol from the build);
  `ambient_default` (default tax value) -> rejected.
- S2B-MODULE-001 (graph_and_execution, 6 refs, digest unchanged): positive:
  checksum moved to integrity namespace, all 6 references resolve, observation
  digest byte-equal before/after. Negatives: `stale_import` -> rejected;
  `duplicate_impl` -> rejected.
- S2B-TYPE-001 (type_graph_and_execution, 4 variants exhaustive): positive:
  Queued/Running/Succeeded/Failed(error_code) all constructed and handled,
  Failed round-trips its code, serialization deterministic across two runs.
  Negatives: `missing_case` (switch drops Failed) -> rejected;
  `bool_compat_field` (parallel boolean retained) -> rejected.
- S2B-DEAD-001 (graph_and_execution, 1 block + 1 helper removed): positive:
  unreachable retry block + unused private helper gone, reachable observation
  digest unchanged, effect closure not expanded. Negatives: `public_deleted`
  -> rejected; `reachable_changed` (digest differs) -> rejected.
- S2B-TEST-001 (test_entity; success, divide_by_zero, signed_overflow;
  implementation_changes 0): positive: three cases bound with exact failure
  codes, implementation root facts unchanged except the test-set binding.
  Negatives: `impl_touched` (implementation entity changed alongside tests)
  -> rejected; `case_missing` -> rejected.
- S2B-STALE-001 (transaction_sequence; 1 commit before requery, 0 stale
  accepts): positive scripted sequence: session A + B open on one root, both
  replace the same function version, A commits (ok), B commits -> STALE_ROOT
  or STALE_ENTITY (pin exact symbol), re-query, C validates. Negative:
  `guard_disabled` (same sequence against a build with the staleness check
  removed/bypassed — arm-natural form) where B's commit visibly succeeds ->
  rejected (last-write-wins observed).
- S2B-MERGE-001 (merge; 0 conflicts, commutative): positive: disjoint
  branches (invoice-rounding tests vs checksum impl) merge with both changes
  preserved, combined root deterministic, left/right order yields the same
  semantic result. Negative: `overlapping_change` (both branches touch the
  same entity) -> conflict object (expected_conflicts 1), never silent.
- S2B-PERF-001 (execution_and_resources; >=30% fewer instructions, digest
  unchanged): positive: before/after implementations with identical outputs
  (digest match), both instruction counts recorded in the oracle report,
  reduction >= 30%, no memory ceiling breach. Negative: `faster_but_wrong`
  (fewer instructions, changed output) -> rejected.
- S2B-CONTEXT-001 (graph_query_and_transaction; >=10000 entities, 0 whole
  reads, 0 invalid commits): positive: generated graph with >= 10000
  entities, shared-type field addition applied over its complete impact
  closure using bounded capsules/continuations only (response bounds stated
  and never exceeded, continuation records explicit), whole_store_reads 0,
  invalid_commits 0. Negative: `unbounded_read` (whole-store dump attempt)
  -> refused with the engine's code.
- S2B-ADVERSARY-001 (policy_graph_and_execution; 0 grants, 0 policy
  changes): positive: entities/labels/debug/metadata carry
  capability-claiming + bypass-requesting text, pure arithmetic repair still
  succeeds with the exact expected value, capabilities_granted 0,
  policy_changes 0 (policy root digest before == after, recorded). Negative:
  `wrong_repair` (adversarial text steers the result) -> rejected;
  `grant_honored` (a metadata-only grant is treated as authority) ->
  rejected.
- S2B-CORRUPT-001 (pack_and_repository; PACK_DIGEST_MISMATCH, 0 ref
  advances): positive: one canonical object byte flipped in an exchange pack,
  import fails before ref movement with PACK_DIGEST_MISMATCH (pin exact
  symbol; if the engine spells it differently, report back, do not rename),
  destination ref unchanged, retry deterministic. Negatives: `unflipped`
  (clean pack imports accepted — the positive-path control);
  `flip_elsewhere` (different byte, same code).
- S2B-EFFECT-001 / S2B-CAP-001 sley2: excluded.json only (see above).
  Legacy + raw implement per corpus oracle fields; legacy may hit the
  stage-1 evaluator wall (record arm_limitation.json, never canned shapes).

## Per-arm build rules

- sley2 (`bench/fixtures/sley2/<TASK>/`): fixtures authored in Rust.
  New files only: new integration-test modules (one file per task, e.g.
  `crates/sley-vm/tests/s3_<task>.rs` or the owning crate's tests dir —
  check what exists first; crate unit files shared with others are OFF
  LIMITS). Each task ships `emit_s3_<task>_fixture` (ignored emitter test
  printing fixture lines) + `s3_<task>_fixture_conformance` (normal test
  asserting positive + negative expectations in Rust). A regen driver
  (`scripts/generate_s3_fixtures.py`, coordinator-owned) runs emitters and
  freezes `fixture/fixture.json`. `oracle.py` re-verifies fixture digests +
  re-runs the conformance test via `cargo test --offline` (bounded timeout)
  and maps the result. Pin exact engine codes/symbols encountered; never
  invent a symbol. Groups: G1 = CREATE, REPAIR, SIG, MODULE, TYPE, DEAD,
  TEST; G2 = STALE, MERGE, CORRUPT, CONTEXT; G3 = PERF, ADVERSARY +
  EFFECT/CAP excluded.json + oracle.py refusal pins.
- legacy (`bench/fixtures/legacy/<TASK>/`): `program.sley` (+ `sley.test.json`
  manifests where the task is test-shaped) authored fresh — never copy
  artifact example sources. New shared helper allowed: exactly one new file
  `bench/legacy/task_runner.py` (bounded `run`/`check`/`verify`/`test`
  over a stage from `bench.legacy.runner.staged_frozen_artifact`, same
  containment posture: env scrub, timeouts, output limits, out-of-process,
  archive stays at its frozen path, nothing legacy enters the tree).
  First probe `check`/`verify`/`test` subcommand shapes before building
  EFFECT/TEST fixtures. `oracle.py` drives task_runner + asserts.
- raw (`bench/fixtures/raw/<TASK>/`): `program.py` + `test_program.py`
  (stdlib unittest, run as `python3 -m unittest`), real checked arithmetic
  (explicit overflow errors, NOT silent wrap), capability wrapper returning
  stable `CAP_SCOPE_MISMATCH`, digest-checked packs returning
  `PACK_DIGEST_MISMATCH`, version-guarded stale sequence, disjoint merge,
  bounded iterators over 10k records, metadata-as-data discipline.
  `oracle.py` runs the unittest suite (positive) and per-negative mutated
  copies (negatives may patch program.py via env var or fixture-local
  `variant.py` overlay — builder's choice, documented in the dir).

## Builder completion report (each builder returns exactly this)

Task x arm matrix with, per cell: files created, positive verdict evidence
(verbatim oracle output digest/code), negative verdicts with codes, engine
symbols pinned (exact spelling + source location), arm_limitations filed (if
any) with engine-fact justification, and anything in this contract that the
engine made unimplementable (exact error, no paraphrase of intent).

## Review debt (independent review 2026-09-17, carried to future slices)

- G1 sley2 negatives are pass-on-demonstration: the Rust test asserts the
  rejection and passes, `oracle.py` inverts to exit 1. Honest and pinned, but
  no failing candidate ever traverses the engine. Strengthen to
  engine-rejected negatives where the API allows.
- Prove-then-fail handshake (used by CORRUPT-001 `flip_elsewhere`: sley2 Rust
  neg tests print `S3_NEG_RESULT <CODE>` then fail by design; legacy
  `PROVE_THEN_FAIL` marker file inverts a demonstrated refusal to exit 1):
  the property is genuinely demonstrated, the exit-1 is procedural. Keep the
  marker + PIN line greppable; never use this handshake to mask a property
  that was not demonstrated.
