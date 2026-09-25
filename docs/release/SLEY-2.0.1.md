# Sley 2.0.1 release notes

**Release:** Sley 2.0.1 (version `2.0.1`), a patch release of 2.0.0.
**Artifact:** `sley-2.0.1-linux-x86_64.tar.gz`, SHA-256 `TO-FILL`, size
`TO-FILL` bytes. The build's records (commit, manifest digest,
reproducibility result) are in `evidence/release/reproducibility-report.json`
and the machine summary's `release_candidate_packaging` section.
**Not a GA claim.** `ga_claimed` stays `false`. 2.0.1 fixes defects and
tightens the release process. It closes no GA acceptance criterion, and every
limit in the [2.0.0 known limits](SLEY-2.0.0.md#known-limits-and-what-is-not-yet-claimed)
still applies.

The protocol version, the schema epoch, and the method table are unchanged,
so a 2.0.0 client talks to a 2.0.1 endpoint the same way. The one change to
recorded identities is the native-test approval fix below.

## Fixes contributed by Fred Nix

Fred Nix ([@nixfred](https://github.com/nixfred)) found and fixed four
defects in 2.0.0, each with a regression test. Thank you, Fred.

- **VM: checked division, remainder, and negation never abort the host**
  ([#7](https://github.com/sley-lang/sley/pull/7)). `int_div_checked` and
  `int_rem_checked` guarded overflow only at the declared width, then ran the
  operation on the raw 128-bit value. An operand outside its declared width,
  which a caller could supply as an approved-package runtime input, made the
  host panic on `i128::MIN / -1`, even in release builds. `int_neg_checked`
  wrapped silently. All three now compute with checked arithmetic and answer
  `ARITHMETIC_OVERFLOW`. In-width behavior is unchanged.
- **Exchange: two entries for one branch name refuse before any write**
  ([#8](https://github.com/sley-lang/sley/pull/8)). An exchange could carry
  two branch entries with the same name and different origin or ref records.
  Both passed preflight, and the import then failed partway with
  `EXCHANGE_IO`, leaving a marked incomplete clone that later imports and
  recovery refused. The importer, and the native exchange profile, now refuse
  the pair with `EXCHANGE_DUPLICATE_ENTRY` before the target is created.
  [REPOSITORY_EXCHANGE_V1](../spec/REPOSITORY_EXCHANGE_V1.md) states the
  rule as import step 5.2.
- **CLI: non-Unicode arguments and oversize non-UTF-8 lines**
  ([#9](https://github.com/sley-lang/sley/pull/9)). A command-line word that
  isn't valid Unicode, such as a `--repository` path with a stray byte, made
  `sley` panic and exit 101. It is now `CLI_USAGE_INVALID` (exit 2) with the
  usual JSON failure object. In `--json` mode, a line over the bridge's text
  ceiling whose bytes weren't valid UTF-8 was answered
  `JSON_BRIDGE_SHAPE_INVALID`, and serving continued from the middle of the
  same line, which produced a stream of rejections. The ceiling is now judged
  on the bytes read, so the line is answered `JSON_BRIDGE_RESOURCE_LIMIT` and
  ends the input, as [SLEY_CLI_V1](../spec/SLEY_CLI_V1.md) section 8
  requires.
- **Transactions: the native-test approval records the accepted parent root**
  ([#10](https://github.com/sley-lang/sley/pull/10)). The approval of a
  native-test commit recorded the proposed root in its `parent_root` field
  instead of the accepted parent root that the plan and the transaction bind.
  2.0.1 writes the accepted parent root. Because the field is sealed into the
  approval ID, a native-test commit made by 2.0.1 gets a different approval
  ID, and so different transaction and receipt IDs, from the same commit made
  by 2.0.0. The frozen native-test conformance responses are regenerated to
  match. Approvals already written by 2.0.0 keep the old value and stay
  readable. The errata note in
  [NATIVE_TEST_ADMISSION_V1](../spec/NATIVE_TEST_ADMISSION_V1.md) section 3
  gives the details.

## Other changes

- **Hardening of caller-supplied runtime inputs.** The VM's approved-package
  execution path judged caller-supplied inputs structurally only. It now
  checks each input's declared type against its register, then refuses any
  input that isn't canonical for that type: an integer outside its declared
  width, or a nested value (an option payload, list element, record field or
  variant payload) whose own type differs from the type its container
  declares. Both checks run before any other work over the value's contents,
  with the existing `VM_EXEC_INPUT_NOT_CANONICAL` code. Out-of-width operands
  like those in #7 now stop at the input boundary, in addition to Fred's
  checked arithmetic.
- **Third-party license texts in the archive.** The static binary
  redistributes the locked third-party crates, so the archive now ships
  `THIRD_PARTY_LICENSES` with their license texts and copyright notices. It
  is generated from the locked crate sources, and the build refuses if the
  tracked file has drifted from them or doesn't list exactly the inventoried
  crates. It also carries the notices of the Rust standard library and musl
  libc, which the toolchain links into the static binary.
- **Demo robustness.** The archive's `demo/run_demo.py` finds its binary and
  fixture next to itself, so it runs from any directory. It works in a
  temporary directory and removes it when it finishes, unless you pass
  `--work <dir>` to keep the repositories.
- **Build: dependency fetch.** `make release-candidate-build` runs
  `cargo fetch --locked` first. The musl build fetches only what `sley-cli`
  needs for its target, while the offline steps that read crate sources (the
  third-party license texts and the supply-chain evidence) need every crate
  in the lockfile. A fresh or partial cargo cache no longer breaks them.
- **Golden-vector checksum gate.** `scripts/check_golden_sha256sums.py`
  checks `crates/sley-tests/golden/` against its `SHA256SUMS`: every listed
  file must exist with the recorded digest, and every file must be listed.
  Nothing checked these vectors before. `make quick` runs it.
- **Documentation cleanup.**
  - The internal maintainer handoff note `RESUME.md` is removed.
  - `SECURITY.md` states the threat register's real status and points to
    [`docs/THREAT_REGISTER.md`](../THREAT_REGISTER.md) and its coverage
    report.
  - The [Quickstart](../QUICKSTART.md) shows the demo's `explicit_gap`
    field and what the demo doesn't cover. It also corrects the exit-code
    table for `--json` mode, notes that session ids change per run, explains
    why `hello` reports `"json_bridge": false`, and sets expectations for
    test times, `make lint`, `make quick`, and `cargo fetch`.
  - The README's build section and `CONTRIBUTING.md` list the gates a
    contributor can run from a fresh clone.
  - `ARCHITECTURE.md` lists the 20 crates that exist and their dependency
    order.
  - The 2.0.0 notes no longer point to a branch that isn't public, and they
    say where the 2.0.0 release records live.

## Build and verify

The build needs a clean checkout at the release commit, the pinned Rust
toolchain (`rust-toolchain.toml`, 1.93.0) with the `x86_64-unknown-linux-musl`
target installed, Python 3 and `uv`.

```sh
make release-candidate-smoke        # fetch, two clean builds + all release records, then verify
make release-candidate-verify       # re-check the records against the tree
sha256sum dist/sley-2.0.1-linux-x86_64.tar.gz
python3 -c 'import json; r = json.load(open("evidence/release/reproducibility-report.json")); print(r["result"], r["commits"])'
```

The digest printed by `sha256sum` must equal the SHA-256 above and the
`artifact_sha256` that the reproducibility report records for the release
commit. To reproduce the build on a second host, follow
`docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md` section 5.1, as
for 2.0.0. The [Quickstart](../QUICKSTART.md#6-run-the-test-gates) lists the
test gates that run from a fresh clone.

## Known limits

Unchanged from 2.0.0. See
[Known limits and what is not yet claimed](SLEY-2.0.0.md#known-limits-and-what-is-not-yet-claimed).
