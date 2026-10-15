# Authoring-cost arithmetic reproducibility

This standalone package recomputes twelve recorded Sley authoring-cost comparisons from a portable numeric projection. It makes no provider calls and needs only Python 3.9+ and its standard library. The comparisons correspond to the [Sley 2.1.1 release notes](../../../docs/release/SLEY-2.1.1.md).

Run from this directory:

```sh
python3 -B recalculate.py projection.json --verify expected.json
python3 -B -m unittest -v test_recalculate
```

The first command emits JSON containing the twelve points, each model's task sets and numerator/denominator counts, infrastructure exclusions, all model/run/arm cells, and held-out correctness counts. `published_values_verified: true` means these supplied numeric records reproduce the expected values to absolute tolerance 1e-12. It is not an independent verification of prices, model identity, oracle results, invoices or runtime behavior. SHA256SUMS.json binds the package files; verify hashes before use.

## Records and normalization

The projection contains 5,440 trials and 5,814 attempts across three runs, including 199 trials marked as provider infrastructure failures. Those trials remain visible in the data and totals but are excluded from the ratios and retained correctness denominators. The complete planned model/set/language/task/repetition grid is declared and validated; missing, extra and duplicate trials refuse.

Eight stable labels replace private model identifiers. Task names and language arms are retained. For each model, every recorded list-price cost is divided by **one shared positive total across all three runs**, including infrastructure trials. Its normalized costs therefore sum to one. The private divisor and identity mapping are omitted. A shared divisor cancels in every same-model ratio, including comparisons between historical release cohorts; normalizing each run separately would change the results. Tokens are retained as recorded integer totals. Attempt counts and flags are supplied, rather than prompt or response text.

These labels provide pseudonymization, not a guarantee against identifying models from performance or token patterns. This packet omits model identities, provider routes, absolute prices, dollar amounts, spending limits, prompts, answers, errors, raw usage objects, feedback, original paths and the private source inventory. Source extraction and original attempt accounting were checked privately, but this packet alone does not prove those original records or independently audit cost-from-token pricing.

## Exact methods

All medians are over per-model ratios. For each comparison, the numerator and denominator are each `sum(recorded value) / count(correct answers)` after the specified infrastructure exclusions. The value is normalized cost or total tokens. Repetitions are pooled within each task. No model outputs or oracle programs execute here.

* **Development cross-language:** pool `final211` and `final2`; use `dev/sley_pym`; within each model intersect retained task identities with C, C++, Rust, Python or Go. Compute cost-per-correct ratios over those common tasks and take the model median. Expected points are approximately 0.8550, 1.0072, 1.0366, 1.1623 and 1.2704.
* **Historical baseline:** compare `final2/sley_pym` against `base210/sley_f`, separately for development and validation. Use each whole retained model cell, **without a common-task intersection**, for both cost and tokens. Cost ratios are approximately 0.6112 and 0.4652; token ratios are 0.4732 and 0.3707.
* **Second held-out set:** use `final2/held2/sley_pym` and the same common-task method against Python, Go and Rust. Ratios are approximately 1.7860, 1.8357 and 1.4140. Minimal-card `sley_pym` has 142 correct of 150 retained trials and 116 first-try successes. Longer-card `sley_py` has 143 correct of 149 retained trials and 139 first-try successes. These are different guidance arms.

The baseline pairing distinction matters: two models have different retained validation task sets after infrastructure exclusions. To inspect the common-task alternative without changing the published calculation:

```sh
python3 -B recalculate.py projection.json --common-task-baseline
```

That alternative yields validation cost ratio 0.4666146381646913 and token ratio 0.3778654903775837. It cannot be combined successfully with verification of the published baseline method. The release-note rounding is unchanged.

A comparison with no common tasks, zero correct answers on either side, or zero reference value is explicitly undefined. Positive means that underflow to zero, and ratios that underflow or overflow, are also undefined with explicit numeric reasons. Oversized JSON integers refuse before conversion to floating point. Each model remains visible with a reason; the median uses defined model ratios only. All twelve supplied published comparisons have eight defined models. Verification refuses fewer models, mismatched denominators or changed points. No confidence intervals or significance claims are generated; this packet does not recreate the earlier per-model bootstrap analysis.

## Validation and limitations

`schema.json` describes the allowed fields and primitive bounds. The standard-library validator also enforces cross-field flag consistency, unique and complete trial grids, repetition/attempt limits, and shared per-model normalization. It rejects unknown fields, duplicate JSON keys, booleans used as integers, nonfinite numbers, negative values and inputs larger than 16 MiB. Schema validation requires no third-party dependency.

The three run names describe preserved historical cohorts. Candidate and pilot iterations and two different held-out sets existed; this packet does not imply a new untouched held-out experiment. The retained `final211` and `final2` campaign binaries are byte-distinct from the public Sley 2.1.1 `sley-agent`. Arithmetic recomputation does not establish semantic equivalence or rerun those tasks on the released executable. The recorded SHA-256 identities are:

| Executable | SHA-256 |
| --- | --- |
| `final211` campaign | `3bbb54f638bd65a964030318cca72bf7f60fe30a5f9983ab38c889d2203d173f` |
| `final2` campaign | `8dde99de4f81d49212cc576a5c23f74684a3c1a1e1844baca91f757d6f58e3d3` |
| Public 2.1.1 `sley-agent` | `4f749c1111e6d53ac62ca8213a7fa176155a8a62c1c8ff4dce25e405936ebf79` |

The public executable was verified from archive SHA-256 `b0898ba7404a1656db2f83f448ea96d2c1175346f35e50b135bedc96e3810bfd`. Original compiler/environment versions and complete execution logs were not frozen. Retained generated sources could support a separately reviewed offline correctness replay, and independent model replication would require model/price disclosure and new provider access and spend authorization. Neither happens in this package.

The authored code and numeric projection in this directory are covered by the repository’s [Apache-2.0 license](../../../LICENSE). Original measurements remain preserved separately. This package includes no generated answers, third-party model outputs or binaries and grants no rights to those omitted materials.
