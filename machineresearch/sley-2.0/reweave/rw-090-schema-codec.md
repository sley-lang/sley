# RW-090 bounded frozen-schema codec — provisional construction record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice supplies the
schema-codec dependency required by RW-090. It is development evidence, not
accepted runtime authority. RW-080 and RW-090 remain BLOCKED and R2 remains
NOT_READY; independent review and acceptance debt are unchanged.

## Construction

`crates/sley-vm/tests/rw080_codec_program/schema_codec.rs` builds two admitted
Sley images for the closed epoch registry used by the current product:

- decode accepts the exact canonical `SLEYEP01 || uvar(1) ||
  uvar(record_len) || record` preimage and returns the derived epoch ID plus
  canonical epoch record;
- encode accepts that exact epoch ID and canonical record and emits the exact
  bootstrap preimage;
- decode rejects every other byte string with `SCHEMA_RECORD_INVALID`;
- encode checks the epoch before the record, returning
  `SCHEMA_EPOCH_MISMATCH` before `SCHEMA_RECORD_INVALID` when both inputs are
  wrong.

The admitted programs own the byte comparisons, control flow, refusal choice,
tuple construction, and emitted value. They import no adapters and call no
native semantic service. The frozen record, ID, preimage, and error bytes are
ordinary Sley constants. Native `sley-schema` and `sley-state-root` code is
used only by the Rust parity tests to derive and validate the expected fixture.

This is deliberately a bounded registry codec. It does not parse an arbitrary
future schema record, select among multiple epochs, perform migration, or
claim an open-ended schema implementation. The current registry contains one
frozen epoch, so a finite exact codec is sufficient for that registered domain
and is honest about rejecting inputs outside it.

## Evidence

Two tests exercise both admitted images:

- the canonical preimage imports natively and decodes to the exact frozen ID
  and canonical record in Sley;
- bad magic, truncation, trailing data, and non-minimal version encoding are
  rejected by both the native importer and the Sley decoder;
- the Sley encoder exactly reproduces `sley_schema::bootstrap_preimage`;
- wrong-epoch and wrong-record cases pin failure precedence.

Validation commands:

- `cargo test -p sley-vm --test rw080_codec_program_outer schema_ -- --nocapture`
- `cargo test -p sley-vm --test rw080_codec_program_outer`
- `cargo clippy -p sley-vm --test rw080_codec_program_outer -- -D warnings`
- `cargo fmt --all -- --check`
- `python3 scripts/build_test_inventory.py --check`
- `python3 scripts/check_decision_dossier.py`
- `python3 scripts/check_local_completion_frontier.py`
- `python3 scripts/check_s20_700_frontier.py`

The four legs are now composed by the bounded executable `codec_main` recorded
in `rw-090-codec-main.md`. Full program-body generality, label/NFC handling,
integration into the canonical retained toolchain graph, independent review,
and acceptance remain future work. This record changes no gate, ledger,
release, or runtime-authority status.
