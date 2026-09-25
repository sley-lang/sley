# Sley 2 trial runner (S20-620)

`bench/sley2/runner.py` is the endpoint-only, trace-complete runner of the
mandatory `sley_2_0` arm (`docs/spec/SLEY2_TRIAL_RUNNER_V1.md`, ADR-0036).
It drives the S20-430 `sley` binary in JSON mode over one disposable
repository seeded through `exchange.import`, hands the agent adapter a
two-operation handle (`exchange`, `affordances`) and nothing else, chains
every frame into a trace before the next request is written, derives ten
metrics only from that trace, and appends explicitly unverified claims
under the S20-610 run manifest.

The endpoint binary is the only process it starts. Models, providers, and
oracles are injected Protocols with no supplied implementation; the smoke
uses a scripted agent that attempts no task, so its claim is `rejected`
and no trial counts as benchmark evidence.

```text
cargo build -p sley-cli --locked
python3 -m bench.sley2.runner smoke --evidence-dir evidence/runtime/s20-620-sley2-smoke
python3 -m bench.sley2.runner verify --run evidence/runtime/s20-620-sley2-smoke/run-<stamp>
python3 -m unittest discover -s bench/sley2/tests
```
