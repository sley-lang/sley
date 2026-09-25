# Succession accounting (S20-630)

`bench/accounting/report.py` derives the succession accounting report of one
benchmark run (`docs/spec/SUCCESSION_ACCOUNTING_V1.md`, ADR-0037) from the
run's create-once manifest and each arm's claim chain, verified by that arm's
own runner. Every claim is an attempt; every quantity is an integer or a
reduced ratio; Accepted Change Tokens are total observable tokens over
accepted correct changes; the plan's section 22 thresholds are evaluated
only between complete arms and are otherwise `UNDETERMINED`.

The report inherits the claims' unverified status
(`DERIVED_FROM_UNVERIFIED_CLAIMS`) and the dossier's succession fields stay
null until a complete report over verified claims exists. No trial has run.

```text
python3 -m bench.accounting.report derive --run <run-directory> [--output report.json] [--require-complete]
python3 -m bench.accounting.report smoke --sley2-evidence evidence/runtime/s20-620-sley2-smoke/evidence.json --output-dir evidence/runtime/s20-630-accounting-smoke
python3 -m unittest discover -s bench/accounting/tests -t .
```
