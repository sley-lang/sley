# Review packet: make scale double its input

Base root: `a6d3d55219be9a45d8977724ca2831b0d3793fbd53808d947b7a4c74cef0eeff`. Proposed root: `b2c5f5f382963a92c37226f09e5886e9e928990267c0b7636962fa075ae847d9`.
Canonical candidate bytes SHA-256: `6935374227a4ce4e61cea7587dffffa4790f1346c531b148c1e0dc3ab0eb4aa4`.

- Change: `scale(x)` returns `x * 2`; `sentinel()` stays `7`.
- [Authoring input and three TestCases](inputs/corrected.json).
- [Before](before.av1.txt), [after](after.av1.txt), [display diff](review.diff).
  AV1 and its diff are review aids, not canonical inputs.
- [Changed-entity descriptions and original test results](observations/10-corrected-proposal.result.json).
- [Failed arithmetic proposal](observations/07-failing-proposal.result.json) and
  [failure diagnosis](observations/09-diagnose-test.result.json).
- [Fresh review results](observations/17-reviewer-test.result.json) replay
  [candidate.hex](candidate.hex) over [base.pack](base.pack), with the optional
  display names in [names.json](names.json). The candidate has three selected TestCases.
- [Native-evidence refusal](observations/18-admission-boundary.result.json):
  advisory matches did not authorize a tested commit.

## Conflict requires new review

The maintainer changed `sentinel()` to `8`, producing base `6bbce34f77c28c12c505d38f1d869e4a46a2ea29f86d82a869f5bcf731393ec2`.
[The old-root authoring request](observations/23-stale-proposal.result.json) and
[the old candidate bytes](observations/24-stale-candidate.result.json) were refused.
After inspecting the new base, the correction was authored again and retested.
Its proposed root is `d3543e66783207d2cc241dbe915d45029d18efb102210a01908bf9d6f8a58d79` and its bytes are
[rebased_candidate.hex](rebased_candidate.hex). The sentinel still returns `8`.
The earlier review does not cover these new bytes. No tested candidate was committed.

[Full command transcript](transcript.json) and [result with file hashes](result.json)
record the demonstration. Hashes bind files within this packet; this unsigned
runner report is not independent human review or native admission evidence.
