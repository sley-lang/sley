# sley-tests

Pure native test evidence owner. Encodes and strictly parses the native
resource policy (`SLEYNRP1`), test plan (`SLEYTPL1`), execution/test reports
and parsed approvals owned by `NATIVE_TEST_EXECUTION_V1.md` and
`NATIVE_TEST_ADMISSION_V1.md`.

This crate performs no host I/O, spawns no workers, reads no clock and
provisions no trust. Parsed records never construct protected authority:
verifying a plan against fresh policy, authenticating measurements and
admitting commits are the policy, runner and transaction owners' jobs
(N3–N5). Dependencies point from tests to VM, never the reverse.
