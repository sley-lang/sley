# VM differential findings

Each `seed-<n>.py` here is a program the seeded differential fuzzer
(`sley_agent::vmfuzz`, `tests/vm_differential.rs`) found the reference
interpreter and the derived tiers disagreeing on, shrunk to a small program
that still disagreed. Its `# args:` line is the case and its `# limits:` line
the limits it disagreed under (`fuel instructions value-units output-units
cancel`). `fixtures_agree` runs every file on every `cargo test`, under those
limits and fresh cuts, so a fixed disagreement stays fixed.

A run writes a finding here with `SLEY_VMFUZZ_SAVE=1`, for example
`SLEY_VMFUZZ_SAVE=1 make vm-fuzz SEEDS=0..100000`. Replay one seed with
`SLEY_VMFUZZ_SEED=<n> cargo test -p sley-agent --test vm_differential -- --ignored replay`.
