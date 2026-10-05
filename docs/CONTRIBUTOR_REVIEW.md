# Reviewing a Sley change from a fresh workspace

This example demonstrates [audit #21 item 10](https://github.com/sley-lang/sley/issues/21):
inspect a change, identify its exact state, diagnose a failing case, contribute a
correction and handle a changed base. A Git pull request carries the review
packet. Merging that Git PR does not itself accept a Sley transaction.

The example uses the published **2.0.6 Linux x86_64** `sley-agent` and Python
3.9 or newer. It creates a program from a new genesis and authoring input,
then exports that newly built base for separate reviewer and maintainer
workspaces. It does not start from the release's prebuilt demo fixture, use
private tooling, contact a provider or require a supervisor installation.

## Run it

Get `sley-2.0.6-linux-x86_64.tar.gz` and `SHA256SUMS` from the
[2.0.6 release](https://github.com/sley-lang/sley/releases/tag/v2.0.6), then:

```sh
sha256sum --check SHA256SUMS
tar -xzf sley-2.0.6-linux-x86_64.tar.gz
python3 /path/to/sley/docs/examples/contributor_review.py \
  --agent ./sley-2.0.6-linux-x86_64/bin/sley-agent \
  --output ./review-run
```

`review-run` must not exist. The runner never resumes or overwrites an old
run. It uses only the supplied binary and Python's standard library, with new
workspaces and a minimal subprocess environment. The archive's SHA-256 is
`90e072569da7d6fc340ba850803b8e13cc0a608dc831f6ddb4c01c7996f4a054`;
the runner additionally pins the agent binary's SHA-256. An explicit
`--expect-agent-sha256` can identify another build, but this does not make it a
qualified release or establish that this example supports it.

The result is `review-run/REVIEW.md`, a navigable packet with the authoring
inputs, canonical base/candidate bytes, display views and diff, exact roots,
per-command results, raw transcript and a file-hash manifest. Once commands begin, failed runs keep their transcript and `result.json`;
inspect them before starting another run in a new directory. A binary identity
mismatch or existing output directory refuses before writing any output.

## What the example proves

The intended change is `scale(x) = x * 2`. The initial program returns `x` and
has an unrelated `sentinel()` returning `7`.

1. **Diagnose behavior.** A proposal returning `x + 2` is statically `Valid`
   but fails the authored cases `-3 → -6`, `0 → 0`, and `4 → 8`. The test
   result gives expected and actual values; the candidate view shows the
   operation to correct. Static validity did not imply correct behavior.
2. **Propose a correction.** A frame changes `scale` and adds those three
   TestCases. `try --base-root ROOT` binds it to the accepted context. The
   packet records that base, proposed root and SHA-256 of decoded canonical
   candidate bytes. `submit` preserves those bytes in `final_candidate.hex`.
3. **Review in another workspace.** The reviewer imports the newly exported
   base and optional names, without the author's `.sley/` cache. `submit`
   accepts the exact candidate file for review; `view --after` reconstructs
   the same proposed root and `test` reproduces all three matches. Canonical
   candidate hex is supplied to `submit`/`view`/`test`, not to `try`, which
   expects authoring JSON or operations. AV1 and its text diff are output-only
   aids, not canonical program inputs. Display names carry no authority.
4. **Keep admission separate.** Attempting the tested commit on this release's
   v1 route returns `TXN_TEST_EVIDENCE_UNSUPPORTED`. The runner checks that all
   accepted repository files remain unchanged by review and refusal. It does
   not remove tests, use `--untested`, or claim native admission.
5. **Handle a changed base.** In a third workspace, the maintainer commits a
   separate setup change, making `sentinel()` return `8`. Both the old-root
   authoring request and old candidate bytes refuse. The maintainer inspects
   the new base, authors the correction again, and repeats the tests. The
   resulting candidate preserves `sentinel() = 8`, has new bytes and a new
   proposed root, and requires new review. Nothing silently rebases approved
   bytes or advances the accepted head.

The initial and concurrent setup commits have no selected tests; they are not
presented as tested application commits. The correction's selected TestCases
run in advisory mode. This example establishes the finite workflow above,
not host resource enforcement, all-input correctness, independent human
adoption or an authoritative tested commit. Native qualification remains the
separate requirement in audit item 4.

## Put the packet in a pull request

Attach or commit the packet's portable files, starting reviewers at
`REVIEW.md`. Include the exact executable identity, base root, proposed root,
candidate-byte digest, change intent and test scope in the PR description.
The `changed` entries and views link the human explanation to the entities
actually proposed. Include failures and corrections, not just the final match.
Do not submit the runner's private `author/`, `reviewer/` or `maintainer/`
working directories as the review interface.

A reviewer can reproduce the packet with a fresh run, or inspect its exact
submitted candidate over its exact base in a new directory:

```sh
mkdir reviewer
cp packet/base.pack reviewer/base.pack
cp packet/names.json reviewer/names.json
sley-agent --workspace reviewer --json view
sley-agent --workspace reviewer --json submit packet/candidate.hex
sley-agent --workspace reviewer --json view --after c1 --ids --types
sley-agent --workspace reviewer --json test c1
```

Here `c1` is the first submission in that new reviewer workspace; in any
existing workspace use the handle returned by `submit`. Compare both roots
and candidate bytes with the packet. Hashes detect changed files against that
manifest; the unsigned runner report is not an independent attestation.

Before accepting a contribution, re-read the target's accepted root. A stale
root is a reason to inspect the intervening changes and generate a new proposal,
not to edit a digest inside old candidate bytes. Re-run relevant cases and
review the new proposed state. If both changes affect the same behavior or
interface, resolve the intended behavior explicitly instead of assuming the
disjoint-change example covers that conflict. Scope guards are per-command
authoring constraints, not persistent capability grants.

## Recorded run

[The checked-in packet](examples/contributor-review-2.0.6/REVIEW.md) was produced
by copying the runner and unpacked release binary into a fresh directory outside
the source checkout, then running all **30 commands** with a minimal environment.
The reviewer reproduced three matches; both stale paths and the native-evidence
boundary refused as expected. Its `result.json` binds the runner, executable,
roots and all portable files. This was a fresh working environment on the
existing host, not a new OS or a third-party adoption trial. The runner and
example are new; they reuse the published workbench and its existing contracts.
