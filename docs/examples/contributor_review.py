#!/usr/bin/env python3
"""Build and replay a root-bound review packet in fresh sley-agent workspaces.

Uses only Python's standard library and an explicitly identified executable.
Creates new local example workspaces; never contacts GitHub or a supervisor.
"""

from __future__ import annotations

import argparse
import difflib
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


RELEASE_AGENT_SHA256 = "7737c4f09c7236555155c25a967b4d52a8fb8f815464e7bcb33935e12e2728c5"


def require(condition: bool, detail: str) -> None:
    if not condition:
        raise RuntimeError(detail)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def files_under(path: Path) -> dict[str, str]:
    return {str(p.relative_to(path)): digest(p.read_bytes())
            for p in sorted(path.rglob("*")) if p.is_file()}


def function(name: str, expression: object) -> dict:
    return {"fn": name, "params": [["x", "i64"]] if name == "scale" else [],
            "returns": "i64", "body": [["return", expression]]}


def frame(expression: object, *, tests: bool = False) -> dict:
    result = {"af1": 1, "afx": 1, "fns": [function("scale", expression)]}
    if tests:
        result["tests"] = [{"name": name, "fn": "scale", "args": [x], "expect": expected}
                           for name, x, expected in [("negative", -3, -6),
                                                     ("zero", 0, 0), ("positive", 4, 8)]]
    return result


class Example:
    def __init__(self, agent: Path, output: Path):
        self.agent = agent
        self.output = output
        self.events: list[dict] = []

    def run(self, name: str, workspace: str, *args: str, expected: int = 0) -> dict:
        command = ["--workspace", workspace, "--json", *args]
        event = {"name": name, "argv": ["sley-agent", *command], "status": "started"}
        self.events.append(event)
        write_json(self.output / "transcript.json", self.events)
        result = subprocess.run([str(self.agent), *command], cwd=self.output,
                                env={"PATH": os.defpath, "LANG": "C.UTF-8"},
                                capture_output=True, text=True, timeout=30, check=False)
        event.update(status="completed", exit_code=result.returncode,
                     stdout=result.stdout, stderr=result.stderr)
        write_json(self.output / "transcript.json", self.events)
        require(result.returncode == expected,
                f"{name}: expected exit {expected}, got {result.returncode}; see transcript.json")
        value = json.loads(result.stdout)
        require(isinstance(value, dict), f"{name}: expected a JSON object")
        write_json(self.output / "observations" / f"{name}.result.json", value)
        return value

    def root(self, name: str, workspace: str) -> str:
        return self.run(name, workspace, "view")["root"]

    def clone(self, name: str, pack: str, names: Path) -> None:
        workspace = self.output / name
        workspace.mkdir()
        shutil.copyfile(self.output / pack, workspace / "base.pack")
        shutil.copyfile(names, workspace / "names.json")


def demonstrate(agent: Path, output: Path) -> dict:
    example = Example(agent, output)
    (output / "observations").mkdir()
    (output / "inputs").mkdir()
    inputs = {
        "initial.json": {"af1": 1, "afx": 1,
                         "fns": [function("scale", "x"), function("sentinel", 7)]},
        "wrong.json": frame(["add", "x", 2], tests=True),
        "corrected.json": frame(["mul", "x", 2], tests=True),
        "concurrent.json": {"af1": 1, "afx": 1, "fns": [function("sentinel", 8)]},
    }
    for name, value in inputs.items():
        write_json(output / "inputs" / name, value)

    # Build the base now, from a new genesis and ordinary authoring input.
    # This untested setup commit is not presented as a tested application.
    example.run("01-init", "author", "init", "--seed", "07" * 32)
    setup = example.run("02-author-base", "author", "try", "inputs/initial.json")
    example.run("03-commit-base", "author", "commit", setup["handle"])
    base = example.root("04-base-root", "author")
    before_view = example.run("05-base-view", "author", "view", "scale", "sentinel", "--ids", "--types")
    example.run("06-export-base", "author", "export", "base.pack")
    base_files = files_under(output / "author" / "repo")

    wrong = example.run("07-failing-proposal", "author", "try", "inputs/wrong.json",
                        "--base-root", base, expected=1)
    require(wrong["verdict"]["valid"] is True, "wrong arithmetic should be statically valid")
    require(any(not test["pass"] for test in wrong["tests"]), "expected behavioral failure")
    example.run("08-diagnose-view", "author", "view", "scale", "--after", wrong["handle"], "--ids")
    example.run("09-diagnose-test", "author", "test", wrong["handle"], expected=1)

    corrected = example.run("10-corrected-proposal", "author", "try", "inputs/corrected.json",
                            "--base-root", base, "--raw")
    require(corrected["verdict"]["valid"] is True, "corrected candidate must be valid")
    require(len(corrected["tests"]) == 3 and all(t["pass"] for t in corrected["tests"]),
            "all three authored cases must match")
    view = example.run("11-corrected-view", "author", "view", "scale", "sentinel", "--after", corrected["handle"],
                       "--ids", "--types")
    proposed = view["root"]
    require(proposed != base, "the correction must change the proposed root")
    example.run("12-submit", "author", "submit", corrected["handle"])
    candidate = output / "candidate.hex"
    shutil.copyfile(output / "author" / "final_candidate.hex", candidate)
    stored = bytes.fromhex(candidate.read_text().strip())
    require(stored == bytes.fromhex(corrected["stored_hex"]), "submission bytes changed")
    names = output / "names.json"
    shutil.copyfile(output / "author" / ".sley" / "names.json", names)
    require(example.root("13-author-still-base", "author") == base, "trial advanced the head")
    require(files_under(output / "author" / "repo") == base_files, "trial changed accepted files")

    # A fresh reviewer receives the exported base and exact submitted bytes,
    # not a copy of the author's candidate/draft cache.
    example.clone("reviewer", "base.pack", names)
    require(example.root("14-reviewer-base", "reviewer") == base, "reviewer base differs")
    reviewer_files = files_under(output / "reviewer" / "repo")
    replay = example.run("15-reviewer-replay", "reviewer", "submit", "candidate.hex")
    require(bytes.fromhex((output / "reviewer" / "final_candidate.hex").read_text().strip()) == stored,
            "reviewer did not replay exact bytes")
    replay_view = example.run("16-reviewer-view", "reviewer", "view", "--after", replay["submitted"],
                              "--ids", "--types")
    require(replay_view["root"] == proposed, "reviewed proposed root differs")
    observed = example.run("17-reviewer-test", "reviewer", "test", replay["submitted"])
    require(len(observed["tests"]) == 3 and all(t["pass"] for t in observed["tests"]),
            "reviewer cases must all match")
    refused = example.run("18-admission-boundary", "reviewer", "commit", replay["submitted"], expected=2)
    require("TXN_TEST_EVIDENCE_UNSUPPORTED" in json.dumps(refused), "unexpected commit refusal")
    require(files_under(output / "reviewer" / "repo") == reviewer_files,
            "advisory review or refused commit changed accepted files")

    # A separate maintainer workspace advances the base with a disjoint change.
    example.clone("maintainer", "base.pack", names)
    require(example.root("19-maintainer-base", "maintainer") == base, "maintainer base differs")
    concurrent = example.run("20-concurrent-change", "maintainer", "try", "inputs/concurrent.json",
                             "--base-root", base, "--functions", "sentinel")
    example.run("21-commit-concurrent", "maintainer", "commit", concurrent["handle"])
    fresh = example.root("22-fresh-root", "maintainer")
    require(fresh != base, "concurrent setup failed to advance the base")
    fresh_files = files_under(output / "maintainer" / "repo")
    stale = example.run("23-stale-proposal", "maintainer", "try", "inputs/corrected.json",
                        "--base-root", base, expected=2)
    require(stale.get("error") == "AGENT_PROPOSAL_STALE", "expected stale proposal refusal")
    stale_bytes = example.run("24-stale-candidate", "maintainer", "submit", "candidate.hex", expected=2)
    require(stale_bytes.get("error") == "AGENT_SUBMISSION_REFUSED"
            and "StaleRoot" in stale_bytes.get("detail", ""),
            "old bytes must not validate on the new base")
    require(files_under(output / "maintainer" / "repo") == fresh_files, "stale attempt changed the head")
    example.run("25-inspect-new-base", "maintainer", "view", "scale", "sentinel", "--ids", "--types")

    # Re-author against the new root; do not edit a root inside old bytes.
    rebased = example.run("26-reapply-correction", "maintainer", "try", "inputs/corrected.json",
                          "--base-root", fresh, "--raw")
    require(len(rebased["tests"]) == 3 and all(t["pass"] for t in rebased["tests"]),
            "fresh proposal cases must match")
    require(bytes.fromhex(rebased["stored_hex"]) != stored, "fresh proposal must have new bytes")
    new_view = example.run("27-new-review-view", "maintainer", "view", "--after", rebased["handle"],
                           "--ids", "--types")
    sentinel = example.run("28-preserved-concurrent-change", "maintainer", "call", "sentinel",
                           "--on", rebased["handle"])
    require(sentinel["result"] == 8, "correction lost the maintainer's change")
    example.run("29-submit-fresh", "maintainer", "submit", rebased["handle"])
    shutil.copyfile(output / "maintainer" / "final_candidate.hex", output / "rebased_candidate.hex")
    require(example.root("30-maintainer-still-fresh", "maintainer") == fresh,
            "fresh proposal was implicitly committed")
    require(files_under(output / "maintainer" / "repo") == fresh_files,
            "fresh review changed accepted files")
    (output / "before.av1.txt").write_text(before_view["view"], encoding="utf-8")
    (output / "after.av1.txt").write_text(view["view"], encoding="utf-8")
    (output / "review.diff").write_text("".join(difflib.unified_diff(
        before_view["view"].splitlines(keepends=True), view["view"].splitlines(keepends=True),
        fromfile="before.av1.txt", tofile="after.av1.txt")), encoding="utf-8")
    (output / "REVIEW.md").write_text(f"""# Review packet: make scale double its input

Base root: `{base}`. Proposed root: `{proposed}`.
Canonical candidate bytes SHA-256: `{digest(stored)}`.

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

The maintainer changed `sentinel()` to `8`, producing base `{fresh}`.
[The old-root authoring request](observations/23-stale-proposal.result.json) and
[the old candidate bytes](observations/24-stale-candidate.result.json) were refused.
After inspecting the new base, the correction was authored again and retested.
Its proposed root is `{new_view['root']}` and its bytes are
[rebased_candidate.hex](rebased_candidate.hex). The sentinel still returns `8`.
The earlier review does not cover these new bytes. No tested candidate was committed.

[Full command transcript](transcript.json) and [result with file hashes](result.json)
record the demonstration. Hashes bind files within this packet; this unsigned
runner report is not independent human review or native admission evidence.
""", encoding="utf-8")
    return {
        "result": "PASS", "contract": "sley-contributor-review-example-v1",
        "agent_sha256": digest(agent.read_bytes()), "base_root": base,
        "runner_sha256": digest(Path(__file__).read_bytes()),
        "reviewed_proposed_root": proposed, "candidate_stored_sha256": digest(stored),
        "reviewer_exact_byte_replay": True, "reviewer_cases_passed": 3,
        "accepted_repository_files_preserved_during_review": True,
        "stale_proposal_refused": True, "stale_candidate_refused": True,
        "fresh_base_root": fresh, "fresh_proposed_root": new_view["root"],
        "fresh_candidate_stored_sha256": digest(bytes.fromhex(rebased["stored_hex"])),
        "concurrent_sentinel_preserved": True, "tested_candidate_committed": False,
        "admission_refusal": "TXN_TEST_EVIDENCE_UNSUPPORTED",
        "steps": len(example.events),
        "artifacts": {str(p.relative_to(output)): digest(p.read_bytes())
                      for p in sorted(output.rglob("*")) if p.is_file()
                      and p.parts[len(output.parts)] not in {"author", "reviewer", "maintainer"}},
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--agent", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True, help="new directory; never reused")
    parser.add_argument("--expect-agent-sha256", default=RELEASE_AGENT_SHA256)
    args = parser.parse_args()
    agent = args.agent.resolve(strict=True)
    require(digest(agent.read_bytes()) == args.expect_agent_sha256, "agent SHA-256 mismatch")
    output = args.output.absolute()
    output.mkdir(parents=True, exist_ok=False)
    try:
        report = demonstrate(agent, output)
    except (RuntimeError, OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        write_json(output / "result.json", {"result": "FAIL", "detail": str(error)})
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    write_json(output / "result.json", report)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
