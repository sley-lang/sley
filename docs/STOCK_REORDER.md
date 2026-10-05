# Stock reorder report with Sley

This small application takes a local inventory CSV and recommends replenishment
units in whole supplier packs. Its intended use is a reviewable offline report:
it does not buy stock, contact a supplier or update an inventory system.

Sley computes the rule and its checked failure paths. Python parses CSV, invokes
the pinned executable, replays and tests the package, formats the report, writes
an optional receipt, and creates the ZIP. Those host capabilities are not supplied
by the Sley function. Execution is **advisory over an exact candidate**, because
this release's route cannot admit the combined code-and-test transaction.

## Rule and supported inputs

Every row has a unique SKU and seven signed 64-bit integer fields:

```csv
sku,on_hand,on_order,reserved,daily_demand,days,safety,pack
BOLT-M8,10,5,3,4,7,2,6
```

All numeric values must be nonnegative, and `pack` must be positive. The Sley
function computes `target = daily_demand * days + safety + reserved` and
`available = on_hand + on_order`. It returns zero when available stock reaches
the target. Otherwise it rounds `target - available` up to a multiple of `pack`.
The example row recommends **18** units.

The contract uses checked signed 64-bit intermediates in that order. A negative
input or zero pack returns `InvalidInput`. Overflow in demand, either sum or
final pack rounding returns `Overflow`, even if a differently rearranged
mathematical expression could fit. This is a specified limitation, not saturation
or silent wrapping. The function does not predict demand; the CSV supplies it.

The host requires the exact header above, UTF-8, at most 2 MB and 10,000 rows,
unique SKUs matching `[A-Za-z0-9][A-Za-z0-9_.-]{0,63}`, and decimal i64 literals.
It rejects malformed, empty or duplicate records before emitting a report.
These are Python input limits, not Sley resource-admission evidence.

## Build and run

Use the published 2.0.6 Linux x86_64 agent, Python 3.9 or newer and the checksum
verification in [Capabilities](CAPABILITIES.md#reproduce).

```sh
python3 docs/examples/stock_reorder.py --agent /path/to/sley-agent \
  build --output ./stock-v1
python3 docs/examples/stock_reorder.py --agent /path/to/sley-agent \
  report --package ./stock-v1/package \
  --inventory docs/examples/stock_reorder_inventory.csv \
  --receipt ./stock-v1-run.json > stock-v1-report.csv
```

Expected report for the checked-in input:

```csv
sku,reorder_units,status
BOLT-M8,18,ok
WASHER-M8,0,ok
CABLE-1M,8,ok
CLIP-LARGE,12,ok
```

Exit 0 means every row returned `Ok`; exit 1 produces a complete report containing
one or more business errors, with empty units and `InvalidInput` or `Overflow`
in the status column. Exit 2 is an input/package/process error and emits no report.
The shell owns the redirected output file, including any truncation performed by
`>` before Python starts. Receipt files and build directories must not exist;
the application refuses to replace them.

Build creates a new genesis, authors the two functions and 17 TestCases together,
checks the kernel-selected count and advisory outcomes, submits the exact bytes,
and replays the package in a separate workspace. It checks the expected
`TXN_TEST_EVIDENCE_UNSUPPORTED` commit refusal and unchanged accepted repository
files. It never removes tests to get a tested candidate accepted.

`stock-v1/package/manifest.json` records the binary hash, base/proposed roots,
decoded candidate digest, selected-test count and file hashes. The ZIP contains
that manifest, the base pack, candidate bytes, optional names, authoring frame,
readable view and standalone Python entrypoint. It deliberately does not bundle
the agent: obtain and verify the named release separately.

After extracting `stock-v1/stock-reorder.zip` elsewhere, run its own
`stock-reorder/stock_reorder.py` with `report --package stock-reorder ...`.
Every report uses a new temporary workspace, validates the file hashes, replays
the exact candidate, compares both roots, runs the selected tests, then calls
`reorder` on the CSV rows in one batch. Accepted repository files are checked for
preservation. The receipt binds the exact input snapshot, output bytes and
candidate identity. Its hash inventory is unsigned and does not authenticate an
untrusted replacement package.

## Verify and maintain

```sh
python3 -B docs/examples/verify_stock_reorder.py \
  --agent /path/to/sley-agent --output ./stock-check
```

The verifier checks 445 inputs against an independent Python integer
specification, including deterministic random and i64 boundary cases. It runs
the extracted ZIP entrypoint, verifies positive and business-error CSV output,
and checks malformed input, changed package bytes and existing-output refusal.
The oracle belongs only to the verifier; report generation does not calculate
the rule in Python. Finite tests do not establish all-input correctness.

For a maintenance change, preserve the existing package and copy its
`frame.json` to a new file. Update the rule and its tests together, review the
intended behavior, and build into a new directory:

```sh
python3 docs/examples/stock_reorder.py --agent /path/to/sley-agent \
  build --frame revised-frame.json --output ./stock-v2
```

Review both manifests, the changed frame and `view.txt`; run relevant CSV cases
and independent checks before choosing the new package. Old review applies only
to the old candidate bytes/root. Keep both versions available; rollback means
selecting the preserved package. This creates a new advisory application
artifact, not a Git merge or accepted Sley transaction. Shared accepted-state
maintenance also needs root/conflict checks; the separate
[contributor walkthrough PR](https://github.com/sley-lang/sley/pull/30) demonstrates that path.

The executable maintenance example adds a reserved-stock regression case. Its
18-test package has a new candidate and proposed root over the same genesis,
while the original version remains unchanged. An intentionally wrong expected
value then fails the build and produces no ZIP. Each attempted build retains its
transcript and result for diagnosis.

## Recorded artifacts

The [recorded evidence](examples/capability-2.0.6/README.md) includes the v1/v2
ZIPs, manifests, matrix, raw command outputs, oracle observations and a report
receipt. The final runs used a fresh directory outside the source checkout on
the existing host. No native supervisor, provider campaign, independent human
adoption or authoritative tested commit is claimed. Native admission remains a
separate requirement in [audit #21](https://github.com/sley-lang/sley/issues/21).
